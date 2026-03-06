use crate::agent_log_buffer::{append_log_to_jsonl, buffer_agent_log, flush_agent_log_buffer};
use crate::sdk_normalized_types::{
    ActionType as SdkActionType, NormalizedEntry as SdkNormalizedEntry,
    NormalizedEntryType as SdkNormalizedEntryType, ToolStatus as SdkToolStatus,
};
use crate::validate_sdk_normalized_entry;
use crate::{AgentEvent, LogMessage, StatusMessage};
use acpms_db::models::AttemptStatus;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::collections::HashMap;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tracing::warn;
use uuid::Uuid;

// ============================================================================
// VK-like normalization for timeline: assistant_message + tool_use
// ============================================================================

static TOOL_START_RE: Lazy<Option<Regex>> =
    Lazy::new(|| Regex::new(r"Using tool:\\s+(\\w+)(?:\\s+(.+))?").ok());
static TOOL_STATUS_RE: Lazy<Option<Regex>> = Lazy::new(|| {
    Regex::new(r"^([✓✗])\\s+(\\w+)\\s+(completed|failed|cancelled)(?::\\s*(.+))?$").ok()
});

#[derive(Debug, Clone)]
struct PendingToolCall {
    tool_name: String,
    log_id: Uuid,
    started_at: String,
    action_type: SdkActionType,
}

#[derive(Debug, Clone)]
struct CursorPendingTool {
    log_id: Uuid,
    started_at: String,
    tool_name: String,
    action_type: SdkActionType,
}

#[derive(Debug, Clone)]
struct GeminiPendingTool {
    log_id: Uuid,
    started_at: String,
    tool_name: String,
    action_type: SdkActionType,
}

#[derive(Debug, Clone)]
struct AssistantAccumulator {
    log_id: Uuid,
    started_at: String,
    content: String,
    last_update: DateTime<Utc>,
}

#[derive(Default)]
struct AttemptRealtimeState {
    pending_tools: Vec<PendingToolCall>,
    cursor_pending_tools: HashMap<String, CursorPendingTool>,
    gemini_pending_tools: HashMap<String, GeminiPendingTool>,
    assistant: Option<AssistantAccumulator>,
}

static REALTIME_STATE: Lazy<Mutex<HashMap<Uuid, AttemptRealtimeState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Status management for orchestrator
pub struct StatusManager;

impl StatusManager {
    /// Append log to JSONL and return (id, created_at). No agent_logs DB (Vibe Kanban style).
    async fn append_log(
        attempt_id: Uuid,
        log_type: &str,
        content: &str,
    ) -> Result<(Uuid, DateTime<Utc>)> {
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        append_log_to_jsonl(attempt_id, log_type, content, id, created_at).await?;
        Ok((id, created_at))
    }

    /// Append update to JSONL with same log_id (append-only; frontend takes last per id when replaying).
    async fn append_log_update(
        log_id: Uuid,
        attempt_id: Uuid,
        log_type: &str,
        content: &str,
    ) -> Result<()> {
        let created_at = Utc::now();
        append_log_to_jsonl(attempt_id, log_type, content, log_id, created_at).await
    }

    fn broadcast_log(
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        log_type: &str,
        content: String,
        id: Uuid,
        created_at: DateTime<Utc>,
        tool_name: Option<String>,
    ) {
        let _ = broadcast_tx.send(AgentEvent::Log(LogMessage {
            attempt_id,
            log_type: log_type.to_string(),
            content,
            timestamp: Utc::now(),
            id: Some(id),
            created_at: Some(created_at),
            tool_name,
        }));
    }

    fn extract_path_from_payload(payload: &str) -> String {
        let trimmed = payload.trim();
        if let Ok(v) = serde_json::from_str::<JsonValue>(trimmed) {
            if let Some(obj) = v.as_object() {
                if let Some(p) = obj
                    .get("file_path")
                    .or_else(|| obj.get("path"))
                    .and_then(JsonValue::as_str)
                {
                    return p.to_string();
                }
            }
        }
        trimmed.to_string()
    }

