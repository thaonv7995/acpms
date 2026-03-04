use acpms_db::models::SystemRole;
use acpms_db::{models::*, PgPool};
use acpms_services::{RefreshTokenService, TokenBlacklistService};
use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::Deserialize;
use std::time::Duration;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::api::{ApiResponse, AuthResponseDto, UserDto};
use crate::error::{ApiError, ApiResult};
use crate::middleware::{AuthUser, RbacChecker, ValidatedJson};

fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
    let parse_ip = |value: &str| {
        value
            .trim()
            .parse::<std::net::IpAddr>()
            .ok()
            .map(|ip| ip.to_string())
    };

    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        for candidate in xff.split(',') {
            if let Some(ip) = parse_ip(candidate) {
                return Some(ip);
            }
        }
    }

    headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_ip)
}

/// Helper function to convert S3 avatar key to presigned URL
/// Same logic as in users.rs
async fn convert_avatar_to_url(
    avatar_url: Option<String>,
    storage_service: &acpms_services::StorageService,
) -> Option<String> {
    match avatar_url {
        Some(url) if url.starts_with("http://") || url.starts_with("https://") => Some(url),
        Some(key) if !key.is_empty() => storage_service
            .get_presigned_download_url(&key, Duration::from_secs(3600))
            .await
            .ok(),
        _ => None,
    }
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
#[serde(deny_unknown_fields)]
pub struct RegisterRequest {
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
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,

    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "Auth",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered successfully", body = AuthResponse),
        (status = 409, description = "Email already exists")
    )
)]
pub async fn register(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    ValidatedJson(req): ValidatedJson<RegisterRequest>,
) -> ApiResult<(StatusCode, Json<ApiResponse<AuthResponseDto>>)> {
    let pool = state.db.clone();

    // Validate password
    acpms_services::validate_password(&req.password)
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    // Hash password
    let password_hash = acpms_services::hash_password(&req.password)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Public self-registration always starts with the least-privileged role.
    let roles = vec![SystemRole::Viewer];

    // Create user
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (email, name, password_hash, global_roles)
        VALUES ($1, $2, $3, $4)
        RETURNING id, email, name, avatar_url, gitlab_id, gitlab_username, password_hash, global_roles, created_at, updated_at
        "#
    )
    .bind(&req.email)
    .bind(&req.name)
    .bind(&password_hash)
    .bind(&roles)
    .fetch_one(&pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            ApiError::Conflict("Email already exists".to_string())
        }
        _ => ApiError::Database(e),
    })?;

    // Generate access token
    let access_token =
        acpms_services::generate_jwt(user.id).map_err(|e| ApiError::Internal(e.to_string()))?;

    // Generate refresh token
    let refresh_service = RefreshTokenService::new(pool.clone());
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let ip_address = extract_client_ip(&headers);

    let (refresh_token, _) = refresh_service
        .generate_refresh_token(user.id, user_agent, ip_address)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let expires_in = acpms_services::get_access_token_expiration().num_seconds();

    let mut user_dto = UserDto::from(user);

    // Convert S3 avatar key to presigned URL
    user_dto.avatar_url = convert_avatar_to_url(user_dto.avatar_url, &state.storage_service).await;

    let dto = AuthResponseDto {
        access_token,
        refresh_token,
        expires_in,
        user: user_dto,
    };

    let response = ApiResponse::created(dto, "User registered successfully");

    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "Auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn login(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    ValidatedJson(req): ValidatedJson<LoginRequest>,
) -> ApiResult<Json<ApiResponse<AuthResponseDto>>> {
    let pool = state.db.clone();

    // Fetch user by email
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, email, name, avatar_url, gitlab_id, gitlab_username, password_hash, global_roles, created_at, updated_at
        FROM users
        WHERE email = $1
        "#
    )
    .bind(&req.email)
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::Unauthorized)?;

    // Verify password
    let valid = match &user.password_hash {
        Some(hash) => acpms_services::verify_password(&req.password, hash).unwrap_or(false),
        None => false,
    };

    if !valid {
        return Err(ApiError::Unauthorized);
    }

    // Mark latest user activity timestamp for user-management status.
    let user = sqlx::query_as::<_, User>(
        r#"
        UPDATE users
        SET updated_at = NOW()
        WHERE id = $1
        RETURNING id, email, name, avatar_url, gitlab_id, gitlab_username, password_hash, global_roles, created_at, updated_at
        "#,
    )
    .bind(user.id)
    .fetch_one(&pool)
    .await?;

    // Generate access token
    let access_token =
        acpms_services::generate_jwt(user.id).map_err(|e| ApiError::Internal(e.to_string()))?;

    // Generate refresh token
    let refresh_service = RefreshTokenService::new(pool.clone());
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let ip_address = extract_client_ip(&headers);

    let (refresh_token, _) = refresh_service
        .generate_refresh_token(user.id, user_agent, ip_address)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let expires_in = acpms_services::get_access_token_expiration().num_seconds();

    let mut user_dto = UserDto::from(user);

    // Convert S3 avatar key to presigned URL
    user_dto.avatar_url = convert_avatar_to_url(user_dto.avatar_url, &state.storage_service).await;

    let dto = AuthResponseDto {
        access_token,
        refresh_token,
        expires_in,
        user: user_dto,
    };

    let response = ApiResponse::success(dto, "Login successful");

    Ok(Json(response))
}

