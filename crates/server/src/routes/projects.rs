use acpms_db::{models::*, PgPool};
use acpms_executors::ExecutorOrchestrator;
use acpms_services::{ProjectService, RepositoryAccessService, SystemSettingsService, TaskService};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::Semaphore;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::api::{ApiResponse, ProjectDto};
use crate::error::{ApiError, ApiResult};
use crate::middleware::{AuthUser, Permission, RbacChecker};
use crate::AppState;

const DEFAULT_PROJECT_PAGE_SIZE: u32 = 9;
const DEFAULT_IMPORT_EXECUTION_CONCURRENCY: usize = 4;

fn import_execution_semaphore() -> &'static Semaphore {
    static IMPORT_EXECUTION_SEMAPHORE: OnceLock<Semaphore> = OnceLock::new();
    IMPORT_EXECUTION_SEMAPHORE.get_or_init(|| {
        let permits = std::env::var("IMPORT_EXECUTION_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(DEFAULT_IMPORT_EXECUTION_CONCURRENCY);
        Semaphore::new(permits)
    })
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ProjectsQuery {
    /// Page size (default: 9, max: 100). When omitted, returns all projects.
    pub limit: Option<u32>,
    /// Page number (1-based). When set, enables offset-based pagination.
    pub page: Option<u32>,
    /// Cursor: fetch projects created before this RFC 3339 timestamp
    pub before: Option<DateTime<Utc>>,
    /// Tie-breaker cursor for projects sharing the same timestamp
    pub before_id: Option<Uuid>,
    /// Filter by project name (case-insensitive substring match)
    pub search: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectMemberDto {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub avatar_url: Option<String>,
    #[schema(value_type = Vec<String>)]
    pub roles: Vec<ProjectRole>,
}

fn project_role_to_str(r: &ProjectRole) -> &'static str {
    match r {
        ProjectRole::Owner => "owner",
        ProjectRole::Admin => "admin",
        ProjectRole::ProductOwner => "product_owner",
        ProjectRole::Developer => "developer",
        ProjectRole::BusinessAnalyst => "business_analyst",
        ProjectRole::QualityAssurance => "quality_assurance",
        ProjectRole::Viewer => "viewer",
    }
}

/// Validate project name for slug/repo compatibility.
fn validate_project_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Project name is required".to_string());
    }
    if trimmed.len() > 64 {
        return Err("Project name must be 64 characters or less".to_string());
    }
    let slug = trimmed
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect::<String>();
    if slug.is_empty() {
        return Err("Project name must contain at least one letter or number".to_string());
    }
    if slug.len() > 64 {
        return Err("Project name produces a slug that is too long".to_string());
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/v1/projects",
    tag = "Projects",
    request_body = CreateProjectRequestDoc,
    responses(
        (status = 201, description = "Project created successfully", body = ProjectResponse),
        (status = 400, description = "Invalid input")
    )
)]
pub async fn create_project(
    State(pool): State<PgPool>,
    State(orchestrator): State<Arc<ExecutorOrchestrator>>,
    auth_user: AuthUser,
    Json(req): Json<CreateProjectRequest>,
) -> ApiResult<(StatusCode, Json<ApiResponse<ProjectDto>>)> {
    validate_project_name(&req.name).map_err(ApiError::BadRequest)?;

    let service = ProjectService::new(pool.clone());
    let project = service
        .create_project(auth_user.id, req.clone())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // If requested, create init task and start execution.
    let should_create_init_task =
        req.create_from_scratch.unwrap_or(false) && req.auto_create_init_task.unwrap_or(true);

    if should_create_init_task {
        let task_service = TaskService::new(pool.clone());
        let visibility = req.visibility.as_deref().unwrap_or("private");

        let init_task = task_service
            .create_from_scratch_task(
                project.id,
                auth_user.id,
                project.project_type,
                &req.name,
                req.description.as_deref().unwrap_or(""),
                req.tech_stack.as_deref(),
                req.stack_selections.as_deref(),
                visibility,
                req.reference_keys.as_deref(),
            )
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Start task execution asynchronously
        let task_id = init_task.id;
        tracing::info!("Spawning init task execution for task_id: {}", task_id);
        tokio::spawn(async move {
            tracing::info!("Starting execution of init task {}", task_id);
            match orchestrator.execute_task(task_id).await {
                Ok(_) => {
                    tracing::info!("Successfully executed init task {}", task_id);
                }
                Err(e) => {
                    tracing::error!("Failed to execute init task {}: {:?}", task_id, e);
                    tracing::error!("Error backtrace: {}", e.backtrace());
                }
            }
        });
    }

    let dto = ProjectDto::from(project);
    let response = ApiResponse::created(dto, "Project created successfully");

    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects",
    tag = "Projects",
    params(ProjectsQuery),
    responses(
        (status = 200, description = "List user projects", body = ProjectListResponse)
    )
)]
pub async fn list_projects(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Query(query): Query<ProjectsQuery>,
) -> ApiResult<Json<ApiResponse<Vec<ProjectDto>>>> {
    let is_admin = RbacChecker::is_system_admin(auth_user.id, &pool).await?;

    if query.before.is_none() && query.before_id.is_some() {
        return Err(ApiError::BadRequest(
            "`before_id` requires `before` timestamp".to_string(),
        ));
    }

    let service = ProjectService::new(pool.clone());
    let is_paginated = query.limit.is_some()
        || query.page.is_some()
        || query.before.is_some()
        || query.search.is_some();

    let page_size = query
        .limit
        .unwrap_or(DEFAULT_PROJECT_PAGE_SIZE)
        .clamp(1, 100);
    let page_num = query.page.unwrap_or(1).max(1);
    let offset = i64::from((page_num - 1) * page_size);
    let limit = i64::from(page_size);
    let search_ref = query.search.as_deref();

    let projects = if is_paginated {
        if query.page.is_some() {
            // Offset-based (page numbers)
            if is_admin {
                service
                    .get_all_projects_page(limit, offset, search_ref)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?
            } else {
                service
                    .get_user_projects_page(auth_user.id, limit, offset, search_ref)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?
            }
        } else {
            // Cursor-based (before/before_id)
            if is_admin {
                service
                    .get_all_projects_paginated(limit, query.before, query.before_id, search_ref)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?
            } else {
                service
                    .get_user_projects_paginated(
                        auth_user.id,
                        limit,
                        query.before,
                        query.before_id,
                        search_ref,
                    )
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?
            }
        }
    } else if is_admin {
        service
            .get_all_projects()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
    } else {
        service
            .get_user_projects(auth_user.id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
    };

    let dtos: Vec<ProjectDto> = projects.into_iter().map(ProjectDto::from).collect();

    let mut response = ApiResponse::success(dtos, "Projects retrieved successfully");

    if is_paginated {
        let result_count = response.data.as_ref().map(|d| d.len()).unwrap_or(0);
        let has_more = result_count >= page_size as usize;

        if query.page.is_some() {
            let total_count = if is_admin {
                service.count_all_projects(search_ref).await.unwrap_or(0)
            } else {
                service
                    .count_user_projects(auth_user.id, search_ref)
                    .await
                    .unwrap_or(0)
            };
            let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i64;

            response.metadata = Some(serde_json::json!({
                "has_more": has_more,
                "total_count": total_count,
                "total_pages": total_pages,
                "page": page_num,
                "page_size": page_size,
            }));
        } else {
            response.metadata = Some(serde_json::json!({
                "has_more": has_more,
            }));
        }
    }

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Get project details", body = ProjectResponse),
        (status = 404, description = "Project not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_project(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<ProjectDto>>> {
    // Check permission (ViewProject = all roles)
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let service = ProjectService::new(state.db.clone());
    let project = service
        .get_project(project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;

    if project_needs_repository_context_backfill(&project) {
        record_repository_backfill_metric(&state.metrics, "project_get", "queued");
        tracing::info!(
            project_id = %project.id,
            repository_url = project.repository_url.as_deref().unwrap_or(""),
            "Queueing legacy repository context backfill"
        );

        let background_state = state.clone();
        tokio::spawn(async move {
            if let Err(error) =
                backfill_project_repository_context(background_state, project_id, "project_get")
                    .await
            {
                tracing::warn!(
                    project_id = %project_id,
                    error = %error,
                    "Legacy repository context backfill failed"
                );
            }
        });
    }

    let dto = ProjectDto::from(project);
    let response = ApiResponse::success(dto, "Project retrieved successfully");

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}/members",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Project members retrieved", body = ApiResponse<Vec<ProjectMemberDto>>),
        (status = 404, description = "Project not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_project_members(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<ProjectMemberDto>>>> {
    type ProjectMemberRow = (Uuid, String, String, Option<String>, Vec<ProjectRole>);

    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &pool).await?;

    let rows: Vec<ProjectMemberRow> = sqlx::query_as(
        r#"
        SELECT u.id, u.name, u.email, u.avatar_url, pm.roles
        FROM project_members pm
        INNER JOIN users u ON u.id = pm.user_id
        WHERE pm.project_id = $1
        ORDER BY u.name ASC
        "#,
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let members = rows
        .into_iter()
        .map(|(id, name, email, avatar_url, roles)| ProjectMemberDto {
            id,
            name,
            email,
            avatar_url,
            roles,
        })
        .collect();

    let response = ApiResponse::success(members, "Project members retrieved successfully");
    Ok(Json(response))
}

/// User option for invite dropdown (id, name, email, avatar_url)
#[derive(Debug, Serialize, ToSchema)]
pub struct InviteableUserDto {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub avatar_url: Option<String>,
}

/// List users that can be invited to the project (not yet members).
/// Requires ManageMembers (Owner). Used for user selector in add-member UI.
#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}/inviteable-users",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Project ID")),
    responses(
        (status = 200, description = "List of users that can be invited"),
        (status = 403, description = "Forbidden - ManageMembers required")
    )
)]
pub async fn list_inviteable_users(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<InviteableUserDto>>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ManageMembers, &pool)
        .await?;

    type UserRow = (Uuid, String, String, Option<String>);
    let rows: Vec<UserRow> = sqlx::query_as(
        r#"
        SELECT u.id, u.name, u.email, u.avatar_url
        FROM users u
        WHERE NOT EXISTS (
            SELECT 1 FROM project_members pm
            WHERE pm.project_id = $1 AND pm.user_id = u.id
        )
        ORDER BY u.name ASC
        "#,
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let users = rows
        .into_iter()
        .map(|(id, name, email, avatar_url)| InviteableUserDto {
            id,
            name,
            email,
            avatar_url,
        })
        .collect();

    Ok(Json(ApiResponse::success(
        users,
        "Inviteable users retrieved successfully",
    )))
}

