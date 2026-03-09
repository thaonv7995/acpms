use acpms_db::{models::AttemptStatus, PgPool};
use acpms_executors::{AgentEvent, ApprovalRequestMessage, StatusMessage};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use std::{
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
    time::Instant,
};
use tokio::sync::broadcast;
use tokio::time::{self, MissedTickBehavior};
use uuid::Uuid;

const OPENCLAW_WEBHOOK_SIGNATURE_HEADER: &str = "X-Agentic-Signature";
const OPENCLAW_WEBHOOK_MAX_ATTEMPTS: i32 = 5;
const OPENCLAW_WEBHOOK_BATCH_SIZE: i64 = 25;
const OPENCLAW_WEBHOOK_POLL_INTERVAL_SECS: u64 = 5;
type HmacSha256 = Hmac<Sha256>;

pub trait OpenClawGatewayMetricsObserver: Send + Sync {
    fn on_event_recorded(&self, event_type: &str);
    fn on_webhook_delivery(&self, success: bool, status_code: Option<u16>, duration_seconds: f64);
    fn on_retained_event_rows_changed(&self, total_rows: i64);
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OpenClawGatewayEvent {
    pub sequence_id: i64,
    pub event_type: String,
    pub occurred_at: DateTime<Utc>,
    pub project_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub attempt_id: Option<Uuid>,
    pub source: String,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct NewOpenClawGatewayEvent {
    pub event_type: String,
    pub project_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub attempt_id: Option<Uuid>,
    pub source: String,
    pub payload: Value,
}

#[derive(Debug, Clone)]
struct OpenClawGatewayWebhookConfig {
    url: String,
    secret: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct ClaimedOpenClawWebhookDelivery {
    delivery_id: Uuid,
    attempt_count: i32,
    max_attempts: i32,
    sequence_id: i64,
    event_type: String,
    occurred_at: DateTime<Utc>,
    project_id: Option<Uuid>,
    task_id: Option<Uuid>,
    attempt_id: Option<Uuid>,
    source: String,
    payload: Value,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FailedOpenClawWebhookDelivery {
    pub id: Uuid,
    pub event_sequence_id: i64,
    pub event_type: String,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub last_status_code: Option<i32>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_attempt_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OpenClawWebhookDeliveryStats {
    pub pending: i64,
    pub processing: i64,
    pub completed: i64,
    pub failed: i64,
}

#[derive(Clone)]
pub struct OpenClawGatewayEventService {
    pool: PgPool,
    live_tx: broadcast::Sender<OpenClawGatewayEvent>,
    retention_hours: i64,
    http_client: reqwest::Client,
    webhook: Option<OpenClawGatewayWebhookConfig>,
    metrics_observer: Option<Arc<dyn OpenClawGatewayMetricsObserver>>,
    retained_event_rows: Arc<AtomicI64>,
}

impl OpenClawGatewayEventService {
    pub fn new(pool: PgPool, retention_hours: i64) -> Self {
        let (live_tx, _) = broadcast::channel(512);
        Self {
            pool,
            live_tx,
            retention_hours,
            http_client: reqwest::Client::new(),
            webhook: None,
            metrics_observer: None,
            retained_event_rows: Arc::new(AtomicI64::new(-1)),
        }
    }

    pub fn with_optional_webhook(
        mut self,
        webhook_url: Option<String>,
        webhook_secret: Option<String>,
    ) -> Self {
        let webhook_url = webhook_url
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let webhook_secret = webhook_secret
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        self.webhook = match (webhook_url, webhook_secret) {
            (Some(url), Some(secret)) => Some(OpenClawGatewayWebhookConfig { url, secret }),
            _ => None,
        };
        self
    }

    pub fn with_metrics_observer(
        mut self,
        metrics_observer: Arc<dyn OpenClawGatewayMetricsObserver>,
    ) -> Self {
        self.metrics_observer = Some(metrics_observer);
        self
    }

    pub fn subscribe_live(&self) -> broadcast::Receiver<OpenClawGatewayEvent> {
        self.live_tx.subscribe()
    }

    pub fn retention_hours(&self) -> i64 {
        self.retention_hours
    }

    pub fn webhook_enabled(&self) -> bool {
        self.webhook.is_some()
    }

    pub async fn record_event(
        &self,
        event: NewOpenClawGatewayEvent,
    ) -> Result<OpenClawGatewayEvent> {
        let stored = sqlx::query_as::<_, OpenClawGatewayEvent>(
            r#"
            INSERT INTO openclaw_gateway_events (
                event_type,
                project_id,
                task_id,
                attempt_id,
                source,
                payload
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                sequence_id,
                event_type,
                occurred_at,
                project_id,
                task_id,
                attempt_id,
                source,
                payload
            "#,
        )
        .bind(event.event_type)
        .bind(event.project_id)
        .bind(event.task_id)
        .bind(event.attempt_id)
        .bind(event.source)
        .bind(event.payload)
        .fetch_one(&self.pool)
        .await
        .context("Failed to insert OpenClaw gateway event")?;

        let _ = self.live_tx.send(stored.clone());
        if let Some(observer) = &self.metrics_observer {
            observer.on_event_recorded(&stored.event_type);
        }
        if let Err(error) = self.bump_retained_event_rows(1).await {
            tracing::warn!("Failed to update OpenClaw retained-row metric after insert: {error}");
        }
        if let Err(error) = self
            .enqueue_optional_webhook_delivery(stored.sequence_id)
            .await
        {
            tracing::warn!(
                sequence_id = stored.sequence_id,
                event_type = %stored.event_type,
                error = %error,
                "Failed to queue OpenClaw webhook delivery"
            );
        }
        Ok(stored)
    }

    pub async fn list_events_after(
        &self,
        after_sequence_id: i64,
        limit: i64,
    ) -> Result<Vec<OpenClawGatewayEvent>> {
        sqlx::query_as::<_, OpenClawGatewayEvent>(
            r#"
            SELECT
                sequence_id,
                event_type,
                occurred_at,
                project_id,
                task_id,
                attempt_id,
                source,
                payload
            FROM openclaw_gateway_events
            WHERE sequence_id > $1
            ORDER BY sequence_id ASC
            LIMIT $2
            "#,
        )
        .bind(after_sequence_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list OpenClaw gateway events")
    }

    pub async fn oldest_sequence_id(&self) -> Result<Option<i64>> {
        sqlx::query_scalar("SELECT MIN(sequence_id) FROM openclaw_gateway_events")
            .fetch_one(&self.pool)
            .await
            .context("Failed to read oldest OpenClaw gateway event cursor")
    }

    pub async fn retained_event_row_count(&self) -> Result<i64> {
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM openclaw_gateway_events")
            .fetch_one(&self.pool)
            .await
            .context("Failed to count retained OpenClaw gateway events")
    }

    pub async fn sync_retained_event_row_count_metric(&self) -> Result<i64> {
        let total_rows = self.retained_event_row_count().await?;
        self.retained_event_rows
            .store(total_rows, Ordering::Relaxed);
        if let Some(observer) = &self.metrics_observer {
            observer.on_retained_event_rows_changed(total_rows);
        }
        Ok(total_rows)
    }

    pub async fn cleanup_expired_events(&self) -> Result<u64> {
        let cutoff = Utc::now() - Duration::hours(self.retention_hours);
        let deleted = sqlx::query(
            r#"
            WITH expired AS (
                SELECT sequence_id
                FROM openclaw_gateway_events
                WHERE occurred_at < $1
                ORDER BY sequence_id ASC
                LIMIT 1000
            )
            DELETE FROM openclaw_gateway_events
            WHERE sequence_id IN (SELECT sequence_id FROM expired)
            "#,
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await
        .context("Failed to cleanup expired OpenClaw gateway events")?
        .rows_affected();

        if deleted > 0 {
            if let Err(error) = self.bump_retained_event_rows(-(deleted as i64)).await {
                tracing::warn!(
                    "Failed to update OpenClaw retained-row metric after cleanup: {error}"
                );
            }
        }

        Ok(deleted)
    }

    pub fn spawn_agent_event_bridge(
        self: std::sync::Arc<Self>,
        mut source_rx: broadcast::Receiver<AgentEvent>,
    ) {
        tokio::spawn(async move {
            while let Ok(event) = source_rx.recv().await {
                if let Err(error) = self.handle_agent_event(event).await {
                    tracing::warn!("Failed to bridge OpenClaw event: {}", error);
                }
            }
        });
    }

    pub fn spawn_webhook_delivery_worker(self: Arc<Self>) {
        if !self.webhook_enabled() {
            return;
        }

        tokio::spawn(async move {
            tracing::info!("Starting OpenClaw webhook delivery worker");
            let mut interval = time::interval(std::time::Duration::from_secs(
                OPENCLAW_WEBHOOK_POLL_INTERVAL_SECS,
            ));
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

            loop {
                interval.tick().await;
                if let Err(error) = self
                    .process_pending_webhook_deliveries(OPENCLAW_WEBHOOK_BATCH_SIZE)
                    .await
                {
                    tracing::warn!("Failed to process OpenClaw webhook deliveries: {error}");
                }
            }
        });
    }

    pub async fn process_pending_webhook_deliveries(&self, limit: i64) -> Result<usize> {
        let Some(webhook) = self.webhook.clone() else {
            return Ok(0);
        };

        let deliveries = self.claim_pending_webhook_deliveries(limit).await?;
        if deliveries.is_empty() {
            return Ok(0);
        }

        let mut processed = 0usize;
        for delivery in deliveries {
            let event = OpenClawGatewayEvent {
                sequence_id: delivery.sequence_id,
                event_type: delivery.event_type.clone(),
                occurred_at: delivery.occurred_at,
                project_id: delivery.project_id,
                task_id: delivery.task_id,
                attempt_id: delivery.attempt_id,
                source: delivery.source.clone(),
                payload: delivery.payload.clone(),
            };

            let started_at = Instant::now();
            let (status_code, result) =
                send_webhook_event(self.http_client.clone(), webhook.clone(), event.clone()).await;
            if let Some(observer) = &self.metrics_observer {
                observer.on_webhook_delivery(
                    result.is_ok(),
                    status_code,
                    started_at.elapsed().as_secs_f64(),
                );
            }

            match result {
                Ok(()) => {
                    self.mark_webhook_delivery_completed(delivery.delivery_id, status_code)
                        .await?;
                }
                Err(error) => {
                    if delivery.attempt_count >= delivery.max_attempts {
                        self.mark_webhook_delivery_failed(
                            delivery.delivery_id,
                            delivery.attempt_count,
                            status_code,
                            &error,
                        )
                        .await?;
                    } else {
                        self.mark_webhook_delivery_pending_retry(
                            delivery.delivery_id,
                            delivery.attempt_count,
                            status_code,
                            &error,
                        )
                        .await?;
                    }

                    tracing::warn!(
                        delivery_id = %delivery.delivery_id,
                        sequence_id = event.sequence_id,
                        event_type = %event.event_type,
                        attempt_count = delivery.attempt_count,
                        max_attempts = delivery.max_attempts,
                        error = %error,
                        "Failed to deliver OpenClaw webhook"
                    );
                }
            }

            processed += 1;
        }

        Ok(processed)
    }

    pub async fn get_failed_webhook_deliveries(
        &self,
        limit: i64,
    ) -> Result<Vec<FailedOpenClawWebhookDelivery>> {
        sqlx::query_as::<_, FailedOpenClawWebhookDelivery>(
            r#"
            SELECT
                d.id,
                d.event_sequence_id,
                e.event_type,
                d.attempt_count,
                d.max_attempts,
                d.last_status_code,
                d.last_error,
                d.created_at,
                d.last_attempt_at
            FROM openclaw_webhook_deliveries d
            JOIN openclaw_gateway_events e ON e.sequence_id = d.event_sequence_id
            WHERE d.status = 'failed'
            ORDER BY d.last_attempt_at DESC NULLS LAST, d.created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await
        .context("Failed to load failed OpenClaw webhook deliveries")
    }

    pub async fn retry_failed_webhook_delivery(&self, delivery_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE openclaw_webhook_deliveries
            SET
                status = 'pending',
                attempt_count = 0,
                next_attempt_at = NOW(),
                last_error = NULL,
                last_status_code = NULL
            WHERE id = $1 AND status = 'failed'
            "#,
        )
        .bind(delivery_id)
        .execute(&self.pool)
        .await
        .context("Failed to reset OpenClaw webhook delivery for retry")?;
        self.kick_optional_webhook_delivery_worker();
        Ok(())
    }

    pub async fn webhook_delivery_stats(&self) -> Result<OpenClawWebhookDeliveryStats> {
        sqlx::query_as::<_, OpenClawWebhookDeliveryStats>(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status = 'pending') AS pending,
                COUNT(*) FILTER (WHERE status = 'processing') AS processing,
                COUNT(*) FILTER (WHERE status = 'completed') AS completed,
                COUNT(*) FILTER (WHERE status = 'failed') AS failed
            FROM openclaw_webhook_deliveries
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to load OpenClaw webhook delivery stats")
    }

    async fn handle_agent_event(&self, event: AgentEvent) -> Result<()> {
        match event {
            AgentEvent::Status(status) => {
                if let Some(event) = self.map_status_event(status).await? {
                    self.record_event(event).await?;
                }
            }
            AgentEvent::ApprovalRequest(approval) => {
                let event = self.map_approval_event(approval).await?;
                self.record_event(event).await?;
            }
            AgentEvent::Log(_) | AgentEvent::UserMessage(_) | AgentEvent::AssistantLog(_) => {}
        }

        Ok(())
    }

    async fn map_status_event(
        &self,
        status: StatusMessage,
    ) -> Result<Option<NewOpenClawGatewayEvent>> {
        let refs = self
            .load_attempt_refs(status.attempt_id)
            .await?
            .context("Attempt event referenced unknown attempt")?;

        let (event_type, payload) = match status.status {
            AttemptStatus::Queued => return Ok(None),
            AttemptStatus::Running => (
                "attempt.started",
                serde_json::json!({
                    "status": "running"
                }),
            ),
            AttemptStatus::Success => (
                "attempt.completed",
                serde_json::json!({
                    "status": "success"
                }),
            ),
            AttemptStatus::Failed => (
                "attempt.failed",
                serde_json::json!({
                    "status": "failed",
                    "error_message": refs.error_message
                }),
            ),
            AttemptStatus::Cancelled => (
                "attempt.cancelled",
                serde_json::json!({
                    "status": "cancelled"
                }),
            ),
        };

        Ok(Some(NewOpenClawGatewayEvent {
            event_type: event_type.to_string(),
            project_id: Some(refs.project_id),
            task_id: Some(refs.task_id),
            attempt_id: Some(status.attempt_id),
            source: "agent_event.status".to_string(),
            payload,
        }))
    }

    async fn map_approval_event(
        &self,
        approval: ApprovalRequestMessage,
    ) -> Result<NewOpenClawGatewayEvent> {
        let refs = self
            .load_attempt_refs(approval.attempt_id)
            .await?
            .context("Approval event referenced unknown attempt")?;

        Ok(NewOpenClawGatewayEvent {
            event_type: "attempt.needs_input".to_string(),
            project_id: Some(refs.project_id),
            task_id: Some(refs.task_id),
            attempt_id: Some(approval.attempt_id),
            source: "agent_event.approval_request".to_string(),
            payload: serde_json::json!({
                "tool_use_id": approval.tool_use_id,
                "tool_name": approval.tool_name,
                "tool_input": approval.tool_input
            }),
        })
    }

    async fn load_attempt_refs(&self, attempt_id: Uuid) -> Result<Option<AttemptRefs>> {
        sqlx::query_as::<_, AttemptRefs>(
            r#"
            SELECT
                t.project_id,
                ta.task_id,
                ta.error_message
            FROM task_attempts ta
            JOIN tasks t ON t.id = ta.task_id
            WHERE ta.id = $1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to load attempt references for OpenClaw gateway event")
    }

    pub async fn record_task_status_changed(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        previous_status: &str,
        new_status: &str,
        source: &str,
    ) -> Result<OpenClawGatewayEvent> {
        self.record_event(NewOpenClawGatewayEvent {
            event_type: "task.status_changed".to_string(),
            project_id: Some(project_id),
            task_id: Some(task_id),
            attempt_id: None,
            source: source.to_string(),
            payload: serde_json::json!({
                "previous_status": previous_status,
                "new_status": new_status
            }),
        })
        .await
    }

    async fn enqueue_optional_webhook_delivery(&self, event_sequence_id: i64) -> Result<()> {
        if !self.webhook_enabled() {
            return Ok(());
        }

        sqlx::query(
            r#"
            INSERT INTO openclaw_webhook_deliveries (
                event_sequence_id,
                status,
                attempt_count,
                max_attempts,
                next_attempt_at
            )
            VALUES ($1, 'pending', 0, $2, NOW())
            ON CONFLICT (event_sequence_id) DO NOTHING
            "#,
        )
        .bind(event_sequence_id)
        .bind(OPENCLAW_WEBHOOK_MAX_ATTEMPTS)
        .execute(&self.pool)
        .await
        .context("Failed to queue OpenClaw webhook delivery")?;

        self.kick_optional_webhook_delivery_worker();
        Ok(())
    }

    fn kick_optional_webhook_delivery_worker(&self) {
        if !self.webhook_enabled() {
            return;
        }

        let service = self.clone();
        tokio::spawn(async move {
            if let Err(error) = service.process_pending_webhook_deliveries(1).await {
                tracing::warn!("Failed to kick OpenClaw webhook delivery worker: {error}");
            }
        });
    }

    async fn bump_retained_event_rows(&self, delta: i64) -> Result<()> {
        let current = self.retained_event_rows.load(Ordering::Relaxed);
        if current < 0 {
            self.sync_retained_event_row_count_metric().await?;
            return Ok(());
        }

        let next = (current + delta).max(0);
        self.retained_event_rows.store(next, Ordering::Relaxed);
        if let Some(observer) = &self.metrics_observer {
            observer.on_retained_event_rows_changed(next);
        }
        Ok(())
    }

    async fn claim_pending_webhook_deliveries(
        &self,
        limit: i64,
    ) -> Result<Vec<ClaimedOpenClawWebhookDelivery>> {
        sqlx::query_as::<_, ClaimedOpenClawWebhookDelivery>(
            r#"
            WITH claimed AS (
                SELECT d.id
                FROM openclaw_webhook_deliveries d
                WHERE d.status = 'pending'
                  AND d.next_attempt_at <= NOW()
                ORDER BY d.next_attempt_at ASC, d.created_at ASC
                FOR UPDATE SKIP LOCKED
                LIMIT $1
            ),
            updated AS (
                UPDATE openclaw_webhook_deliveries d
                SET
                    status = 'processing',
                    attempt_count = d.attempt_count + 1,
                    last_attempt_at = NOW()
                FROM claimed
                WHERE d.id = claimed.id
                RETURNING
                    d.id AS delivery_id,
                    d.attempt_count,
                    d.max_attempts,
                    d.event_sequence_id
            )
            SELECT
                updated.delivery_id,
                updated.attempt_count,
                updated.max_attempts,
                e.sequence_id,
                e.event_type,
                e.occurred_at,
                e.project_id,
                e.task_id,
                e.attempt_id,
                e.source,
                e.payload
            FROM updated
            JOIN openclaw_gateway_events e ON e.sequence_id = updated.event_sequence_id
            ORDER BY e.sequence_id ASC
            "#,
        )
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await
        .context("Failed to claim pending OpenClaw webhook deliveries")
    }

    async fn mark_webhook_delivery_completed(
        &self,
        delivery_id: Uuid,
        status_code: Option<u16>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE openclaw_webhook_deliveries
            SET
                status = 'completed',
                last_status_code = $2,
                completed_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(delivery_id)
        .bind(status_code.map(i32::from))
        .execute(&self.pool)
        .await
        .context("Failed to mark OpenClaw webhook delivery as completed")?;
        Ok(())
    }

    async fn mark_webhook_delivery_failed(
        &self,
        delivery_id: Uuid,
        attempt_count: i32,
        status_code: Option<u16>,
        error: &anyhow::Error,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE openclaw_webhook_deliveries
            SET
                status = 'failed',
                attempt_count = $2,
                last_status_code = $3,
                last_error = $4
            WHERE id = $1
            "#,
        )
        .bind(delivery_id)
        .bind(attempt_count)
        .bind(status_code.map(i32::from))
        .bind(error.to_string())
        .execute(&self.pool)
        .await
        .context("Failed to mark OpenClaw webhook delivery as failed")?;
        Ok(())
    }

    async fn mark_webhook_delivery_pending_retry(
        &self,
        delivery_id: Uuid,
        attempt_count: i32,
        status_code: Option<u16>,
        error: &anyhow::Error,
    ) -> Result<()> {
        let retry_delay_seconds = 2_i64.pow((attempt_count.saturating_sub(1)).min(5) as u32);
        let next_attempt_at = Utc::now() + Duration::seconds(retry_delay_seconds.max(1));

        sqlx::query(
            r#"
            UPDATE openclaw_webhook_deliveries
            SET
                status = 'pending',
                attempt_count = $2,
                next_attempt_at = $3,
                last_status_code = $4,
                last_error = $5
            WHERE id = $1
            "#,
        )
        .bind(delivery_id)
        .bind(attempt_count)
        .bind(next_attempt_at)
        .bind(status_code.map(i32::from))
        .bind(error.to_string())
        .execute(&self.pool)
        .await
        .context("Failed to requeue OpenClaw webhook delivery")?;
        Ok(())
    }
}

fn build_webhook_signature(secret: &str, payload: &[u8]) -> Result<String> {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).context("Invalid OpenClaw webhook secret")?;
    mac.update(payload);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

async fn send_webhook_event(
    client: reqwest::Client,
    webhook: OpenClawGatewayWebhookConfig,
    event: OpenClawGatewayEvent,
) -> (Option<u16>, Result<()>) {
    let payload = match serde_json::to_vec(&event).context("Failed to serialize OpenClaw webhook") {
        Ok(payload) => payload,
        Err(error) => return (None, Err(error)),
    };
    let signature = match build_webhook_signature(&webhook.secret, &payload) {
        Ok(signature) => signature,
        Err(error) => return (None, Err(error)),
    };

    let response = client
        .post(&webhook.url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(OPENCLAW_WEBHOOK_SIGNATURE_HEADER, signature)
        .header("X-Agentic-Event-Id", event.sequence_id.to_string())
        .header("X-Agentic-Event-Type", &event.event_type)
        .body(payload)
        .send()
        .await
        .context("Failed to send OpenClaw webhook");

    let response = match response {
        Ok(response) => response,
        Err(error) => return (None, Err(error)),
    };

    let status_code = response.status().as_u16();

    if !response.status().is_success() {
        return (
            Some(status_code),
            Err(anyhow::anyhow!(
                "OpenClaw webhook returned non-success status {}",
                response.status()
            )),
        );
    }

    (Some(status_code), Ok(()))
}

#[derive(Debug, sqlx::FromRow)]
struct AttemptRefs {
    project_id: Uuid,
    task_id: Uuid,
    error_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::build_webhook_signature;

    #[test]
    fn build_webhook_signature_matches_expected_hex() {
        let signature = build_webhook_signature("secret", br#"{"hello":"world"}"#)
            .expect("signature should be generated");

        assert_eq!(
            signature,
            "2677ad3e7c090b2fa2c0fb13020d66d5420879b8316eb356a2d60fb9073bc778"
        );
    }
}
