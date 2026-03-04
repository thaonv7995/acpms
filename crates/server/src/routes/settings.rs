use crate::middleware::{AuthUser, RbacChecker};
use crate::routes::agent;
use crate::{api::ApiResponse, error::ApiError, AppState};
use acpms_db::models::{SystemSettingsResponse, UpdateSystemSettingsRequest};
use axum::{extract::State, routing::get, Json, Router};
use std::path::PathBuf;

pub fn create_routes() -> Router<AppState> {
    Router::new().route("/settings", get(get_settings).put(update_settings))
}

/// Get current system settings (safe DTO)
async fn get_settings(
    auth_user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<SystemSettingsResponse>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    let settings = state
        .settings_service
        .get_response()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(ApiResponse::success(
        settings,
        "Settings retrieved successfully",
    )))
}

#[utoipa::path(
    put,
    path = "/api/v1/settings",
    request_body = UpdateSystemSettingsRequest,
    responses(
        (status = 200, description = "Update system settings", body = ApiResponse<SystemSettingsResponse>),
    ),
    tag = "Settings"
)]
pub async fn update_settings(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Json(mut payload): Json<UpdateSystemSettingsRequest>,
) -> Result<Json<ApiResponse<SystemSettingsResponse>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    if let Some(provider) = payload.agent_cli_provider.take() {
        let normalized = agent::normalize_agent_cli_provider(&provider);
        let status = agent::check_provider_status(&normalized).await;
        if !status.available {
            return Err(ApiError::BadRequest(format!(
                "Cannot set default provider '{}' because it is not available: {}",
                normalized, status.message
            )));
        }
        payload.agent_cli_provider = Some(normalized);
    }

    let had_worktrees_update = payload.worktrees_path.is_some();
    let settings = state
        .settings_service
        .update(payload)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Sync worktrees_path to in-memory state so it applies immediately (no restart)
    if had_worktrees_update {
        let new_path = PathBuf::from(&settings.worktrees_path);
        *state.worktrees_path.write().await = new_path;
    }

    Ok(Json(ApiResponse::success(
        settings,
        "Settings updated successfully",
    )))
}
