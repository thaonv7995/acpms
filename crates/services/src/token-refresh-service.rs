use anyhow::{Context, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::auth::get_refresh_token_expiration;

#[derive(FromRow)]
struct TokenIdResult {
    id: Uuid,
}

#[derive(FromRow)]
struct UserIdResult {
    user_id: Uuid,
}

/// Service for managing refresh tokens (generation, verification, revocation)
pub struct RefreshTokenService {
    pool: PgPool,
}

impl RefreshTokenService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Generate a new refresh token and store its hash in the database
    /// Returns (plain_token, token_id) - plain token should be sent to client
    pub async fn generate_refresh_token(
        &self,
        user_id: Uuid,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<(String, Uuid)> {
        // Generate random token
        let token = Uuid::new_v4().to_string();

        // Hash token before storing (SHA-256)
        let token_hash = hash_token(&token);

        // Calculate expiration
        let expires_at = Utc::now()
            .checked_add_signed(get_refresh_token_expiration())
            .context("Failed to calculate expiration")?;

        // Store in database
        let record = sqlx::query_as::<_, TokenIdResult>(
            r#"
            INSERT INTO refresh_tokens (user_id, token_hash, expires_at, user_agent, ip_address)
            VALUES ($1, $2, $3, $4, $5::inet)
            RETURNING id
            "#,
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .bind(user_agent)
        .bind(ip_address)
        .fetch_one(&self.pool)
        .await
        .context("Failed to store refresh token")?;

        Ok((token, record.id))
    }

    /// Verify refresh token and return user_id if valid
    /// Also updates last_used_at timestamp
    pub async fn verify_refresh_token(&self, token: &str) -> Result<Uuid> {
        let token_hash = hash_token(token);
        let now = Utc::now();

        // Find and update token in one query
        let record = sqlx::query_as::<_, UserIdResult>(
            r#"
            UPDATE refresh_tokens
            SET last_used_at = $1
            WHERE token_hash = $2 AND expires_at > $1
            RETURNING user_id
            "#,
        )
        .bind(now)
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to verify refresh token")?
        .context("Invalid or expired refresh token")?;

        Ok(record.user_id)
    }

    /// Revoke a specific refresh token by its plain value
    pub async fn revoke_refresh_token(&self, token: &str) -> Result<()> {
        let token_hash = hash_token(token);

        let result = sqlx::query("DELETE FROM refresh_tokens WHERE token_hash = $1")
            .bind(token_hash)
            .execute(&self.pool)
            .await
            .context("Failed to revoke refresh token")?;

        if result.rows_affected() == 0 {
            anyhow::bail!("Refresh token not found");
        }

        Ok(())
    }

    /// Revoke all refresh tokens for a specific user
    pub async fn revoke_all_user_tokens(&self, user_id: Uuid) -> Result<u64> {
        let result = sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .context("Failed to revoke user tokens")?;

        Ok(result.rows_affected())
    }

    /// Clean up expired refresh tokens (should be run periodically as background job)
    pub async fn cleanup_expired_tokens(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < NOW()")
            .execute(&self.pool)
            .await
            .context("Failed to cleanup expired refresh tokens")?;

        Ok(result.rows_affected())
    }
}

/// Helper function to hash tokens using SHA-256
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_hashing() {
        let token = "test-token-123";
        let hash1 = hash_token(token);
        let hash2 = hash_token(token);

        // Same input should produce same hash
        assert_eq!(hash1, hash2);

        // Different input should produce different hash
        let hash3 = hash_token("different-token");
        assert_ne!(hash1, hash3);
    }
}