    fn sdk_action_from_cli(tool_name: &str, payload: &str) -> SdkActionType {
        let normalized = tool_name.to_lowercase();
        let payload = payload.trim().to_string();
        match normalized.as_str() {
            "read" | "read_file" => SdkActionType::FileRead {
                path: Self::extract_path_from_payload(&payload),
            },
            "edit" | "write" | "replace" => SdkActionType::FileEdit {
                path: Self::extract_path_from_payload(&payload),
                changes: vec![],
            },
            "bash" | "run_shell_command" => SdkActionType::CommandRun {
                command: payload,
                result: None,
            },
            "grep" | "glob" => SdkActionType::Search { query: payload },
            "webfetch" | "web_fetch" => SdkActionType::WebFetch { url: payload },
            "task" => SdkActionType::TaskCreate {
                description: payload,
            },
            _ => SdkActionType::Other {
                description: payload,
            },
        }
    }

    fn join_assistant_fragments(prev: &str, next: &str) -> String {
        if prev.is_empty() {
            return next.to_string();
        }
        if next.is_empty() {
            return prev.to_string();
        }

        // Snapshot-style updates (next already contains full previous content).
        if next.starts_with(prev) {
            return next.to_string();
        }

        // Stale/backward updates: keep the longest known content.
        if prev.starts_with(next) {
            return prev.to_string();
        }

        // Heuristic: treat leading whitespace as token continuation.
        let needs_separator = !prev.ends_with('\n') && !next.starts_with('\n');
        if needs_separator && !next.starts_with(' ') && !next.starts_with('\t') {
            format!("{}\n{}", prev, next)
        } else {
            format!("{}{}", prev, next)
        }
    }

    async fn append_assistant_message(
        _pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        fragment: &str,
    ) -> Result<()> {
        let fragment = fragment.to_string();
        if fragment.is_empty() {
            return Ok(());
        }

        const GAP_MS: i64 = 15_000;
        let now = Utc::now();

        // Compute state update and prepare async work while holding lock briefly.
        // Avoid holding lock during .await to reduce contention and CPU.
        enum UpdateResult {
            UpdateExisting { log_id: Uuid, json: String },
            CreateNew { json: String, started_at: String },
        }

        let result = {
            let mut state_guard = REALTIME_STATE.lock().await;
            let state = state_guard.entry(attempt_id).or_default();

            if let Some(acc) = state.assistant.as_mut() {
                let gap = now
                    .signed_duration_since(acc.last_update)
                    .num_milliseconds();
                if gap <= GAP_MS {
                    acc.content = Self::join_assistant_fragments(&acc.content, &fragment);
                    acc.last_update = now;

                    let entry = SdkNormalizedEntry {
                        timestamp: Some(acc.started_at.clone()),
                        entry_type: SdkNormalizedEntryType::AssistantMessage,
                        content: acc.content.clone(),
                    };

                    let json = serde_json::to_string(&entry)?;
                    UpdateResult::UpdateExisting {
                        log_id: acc.log_id,
                        json,
                    }
                } else {
                    let started_at = now.to_rfc3339();
                    let entry = SdkNormalizedEntry {
                        timestamp: Some(started_at.clone()),
                        entry_type: SdkNormalizedEntryType::AssistantMessage,
                        content: fragment.clone(),
                    };
                    let json = serde_json::to_string(&entry)?;
                    UpdateResult::CreateNew { json, started_at }
                }
            } else {
                let started_at = now.to_rfc3339();
                let entry = SdkNormalizedEntry {
                    timestamp: Some(started_at.clone()),
                    entry_type: SdkNormalizedEntryType::AssistantMessage,
                    content: fragment.clone(),
                };
                let json = serde_json::to_string(&entry)?;
                UpdateResult::CreateNew { json, started_at }
            }
        };

        match result {
            UpdateResult::UpdateExisting { log_id, json } => {
                Self::append_log_update(log_id, attempt_id, "normalized", &json).await?;
                Self::broadcast_log(
                    broadcast_tx,
                    attempt_id,
                    "normalized",
                    json,
                    log_id,
                    now,
                    None,
                );
            }
            UpdateResult::CreateNew { json, started_at } => {
                let (log_id, created_at) =
                    Self::append_log(attempt_id, "normalized", &json).await?;
                Self::broadcast_log(
                    broadcast_tx,
                    attempt_id,
                    "normalized",
                    json,
                    log_id,
                    created_at,
                    None,
                );

                let mut state_guard = REALTIME_STATE.lock().await;
                let state = state_guard.entry(attempt_id).or_default();
                state.assistant = Some(AssistantAccumulator {
                    log_id,
                    started_at,
                    content: fragment,
                    last_update: now,
                });
            }
        }

        Ok(())
    }