/// Request to add a member to a project (ManageMembers = Owner only)
#[derive(Debug, Deserialize, ToSchema)]
pub struct AddProjectMemberRequest {
    /// User ID (from system users list)
    pub user_id: Uuid,
    /// Project role to assign
    pub roles: Vec<ProjectRole>,
}

/// Request to update a member's role
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProjectMemberRequest {
    pub roles: Vec<ProjectRole>,
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{id}/members",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Project ID")),
    request_body = AddProjectMemberRequest,
    responses(
        (status = 201, description = "Member added"),
        (status = 400, description = "User not found or already a member"),
        (status = 403, description = "Forbidden - ManageMembers required")
    )
)]
pub async fn add_project_member(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    Path(project_id): Path<Uuid>,
    Json(req): Json<AddProjectMemberRequest>,
) -> ApiResult<(StatusCode, Json<ApiResponse<ProjectMemberDto>>)> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ManageMembers, &pool)
        .await?;

    if req.roles.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one role is required".to_string(),
        ));
    }

    let user_id = req.user_id;

    // Verify user exists
    let user: (String, String, Option<String>) =
        sqlx::query_as("SELECT name, email, avatar_url FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::BadRequest("User not found".to_string()))?;

    let (name, email, avatar_url) = user;

    // Check if already a member
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM project_members WHERE project_id = $1 AND user_id = $2)",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if exists {
        return Err(ApiError::BadRequest(
            "User is already a project member".to_string(),
        ));
    }

    let roles_array: Vec<String> = req
        .roles
        .iter()
        .map(|r| project_role_to_str(r).to_string())
        .collect();

    sqlx::query(
        r#"
        INSERT INTO project_members (project_id, user_id, roles)
        VALUES ($1, $2, $3::project_role[])
        "#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(roles_array)
    .execute(&pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = ProjectMemberDto {
        id: user_id,
        name,
        email,
        avatar_url,
        roles: req.roles,
    };
    let response = ApiResponse::created(dto, "Member added successfully");
    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{id}/members/{user_id}",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID"),
        ("user_id" = Uuid, Path, description = "User ID to update")
    ),
    request_body = UpdateProjectMemberRequest,
    responses(
        (status = 200, description = "Member updated"),
        (status = 400, description = "Cannot change owner or invalid roles"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_project_member(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    Path((project_id, target_user_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateProjectMemberRequest>,
) -> ApiResult<Json<ApiResponse<ProjectMemberDto>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ManageMembers, &pool)
        .await?;

    if req.roles.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one role is required".to_string(),
        ));
    }

    // Cannot remove owner role from the last owner
    let is_owner: bool = sqlx::query_scalar(
        r#"SELECT 'owner' = ANY(roles) FROM project_members WHERE project_id = $1 AND user_id = $2"#,
    )
    .bind(project_id)
    .bind(target_user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .unwrap_or(false);

    let new_has_owner = req.roles.iter().any(|r| matches!(r, ProjectRole::Owner));
    if is_owner && !new_has_owner {
        let owner_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM project_members WHERE project_id = $1 AND 'owner' = ANY(roles)"#,
        )
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
        if owner_count <= 1 {
            return Err(ApiError::BadRequest(
                "Cannot remove the last owner from the project".to_string(),
            ));
        }
    }

    let roles_array: Vec<String> = req
        .roles
        .iter()
        .map(|r| project_role_to_str(r).to_string())
        .collect();

    sqlx::query(
        r#"UPDATE project_members SET roles = $1::project_role[] WHERE project_id = $2 AND user_id = $3"#,
    )
    .bind(roles_array)
    .bind(project_id)
    .bind(target_user_id)
    .execute(&pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let row: (String, String, Option<String>, Vec<ProjectRole>) = sqlx::query_as(
        r#"SELECT u.name, u.email, u.avatar_url, pm.roles FROM project_members pm
           INNER JOIN users u ON u.id = pm.user_id
           WHERE pm.project_id = $1 AND pm.user_id = $2"#,
    )
    .bind(project_id)
    .bind(target_user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = ProjectMemberDto {
        id: target_user_id,
        name: row.0,
        email: row.1,
        avatar_url: row.2,
        roles: row.3,
    };
    Ok(Json(ApiResponse::success(
        dto,
        "Member updated successfully",
    )))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{id}/members/{user_id}",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID"),
        ("user_id" = Uuid, Path, description = "User ID to remove")
    ),
    responses(
        (status = 200, description = "Member removed"),
        (status = 400, description = "Cannot remove last owner"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn remove_project_member(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    Path((project_id, target_user_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<()>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ManageMembers, &pool)
        .await?;

    // Cannot remove the last owner
    let is_owner: bool = sqlx::query_scalar(
        r#"SELECT 'owner' = ANY(roles) FROM project_members WHERE project_id = $1 AND user_id = $2"#,
    )
    .bind(project_id)
    .bind(target_user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .unwrap_or(false);

    if is_owner {
        let owner_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM project_members WHERE project_id = $1 AND 'owner' = ANY(roles)"#,
        )
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
        if owner_count <= 1 {
            return Err(ApiError::BadRequest(
                "Cannot remove the last owner from the project".to_string(),
            ));
        }
    }

    sqlx::query("DELETE FROM project_members WHERE project_id = $1 AND user_id = $2")
        .bind(project_id)
        .bind(target_user_id)
        .execute(&pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(ApiResponse::success(
        (),
        "Member removed successfully",
    )))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{id}",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    request_body = UpdateProjectRequestDoc,
    responses(
        (status = 200, description = "Update project", body = ProjectResponse),
        (status = 404, description = "Project not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_project(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    Path(project_id): Path<Uuid>,
    Json(req): Json<UpdateProjectRequest>,
) -> ApiResult<Json<ApiResponse<ProjectDto>>> {
    // Check permission (ManageProject = Owner, Admin)
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ManageProject, &pool)
        .await?;

    let service = ProjectService::new(pool.clone());

    let project = service
        .update_project(project_id, req)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dto = ProjectDto::from(project);
    let response = ApiResponse::success(dto, "Project updated successfully");

    Ok(Json(response))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RecheckRepositoryAccessResponse {
    pub project: ProjectDto,
    pub recommended_action: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LinkExistingForkRequest {
    pub repository_url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LinkExistingForkResponse {
    pub project: ProjectDto,
    pub recommended_action: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateForkResponse {
    pub project: ProjectDto,
    pub created_repository_url: String,
    pub recommended_action: Option<String>,
    pub warnings: Vec<String>,
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{id}/repository-context/recheck",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Repository access re-checked successfully", body = RecheckRepositoryAccessResponse),
        (status = 400, description = "Project has no repository URL or URL is invalid"),
        (status = 404, description = "Project not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn recheck_project_repository_access(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<RecheckRepositoryAccessResponse>>> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    let service = ProjectService::new(state.db.clone());
    let project = service
        .get_project(project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;

    let repository_url = project.repository_url.clone().ok_or_else(|| {
        ApiError::BadRequest("Project has no repository URL to re-check".to_string())
    })?;

    validate_repo_url(&state.db, &repository_url).await?;

    let clone_error = check_repository_cloneable(&state.settings_service, &repository_url)
        .await
        .err()
        .map(|error| error.to_string());
    let can_clone = clone_error.is_none();

    let repository_access_service = RepositoryAccessService::new((*state.settings_service).clone());
    let repo_context = repository_context_with_clone_result(
        repository_access_service
            .preflight(&repository_url, can_clone)
            .await,
        clone_error,
    );
    let repository_context = if project.repository_context.access_mode
        == RepositoryAccessMode::ForkGitops
        || project
            .repository_context
            .upstream_repository_url
            .as_deref()
            .filter(|upstream| {
                normalize_repo_url_for_comparison(upstream)
                    != normalize_repo_url_for_comparison(&repository_url)
            })
            .is_some()
    {
        let upstream_repository_url = project
            .repository_context
            .upstream_repository_url
            .clone()
            .unwrap_or_else(|| repository_url.clone());
        let upstream_context = repository_access_service
            .preflight(&upstream_repository_url, true)
            .await;
        build_linked_fork_repository_context(
            &project.repository_context,
            &upstream_repository_url,
            &repository_url,
            upstream_context,
            repo_context,
        )
        .unwrap_or_else(|_| project.repository_context.clone())
    } else {
        repo_context
    };
    record_repository_context_evaluation_metric(
        &state.metrics,
        "project_recheck",
        &repository_context,
    );

    let updated_project = service
        .update_project(
            project_id,
            UpdateProjectRequest {
                name: None,
                description: None,
                repository_url: None,
                repository_context: Some(repository_context.clone()),
                metadata: None,
                require_review: None,
            },
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = RecheckRepositoryAccessResponse {
        project: ProjectDto::from(updated_project),
        recommended_action: recommended_action_for_repository_context(&repository_context),
        warnings: warnings_for_repository_context(&repository_context),
    };

    Ok(Json(ApiResponse::success(
        response,
        "Repository access re-checked successfully",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{id}/repository-context/link-fork",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    request_body = LinkExistingForkRequest,
    responses(
        (status = 200, description = "Existing writable fork linked successfully", body = LinkExistingForkResponse),
        (status = 400, description = "Invalid fork URL or fork is not writable"),
        (status = 404, description = "Project not found"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Fork repository already linked to another project")
    )
)]
pub async fn link_existing_fork(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(req): Json<LinkExistingForkRequest>,
) -> ApiResult<Json<ApiResponse<LinkExistingForkResponse>>> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    let service = ProjectService::new(state.db.clone());
    let project = service
        .get_project(project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;

    let current_repository_url = project.repository_url.clone().ok_or_else(|| {
        ApiError::BadRequest("Project has no repository URL to upgrade".to_string())
    })?;
    let upstream_repository_url = project
        .repository_context
        .upstream_repository_url
        .clone()
        .unwrap_or_else(|| current_repository_url.clone());

    validate_repo_url(&state.db, &req.repository_url).await?;
    check_repository_not_duplicate_except(&state.db, &req.repository_url, project_id).await?;

    if normalize_repo_url_for_comparison(&req.repository_url)
        == normalize_repo_url_for_comparison(&upstream_repository_url)
    {
        return Err(ApiError::BadRequest(
            "Fork URL must be different from the upstream repository URL.".to_string(),
        ));
    }

    let clone_error = check_repository_cloneable(&state.settings_service, &req.repository_url)
        .await
        .err()
        .map(|error| error.to_string());
    let can_clone = clone_error.is_none();

    let repository_access_service = RepositoryAccessService::new((*state.settings_service).clone());
    let upstream_context = repository_access_service
        .preflight(&upstream_repository_url, true)
        .await;
    let fork_context = repository_context_with_clone_result(
        repository_access_service
            .preflight(&req.repository_url, can_clone)
            .await,
        clone_error,
    );

    if !fork_context.can_push {
        record_repository_fork_operation_metric(
            &state.metrics,
            "project_link_fork",
            fork_context.provider,
            "rejected_not_pushable",
        );
        return Err(ApiError::BadRequest(
            "Linked fork must be writable. Current credentials cannot push to this repository."
                .to_string(),
        ));
    }

    let current_provider = project.repository_context.provider;
    if current_provider != RepositoryProvider::Unknown
        && fork_context.provider != RepositoryProvider::Unknown
        && current_provider != fork_context.provider
    {
        return Err(ApiError::BadRequest(
            "Fork provider must match the provider of the imported upstream repository."
                .to_string(),
        ));
    }

    let linked_context = build_linked_fork_repository_context(
        &project.repository_context,
        &upstream_repository_url,
        &req.repository_url,
        upstream_context,
        fork_context,
    )?;
    record_repository_context_evaluation_metric(
        &state.metrics,
        "project_link_fork",
        &linked_context,
    );
    record_repository_fork_operation_metric(
        &state.metrics,
        "project_link_fork",
        linked_context.provider,
        "success",
    );

    let updated_project = service
        .update_project(
            project_id,
            UpdateProjectRequest {
                name: None,
                description: None,
                repository_url: Some(req.repository_url.clone()),
                repository_context: Some(linked_context.clone()),
                metadata: None,
                require_review: None,
            },
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = LinkExistingForkResponse {
        project: ProjectDto::from(updated_project),
        recommended_action: recommended_action_for_repository_context(&linked_context),
        warnings: warnings_for_repository_context(&linked_context),
    };

    Ok(Json(ApiResponse::success(
        response,
        "Existing writable fork linked successfully",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{id}/repository-context/create-fork",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Writable fork created and linked successfully", body = CreateForkResponse),
        (status = 400, description = "Project cannot be upgraded with an automatic fork"),
        (status = 404, description = "Project not found"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Generated fork repository is already linked to another project")
    )
)]
pub async fn create_project_fork(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<CreateForkResponse>>> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    let service = ProjectService::new(state.db.clone());
    let project = service
        .get_project(project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;

    let current_repository_url = project.repository_url.clone().ok_or_else(|| {
        ApiError::BadRequest("Project has no repository URL to upgrade".to_string())
    })?;
    let upstream_repository_url = project
        .repository_context
        .upstream_repository_url
        .clone()
        .unwrap_or_else(|| current_repository_url.clone());

    if project.repository_context.access_mode == RepositoryAccessMode::ForkGitops {
        record_repository_fork_operation_metric(
            &state.metrics,
            "project_create_fork",
            project.repository_context.provider,
            "skipped_already_fork_gitops",
        );
        return Err(ApiError::BadRequest(
            "Project is already linked to a writable fork.".to_string(),
        ));
    }

    if project.repository_context.supports_gitops() {
        record_repository_fork_operation_metric(
            &state.metrics,
            "project_create_fork",
            project.repository_context.provider,
            "skipped_already_gitops",
        );
        return Err(ApiError::BadRequest(
            "Project already supports full GitOps. Automatic fork creation is not required."
                .to_string(),
        ));
    }

    if !project.repository_context.can_fork {
        record_repository_fork_operation_metric(
            &state.metrics,
            "project_create_fork",
            project.repository_context.provider,
            "rejected_cannot_fork",
        );
        return Err(ApiError::BadRequest(
            "Current credentials cannot create a fork for this repository.".to_string(),
        ));
    }

    let repository_access_service = RepositoryAccessService::new((*state.settings_service).clone());
    let created_repository_url = repository_access_service
        .create_fork_repository(&upstream_repository_url)
        .await
        .map_err(|e| {
            record_repository_fork_operation_metric(
                &state.metrics,
                "project_create_fork",
                project.repository_context.provider,
                "failure_create_fork",
            );
            ApiError::BadRequest(format!("Failed to create fork: {}", e))
        })?;

    validate_repo_url(&state.db, &created_repository_url).await?;
    check_repository_not_duplicate_except(&state.db, &created_repository_url, project_id).await?;

    if normalize_repo_url_for_comparison(&created_repository_url)
        == normalize_repo_url_for_comparison(&upstream_repository_url)
    {
        return Err(ApiError::BadRequest(
            "Automatic fork creation returned the upstream repository URL instead of a writable fork."
                .to_string(),
        ));
    }

    let clone_error = check_repository_cloneable_with_retry(
        &state.settings_service,
        &created_repository_url,
        6,
        Duration::from_secs(2),
    )
    .await;
    let can_clone = clone_error.is_none();

    let upstream_context = repository_access_service
        .preflight(&upstream_repository_url, true)
        .await;
    let fork_context = repository_context_with_clone_result(
        repository_access_service
            .preflight(&created_repository_url, can_clone)
            .await,
        clone_error,
    );

    if !fork_context.can_push {
        record_repository_fork_operation_metric(
            &state.metrics,
            "project_create_fork",
            fork_context.provider,
            "failure_not_pushable",
        );
        return Err(ApiError::BadRequest(
            "Fork was created but current credentials still cannot push to it.".to_string(),
        ));
    }

    let linked_context = build_linked_fork_repository_context(
        &project.repository_context,
        &upstream_repository_url,
        &created_repository_url,
        upstream_context,
        fork_context,
    )?;
    record_repository_context_evaluation_metric(
        &state.metrics,
        "project_create_fork",
        &linked_context,
    );
    record_repository_fork_operation_metric(
        &state.metrics,
        "project_create_fork",
        linked_context.provider,
        "success",
    );

    let updated_project = service
        .update_project(
            project_id,
            UpdateProjectRequest {
                name: None,
                description: None,
                repository_url: Some(created_repository_url.clone()),
                repository_context: Some(linked_context.clone()),
                metadata: None,
                require_review: None,
            },
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = CreateForkResponse {
        project: ProjectDto::from(updated_project),
        created_repository_url,
        recommended_action: recommended_action_for_repository_context(&linked_context),
        warnings: warnings_for_repository_context(&linked_context),
    };

    Ok(Json(ApiResponse::success(
        response,
        "Writable fork created and linked successfully",
    )))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{id}",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Delete project", body = EmptyResponse),
        (status = 404, description = "Project not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_project(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<()>>> {
    // Check permission (ManageProject = Owner, Admin)
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ManageProject, &pool)
        .await?;

    let service = ProjectService::new(pool.clone());
    service
        .delete_project(project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = ApiResponse::success((), "Project deleted successfully");

    Ok(Json(response))
}

/// Response for sync repository
#[derive(Debug, Serialize, ToSchema)]
pub struct SyncRepositoryResponse {
    pub last_sync_at: DateTime<Utc>,
    pub branches_synced: u32,
    pub merge_requests_synced: u32,
    pub pipelines_synced: u32,
}

/// Sync project with GitLab (branches, MRs, pipelines).
/// Requires project to be linked to GitLab.
#[utoipa::path(
    post,
    path = "/api/v1/projects/{id}/sync",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Project ID")),
    responses(
        (status = 200, description = "Sync completed", body = ApiResponse<SyncRepositoryResponse>),
        (status = 400, description = "Project not linked to GitLab"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn sync_project_repository(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<SyncRepositoryResponse>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let result = state
        .gitlab_sync_service
        .sync_project(project_id)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let last_sync: DateTime<Utc> =
        sqlx::query_scalar("SELECT last_sync_at FROM gitlab_sync_metadata WHERE project_id = $1")
            .bind(project_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .unwrap_or_else(Utc::now);

    let response = SyncRepositoryResponse {
        last_sync_at: last_sync,
        branches_synced: result.branches_synced,
        merge_requests_synced: result.merge_requests_synced,
        pipelines_synced: result.pipelines_synced,
    };
    Ok(Json(ApiResponse::success(
        response,
        "Repository synced successfully",
    )))
}

/// Request for project init reference file upload URL
#[derive(Debug, Deserialize, ToSchema)]
pub struct InitRefUploadUrlRequest {
    pub filename: String,
    pub content_type: String,
}

/// Response for init ref upload URL
#[derive(Debug, Serialize, ToSchema)]
pub struct InitRefUploadUrlResponse {
    pub upload_url: String,
    pub key: String,
}

fn sanitize_init_ref_filename(filename: &str) -> String {
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
        "ref.bin".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Get presigned upload URL for project init reference files (before project exists).
#[utoipa::path(
    post,
    path = "/api/v1/projects/init-refs/upload-url",
    tag = "Projects",
    request_body = InitRefUploadUrlRequest,
    responses(
        (status = 200, description = "Upload URL generated", body = ApiResponse<InitRefUploadUrlResponse>),
        (status = 400, description = "Invalid input")
    )
)]
pub async fn get_init_ref_upload_url(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<InitRefUploadUrlRequest>,
) -> ApiResult<Json<ApiResponse<InitRefUploadUrlResponse>>> {
    let safe_name = sanitize_init_ref_filename(&req.filename);
    let key = format!(
        "project-init-refs/{}/{}-{}",
        auth_user.id,
        Uuid::new_v4(),
        safe_name
    );

    let upload_url = state
        .storage_service
        .get_presigned_upload_url(&key, &req.content_type, Duration::from_secs(3600))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = InitRefUploadUrlResponse { upload_url, key };
    Ok(Json(ApiResponse::success(
        response,
        "Upload URL generated successfully",
    )))
}

/// Request body for repository import preflight.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportProjectPreflightRequest {
    pub repository_url: String,
    pub upstream_repository_url: Option<String>,
}

/// Response for repository import preflight.
#[derive(Debug, Serialize, ToSchema)]
pub struct ImportProjectPreflightResponse {
    pub repository_context: RepositoryContext,
    #[schema(nullable = true)]
    pub recommended_action: Option<String>,
    #[schema(value_type = Vec<String>)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportProjectCreateForkRequest {
    pub repository_url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportProjectCreateForkResponse {
    pub upstream_repository_url: String,
    pub fork_repository_url: String,
    pub repository_context: RepositoryContext,
    #[schema(nullable = true)]
    pub recommended_action: Option<String>,
    #[schema(value_type = Vec<String>)]
    pub warnings: Vec<String>,
}

/// Request body for importing Git repository project
#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportProjectRequest {
    pub name: String,
    pub repository_url: String,
    pub upstream_repository_url: Option<String>,
    pub description: Option<String>,
    /// If true, agent changes require human review before commit/push.
    pub require_review: Option<bool>,
    /// Project type classification (web, mobile, desktop, extension, api, microservice).
    #[schema(value_type = String)]
    pub project_type: Option<ProjectType>,
    /// If true (default), create and run the init task automatically.
    pub auto_create_init_task: Option<bool>,
    /// If true, enable preview deployments for this project.
    pub preview_enabled: Option<bool>,
}

/// Response for import project
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct ImportProjectResponse {
    pub project: ProjectDto,
    #[schema(nullable = true)]
    pub init_task_id: Option<Uuid>,
}

/// Preflight repository import to classify access mode and capabilities.
#[utoipa::path(
    post,
    path = "/api/v1/projects/import/preflight",
    tag = "Projects",
    request_body = ImportProjectPreflightRequest,
    responses(
        (status = 200, description = "Repository preflight completed", body = ImportProjectPreflightResponse),
        (status = 400, description = "Invalid repository URL or repository is not cloneable")
    )
)]
pub async fn import_project_preflight(
    State(state): State<AppState>,
    Json(req): Json<ImportProjectPreflightRequest>,
) -> ApiResult<Json<ApiResponse<ImportProjectPreflightResponse>>> {
    validate_repo_url(&state.db, &req.repository_url).await?;

    let normalized_upstream_repository_url = req
        .upstream_repository_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    if let Some(upstream_repository_url) = normalized_upstream_repository_url.as_deref() {
        validate_repo_url(&state.db, upstream_repository_url).await?;
    }

    check_repository_cloneable(&state.settings_service, &req.repository_url)
        .await
        .map_err(|e| {
            ApiError::BadRequest(format!(
                "Cannot clone repository: {}. Ensure the URL is correct, the repo exists, and for private repos configure PAT in Settings.",
                e
            ))
        })?;

    let repository_access_service = RepositoryAccessService::new((*state.settings_service).clone());
    let fork_or_primary_context = repository_access_service
        .preflight(&req.repository_url, true)
        .await;
    let repository_context = if let Some(upstream_repository_url) =
        normalized_upstream_repository_url
            .as_deref()
            .filter(|upstream| {
                normalize_repo_url_for_comparison(upstream)
                    != normalize_repo_url_for_comparison(&req.repository_url)
            }) {
        check_repository_cloneable(&state.settings_service, upstream_repository_url)
            .await
            .map_err(|e| {
                ApiError::BadRequest(format!(
                    "Cannot clone upstream repository: {}. Ensure the URL is correct, the repo exists, and for private repos configure PAT in Settings.",
                    e
                ))
            })?;

        let upstream_context = repository_access_service
            .preflight(upstream_repository_url, true)
            .await;
        build_linked_fork_repository_context(
            &RepositoryContext::default(),
            upstream_repository_url,
            &req.repository_url,
            upstream_context,
            fork_or_primary_context,
        )?
    } else {
        fork_or_primary_context
    };
    record_repository_context_evaluation_metric(
        &state.metrics,
        "import_preflight",
        &repository_context,
    );
    let response = ImportProjectPreflightResponse {
        recommended_action: recommended_action_for_repository_context(&repository_context),
        warnings: warnings_for_repository_context(&repository_context),
        repository_context,
    };

    Ok(Json(ApiResponse::success(
        response,
        "Repository preflight completed successfully",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/import/create-fork",
    tag = "Projects",
    request_body = ImportProjectCreateForkRequest,
    responses(
        (status = 200, description = "Writable fork created for import", body = ImportProjectCreateForkResponse),
        (status = 400, description = "Repository cannot be forked or fork creation failed"),
        (status = 409, description = "Fork repository already imported")
    )
)]
pub async fn import_project_create_fork(
    State(state): State<AppState>,
    _auth_user: AuthUser,
    Json(req): Json<ImportProjectCreateForkRequest>,
) -> ApiResult<Json<ApiResponse<ImportProjectCreateForkResponse>>> {
    validate_repo_url(&state.db, &req.repository_url).await?;

    check_repository_cloneable(&state.settings_service, &req.repository_url)
        .await
        .map_err(|e| {
            ApiError::BadRequest(format!(
                "Cannot clone repository: {}. Ensure the URL is correct, the repo exists, and for private repos configure PAT in Settings.",
                e
            ))
        })?;

    let repository_access_service = RepositoryAccessService::new((*state.settings_service).clone());
    let upstream_context = repository_access_service
        .preflight(&req.repository_url, true)
        .await;

    if !upstream_context.can_fork {
        record_repository_fork_operation_metric(
            &state.metrics,
            "import_create_fork",
            upstream_context.provider,
            "rejected_cannot_fork",
        );
        return Err(ApiError::BadRequest(
            "Current credentials cannot create a fork for this repository.".to_string(),
        ));
    }

    let fork_repository_url = repository_access_service
        .create_fork_repository(&req.repository_url)
        .await
        .map_err(|e| {
            record_repository_fork_operation_metric(
                &state.metrics,
                "import_create_fork",
                upstream_context.provider,
                "failure_create_fork",
            );
            ApiError::BadRequest(format!("Failed to create fork: {}", e))
        })?;

    validate_repo_url(&state.db, &fork_repository_url).await?;
    check_repository_not_duplicate(&state.db, &fork_repository_url).await?;

    if normalize_repo_url_for_comparison(&fork_repository_url)
        == normalize_repo_url_for_comparison(&req.repository_url)
    {
        return Err(ApiError::BadRequest(
            "Automatic fork creation returned the upstream repository URL instead of a writable fork."
                .to_string(),
        ));
    }

    let clone_error = check_repository_cloneable_with_retry(
        &state.settings_service,
        &fork_repository_url,
        6,
        Duration::from_secs(2),
    )
    .await;
    let can_clone = clone_error.is_none();
    let fork_context = repository_context_with_clone_result(
        repository_access_service
            .preflight(&fork_repository_url, can_clone)
            .await,
        clone_error,
    );

    let linked_context = build_linked_fork_repository_context(
        &RepositoryContext::default(),
        &req.repository_url,
        &fork_repository_url,
        upstream_context,
        fork_context,
    )?;
    record_repository_context_evaluation_metric(
        &state.metrics,
        "import_create_fork",
        &linked_context,
    );
    record_repository_fork_operation_metric(
        &state.metrics,
        "import_create_fork",
        linked_context.provider,
        "success",
    );

    let response = ImportProjectCreateForkResponse {
        upstream_repository_url: req.repository_url,
        fork_repository_url,
        recommended_action: recommended_action_for_repository_context(&linked_context),
        warnings: warnings_for_repository_context(&linked_context),
        repository_context: linked_context,
    };

    Ok(Json(ApiResponse::success(
        response,
        "Writable fork created for import",
    )))
}

/// Import existing GitLab project
#[utoipa::path(
    post,
    path = "/api/v1/projects/import",
    tag = "Projects",
    request_body = ImportProjectRequest,
    responses(
        (status = 201, description = "Project imported successfully", body = ImportProjectResponse),
        (status = 400, description = "Invalid input - bad repository URL or validation error"),
        (status = 409, description = "Repository already imported")
    )
)]
pub async fn import_project(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(req): Json<ImportProjectRequest>,
) -> ApiResult<(StatusCode, Json<ApiResponse<ImportProjectResponse>>)> {
    let pool = &state.db;

    validate_project_name(&req.name).map_err(ApiError::BadRequest)?;

    // Validate repository URL (GitLab, GitHub, or configured self-hosted).
    validate_repo_url(pool, &req.repository_url).await?;

    let normalized_upstream_repository_url = req
        .upstream_repository_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    if let Some(upstream_repository_url) = normalized_upstream_repository_url.as_deref() {
        validate_repo_url(pool, upstream_repository_url).await?;
    }

    // Check if repository is already imported (duplicate in database)
    check_repository_not_duplicate(pool, &req.repository_url).await?;

    // Pre-check: verify repository is cloneable before creating project
    check_repository_cloneable(&state.settings_service, &req.repository_url).await
        .map_err(|e| ApiError::BadRequest(format!(
            "Cannot clone repository: {}. Ensure the URL is correct, the repo exists, and for private repos configure PAT in Settings.",
            e
        )))?;

    let repository_access_service = RepositoryAccessService::new((*state.settings_service).clone());
    let fork_or_primary_context = repository_access_service
        .preflight(&req.repository_url, true)
        .await;
    let repository_context = if let Some(upstream_repository_url) =
        normalized_upstream_repository_url
            .as_deref()
            .filter(|upstream| {
                normalize_repo_url_for_comparison(upstream)
                    != normalize_repo_url_for_comparison(&req.repository_url)
            }) {
        check_repository_cloneable(&state.settings_service, upstream_repository_url)
            .await
            .map_err(|e| ApiError::BadRequest(format!(
                "Cannot clone upstream repository: {}. Ensure the URL is correct, the repo exists, and for private repos configure PAT in Settings.",
                e
            )))?;

        let upstream_context = repository_access_service
            .preflight(upstream_repository_url, true)
            .await;
        build_linked_fork_repository_context(
            &RepositoryContext::default(),
            upstream_repository_url,
            &req.repository_url,
            upstream_context,
            fork_or_primary_context,
        )?
    } else {
        fork_or_primary_context
    };
    record_repository_context_evaluation_metric(
        &state.metrics,
        "import_create",
        &repository_context,
    );

    let mut tx =
        state.db.begin().await.map_err(|e| {
            ApiError::Internal(format!("Failed to begin import transaction: {}", e))
        })?;

    // Create project and init-task atomically to avoid partial import state on failure.
    let create_req = CreateProjectRequest {
        name: req.name.clone(),
        description: req.description.clone(),
        repository_url: Some(req.repository_url.clone()),
        repository_context: Some(repository_context),
        metadata: None,
        create_from_scratch: None,
        visibility: None,
        tech_stack: None,
        stack_selections: None,
        auto_create_init_task: req.auto_create_init_task,
        require_review: req.require_review,
        project_type: req.project_type,
        template_id: None,
        preview_enabled: req.preview_enabled,
        reference_keys: None,
    };

    let project = ProjectService::create_project_in_tx(&mut tx, auth_user.id, create_req)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut init_task_id = None;
    if req.auto_create_init_task.unwrap_or(true) {
        let init_task = TaskService::create_gitlab_import_task_in_tx(
            &mut tx,
            project.id,
            auth_user.id,
            &req.repository_url,
            req.project_type,
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
        init_task_id = Some(init_task.id);
    }

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to commit import transaction: {}", e)))?;

    if let Some(task_id) = init_task_id {
        tracing::info!(
            "Spawning GitLab import task execution for task_id: {}",
            task_id
        );
        let orch = state.orchestrator.clone();
        tokio::spawn(async move {
            let permit = match import_execution_semaphore().acquire().await {
                Ok(permit) => permit,
                Err(e) => {
                    tracing::error!(
                        "Import execution semaphore closed before task {}: {:?}",
                        task_id,
                        e
                    );
                    return;
                }
            };
            let _permit = permit;

            tracing::info!("Starting execution of GitLab import task {}", task_id);
            if let Err(e) = orch.execute_task(task_id).await {
                tracing::error!("Failed to execute GitLab import task {}: {:?}", task_id, e);
                tracing::error!("Error backtrace: {}", e.backtrace());
            } else {
                tracing::info!("Successfully executed GitLab import task {}", task_id);
            }
        });
    }

    let has_init_task = init_task_id.is_some();
    let dto = ProjectDto::from(project);
    let response_data = ImportProjectResponse {
        project: dto,
        init_task_id,
    };

    let response_message = if has_init_task {
        "Project import started successfully"
    } else {
        "Project imported successfully"
    };
    let response = ApiResponse::created(response_data, response_message);

    Ok((StatusCode::CREATED, Json(response)))
}

// ===== Architecture Config Endpoints =====

/// Request body for updating architecture config
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateArchitectureRequest {
    #[schema(value_type = Object)]
    pub config: serde_json::Value,
}

/// Response for architecture config
#[allow(dead_code)]
pub type ArchitectureResponse = ApiResponse<serde_json::Value>;

fn validate_architecture_config(config: &serde_json::Value) -> Result<(), ApiError> {
    let object = config
        .as_object()
        .ok_or_else(|| ApiError::BadRequest("Architecture config must be an object".to_string()))?;

    let nodes = object
        .get("nodes")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            ApiError::BadRequest("Architecture config.nodes must be an array".to_string())
        })?;

    let edges = object
        .get("edges")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            ApiError::BadRequest("Architecture config.edges must be an array".to_string())
        })?;

    let mut node_ids = HashSet::<String>::new();
    for (index, node) in nodes.iter().enumerate() {
        let node_obj = node.as_object().ok_or_else(|| {
            ApiError::BadRequest(format!(
                "Architecture node at index {} must be an object",
                index
            ))
        })?;

        let id = node_obj
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ApiError::BadRequest(format!(
                    "Architecture node at index {} is missing non-empty 'id'",
                    index
                ))
            })?;

        if !node_ids.insert(id.to_string()) {
            return Err(ApiError::BadRequest(format!(
                "Duplicate architecture node id '{}'",
                id
            )));
        }

        let _label = node_obj
            .get("label")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ApiError::BadRequest(format!(
                    "Architecture node '{}' is missing non-empty 'label'",
                    id
                ))
            })?;

        let _node_type = node_obj
            .get("type")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ApiError::BadRequest(format!(
                    "Architecture node '{}' is missing non-empty 'type'",
                    id
                ))
            })?;

        if let Some(status) = node_obj.get("status").and_then(|value| value.as_str()) {
            if !matches!(status, "healthy" | "warning" | "error") {
                return Err(ApiError::BadRequest(format!(
                    "Architecture node '{}' has invalid status '{}'",
                    id, status
                )));
            }
        }
    }

    for (index, edge) in edges.iter().enumerate() {
        let edge_obj = edge.as_object().ok_or_else(|| {
            ApiError::BadRequest(format!(
                "Architecture edge at index {} must be an object",
                index
            ))
        })?;

        let source = edge_obj
            .get("source")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ApiError::BadRequest(format!(
                    "Architecture edge at index {} is missing non-empty 'source'",
                    index
                ))
            })?;

        let target = edge_obj
            .get("target")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ApiError::BadRequest(format!(
                    "Architecture edge at index {} is missing non-empty 'target'",
                    index
                ))
            })?;

        if !node_ids.contains(source) {
            return Err(ApiError::BadRequest(format!(
                "Architecture edge source '{}' does not exist in nodes",
                source
            )));
        }

        if !node_ids.contains(target) {
            return Err(ApiError::BadRequest(format!(
                "Architecture edge target '{}' does not exist in nodes",
                target
            )));
        }

        if let Some(label) = edge_obj.get("label") {
            if !label.is_string() {
                return Err(ApiError::BadRequest(format!(
                    "Architecture edge {} has non-string label",
                    index
                )));
            }
        }
    }

    Ok(())
}

/// Get project architecture configuration
#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}/architecture",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Architecture config retrieved", body = ArchitectureResponse),
        (status = 404, description = "Project not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_architecture(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    State(orchestrator): State<Arc<ExecutorOrchestrator>>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<serde_json::Value>>> {
    // Check permission (ViewProject = all roles)
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &pool).await?;

    let config: serde_json::Value =
        sqlx::query_scalar(r#"SELECT architecture_config FROM projects WHERE id = $1"#)
            .bind(project_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;

    let is_empty = config
        .get("nodes")
        .and_then(|nodes| nodes.as_array())
        .map(|nodes| nodes.is_empty())
        .unwrap_or(true);
    let is_legacy_frontend_only = {
        let nodes = config
            .get("nodes")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        if nodes.len() != 2 {
            false
        } else {
            let has_browser = nodes
                .iter()
                .any(|node| node.get("id").and_then(|value| value.as_str()) == Some("browser"));
            let has_frontend = nodes
                .iter()
                .any(|node| node.get("id").and_then(|value| value.as_str()) == Some("frontend"));
            let edges = config
                .get("edges")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            let edge_matches = if edges.is_empty() {
                true
            } else {
                edges.len() == 1
                    && edges[0].get("source").and_then(|value| value.as_str()) == Some("browser")
                    && edges[0].get("target").and_then(|value| value.as_str()) == Some("frontend")
            };
            has_browser && has_frontend && edge_matches
        }
    };

    if is_empty || is_legacy_frontend_only {
        // Spawn seeding in background so GET stays fast and predictable.
        // User can refresh to see seeded data once bootstrap completes.
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

    let response = ApiResponse::success(config, "Architecture config retrieved");
    Ok(Json(response))
}

/// Update project architecture configuration
#[utoipa::path(
    put,
    path = "/api/v1/projects/{id}/architecture",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    request_body = UpdateArchitectureRequest,
    responses(
        (status = 200, description = "Architecture config updated", body = ArchitectureResponse),
        (status = 404, description = "Project not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_architecture(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    Path(project_id): Path<Uuid>,
    Json(req): Json<UpdateArchitectureRequest>,
) -> ApiResult<Json<ApiResponse<serde_json::Value>>> {
    // Check permission (ManageProject = Owner, Admin)
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ManageProject, &pool)
        .await?;

    validate_architecture_config(&req.config)?;

    let config: serde_json::Value = sqlx::query_scalar(
        r#"
        UPDATE projects
        SET architecture_config = $2, updated_at = NOW()
        WHERE id = $1
        RETURNING architecture_config
        "#,
    )
    .bind(project_id)
    .bind(&req.config)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;

    let response = ApiResponse::success(config, "Architecture config updated");
    Ok(Json(response))
}

#[cfg(test)]
mod architecture_validation_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_valid_architecture_config() {
        let config = json!({
            "nodes": [
                { "id": "frontend", "label": "Frontend", "type": "frontend", "status": "healthy" },
                { "id": "api", "label": "API", "type": "api" }
            ],
            "edges": [
                { "source": "frontend", "target": "api", "label": "REST" }
            ]
        });

        assert!(validate_architecture_config(&config).is_ok());
    }

    #[test]
    fn rejects_duplicate_node_ids() {
        let config = json!({
            "nodes": [
                { "id": "api", "label": "API 1", "type": "api" },
                { "id": "api", "label": "API 2", "type": "api" }
            ],
            "edges": []
        });

        assert!(validate_architecture_config(&config).is_err());
    }

    #[test]
    fn rejects_unknown_edge_target() {
        let config = json!({
            "nodes": [
                { "id": "frontend", "label": "Frontend", "type": "frontend" }
            ],
            "edges": [
                { "source": "frontend", "target": "api" }
            ]
        });

        assert!(validate_architecture_config(&config).is_err());
    }

    #[test]
    fn rejects_invalid_status_value() {
        let config = json!({
            "nodes": [
                { "id": "api", "label": "API", "type": "api", "status": "unknown" }
            ],
            "edges": []
        });

        assert!(validate_architecture_config(&config).is_err());
    }
}

/// Normalize repository URL for duplicate comparison (strip .git, trailing slash, lowercase).
fn normalize_repo_url_for_comparison(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let lower = trimmed.to_lowercase();
    // Remove credentials from HTTPS URLs for comparison
    if let Some(rest) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
    {
        let without_auth = rest.rsplit('@').next().unwrap_or(rest);
        return without_auth.to_string();
    }
    // SSH: git@host:path -> host/path
    if let Some((left, path)) = lower.split_once(':') {
        if let Some(host) = left.split('@').nth(1) {
            let path = path.trim_start_matches('/');
            return format!("{}/{}", host, path);
        }
    }
    lower
}

/// Check that repository URL is not already imported (duplicate in database).
async fn check_repository_not_duplicate(
    pool: &PgPool,
    repository_url: &str,
) -> Result<(), ApiError> {
    let normalized_input = normalize_repo_url_for_comparison(repository_url);
    if normalized_input.is_empty() {
        return Ok(());
    }

    let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        r#"SELECT id, name, repository_url FROM projects WHERE repository_url IS NOT NULL AND repository_url != ''"#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    for (id, name, repo_url) in rows {
        if let Some(url) = repo_url {
            if normalize_repo_url_for_comparison(&url) == normalized_input {
                return Err(ApiError::Conflict(format!(
                    "Repository already imported as project \"{}\" (ID: {}). Use that project instead.",
                    name, id
                )));
            }
        }
    }
    Ok(())
}

async fn check_repository_not_duplicate_except(
    pool: &PgPool,
    repository_url: &str,
    project_id: Uuid,
) -> Result<(), ApiError> {
    let normalized_input = normalize_repo_url_for_comparison(repository_url);
    if normalized_input.is_empty() {
        return Ok(());
    }

    let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT id, name, repository_url
        FROM projects
        WHERE id != $1
          AND repository_url IS NOT NULL
          AND repository_url != ''
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    for (id, name, repo_url) in rows {
        if let Some(url) = repo_url {
            if normalize_repo_url_for_comparison(&url) == normalized_input {
                return Err(ApiError::Conflict(format!(
                    "Repository already imported as project \"{}\" (ID: {}). Use that project instead.",
                    name, id
                )));
            }
        }
    }

    Ok(())
}

/// Pre-check that repository is cloneable (exists, accessible) using git ls-remote.
async fn check_repository_cloneable(
    settings_service: &SystemSettingsService,
    repo_url: &str,
) -> Result<(), String> {
    RepositoryAccessService::new(settings_service.clone())
        .check_cloneable(repo_url)
        .await
}

async fn check_repository_cloneable_with_retry(
    settings_service: &SystemSettingsService,
    repo_url: &str,
    attempts: usize,
    delay: Duration,
) -> Option<String> {
    RepositoryAccessService::new(settings_service.clone())
        .check_cloneable_with_retry(repo_url, attempts, delay)
        .await
}

fn repository_provider_label(provider: RepositoryProvider) -> &'static str {
    match provider {
        RepositoryProvider::Github => "github",
        RepositoryProvider::Gitlab => "gitlab",
        RepositoryProvider::Unknown => "unknown",
    }
}

fn repository_access_mode_metric_label(mode: RepositoryAccessMode) -> &'static str {
    match mode {
        RepositoryAccessMode::AnalysisOnly => "analysis_only",
        RepositoryAccessMode::DirectGitops => "direct_gitops",
        RepositoryAccessMode::BranchPushOnly => "branch_push_only",
        RepositoryAccessMode::ForkGitops => "fork_gitops",
        RepositoryAccessMode::Unknown => "unknown",
    }
}

fn repository_verification_status_label(status: RepositoryVerificationStatus) -> &'static str {
    match status {
        RepositoryVerificationStatus::Verified => "verified",
        RepositoryVerificationStatus::Unauthenticated => "unauthenticated",
        RepositoryVerificationStatus::Failed => "failed",
        RepositoryVerificationStatus::Unknown => "unknown",
    }
}

