use crate::api::{
    ApiResponse, GitLabConfigurationDto, MergeRequestDto, MergeRequestOverviewDto,
    MergeRequestStatsDto,
};
use crate::error::ApiError;
use crate::middleware::{AuthUser, Permission, RbacChecker};
use crate::state::AppState;
use acpms_db::models::LinkGitLabProjectRequest;
use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::FromRow;
use utoipa::IntoParams;
use uuid::Uuid;

pub fn create_routes() -> Router<AppState> {
    Router::new()
        .route("/merge-requests", get(list_merge_requests))
        .route("/merge-requests/stats", get(get_merge_request_stats))
        .route("/projects/:id/gitlab/link", post(link_project))
        .route("/projects/:id/gitlab/status", get(get_status))
        .route(
            "/tasks/:id/gitlab/merge_requests",
            get(get_task_merge_requests),
        )
        .route("/webhooks/gitlab", post(handle_webhook))
}

#[derive(Debug, Deserialize, IntoParams)]
struct ListMergeRequestsQuery {
    /// Optional status filter: open, pending_review, merged, closed
    pub status: Option<String>,
    /// Optional full text search over title/description/author/project/MR number
    pub search: Option<String>,
    /// Page size (1-100, default 50)
    pub limit: Option<u32>,
    /// Offset for pagination (default 0)
    pub offset: Option<u32>,
}

