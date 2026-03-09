//! Review workflow API routes
//!
//! Handles review comments and request-changes operations for the Phase 4 review workflow.

use acpms_db::models::{
    project_repo_relative_path, AddReviewCommentRequest, AttemptStatus, RequestChangesRequest,
    TaskStatus,
};
use acpms_services::{ProjectService, ReviewService, TaskAttemptService, TaskService};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::api::{ApiResponse, RequestChangesResponseDto, ReviewCommentDto};
use crate::error::{ApiError, ApiResult};
use crate::middleware::{AuthUser, Permission, RbacChecker};
use crate::routes::openclaw;
use crate::AppState;

#[derive(Debug, FromRow)]
struct ReviewCommentWithUsersRow {
    id: Uuid,
    attempt_id: Uuid,
    user_id: Uuid,
    user_name: String,
    user_avatar: Option<String>,
    file_path: Option<String>,
    line_number: Option<i32>,
    content: String,
    resolved: bool,
    resolved_by: Option<Uuid>,
    resolved_by_name: Option<String>,
    resolved_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ReviewCommentWithUsersRow> for ReviewCommentDto {
    fn from(row: ReviewCommentWithUsersRow) -> Self {
        Self {
            id: row.id,
            attempt_id: row.attempt_id,
            user_id: row.user_id,
            user_name: row.user_name,
            user_avatar: row.user_avatar,
            file_path: row.file_path,
            line_number: row.line_number,
            content: row.content,
            resolved: row.resolved,
            resolved_by: row.resolved_by,
            resolved_by_name: row.resolved_by_name,
            resolved_at: row.resolved_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

async fn get_comment_dto_by_id(
    pool: &acpms_db::PgPool,
    comment_id: Uuid,
) -> Result<ReviewCommentDto, ApiError> {
    let row = sqlx::query_as::<_, ReviewCommentWithUsersRow>(
        r#"
        SELECT
            rc.id,
            rc.attempt_id,
            rc.user_id,
            commenter.name AS user_name,
            commenter.avatar_url AS user_avatar,
            rc.file_path,
            rc.line_number,
            rc.content,
            rc.resolved,
            rc.resolved_by,
            resolver.name AS resolved_by_name,
            rc.resolved_at,
            rc.created_at,
            rc.updated_at
        FROM review_comments rc
        INNER JOIN users commenter ON commenter.id = rc.user_id
        LEFT JOIN users resolver ON resolver.id = rc.resolved_by
        WHERE rc.id = $1
        "#,
    )
    .bind(comment_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to load comment details: {}", e)))?
    .ok_or_else(|| ApiError::NotFound("Comment not found".to_string()))?;

    Ok(ReviewCommentDto::from(row))
}

async fn list_comment_dtos_by_attempt_id(
    pool: &acpms_db::PgPool,
    attempt_id: Uuid,
) -> Result<Vec<ReviewCommentDto>, ApiError> {
    let rows = sqlx::query_as::<_, ReviewCommentWithUsersRow>(
        r#"
        SELECT
            rc.id,
            rc.attempt_id,
            rc.user_id,
            commenter.name AS user_name,
            commenter.avatar_url AS user_avatar,
            rc.file_path,
            rc.line_number,
            rc.content,
            rc.resolved,
            rc.resolved_by,
            resolver.name AS resolved_by_name,
            rc.resolved_at,
            rc.created_at,
            rc.updated_at
        FROM review_comments rc
        INNER JOIN users commenter ON commenter.id = rc.user_id
        LEFT JOIN users resolver ON resolver.id = rc.resolved_by
        WHERE rc.attempt_id = $1
        ORDER BY rc.created_at ASC
        "#,
    )
    .bind(attempt_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to load review comments: {}", e)))?;

    Ok(rows.into_iter().map(ReviewCommentDto::from).collect())
}

// ============================================================================
// Review Comments Endpoints
// ============================================================================

#[utoipa::path(
    post,
    path = "/api/v1/attempts/{id}/comments",
    tag = "Reviews",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    request_body = AddReviewCommentRequest,
    responses(
        (status = 201, description = "Comment created", body = ApiResponse<ReviewCommentDto>),
        (status = 404, description = "Attempt not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn add_comment(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
    Json(payload): Json<AddReviewCommentRequest>,
) -> ApiResult<(StatusCode, Json<ApiResponse<ReviewCommentDto>>)> {
    let pool = state.db.clone();

    // Get attempt to find task for permission check
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission - need at least ViewProject to add comments
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    // Add comment
    let review_service = ReviewService::new(pool.clone());
    let comment = review_service
        .add_comment(attempt_id, auth_user.id, payload)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = get_comment_dto_by_id(&pool, comment.id).await?;
    let response = ApiResponse::created(dto, "Review comment added successfully");

    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    get,
    path = "/api/v1/attempts/{id}/comments",
    tag = "Reviews",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    responses(
        (status = 200, description = "List of comments", body = ApiResponse<Vec<ReviewCommentDto>>),
        (status = 404, description = "Attempt not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_comments(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<ReviewCommentDto>>>> {
    let pool = state.db.clone();

    // Get attempt to find task for permission check
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    let dtos = list_comment_dtos_by_attempt_id(&pool, attempt_id).await?;
    let response = ApiResponse::success(dtos, "Review comments retrieved successfully");

    Ok(Json(response))
}

#[utoipa::path(
    patch,
    path = "/api/v1/comments/{id}/resolve",
    tag = "Reviews",
    params(
        ("id" = Uuid, Path, description = "Comment ID")
    ),
    responses(
        (status = 200, description = "Comment resolved", body = ApiResponse<ReviewCommentDto>),
        (status = 404, description = "Comment not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn resolve_comment(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(comment_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<ReviewCommentDto>>> {
    let pool = state.db.clone();
    let review_service = ReviewService::new(pool.clone());

    // Get comment to find attempt for permission check
    let comment = review_service
        .get_comment(comment_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Comment not found".to_string()))?;

    // Get attempt to find task
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(comment.attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    // Resolve comment
    let resolved = review_service
        .resolve_comment(comment_id, auth_user.id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = get_comment_dto_by_id(&pool, resolved.id).await?;
    let response = ApiResponse::success(dto, "Comment resolved successfully");

    Ok(Json(response))
}

#[utoipa::path(
    patch,
    path = "/api/v1/comments/{id}/unresolve",
    tag = "Reviews",
    params(
        ("id" = Uuid, Path, description = "Comment ID")
    ),
    responses(
        (status = 200, description = "Comment unresolved", body = ApiResponse<ReviewCommentDto>),
        (status = 404, description = "Comment not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn unresolve_comment(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(comment_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<ReviewCommentDto>>> {
    let pool = state.db.clone();
    let review_service = ReviewService::new(pool.clone());

    // Get comment to find attempt for permission check
    let comment = review_service
        .get_comment(comment_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Comment not found".to_string()))?;

    // Get attempt to find task
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(comment.attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    // Unresolve comment
    let unresolved = review_service
        .unresolve_comment(comment_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = get_comment_dto_by_id(&pool, unresolved.id).await?;
    let response = ApiResponse::success(dto, "Comment unresolved successfully");

    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/api/v1/comments/{id}",
    tag = "Reviews",
    params(
        ("id" = Uuid, Path, description = "Comment ID")
    ),
    responses(
        (status = 200, description = "Comment deleted", body = ApiResponse<()>),
        (status = 404, description = "Comment not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_comment(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(comment_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<()>>> {
    let pool = state.db.clone();
    let review_service = ReviewService::new(pool.clone());

    // Get comment to find attempt for permission check
    let comment = review_service
        .get_comment(comment_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Comment not found".to_string()))?;

    // Get attempt to find task
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(comment.attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission - user can only delete their own comments
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    // Delete comment (service will verify ownership)
    review_service
        .delete_comment(comment_id, auth_user.id)
        .await
        .map_err(|e| ApiError::Forbidden(e.to_string()))?;

    let response = ApiResponse::success((), "Comment deleted successfully");
    Ok(Json(response))
}

// ============================================================================
// Request Changes Endpoint
// ============================================================================

#[utoipa::path(
    post,
    path = "/api/v1/attempts/{id}/request-changes",
    tag = "Reviews",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    request_body = RequestChangesRequest,
    responses(
        (status = 201, description = "New attempt created with feedback", body = ApiResponse<RequestChangesResponseDto>),
        (status = 404, description = "Attempt not found"),
        (status = 400, description = "Task not in review state"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn request_changes(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
    Json(payload): Json<RequestChangesRequest>,
) -> ApiResult<(StatusCode, Json<ApiResponse<RequestChangesResponseDto>>)> {
    let pool = state.db.clone();

    // Get attempt
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions and status
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission - need ExecuteTask to request changes
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ExecuteTask,
        &pool,
    )
    .await?;

    // Allow when InReview (normal) or Done (e.g. MR opened but merge failed, user wants agent to resolve conflicts).
    if task.status != TaskStatus::InReview && task.status != TaskStatus::Done {
        return Err(ApiError::BadRequest(format!(
            "Task is not in review or done state (current: {:?})",
            task.status
        )));
    }

    // Only allow follow-up on completed attempts (Success, Failed, Cancelled)
    match attempt.status {
        AttemptStatus::Success | AttemptStatus::Failed | AttemptStatus::Cancelled => {}
        _ => {
            return Err(ApiError::BadRequest(format!(
                "Cannot request changes on attempt in '{:?}' state. Wait for completion.",
                attempt.status
            )));
        }
    }

    // Build feedback with optional comments (follow-up on SAME attempt, no new attempt)
    let review_service = ReviewService::new(pool.clone());
    let (feedback_with_comments, comments_included) = if payload.include_comments {
        let comments = review_service
            .get_comments(attempt_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to load comments: {}", e)))?;
        let formatted = review_service
            .format_comments_as_feedback(attempt_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to format comments: {}", e)))?;
        let full = if formatted.is_empty() {
            payload.feedback.clone()
        } else {
            format!("{}\n{}", payload.feedback, formatted)
        };
        (full, comments.len() as i32)
    } else {
        (payload.feedback.clone(), 0)
    };

    let should_create_new_attempt = task.status == TaskStatus::Done;

    if should_create_new_attempt {
        let (new_attempt_id, comments_included) = review_service
            .create_attempt_with_feedback(
                attempt_id,
                task.id,
                &payload.feedback,
                payload.include_comments,
            )
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to create follow-up attempt: {}", e))
            })?;

        sqlx::query(
            r#"
            UPDATE task_attempts
            SET metadata = metadata || jsonb_build_object('previous_task_status', 'done')
            WHERE id = $1
            "#,
        )
        .bind(new_attempt_id)
        .execute(&pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to persist follow-up metadata: {}", e)))?;

        if let Err(e) = acpms_executors::StatusManager::log(
            &pool,
            &state.broadcast_tx,
            new_attempt_id,
            "user",
            &format!("[Request Changes] {}", feedback_with_comments),
        )
        .await
        {
            tracing::warn!("Failed to log request-changes on new attempt: {}", e);
        }

        if let Err(error) = task_service
            .update_task_status(task.id, TaskStatus::InProgress)
            .await
        {
            return Err(ApiError::Internal(format!(
                "Failed to update task status: {}",
                error
            )));
        }
        openclaw::emit_task_status_changed(
            &state,
            task.project_id,
            task.id,
            task.status,
            TaskStatus::InProgress,
            "routes.reviews.request_changes.new_attempt",
        )
        .await;

        let instruction = format!(
            r#"## Previous Context
You previously worked on this task:
{}

## Review Feedback (Request Changes)
{}

Address the feedback and continue. Build on your previous work."#,
            task.description.as_deref().unwrap_or(&task.title),
            feedback_with_comments
        );

        let project_service = ProjectService::new(pool.clone());
        let project = project_service
            .get_project(task.project_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;
        let repo = std::path::PathBuf::from(std::env::var("WORKTREES_PATH").unwrap_or_else(|_| {
            std::env::var("HOME")
                .ok()
                .map(|h| format!("{}/Projects", h.trim_end_matches('/')))
                .unwrap_or_else(|| "./worktrees".to_string())
        }))
        .join(project_repo_relative_path(
            project.id,
            &project.metadata,
            &project.name,
        ));

        let _ = sqlx::query(
            r#"
            INSERT INTO execution_processes (attempt_id, process_id, worktree_path, branch_name)
            VALUES ($1, NULL, $2, NULL)
            "#,
        )
        .bind(new_attempt_id)
        .bind(repo.to_string_lossy().to_string())
        .execute(&pool)
        .await;

        let orchestrator = state.orchestrator.clone();
        let instr = instruction.clone();
        tokio::spawn(async move {
            if let Err(e) = orchestrator
                .execute_agent_for_attempt(new_attempt_id, &repo, &instr)
                .await
            {
                tracing::error!(
                    "Request-changes follow-up failed for new attempt {}: {:?}",
                    new_attempt_id,
                    e
                );
            }
        });

        let response_dto = RequestChangesResponseDto {
            original_attempt_id: attempt_id,
            new_attempt_id,
            feedback: payload.feedback,
            comments_included,
        };

        let response = ApiResponse::created(
            response_dto,
            "Changes requested, follow-up started in a new attempt",
        );
        return Ok((StatusCode::CREATED, Json(response)));
    }

    // Log user follow-up (request changes) so it appears in timeline
    if let Err(e) = acpms_executors::StatusManager::log(
        &pool,
        &state.broadcast_tx,
        attempt_id,
        "user",
        &format!("[Request Changes] {}", feedback_with_comments),
    )
    .await
    {
        tracing::warn!("Failed to log request-changes: {}", e);
    }

    // Atomic transition: update task status and attempt to running in one go.
    // Prevents race where two concurrent request-changes could both spawn execute_agent_for_attempt.
    if let Err(error) = task_service
        .update_task_status(task.id, TaskStatus::InProgress)
        .await
    {
        return Err(ApiError::Internal(format!(
            "Failed to update task status: {}",
            error
        )));
    }
    openclaw::emit_task_status_changed(
        &state,
        task.project_id,
        task.id,
        task.status,
        TaskStatus::InProgress,
        "routes.reviews.request_changes.resume_attempt",
    )
    .await;
    match attempt_service
        .transition_completed_to_running(attempt_id)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Err(ApiError::Conflict(
                "Attempt already transitioned or not in valid state. Refresh and try again."
                    .to_string(),
            ));
        }
        Err(e) => {
            let msg = e.to_string();
            return Err(ApiError::BadRequest(if msg.contains("active attempt") {
                "Task already has an active attempt (queued or running). Wait for it to complete."
                    .to_string()
            } else {
                format!("Failed to transition attempt: {}", msg)
            }));
        }
    }

    // Broadcast running status
    let _ = state.broadcast_tx.send(acpms_executors::AgentEvent::Status(
        acpms_executors::StatusMessage {
            attempt_id,
            status: AttemptStatus::Running,
            timestamp: Utc::now(),
        },
    ));

    // Build follow-up instruction (reuse worktree via execute_agent_for_attempt)
    let is_merge_conflict = payload.feedback.to_lowercase().contains("merge")
        && (payload.feedback.to_lowercase().contains("conflict")
            || payload.feedback.to_lowercase().contains("pull main")
            || payload.feedback.to_lowercase().contains("resolve"));

    let instruction = if is_merge_conflict {
        format!(
            r#"## CRITICAL: Merge Conflict Resolution (You MUST do this, do NOT suggest user retry on GitLab)

The MR merge failed due to conflicts. You MUST resolve them locally in this worktree.

### Required steps (execute them, do not skip):
1. `git fetch origin`
2. `git rebase origin/main` (or target branch)
3. If conflicts occur: edit the conflicted files, remove conflict markers, keep the correct code
4. After resolving: `git add .` and `git rebase --continue` (or `git commit` if you used merge)
5. `git push --force-with-lease origin HEAD`

Do NOT suggest "refresh/retry on GitLab" or "send me the error". You must run these commands and fix the conflicts.

## Previous Context
You previously worked on this task:
{}

## Review Feedback
{}"#,
            task.description.as_deref().unwrap_or(&task.title),
            feedback_with_comments
        )
    } else {
        format!(
            r#"## Previous Context
You previously worked on this task:
{}

## Review Feedback (Request Changes)
{}

Address the feedback and continue. Build on your previous work."#,
            task.description.as_deref().unwrap_or(&task.title),
            feedback_with_comments
        )
    };

    let project_service = ProjectService::new(pool.clone());
    let project = project_service
        .get_project(task.project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;
    let base = state.worktrees_path.read().await.clone();
    let repo_path = base.join(project_repo_relative_path(
        project.id,
        &project.metadata,
        &project.name,
    ));

    // Spawn follow-up on SAME attempt (reuses worktree if exists)
    let orchestrator = state.orchestrator.clone();
    let aid = attempt_id;
    let instr = instruction.clone();
    let repo = repo_path.clone();
    tokio::spawn(async move {
        if let Err(e) = orchestrator
            .execute_agent_for_attempt(aid, &repo, &instr)
            .await
        {
            tracing::error!(
                "Request-changes follow-up failed for attempt {}: {:?}",
                aid,
                e
            );
        }
    });

    let response_dto = RequestChangesResponseDto {
        original_attempt_id: attempt_id,
        new_attempt_id: attempt_id,
        feedback: payload.feedback,
        comments_included,
    };

    let response = ApiResponse::created(
        response_dto,
        "Changes requested, follow-up sent to same attempt",
    );
    Ok((StatusCode::CREATED, Json(response)))
}
