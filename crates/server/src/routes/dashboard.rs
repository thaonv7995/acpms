use acpms_services::{dashboard::DashboardData, DashboardService};
use axum::{extract::State, Json};

use crate::{
    api::ApiResponse,
    error::{ApiError, ApiResult},
    middleware::auth::AuthUser,
    AppState,
};

#[utoipa::path(
    get,
    path = "/api/v1/dashboard",
    tag = "Dashboard",
    responses(
        (status = 200, description = "Get dashboard data", body = DashboardResponse),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_dashboard(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> ApiResult<Json<ApiResponse<DashboardData>>> {
    let service = DashboardService::new(state.db.clone());
    let data = service
        .get_dashboard_data(auth_user.id, Some(state.storage_service.clone()))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = ApiResponse::success(data, "Dashboard data retrieved successfully");

    Ok(Json(response))
}
