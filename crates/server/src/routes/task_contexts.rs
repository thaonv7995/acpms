use acpms_db::{models::Task, PgPool};
use acpms_services::{
    CreateTaskContextAttachmentInput, CreateTaskContextInput, TaskContextService, TaskService,
    UpdateTaskContextInput,
};
use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[allow(unused_imports)]
use crate::api::{
    ApiResponse, CreateTaskContextAttachmentRequestDoc, CreateTaskContextRequestDoc,
    TaskContextAttachmentDto, TaskContextAttachmentResponse, TaskContextDto,
    TaskContextListResponse, TaskContextResponse, UpdateTaskContextRequestDoc,
};
use crate::error::{ApiError, ApiResult};
use crate::middleware::{AuthUser, Permission, RbacChecker, ValidatedJson};
use crate::AppState;

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateTaskContextRequest {
    #[validate(length(max = 255, message = "Title must not exceed 255 characters"))]
    pub title: Option<String>,
    #[validate(length(min = 1, max = 64, message = "Content type is required"))]
    pub content_type: String,
    #[validate(length(
        max = 20000,
        message = "Context content must not exceed 20000 characters"
    ))]
    pub raw_content: String,
    #[validate(length(min = 1, max = 32, message = "Source is required"))]
    pub source: String,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct UpdateTaskContextRequest {
    #[validate(length(max = 255, message = "Title must not exceed 255 characters"))]
    pub title: Option<Option<String>>,
    #[validate(length(min = 1, max = 64, message = "Content type is required"))]
    pub content_type: Option<String>,
    #[validate(length(
        max = 20000,
        message = "Context content must not exceed 20000 characters"
    ))]
    pub raw_content: Option<String>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct TaskContextAttachmentUploadUrlRequest {
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
pub struct TaskContextAttachmentUploadUrlResponse {
    pub upload_url: String,
    pub key: String,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateTaskContextAttachmentRequest {
    #[validate(length(
        min = 1,
        max = 512,
        message = "Storage key must be between 1 and 512 characters"
    ))]
    pub storage_key: String,
    #[validate(length(
        min = 1,
        max = 255,
        message = "Filename must be between 1 and 255 characters"
    ))]
    pub filename: String,
    #[validate(length(min = 1, max = 255, message = "Content type is required"))]
    pub content_type: String,
    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct TaskContextAttachmentDownloadUrlRequest {
    #[validate(length(min = 1, max = 512, message = "Key is required"))]
    pub key: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskContextAttachmentDownloadUrlResponse {
    pub download_url: String,
}

fn sanitize_task_context_attachment_filename(filename: &str) -> String {
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

async fn fetch_task_for_permission(
    pool: &PgPool,
    auth_user: &AuthUser,
    task_id: Uuid,
    permission: Permission,
) -> ApiResult<Task> {
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    RbacChecker::check_permission(auth_user.id, task.project_id, permission, pool).await?;
    Ok(task)
}

#[utoipa::path(
    get,
    path = "/api/v1/tasks/{task_id}/contexts",
    tag = "Tasks",
    params(("task_id" = Uuid, Path, description = "Task ID")),
    responses(
        (status = 200, description = "List task contexts", body = TaskContextListResponse),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task not found")
    )
)]
pub async fn list_task_contexts(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<TaskContextDto>>>> {
    fetch_task_for_permission(&pool, &auth_user, task_id, Permission::ViewProject).await?;

    let service = TaskContextService::new(pool);
    let contexts = service
        .list_task_contexts(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .into_iter()
        .map(TaskContextDto::from)
        .collect::<Vec<_>>();

    Ok(Json(ApiResponse::success(
        contexts,
        "Task contexts fetched successfully",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/tasks/{task_id}/contexts",
    tag = "Tasks",
    params(("task_id" = Uuid, Path, description = "Task ID")),
    request_body = CreateTaskContextRequestDoc,
    responses(
        (status = 200, description = "Create task context", body = TaskContextResponse),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task not found")
    )
)]
pub async fn create_task_context(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<CreateTaskContextRequest>,
) -> ApiResult<Json<ApiResponse<TaskContextDto>>> {
    let _task =
        fetch_task_for_permission(&pool, &auth_user, task_id, Permission::ModifyTask).await?;
    let service = TaskContextService::new(pool.clone());

    if service
        .count_task_contexts(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        >= 10
    {
        return Err(ApiError::BadRequest(
            "A task cannot have more than 10 context blocks".to_string(),
        ));
    }

    let context = service
        .create_task_context(
            task_id,
            auth_user.id,
            CreateTaskContextInput {
                title: req.title,
                content_type: req.content_type,
                raw_content: req.raw_content,
                source: req.source,
                sort_order: req.sort_order,
            },
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(ApiResponse::success(
        TaskContextDto::from_parts(context, Vec::new()),
        "Task context created successfully",
    )))
}

#[utoipa::path(
    patch,
    path = "/api/v1/tasks/{task_id}/contexts/{context_id}",
    tag = "Tasks",
    params(
        ("task_id" = Uuid, Path, description = "Task ID"),
        ("context_id" = Uuid, Path, description = "Task context ID")
    ),
    request_body = UpdateTaskContextRequestDoc,
    responses(
        (status = 200, description = "Update task context", body = TaskContextResponse),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task context not found")
    )
)]
pub async fn update_task_context(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path((task_id, context_id)): Path<(Uuid, Uuid)>,
    ValidatedJson(req): ValidatedJson<UpdateTaskContextRequest>,
) -> ApiResult<Json<ApiResponse<TaskContextDto>>> {
    fetch_task_for_permission(&pool, &auth_user, task_id, Permission::ModifyTask).await?;
    let service = TaskContextService::new(pool.clone());

    let context = service
        .update_task_context(
            task_id,
            context_id,
            auth_user.id,
            UpdateTaskContextInput {
                title: req.title,
                content_type: req.content_type,
                raw_content: req.raw_content,
                sort_order: req.sort_order,
            },
        )
        .await
        .map_err(|e| match e.to_string().as_str() {
            "Task context not found" => ApiError::NotFound("Task context not found".to_string()),
            _ => ApiError::Internal(e.to_string()),
        })?;

    let existing = service
        .get_task_context(task_id, context_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task context not found".to_string()))?;

    Ok(Json(ApiResponse::success(
        TaskContextDto::from_parts(
            context,
            existing
                .attachments
                .into_iter()
                .map(TaskContextAttachmentDto::from)
                .collect(),
        ),
        "Task context updated successfully",
    )))
}

#[utoipa::path(
    delete,
    path = "/api/v1/tasks/{task_id}/contexts/{context_id}",
    tag = "Tasks",
    params(
        ("task_id" = Uuid, Path, description = "Task ID"),
        ("context_id" = Uuid, Path, description = "Task context ID")
    ),
    responses(
        (status = 200, description = "Delete task context", body = ApiResponse<()>),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task context not found")
    )
)]
pub async fn delete_task_context(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path((task_id, context_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<()>>> {
    fetch_task_for_permission(&pool, &auth_user, task_id, Permission::ModifyTask).await?;
    let service = TaskContextService::new(pool);

    service
        .delete_task_context(task_id, context_id)
        .await
        .map_err(|e| match e.to_string().as_str() {
            "Task context not found" => ApiError::NotFound("Task context not found".to_string()),
            _ => ApiError::Internal(e.to_string()),
        })?;

    Ok(Json(ApiResponse::success(
        (),
        "Task context deleted successfully",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/tasks/{task_id}/context-attachments/upload-url",
    tag = "Tasks",
    params(("task_id" = Uuid, Path, description = "Task ID")),
    request_body = TaskContextAttachmentUploadUrlRequest,
    responses(
        (status = 200, description = "Create task context attachment upload URL", body = ApiResponse<TaskContextAttachmentUploadUrlResponse>),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task not found")
    )
)]
pub async fn get_task_context_attachment_upload_url(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<TaskContextAttachmentUploadUrlRequest>,
) -> ApiResult<Json<ApiResponse<TaskContextAttachmentUploadUrlResponse>>> {
    let task =
        fetch_task_for_permission(&state.db, &auth_user, task_id, Permission::ModifyTask).await?;
    let safe_name = sanitize_task_context_attachment_filename(&req.filename);
    let key = format!(
        "task-context-attachments/{}/{}/{}-{}",
        task.project_id,
        task_id,
        Uuid::new_v4(),
        safe_name
    );

    let upload_url = state
        .storage_service
        .get_presigned_upload_url(&key, &req.content_type, Duration::from_secs(3600))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(ApiResponse::success(
        TaskContextAttachmentUploadUrlResponse { upload_url, key },
        "Upload URL generated successfully",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/tasks/{task_id}/contexts/{context_id}/attachments",
    tag = "Tasks",
    params(
        ("task_id" = Uuid, Path, description = "Task ID"),
        ("context_id" = Uuid, Path, description = "Task context ID")
    ),
    request_body = CreateTaskContextAttachmentRequestDoc,
    responses(
        (status = 200, description = "Create task context attachment metadata", body = TaskContextAttachmentResponse),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task context not found")
    )
)]
pub async fn create_task_context_attachment(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path((task_id, context_id)): Path<(Uuid, Uuid)>,
    ValidatedJson(req): ValidatedJson<CreateTaskContextAttachmentRequest>,
) -> ApiResult<Json<ApiResponse<TaskContextAttachmentDto>>> {
    let task =
        fetch_task_for_permission(&pool, &auth_user, task_id, Permission::ModifyTask).await?;

    let prefix = format!("task-context-attachments/{}/{}/", task.project_id, task_id);
    if !req.storage_key.starts_with(&prefix) {
        return Err(ApiError::BadRequest(
            "Attachment key does not belong to this task".to_string(),
        ));
    }

    let service = TaskContextService::new(pool);
    let attachment = service
        .create_attachment(
            task_id,
            context_id,
            auth_user.id,
            CreateTaskContextAttachmentInput {
                storage_key: req.storage_key,
                filename: req.filename,
                content_type: req.content_type,
                size_bytes: req.size_bytes,
                checksum: req.checksum,
            },
        )
        .await
        .map_err(|e| match e.to_string().as_str() {
            "Task context not found" => ApiError::NotFound("Task context not found".to_string()),
            _ => ApiError::Internal(e.to_string()),
        })?;

    Ok(Json(ApiResponse::success(
        TaskContextAttachmentDto::from(attachment),
        "Task context attachment created successfully",
    )))
}

#[utoipa::path(
    delete,
    path = "/api/v1/tasks/{task_id}/contexts/{context_id}/attachments/{attachment_id}",
    tag = "Tasks",
    params(
        ("task_id" = Uuid, Path, description = "Task ID"),
        ("context_id" = Uuid, Path, description = "Task context ID"),
        ("attachment_id" = Uuid, Path, description = "Task context attachment ID")
    ),
    responses(
        (status = 200, description = "Delete task context attachment", body = ApiResponse<()>),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task context attachment not found")
    )
)]
pub async fn delete_task_context_attachment(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((task_id, context_id, attachment_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<()>>> {
    fetch_task_for_permission(&state.db, &auth_user, task_id, Permission::ModifyTask).await?;
    let service = TaskContextService::new(state.db.clone());

    let storage_key = service
        .delete_attachment(task_id, context_id, attachment_id)
        .await
        .map_err(|e| match e.to_string().as_str() {
            "Task context attachment not found" => {
                ApiError::NotFound("Task context attachment not found".to_string())
            }
            _ => ApiError::Internal(e.to_string()),
        })?;

    if let Some(storage_key) = storage_key {
        if let Err(error) = state.storage_service.delete_file(&storage_key).await {
            tracing::warn!(
                task_id = %task_id,
                context_id = %context_id,
                attachment_id = %attachment_id,
                error = %error,
                "Failed to delete task context attachment object from storage"
            );
        }
    }

    Ok(Json(ApiResponse::success(
        (),
        "Task context attachment deleted successfully",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/tasks/{task_id}/context-attachments/download-url",
    tag = "Tasks",
    params(("task_id" = Uuid, Path, description = "Task ID")),
    request_body = TaskContextAttachmentDownloadUrlRequest,
    responses(
        (status = 200, description = "Create task context attachment download URL", body = ApiResponse<TaskContextAttachmentDownloadUrlResponse>),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task not found")
    )
)]
pub async fn get_task_context_attachment_download_url(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<TaskContextAttachmentDownloadUrlRequest>,
) -> ApiResult<Json<ApiResponse<TaskContextAttachmentDownloadUrlResponse>>> {
    let task =
        fetch_task_for_permission(&state.db, &auth_user, task_id, Permission::ViewProject).await?;

    let prefix = format!("task-context-attachments/{}/{}/", task.project_id, task_id);
    if !req.key.starts_with(&prefix) {
        return Err(ApiError::BadRequest(
            "Attachment key does not belong to this task".to_string(),
        ));
    }

    let download_url = state
        .storage_service
        .get_presigned_download_url(&req.key, Duration::from_secs(3600))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(ApiResponse::success(
        TaskContextAttachmentDownloadUrlResponse { download_url },
        "Download URL generated successfully",
    )))
}
