//! API routes for tool approval workflow (SDK mode)

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::api::ApiResponse;
use crate::error::{ApiError, ApiResult};
use crate::middleware::{AuthUser, Permission, RbacChecker};
use crate::AppState;

/// Tool approval DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ToolApprovalDto {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub execution_process_id: Option<Uuid>,
    pub tool_use_id: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Approval decision request
#[derive(Debug, Deserialize, ToSchema)]
pub struct ApprovalDecisionRequest {
    pub decision: ApprovalDecision,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Approval decision enum
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalDecision {
    Approve,
    Deny,
}

#[derive(Debug, Clone, FromRow)]
struct ApprovalContextRow {
    id: Uuid,
    tool_use_id: String,
    project_id: Uuid,
}

#[derive(Debug, Clone, FromRow)]
struct ProcessContextRow {
    project_id: Uuid,
}

type PendingApprovalRow = (
    Uuid,
    Uuid,
    Option<Uuid>,
    String,
    String,
    serde_json::Value,
    String,
    chrono::DateTime<chrono::Utc>,
);

fn map_pending_approval_row_to_dto(
    (
        id,
        attempt_id,
        execution_process_id,
        tool_use_id,
        tool_name,
        tool_input,
        status,
        created_at,
    ): PendingApprovalRow,
) -> ToolApprovalDto {
    ToolApprovalDto {
        id,
        attempt_id,
        execution_process_id,
        tool_use_id,
        tool_name,
        tool_input,
        status,
        created_at,
    }
}

async fn resolve_approval_context(
    pool: &sqlx::PgPool,
    approval_ref: &str,
) -> Result<Option<ApprovalContextRow>, sqlx::Error> {
    if let Ok(approval_id) = Uuid::parse_str(approval_ref) {
        let approval_by_id: Option<ApprovalContextRow> = sqlx::query_as(
            r#"
            SELECT ta.id, ta.tool_use_id, t.project_id
            FROM tool_approvals ta
            JOIN task_attempts att ON ta.attempt_id = att.id
            JOIN tasks t ON att.task_id = t.id
            WHERE ta.id = $1
            "#,
        )
        .bind(approval_id)
        .fetch_optional(pool)
        .await?;

        if approval_by_id.is_some() {
            return Ok(approval_by_id);
        }
    }

    sqlx::query_as(
        r#"
        SELECT ta.id, ta.tool_use_id, t.project_id
        FROM tool_approvals ta
        JOIN task_attempts att ON ta.attempt_id = att.id
        JOIN tasks t ON att.task_id = t.id
        WHERE ta.tool_use_id = $1
        "#,
    )
    .bind(approval_ref)
    .fetch_optional(pool)
    .await
}

async fn resolve_process_context(
    pool: &sqlx::PgPool,
    process_id: Uuid,
) -> Result<Option<ProcessContextRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            t.project_id
        FROM execution_processes ep
        JOIN task_attempts ta ON ta.id = ep.attempt_id
        JOIN tasks t ON t.id = ta.task_id
        WHERE ep.id = $1
        "#,
    )
    .bind(process_id)
    .fetch_optional(pool)
    .await
}

/// Get pending approvals for a specific execution process
#[utoipa::path(
    get,
    path = "/api/v1/execution-processes/{id}/approvals/pending",
    tag = "Approvals",
    params(
        ("id" = Uuid, Path, description = "Execution process ID")
    ),
    responses(
        (status = 200, description = "Pending approvals retrieved", body = Vec<ToolApprovalDto>),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Execution process not found")
    )
)]
pub async fn get_pending_approvals_for_process(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(process_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<ToolApprovalDto>>>> {
    let pool = state.db.clone();
    let process_ctx = resolve_process_context(&pool, process_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Execution process not found".to_string()))?;

    RbacChecker::check_permission(
        auth_user.id,
        process_ctx.project_id,
        Permission::ViewTask,
        &pool,
    )
    .await?;

    let approvals: Vec<PendingApprovalRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            attempt_id,
            execution_process_id,
            tool_use_id,
            tool_name,
            tool_input,
            status::text as status,
            created_at
        FROM tool_approvals
        WHERE status = 'pending'::approval_status
          AND execution_process_id = $1
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .bind(process_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dtos: Vec<ToolApprovalDto> = approvals
        .into_iter()
        .map(map_pending_approval_row_to_dto)
        .collect();

    Ok(Json(ApiResponse::success(
        dtos,
        "Pending approvals for execution process retrieved successfully",
    )))
}

/// Respond to a tool approval request
#[utoipa::path(
    post,
    path = "/api/v1/approvals/{approval_ref}/respond",
    tag = "Approvals",
    params(
        ("approval_ref" = String, Path, description = "Approval ID (UUID) or legacy tool use ID")
    ),
    request_body = ApprovalDecisionRequest,
    responses(
        (status = 200, description = "Approval decision recorded"),
        (status = 409, description = "Approval already responded"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Approval not found")
    )
)]
pub async fn respond_to_approval(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(approval_ref): Path<String>,
    Json(payload): Json<ApprovalDecisionRequest>,
) -> ApiResult<Json<ApiResponse<()>>> {
    let pool = state.db.clone();

    let approval = resolve_approval_context(&pool, &approval_ref)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Approval not found".to_string()))?;

    // Check permission
    RbacChecker::check_permission(
        auth_user.id,
        approval.project_id,
        Permission::ApproveTools,
        &pool,
    )
    .await?;

    // Update approval status
    let status = match payload.decision {
        ApprovalDecision::Approve => "approved",
        ApprovalDecision::Deny => "denied",
    };
    let denied_reason = match payload.decision {
        ApprovalDecision::Approve => None,
        ApprovalDecision::Deny => payload.reason.as_deref(),
    };

    let result = sqlx::query(
        r#"
        UPDATE tool_approvals
        SET status = $1::approval_status,
            approved_by = $2,
            denied_reason = $3,
            responded_at = NOW()
        WHERE id = $4
          AND status = 'pending'::approval_status
        "#,
    )
    .bind(status)
    .bind(auth_user.id)
    .bind(denied_reason)
    .bind(approval.id)
    .execute(&pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if result.rows_affected() == 0 {
        let current_status: Option<String> =
            sqlx::query_scalar("SELECT status::text FROM tool_approvals WHERE id = $1")
                .bind(approval.id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

        if let Some(current_status) = current_status {
            return Err(ApiError::Conflict(format!(
                "Approval already resolved with status '{}'",
                current_status
            )));
        }

        return Err(ApiError::NotFound("Approval not found".to_string()));
    }

    tracing::info!(
        approval_id = %approval.id,
        tool_use_id = %approval.tool_use_id,
        user_id = %auth_user.id,
        decision = %status,
        "Tool approval decision recorded"
    );

    Ok(Json(ApiResponse::success(
        (),
        format!("Approval {} successfully", status),
    )))
}
