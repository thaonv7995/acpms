use anyhow::{Context, Result};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Admin operations for webhook management (dead letter queue, retry)
#[derive(Clone)]
pub struct WebhookAdminService {
    db: PgPool,
}

impl WebhookAdminService {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Get failed webhook events (dead letter queue)
    pub async fn get_failed_events(
        &self,
        project_id: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<FailedWebhookEvent>> {
        let events = if let Some(pid) = project_id {
            sqlx::query_as::<_, FailedWebhookEvent>(
                r#"
                SELECT id, project_id, event_id, event_type, payload,
                       attempt_count, last_error, created_at, last_attempt_at
                FROM webhook_events
                WHERE status = 'failed' AND project_id = $1
                ORDER BY last_attempt_at DESC
                LIMIT $2
                "#,
            )
            .bind(pid)
            .bind(limit)
            .fetch_all(&self.db)
            .await
        } else {
            sqlx::query_as::<_, FailedWebhookEvent>(
                r#"
                SELECT id, project_id, event_id, event_type, payload,
                       attempt_count, last_error, created_at, last_attempt_at
                FROM webhook_events
                WHERE status = 'failed'
                ORDER BY last_attempt_at DESC
                LIMIT $1
                "#,
            )
            .bind(limit)
            .fetch_all(&self.db)
            .await
        };

        events.context("Failed to fetch failed webhook events")
    }

    /// Retry a failed webhook event
    pub async fn retry_event(&self, event_id: Uuid) -> Result<()> {
        // Reset status and attempt count
        sqlx::query(
            r#"
            UPDATE webhook_events
            SET status = 'pending',
                attempt_count = 0,
                last_error = NULL
            WHERE id = $1 AND status = 'failed'
            "#,
        )
        .bind(event_id)
        .execute(&self.db)
        .await
        .context("Failed to reset webhook event for retry")?;

        tracing::info!("Manually retrying webhook event {}", event_id);

        Ok(())
    }

    /// Get webhook event statistics
    pub async fn get_stats(&self, project_id: Option<Uuid>) -> Result<WebhookStats> {
        let stats = if let Some(pid) = project_id {
            sqlx::query_as::<_, WebhookStats>(
                r#"
                SELECT
                    COUNT(*) FILTER (WHERE status = 'pending') as pending,
                    COUNT(*) FILTER (WHERE status = 'processing') as processing,
                    COUNT(*) FILTER (WHERE status = 'completed') as completed,
                    COUNT(*) FILTER (WHERE status = 'failed') as failed
                FROM webhook_events
                WHERE project_id = $1
                "#,
            )
            .bind(pid)
            .fetch_one(&self.db)
            .await
        } else {
            sqlx::query_as::<_, WebhookStats>(
                r#"
                SELECT
                    COUNT(*) FILTER (WHERE status = 'pending') as pending,
                    COUNT(*) FILTER (WHERE status = 'processing') as processing,
                    COUNT(*) FILTER (WHERE status = 'completed') as completed,
                    COUNT(*) FILTER (WHERE status = 'failed') as failed
                FROM webhook_events
                "#,
            )
            .fetch_one(&self.db)
            .await
        };

        stats.context("Failed to fetch webhook statistics")
    }

    /// Delete old completed webhook events (cleanup)
    pub async fn cleanup_old_events(&self, days: i32) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM webhook_events
            WHERE status = 'completed'
            AND completed_at < NOW() - INTERVAL '1 day' * $1
            "#,
        )
        .bind(days)
        .execute(&self.db)
        .await
        .context("Failed to cleanup old webhook events")?;

        Ok(result.rows_affected())
    }
}

/// Failed webhook event for admin panel
#[derive(Debug, Clone, FromRow)]
pub struct FailedWebhookEvent {
    pub id: Uuid,
    pub project_id: Uuid,
    pub event_id: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub attempt_count: i32,
    pub last_error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_attempt_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Webhook processing statistics
#[derive(Debug, Clone, FromRow)]
pub struct WebhookStats {
    pub pending: i64,
    pub processing: i64,
    pub completed: i64,
    pub failed: i64,
}
