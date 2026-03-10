use acpms_db::models::SystemRole;
use axum::extract::{Json, Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use std::time::Duration;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

use crate::api::{ApiResponse, UserDto};
use crate::error::{ApiError, ApiResult};
use crate::middleware::{AuthUser, RbacChecker, ValidatedJson};
use crate::AppState;
use acpms_services::{UserDirectoryStats, UserListStatus};
use serde::Serialize;
use validator::ValidationError;

/// Validate avatar_url: accept S3 key (avatars/...) or full URL. Reject invalid formats.
fn validate_avatar_url(val: &str) -> Result<(), ValidationError> {
    if val.is_empty() {
        return Ok(());
    }
    if val.starts_with("avatars/") || val.starts_with("http://") || val.starts_with("https://") {
        return Ok(());
    }
    Err(
        ValidationError::new("avatar_url").with_message(std::borrow::Cow::Borrowed(
            "Must be S3 key (avatars/...) or full URL",
        )),
    )
}

/// Helper function to convert S3 avatar key to presigned URL
/// If avatar_url is already a full URL (http/https), return as-is
async fn convert_avatar_to_url(
    avatar_url: Option<String>,
    storage_service: &acpms_services::StorageService,
) -> Option<String> {
    match avatar_url {
        Some(url) if url.starts_with("http://") || url.starts_with("https://") => {
            // Already a full URL, return as-is
            Some(url)
        }
        Some(key) if !key.is_empty() => {
            // S3 key, convert to presigned URL
            storage_service
                .get_presigned_download_url(&key, Duration::from_secs(3600))
                .await
                .ok()
        }
        _ => None,
    }
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct UpdateUserRequest {
    #[validate(length(
        min = 1,
        max = 100,
        message = "Name must be between 1 and 100 characters"
    ))]
    pub name: Option<String>,

    /// Avatar: S3 key (avatars/...) or full URL. Backend converts S3 keys to presigned URLs on read.
    #[validate(custom(function = "validate_avatar_url"))]
    pub avatar_url: Option<String>,

    #[validate(length(
        min = 1,
        max = 50,
        message = "GitLab username must be between 1 and 50 characters"
    ))]
    pub gitlab_username: Option<String>,

    /// Global roles for the user
    #[schema(value_type = Option<Vec<String>>)]
    pub global_roles: Option<Vec<SystemRole>>,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 1, message = "Current password is required"))]
    pub current_password: String,

    #[validate(length(min = 8, message = "New password must be at least 8 characters"))]
    pub new_password: String,
}

fn sanitize_avatar_filename(filename: &str) -> String {
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
        "avatar.jpg".to_string()
    } else {
        trimmed.to_string()
    }
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateUserRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,

    #[validate(length(
        min = 1,
        max = 100,
        message = "Name must be between 1 and 100 characters"
    ))]
    pub name: String,

    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,

    /// Global roles for the new user. Defaults to [viewer] if empty.
    #[schema(value_type = Vec<String>)]
    pub global_roles: Option<Vec<SystemRole>>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserStatusQuery {
    Active,
    Inactive,
    Pending,
}

