use axum::{
    extract::State,
    http::{header, HeaderMap, Request},
    middleware::Next,
    response::Response,
};

use crate::{
    error::ApiError,
    middleware::AuthUser,
    state::{AppState, OpenClawGatewayConfig},
};

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

async fn resolve_actor_user_id(state: &AppState) -> Result<uuid::Uuid, ApiError> {
    if let Some(user_id) = state.openclaw_gateway.actor_user_id {
        return Ok(user_id);
    }

    sqlx::query_scalar(
        r#"
        SELECT id
        FROM users
        WHERE global_roles @> ARRAY['admin']::system_role[]
        ORDER BY created_at ASC
        LIMIT 1
        "#,
    )
    .fetch_optional(&state.db)
    .await
    .map_err(ApiError::Database)?
    .ok_or_else(|| {
        ApiError::Internal(
            "OpenClaw gateway could not resolve a system admin actor user".to_string(),
        )
    })
}

pub async fn authenticate_openclaw_request(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthUser, ApiError> {
    ensure_gateway_enabled(&state.openclaw_gateway)?;

    let token = extract_bearer_token(headers)?;
    let expected = state
        .openclaw_gateway
        .api_key
        .as_deref()
        .ok_or_else(|| ApiError::Forbidden("OpenClaw gateway is not configured".to_string()))?;

    if token != expected {
        return Err(ApiError::Unauthorized);
    }

    let actor_user_id = resolve_actor_user_id(state).await?;
    Ok(AuthUser {
        id: actor_user_id,
        jti: "openclaw-gateway".to_string(),
    })
}

pub async fn require_openclaw_auth(
    State(state): State<AppState>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let auth_user = authenticate_openclaw_request(&state, request.headers()).await?;
    request.extensions_mut().insert(auth_user);
    Ok(next.run(request).await)
}
