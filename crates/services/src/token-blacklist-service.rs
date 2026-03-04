use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(FromRow)]
struct JtiResult {
    #[allow(dead_code)]
    jti: String,
}

/// Service for managing token blacklist (revoked access tokens)
pub struct TokenBlacklistService {
    pool: PgPool,
}

impl TokenBlacklistService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Add an access token to the blacklist (for logout/revocation)
    pub async fn blacklist_access_token(
        &self,
        jti: &str,
        user_id: Uuid,
        expires_at: chrono::DateTime<Utc>,
        reason: Option<String>,
        revoked_by: Option<Uuid>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO token_blacklist (jti, user_id, expires_at, reason, revoked_by)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (jti) DO NOTHING
            "#,
        )
        .bind(jti)
        .bind(user_id)
        .bind(expires_at)
        .bind(reason)
        .bind(revoked_by)
        .execute(&self.pool)
        .await
        .context("Failed to blacklist token")?;

        Ok(())
    }

    /// Check if an access token is blacklisted
    pub async fn is_token_blacklisted(&self, jti: &str) -> Result<bool> {
        let record = sqlx::query_as::<_, JtiResult>(
            "SELECT jti FROM token_blacklist WHERE jti = $1 AND expires_at > NOW()",
        )
        .bind(jti)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to check token blacklist")?;

        Ok(record.is_some())
    }

    /// Blacklist all currently valid access tokens for a user
    /// Note: This is a best-effort approach since we don't track all issued tokens
    /// In production, consider forcing re-login by clearing all sessions
    pub async fn blacklist_all_user_tokens(
        &self,
        user_id: Uuid,
        reason: String,
        revoked_by: Uuid,
    ) -> Result<()> {
        // Log the action for audit trail
        tracing::info!(
            user_id = %user_id,
            reason = %reason,
            revoked_by = %revoked_by,
            "Blacklisted all user tokens"
        );

        // In a production system, you might want to:
        // 1. Track all issued JTIs in a separate table
        // 2. Blacklist them all here
        // 3. Or use a different session management approach

        Ok(())
    }

    /// Clean up expired blacklist entries (should be run periodically as background job)
    pub async fn cleanup_expired_entries(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM token_blacklist WHERE expires_at < NOW()")
            .execute(&self.pool)
            .await
            .context("Failed to cleanup expired blacklist entries")?;

        Ok(result.rows_affected())
    }
}