#[derive(Debug, FromRow)]
struct MergeRequestOverviewRow {
    id: Uuid,
    task_id: Uuid,
    project_id: Uuid,
    project_name: String,
    task_title: String,
    task_description: Option<String>,
    mr_number: i64,
    web_url: String,
    gitlab_status: String,
    task_status: String,
    author_name: String,
    author_avatar: Option<String>,
    latest_attempt_id: Option<Uuid>,
    changed_files: Option<i32>,
    additions: Option<i32>,
    deletions: Option<i32>,
    source_branch: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn normalize_mr_status(gitlab_status: &str, task_status: &str) -> String {
    let gitlab = gitlab_status.to_ascii_lowercase();
    match gitlab.as_str() {
        "merged" => "merged".to_string(),
        "closed" => "closed".to_string(),
        "opened" | "open" => {
            if task_status == "in_review" {
                "pending_review".to_string()
            } else {
                "open".to_string()
            }
        }
        _ => "open".to_string(),
    }
}

fn infer_agent_author(author_name: &str) -> bool {
    let lower = author_name.to_ascii_lowercase();
    lower.starts_with("agent")
        || lower.contains(" bot")
        || lower.ends_with("bot")
        || lower.contains("[bot]")
}

fn map_row_to_overview_dto(row: MergeRequestOverviewRow) -> MergeRequestOverviewDto {
    let source_branch = row.source_branch.unwrap_or_else(|| {
        let task_id_short = row.task_id.to_string().chars().take(8).collect::<String>();
        format!("feat/task-{}", task_id_short)
    });

    MergeRequestOverviewDto {
        id: row.id,
        task_id: row.task_id,
        project_id: row.project_id,
        project_name: row.project_name,
        title: row.task_title,
        description: row.task_description,
        mr_number: row.mr_number,
        status: normalize_mr_status(&row.gitlab_status, &row.task_status),
        web_url: row.web_url,
        author_name: row.author_name.clone(),
        author_avatar: row.author_avatar,
        author_is_agent: infer_agent_author(&row.author_name),
        source_branch,
        target_branch: "main".to_string(),
        changed_files: row.changed_files.unwrap_or(0),
        additions: row.additions.unwrap_or(0),
        deletions: row.deletions.unwrap_or(0),
        latest_attempt_id: row.latest_attempt_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

/// Escape special chars for ILIKE: % and _ (PostgreSQL uses \ as escape)
fn escape_like_pattern(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Normalized status in SQL (matches normalize_mr_status). Use with base CTE.
const NORMALIZED_STATUS_CASE: &str = r#"
    CASE
        WHEN LOWER(base.gitlab_status) = 'merged' THEN 'merged'
        WHEN LOWER(base.gitlab_status) = 'closed' THEN 'closed'
        WHEN LOWER(base.gitlab_status) IN ('opened','open') AND base.task_status = 'in_review' THEN 'pending_review'
        ELSE 'open'
    END
"#;

#[derive(Debug, FromRow)]
struct MergeRequestOverviewRowWithTotal {
    id: Uuid,
    task_id: Uuid,
    project_id: Uuid,
    project_name: String,
    task_title: String,
    task_description: Option<String>,
    mr_number: i64,
    web_url: String,
    gitlab_status: String,
    task_status: String,
    author_name: String,
    author_avatar: Option<String>,
    latest_attempt_id: Option<Uuid>,
    changed_files: Option<i32>,
    additions: Option<i32>,
    deletions: Option<i32>,
    source_branch: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    total_count: i64,
}

const MR_BASE_CTE: &str = r#"
    WITH accessible_mr_tasks AS (
        SELECT t.id
        FROM merge_requests mr
        INNER JOIN tasks t ON t.id = mr.task_id
        WHERE ($1::boolean OR EXISTS (SELECT 1 FROM project_members pm WHERE pm.project_id = t.project_id AND pm.user_id = $2))
    ),
    latest_attempt AS (
        SELECT DISTINCT ON (ta.task_id)
            ta.id AS latest_attempt_id,
            ta.task_id,
            ta.diff_total_files,
            ta.diff_total_additions,
            ta.diff_total_deletions,
            ta.metadata
        FROM task_attempts ta
        WHERE ta.task_id IN (SELECT id FROM accessible_mr_tasks)
        ORDER BY ta.task_id, ta.created_at DESC
    ),
    attempt_for_mr AS (
        SELECT mr.id AS mr_id,
               COALESCE(mr.attempt_id, la.latest_attempt_id) AS attempt_id
        FROM merge_requests mr
        LEFT JOIN latest_attempt la ON la.task_id = mr.task_id
    ),
    base AS (
        SELECT
            mr.id,
            mr.task_id,
            t.project_id,
            p.name AS project_name,
            t.title AS task_title,
            t.description AS task_description,
            COALESCE(mr.gitlab_mr_iid, mr.github_pr_number, 0)::bigint AS mr_number,
            mr.web_url,
            mr.status AS gitlab_status,
            t.status::text AS task_status,
            u.name AS author_name,
            u.avatar_url AS author_avatar,
            afm.attempt_id AS latest_attempt_id,
            ta.diff_total_files AS changed_files,
            ta.diff_total_additions AS additions,
            ta.diff_total_deletions AS deletions,
            (ta.metadata->>'branch') AS source_branch,
            mr.created_at,
            mr.updated_at
        FROM merge_requests mr
        INNER JOIN tasks t ON t.id = mr.task_id
        INNER JOIN projects p ON p.id = t.project_id
        INNER JOIN users u ON u.id = t.created_by
        INNER JOIN attempt_for_mr afm ON afm.mr_id = mr.id
        LEFT JOIN task_attempts ta ON ta.id = afm.attempt_id
        WHERE (
            $1::boolean
            OR EXISTS (
                SELECT 1
                FROM project_members pm
                WHERE pm.project_id = t.project_id
                  AND pm.user_id = $2
            )
        )
    )
"#;

async fn load_merge_requests_paginated(
    state: &AppState,
    user_id: Uuid,
    status_filter: Option<&str>,
    search_pattern: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<MergeRequestOverviewRow>, i64), ApiError> {
    let is_admin = RbacChecker::is_system_admin(user_id, &state.db).await?;

    let (rows, total) = match (status_filter, search_pattern) {
        (None, None) => {
            let sql = format!(
                r#"{} SELECT base.*, COUNT(*) OVER () AS total_count
                FROM base
                ORDER BY base.updated_at DESC
                LIMIT {} OFFSET {}"#,
                MR_BASE_CTE, limit, offset
            );
            let rows: Vec<MergeRequestOverviewRowWithTotal> = sqlx::query_as(&sql)
                .bind(is_admin)
                .bind(user_id)
                .fetch_all(&state.db)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to load merge requests: {}", e)))?;
            let total = rows.first().map(|r| r.total_count).unwrap_or(0);
            (rows, total)
        }
        (Some(status), None) => {
            let sql = format!(
                r#"{} SELECT base.*, COUNT(*) OVER () AS total_count
                FROM base
                WHERE {} = $3
                ORDER BY base.updated_at DESC
                LIMIT {} OFFSET {}"#,
                MR_BASE_CTE,
                NORMALIZED_STATUS_CASE.trim(),
                limit,
                offset
            );
            let rows: Vec<MergeRequestOverviewRowWithTotal> = sqlx::query_as(&sql)
                .bind(is_admin)
                .bind(user_id)
                .bind(status)
                .fetch_all(&state.db)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to load merge requests: {}", e)))?;
            let total = rows.first().map(|r| r.total_count).unwrap_or(0);
            (rows, total)
        }
        (None, Some(pattern)) => {
            let sql = format!(
                r#"{} SELECT base.*, COUNT(*) OVER () AS total_count
                FROM base
                WHERE LOWER(CONCAT_WS(' ', base.task_title, COALESCE(base.task_description,''), base.author_name, base.project_name, base.mr_number::text)) LIKE LOWER($3)
                ORDER BY base.updated_at DESC
                LIMIT {} OFFSET {}"#,
                MR_BASE_CTE, limit, offset
            );
            let rows: Vec<MergeRequestOverviewRowWithTotal> = sqlx::query_as(&sql)
                .bind(is_admin)
                .bind(user_id)
                .bind(pattern)
                .fetch_all(&state.db)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to load merge requests: {}", e)))?;
            let total = rows.first().map(|r| r.total_count).unwrap_or(0);
            (rows, total)
        }
        (Some(status), Some(pattern)) => {
            let sql = format!(
                r#"{} SELECT base.*, COUNT(*) OVER () AS total_count
                FROM base
                WHERE {} = $3
                  AND LOWER(CONCAT_WS(' ', base.task_title, COALESCE(base.task_description,''), base.author_name, base.project_name, base.mr_number::text)) LIKE LOWER($4)
                ORDER BY base.updated_at DESC
                LIMIT {} OFFSET {}"#,
                MR_BASE_CTE,
                NORMALIZED_STATUS_CASE.trim(),
                limit,
                offset
            );
            let rows: Vec<MergeRequestOverviewRowWithTotal> = sqlx::query_as(&sql)
                .bind(is_admin)
                .bind(user_id)
                .bind(status)
                .bind(pattern)
                .fetch_all(&state.db)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to load merge requests: {}", e)))?;
            let total = rows.first().map(|r| r.total_count).unwrap_or(0);
            (rows, total)
        }
    };

    let rows: Vec<MergeRequestOverviewRow> = rows
        .into_iter()
        .map(|r| MergeRequestOverviewRow {
            id: r.id,
            task_id: r.task_id,
            project_id: r.project_id,
            project_name: r.project_name,
            task_title: r.task_title,
            task_description: r.task_description,
            mr_number: r.mr_number,
            web_url: r.web_url,
            gitlab_status: r.gitlab_status,
            task_status: r.task_status,
            author_name: r.author_name,
            author_avatar: r.author_avatar,
            latest_attempt_id: r.latest_attempt_id,
            changed_files: r.changed_files,
            additions: r.additions,
            deletions: r.deletions,
            source_branch: r.source_branch,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();

    Ok((rows, total))
}

#[utoipa::path(
    get,
    path = "/api/v1/merge-requests",
    tag = "GitLab",
    params(ListMergeRequestsQuery),
    responses(
        (status = 200, description = "Merge request overview list", body = MergeRequestOverviewListResponse),
        (status = 500, description = "Internal Server Error")
    )
)]
async fn list_merge_requests(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<ListMergeRequestsQuery>,
) -> Result<Json<ApiResponse<Vec<MergeRequestOverviewDto>>>, ApiError> {
    let requested_status: Option<String> = query
        .status
        .as_ref()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    let requested_status = requested_status.as_deref();
    let search_pattern = query
        .search
        .as_ref()
        .map(|s| {
            let escaped = escape_like_pattern(s.trim());
            format!("%{}%", escaped)
        })
        .filter(|s| s != "%%");

    let limit = query
        .limit
        .map(|l| (l as i64).min(100).max(1))
        .unwrap_or(50);
    let offset = (query.offset.unwrap_or(0) as i64).max(0);

    let (rows, _total) = load_merge_requests_paginated(
        &state,
        auth_user.id,
        requested_status,
        search_pattern.as_deref(),
        limit,
        offset,
    )
    .await?;

    let items: Vec<MergeRequestOverviewDto> =
        rows.into_iter().map(map_row_to_overview_dto).collect();

    let response = ApiResponse::success(items, "Merge requests retrieved successfully");
    Ok(Json(response))
}

#[derive(Debug, FromRow)]
struct MergeRequestStatsRow {
    open: i64,
    pending_review: i64,
    merged: i64,
    ai_generated: i64,
}

async fn load_merge_request_stats(
    state: &AppState,
    user_id: Uuid,
) -> Result<MergeRequestStatsDto, ApiError> {
    let is_admin = RbacChecker::is_system_admin(user_id, &state.db).await?;

    let sql = format!(
        r#"{} SELECT
            COUNT(*) FILTER (WHERE {} = 'open')::bigint AS open,
            COUNT(*) FILTER (WHERE {} = 'pending_review')::bigint AS pending_review,
            COUNT(*) FILTER (WHERE {} = 'merged')::bigint AS merged,
            COUNT(*) FILTER (WHERE (LOWER(base.author_name) LIKE 'agent%%'
                OR LOWER(base.author_name) LIKE '%% bot%%'
                OR LOWER(base.author_name) LIKE '%%bot'
                OR LOWER(base.author_name) LIKE '%%[bot]%%'))::bigint AS ai_generated
        FROM base"#,
        MR_BASE_CTE,
        NORMALIZED_STATUS_CASE.trim(),
        NORMALIZED_STATUS_CASE.trim(),
        NORMALIZED_STATUS_CASE.trim(),
    );

    let row: MergeRequestStatsRow = sqlx::query_as(&sql)
        .bind(is_admin)
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load merge request stats: {}", e)))?;

    Ok(MergeRequestStatsDto {
        open: row.open,
        pending_review: row.pending_review,
        merged: row.merged,
        ai_generated: row.ai_generated,
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/merge-requests/stats",
    tag = "GitLab",
    responses(
        (status = 200, description = "Merge request dashboard stats", body = MergeRequestStatsResponse),
        (status = 500, description = "Internal Server Error")
    )
)]
async fn get_merge_request_stats(
    auth_user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<MergeRequestStatsDto>>, ApiError> {
    let stats = load_merge_request_stats(&state, auth_user.id).await?;
    let response = ApiResponse::success(stats, "Merge request stats retrieved successfully");
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{id}/gitlab/link",
    tag = "GitLab",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    request_body = LinkGitLabProjectRequestDoc,
    responses(
        (status = 200, description = "GitLab project linked successfully", body = GitLabConfigurationResponse),
        (status = 500, description = "Internal Server Error")
    )
)]
async fn link_project(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(req): Json<LinkGitLabProjectRequest>,
) -> Result<Json<ApiResponse<GitLabConfigurationDto>>, ApiError> {
    // Chỉ System Admin mới được link GitLab - tránh user có quá nhiều quyền access đến GitLab
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    let config = state
        .gitlab_service
        .link_project(project_id, req)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = GitLabConfigurationDto::from(config);
    let response = ApiResponse::success(dto, "GitLab project linked successfully");
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}/gitlab/status",
    tag = "GitLab",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "GitLab configuration retrieved", body = GitLabConfigurationResponse),
        (status = 500, description = "Internal Server Error")
    )
)]
async fn get_status(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Option<GitLabConfigurationDto>>>, ApiError> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let config = state
        .gitlab_service
        .get_config(project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = config.map(GitLabConfigurationDto::from);
    let response = ApiResponse::success(dto, "GitLab configuration retrieved successfully");
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/tasks/{id}/gitlab/merge_requests",
    tag = "GitLab",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task merge requests retrieved", body = MergeRequestListResponse),
        (status = 500, description = "Internal Server Error")
    )
)]
async fn get_task_merge_requests(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<MergeRequestDto>>>, ApiError> {
    let project_id: Option<Uuid> = sqlx::query_scalar("SELECT project_id FROM tasks WHERE id = $1")
        .bind(task_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let project_id = project_id.ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewTask, &state.db)
        .await?;

    let mrs = state
        .gitlab_service
        .get_task_merge_requests(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dtos: Vec<MergeRequestDto> = mrs.into_iter().map(MergeRequestDto::from).collect();
    let response = ApiResponse::success(dtos, "Task merge requests retrieved successfully");
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/webhooks/gitlab",
    tag = "GitLab",
    responses(
        (status = 200, description = "Webhook processed", body = EmptyResponse),
        (status = 400, description = "Bad Request")
    )
)]
async fn handle_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    // 1. Validate X-Gitlab-Token header
    let token = headers
        .get("X-Gitlab-Token")
        .and_then(|h| h.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    // Find project by webhook secret
    let config = sqlx::query_as::<_, acpms_db::models::GitLabConfiguration>(
        "SELECT * FROM gitlab_configurations WHERE webhook_secret = $1",
    )
    .bind(token)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or(ApiError::Unauthorized)?;

    // 2. Extract event type and ID for deduplication
    let event_type = payload
        .get("object_kind")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    // Use GitLab's event ID if available, otherwise generate one
    let event_id = if let Some(id_str) = payload.get("event_id").and_then(|v| v.as_str()) {
        id_str.to_string()
    } else if let Some(id_num) = payload.get("id").and_then(|v| v.as_u64()) {
        id_num.to_string()
    } else {
        uuid::Uuid::new_v4().to_string()
    };

    // 3. Queue event for async processing (non-blocking)
    let webhook_event_id = state
        .webhook_manager
        .queue_event(config.project_id, event_id, event_type, payload)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to queue webhook event: {}", e)))?;

    tracing::info!(
        "Queued webhook event {} for async processing",
        webhook_event_id
    );

    // 4. Return immediately (200 OK) - processing happens in background
    let response = ApiResponse::success((), "Webhook received and queued for processing");
    Ok(Json(response))
}
