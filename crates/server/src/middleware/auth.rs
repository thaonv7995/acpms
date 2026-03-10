use acpms_db::PgPool;
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
    RequestPartsExt,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub jti: String, // JWT ID for potential blacklist operations
}

async fn is_token_blacklisted(pool: &PgPool, jti: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM token_blacklist
            WHERE jti = $1 AND expires_at > NOW()
        )
        "#,
    )
    .bind(jti)
    .fetch_one(pool)
    .await
}

pub async fn authenticate_bearer_token<S>(token: &str, state: &S) -> Result<AuthUser, ApiError>
where
    S: Send + Sync,
    PgPool: FromRef<S>,
{
    // Verify JWT and extract claims
    let claims = acpms_services::verify_jwt(token).map_err(|e| {
        tracing::debug!(error = %e, "JWT verification failed");
        ApiError::Unauthorized
    })?;

    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| ApiError::Unauthorized)?;

    // Get database pool from state
    let pool = PgPool::from_ref(state);

    // Fail closed: if blacklist query fails, reject token.
    let is_blacklisted = is_token_blacklisted(&pool, &claims.jti)
        .await
        .map_err(|e| {
            tracing::error!("Failed to check token blacklist: {:?}", e);
            ApiError::Unauthorized
        })?;

    if is_blacklisted {
        return Err(ApiError::Unauthorized);
    }

    Ok(AuthUser {
        id: user_id,
        jti: claims.jti,
    })
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    PgPool: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if let Some(auth_user) = parts.extensions.get::<AuthUser>().cloned() {
            return Ok(auth_user);
        }

        // Extract Authorization header
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|e| {
                tracing::debug!("Missing or invalid Authorization header: {:?}", e);
                ApiError::Unauthorized
            })?;

        authenticate_bearer_token(bearer.token(), state).await
    }
}
