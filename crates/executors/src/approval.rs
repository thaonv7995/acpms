//! Tool approval service for managing permission requests from Claude agents.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;
use uuid::Uuid;

/// Auto-approve callback ID constant (Phase 6: vibe-kanban alignment)
pub const AUTO_APPROVE_CALLBACK_ID: &str = "auto-approve-internal-bypass";

/// Approval status for tool execution
/// Note: Database enum only has 4 values (pending, approved, denied, timed_out).
/// The denial reason is stored separately in denied_reason column.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied {
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    TimedOut,
}

/// Database representation (enum only, no fields)
#[derive(Debug, Clone, Copy, sqlx::Type)]
#[sqlx(type_name = "approval_status", rename_all = "snake_case")]
enum DbApprovalStatus {
    Pending,
    Approved,
    Denied,
    TimedOut,
}

/// Approval request event (broadcast to frontend)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub attempt_id: Uuid,
    pub tool_use_id: String,
    pub tool_name: String,
    pub tool_input: Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Trait for approval services
#[async_trait::async_trait]
pub trait ApprovalService: Send + Sync {
    /// Request approval for tool execution
    ///
    /// This inserts a pending approval into the database and broadcasts
    /// the request to connected clients. It then polls the database until
    /// the approval is resolved (approved/denied) or times out.
    async fn request_tool_approval(
        &self,
        attempt_id: Uuid,
        tool_name: &str,
        tool_input: Value,
        tool_use_id: &str,
    ) -> Result<ApprovalStatus>;

    /// Check approval status (for polling)
    async fn check_approval_status(&self, tool_use_id: &str) -> Result<Option<ApprovalStatus>>;
}

/// Database-backed approval service with polling
pub struct DatabaseApprovalService {
    db_pool: PgPool,
    broadcast_tx: broadcast::Sender<crate::AgentEvent>,
    timeout: Duration,
    poll_interval: Duration,
}

impl DatabaseApprovalService {
    /// Create new approval service
    pub fn new(db_pool: PgPool, broadcast_tx: broadcast::Sender<crate::AgentEvent>) -> Arc<Self> {
        Arc::new(Self {
            db_pool,
            broadcast_tx,
            timeout: Duration::from_secs(300), // 5 minutes default
            poll_interval: Duration::from_millis(500), // Poll every 500ms
        })
    }

    /// Create with custom timeout
    pub fn with_timeout(
        db_pool: PgPool,
        broadcast_tx: broadcast::Sender<crate::AgentEvent>,
        timeout: Duration,
    ) -> Arc<Self> {
        Arc::new(Self {
            db_pool,
            broadcast_tx,
            timeout,
            poll_interval: Duration::from_millis(500),
        })
    }