    pub async fn log_assistant_delta(
        db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        fragment: &str,
    ) -> Result<()> {
        Self::append_assistant_message(db_pool, broadcast_tx, attempt_id, fragment).await
    }

    pub async fn reset_assistant_accumulator(attempt_id: Uuid) {
        let mut state_guard = REALTIME_STATE.lock().await;
        if let Some(state) = state_guard.get_mut(&attempt_id) {
            state.assistant = None;
        }
    }

    pub async fn log_normalized_entry_and_get_id(
        _db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        entry: &SdkNormalizedEntry,
        tool_name: Option<String>,
    ) -> Result<Uuid> {
        let json = serde_json::to_string(entry)?;
        let (log_id, created_at) = Self::append_log(attempt_id, "normalized", &json).await?;
        Self::broadcast_log(
            broadcast_tx,
            attempt_id,
            "normalized",
            json,
            log_id,
            created_at,
            tool_name,
        );
        Ok(log_id)
    }

    pub async fn update_normalized_entry(
        _db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        log_id: Uuid,
        entry: &SdkNormalizedEntry,
        tool_name: Option<String>,
    ) -> Result<()> {
        let json = serde_json::to_string(entry)?;
        Self::append_log_update(log_id, attempt_id, "normalized", &json).await?;
        Self::broadcast_log(
            broadcast_tx,
            attempt_id,
            "normalized",
            json,
            log_id,
            Utc::now(),
            tool_name,
        );
        Ok(())
    }

    /// Create normalized ToolUse entry for Cursor shell/glob tool call (started).
    pub async fn create_cursor_tool_start(
        db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        call_id: &str,
        tool_name: &str,
        payload: &str,
    ) -> Result<()> {
        Self::reset_assistant_accumulator(attempt_id).await;
        let action_type = Self::sdk_action_from_cli(tool_name, payload);
        let started_at = Utc::now().to_rfc3339();
        let entry = SdkNormalizedEntry {
            timestamp: Some(started_at.clone()),
            entry_type: SdkNormalizedEntryType::ToolUse {
                tool_name: tool_name.to_string(),
                action_type: action_type.clone(),
                status: SdkToolStatus::Created,
            },
            content: String::new(),
        };
        let log_id = Self::log_normalized_entry_and_get_id(
            db_pool,
            broadcast_tx,
            attempt_id,
            &entry,
            Some(tool_name.to_string()),
        )
        .await?;
        let mut guard = REALTIME_STATE.lock().await;
        let state = guard.entry(attempt_id).or_default();
        state.cursor_pending_tools.insert(
            call_id.to_string(),
            CursorPendingTool {
                log_id,
                started_at,
                tool_name: tool_name.to_string(),
                action_type,
            },
        );
        Ok(())
    }

