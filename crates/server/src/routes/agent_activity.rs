use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::api::{AgentActivityLogDto, AgentActivityStatusDto, ApiResponse};
use crate::error::{ApiError, ApiResult};
use crate::middleware::{AuthUser, RbacChecker};
use crate::AppState;

/// Max bytes to read per attempt when fetching logs (caps I/O).
const MAX_TAIL_BYTES_PER_ATTEMPT: usize = 150_000; // ~250 lines at ~600 bytes/line
/// Max attempts to read when aggregating logs (project_id or all-projects).
const MAX_ATTEMPTS_FOR_LOGS: i64 = 15;

#[derive(Debug, Deserialize)]
pub struct AgentLogsQuery {
    pub attempt_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub limit: Option<i64>,
}

/// Get status of all running/recent agent attempts
#[utoipa::path(
    get,
    path = "/api/v1/agent-activity/status",
    tag = "Agent Activity",
    responses(
        (status = 200, description = "Agent statuses retrieved"),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn get_agent_status(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> ApiResult<Json<ApiResponse<Vec<AgentActivityStatusDto>>>> {
    let is_admin = RbacChecker::is_system_admin(auth_user.id, &state.db).await?;

    let statuses = sqlx::query_as::<
        _,
        (
            Uuid,
            Uuid,
            String,
            String,
            String,
            Option<chrono::DateTime<chrono::Utc>>,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        r#"
        SELECT
            ta.id,
            ta.task_id,
            t.title as task_title,
            p.name as project_name,
            ta.status::text,
            ta.started_at,
            ta.created_at
        FROM task_attempts ta
        JOIN tasks t ON t.id = ta.task_id
        JOIN projects p ON p.id = t.project_id
        WHERE (
                $1
                OR EXISTS (
                    SELECT 1
                    FROM project_members pm
                    WHERE pm.project_id = p.id
                      AND pm.user_id = $2
                )
            )
          AND (
                ta.status = 'running'
                OR (ta.status = 'queued' AND ta.created_at > NOW() - INTERVAL '1 hour')
                OR (ta.status NOT IN ('queued', 'running') AND ta.created_at > NOW() - INTERVAL '1 hour')
              )
        ORDER BY
            CASE ta.status
                WHEN 'running' THEN 1
                WHEN 'queued' THEN 2
                ELSE 3
            END,
            ta.created_at DESC
        LIMIT 10
        "#,
    )
    .bind(is_admin)
    .bind(auth_user.id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dtos: Vec<AgentActivityStatusDto> = statuses
        .into_iter()
        .enumerate()
        .map(
            |(i, (id, _task_id, task_title, project_name, status_str, started_at, created_at))| {
                AgentActivityStatusDto {
                    id,
                    name: format!("Agent-{}", i + 1),
                    task_title,
                    project_name,
                    status: parse_attempt_status(&status_str),
                    started_at,
                    created_at,
                }
            },
        )
        .collect();

    Ok(Json(ApiResponse::success(dtos, "Agent statuses retrieved")))
}

/// Get recent logs across all attempts
#[utoipa::path(
    get,
    path = "/api/v1/agent-activity/logs",
    tag = "Agent Activity",
    params(
        ("attempt_id" = Option<Uuid>, Query, description = "Filter by attempt ID"),
        ("project_id" = Option<Uuid>, Query, description = "Filter by project ID"),
        ("limit" = Option<i64>, Query, description = "Max logs to return (default 100)")
    ),
    responses(
        (status = 200, description = "Agent logs retrieved"),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn get_agent_logs(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<AgentLogsQuery>,
) -> ApiResult<Json<ApiResponse<Vec<AgentActivityLogDto>>>> {
    let limit = query.limit.unwrap_or(100).min(500);
    let is_admin = RbacChecker::is_system_admin(auth_user.id, &state.db).await?;

    let logs = if let Some(attempt_id) = query.attempt_id {
        // Filter by specific attempt - use tail read to cap backend work
        let attempt_row = sqlx::query_as::<_, (Option<String>,)>(
            "SELECT s3_log_key FROM task_attempts WHERE id = $1",
        )
        .bind(attempt_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        let s3_log_key = attempt_row.and_then(|r| r.0);

        let (task_id, task_title, project_name) = sqlx::query_as::<_, (Uuid, String, String)>(
            r#"
            SELECT t.id, t.title, p.name
            FROM task_attempts ta
            JOIN tasks t ON t.id = ta.task_id
            JOIN projects p ON p.id = t.project_id
            WHERE ta.id = $1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".into()))?;

        let has_access = is_admin
            || sqlx::query_scalar::<_, bool>(
                r#"
                SELECT EXISTS(
                    SELECT 1
                    FROM task_attempts ta
                    JOIN tasks t ON t.id = ta.task_id
                    JOIN project_members pm ON pm.project_id = t.project_id
                    WHERE ta.id = $1 AND pm.user_id = $2
                )
                "#,
            )
            .bind(attempt_id)
            .bind(auth_user.id)
            .fetch_one(&state.db)
            .await
            .unwrap_or(false);

        if !has_access {
            return Err(ApiError::Forbidden("Access denied".to_string()));
        }

        // Tail read: cap bytes to limit backend work (was: full blob + parse all)
        let max_bytes = ((limit as usize + 50) * 600).min(500_000);
        let bytes = if let Some(s3_key) = s3_log_key {
            state
                .storage_service
                .get_log_bytes_tail(&s3_key, max_bytes)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        "Failed to load log tail from S3 for attempt {}: {}",
                        attempt_id,
                        e
                    );
                    vec![]
                })
        } else {
            acpms_executors::read_attempt_log_file_tail(attempt_id, max_bytes)
                .await
                .unwrap_or_default()
        };

        let logs = acpms_executors::parse_jsonl_tail_to_agent_logs(&bytes, limit as usize);

        let from_db: Vec<(Uuid, Uuid, String, String, chrono::DateTime<chrono::Utc>)> = logs
            .into_iter()
            .map(|l| (l.id, l.attempt_id, l.log_type, l.content, l.created_at))
            .collect();

        from_db
            .into_iter()
            .map(|(id, attempt_id, log_type, content, created_at)| {
                (
                    id,
                    attempt_id,
                    task_id,
                    task_title.clone(),
                    project_name.clone(),
                    log_type,
                    content,
                    created_at,
                )
            })
            .collect()
    } else if let Some(project_id) = query.project_id {
        // JSONL-only: aggregate logs from attempts in project (no agent_logs)
        load_logs_from_attempts(&state, Some(project_id), is_admin, auth_user.id, limit).await?
    } else {
        // JSONL-only: aggregate logs from all accessible attempts
        load_logs_from_attempts(&state, None, is_admin, auth_user.id, limit).await?
    };

    let dtos: Vec<AgentActivityLogDto> = logs
        .into_iter()
        .map(
            |(id, attempt_id, task_id, task_title, project_name, log_type, content, created_at)| {
                AgentActivityLogDto {
                    id,
                    attempt_id,
                    task_id,
                    task_title,
                    project_name,
                    log_type,
                    content,
                    created_at,
                }
            },
        )
        .collect();

    Ok(Json(ApiResponse::success(dtos, "Agent logs retrieved")))
}

type LogRow = (
    Uuid,
    Uuid,
    Uuid,
    String,
    String,
    String,
    String,
    chrono::DateTime<chrono::Utc>,
);

async fn load_logs_from_attempts(
    state: &crate::AppState,
    project_id: Option<Uuid>,
    is_admin: bool,
    user_id: Uuid,
    limit: i64,
) -> ApiResult<Vec<LogRow>> {
    // Cap attempts to limit backend work (was: full read of 50 attempts)
    let attempts: Vec<(Uuid, Option<String>, Uuid, String, String)> = if let Some(pid) = project_id
    {
        sqlx::query_as(
            r#"
            SELECT ta.id, ta.s3_log_key, t.id as task_id, t.title, p.name
            FROM task_attempts ta
            JOIN tasks t ON t.id = ta.task_id
            JOIN projects p ON p.id = t.project_id
            WHERE p.id = $1
              AND ($2 OR EXISTS (SELECT 1 FROM project_members pm WHERE pm.project_id = p.id AND pm.user_id = $3))
            ORDER BY ta.created_at DESC
            LIMIT $4
            "#,
        )
        .bind(pid)
        .bind(is_admin)
        .bind(user_id)
        .bind(MAX_ATTEMPTS_FOR_LOGS)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as(
            r#"
            SELECT ta.id, ta.s3_log_key, t.id as task_id, t.title, p.name
            FROM task_attempts ta
            JOIN tasks t ON t.id = ta.task_id
            JOIN projects p ON p.id = t.project_id
            WHERE $1 OR EXISTS (SELECT 1 FROM project_members pm WHERE pm.project_id = p.id AND pm.user_id = $2)
            ORDER BY ta.created_at DESC
            LIMIT $3
            "#,
        )
        .bind(is_admin)
        .bind(user_id)
        .bind(MAX_ATTEMPTS_FOR_LOGS)
        .fetch_all(&state.db)
        .await
    }
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut all_logs: Vec<LogRow> = Vec::new();
    for (attempt_id, s3_log_key, task_id, task_title, project_name) in attempts {
        // Tail read per attempt: cap bytes instead of full blob
        let bytes = if let Some(key) = s3_log_key {
            state
                .storage_service
                .get_log_bytes_tail(&key, MAX_TAIL_BYTES_PER_ATTEMPT)
                .await
                .unwrap_or_default()
        } else {
            acpms_executors::read_attempt_log_file_tail(attempt_id, MAX_TAIL_BYTES_PER_ATTEMPT)
                .await
                .unwrap_or_default()
        };
        let logs = acpms_executors::parse_jsonl_tail_to_agent_logs(
            &bytes,
            (limit as usize + 50).min(200), // max entries per attempt
        );
        for l in logs {
            all_logs.push((
                l.id,
                l.attempt_id,
                task_id,
                task_title.clone(),
                project_name.clone(),
                l.log_type,
                l.content,
                l.created_at,
            ));
        }
    }
    all_logs.sort_by(|a, b| b.7.cmp(&a.7));
    all_logs.truncate(limit as usize);
    Ok(all_logs)
}

fn parse_attempt_status(s: &str) -> acpms_db::models::AttemptStatus {
    match s.to_lowercase().as_str() {
        "queued" => acpms_db::models::AttemptStatus::Queued,
        "running" => acpms_db::models::AttemptStatus::Running,
        "success" => acpms_db::models::AttemptStatus::Success,
        "failed" => acpms_db::models::AttemptStatus::Failed,
        "cancelled" => acpms_db::models::AttemptStatus::Cancelled,
        _ => acpms_db::models::AttemptStatus::Queued,
    }
}
