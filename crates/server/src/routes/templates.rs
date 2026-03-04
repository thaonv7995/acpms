//! Project Templates API Routes
//!
//! Endpoints for managing project templates:
//! - GET /api/v1/templates - List all templates
//! - GET /api/v1/templates/{id} - Get template by ID
//! - POST /api/v1/templates - Create new template (admin only)
//! - PUT /api/v1/templates/{id} - Update template (admin only)
//! - DELETE /api/v1/templates/{id} - Delete template (admin only)

use acpms_db::models::*;
use acpms_db::PgPool;
use acpms_services::{ProjectTemplateService, UserService};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::api::ApiResponse;
use crate::error::{ApiError, ApiResult};
use crate::middleware::AuthUser;
use crate::AppState;

/// Create routes for templates
pub fn create_routes() -> Router<AppState> {
    Router::new()
        .route("/templates", get(list_templates).post(create_template))
        .route(
            "/templates/:id",
            get(get_template)
                .put(update_template)
                .delete(delete_template),
        )
}

/// Helper function to check if user is admin
async fn is_admin(pool: &PgPool, user_id: Uuid) -> Result<bool, ApiError> {
    let user_service = UserService::new(pool.clone());
    let user = user_service
        .get_user_by_id(user_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::Unauthorized)?;

    Ok(user.global_roles.contains(&SystemRole::Admin))
}

/// List all project templates
#[utoipa::path(
    get,
    path = "/api/v1/templates",
    tag = "Templates",
    params(
        ("project_type" = Option<String>, Query, description = "Filter by project type"),
        ("official_only" = Option<bool>, Query, description = "Filter by official templates only")
    ),
    responses(
        (status = 200, description = "List of templates")
    )
)]
pub async fn list_templates(
    State(pool): State<PgPool>,
    _auth_user: AuthUser,
    Query(query): Query<ListTemplatesQuery>,
) -> ApiResult<Json<ApiResponse<Vec<ProjectTemplate>>>> {
    let service = ProjectTemplateService::new(pool);
    let templates = service
        .list_templates(query)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = ApiResponse::success(templates, "Templates retrieved successfully");
    Ok(Json(response))
}

/// Get a single template by ID
#[utoipa::path(
    get,
    path = "/api/v1/templates/{id}",
    tag = "Templates",
    params(
        ("id" = Uuid, Path, description = "Template ID")
    ),
    responses(
        (status = 200, description = "Template details"),
        (status = 404, description = "Template not found")
    )
)]
pub async fn get_template(
    State(pool): State<PgPool>,
    _auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<ProjectTemplate>>> {
    let service = ProjectTemplateService::new(pool);
    let template = service
        .get_template(id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Template not found".to_string()))?;

    let response = ApiResponse::success(template, "Template retrieved successfully");
    Ok(Json(response))
}

/// Create a new template (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/templates",
    tag = "Templates",
    request_body = CreateProjectTemplateRequest,
    responses(
        (status = 201, description = "Template created successfully"),
        (status = 403, description = "Forbidden - admin only")
    )
)]
pub async fn create_template(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Json(req): Json<CreateProjectTemplateRequest>,
) -> ApiResult<(StatusCode, Json<ApiResponse<ProjectTemplate>>)> {
    // Check if user is admin (only admins can create templates)
    if !is_admin(&pool, auth_user.id).await? {
        return Err(ApiError::Forbidden(
            "Only admins can create templates".to_string(),
        ));
    }

    let service = ProjectTemplateService::new(pool);
    let template = service
        .create_template(auth_user.id, req)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = ApiResponse::created(template, "Template created successfully");
    Ok((StatusCode::CREATED, Json(response)))
}

/// Update an existing template (admin only)
#[utoipa::path(
    put,
    path = "/api/v1/templates/{id}",
    tag = "Templates",
    params(
        ("id" = Uuid, Path, description = "Template ID")
    ),
    request_body = UpdateProjectTemplateRequest,
    responses(
        (status = 200, description = "Template updated successfully"),
        (status = 403, description = "Forbidden - admin only"),
        (status = 404, description = "Template not found")
    )
)]
pub async fn update_template(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProjectTemplateRequest>,
) -> ApiResult<Json<ApiResponse<ProjectTemplate>>> {
    // Check if user is admin
    if !is_admin(&pool, auth_user.id).await? {
        return Err(ApiError::Forbidden(
            "Only admins can update templates".to_string(),
        ));
    }

    let service = ProjectTemplateService::new(pool);
    let template = service
        .update_template(id, req)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = ApiResponse::success(template, "Template updated successfully");
    Ok(Json(response))
}

/// Delete a template (admin only)
#[utoipa::path(
    delete,
    path = "/api/v1/templates/{id}",
    tag = "Templates",
    params(
        ("id" = Uuid, Path, description = "Template ID")
    ),
    responses(
        (status = 200, description = "Template deleted successfully"),
        (status = 403, description = "Forbidden - admin only"),
        (status = 404, description = "Template not found")
    )
)]
pub async fn delete_template(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<()>>> {
    // Check if user is admin
    if !is_admin(&pool, auth_user.id).await? {
        return Err(ApiError::Forbidden(
            "Only admins can delete templates".to_string(),
        ));
    }

    let service = ProjectTemplateService::new(pool);
    service
        .delete_template(id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = ApiResponse::success((), "Template deleted successfully");
    Ok(Json(response))
}