    /// Complete Cursor shell/glob tool call — update normalized entry with success/failed.
    pub async fn complete_cursor_tool(
        db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        call_id: &str,
        success: bool,
    ) -> Result<()> {
        let pending = {
            let mut guard = REALTIME_STATE.lock().await;
            let state = guard.entry(attempt_id).or_default();
            state.cursor_pending_tools.remove(call_id)
        };
        if let Some(p) = pending {
            let tool_name = p.tool_name.clone();
            let status = if success {
                SdkToolStatus::Success
            } else {
                SdkToolStatus::Failed
            };
            let entry = SdkNormalizedEntry {
                timestamp: Some(p.started_at),
                entry_type: SdkNormalizedEntryType::ToolUse {
                    tool_name: p.tool_name,
                    action_type: p.action_type,
                    status,
                },
                content: String::new(),
            };
            Self::update_normalized_entry(
                db_pool,
                broadcast_tx,
                attempt_id,
                p.log_id,
                &entry,
                Some(tool_name),
            )
            .await?;
        }
        Ok(())
    }

    /// Create normalized ToolUse entry for Gemini tool call (started).
    /// Skips if tool_id is already pending (deduplicates duplicate tool_use events from stream).
    pub async fn create_gemini_tool_start(
        db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        tool_id: &str,
        tool_name: &str,
        payload: &str,
    ) -> Result<()> {
        {
            let guard = REALTIME_STATE.lock().await;
            if guard
                .get(&attempt_id)
                .map_or(false, |s| s.gemini_pending_tools.contains_key(tool_id))
            {
                return Ok(());
            }
        }
        Self::reset_assistant_accumulator(attempt_id).await;
        let action_type = Self::sdk_action_from_cli(tool_name, payload);
        let display_name = match tool_name.to_lowercase().as_str() {
            "run_shell_command" => "Bash",
            "read_file" | "read" => "Read",
            "replace" | "edit" | "write" => "Edit",
            _ => tool_name,
        };
        let started_at = Utc::now().to_rfc3339();
        let entry = SdkNormalizedEntry {
            timestamp: Some(started_at.clone()),
            entry_type: SdkNormalizedEntryType::ToolUse {
                tool_name: display_name.to_string(),
                action_type: action_type.clone(),
                status: SdkToolStatus::Created,
            },
            content: String::new(),
        };
        let log_id = Self::log_normalized_entry_and_get_id(
            db_pool,
            broadcast_tx,
            attempt_id,
            &entry,
            Some(display_name.to_string()),
        )
        .await?;
        let mut guard = REALTIME_STATE.lock().await;
        let state = guard.entry(attempt_id).or_default();
        state.gemini_pending_tools.insert(
            tool_id.to_string(),
            GeminiPendingTool {
                log_id,
                started_at,
                tool_name: display_name.to_string(),
                action_type,
            },
        );
        Ok(())
    }

    /// Complete Gemini tool call — update normalized entry with success/failed.
    pub async fn complete_gemini_tool(
        db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        tool_id: &str,
        success: bool,
    ) -> Result<()> {
        let pending = {
            let mut guard = REALTIME_STATE.lock().await;
            let state = guard.entry(attempt_id).or_default();
            state.gemini_pending_tools.remove(tool_id)
        };
        if let Some(p) = pending {
            let tool_name = p.tool_name.clone();
            let status = if success {
                SdkToolStatus::Success
            } else {
                SdkToolStatus::Failed
            };
            let entry = SdkNormalizedEntry {
                timestamp: Some(p.started_at),
                entry_type: SdkNormalizedEntryType::ToolUse {
                    tool_name: p.tool_name,
                    action_type: p.action_type,
                    status,
                },
                content: String::new(),
            };
            Self::update_normalized_entry(
                db_pool,
                broadcast_tx,
                attempt_id,
                p.log_id,
                &entry,
                Some(tool_name),
            )
            .await?;
        }
        Ok(())
    }

