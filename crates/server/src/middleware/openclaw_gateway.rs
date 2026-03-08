use axum::{
    extract::State,
    http::{header, HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::{
    error::ApiError,
    middleware::AuthUser,
    state::{AppState, OpenClawGatewayConfig},
};
use acpms_db::models::SystemRole;

const OPENCLAW_SERVICE_USER_EMAIL: &str = "openclaw-gateway@acpms.local";
const OPENCLAW_SERVICE_USER_NAME: &str = "OpenClaw Gateway";

fn default_openclaw_service_user_id() -> Uuid {
    Uuid::from_u128(0x6a962b11c7df4b5d8f31e1cb7606aa10)
}

fn extract_bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .ok_or(ApiError::Unauthorized)?
        .to_str()
        .map_err(|_| ApiError::Unauthorized)?;

    let token = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .ok_or(ApiError::Unauthorized)?
        .trim();

    if token.is_empty() {
        return Err(ApiError::Unauthorized);
    }

    Ok(token)
}

fn ensure_gateway_enabled(config: &OpenClawGatewayConfig) -> Result<(), ApiError> {
    if !config.enabled {
        return Err(ApiError::Forbidden(
            "OpenClaw gateway is disabled".to_string(),
        ));
    }

    if config.api_key.is_none() {
        return Err(ApiError::Forbidden(
            "OpenClaw gateway is not configured".to_string(),
        ));
    }

    Ok(())
}

async fn resolve_actor_user_id(state: &AppState) -> Result<Uuid, ApiError> {
    let desired_user_id = state
        .openclaw_gateway
        .actor_user_id
        .unwrap_or_else(default_openclaw_service_user_id);

    if let Some(existing_email) = sqlx::query_scalar::<_, String>(
        r#"
        SELECT email
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(desired_user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(ApiError::Database)?
    {
        if existing_email != OPENCLAW_SERVICE_USER_EMAIL {
            return Err(ApiError::Internal(
                "OPENCLAW_ACTOR_USER_ID must reference the dedicated OpenClaw service principal"
                    .to_string(),
            ));
        }
    }

    sqlx::query_scalar(
        r#"
        INSERT INTO users (id, email, name, password_hash, global_roles)
        VALUES ($1, $2, $3, NULL, $4)
        ON CONFLICT (email) DO UPDATE
        SET
            name = EXCLUDED.name,
            global_roles = (
                SELECT ARRAY(
                    SELECT DISTINCT role_value
                    FROM unnest(users.global_roles || EXCLUDED.global_roles) AS role_value
                )
            ),
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(desired_user_id)
    .bind(OPENCLAW_SERVICE_USER_EMAIL)
    .bind(OPENCLAW_SERVICE_USER_NAME)
    .bind(vec![SystemRole::Admin])
    .fetch_one(&state.db)
    .await
    .map_err(|error| {
        ApiError::Internal(format!(
            "OpenClaw gateway could not provision its service principal: {error}"
        ))
    })
}

pub async fn authenticate_openclaw_token(
    state: &AppState,
    token: &str,
) -> Result<AuthUser, ApiError> {
    if let Err(error) = ensure_gateway_enabled(&state.openclaw_gateway) {
        state
            .metrics
            .openclaw_gateway_auth_total
            .with_label_values(&["forbidden"])
            .inc();
        return Err(error);
    }

    let expected = state
        .openclaw_gateway
        .api_key
        .as_deref()
        .ok_or_else(|| ApiError::Forbidden("OpenClaw gateway is not configured".to_string()))?;

    if token != expected {
        state
            .metrics
            .openclaw_gateway_auth_total
            .with_label_values(&["unauthorized"])
            .inc();
        return Err(ApiError::Unauthorized);
    }

    let actor_user_id = match resolve_actor_user_id(state).await {
        Ok(actor_user_id) => actor_user_id,
        Err(error) => {
            state
                .metrics
                .openclaw_gateway_auth_total
                .with_label_values(&["internal_error"])
                .inc();
            return Err(error);
        }
    };
    state
        .metrics
        .openclaw_gateway_auth_total
        .with_label_values(&["success"])
        .inc();
    Ok(AuthUser {
        id: actor_user_id,
        jti: "openclaw-gateway".to_string(),
    })
}

pub async fn authenticate_openclaw_request(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthUser, ApiError> {
    let token = extract_bearer_token(headers)?;
    authenticate_openclaw_token(state, token).await
}

pub async fn require_openclaw_auth(
    State(state): State<AppState>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let path = request.uri().path().to_string();
    let user_agent = request
        .headers()
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let forwarded_for = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    let auth_user = match authenticate_openclaw_request(&state, request.headers()).await {
        Ok(auth_user) => auth_user,
        Err(error) => {
            tracing::warn!(
                path,
                user_agent = user_agent.as_deref().unwrap_or("-"),
                forwarded_for = forwarded_for.as_deref().unwrap_or("-"),
                error = %error,
                "OpenClaw gateway authentication failed"
            );
            return Err(error);
        }
    };

    tracing::info!(
        path,
        user_agent = user_agent.as_deref().unwrap_or("-"),
        forwarded_for = forwarded_for.as_deref().unwrap_or("-"),
        actor_user_id = %auth_user.id,
        "OpenClaw gateway request authenticated"
    );
    request.extensions_mut().insert(auth_user);
    Ok(next.run(request).await)
}
