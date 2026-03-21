use acpms_services::{
    ProjectDocumentIndexService, ProjectDocumentService, ProjectDocumentServiceError,
    UpdateProjectDocumentInput, UpsertProjectDocumentInput, TASK_DOCUMENT_KINDS,
};
use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::api::{ApiResponse, ProjectDocumentDto};
use crate::error::{ApiError, ApiResult};
use crate::middleware::{AuthUser, Permission, RbacChecker, ValidatedJson};
use crate::AppState;

const MAX_PROJECT_DOCUMENT_SIZE_BYTES: i64 = 5 * 1024 * 1024;
const PROJECT_DOCUMENT_SOURCES: &[&str] = &["upload", "repo_sync", "api"];

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct ProjectDocumentUploadUrlRequest {
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
pub struct ProjectDocumentUploadUrlResponse {
    pub upload_url: String,
    pub key: String,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateProjectDocumentRequest {
    #[validate(length(
        min = 1,
        max = 255,
        message = "Title must be between 1 and 255 characters"
    ))]
    pub title: String,
    #[validate(length(
        min = 1,
        max = 255,
        message = "Filename must be between 1 and 255 characters"
    ))]
    pub filename: String,
    #[validate(length(min = 1, max = 32, message = "Document kind is required"))]
    pub document_kind: String,
    #[validate(length(min = 1, max = 255, message = "Content type is required"))]
    pub content_type: String,
    #[validate(length(
        min = 1,
        max = 512,
        message = "Storage key must be between 1 and 512 characters"
    ))]
    pub storage_key: Option<String>,
    pub checksum: Option<String>,
    pub size_bytes: Option<i64>,
    #[validate(length(min = 1, message = "Document content must not be empty"))]
    pub content_text: Option<String>,
    #[validate(length(min = 1, max = 32, message = "Source is required"))]
    pub source: String,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct UpdateProjectDocumentRequest {
    #[validate(length(
        min = 1,
        max = 255,
        message = "Title must be between 1 and 255 characters"
    ))]
    pub title: Option<String>,
    #[validate(length(min = 1, max = 32, message = "Document kind is required"))]
    pub document_kind: Option<String>,
    #[validate(length(min = 1, max = 255, message = "Content type is required"))]
    pub content_type: Option<String>,
    #[validate(length(
        min = 1,
        max = 512,
        message = "Storage key must be between 1 and 512 characters"
    ))]
    pub storage_key: Option<String>,
    pub checksum: Option<Option<String>>,
    pub size_bytes: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct ProjectDocumentDownloadUrlRequest {
    #[validate(length(min = 1, max = 512, message = "Key is required"))]
    pub key: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectDocumentDownloadUrlResponse {
    pub download_url: String,
}

fn sanitize_project_document_filename(filename: &str) -> String {
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
        "document.md".to_string()
    } else {
        trimmed.to_string()
    }
}

fn project_document_storage_prefix(project_id: Uuid) -> String {
    format!("project-documents/{project_id}/")
}

fn openclaw_project_document_storage_key(project_id: Uuid, filename: &str) -> String {
    let safe_name = sanitize_project_document_filename(filename);
    format!(
        "{}openclaw/{}-{}",
        project_document_storage_prefix(project_id),
        Uuid::new_v4(),
        safe_name
    )
}

struct ResolvedProjectDocumentUpload {
    storage_key: String,
    checksum: Option<String>,
    size_bytes: i64,
}

fn validate_project_document_kind(kind: &str) -> ApiResult<()> {
    if TASK_DOCUMENT_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(ApiError::BadRequest("Invalid document kind".into()))
    }
}

fn validate_project_document_source(source: &str) -> ApiResult<()> {
    if PROJECT_DOCUMENT_SOURCES.contains(&source) {
        Ok(())
    } else {
        Err(ApiError::BadRequest("Invalid document source".into()))
    }
}

fn validate_project_document_storage_key(project_id: Uuid, key: &str) -> ApiResult<()> {
    if key.starts_with(&project_document_storage_prefix(project_id)) {
        Ok(())
    } else {
        Err(ApiError::BadRequest("Invalid document storage key".into()))
    }
}

fn validate_project_document_size(size_bytes: i64) -> ApiResult<()> {
    if size_bytes < 0 {
        return Err(ApiError::BadRequest(
            "Document size must be non-negative".into(),
        ));
    }
    if size_bytes > MAX_PROJECT_DOCUMENT_SIZE_BYTES {
        return Err(ApiError::BadRequest("Document exceeds 5 MiB limit".into()));
    }
    Ok(())
}

fn map_project_document_error(error: ProjectDocumentServiceError) -> ApiError {
    match error {
        ProjectDocumentServiceError::NotFound => {
            ApiError::NotFound("Project document not found".into())
        }
        ProjectDocumentServiceError::TitleConflict => {
            ApiError::Conflict("A different filename already uses this title".into())
        }
        ProjectDocumentServiceError::Other(error) => ApiError::Internal(error.to_string()),
    }
}

