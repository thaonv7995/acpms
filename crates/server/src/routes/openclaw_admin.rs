use crate::{
    api::ApiResponse,
    error::ApiError,
    middleware::{AuthUser, RbacChecker},
    AppState,
};
use acpms_services::{
    CreateOpenClawBootstrapTokenInput, OpenClawAdminClientSummary, OpenClawAdminService,
    OpenClawAdminServiceError,
};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn create_routes() -> Router<AppState> {
    Router::new()
        .route("/openclaw/clients", get(list_openclaw_clients))
        .route(
            "/openclaw/bootstrap-tokens",
            post(create_openclaw_bootstrap_token),
        )
        .route(
            "/openclaw/clients/:client_id/disable",
            post(disable_openclaw_client),
        )
        .route(
            "/openclaw/clients/:client_id/enable",
            post(enable_openclaw_client),
        )
        .route(
            "/openclaw/clients/:client_id/revoke",
            post(revoke_openclaw_client),
        )
        .route(
            "/openclaw/clients/:client_id/delete",
            post(delete_openclaw_client),
        )
}

fn ensure_openclaw_gateway_enabled(state: &AppState) -> Result<(), ApiError> {
    if !state.openclaw_gateway.enabled {
        return Err(ApiError::NotFound(
            "OpenClaw gateway is not enabled for this ACPMS installation".to_string(),
        ));
    }

    Ok(())
}

#[derive(Debug, Serialize)]
struct OpenClawClientsResponse {
    clients: Vec<OpenClawAdminClientSummary>,
}

#[derive(Debug, Serialize)]
struct OpenClawClientMutationResponse {
    client: OpenClawAdminClientSummary,
}

#[derive(Debug, Serialize)]
struct OpenClawClientDeleteResponse {
    deleted: OpenClawAdminClientSummary,
}

#[derive(Debug, Deserialize)]
struct CreateOpenClawBootstrapTokenRequest {
    label: String,
    expires_in_minutes: Option<i64>,
    suggested_display_name: Option<String>,
    metadata: Option<Value>,
}

#[derive(Debug, Serialize)]
struct OpenClawBootstrapPromptResponse {
    bootstrap_token_id: uuid::Uuid,
    expires_at: chrono::DateTime<chrono::Utc>,
    prompt_text: String,
    token_preview: String,
}

async fn list_openclaw_clients(
    auth_user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<OpenClawClientsResponse>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;
    ensure_openclaw_gateway_enabled(&state)?;

    let service = OpenClawAdminService::new(state.db.clone());
    let clients = service
        .list_clients()
        .await
        .map_err(map_admin_service_error)?;

    Ok(Json(ApiResponse::success(
        OpenClawClientsResponse { clients },
        "OpenClaw clients retrieved successfully",
    )))
}

async fn create_openclaw_bootstrap_token(
    auth_user: AuthUser,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateOpenClawBootstrapTokenRequest>,
) -> Result<Json<ApiResponse<OpenClawBootstrapPromptResponse>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;
    ensure_openclaw_gateway_enabled(&state)?;

    let label = payload.label.trim();
    if label.is_empty() {
        return Err(ApiError::BadRequest(
            "Bootstrap token label must not be empty".to_string(),
        ));
    }

    let service = OpenClawAdminService::new(state.db.clone());
    let prompt = service
        .create_bootstrap_token(
            CreateOpenClawBootstrapTokenInput {
                label: label.to_string(),
                expires_in_minutes: payload.expires_in_minutes.unwrap_or(15),
                suggested_display_name: payload.suggested_display_name,
                metadata: payload.metadata,
                created_by: Some(auth_user.id),
            },
            &infer_public_base_url(&headers),
            state.openclaw_gateway.api_key.as_deref(),
            state.openclaw_gateway.webhook_secret.as_deref(),
        )
        .await
        .map_err(map_admin_service_error)?;

    Ok(Json(ApiResponse::success(
        OpenClawBootstrapPromptResponse {
            bootstrap_token_id: prompt.bootstrap_token_id,
            expires_at: prompt.expires_at,
            prompt_text: prompt.prompt_text,
            token_preview: prompt.token_preview,
        },
        "Bootstrap prompt generated successfully",
    )))
}

