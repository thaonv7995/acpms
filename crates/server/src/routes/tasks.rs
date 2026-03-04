use acpms_db::{models::*, PgPool};
use acpms_services::TaskService;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;
use validator::Validate;

use crate::api::{ApiResponse, TaskDto};
use crate::error::{ApiError, ApiResult};
use crate::middleware::{AuthUser, Permission, RbacChecker, ValidatedJson};
use crate::AppState;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams, Default)]
#[serde(default)]
pub struct ListTasksQuery {
    pub project_id: Option<Uuid>,
    /// Optional sprint ID to filter tasks. If provided, only tasks in this sprint are returned.
    pub sprint_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct UpdateStatusRequest {
    #[schema(value_type = String)]
    pub status: TaskStatus,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct AssignTaskRequest {
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct UpdateMetadataRequest {
    #[schema(value_type = String)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct TaskAttachmentUploadUrlRequest {
    pub project_id: Uuid,
    #[validate(length(
        min = 1,
        max = 255,
        message = "Filename must be between 1 and 255 characters"
    ))]
    pub filename: String,
    #[validate(length(min = 1, max = 255, message = "Content type is required"))]
    pub content_type: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskAttachmentUploadUrlResponse {
    pub upload_url: String,
    pub key: String,
}

fn sanitize_task_attachment_filename(filename: &str) -> String {
    let mut out = String::with_capacity(filename.len());
    for ch in filename.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "attachment.bin".to_string()
    } else {
        trimmed.to_string()
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/tasks",
    tag = "Tasks",
    request_body = CreateTaskRequestDoc,
    responses(
        (status = 201, description = "Task created successfully", body = TaskResponse),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid input")
    )
)]
pub async fn create_task(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Json(req): Json<CreateTaskRequest>,
) -> ApiResult<(StatusCode, Json<ApiResponse<TaskDto>>)> {
    // Check permission using RBAC
    RbacChecker::check_permission(auth_user.id, req.project_id, Permission::CreateTask, &pool)
        .await?;

    let task_service = TaskService::new(pool);
    let task = task_service
        .create_task(auth_user.id, req)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = TaskDto::from(task);
    let response = ApiResponse::created(dto, "Task created successfully");

    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    post,
    path = "/api/v1/tasks/attachments/upload-url",
    tag = "Tasks",
    request_body = TaskAttachmentUploadUrlRequest,
    responses(
        (status = 200, description = "Upload URL created", body = ApiResponse<TaskAttachmentUploadUrlResponse>),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid input")
    )
)]
pub async fn get_task_attachment_upload_url(
    auth_user: AuthUser,
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<TaskAttachmentUploadUrlRequest>,
) -> ApiResult<Json<ApiResponse<TaskAttachmentUploadUrlResponse>>> {
    RbacChecker::check_permission(
        auth_user.id,
        req.project_id,
        Permission::ViewProject,
        &state.db,
    )
    .await?;

    let safe_name = sanitize_task_attachment_filename(&req.filename);
    let key = format!(
        "task-attachments/{}/{}/{}-{}",
        req.project_id,
        auth_user.id,
        Uuid::new_v4(),
        safe_name
    );

    let upload_url = state
        .storage_service
        .get_presigned_upload_url(&key, &req.content_type, Duration::from_secs(3600))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = TaskAttachmentUploadUrlResponse { upload_url, key };
    Ok(Json(ApiResponse::success(
        response,
        "Upload URL generated successfully",
    )))
}