async fn maybe_index_project_document(
    state: &AppState,
    document: acpms_db::models::ProjectDocument,
) -> acpms_db::models::ProjectDocument {
    if document.ingestion_status != "pending" {
        return document;
    }

    let index_service =
        ProjectDocumentIndexService::new(state.db.clone(), state.storage_service.clone());
    match index_service.index_document(&document).await {
        Ok(updated) => updated,
        Err(error) => {
            tracing::warn!(
                project_id = %document.project_id,
                document_id = %document.id,
                error = %error,
                "Project document indexing failed"
            );
            ProjectDocumentService::new(state.db.clone())
                .get_project_document(document.project_id, document.id)
                .await
                .ok()
                .flatten()
                .unwrap_or(document)
        }
    }
}

async fn resolve_project_document_upload(
    state: &AppState,
    project_id: Uuid,
    req: &CreateProjectDocumentRequest,
) -> ApiResult<ResolvedProjectDocumentUpload> {
    let uses_inline_content = req.content_text.is_some();
    let has_storage_metadata =
        req.storage_key.is_some() || req.size_bytes.is_some() || req.checksum.is_some();

    if uses_inline_content && has_storage_metadata {
        return Err(ApiError::BadRequest(
            "Provide either content_text or storage-backed upload metadata, not both".into(),
        ));
    }

    if let Some(content_text) = req.content_text.as_deref() {
        if req.source != "api" {
            return Err(ApiError::BadRequest(
                "Inline content_text is only supported for source='api'".into(),
            ));
        }

        let size_bytes = i64::try_from(content_text.as_bytes().len())
            .map_err(|_| ApiError::BadRequest("Document exceeds supported size".into()))?;
        validate_project_document_size(size_bytes)?;

        let storage_key = openclaw_project_document_storage_key(project_id, &req.filename);
        state
            .storage_service
            .upload_text(&storage_key, content_text, &req.content_type)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        return Ok(ResolvedProjectDocumentUpload {
            storage_key,
            checksum: None,
            size_bytes,
        });
    }

    let storage_key = req.storage_key.clone().ok_or_else(|| {
        ApiError::BadRequest("storage_key is required when content_text is omitted".into())
    })?;
    let size_bytes = req.size_bytes.ok_or_else(|| {
        ApiError::BadRequest("size_bytes is required when content_text is omitted".into())
    })?;

    validate_project_document_size(size_bytes)?;
    validate_project_document_storage_key(project_id, &storage_key)?;

    Ok(ResolvedProjectDocumentUpload {
        storage_key,
        checksum: req.checksum.clone(),
        size_bytes,
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/documents",
    tag = "Project Documents",
    params(("project_id" = Uuid, Path, description = "Project ID")),
    responses(
        (status = 200, description = "Project documents retrieved", body = crate::api::ProjectDocumentListResponse),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_project_documents(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<ProjectDocumentDto>>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let service = ProjectDocumentService::new(state.db.clone());
    let documents = service
        .list_project_documents(project_id)
        .await
        .map_err(map_project_document_error)?;

    Ok(Json(ApiResponse::success(
        documents
            .into_iter()
            .map(ProjectDocumentDto::from)
            .collect(),
        "Project documents retrieved successfully",
    )))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/documents/{document_id}",
    tag = "Project Documents",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("document_id" = Uuid, Path, description = "Document ID")
    ),
    responses(
        (status = 200, description = "Project document retrieved", body = crate::api::ProjectDocumentResponse),
        (status = 404, description = "Document not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_project_document(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, document_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<ProjectDocumentDto>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let service = ProjectDocumentService::new(state.db.clone());
    let document = service
        .get_project_document(project_id, document_id)
        .await
        .map_err(map_project_document_error)?
        .ok_or_else(|| ApiError::NotFound("Project document not found".into()))?;

    Ok(Json(ApiResponse::success(
        ProjectDocumentDto::from(document),
        "Project document retrieved successfully",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/documents/upload-url",
    tag = "Project Documents",
    params(("project_id" = Uuid, Path, description = "Project ID")),
    request_body = ProjectDocumentUploadUrlRequest,
    responses(
        (status = 200, description = "Upload URL created", body = ApiResponse<ProjectDocumentUploadUrlResponse>),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid input")
    )
)]
pub async fn get_project_document_upload_url(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<ProjectDocumentUploadUrlRequest>,
) -> ApiResult<Json<ApiResponse<ProjectDocumentUploadUrlResponse>>> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    let safe_name = sanitize_project_document_filename(&req.filename);
    let key = format!(
        "project-documents/{}/{}/{}-{}",
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

    Ok(Json(ApiResponse::success(
        ProjectDocumentUploadUrlResponse { upload_url, key },
        "Upload URL generated successfully",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/documents",
    tag = "Project Documents",
    params(("project_id" = Uuid, Path, description = "Project ID")),
    request_body = crate::api::CreateProjectDocumentRequestDoc,
    responses(
        (status = 200, description = "Project document created or updated", body = crate::api::ProjectDocumentResponse),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Title conflict"),
        (status = 400, description = "Invalid input")
    )
)]
pub async fn create_or_upsert_project_document(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<CreateProjectDocumentRequest>,
) -> ApiResult<Json<ApiResponse<ProjectDocumentDto>>> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    validate_project_document_kind(&req.document_kind)?;
    validate_project_document_source(&req.source)?;
    let resolved_upload = resolve_project_document_upload(&state, project_id, &req).await?;

    let CreateProjectDocumentRequest {
        title,
        filename,
        document_kind,
        content_type,
        source,
        ..
    } = req;

    let service = ProjectDocumentService::new(state.db.clone());
    let document = service
        .create_or_upsert_project_document(
            project_id,
            auth_user.id,
            UpsertProjectDocumentInput {
                title,
                filename,
                document_kind,
                content_type,
                storage_key: resolved_upload.storage_key,
                checksum: resolved_upload.checksum,
                size_bytes: resolved_upload.size_bytes,
                source,
            },
        )
        .await
        .map_err(map_project_document_error)?;
    let document = maybe_index_project_document(&state, document).await;

    Ok(Json(ApiResponse::success(
        ProjectDocumentDto::from(document),
        "Project document saved successfully",
    )))
}

#[utoipa::path(
    patch,
    path = "/api/v1/projects/{project_id}/documents/{document_id}",
    tag = "Project Documents",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("document_id" = Uuid, Path, description = "Document ID")
    ),
    request_body = crate::api::UpdateProjectDocumentRequestDoc,
    responses(
        (status = 200, description = "Project document updated", body = crate::api::ProjectDocumentResponse),
        (status = 404, description = "Document not found"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Title conflict"),
        (status = 400, description = "Invalid input")
    )
)]
pub async fn update_project_document(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, document_id)): Path<(Uuid, Uuid)>,
    ValidatedJson(req): ValidatedJson<UpdateProjectDocumentRequest>,
) -> ApiResult<Json<ApiResponse<ProjectDocumentDto>>> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    if let Some(document_kind) = req.document_kind.as_deref() {
        validate_project_document_kind(document_kind)?;
    }
    if let Some(size_bytes) = req.size_bytes {
        validate_project_document_size(size_bytes)?;
    }
    if let Some(storage_key) = req.storage_key.as_deref() {
        validate_project_document_storage_key(project_id, storage_key)?;
    }

    let service = ProjectDocumentService::new(state.db.clone());
    let document = service
        .update_project_document(
            project_id,
            document_id,
            auth_user.id,
            UpdateProjectDocumentInput {
                title: req.title,
                document_kind: req.document_kind,
                content_type: req.content_type,
                storage_key: req.storage_key,
                checksum: req.checksum,
                size_bytes: req.size_bytes,
            },
        )
        .await
        .map_err(map_project_document_error)?;
    let document = maybe_index_project_document(&state, document).await;

    Ok(Json(ApiResponse::success(
        ProjectDocumentDto::from(document),
        "Project document updated successfully",
    )))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/documents/{document_id}",
    tag = "Project Documents",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("document_id" = Uuid, Path, description = "Document ID")
    ),
    responses(
        (status = 200, description = "Project document deleted", body = crate::api::EmptyResponse),
        (status = 404, description = "Document not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_project_document(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, document_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<()>>> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    let service = ProjectDocumentService::new(state.db.clone());
    let deleted = service
        .delete_project_document(project_id, document_id)
        .await
        .map_err(map_project_document_error)?
        .ok_or_else(|| ApiError::NotFound("Project document not found".into()))?;

    if let Err(error) = state
        .storage_service
        .delete_file(&deleted.storage_key)
        .await
    {
        tracing::warn!(
            "Failed to delete project document object {}: {}",
            deleted.storage_key,
            error
        );
    }

    Ok(Json(ApiResponse::success(
        (),
        "Project document deleted successfully",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/documents/download-url",
    tag = "Project Documents",
    params(("project_id" = Uuid, Path, description = "Project ID")),
    request_body = ProjectDocumentDownloadUrlRequest,
    responses(
        (status = 200, description = "Download URL created", body = ApiResponse<ProjectDocumentDownloadUrlResponse>),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid input")
    )
)]
pub async fn get_project_document_download_url(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<ProjectDocumentDownloadUrlRequest>,
) -> ApiResult<Json<ApiResponse<ProjectDocumentDownloadUrlResponse>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    validate_project_document_storage_key(project_id, &req.key)?;

    let download_url = state
        .storage_service
        .get_presigned_download_url(&req.key, Duration::from_secs(3600))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(ApiResponse::success(
        ProjectDocumentDownloadUrlResponse { download_url },
        "Download URL generated successfully",
    )))
}