fn record_repository_context_evaluation_metric(
    metrics: &crate::observability::Metrics,
    source: &str,
    context: &RepositoryContext,
) {
    metrics
        .repository_access_evaluations_total
        .with_label_values(&[
            source,
            repository_provider_label(context.provider),
            repository_access_mode_metric_label(context.access_mode),
            repository_verification_status_label(context.verification_status),
        ])
        .inc();
}

fn record_repository_fork_operation_metric(
    metrics: &crate::observability::Metrics,
    source: &str,
    provider: RepositoryProvider,
    result: &str,
) {
    metrics
        .repository_fork_operations_total
        .with_label_values(&[source, repository_provider_label(provider), result])
        .inc();
}

fn record_repository_backfill_metric(
    metrics: &crate::observability::Metrics,
    source: &str,
    result: &str,
) {
    metrics
        .repository_backfill_total
        .with_label_values(&[source, result])
        .inc();
}

fn project_needs_repository_context_backfill(project: &Project) -> bool {
    project
        .repository_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        && project.repository_context.needs_backfill()
}

async fn backfill_project_repository_context(
    state: AppState,
    project_id: Uuid,
    source: &str,
) -> Result<(), String> {
    let service = ProjectService::new(state.db.clone());
    let Some(project) = service
        .get_project(project_id)
        .await
        .map_err(|e| e.to_string())?
    else {
        record_repository_backfill_metric(&state.metrics, source, "project_missing");
        return Ok(());
    };

    if !project_needs_repository_context_backfill(&project) {
        record_repository_backfill_metric(&state.metrics, source, "skipped");
        return Ok(());
    }

    let repository_url = project
        .repository_url
        .clone()
        .ok_or_else(|| "Project has no repository URL to backfill".to_string())?;
    let repository_access_service = RepositoryAccessService::new((*state.settings_service).clone());
    let clone_error = repository_access_service
        .check_cloneable(&repository_url)
        .await
        .err();
    let can_clone = clone_error.is_none();
    let repository_context = repository_context_with_clone_result(
        repository_access_service
            .preflight(&repository_url, can_clone)
            .await,
        clone_error,
    );
    record_repository_context_evaluation_metric(&state.metrics, source, &repository_context);

    service
        .update_project(
            project_id,
            UpdateProjectRequest {
                name: None,
                description: None,
                repository_url: None,
                repository_context: Some(repository_context.clone()),
                metadata: None,
                require_review: None,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    record_repository_backfill_metric(&state.metrics, source, "success");
    tracing::info!(
        project_id = %project_id,
        repository_url = %repository_url,
        provider = repository_provider_label(repository_context.provider),
        access_mode = repository_access_mode_metric_label(repository_context.access_mode),
        verification_status = repository_verification_status_label(repository_context.verification_status),
        "Legacy repository context backfilled"
    );

    Ok(())
}

fn warnings_for_repository_context(context: &RepositoryContext) -> Vec<String> {
    let mut warnings = Vec::new();

    if context.is_read_only() {
        warnings.push(
            "Repository is currently read-only for agent workflows. Import can proceed for analysis, but coding attempts should be blocked until writable access is configured."
                .to_string(),
        );
    }

    if !context.can_push {
        warnings.push("Current credentials cannot push branches to this repository.".to_string());
    }

    if !context.can_open_change_request {
        warnings.push(
            "Current credentials cannot create pull/merge requests automatically.".to_string(),
        );
    }

    if let Some(error) = context
        .verification_error
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        warnings.push(error.clone());
    }

    warnings
}

fn repository_context_with_clone_result(
    mut context: RepositoryContext,
    clone_error: Option<String>,
) -> RepositoryContext {
    if let Some(error) = clone_error {
        context.access_mode = RepositoryAccessMode::Unknown;
        context.verification_status = RepositoryVerificationStatus::Failed;
        context.verification_error = Some(match context.verification_error {
            Some(existing) if !existing.trim().is_empty() => {
                format!("{} Clone check failed: {}", existing, error)
            }
            _ => format!("Clone check failed: {}", error),
        });
        context.can_clone = false;
        context.can_push = false;
        context.can_open_change_request = false;
        context.can_merge = false;
        context.can_manage_webhooks = false;
    }

    context
}

fn build_linked_fork_repository_context(
    current_context: &RepositoryContext,
    upstream_repository_url: &str,
    writable_repository_url: &str,
    upstream_context: RepositoryContext,
    fork_context: RepositoryContext,
) -> Result<RepositoryContext, ApiError> {
    let provider = if fork_context.provider != RepositoryProvider::Unknown {
        fork_context.provider
    } else if upstream_context.provider != RepositoryProvider::Unknown {
        upstream_context.provider
    } else {
        current_context.provider
    };

    let access_mode = if fork_context.can_push && fork_context.can_open_change_request {
        RepositoryAccessMode::ForkGitops
    } else if fork_context.can_push {
        RepositoryAccessMode::BranchPushOnly
    } else {
        return Err(ApiError::BadRequest(
            "Linked fork must allow push access.".to_string(),
        ));
    };

    let normalized_upstream = normalize_repo_url_for_comparison(upstream_repository_url);
    let normalized_writable = normalize_repo_url_for_comparison(writable_repository_url);
    if normalized_upstream.is_empty()
        || normalized_writable.is_empty()
        || normalized_upstream == normalized_writable
    {
        return Err(ApiError::BadRequest(
            "Fork URL must point to a repository that is different from the upstream repository."
                .to_string(),
        ));
    }

    Ok(RepositoryContext {
        provider,
        access_mode,
        verification_status: fork_context.verification_status,
        verification_error: fork_context.verification_error.clone(),
        can_clone: fork_context.can_clone,
        can_push: fork_context.can_push,
        can_open_change_request: fork_context.can_open_change_request,
        can_merge: fork_context.can_merge,
        can_manage_webhooks: fork_context.can_manage_webhooks,
        can_fork: fork_context.can_fork || upstream_context.can_fork || current_context.can_fork,
        upstream_repository_url: Some(
            upstream_context
                .upstream_repository_url
                .clone()
                .unwrap_or_else(|| upstream_repository_url.to_string()),
        ),
        writable_repository_url: Some(
            fork_context
                .writable_repository_url
                .clone()
                .unwrap_or_else(|| writable_repository_url.to_string()),
        ),
        effective_clone_url: Some(
            fork_context
                .effective_clone_url
                .clone()
                .unwrap_or_else(|| writable_repository_url.to_string()),
        ),
        default_branch: upstream_context
            .default_branch
            .clone()
            .or_else(|| fork_context.default_branch.clone())
            .or_else(|| current_context.default_branch.clone()),
        upstream_project_id: upstream_context
            .upstream_project_id
            .or(current_context.upstream_project_id),
        writable_project_id: fork_context
            .writable_project_id
            .or(fork_context.upstream_project_id)
            .or(current_context.writable_project_id),
        verified_at: fork_context.verified_at.or(upstream_context.verified_at),
    })
}

fn recommended_action_for_repository_context(context: &RepositoryContext) -> Option<String> {
    match context.access_mode {
        RepositoryAccessMode::DirectGitops => {
            Some("Repository is ready for full GitOps workflow.".to_string())
        }
        RepositoryAccessMode::AnalysisOnly => Some(
            "Import for analysis only, then link or create a writable fork before starting coding tasks."
                .to_string(),
        ),
        RepositoryAccessMode::BranchPushOnly => Some(
            "Repository allows branch push but not automatic PR/MR creation. Expect manual review flow."
                .to_string(),
        ),
        RepositoryAccessMode::ForkGitops => Some(
            "Repository should use fork-based GitOps. Push to the writable fork and open PR/MR back to upstream."
                .to_string(),
        ),
        RepositoryAccessMode::Unknown => Some(
            "Re-check repository access after configuring credentials. Until then, treat this import as analysis-only."
                .to_string(),
        ),
    }
}

#[cfg(test)]
mod repository_context_tests {
    use super::*;

    fn base_context(provider: RepositoryProvider) -> RepositoryContext {
        RepositoryContext {
            provider,
            verification_status: RepositoryVerificationStatus::Verified,
            can_clone: true,
            verified_at: Some(Utc::now()),
            ..RepositoryContext::default()
        }
    }

    #[test]
    fn linked_fork_context_uses_fork_gitops_when_fork_can_open_changes() {
        let mut upstream = base_context(RepositoryProvider::Github);
        upstream.can_fork = true;
        upstream.upstream_repository_url = Some("https://github.com/acme/app".to_string());
        upstream.default_branch = Some("main".to_string());
        upstream.upstream_project_id = Some(101);

        let mut fork = base_context(RepositoryProvider::Github);
        fork.can_push = true;
        fork.can_open_change_request = true;
        fork.can_merge = true;
        fork.writable_repository_url = Some("https://github.com/me/app".to_string());
        fork.effective_clone_url = Some("https://github.com/me/app".to_string());
        fork.writable_project_id = Some(202);

        let linked = build_linked_fork_repository_context(
            &RepositoryContext::default(),
            "https://github.com/acme/app",
            "https://github.com/me/app",
            upstream,
            fork,
        )
        .expect("linked fork context should succeed");

        assert_eq!(linked.access_mode, RepositoryAccessMode::ForkGitops);
        assert_eq!(
            linked.upstream_repository_url.as_deref(),
            Some("https://github.com/acme/app")
        );
        assert_eq!(
            linked.writable_repository_url.as_deref(),
            Some("https://github.com/me/app")
        );
        assert_eq!(linked.upstream_project_id, Some(101));
        assert_eq!(linked.writable_project_id, Some(202));
        assert_eq!(linked.default_branch.as_deref(), Some("main"));
    }

    #[test]
    fn linked_fork_context_falls_back_to_branch_push_only_when_pr_is_blocked() {
        let mut upstream = base_context(RepositoryProvider::Gitlab);
        upstream.upstream_repository_url = Some("https://gitlab.com/group/app".to_string());

        let mut fork = base_context(RepositoryProvider::Gitlab);
        fork.can_push = true;
        fork.can_open_change_request = false;
        fork.writable_repository_url = Some("https://gitlab.com/me/app".to_string());
        fork.effective_clone_url = Some("https://gitlab.com/me/app".to_string());

        let linked = build_linked_fork_repository_context(
            &RepositoryContext::default(),
            "https://gitlab.com/group/app",
            "https://gitlab.com/me/app",
            upstream,
            fork,
        )
        .expect("branch-push-only fork should still be accepted");

        assert_eq!(linked.access_mode, RepositoryAccessMode::BranchPushOnly);
        assert!(linked.can_push);
        assert!(!linked.can_open_change_request);
    }

    #[test]
    fn linked_fork_context_rejects_same_upstream_and_fork_url() {
        let mut upstream = base_context(RepositoryProvider::Github);
        upstream.upstream_repository_url = Some("https://github.com/acme/app".to_string());

        let mut fork = base_context(RepositoryProvider::Github);
        fork.can_push = true;
        fork.can_open_change_request = true;

        let error = build_linked_fork_repository_context(
            &RepositoryContext::default(),
            "https://github.com/acme/app",
            "https://github.com/acme/app.git",
            upstream,
            fork,
        )
        .expect_err("same upstream/fork URL should be rejected");

        match error {
            ApiError::BadRequest(message) => {
                assert!(message.contains("different from the upstream repository"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn normalize_repo_url_for_comparison_strips_credentials_suffix_and_case() {
        let normalized =
            normalize_repo_url_for_comparison("https://oauth2:secret@GitHub.com/Acme/App.git/");
        assert_eq!(normalized, "github.com/acme/app");
    }

    #[test]
    fn clone_failure_downgrades_repository_context_to_failed_unknown() {
        let context = repository_context_with_clone_result(
            RepositoryContext {
                provider: RepositoryProvider::Github,
                access_mode: RepositoryAccessMode::DirectGitops,
                verification_status: RepositoryVerificationStatus::Verified,
                can_clone: true,
                can_push: true,
                can_open_change_request: true,
                can_merge: true,
                can_manage_webhooks: true,
                verified_at: Some(Utc::now()),
                ..RepositoryContext::default()
            },
            Some("git ls-remote exited with status 128".to_string()),
        );

        assert_eq!(context.access_mode, RepositoryAccessMode::Unknown);
        assert_eq!(
            context.verification_status,
            RepositoryVerificationStatus::Failed
        );
        assert!(!context.can_clone);
        assert!(!context.can_push);
        assert!(!context.can_open_change_request);
        assert!(!context.can_merge);
        assert!(!context.can_manage_webhooks);
        assert!(context
            .verification_error
            .as_deref()
            .is_some_and(|message| message.contains("Clone check failed")));
    }

    #[test]
    fn warnings_include_read_only_push_pr_and_error_guidance() {
        let warnings = warnings_for_repository_context(&RepositoryContext {
            access_mode: RepositoryAccessMode::AnalysisOnly,
            verification_status: RepositoryVerificationStatus::Failed,
            can_clone: true,
            can_push: false,
            can_open_change_request: false,
            verification_error: Some("Clone check failed: auth denied".to_string()),
            ..RepositoryContext::default()
        });

        assert_eq!(warnings.len(), 4);
        assert!(warnings.iter().any(|warning| warning.contains("read-only")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("cannot push")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("cannot create pull/merge requests")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("Clone check failed: auth denied")));
    }

    #[test]
    fn recommended_action_matches_access_mode() {
        assert!(
            recommended_action_for_repository_context(&RepositoryContext {
                access_mode: RepositoryAccessMode::DirectGitops,
                ..RepositoryContext::default()
            })
            .is_some_and(|value| value.contains("full GitOps workflow"))
        );
        assert!(
            recommended_action_for_repository_context(&RepositoryContext {
                access_mode: RepositoryAccessMode::AnalysisOnly,
                ..RepositoryContext::default()
            })
            .is_some_and(|value| value.contains("analysis only"))
        );
        assert!(
            recommended_action_for_repository_context(&RepositoryContext {
                access_mode: RepositoryAccessMode::BranchPushOnly,
                ..RepositoryContext::default()
            })
            .is_some_and(|value| value.contains("manual review flow"))
        );
        assert!(
            recommended_action_for_repository_context(&RepositoryContext {
                access_mode: RepositoryAccessMode::ForkGitops,
                ..RepositoryContext::default()
            })
            .is_some_and(|value| value.contains("fork-based GitOps"))
        );
        assert!(
            recommended_action_for_repository_context(&RepositoryContext {
                access_mode: RepositoryAccessMode::Unknown,
                ..RepositoryContext::default()
            })
            .is_some_and(|value| value.contains("Re-check repository access"))
        );
    }
}

/// Validate that repository URL host is allowed.
/// Allowed: gitlab.com, github.com, or host from configured gitlab_url (GitLab/GitHub - system chỉ setup 1).
async fn validate_repo_url(pool: &PgPool, url: &str) -> Result<(), ApiError> {
    let parsed = url::Url::parse(url)
        .map_err(|_| ApiError::BadRequest("Invalid repository URL".to_string()))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| ApiError::BadRequest("Repository URL missing host".to_string()))?;

    let mut allowed_hosts = BTreeSet::<String>::new();
    allowed_hosts.insert("gitlab.com".to_string());
    allowed_hosts.insert("github.com".to_string());

    let configured_url =
        sqlx::query_scalar::<_, Option<String>>("SELECT gitlab_url FROM system_settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .flatten();
    if let Some(ref u) = configured_url {
        if let Some(h) = parse_host_from_urlish(u) {
            allowed_hosts.insert(h);
        }
    }

    if !allowed_hosts.iter().any(|h| h.eq_ignore_ascii_case(host)) {
        let allowed_list = allowed_hosts.into_iter().collect::<Vec<_>>().join(", ");
        return Err(ApiError::BadRequest(format!(
            "Repository URL host '{}' is not allowed. Allowed hosts: {}. \
            Configure the instance URL in Settings (GitLab or GitHub).",
            host, allowed_list
        )));
    }

    Ok(())
}

fn parse_host_from_urlish(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(parsed) = url::Url::parse(trimmed) {
        return parsed.host_str().map(|h| h.to_string());
    }

    let with_scheme = format!("https://{}", trimmed.trim_start_matches('/'));
    url::Url::parse(&with_scheme)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
}

// ===== Project Settings Endpoints =====

/// Response for project settings
#[allow(dead_code)]
pub type ProjectSettingsApiResponse = ApiResponse<ProjectSettingsResponse>;

/// Get project settings
#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}/settings",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Project settings retrieved", body = ProjectSettingsApiResponse),
        (status = 404, description = "Project not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_project_settings(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<ProjectSettingsResponse>>> {
    // Check permission (ViewProject = all roles)
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &pool).await?;

    let service = ProjectService::new(pool.clone());
    let settings = service
        .get_settings(project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response_data = ProjectSettingsResponse {
        settings,
        defaults: ProjectSettings::default(),
    };

    let response = ApiResponse::success(response_data, "Project settings retrieved");
    Ok(Json(response))
}

/// Update project settings (full replacement)
#[utoipa::path(
    put,
    path = "/api/v1/projects/{id}/settings",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    request_body = ProjectSettings,
    responses(
        (status = 200, description = "Project settings updated", body = ProjectSettingsApiResponse),
        (status = 404, description = "Project not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_project_settings(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    Path(project_id): Path<Uuid>,
    Json(settings): Json<ProjectSettings>,
) -> ApiResult<Json<ApiResponse<ProjectSettingsResponse>>> {
    // Check permission (ManageProject = Owner, Admin)
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ManageProject, &pool)
        .await?;

    let service = ProjectService::new(pool.clone());
    let updated_settings = service
        .update_settings(project_id, settings)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response_data = ProjectSettingsResponse {
        settings: updated_settings,
        defaults: ProjectSettings::default(),
    };

    let response = ApiResponse::success(response_data, "Project settings updated");
    Ok(Json(response))
}

/// Request body for updating a single setting
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSingleSettingRequest {
    #[schema(value_type = Object)]
    pub value: serde_json::Value,
}

/// Path parameters for single setting update
#[derive(Debug, Deserialize)]
pub struct SettingPathParams {
    pub id: Uuid,
    pub key: String,
}

/// Update a single project setting by key (partial update)
#[utoipa::path(
    patch,
    path = "/api/v1/projects/{id}/settings/{key}",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project ID"),
        ("key" = String, Path, description = "Setting key to update")
    ),
    request_body = UpdateSingleSettingRequest,
    responses(
        (status = 200, description = "Project setting updated", body = ProjectSettingsApiResponse),
        (status = 400, description = "Invalid setting key"),
        (status = 404, description = "Project not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_single_project_setting(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    Path(params): Path<SettingPathParams>,
    Json(req): Json<UpdateSingleSettingRequest>,
) -> ApiResult<Json<ApiResponse<ProjectSettingsResponse>>> {
    let project_id = params.id;
    let key = params.key;

    // Validate the setting key
    let valid_keys = [
        "require_review",
        "auto_deploy",
        "preview_enabled",
        "production_deploy_on_merge",
        "gitops_enabled",
        "max_retries",
        "timeout_mins",
        "preview_ttl_days",
        "auto_merge",
        "deploy_branch",
        "notify_on_success",
        "notify_on_failure",
        "notify_on_review",
        "notify_channels",
        "auto_execute",
        "auto_execute_types",
        "auto_retry",
        "auto_execute_priority",
        "retry_backoff",
        "max_concurrent",
    ];

    if !valid_keys.contains(&key.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid setting key '{}'. Valid keys: {}",
            key,
            valid_keys.join(", ")
        )));
    }

    // Check permission (ManageProject = Owner, Admin)
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ManageProject, &pool)
        .await?;

    let service = ProjectService::new(pool.clone());
    let updated_settings = service
        .update_single_setting(project_id, &key, req.value)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response_data = ProjectSettingsResponse {
        settings: updated_settings,
        defaults: ProjectSettings::default(),
    };

    let response = ApiResponse::success(response_data, format!("Setting '{}' updated", key));
    Ok(Json(response))
}
