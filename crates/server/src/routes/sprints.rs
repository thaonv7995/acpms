#[allow(unused_imports)]
use crate::api::{
    ApiResponse, CloseSprintRequestDoc, CloseSprintResultResponse, SprintDto,
    SprintOverviewResponse,
};
use crate::error::{ApiError, ApiResult};
use crate::middleware::{AuthUser, Permission, RbacChecker};
use crate::AppState;
use acpms_db::models::{
    CloseSprintRequest, CreateSprintRequest, GenerateSprintsRequest, SprintOverview,
    UpdateSprintRequest,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateSprintPayload {
    pub sequence: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub goal: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateSprintsPayload {
    pub start_date: DateTime<Utc>,
    pub duration_weeks: i32,
    pub count: i32,
}

fn map_sprint_service_error(err: anyhow::Error) -> ApiError {
    let msg = err.to_string();
    let lower = msg.to_ascii_lowercase();

    if lower.contains("not found") {
        ApiError::NotFound(msg)
    } else if lower.contains("duplicate key")
        || lower.contains("already exists")
        || lower.contains("unique constraint")
    {
        ApiError::Conflict(msg)
    } else if lower.contains("cannot")
        || lower.contains("must")
        || lower.contains("missing")
        || lower.contains("invalid")
        || lower.contains("only active")
    {
        ApiError::BadRequest(msg)
    } else {
        ApiError::Internal(msg)
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/sprints",
    tag = "Sprints",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "List sprints", body = SprintListResponse),
        (status = 404, description = "Project not found")
    )
)]
pub async fn list_project_sprints(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<SprintDto>>>> {
    // Check permission using RBAC
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let sprints = state
        .sprint_service
        .list_project_sprints(project_id)
        .await
        .map_err(map_sprint_service_error)?;

    let dtos: Vec<SprintDto> = sprints.into_iter().map(SprintDto::from).collect();
    let response = ApiResponse::success(dtos, "Sprints retrieved successfully");

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/sprints",
    tag = "Sprints",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    request_body = CreateSprintRequestDoc,
    responses(
        (status = 201, description = "Sprint created successfully", body = SprintResponse),
        (status = 404, description = "Project not found")
    )
)]
pub async fn create_sprint(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<CreateSprintPayload>,
) -> ApiResult<(StatusCode, Json<ApiResponse<SprintDto>>)> {
    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageSprints,
        &state.db,
    )
    .await?;

    let payload = CreateSprintRequest {
        project_id,
        sequence: payload.sequence,
        name: payload.name,
        description: payload.description,
        goal: payload.goal,
        start_date: payload.start_date,
        end_date: payload.end_date,
    };

    let sprint = state
        .sprint_service
        .create_sprint(payload)
        .await
        .map_err(map_sprint_service_error)?;

    let dto = SprintDto::from(sprint);
    let response = ApiResponse::created(dto, "Sprint created successfully");

    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/sprints/generate",
    tag = "Sprints",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    request_body = GenerateSprintsRequestDoc,
    responses(
        (status = 201, description = "Sprints generated successfully", body = SprintListResponse),
        (status = 404, description = "Project not found")
    )
)]
pub async fn generate_sprints(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<GenerateSprintsPayload>,
) -> ApiResult<(StatusCode, Json<ApiResponse<Vec<SprintDto>>>)> {
    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageSprints,
        &state.db,
    )
    .await?;

    let payload = GenerateSprintsRequest {
        project_id,
        start_date: payload.start_date,
        duration_weeks: payload.duration_weeks,
        count: payload.count,
    };

    let sprints = state
        .sprint_service
        .generate_sprints(payload)
        .await
        .map_err(map_sprint_service_error)?;

    let dtos: Vec<SprintDto> = sprints.into_iter().map(SprintDto::from).collect();
    let response = ApiResponse::created(dtos, "Sprints generated successfully");

    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/sprints/{sprint_id}",
    tag = "Sprints",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("sprint_id" = Uuid, Path, description = "Sprint ID")
    ),
    responses(
        (status = 200, description = "Sprint retrieved", body = SprintResponse),
        (status = 404, description = "Sprint not found")
    )
)]
pub async fn get_sprint(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, sprint_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<SprintDto>>> {
    // Check permission using RBAC
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let sprint = state
        .sprint_service
        .get_sprint(sprint_id)
        .await
        .map_err(map_sprint_service_error)?
        .ok_or_else(|| ApiError::NotFound("Sprint not found".to_string()))?;

    // Verify sprint belongs to project
    if sprint.project_id != project_id {
        return Err(ApiError::NotFound(
            "Sprint not found in this project".to_string(),
        ));
    }

    let dto = SprintDto::from(sprint);
    let response = ApiResponse::success(dto, "Sprint retrieved successfully");

    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/sprints/{sprint_id}",
    tag = "Sprints",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("sprint_id" = Uuid, Path, description = "Sprint ID")
    ),
    request_body = UpdateSprintRequestDoc,
    responses(
        (status = 200, description = "Sprint updated", body = SprintResponse),
        (status = 404, description = "Sprint not found")
    )
)]
pub async fn update_sprint(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, sprint_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateSprintRequest>,
) -> ApiResult<Json<ApiResponse<SprintDto>>> {
    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageSprints,
        &state.db,
    )
    .await?;

    // Verify sprint exists and belongs to project
    let existing = state
        .sprint_service
        .get_sprint(sprint_id)
        .await
        .map_err(map_sprint_service_error)?
        .ok_or_else(|| ApiError::NotFound("Sprint not found".to_string()))?;

    if existing.project_id != project_id {
        return Err(ApiError::NotFound(
            "Sprint not found in this project".to_string(),
        ));
    }

    let sprint = state
        .sprint_service
        .update_sprint(sprint_id, payload)
        .await
        .map_err(map_sprint_service_error)?;

    let dto = SprintDto::from(sprint);
    let response = ApiResponse::success(dto, "Sprint updated successfully");

    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/sprints/{sprint_id}",
    tag = "Sprints",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("sprint_id" = Uuid, Path, description = "Sprint ID")
    ),
    responses(
        (status = 200, description = "Sprint deleted"),
        (status = 404, description = "Sprint not found")
    )
)]
pub async fn delete_sprint(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, sprint_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<()>>> {
    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageSprints,
        &state.db,
    )
    .await?;

    // Verify sprint exists and belongs to project
    let existing = state
        .sprint_service
        .get_sprint(sprint_id)
        .await
        .map_err(map_sprint_service_error)?
        .ok_or_else(|| ApiError::NotFound("Sprint not found".to_string()))?;

    if existing.project_id != project_id {
        return Err(ApiError::NotFound(
            "Sprint not found in this project".to_string(),
        ));
    }

    state
        .sprint_service
        .delete_sprint(sprint_id)
        .await
        .map_err(map_sprint_service_error)?;

    let response = ApiResponse::success((), "Sprint deleted successfully");

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/sprints/active",
    tag = "Sprints",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Active sprint retrieved", body = SprintResponse)
    )
)]
pub async fn get_active_sprint(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Option<SprintDto>>>> {
    // Check permission using RBAC
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let sprint = state
        .sprint_service
        .get_active_sprint(project_id)
        .await
        .map_err(map_sprint_service_error)?;

    let response = match sprint {
        Some(sprint) => ApiResponse::success(
            Some(SprintDto::from(sprint)),
            "Active sprint retrieved successfully",
        ),
        None => ApiResponse::success(None::<SprintDto>, "No active sprint"),
    };

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/sprints/{sprint_id}/activate",
    tag = "Sprints",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("sprint_id" = Uuid, Path, description = "Sprint ID")
    ),
    responses(
        (status = 200, description = "Sprint activated", body = SprintResponse),
        (status = 404, description = "Sprint not found"),
        (status = 400, description = "Invalid sprint state")
    )
)]
pub async fn activate_sprint(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, sprint_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<SprintDto>>> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageSprints,
        &state.db,
    )
    .await?;

    let sprint = state
        .sprint_service
        .activate_sprint(project_id, sprint_id, Some(auth_user.id))
        .await
        .map_err(map_sprint_service_error)?;

    let response = ApiResponse::success(SprintDto::from(sprint), "Sprint activated successfully");
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/sprints/{sprint_id}/close",
    tag = "Sprints",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("sprint_id" = Uuid, Path, description = "Sprint ID")
    ),
    request_body = CloseSprintRequestDoc,
    responses(
        (status = 200, description = "Sprint closed", body = CloseSprintResultResponse),
        (status = 404, description = "Sprint not found"),
        (status = 400, description = "Invalid close request")
    )
)]
pub async fn close_sprint(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, sprint_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<CloseSprintRequest>,
) -> ApiResult<Json<ApiResponse<acpms_db::models::CloseSprintResult>>> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageSprints,
        &state.db,
    )
    .await?;

    let result = state
        .sprint_service
        .close_sprint(project_id, sprint_id, auth_user.id, payload)
        .await
        .map_err(map_sprint_service_error)?;

    let response = ApiResponse::success(result, "Sprint closed successfully");
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/sprints/{sprint_id}/overview",
    tag = "Sprints",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("sprint_id" = Uuid, Path, description = "Sprint ID")
    ),
    responses(
        (status = 200, description = "Sprint overview", body = SprintOverviewResponse),
        (status = 404, description = "Sprint not found")
    )
)]
pub async fn get_sprint_overview(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, sprint_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<SprintOverview>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let overview = state
        .sprint_service
        .get_sprint_overview(project_id, sprint_id)
        .await
        .map_err(map_sprint_service_error)?;

    let response = ApiResponse::success(overview, "Sprint overview retrieved successfully");
    Ok(Json(response))
}
