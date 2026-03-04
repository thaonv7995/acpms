use acpms_db::models::{AgentLog as DbAgentLog, ExecutionProcess as DbExecutionProcess};
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::path::Path as FsPath;
use tokio::process::Command;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::api::{AgentLogDto, ApiResponse, TaskAttemptDto};
use crate::error::{ApiError, ApiResult};
use crate::middleware::{AuthUser, Permission, RbacChecker};
use crate::routes::task_attempts::{self, ResumeAttemptRequest};
use crate::AppState;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExecutionProcessDto {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub process_id: Option<i32>,
    pub worktree_path: Option<String>,
    pub branch_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResetExecutionProcessRequest {
    #[serde(default)]
    pub perform_git_reset: bool,
    #[serde(default)]
    pub force_when_dirty: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ResetExecutionProcessResponse {
    pub process_id: Uuid,
    pub worktree_path: Option<String>,
    pub git_reset_applied: bool,
    pub worktree_was_dirty: bool,
    pub force_when_dirty: bool,
    pub requested_by_user_id: Uuid,
    pub requested_at: chrono::DateTime<chrono::Utc>,
}

impl From<DbExecutionProcess> for ExecutionProcessDto {
    fn from(value: DbExecutionProcess) -> Self {
        Self {
            id: value.id,
            attempt_id: value.attempt_id,
            process_id: value.process_id,
            worktree_path: value.worktree_path,
            branch_name: value.branch_name,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListExecutionProcessesQuery {
    pub attempt_id: Uuid,
}

#[derive(Debug, Clone)]
struct ExecutionProcessWindowContext {
    attempt_id: Uuid,
    project_id: Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
    next_created_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn resolve_project_id_for_attempt(
    pool: &sqlx::PgPool,
    attempt_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT t.project_id
        FROM task_attempts ta
        JOIN tasks t ON t.id = ta.task_id
        WHERE ta.id = $1
        "#,
    )
    .bind(attempt_id)
    .fetch_optional(pool)
    .await
}

async fn resolve_attempt_id_for_process(
    pool: &sqlx::PgPool,
    process_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT attempt_id
        FROM execution_processes
        WHERE id = $1
        "#,
    )
    .bind(process_id)
    .fetch_optional(pool)
    .await
}

fn parse_git_status_porcelain_is_dirty(output: &str) -> bool {
    output.lines().any(|line| !line.trim().is_empty())
}

async fn read_git_worktree_dirty(path: &FsPath) -> Result<bool, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("status")
        .arg("--porcelain")
        .output()
        .await
        .map_err(|e| format!("Failed to run git status: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            format!("exit status {}", output.status)
        } else {
            stderr
        };
        return Err(format!("git status --porcelain failed: {}", detail));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_git_status_porcelain_is_dirty(&stdout))
}

async fn run_git_hard_reset(path: &FsPath) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("reset")
        .arg("--hard")
        .arg("HEAD")
        .output()
        .await
        .map_err(|e| format!("Failed to run git reset --hard: {}", e))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let detail = if stderr.is_empty() {
        format!("exit status {}", output.status)
    } else {
        stderr
    };
    Err(format!("git reset --hard HEAD failed: {}", detail))
}

async fn resolve_process_window_context(
    pool: &sqlx::PgPool,
    process_id: Uuid,
) -> Result<Option<ExecutionProcessWindowContext>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct ProcessContextRow {
        attempt_id: Uuid,
        project_id: Uuid,
        created_at: chrono::DateTime<chrono::Utc>,
        next_created_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let row: Option<ProcessContextRow> = sqlx::query_as(
        r#"
        SELECT
            ep.attempt_id,
            t.project_id,
            ep.created_at,
            (
                SELECT ep2.created_at
                FROM execution_processes ep2
                WHERE ep2.attempt_id = ep.attempt_id
                  AND (
                    ep2.created_at > ep.created_at
                    OR (ep2.created_at = ep.created_at AND ep2.id > ep.id)
                  )
                ORDER BY ep2.created_at ASC, ep2.id ASC
                LIMIT 1
            ) AS next_created_at
        FROM execution_processes ep
        JOIN task_attempts ta ON ta.id = ep.attempt_id
        JOIN tasks t ON t.id = ta.task_id
        WHERE ep.id = $1
        "#,
    )
    .bind(process_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|value| ExecutionProcessWindowContext {
        attempt_id: value.attempt_id,
        project_id: value.project_id,
        created_at: value.created_at,
        next_created_at: value.next_created_at,
    }))
}

async fn fetch_process_logs(
    state: &crate::AppState,
    ctx: &ExecutionProcessWindowContext,
    is_normalized: bool,
) -> Result<Vec<DbAgentLog>, ApiError> {
    let bytes = if let Some(s3_key) = sqlx::query_scalar::<_, Option<String>>(
        "SELECT s3_log_key FROM task_attempts WHERE id = $1",
    )
    .bind(ctx.attempt_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .flatten()
    {
        state
            .storage_service
            .get_log_bytes(&s3_key)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
    } else {
        acpms_executors::read_attempt_log_file(ctx.attempt_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
    };

    let mut logs = acpms_executors::parse_jsonl_to_agent_logs(&bytes);
    logs.retain(|log| {
        log.created_at >= ctx.created_at
            && (ctx.next_created_at.is_none() || log.created_at < ctx.next_created_at.unwrap())
    });
    logs.retain(|log| {
        if is_normalized {
            log.log_type == "normalized"
        } else {
            matches!(
                log.log_type.as_str(),
                "process_stdout" | "process_stderr" | "stdout" | "stderr"
            )
        }
    });
    Ok(logs)
}

#[utoipa::path(
    get,
    path = "/api/v1/execution-processes",
    tag = "Execution Processes",
    params(ListExecutionProcessesQuery),
    responses(
        (status = 200, description = "Execution process list", body = Vec<ExecutionProcessDto>),
        (status = 404, description = "Attempt not found")
    )
)]
pub async fn list_execution_processes(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<ListExecutionProcessesQuery>,
) -> ApiResult<Json<ApiResponse<Vec<ExecutionProcessDto>>>> {
    let pool = state.db.clone();
    let project_id = resolve_project_id_for_attempt(&pool, query.attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &pool).await?;

    let processes: Vec<DbExecutionProcess> = sqlx::query_as(
        r#"
        SELECT id, attempt_id, process_id, worktree_path, branch_name, created_at
        FROM execution_processes
        WHERE attempt_id = $1
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .bind(query.attempt_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch execution processes: {}", e)))?;

    let items = processes
        .into_iter()
        .map(ExecutionProcessDto::from)
        .collect();
    Ok(Json(ApiResponse::success(
        items,
        "Execution processes retrieved successfully",
    )))
}

#[utoipa::path(
    get,
    path = "/api/v1/execution-processes/{id}",
    tag = "Execution Processes",
    params(
        ("id" = Uuid, Path, description = "Execution process ID")
    ),
    responses(
        (status = 200, description = "Execution process detail", body = ExecutionProcessDto),
        (status = 404, description = "Execution process not found")
    )
)]
pub async fn get_execution_process(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(process_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<ExecutionProcessDto>>> {
    let pool = state.db.clone();

    let process: DbExecutionProcess = sqlx::query_as(
        r#"
        SELECT id, attempt_id, process_id, worktree_path, branch_name, created_at
        FROM execution_processes
        WHERE id = $1
        "#,
    )
    .bind(process_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("Execution process not found".to_string()))?;

    let project_id = resolve_project_id_for_attempt(&pool, process.attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &pool).await?;

    Ok(Json(ApiResponse::success(
        ExecutionProcessDto::from(process),
        "Execution process retrieved successfully",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/execution-processes/{id}/follow-up",
    tag = "Execution Processes",
    params(
        ("id" = Uuid, Path, description = "Execution process ID")
    ),
    request_body = ResumeAttemptRequest,
    responses(
        (status = 200, description = "Follow-up accepted for execution process", body = TaskAttemptDto),
        (status = 404, description = "Execution process not found")
    )
)]
pub async fn follow_up_execution_process(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(process_id): Path<Uuid>,
    Json(payload): Json<ResumeAttemptRequest>,
) -> ApiResult<Json<ApiResponse<TaskAttemptDto>>> {
    let pool = state.db.clone();
    let attempt_id = resolve_attempt_id_for_process(&pool, process_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Execution process not found".to_string()))?;

    let mut follow_up_payload = payload;
    follow_up_payload.source_execution_process_id = Some(process_id);

    task_attempts::resume_attempt(
        State(state),
        auth_user,
        Path(attempt_id),
        Json(follow_up_payload),
    )
    .await
}

#[utoipa::path(
    post,
    path = "/api/v1/execution-processes/{id}/reset",
    tag = "Execution Processes",
    params(
        ("id" = Uuid, Path, description = "Execution process ID")
    ),
    request_body = ResetExecutionProcessRequest,
    responses(
        (status = 200, description = "Execution process reset completed", body = ResetExecutionProcessResponse),
        (status = 400, description = "Invalid reset request"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Execution process not found")
    )
)]
pub async fn reset_execution_process(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(process_id): Path<Uuid>,
    Json(payload): Json<ResetExecutionProcessRequest>,
) -> ApiResult<Json<ApiResponse<ResetExecutionProcessResponse>>> {
    let pool = state.db.clone();
    let requested_by_user_id = auth_user.id;
    let requested_at = chrono::Utc::now();

    let process: DbExecutionProcess = sqlx::query_as(
        r#"
        SELECT id, attempt_id, process_id, worktree_path, branch_name, created_at
        FROM execution_processes
        WHERE id = $1
        "#,
    )
    .bind(process_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("Execution process not found".to_string()))?;

    let project_id = resolve_project_id_for_attempt(&pool, process.attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    RbacChecker::check_permission(
        requested_by_user_id,
        project_id,
        Permission::ExecuteTask,
        &pool,
    )
    .await?;

    let worktree_path = process
        .worktree_path
        .clone()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if !payload.perform_git_reset {
        let response = ResetExecutionProcessResponse {
            process_id,
            worktree_path,
            git_reset_applied: false,
            worktree_was_dirty: false,
            force_when_dirty: payload.force_when_dirty,
            requested_by_user_id,
            requested_at,
        };
        return Ok(Json(ApiResponse::success(
            response,
            "Execution process reset acknowledged (no git reset requested)",
        )));
    }

    let Some(worktree_path) = worktree_path.clone() else {
        return Err(ApiError::BadRequest(
            "Execution process has no worktree path to reset".to_string(),
        ));
    };
    let repo_path = FsPath::new(&worktree_path);
    if !repo_path.exists() {
        return Err(ApiError::BadRequest(format!(
            "Execution process worktree path does not exist: {}",
            worktree_path
        )));
    }

    let is_dirty = read_git_worktree_dirty(repo_path)
        .await
        .map_err(ApiError::Internal)?;
    if is_dirty && !payload.force_when_dirty {
        return Err(ApiError::BadRequest(
            "Worktree has uncommitted changes. Set force_when_dirty=true to continue reset."
                .to_string(),
        ));
    }

    run_git_hard_reset(repo_path)
        .await
        .map_err(ApiError::Internal)?;

    let response = ResetExecutionProcessResponse {
        process_id,
        worktree_path: Some(worktree_path),
        git_reset_applied: true,
        worktree_was_dirty: is_dirty,
        force_when_dirty: payload.force_when_dirty,
        requested_by_user_id,
        requested_at,
    };
    Ok(Json(ApiResponse::success(
        response,
        "Execution process reset successfully",
    )))
}

#[utoipa::path(
    get,
    path = "/api/v1/execution-processes/{id}/raw-logs",
    tag = "Execution Processes",
    params(
        ("id" = Uuid, Path, description = "Execution process ID")
    ),
    responses(
        (status = 200, description = "Execution process raw logs", body = Vec<AgentLogDto>),
        (status = 404, description = "Execution process not found")
    )
)]
pub async fn get_execution_process_raw_logs(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(process_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<AgentLogDto>>>> {
    let pool = state.db.clone();
    let process_ctx = resolve_process_window_context(&pool, process_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Execution process not found".to_string()))?;

    RbacChecker::check_permission(
        auth_user.id,
        process_ctx.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    let logs = fetch_process_logs(&state, &process_ctx, false).await?;

    let dtos = logs.into_iter().map(AgentLogDto::from).collect();
    Ok(Json(ApiResponse::success(
        dtos,
        "Execution process raw logs retrieved successfully",
    )))
}

#[utoipa::path(
    get,
    path = "/api/v1/execution-processes/{id}/normalized-logs",
    tag = "Execution Processes",
    params(
        ("id" = Uuid, Path, description = "Execution process ID")
    ),
    responses(
        (status = 200, description = "Execution process normalized logs", body = Vec<AgentLogDto>),
        (status = 404, description = "Execution process not found")
    )
)]
pub async fn get_execution_process_normalized_logs(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(process_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<AgentLogDto>>>> {
    let pool = state.db.clone();
    let process_ctx = resolve_process_window_context(&pool, process_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Execution process not found".to_string()))?;

    RbacChecker::check_permission(
        auth_user.id,
        process_ctx.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    let logs = fetch_process_logs(&state, &process_ctx, true).await?;

    let dtos = logs.into_iter().map(AgentLogDto::from).collect();
    Ok(Json(ApiResponse::success(
        dtos,
        "Execution process normalized logs retrieved successfully",
    )))
}

#[cfg(test)]
mod tests {
    use super::parse_git_status_porcelain_is_dirty;

    #[test]
    fn parse_git_status_porcelain_detects_dirty_lines() {
        let output = " M src/main.rs\n?? docs/new-file.md\n";
        assert!(parse_git_status_porcelain_is_dirty(output));
    }

    #[test]
    fn parse_git_status_porcelain_treats_empty_output_as_clean() {
        assert!(!parse_git_status_porcelain_is_dirty(""));
        assert!(!parse_git_status_porcelain_is_dirty("\n\n"));
        assert!(!parse_git_status_porcelain_is_dirty("   \n\t\n"));
    }
}