#[utoipa::path(
    get,
    path = "/api/v1/tasks",
    tag = "Tasks",
    params(
        ListTasksQuery
    ),
    responses(
        (status = 200, description = "List tasks", body = TaskListResponse),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_tasks(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<ListTasksQuery>,
) -> ApiResult<Json<ApiResponse<Vec<TaskDto>>>> {
    let pool = state.db.clone();
    let task_service = TaskService::new(pool.clone());

    let tasks = if let Some(project_id) = query.project_id {
        // Check permission using RBAC
        RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &pool)
            .await?;

        // Sync MR status from GitLab (fire-and-forget) so Kanban reflects merged tasks without opening diff view
        let sync = state.gitlab_sync_service.clone();
        let orchestrator = state.orchestrator.clone();
        let pid = project_id;
        tokio::spawn(async move {
            match sync.sync_mr_status_for_project(pid).await {
                Ok(attempt_ids) => {
                    for attempt_id in attempt_ids {
                        if let Err(e) = orchestrator.cleanup_worktree_public(attempt_id).await {
                            tracing::warn!("Worktree cleanup after Kanban sync: {}", e);
                        }
                    }
                }
                Err(e) => tracing::debug!("sync_mr_status_for_project: {}", e),
            }
        });

        // Use the new query that includes attempt status for kanban display
        // Now supports sprint filtering
        task_service
            .get_project_tasks_with_attempt_status(project_id, query.sprint_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
    } else {
        // No project_id provided, fetch all tasks for the user across projects
        task_service
            .get_all_user_tasks_with_attempt_status(
                auth_user.id,
                false, // TODO: Fetch from actual user roles if needed
            )
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
    };

    let dtos: Vec<TaskDto> = tasks.into_iter().map(TaskDto::from).collect();
    let response = ApiResponse::success(dtos, "Tasks retrieved successfully");

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/tasks/{id}",
    tag = "Tasks",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Get task details", body = TaskResponse),
        (status = 404, description = "Task not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_task(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<TaskDto>>> {
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    let dto = TaskDto::from(task);
    let response = ApiResponse::success(dto, "Task retrieved successfully");

    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/v1/tasks/{id}",
    tag = "Tasks",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    request_body = UpdateTaskRequestDoc,
    responses(
        (status = 200, description = "Update task", body = TaskResponse),
        (status = 404, description = "Task not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_task(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
    Json(req): Json<UpdateTaskRequest>,
) -> ApiResult<Json<ApiResponse<TaskDto>>> {
    let task_service = TaskService::new(pool.clone());
    let existing_task = task_service
        .get_task(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        existing_task.project_id,
        Permission::ModifyTask,
        &pool,
    )
    .await?;

    let task = task_service
        .update_task(task_id, req)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = TaskDto::from(task);
    let response = ApiResponse::success(dto, "Task updated successfully");

    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/api/v1/tasks/{id}",
    tag = "Tasks",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Delete task", body = EmptyResponse),
        (status = 404, description = "Task not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_task(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<()>>> {
    let task_service = TaskService::new(pool.clone());
    let existing_task = task_service
        .get_task(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        existing_task.project_id,
        Permission::DeleteTask,
        &pool,
    )
    .await?;

    task_service
        .delete_task(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = ApiResponse::success((), "Task deleted successfully");

    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/v1/tasks/{id}/status",
    tag = "Tasks",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    request_body = UpdateStatusRequest,
    responses(
        (status = 200, description = "Update task status", body = TaskResponse),
        (status = 404, description = "Task not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_task_status(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<UpdateStatusRequest>,
) -> ApiResult<Json<ApiResponse<TaskDto>>> {
    let task_service = TaskService::new(pool.clone());
    let existing_task = task_service
        .get_task(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        existing_task.project_id,
        Permission::ModifyTask,
        &pool,
    )
    .await?;

    let task = task_service
        .update_task_status(task_id, req.status)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let dto = TaskDto::from(task);
    let response = ApiResponse::success(dto, "Task status updated successfully");

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/tasks/{id}/children",
    tag = "Tasks",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Get child tasks", body = TaskListResponse),
        (status = 404, description = "Task not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_task_children(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<TaskDto>>>> {
    let task_service = TaskService::new(pool.clone());
    let existing_task = task_service
        .get_task(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        existing_task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    let children = task_service
        .get_children(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dtos: Vec<TaskDto> = children.into_iter().map(TaskDto::from).collect();
    let response = ApiResponse::success(dtos, "Children tasks retrieved successfully");

    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/v1/tasks/{id}/assign",
    tag = "Tasks",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    request_body = AssignTaskRequest,
    responses(
        (status = 200, description = "Assign task", body = TaskResponse),
        (status = 404, description = "Task not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn assign_task(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<AssignTaskRequest>,
) -> ApiResult<Json<ApiResponse<TaskDto>>> {
    let task_service = TaskService::new(pool.clone());
    let existing_task = task_service
        .get_task(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        existing_task.project_id,
        Permission::ModifyTask,
        &pool,
    )
    .await?;

    let task = task_service
        .assign_task(task_id, req.user_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = TaskDto::from(task);
    let response = ApiResponse::success(dto, "Task assigned successfully");

    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/v1/tasks/{id}/metadata",
    tag = "Tasks",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    request_body = UpdateMetadataRequest,
    responses(
        (status = 200, description = "Update task metadata", body = TaskResponse),
        (status = 404, description = "Task not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_task_metadata(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<UpdateMetadataRequest>,
) -> ApiResult<Json<ApiResponse<TaskDto>>> {
    let task_service = TaskService::new(pool.clone());
    let existing_task = task_service
        .get_task(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        existing_task.project_id,
        Permission::ModifyTask,
        &pool,
    )
    .await?;

    let task = task_service
        .update_metadata(task_id, req.metadata)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = TaskDto::from(task);
    let response = ApiResponse::success(dto, "Task metadata updated successfully");

    Ok(Json(response))
}