    pub async fn update_status(
        db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        status: AttemptStatus,
    ) -> Result<()> {
        // Update status with appropriate timestamps
        match status {
            AttemptStatus::Running => {
                // Set started_at when beginning execution
                sqlx::query(
                    r#"
                    UPDATE task_attempts
                    SET status = $1, started_at = NOW()
                    WHERE id = $2
                    "#,
                )
                .bind(status)
                .bind(attempt_id)
                .execute(db_pool)
                .await
                .context("Failed to update attempt status to running")?;

                // Sync task status: when attempt starts running, task must be in_progress.
                // (Regular tasks were never updated; init flow does it explicitly but this ensures consistency.)
                sqlx::query(
                    r#"
                    UPDATE tasks
                    SET status = 'in_progress', updated_at = NOW()
                    WHERE id = (SELECT task_id FROM task_attempts WHERE id = $1)
                      AND status = 'todo'
                    "#,
                )
                .bind(attempt_id)
                .execute(db_pool)
                .await
                .context("Failed to sync task status to in_progress")?;
            }
            AttemptStatus::Success | AttemptStatus::Failed | AttemptStatus::Cancelled => {
                // Flush any buffered logs before marking attempt complete
                let _ = flush_agent_log_buffer().await;
                // Set completed_at when finishing execution
                sqlx::query(
                    r#"
                    UPDATE task_attempts
                    SET status = $1, completed_at = NOW()
                    WHERE id = $2
                    "#,
                )
                .bind(status)
                .bind(attempt_id)
                .execute(db_pool)
                .await
                .context("Failed to update attempt status to completed")?;
            }
            AttemptStatus::Queued => {
                // Just update status for queued
                sqlx::query(
                    r#"
                    UPDATE task_attempts
                    SET status = $1
                    WHERE id = $2
                    "#,
                )
                .bind(status)
                .bind(attempt_id)
                .execute(db_pool)
                .await
                .context("Failed to update attempt status to queued")?;
            }
        }

        let _ = broadcast_tx.send(AgentEvent::Status(StatusMessage {
            attempt_id,
            status,
            timestamp: Utc::now(),
        }));

        Ok(())
    }

    pub async fn fail_attempt(
        db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        error: &str,
    ) -> Result<()> {
        // Log error to attempt logs so user sees it in the timeline (not just DB)
        if !error.trim().is_empty() {
            let sanitized = crate::sanitize_log(error);
            let (log_id, created_at) = buffer_agent_log(attempt_id, "stderr", &sanitized).await;
            Self::broadcast_log(
                broadcast_tx,
                attempt_id,
                "stderr",
                sanitized,
                log_id,
                created_at,
                None,
            );
        }
        let _ = flush_agent_log_buffer().await;
        // Update attempt status to failed with completed_at timestamp
        let result = sqlx::query(
            r#"
            UPDATE task_attempts
            SET status = 'failed', error_message = $1, completed_at = NOW()
            WHERE id = $2
              AND status != 'cancelled'
            "#,
        )
        .bind(error)
        .bind(attempt_id)
        .execute(db_pool)
        .await
        .context("Failed to set attempt failed status")?;

        if result.rows_affected() == 0 {
            return Ok(());
        }

        // Reset task status from 'in_progress' to 'todo' when attempt fails
        // This prevents tasks from being stuck in 'in_progress' state
        sqlx::query(
            r#"
            UPDATE tasks
            SET status = 'todo', updated_at = NOW()
            WHERE id = (SELECT task_id FROM task_attempts WHERE id = $1)
              AND status = 'in_progress'
            "#,
        )
        .bind(attempt_id)
        .execute(db_pool)
        .await
        .context("Failed to reset task status")?;

        let _ = broadcast_tx.send(AgentEvent::Status(StatusMessage {
            attempt_id,
            status: AttemptStatus::Failed,
            timestamp: Utc::now(),
        }));

        Ok(())
    }