// Refresh Token Request DTO
#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct RefreshTokenRequest {
    #[validate(length(min = 1, message = "Refresh token is required"))]
    pub refresh_token: String,
}

// Refresh Token Response DTO
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct RefreshTokenResponse {
    pub access_token: String,
    pub expires_in: i64,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "Auth",
    request_body = RefreshTokenRequest,
    responses(
        (status = 200, description = "Token refreshed successfully", body = RefreshTokenResponse),
        (status = 401, description = "Invalid or expired refresh token")
    )
)]
pub async fn refresh_token(
    State(pool): State<PgPool>,
    _headers: HeaderMap,
    ValidatedJson(req): ValidatedJson<RefreshTokenRequest>,
) -> ApiResult<Json<ApiResponse<RefreshTokenResponse>>> {
    let refresh_service = RefreshTokenService::new(pool.clone());

    // Verify refresh token and get user_id
    let user_id = refresh_service
        .verify_refresh_token(&req.refresh_token)
        .await
        .map_err(|_| ApiError::Unauthorized)?;

    // Track user activity on token refresh as well.
    sqlx::query("UPDATE users SET updated_at = NOW() WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await?;

    // Generate new access token
    let access_token =
        acpms_services::generate_jwt(user_id).map_err(|e| ApiError::Internal(e.to_string()))?;

    let expires_in = acpms_services::get_access_token_expiration().num_seconds();

    // Optional: Rotate refresh token (revoke old, generate new)
    // For now, we'll keep the same refresh token and just update last_used_at

    let response_dto = RefreshTokenResponse {
        access_token,
        expires_in,
    };

    let response = ApiResponse::success(response_dto, "Token refreshed successfully");
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    tag = "Auth",
    security(("bearer_auth" = [])),
    request_body = RefreshTokenRequest,
    responses(
        (status = 200, description = "Logged out successfully"),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn logout(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    ValidatedJson(req): ValidatedJson<RefreshTokenRequest>,
) -> ApiResult<Json<ApiResponse<()>>> {
    let refresh_service = RefreshTokenService::new(pool.clone());
    let blacklist_service = TokenBlacklistService::new(pool.clone());

    // Revoke the refresh token
    refresh_service
        .revoke_refresh_token(&req.refresh_token)
        .await
        .ok(); // Ignore error if token not found

    // Blacklist the current access token
    let expires_at = Utc::now() + acpms_services::get_access_token_expiration();
    blacklist_service
        .blacklist_access_token(
            &auth_user.jti,
            auth_user.id,
            expires_at,
            Some("User logout".to_string()),
            None,
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = ApiResponse::success((), "Logged out successfully");
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/revoke/{user_id}",
    tag = "Auth",
    security(("bearer_auth" = [])),
    params(
        ("user_id" = Uuid, Path, description = "User ID to revoke tokens for")
    ),
    responses(
        (status = 200, description = "User tokens revoked successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin only")
    )
)]
pub async fn revoke_user_tokens(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    Path(user_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<()>>> {
    RbacChecker::check_system_admin(auth_user.id, &pool).await?;

    let refresh_service = RefreshTokenService::new(pool.clone());

    // Revoke all refresh tokens for the user
    let count = refresh_service
        .revoke_all_user_tokens(user_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let message = format!("Revoked {} refresh tokens for user", count);
    let response = ApiResponse::success((), &message);
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::extract_client_ip;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn extract_client_ip_prefers_first_valid_xff_ip() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("198.51.100.42, 203.0.113.15"),
        );
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.10"));

        assert_eq!(
            extract_client_ip(&headers).as_deref(),
            Some("198.51.100.42")
        );
    }

    #[test]
    fn extract_client_ip_falls_back_to_x_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("not-an-ip, still-not-an-ip"),
        );
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.10"));

        assert_eq!(extract_client_ip(&headers).as_deref(), Some("203.0.113.10"));
    }

    #[test]
    fn extract_client_ip_returns_none_for_invalid_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("invalid"));
        headers.insert("x-real-ip", HeaderValue::from_static("invalid"));

        assert_eq!(extract_client_ip(&headers), None);
    }
}
