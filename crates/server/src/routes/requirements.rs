use acpms_db::{models::*, PgPool};
use acpms_executors::ExecutorOrchestrator;
use acpms_services::RequirementService;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    api::{ApiResponse, RequirementDto},
    error::{ApiError, ApiResult},
    middleware::{AuthUser, Permission, RbacChecker, ValidatedJson},
    AppState,
};

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct RequirementAttachmentUploadUrlRequest {
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
pub struct RequirementAttachmentUploadUrlResponse {
    pub upload_url: String,
    pub key: String,
}

fn sanitize_attachment_filename(filename: &str) -> String {
    let mut out = String::with_capacity(filename.len());
    for ch in filename.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
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
    path = "/api/v1/projects/{project_id}/requirements",
    tag = "Requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    request_body = CreateRequirementRequestDoc,
    responses(
        (status = 201, description = "Requirement created successfully", body = RequirementResponse),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid input")
    )
)]
pub async fn create_requirement(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Json(req): Json<CreateRequirementRequest>,
) -> ApiResult<(StatusCode, Json<ApiResponse<RequirementDto>>)> {
    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        req.project_id,
        Permission::CreateRequirement,
        &pool,
    )
    .await?;

    let service = RequirementService::new(pool);
    let requirement = service
        .create_requirement(auth_user.id, req)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = RequirementDto::from(requirement);
    let response = ApiResponse::created(dto, "Requirement created successfully");

    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/requirements",
    tag = "Requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "List requirements", body = RequirementListResponse),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_project_requirements(
    State(pool): State<PgPool>,
    State(orchestrator): State<Arc<ExecutorOrchestrator>>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<RequirementDto>>>> {
    // Check permission using RBAC
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &pool).await?;

    let service = RequirementService::new(pool);
    let requirements = service
        .get_project_requirements(project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    if requirements.is_empty() {
        // Spawn seeding in background so GET stays fast and predictable.
        // User can refresh to see seeded requirements once bootstrap completes.
        let orch = orchestrator.clone();
        tokio::spawn(async move {
            if let Err(e) = orch.ensure_project_context_seeded(project_id).await {
                tracing::warn!(
                    "Background seed of architecture/PRD for project {} failed: {}",
                    project_id,
                    e
                );
            }
        });
    }

    let dtos: Vec<RequirementDto> = requirements.into_iter().map(RequirementDto::from).collect();
    let response = ApiResponse::success(dtos, "Requirements retrieved successfully");

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/requirements/{id}",
    tag = "Requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("id" = Uuid, Path, description = "Requirement ID")
    ),
    responses(
        (status = 200, description = "Get requirement details", body = RequirementResponse),
        (status = 404, description = "Requirement not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_requirement(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path((project_id, requirement_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<RequirementDto>>> {
    // Check permission using RBAC
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &pool).await?;

    let service = RequirementService::new(pool);
    let requirement = service
        .get_requirement(requirement_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = RequirementDto::from(requirement);
    let response = ApiResponse::success(dto, "Requirement retrieved successfully");

    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/requirements/{id}",
    tag = "Requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("id" = Uuid, Path, description = "Requirement ID")
    ),
    request_body = UpdateRequirementRequestDoc,
    responses(
        (status = 200, description = "Update requirement", body = RequirementResponse),
        (status = 404, description = "Requirement not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_requirement(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path((project_id, requirement_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateRequirementRequest>,
) -> ApiResult<Json<ApiResponse<RequirementDto>>> {
    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ModifyRequirement,
        &pool,
    )
    .await?;

    let service = RequirementService::new(pool);
    let requirement = service
        .update_requirement(requirement_id, req)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = RequirementDto::from(requirement);
    let response = ApiResponse::success(dto, "Requirement updated successfully");

    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/requirements/{id}",
    tag = "Requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("id" = Uuid, Path, description = "Requirement ID")
    ),
    responses(
        (status = 200, description = "Delete requirement", body = EmptyResponse),
        (status = 404, description = "Requirement not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_requirement(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path((project_id, requirement_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<()>>> {
    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::DeleteRequirement,
        &pool,
    )
    .await?;

    let service = RequirementService::new(pool);
    service
        .delete_requirement(requirement_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = ApiResponse::success((), "Requirement deleted successfully");

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/requirements/attachments/upload-url",
    tag = "Requirements",
    params(("project_id" = Uuid, Path, description = "Project ID")),
    request_body = RequirementAttachmentUploadUrlRequest,
    responses(
        (status = 200, description = "Upload URL created", body = ApiResponse<RequirementAttachmentUploadUrlResponse>),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid input")
    )
)]
pub async fn get_requirement_attachment_upload_url(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<RequirementAttachmentUploadUrlRequest>,
) -> ApiResult<Json<ApiResponse<RequirementAttachmentUploadUrlResponse>>> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::CreateRequirement,
        &state.db,
    )
    .await?;

    let safe_name = sanitize_attachment_filename(&req.filename);
    let key = format!(
        "requirement-attachments/{}/{}/{}-{}",
        project_id,
        auth_user.id,
        Uuid::new_v4(),
        safe_name
    );

    let upload_url = state
        .storage_service
        .get_presigned_upload_url(&key, &req.content_type, Duration::from_secs(3600))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = RequirementAttachmentUploadUrlResponse { upload_url, key };
    Ok(Json(ApiResponse::success(
        response,
        "Upload URL generated successfully",
    )))
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct RequirementAttachmentDownloadUrlRequest {
    #[validate(length(min = 1, max = 512, message = "Key is required"))]
    pub key: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RequirementAttachmentDownloadUrlResponse {
    pub download_url: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/requirements/attachments/download-url",
    tag = "Requirements",
    params(("project_id" = Uuid, Path, description = "Project ID")),
    request_body = RequirementAttachmentDownloadUrlRequest,
    responses(
        (status = 200, description = "Download URL created", body = ApiResponse<RequirementAttachmentDownloadUrlResponse>),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid input")
    )
)]
pub async fn get_requirement_attachment_download_url(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<RequirementAttachmentDownloadUrlRequest>,
) -> ApiResult<Json<ApiResponse<RequirementAttachmentDownloadUrlResponse>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    if !req.key.starts_with("requirement-attachments/") {
        return Err(ApiError::BadRequest("Invalid attachment key".into()));
    }

    let download_url = state
        .storage_service
        .get_presigned_download_url(&req.key, Duration::from_secs(3600))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = RequirementAttachmentDownloadUrlResponse { download_url };
    Ok(Json(ApiResponse::success(
        response,
        "Download URL generated successfully",
    )))
}