    /// Security-hardened approval validation (Phase 6)
    /// Verifies AUTO_APPROVE_CALLBACK_ID against database metadata
    #[allow(dead_code)]
    async fn validate_auto_approve(
        &self,
        attempt_id: Uuid,
        callback_id: &str,
        tool_name: &str,
    ) -> Result<bool> {
        // Only check if callback matches constant
        if callback_id != AUTO_APPROVE_CALLBACK_ID {
            return Ok(false); // Not auto-approve
        }

        // Layer 1: Verify attempt configuration (source of truth)
        let metadata = sqlx::query_scalar::<_, Option<serde_json::Value>>(
            "SELECT metadata FROM task_attempts WHERE id = $1",
        )
        .bind(attempt_id)
        .fetch_optional(&self.db_pool)
        .await?
        .flatten();

        let auto_approve_enabled = metadata
            .as_ref()
            .and_then(|m| m.get("auto_approve"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !auto_approve_enabled {
            // SECURITY: Reject spoofed auto-approve
            tracing::warn!(
                attempt_id = %attempt_id,
                tool_name = %tool_name,
                "Rejected spoofed AUTO_APPROVE_CALLBACK_ID"
            );

            // Log security event
            self.log_security_event(
                attempt_id,
                "auto_approve_spoofing_attempt",
                serde_json::json!({
                    "callback_id": callback_id,
                    "tool_name": tool_name,
                    "auto_approve_enabled": false,
                }),
            )
            .await?;

            return Ok(false);
        }

        // Layer 2: Tool whitelist (optional)
        if let Some(whitelist) = metadata.as_ref().and_then(|m| m.get("auto_approve_tools")) {
            let allowed_tools: Vec<String> =
                serde_json::from_value(whitelist.clone()).unwrap_or_default();

            if !allowed_tools.is_empty() && !allowed_tools.contains(&tool_name.to_string()) {
                tracing::warn!(
                    attempt_id = %attempt_id,
                    tool_name = %tool_name,
                    "Tool not in auto-approve whitelist"
                );
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Log security event for audit trail
    #[allow(dead_code)]
    async fn log_security_event(
        &self,
        attempt_id: Uuid,
        event_type: &str,
        details: serde_json::Value,
    ) -> Result<()> {
        let _ = sqlx::query(
            r#"INSERT INTO security_events (attempt_id, event_type, details)
               VALUES ($1, $2, $3)"#,
        )
        .bind(attempt_id)
        .bind(event_type)
        .bind(details)
        .execute(&self.db_pool)
        .await;

        Ok(())
    }

    async fn resolve_latest_execution_process_id(&self, attempt_id: Uuid) -> Result<Option<Uuid>> {
        let process_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id
            FROM execution_processes
            WHERE attempt_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db_pool)
        .await
        .context("Failed to resolve execution process for approval request")?;

        Ok(process_id)
    }

    async fn resolve_execution_process_id_with_retry(
        &self,
        attempt_id: Uuid,
    ) -> Result<Option<Uuid>> {
        const MAX_ATTEMPTS: usize = 6;
        const RETRY_DELAY: Duration = Duration::from_millis(100);

        for attempt_index in 0..MAX_ATTEMPTS {
            if let Some(process_id) = self.resolve_latest_execution_process_id(attempt_id).await? {
                return Ok(Some(process_id));
            }

            if attempt_index + 1 < MAX_ATTEMPTS {
                sleep(RETRY_DELAY).await;
            }
        }

        Ok(None)
    }
}

#[async_trait::async_trait]
impl ApprovalService for DatabaseApprovalService {
    async fn request_tool_approval(
        &self,
        attempt_id: Uuid,
        tool_name: &str,
        tool_input: Value,
        tool_use_id: &str,
    ) -> Result<ApprovalStatus> {
        let execution_process_id = self
            .resolve_execution_process_id_with_retry(attempt_id)
            .await?;
        if execution_process_id.is_none() {
            tracing::warn!(
                attempt_id = %attempt_id,
                tool_use_id = %tool_use_id,
                "No execution process found when creating tool approval; storing without process scope"
            );
        }

        // Clone tool_input before binding (bind moves ownership)
        let tool_input_for_broadcast = tool_input.clone();

        // Insert pending approval into database
        // Note: Using sqlx::query instead of query! to avoid compile-time schema validation
        sqlx::query(
            r#"
            INSERT INTO tool_approvals (
                attempt_id,
                execution_process_id,
                tool_use_id,
                tool_name,
                tool_input,
                status
            )
            VALUES ($1, $2, $3, $4, $5, 'pending'::approval_status)
            ON CONFLICT (tool_use_id) DO NOTHING
            "#,
        )
        .bind(attempt_id)
        .bind(execution_process_id)
        .bind(tool_use_id)
        .bind(tool_name)
        .bind(tool_input)
        .execute(&self.db_pool)
        .await
        .context("Failed to insert approval request")?;

        // Broadcast to frontend via AgentEvent
        let _ = self.broadcast_tx.send(crate::AgentEvent::ApprovalRequest(
            crate::ApprovalRequestMessage {
                attempt_id,
                tool_use_id: tool_use_id.to_string(),
                tool_name: tool_name.to_string(),
                tool_input: tool_input_for_broadcast,
                timestamp: chrono::Utc::now(),
            },
        ));

        tracing::info!(
            tool_use_id = %tool_use_id,
            tool_name = %tool_name,
            "Tool approval requested, waiting for user response"
        );

        // Poll database for approval status
        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > self.timeout {
                // Timeout - mark as timed out
                sqlx::query(
                    r#"
                    UPDATE tool_approvals
                    SET status = 'timed_out'::approval_status, responded_at = NOW()
                    WHERE tool_use_id = $1 AND status = 'pending'::approval_status
                    "#,
                )
                .bind(tool_use_id)
                .execute(&self.db_pool)
                .await?;

                tracing::warn!(
                    tool_use_id = %tool_use_id,
                    elapsed_secs = start.elapsed().as_secs(),
                    "Tool approval timed out"
                );

                return Ok(ApprovalStatus::TimedOut);
            }

            // Check status
            if let Some(status) = self.check_approval_status(tool_use_id).await? {
                match status {
                    ApprovalStatus::Pending => {
                        // Still pending - continue polling
                    }
                    ApprovalStatus::Approved => {
                        tracing::info!(
                            tool_use_id = %tool_use_id,
                            elapsed_secs = start.elapsed().as_secs(),
                            "Tool approval granted"
                        );
                        return Ok(ApprovalStatus::Approved);
                    }
                    ApprovalStatus::Denied { reason } => {
                        tracing::info!(
                            tool_use_id = %tool_use_id,
                            reason = ?reason,
                            "Tool approval denied"
                        );
                        return Ok(ApprovalStatus::Denied { reason });
                    }
                    ApprovalStatus::TimedOut => {
                        return Ok(ApprovalStatus::TimedOut);
                    }
                }
            }

            // Sleep before next poll
            tokio::time::sleep(self.poll_interval).await;
        }
    }

    async fn check_approval_status(&self, tool_use_id: &str) -> Result<Option<ApprovalStatus>> {
        let row: Option<(DbApprovalStatus, Option<String>)> = sqlx::query_as(
            r#"
            SELECT status, denied_reason
            FROM tool_approvals
            WHERE tool_use_id = $1
            "#,
        )
        .bind(tool_use_id)
        .fetch_optional(&self.db_pool)
        .await
        .context("Failed to fetch approval status")?;

        Ok(row.map(|(status, denied_reason)| match status {
            DbApprovalStatus::Pending => ApprovalStatus::Pending,
            DbApprovalStatus::Approved => ApprovalStatus::Approved,
            DbApprovalStatus::Denied => ApprovalStatus::Denied {
                reason: denied_reason,
            },
            DbApprovalStatus::TimedOut => ApprovalStatus::TimedOut,
        }))
    }
}
