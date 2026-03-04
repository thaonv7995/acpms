use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use uuid::Uuid;

const JWT_SECRET_ENV: &str = "JWT_SECRET";
const ACCESS_TOKEN_EXPIRATION_MINUTES: i64 = 30; // 30 minutes for access tokens
const REFRESH_TOKEN_EXPIRATION_DAYS: i64 = 7; // 7 days for refresh tokens

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub exp: i64,    // expiration timestamp
    pub jti: String, // JWT ID for blacklist/revocation
    pub iat: i64,    // issued at timestamp
}

/// Hash password using bcrypt
pub fn hash_password(password: &str) -> Result<String> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST).context("Failed to hash password")
}

/// Verify password against hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    // Compatibility: some seeded/legacy bcrypt hashes use the `$2y$` prefix (common in Apache/PHP).
    // Rust `bcrypt` expects `$2a$`/`$2b$`, so normalize `$2y$` -> `$2b$`.
    let normalized: Cow<'_, str> = if hash.starts_with("$2y$") {
        Cow::Owned(hash.replacen("$2y$", "$2b$", 1))
    } else {
        Cow::Borrowed(hash)
    };

    bcrypt::verify(password, normalized.as_ref()).context("Failed to verify password")
}

/// Generate access JWT token for user (short-lived with JTI)
pub fn generate_jwt(user_id: Uuid) -> Result<String> {
    let secret =
        std::env::var(JWT_SECRET_ENV).context("JWT_SECRET environment variable not set")?;

    let now = Utc::now();
    let expiration = now
        .checked_add_signed(Duration::minutes(ACCESS_TOKEN_EXPIRATION_MINUTES))
        .context("Failed to calculate expiration")?
        .timestamp();

    let jti = Uuid::new_v4().to_string();

    let claims = Claims {
        sub: user_id.to_string(),
        exp: expiration,
        jti,
        iat: now.timestamp(),
    };

    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .context("Failed to generate JWT")
}

/// Get expiration duration for access tokens (for external use)
pub fn get_access_token_expiration() -> Duration {
    Duration::minutes(ACCESS_TOKEN_EXPIRATION_MINUTES)
}

/// Get expiration duration for refresh tokens (for external use)
pub fn get_refresh_token_expiration() -> Duration {
    Duration::days(REFRESH_TOKEN_EXPIRATION_DAYS)
}

/// Verify and decode JWT token
pub fn verify_jwt(token: &str) -> Result<Claims> {
    let secret =
        std::env::var(JWT_SECRET_ENV).context("JWT_SECRET environment variable not set")?;

    let token_data = jsonwebtoken::decode::<Claims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::default(),
    )
    .context("Failed to verify JWT")?;

    Ok(token_data.claims)
}

/// Validate password strength
pub fn validate_password(password: &str) -> Result<()> {
    if password.len() < 8 {
        anyhow::bail!("Password must be at least 8 characters");
    }

    // Check for at least one digit and one letter
    let has_digit = password.chars().any(|c| c.is_numeric());
    let has_letter = password.chars().any(|c| c.is_alphabetic());

    if !has_digit || !has_letter {
        anyhow::bail!("Password must contain both letters and numbers");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let password = "test_password123";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_password_validation() {
        assert!(validate_password("password123").is_ok());
        assert!(validate_password("short").is_err());
        assert!(validate_password("nodigits").is_err());
        assert!(validate_password("12345678").is_err());
    }

    #[test]
    fn test_verify_password_accepts_2y_hash_prefix() {
        // This matches the seeded admin hash in db migrations for "admin123".
        let hash_2y = "$2y$12$ovlS6fjllYtHTCmNjNANPegmUp96x.67NXlc.cPoWTcEurDB4rbJK";
        assert!(verify_password("admin123", hash_2y).unwrap());
        assert!(!verify_password("wrong_password", hash_2y).unwrap());
    }
}