async fn disable_openclaw_client(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(client_id): Path<String>,
) -> Result<Json<ApiResponse<OpenClawClientMutationResponse>>, ApiError> {
    mutate_client_status(auth_user, state, &client_id, ClientMutationKind::Disable).await
}

async fn enable_openclaw_client(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(client_id): Path<String>,
) -> Result<Json<ApiResponse<OpenClawClientMutationResponse>>, ApiError> {
    mutate_client_status(auth_user, state, &client_id, ClientMutationKind::Enable).await
}

async fn revoke_openclaw_client(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(client_id): Path<String>,
) -> Result<Json<ApiResponse<OpenClawClientMutationResponse>>, ApiError> {
    mutate_client_status(auth_user, state, &client_id, ClientMutationKind::Revoke).await
}

async fn delete_openclaw_client(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(client_id): Path<String>,
) -> Result<Json<ApiResponse<OpenClawClientDeleteResponse>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;
    ensure_openclaw_gateway_enabled(&state)?;

    let service = OpenClawAdminService::new(state.db.clone());
    let deleted = service
        .delete_client(&client_id)
        .await
        .map_err(map_admin_service_error)?;

    Ok(Json(ApiResponse::success(
        OpenClawClientDeleteResponse { deleted },
        "OpenClaw installation deleted successfully",
    )))
}

enum ClientMutationKind {
    Disable,
    Enable,
    Revoke,
}

async fn mutate_client_status(
    auth_user: AuthUser,
    state: AppState,
    client_id: &str,
    mutation_kind: ClientMutationKind,
) -> Result<Json<ApiResponse<OpenClawClientMutationResponse>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;
    ensure_openclaw_gateway_enabled(&state)?;

    let service = OpenClawAdminService::new(state.db.clone());
    let (client, message) = match mutation_kind {
        ClientMutationKind::Disable => (
            service
                .disable_client(client_id)
                .await
                .map_err(map_admin_service_error)?,
            "OpenClaw client disabled successfully",
        ),
        ClientMutationKind::Enable => (
            service
                .enable_client(client_id)
                .await
                .map_err(map_admin_service_error)?,
            "OpenClaw client enabled successfully",
        ),
        ClientMutationKind::Revoke => (
            service
                .revoke_client(client_id)
                .await
                .map_err(map_admin_service_error)?,
            "OpenClaw client revoked successfully",
        ),
    };

    Ok(Json(ApiResponse::success(
        OpenClawClientMutationResponse { client },
        message,
    )))
}

fn map_admin_service_error(error: OpenClawAdminServiceError) -> ApiError {
    match error {
        OpenClawAdminServiceError::InvalidBootstrapToken => ApiError::Unauthorized,
        OpenClawAdminServiceError::ClientNotFound(message) => ApiError::NotFound(message),
        OpenClawAdminServiceError::RevokedClient(message) => ApiError::Conflict(message),
        OpenClawAdminServiceError::Internal(_, message) => ApiError::Internal(message),
    }
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn infer_public_base_url(headers: &HeaderMap) -> String {
    if let Ok(value) = std::env::var("ACPMS_PUBLIC_URL") {
        let trimmed = value.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let host = header_value(headers, "x-forwarded-host")
        .or_else(|| header_value(headers, "host"))
        .unwrap_or_else(|| "localhost:3000".to_string());
    let proto = header_value(headers, "x-forwarded-proto").unwrap_or_else(|| {
        if host.starts_with("localhost") || host.starts_with("127.0.0.1") {
            "http".to_string()
        } else {
            "https".to_string()
        }
    });

    format!("{}://{}", proto, host)
}