    pub async fn cancel_attempt(
        db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        reason: &str,
    ) -> Result<()> {
        let _ = flush_agent_log_buffer().await;
        // Update attempt status to cancelled with completed_at timestamp
        sqlx::query(
            r#"
            UPDATE task_attempts
            SET status = 'cancelled', error_message = $1, completed_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(reason)
        .bind(attempt_id)
        .execute(db_pool)
        .await
        .context("Failed to set attempt cancelled status")?;

        // Reset task status from 'in_progress' to 'todo' when attempt is cancelled
        sqlx::query(
            r#"
            UPDATE tasks
            SET status = 'todo', updated_at = NOW()
            WHERE id = (SELECT task_id FROM task_attempts WHERE id = $1)
              AND status = 'in_progress'
            "#,
        )
        .bind(attempt_id)
        .execute(db_pool)
        .await
        .context("Failed to reset task status")?;

        let _ = broadcast_tx.send(AgentEvent::Status(StatusMessage {
            attempt_id,
            status: AttemptStatus::Cancelled,
            timestamp: Utc::now(),
        }));

        Ok(())
    }

    pub async fn log(
        db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        role: &str,
        content: &str,
    ) -> Result<()> {
        match role {
            // Raw process output should not drive the chat timeline directly.
            // We store it under process_* types and emit VK-like normalized entries for the timeline.
            "stdout" => Self::log_stdout(db_pool, broadcast_tx, attempt_id, content).await,
            "stderr" => Self::log_stderr(db_pool, broadcast_tx, attempt_id, content).await,
            // Already-normalized entries (SDK/vibe-kanban style JSON).
            "normalized" => {
                Self::log_normalized_raw(db_pool, broadcast_tx, attempt_id, content).await
            }
            _ => Self::log_simple(db_pool, broadcast_tx, attempt_id, role, content).await,
        }
    }

    async fn log_simple(
        _db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        role: &str,
        content: &str,
    ) -> Result<()> {
        let (log_id, created_at) = buffer_agent_log(attempt_id, role, content).await;
        Self::broadcast_log(
            broadcast_tx,
            attempt_id,
            role,
            content.to_string(),
            log_id,
            created_at,
            None,
        );

        // User input should break an in-flight assistant accumulator.
        if role == "user" || role == "stdin" {
            Self::reset_assistant_accumulator(attempt_id).await;
        }

        Ok(())
    }

    async fn log_normalized_raw(
        _db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        content: &str,
    ) -> Result<()> {
        if let Ok(entry) = serde_json::from_str::<SdkNormalizedEntry>(content) {
            if let Err(reason) = validate_sdk_normalized_entry(&entry) {
                warn!(
                    attempt_id = %attempt_id,
                    reason = %reason,
                    "Normalized entry failed contract validation"
                );
            }
        }

        // Normalized entries may be updated (assistant, tool calls) - use immediate insert
        let (log_id, created_at) = Self::append_log(attempt_id, "normalized", content).await?;
        Self::broadcast_log(
            broadcast_tx,
            attempt_id,
            "normalized",
            content.to_string(),
            log_id,
            created_at,
            None,
        );
        Ok(())
    }

    async fn log_stderr(
        _db_pool: &PgPool,
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        content: &str,
    ) -> Result<()> {
        let (log_id, created_at) = buffer_agent_log(attempt_id, "process_stderr", content).await;
        Self::broadcast_log(
            broadcast_tx,
            attempt_id,
            "process_stderr",
            content.to_string(),
            log_id,
            created_at,
            None,
        );
        Ok(())
    }

    async fn log_stdout(
        db_pool: &PgPool, // Passed to callees for API compat
        broadcast_tx: &broadcast::Sender<AgentEvent>,
        attempt_id: Uuid,
        content: &str,
    ) -> Result<()> {
        // R2: Buffer raw for persistence (DB + JSONL). Do NOT broadcast raw -
        // we emit normalized below; frontend prefers normalized.
        let _ = buffer_agent_log(attempt_id, "process_stdout", content).await;

        let line = content.trim();
        if content.is_empty() {
            return Ok(());
        }

        // Tool start: "Using tool: Bash <payload>"
        if let Some(start_re) = TOOL_START_RE.as_ref() {
            if let Some(caps) = start_re.captures(line) {
                Self::reset_assistant_accumulator(attempt_id).await;
                let tool_name = caps.get(1).map(|m| m.as_str()).unwrap_or("Tool");
                let payload = caps.get(2).map(|m| m.as_str()).unwrap_or("").trim();

                let started_at = Utc::now().to_rfc3339();
                let action_type = Self::sdk_action_from_cli(tool_name, payload);

                let entry = SdkNormalizedEntry {
                    timestamp: Some(started_at.clone()),
                    entry_type: SdkNormalizedEntryType::ToolUse {
                        tool_name: tool_name.to_string(),
                        action_type: action_type.clone(),
                        status: SdkToolStatus::Created,
                    },
                    content: String::new(),
                };

                let json = serde_json::to_string(&entry)?;
                let (tool_log_id, tool_created_at) =
                    Self::append_log(attempt_id, "normalized", &json).await?;
                Self::broadcast_log(
                    broadcast_tx,
                    attempt_id,
                    "normalized",
                    json,
                    tool_log_id,
                    tool_created_at,
                    Some(tool_name.to_string()),
                );

                let mut guard = REALTIME_STATE.lock().await;
                let state = guard.entry(attempt_id).or_default();
                state.pending_tools.push(PendingToolCall {
                    tool_name: tool_name.to_string(),
                    log_id: tool_log_id,
                    started_at,
                    action_type,
                });

                return Ok(());
            }
        }

        // Tool completion: "✓ Bash completed" / "✗ Bash failed: ..."
        if let Some(status_re) = TOOL_STATUS_RE.as_ref() {
            if let Some(caps) = status_re.captures(line) {
                Self::reset_assistant_accumulator(attempt_id).await;
                let symbol = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let tool_name = caps.get(2).map(|m| m.as_str()).unwrap_or("Tool");
                let verb = caps
                    .get(3)
                    .map(|m| m.as_str())
                    .unwrap_or("completed")
                    .to_lowercase();

                let status = if verb == "failed" || symbol == "✗" {
                    SdkToolStatus::Failed
                } else {
                    SdkToolStatus::Success
                };

                // Find the most recent pending tool call for this tool and update it in-place.
                let pending = {
                    let mut guard = REALTIME_STATE.lock().await;
                    let state = guard.entry(attempt_id).or_default();
                    let idx = state
                        .pending_tools
                        .iter()
                        .rposition(|p| p.tool_name.eq_ignore_ascii_case(tool_name));
                    idx.map(|i| state.pending_tools.remove(i))
                };

                if let Some(p) = pending {
                    let entry = SdkNormalizedEntry {
                        timestamp: Some(p.started_at.clone()),
                        entry_type: SdkNormalizedEntryType::ToolUse {
                            tool_name: p.tool_name.clone(),
                            action_type: p.action_type.clone(),
                            status,
                        },
                        content: String::new(),
                    };

                    Self::update_normalized_entry(
                        db_pool,
                        broadcast_tx,
                        attempt_id,
                        p.log_id,
                        &entry,
                        Some(p.tool_name.clone()),
                    )
                    .await?;
                    return Ok(());
                }

                // If we couldn't find a matching start, emit a standalone tool card.
                let started_at = Utc::now().to_rfc3339();
                let entry = SdkNormalizedEntry {
                    timestamp: Some(started_at),
                    entry_type: SdkNormalizedEntryType::ToolUse {
                        tool_name: tool_name.to_string(),
                        action_type: SdkActionType::Other {
                            description: String::new(),
                        },
                        status,
                    },
                    content: String::new(),
                };
                let _ = Self::log_normalized_entry_and_get_id(
                    db_pool,
                    broadcast_tx,
                    attempt_id,
                    &entry,
                    Some(tool_name.to_string()),
                )
                .await?;
                return Ok(());
            }
        }

        // Default: treat stdout as assistant message content for the timeline.
        Self::append_assistant_message(db_pool, broadcast_tx, attempt_id, content).await
    }
}
