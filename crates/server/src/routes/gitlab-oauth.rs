use crate::api::ApiResponse;
use crate::error::ApiError;
use crate::middleware::{AuthUser, Permission, RbacChecker};
use crate::state::AppState;
use axum::{
    extract::{Query, State},
    response::Redirect,
    routing::get,
    Json, Router,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn create_routes() -> Router<AppState> {
    Router::new()
        .route("/gitlab/oauth/authorize", get(authorize))
        .route("/gitlab/oauth/callback", get(callback))
}

#[derive(Debug, Deserialize)]
struct AuthorizeQuery {
    project_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OAuthStateClaims {
    sub: String,
    project_id: Option<String>,
    exp: usize,
    iat: usize,
}

fn encode_oauth_state(user_id: Uuid, project_id: Option<Uuid>) -> Result<String, ApiError> {
    let secret = std::env::var("JWT_SECRET").map_err(|_| {
        ApiError::Internal("JWT_SECRET environment variable must be set".to_string())
    })?;
    let now = Utc::now();
    let exp = (now + Duration::minutes(10)).timestamp() as usize;
    let claims = OAuthStateClaims {
        sub: user_id.to_string(),
        project_id: project_id.map(|id| id.to_string()),
        exp,
        iat: now.timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| ApiError::BadRequest(format!("Failed to create OAuth state: {}", e)))
}

fn decode_oauth_state(state_token: &str) -> Result<(Uuid, Option<Uuid>), ApiError> {
    let secret = std::env::var("JWT_SECRET").map_err(|_| {
        ApiError::Internal("JWT_SECRET environment variable must be set".to_string())
    })?;

    let token_data = decode::<OAuthStateClaims>(
        state_token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| ApiError::BadRequest("Invalid or expired OAuth state token".to_string()))?;

    let user_id = Uuid::parse_str(&token_data.claims.sub)
        .map_err(|_| ApiError::BadRequest("Invalid OAuth state user id".to_string()))?;

    let project_id = token_data
        .claims
        .project_id
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| ApiError::BadRequest("Invalid OAuth state project id".to_string()))?;

    Ok((user_id, project_id))
}

/// Initiate GitLab OAuth flow
///
/// Generates authorization URL and redirects user to GitLab
#[utoipa::path(
    get,
    path = "/api/v1/gitlab/oauth/authorize",
    tag = "GitLab OAuth",
    security(("bearer_auth" = [])),
    params(
        ("project_id" = Option<Uuid>, Query, description = "Optional project ID to link OAuth token")
    ),
    responses(
        (status = 302, description = "Redirect to GitLab authorization page"),
        (status = 401, description = "Unauthorized - user not authenticated")
    )
)]
async fn authorize(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<AuthorizeQuery>,
) -> Result<Redirect, ApiError> {
    if let Some(project_id) = query.project_id {
        RbacChecker::check_permission(
            auth_user.id,
            project_id,
            Permission::ManageProject,
            &state.db,
        )
        .await?;
    }

    // Signed short-lived state token to bind callback to authenticated user context.
    let state_param = encode_oauth_state(auth_user.id, query.project_id)?;

    // Generate authorization URL
    let auth_url = state
        .gitlab_oauth_service
        .get_authorization_url(&state_param, Some("api read_user"));

    Ok(Redirect::to(&auth_url))
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: String,
    state: String,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Serialize)]
struct OAuthCallbackResponse {
    success: bool,
    gitlab_user_id: i64,
    gitlab_username: String,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Handle GitLab OAuth callback
///
/// Exchanges authorization code for access token and stores encrypted
#[utoipa::path(
    get,
    path = "/api/v1/gitlab/oauth/callback",
    tag = "GitLab OAuth",
    params(
        ("code" = String, Query, description = "Authorization code from GitLab"),
        ("state" = String, Query, description = "CSRF protection state token")
    ),
    responses(
        (status = 200, description = "OAuth flow completed successfully"),
        (status = 400, description = "OAuth error or invalid state"),
        (status = 401, description = "Unauthorized")
    )
)]
async fn callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Result<Json<ApiResponse<OAuthCallbackResponse>>, ApiError> {
    // Check for OAuth errors
    if let Some(error) = query.error {
        let description = query
            .error_description
            .unwrap_or_else(|| "Unknown error".to_string());
        return Err(ApiError::BadRequest(format!(
            "OAuth error: {} - {}",
            error, description
        )));
    }

    let (user_id, project_id) = decode_oauth_state(&query.state)?;
    if let Some(project_id) = project_id {
        RbacChecker::check_permission(user_id, project_id, Permission::ManageProject, &state.db)
            .await?;
    }

    // Exchange code for token
    let token = state
        .gitlab_oauth_service
        .exchange_code(&query.code, user_id, project_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to exchange OAuth code: {}", e)))?;

    // If project_id is set, trigger automatic project sync
    if let Some(pid) = project_id {
        // TODO: Queue background job to sync GitLab projects
        tracing::info!("OAuth completed for project {}, triggering auto-sync", pid);
    }

    let response_data = OAuthCallbackResponse {
        success: true,
        gitlab_user_id: token.gitlab_user_id,
        gitlab_username: token.gitlab_username,
        expires_at: token.expires_at,
    };

    let response = ApiResponse::success(response_data, "GitLab OAuth completed successfully");
    Ok(Json(response))
}