impl From<UserStatusQuery> for UserListStatus {
    fn from(value: UserStatusQuery) -> Self {
        match value {
            UserStatusQuery::Active => UserListStatus::Active,
            UserStatusQuery::Inactive => UserListStatus::Inactive,
            UserStatusQuery::Pending => UserListStatus::Pending,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct UsersQuery {
    /// Max results per page (default: 10, max: 100)
    pub limit: Option<u32>,
    /// Page number (1-based)
    pub page: Option<u32>,
    /// Case-insensitive search across name and email
    pub search: Option<String>,
    pub role: Option<SystemRole>,
    pub status: Option<UserStatusQuery>,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct GetUploadUrlRequest {
    pub filename: String,
    pub content_type: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UploadUrlResponse {
    pub upload_url: String,
    pub key: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/users",
    tag = "Users",
    params(UsersQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List all users", body = UserListResponse)
    )
)]
pub async fn list_users(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<UsersQuery>,
) -> ApiResult<Json<ApiResponse<Vec<UserDto>>>> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    let page_size = query.limit.unwrap_or(10).clamp(1, 100);
    let page_num = query.page.unwrap_or(1).max(1);
    let offset = i64::from((page_num - 1) * page_size);
    let role_filter = query.role;
    let status_filter = query.status.map(Into::into);
    let search_ref = query.search.as_deref();

    let users = state
        .user_service
        .get_users_page(
            i64::from(page_size),
            offset,
            search_ref,
            role_filter,
            status_filter,
        )
        .await
        .map_err(ApiError::Database)?;
    let total_count = state
        .user_service
        .count_users(search_ref, role_filter, status_filter)
        .await
        .map_err(ApiError::Database)?;
    let stats = state
        .user_service
        .get_user_directory_stats()
        .await
        .map_err(ApiError::Database)?;

    let mut dtos: Vec<UserDto> = users.into_iter().map(UserDto::from).collect();

    // Convert S3 avatar keys to presigned URLs for all users
    for dto in &mut dtos {
        dto.avatar_url =
            convert_avatar_to_url(dto.avatar_url.clone(), &state.storage_service).await;
    }

    let total_pages = if total_count == 0 {
        1
    } else {
        ((total_count as f64) / (page_size as f64)).ceil() as i64
    };
    let has_more = i64::from(page_num) < total_pages;

    let mut response = ApiResponse::success(dtos, "Users retrieved successfully");
    response.metadata = Some(build_user_list_metadata(
        page_num,
        page_size,
        total_count,
        total_pages,
        has_more,
        stats,
    ));

    Ok(Json(response))
}

fn build_user_list_metadata(
    page: u32,
    page_size: u32,
    total_count: i64,
    total_pages: i64,
    has_more: bool,
    stats: UserDirectoryStats,
) -> serde_json::Value {
    serde_json::json!({
        "page": page,
        "page_size": page_size,
        "total_count": total_count,
        "total_pages": total_pages,
        "has_more": has_more,
        "stats": {
            "total": stats.total,
            "active": stats.active,
            "agents_paired": stats.agents_paired,
            "pending": stats.pending,
        }
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/users",
    tag = "Users",
    security(("bearer_auth" = [])),
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created successfully", body = UserResponse),
        (status = 403, description = "Forbidden - Admin only"),
        (status = 409, description = "Email already exists")
    )
)]
pub async fn create_user(
    auth_user: AuthUser,
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<CreateUserRequest>,
) -> ApiResult<(StatusCode, Json<ApiResponse<UserDto>>)> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    acpms_services::validate_password(&req.password)
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let password_hash = acpms_services::hash_password(&req.password)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let roles = req
        .global_roles
        .filter(|r| !r.is_empty())
        .unwrap_or_else(|| vec![SystemRole::Viewer]);

    let user = state
        .user_service
        .create_user(&req.email, &req.name, &password_hash, &roles)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                ApiError::Conflict("Email already exists".to_string())
            }
            _ => ApiError::Database(e),
        })?;

    let mut dto = UserDto::from(user);
    dto.avatar_url = convert_avatar_to_url(dto.avatar_url, &state.storage_service).await;

    let response = ApiResponse::created(dto, "User created successfully");
    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/{id}",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(
        ("id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "Get user details", body = UserResponse),
        (status = 404, description = "User not found")
    )
)]
pub async fn get_user(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<UserDto>>> {
    if auth_user.id != id {
        RbacChecker::check_system_admin(auth_user.id, &state.db).await?;
    }

    let user = state
        .user_service
        .get_user_by_id(id)
        .await
        .map_err(ApiError::Database)?
        .ok_or(ApiError::NotFound("User not found".to_string()))?;

    let mut dto = UserDto::from(user);

    // Convert S3 avatar key to presigned URL
    dto.avatar_url = convert_avatar_to_url(dto.avatar_url, &state.storage_service).await;

    let response = ApiResponse::success(dto, "User retrieved successfully");
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/v1/users/{id}",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(
        ("id" = Uuid, Path, description = "User ID")
    ),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "Update user", body = UserResponse),
        (status = 403, description = "Forbidden - Only admins can modify roles"),
        (status = 404, description = "User not found")
    )
)]
pub async fn update_user(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<UpdateUserRequest>,
) -> ApiResult<Json<ApiResponse<UserDto>>> {
    // Users can update their own profile (name/avatar/gitlab_username).
    // Updating other users OR updating global roles requires admin access.
    let updating_other_user = auth_user.id != id;
    let updating_roles = req.global_roles.is_some();
    if updating_other_user || updating_roles {
        RbacChecker::check_system_admin(auth_user.id, &state.db).await?;
    }

    let user = state
        .user_service
        .update_user(
            id,
            req.name,
            req.avatar_url,
            req.gitlab_username,
            req.global_roles,
        )
        .await
        .map_err(ApiError::Database)?
        .ok_or(ApiError::NotFound("User not found".to_string()))?;

    let mut dto = UserDto::from(user);

    // Convert S3 avatar key to presigned URL
    dto.avatar_url = convert_avatar_to_url(dto.avatar_url, &state.storage_service).await;

    let response = ApiResponse::success(dto, "User updated successfully");
    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/api/v1/users/{id}",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(
        ("id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "Delete user", body = EmptyResponse),
        (status = 404, description = "User not found")
    )
)]
pub async fn delete_user(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<()>>> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    if auth_user.id == id {
        return Err(ApiError::BadRequest(
            "Cannot delete your own account".to_string(),
        ));
    }

    let deleted = state
        .user_service
        .delete_user(id)
        .await
        .map_err(ApiError::Database)?;

    if !deleted {
        return Err(ApiError::NotFound("User not found".to_string()));
    }

    let response = ApiResponse::success((), "User deleted successfully");
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/v1/users/{id}/password",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(
        ("id" = Uuid, Path, description = "User ID")
    ),
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password changed successfully", body = EmptyResponse),
        (status = 400, description = "Invalid current password"),
        (status = 403, description = "Forbidden - can only change own password"),
        (status = 404, description = "User not found")
    )
)]
pub async fn change_password(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<ChangePasswordRequest>,
) -> ApiResult<Json<ApiResponse<()>>> {
    // Users can only change their own password
    if auth_user.id != id {
        return Err(ApiError::Forbidden(
            "You can only change your own password".to_string(),
        ));
    }

    // Get the user to verify current password
    let user = state
        .user_service
        .get_user_by_id(id)
        .await
        .map_err(ApiError::Database)?
        .ok_or(ApiError::NotFound("User not found".to_string()))?;

    // Verify current password
    let password_valid = match &user.password_hash {
        Some(hash) => acpms_services::verify_password(&req.current_password, hash).unwrap_or(false),
        None => false,
    };

    if !password_valid {
        return Err(ApiError::Validation(
            "Current password is incorrect".to_string(),
        ));
    }

    // Validate new password
    acpms_services::validate_password(&req.new_password)
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    // Hash new password
    let new_hash = acpms_services::hash_password(&req.new_password)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Update password
    state
        .user_service
        .change_password(id, new_hash)
        .await
        .map_err(ApiError::Database)?
        .ok_or(ApiError::NotFound("User not found".to_string()))?;

    let response = ApiResponse::success((), "Password changed successfully");
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/users/avatar/upload-url",
    tag = "Users",
    security(("bearer_auth" = [])),
    request_body = GetUploadUrlRequest,
    responses(
        (status = 200, description = "Get upload URL", body = UploadUrlResponse),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_avatar_upload_url(
    auth_user: AuthUser,
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<GetUploadUrlRequest>,
) -> ApiResult<Json<ApiResponse<UploadUrlResponse>>> {
    let safe_filename = sanitize_avatar_filename(&req.filename);
    let key = format!("avatars/{}/{}", auth_user.id, safe_filename);

    let url = state
        .storage_service
        .get_presigned_upload_url(
            &key,
            &req.content_type,
            std::time::Duration::from_secs(3600),
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = UploadUrlResponse {
        upload_url: url,
        key,
    };

    Ok(Json(ApiResponse::success(
        response,
        "Upload URL generated successfully",
    )))
}
