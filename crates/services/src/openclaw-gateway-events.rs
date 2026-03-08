use acpms_db::{models::AttemptStatus, PgPool};
use acpms_executors::{AgentEvent, ApprovalRequestMessage, StatusMessage};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use tokio::sync::broadcast;
use uuid::Uuid;

const OPENCLAW_WEBHOOK_SIGNATURE_HEADER: &str = "X-Agentic-Signature";
type HmacSha256 = Hmac<Sha256>;

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

#[derive(Clone)]
pub struct OpenClawGatewayEventService {
    pool: PgPool,
    live_tx: broadcast::Sender<OpenClawGatewayEvent>,
    retention_hours: i64,
    http_client: reqwest::Client,
    webhook: Option<OpenClawGatewayWebhookConfig>,
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

    pub fn subscribe_live(&self) -> broadcast::Receiver<OpenClawGatewayEvent> {
        self.live_tx.subscribe()
    }

    pub fn retention_hours(&self) -> i64 {
        self.retention_hours
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
        self.dispatch_optional_webhook(stored.clone());
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

    fn dispatch_optional_webhook(&self, event: OpenClawGatewayEvent) {
        let Some(webhook) = self.webhook.clone() else {
            return;
        };

        let client = self.http_client.clone();
        tokio::spawn(async move {
            if let Err(error) = send_webhook_event(client, webhook, event.clone()).await {
                tracing::warn!(
                    sequence_id = event.sequence_id,
                    event_type = %event.event_type,
                    error = %error,
                    "Failed to deliver OpenClaw webhook"
                );
            }
        });
    }
}

fn build_webhook_signature(secret: &str, payload: &[u8]) -> Result<String> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .context("Invalid OpenClaw webhook secret")?;
    mac.update(payload);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

async fn send_webhook_event(
    client: reqwest::Client,
    webhook: OpenClawGatewayWebhookConfig,
    event: OpenClawGatewayEvent,
) -> Result<()> {
    let payload = serde_json::to_vec(&event).context("Failed to serialize OpenClaw webhook")?;
    let signature = build_webhook_signature(&webhook.secret, &payload)?;

    let response = client
        .post(&webhook.url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(OPENCLAW_WEBHOOK_SIGNATURE_HEADER, signature)
        .header("X-Agentic-Event-Id", event.sequence_id.to_string())
        .header("X-Agentic-Event-Type", &event.event_type)
        .body(payload)
        .send()
        .await
        .context("Failed to send OpenClaw webhook")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "OpenClaw webhook returned non-success status {}",
            response.status()
        );
    }

    Ok(())
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
