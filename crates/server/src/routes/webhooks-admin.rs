use crate::api::ApiResponse;
use crate::error::ApiError;
use crate::middleware::{AuthUser, RbacChecker};
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn create_routes() -> Router<AppState> {
    Router::new()
        .route("/webhooks/failed", get(get_failed_webhooks))
        .route("/webhooks/:id/retry", post(retry_webhook))
        .route("/webhooks/stats", get(get_webhook_stats))
        .route(
            "/openclaw-webhooks/failed",
            get(get_failed_openclaw_webhooks),
        )
        .route("/openclaw-webhooks/:id/retry", post(retry_openclaw_webhook))
        .route("/openclaw-webhooks/stats", get(get_openclaw_webhook_stats))
}

#[derive(Debug, Deserialize)]
struct FailedWebhooksQuery {
    project_id: Option<Uuid>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
struct FailedWebhookDto {
    id: Uuid,
    project_id: Uuid,
    event_id: String,
    event_type: String,
    attempt_count: i32,
    last_error: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_attempt_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Get failed webhook events (dead letter queue)
///
/// ## Admin Only
/// Requires admin role to access
#[utoipa::path(
    get,
    path = "/api/v1/admin/webhooks/failed",
    tag = "Webhooks Admin",
    params(
        ("project_id" = Option<Uuid>, Query, description = "Filter by project ID"),
        ("limit" = Option<i64>, Query, description = "Max results (default: 50)")
    ),
    responses(
        (status = 200, description = "Failed webhooks retrieved"),
        (status = 403, description = "Forbidden - admin only")
    )
)]
async fn get_failed_webhooks(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<FailedWebhooksQuery>,
) -> Result<Json<ApiResponse<Vec<FailedWebhookDto>>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    let limit = query.limit.unwrap_or(50).min(200); // Cap at 200

    let failed_events = state
        .webhook_admin_service
        .get_failed_events(query.project_id, limit)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch failed webhooks: {}", e)))?;

    let dtos: Vec<FailedWebhookDto> = failed_events
        .into_iter()
        .map(|e| FailedWebhookDto {
            id: e.id,
            project_id: e.project_id,
            event_id: e.event_id,
            event_type: e.event_type,
            attempt_count: e.attempt_count,
            last_error: e.last_error,
            created_at: e.created_at,
            last_attempt_at: e.last_attempt_at,
        })
        .collect();

    let response = ApiResponse::success(dtos, "Failed webhooks retrieved successfully");
    Ok(Json(response))
}

/// Retry a failed webhook event
///
/// Resets status to 'pending' and attempt count to 0, allowing reprocessing
#[utoipa::path(
    post,
    path = "/api/v1/admin/webhooks/{id}/retry",
    tag = "Webhooks Admin",
    params(
        ("id" = Uuid, Path, description = "Webhook event ID")
    ),
    responses(
        (status = 200, description = "Webhook queued for retry"),
        (status = 403, description = "Forbidden - admin only"),
        (status = 404, description = "Webhook event not found")
    )
)]
async fn retry_webhook(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    state
        .webhook_admin_service
        .retry_event(event_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to retry webhook: {}", e)))?;

    let response = ApiResponse::success((), "Webhook queued for retry");
    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
struct WebhookStatsQuery {
    project_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct WebhookStatsDto {
    pending: i64,
    processing: i64,
    completed: i64,
    failed: i64,
}

#[derive(Debug, Serialize)]
struct FailedOpenClawWebhookDto {
    id: Uuid,
    event_sequence_id: i64,
    event_type: String,
    attempt_count: i32,
    max_attempts: i32,
    last_status_code: Option<i32>,
    last_error: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_attempt_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Get webhook processing statistics
#[utoipa::path(
    get,
    path = "/api/v1/admin/webhooks/stats",
    tag = "Webhooks Admin",
    params(
        ("project_id" = Option<Uuid>, Query, description = "Filter by project ID")
    ),
    responses(
        (status = 200, description = "Webhook statistics retrieved"),
        (status = 403, description = "Forbidden - admin only")
    )
)]
async fn get_webhook_stats(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<WebhookStatsQuery>,
) -> Result<Json<ApiResponse<WebhookStatsDto>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    let stats = state
        .webhook_admin_service
        .get_stats(query.project_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch webhook stats: {}", e)))?;

    let dto = WebhookStatsDto {
        pending: stats.pending,
        processing: stats.processing,
        completed: stats.completed,
        failed: stats.failed,
    };

    let response = ApiResponse::success(dto, "Webhook statistics retrieved successfully");
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/openclaw-webhooks/failed",
    tag = "Webhooks Admin",
    params(
        ("limit" = Option<i64>, Query, description = "Max results (default: 50)")
    ),
    responses(
        (status = 200, description = "Failed OpenClaw webhooks retrieved"),
        (status = 403, description = "Forbidden - admin only")
    )
)]
async fn get_failed_openclaw_webhooks(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<FailedWebhooksQuery>,
) -> Result<Json<ApiResponse<Vec<FailedOpenClawWebhookDto>>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    let limit = query.limit.unwrap_or(50).min(200);
    let failed_deliveries = state
        .openclaw_event_service
        .get_failed_webhook_deliveries(limit)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch OpenClaw webhooks: {}", e)))?;

    let response = ApiResponse::success(
        failed_deliveries
            .into_iter()
            .map(|delivery| FailedOpenClawWebhookDto {
                id: delivery.id,
                event_sequence_id: delivery.event_sequence_id,
                event_type: delivery.event_type,
                attempt_count: delivery.attempt_count,
                max_attempts: delivery.max_attempts,
                last_status_code: delivery.last_status_code,
                last_error: delivery.last_error,
                created_at: delivery.created_at,
                last_attempt_at: delivery.last_attempt_at,
            })
            .collect(),
        "Failed OpenClaw webhooks retrieved successfully",
    );
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/openclaw-webhooks/{id}/retry",
    tag = "Webhooks Admin",
    params(
        ("id" = Uuid, Path, description = "OpenClaw webhook delivery ID")
    ),
    responses(
        (status = 200, description = "OpenClaw webhook queued for retry"),
        (status = 403, description = "Forbidden - admin only")
    )
)]
async fn retry_openclaw_webhook(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(delivery_id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    state
        .openclaw_event_service
        .retry_failed_webhook_delivery(delivery_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to retry OpenClaw webhook: {}", e)))?;

    let response = ApiResponse::success((), "OpenClaw webhook queued for retry");
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/openclaw-webhooks/stats",
    tag = "Webhooks Admin",
    responses(
        (status = 200, description = "OpenClaw webhook statistics retrieved"),
        (status = 403, description = "Forbidden - admin only")
    )
)]
async fn get_openclaw_webhook_stats(
    auth_user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<WebhookStatsDto>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    let stats = state
        .openclaw_event_service
        .webhook_delivery_stats()
        .await
        .map_err(|e| {
            ApiError::Internal(format!(
                "Failed to fetch OpenClaw webhook statistics: {}",
                e
            ))
        })?;

    let response = ApiResponse::success(
        WebhookStatsDto {
            pending: stats.pending,
            processing: stats.processing,
            completed: stats.completed,
            failed: stats.failed,
        },
        "OpenClaw webhook statistics retrieved successfully",
    );
    Ok(Json(response))
}
