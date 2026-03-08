use crate::webhook_event_handlers::WebhookEventHandlers;
use crate::OpenClawGatewayEventService;
use acpms_executors::webhook_job::WebhookJob;
use anyhow::{Context, Result};
use sqlx::{FromRow, PgPool};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

#[derive(FromRow)]
struct EventIdResult {
    id: Uuid,
}

/// Webhook manager for processing GitLab webhook events asynchronously
#[derive(Clone)]
pub struct WebhookManager {
    db: PgPool,
    handlers: WebhookEventHandlers,
}

impl WebhookManager {
    pub fn new(db: PgPool) -> Self {
        Self {
            handlers: WebhookEventHandlers::new(db.clone()),
            db,
        }
    }

    pub fn with_openclaw_events(
        mut self,
        openclaw_event_service: Arc<OpenClawGatewayEventService>,
    ) -> Self {
        self.handlers = self.handlers.with_openclaw_events(openclaw_event_service);
        self
    }

    /// Queue a webhook event for asynchronous processing
    ///
    /// ## Deduplication
    /// Uses event_id to prevent duplicate processing of the same GitLab event
    ///
    /// ## Returns
    /// - Ok(webhook_event_id) if queued successfully
    /// - Err if event already exists (idempotent - returns existing ID)
    pub async fn queue_event(
        &self,
        project_id: Uuid,
        event_id: String,
        event_type: String,
        payload: serde_json::Value,
    ) -> Result<Uuid> {
        // Check for existing event (deduplication)
        let existing = sqlx::query_as::<_, EventIdResult>(
            "SELECT id FROM webhook_events WHERE project_id = $1 AND event_id = $2",
        )
        .bind(project_id)
        .bind(&event_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to check for existing webhook event")?;

        if let Some(record) = existing {
            return Ok(record.id); // Idempotent
        }

        // Insert new event
        let webhook_event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO webhook_events
            (project_id, event_id, event_type, payload, status, attempt_count)
            VALUES ($1, $2, $3, $4, 'pending', 0)
            RETURNING id
            "#,
        )
        .bind(project_id)
        .bind(event_id)
        .bind(event_type)
        .bind(payload)
        .fetch_one(&self.db)
        .await
        .context("Failed to queue webhook event")?;

        Ok(webhook_event_id)
    }

    /// Process a webhook job with retry logic
    ///
    /// ## Retry Strategy
    /// - Max 3 attempts
    /// - Exponential backoff: 1s, 2s, 4s
    /// - Failures moved to dead letter queue (status=failed)
    pub async fn process_job(&self, job: WebhookJob) -> Result<()> {
        let max_attempts = 3;
        let mut current_attempt = job.attempt;

        loop {
            self.mark_processing(job.webhook_event_id, current_attempt)
                .await?;

            match self.handle_event(&job).await {
                Ok(_) => {
                    self.mark_completed(job.webhook_event_id).await?;
                    return Ok(());
                }
                Err(e) => {
                    current_attempt += 1;

                    if current_attempt >= max_attempts {
                        self.mark_failed(job.webhook_event_id, current_attempt, &e)
                            .await?;
                        return Err(e);
                    }

                    let delay_ms = 1000 * 2_u64.pow(current_attempt - 1);
                    let delay = Duration::from_millis(delay_ms);

                    self.mark_pending_retry(job.webhook_event_id, &e).await?;

                    tracing::warn!(
                        "Webhook event {} failed (attempt {}/{}), retrying in {:?}: {}",
                        job.webhook_event_id,
                        current_attempt,
                        max_attempts,
                        delay,
                        e
                    );

                    sleep(delay).await;
                }
            }
        }
    }

    /// Dispatch webhook event to appropriate handler
    async fn handle_event(&self, job: &WebhookJob) -> Result<()> {
        match job.event_type.as_str() {
            "push" => self.handlers.handle_push(job).await,
            "merge_request" => self.handlers.handle_merge_request(job).await,
            "pipeline" => self.handlers.handle_pipeline(job).await,
            _ => {
                tracing::warn!("Unknown webhook event type: {}", job.event_type);
                Ok(())
            }
        }
    }

    async fn mark_processing(&self, event_id: Uuid, attempt: u32) -> Result<()> {
        sqlx::query(
            "UPDATE webhook_events SET status = 'processing', attempt_count = $1, last_attempt_at = NOW() WHERE id = $2"
        )
        .bind(attempt as i32)
        .bind(event_id)
        .execute(&self.db)
        .await
        .context("Failed to update webhook event status")
        .map(|_| ())
    }

    async fn mark_completed(&self, event_id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE webhook_events SET status = 'completed', completed_at = NOW() WHERE id = $1",
        )
        .bind(event_id)
        .execute(&self.db)
        .await
        .context("Failed to mark webhook event as completed")
        .map(|_| ())
    }

    async fn mark_failed(&self, event_id: Uuid, attempt: u32, error: &anyhow::Error) -> Result<()> {
        sqlx::query(
            "UPDATE webhook_events SET status = 'failed', last_error = $1, attempt_count = $2 WHERE id = $3"
        )
        .bind(error.to_string())
        .bind(attempt as i32)
        .bind(event_id)
        .execute(&self.db)
        .await
        .context("Failed to mark webhook event as failed")
        .map(|_| ())
    }

    async fn mark_pending_retry(&self, event_id: Uuid, error: &anyhow::Error) -> Result<()> {
        sqlx::query("UPDATE webhook_events SET status = 'pending', last_error = $1 WHERE id = $2")
            .bind(error.to_string())
            .bind(event_id)
            .execute(&self.db)
            .await
            .context("Failed to update webhook event for retry")
            .map(|_| ())
    }
}
