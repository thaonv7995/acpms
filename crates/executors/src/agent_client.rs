//! Claude agent client - implements ProtocolHandler for approval workflow
//!
//! This module handles the Claude SDK protocol and extracts structured entries
//! including tool_use blocks for vibe-kanban style display.

use crate::approval::{ApprovalService, ApprovalStatus};
use crate::assistant_log_buffer::AgentTextBuffer;
use crate::log_writer::LogWriter;
use crate::normalization::{
    NormalizedEntry as NewNormalizedEntry, NormalizedEntryType as NormalizedEntryTypeTrait,
    SubagentSpawn,
};
use crate::orchestrator_status::StatusManager;
use crate::protocol::{PermissionResult, PermissionUpdate, ProtocolHandler};
use crate::sdk_normalized_types::{
    format_tool_content, map_tool_to_action, NormalizedEntry, NormalizedEntryType, ToolStatus,
};
use anyhow::Result;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use uuid::Uuid;

/// Partial tool call data for tracking streaming tool calls
#[derive(Debug, Clone, Default)]
struct PartialToolCall {
    id: String,
    name: String,
    input_json: String,
    started_at: String,
    timeline_log_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TokenUsageSnapshot {
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: Option<u64>,
    model_context_window: Option<u64>,
}

/// Claude agent client - handles protocol callbacks
pub struct ClaudeAgentClient {
    attempt_id: Uuid,
    log_writer: LogWriter,
    approval_service: Option<Arc<dyn ApprovalService>>,
    auto_approve: bool,
    // Database logging (instead of LogWriter)
    db_pool: Option<PgPool>,
    broadcast_tx: Option<broadcast::Sender<crate::AgentEvent>>,
    // Tool call state tracking
    tool_calls: RwLock<HashMap<String, PartialToolCall>>,
    // Dedupe for metadata-derived normalized entries
    last_token_usage: RwLock<Option<TokenUsageSnapshot>>,
    last_next_action: RwLock<Option<String>>,
    last_answered_question: RwLock<Option<(String, String)>>,
    assistant_buffer: Mutex<AgentTextBuffer>,
    runtime_tool_tx: Option<mpsc::UnboundedSender<Value>>,
}

impl ClaudeAgentClient {
    /// Create new agent client
    ///
    /// ## Arguments:
    /// - `attempt_id`: Task attempt ID (for approval tracking)
    /// - `log_writer`: Writer for forwarding non-control messages
    /// - `approval_service`: Optional approval service (if None, auto-approves all)
    pub fn new(
        attempt_id: Uuid,
        log_writer: LogWriter,
        approval_service: Option<Arc<dyn ApprovalService>>,
        runtime_tool_tx: Option<mpsc::UnboundedSender<Value>>,
    ) -> Arc<Self> {
        let auto_approve = approval_service.is_none();
        Arc::new(Self {
            attempt_id,
            log_writer,
            approval_service,
            auto_approve,
            db_pool: None,
            broadcast_tx: None,
            tool_calls: RwLock::new(HashMap::new()),
            last_token_usage: RwLock::new(None),
            last_next_action: RwLock::new(None),
            last_answered_question: RwLock::new(None),
            assistant_buffer: Mutex::new(AgentTextBuffer::new()),
            runtime_tool_tx,
        })
    }

    /// Create with database logging support
    pub fn with_database(
        attempt_id: Uuid,
        log_writer: LogWriter,
        approval_service: Option<Arc<dyn ApprovalService>>,
        db_pool: PgPool,
        broadcast_tx: broadcast::Sender<crate::AgentEvent>,
        runtime_tool_tx: Option<mpsc::UnboundedSender<Value>>,
    ) -> Arc<Self> {
        let auto_approve = approval_service.is_none();
        Arc::new(Self {
            attempt_id,
            log_writer,
            approval_service,
            auto_approve,
            db_pool: Some(db_pool),
            broadcast_tx: Some(broadcast_tx),
            tool_calls: RwLock::new(HashMap::new()),
            last_token_usage: RwLock::new(None),
            last_next_action: RwLock::new(None),
            last_answered_question: RwLock::new(None),
            assistant_buffer: Mutex::new(AgentTextBuffer::new()),
            runtime_tool_tx,
        })
    }

    async fn emit_normalized_entry(&self, entry: &NormalizedEntry) {
        if let (Some(pool), Some(tx)) = (&self.db_pool, &self.broadcast_tx) {
            match serde_json::to_string(entry) {
                Ok(json) => {
                    if let Err(e) =
                        StatusManager::log(pool, tx, self.attempt_id, "normalized", &json).await
                    {
                        tracing::warn!(
                            attempt_id = %self.attempt_id,
                            error = %e,
                            "Failed to emit normalized entry"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        attempt_id = %self.attempt_id,
                        error = %e,
                        "Failed to serialize normalized entry"
                    );
                }
            }
        }
    }

    async fn emit_tool_status_entry(
        &self,
        tool_name: &str,
        input: &Value,
        status: ToolStatus,
        content_override: Option<String>,
    ) {
        let action_type = map_tool_to_action(tool_name, Some(input));
        let entry = NormalizedEntry {
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: tool_name.to_string(),
                action_type,
                status,
            },
            content: content_override
                .unwrap_or_else(|| format_tool_content(tool_name, Some(input))),
        };

        self.emit_normalized_entry(&entry).await;
    }

    async fn emit_metadata_entries(&self, line: &str) {
        let entries = parse_claude_metadata_entries(line);
        if entries.is_empty() {
            return;
        }

        for entry in entries {
            let should_emit = match &entry.entry_type {
                NormalizedEntryType::TokenUsageInfo {
                    input_tokens,
                    output_tokens,
                    total_tokens,
                    model_context_window,
                } => {
                    let snapshot = TokenUsageSnapshot {
                        input_tokens: *input_tokens,
                        output_tokens: *output_tokens,
                        total_tokens: *total_tokens,
                        model_context_window: *model_context_window,
                    };
                    let mut guard = self.last_token_usage.write().await;
                    if guard.as_ref() == Some(&snapshot) {
                        false
                    } else {
                        *guard = Some(snapshot);
                        true
                    }
                }
                NormalizedEntryType::NextAction { text } => {
                    let mut guard = self.last_next_action.write().await;
                    if guard.as_ref() == Some(text) {
                        false
                    } else {
                        *guard = Some(text.clone());
                        true
                    }
                }
                NormalizedEntryType::UserAnsweredQuestions { question, answer } => {
                    let snapshot = (question.clone(), answer.clone());
                    let mut guard = self.last_answered_question.write().await;
                    if guard.as_ref() == Some(&snapshot) {
                        false
                    } else {
                        *guard = Some(snapshot);
                        true
                    }
                }
                _ => true,
            };

            if should_emit {
                self.emit_normalized_entry(&entry).await;
            }
        }
    }

    async fn emit_assistant_delta(&self, content: &str) {
        if content.trim().is_empty() {
            return;
        }

        if let (Some(pool), Some(tx)) = (&self.db_pool, &self.broadcast_tx) {
            let _ = StatusManager::log_assistant_delta(pool, tx, self.attempt_id, content).await;
        }
    }

    async fn emit_runtime_tool_metadata(&self, metadata: Option<Value>) {
        let Some(metadata) = metadata else {
            return;
        };
        let Some(tx) = &self.runtime_tool_tx else {
            return;
        };

        let _ = tx.send(metadata);
    }

    async fn emit_runtime_capable_assistant_content(&self, content: &str) {
        let mut buffer = self.assistant_buffer.lock().await;
        buffer.push(content);

        let mut emitted_any = false;
        while let Some((text, metadata)) = buffer.pop_next() {
            emitted_any = true;
            drop(buffer);
            self.emit_assistant_delta(&text).await;
            self.emit_runtime_tool_metadata(metadata).await;
            buffer = self.assistant_buffer.lock().await;
        }

        if !emitted_any {
            if let Some((text, metadata)) = buffer.pop_partial_text_for_display() {
                drop(buffer);
                self.emit_assistant_delta(&text).await;
                self.emit_runtime_tool_metadata(metadata).await;
            }
        }
    }

    async fn flush_runtime_capable_assistant_content(&self) {
        let mut buffer = self.assistant_buffer.lock().await;
        if let Some((text, metadata)) = buffer.flush() {
            drop(buffer);
            self.emit_assistant_delta(&text).await;
            self.emit_runtime_tool_metadata(metadata).await;
        }
    }
}

/// Result of extracting content from SDK message
enum ExtractResult {
    /// Human-readable content to save (text_delta, thinking_delta)
    Content(String),
    /// Normalized entry (tool_use, complete messages)
    Normalized(Box<NormalizedEntry>),
    /// Boundary markers for assistant messages (SDK protocol).
    AssistantMessageStart,
    AssistantMessageStop,
    /// Skip this message (protocol noise, not displayable)
    Skip,
    /// Failed to parse - unknown format
    Unknown,
}

fn parse_claude_metadata_entries(line: &str) -> Vec<NormalizedEntry> {
    let value: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut entries = Vec::new();

    if let Some((input_tokens, output_tokens, total_tokens, model_context_window)) =
        extract_token_usage(&value)
    {
        entries.push(NormalizedEntry {
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            entry_type: NormalizedEntryType::TokenUsageInfo {
                input_tokens,
                output_tokens,
                total_tokens,
                model_context_window,
            },
            content: String::new(),
        });
    }

    if let Some(next_action) = extract_next_action_text(&value) {
        entries.push(NormalizedEntry {
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            entry_type: NormalizedEntryType::NextAction { text: next_action },
            content: String::new(),
        });
    }

    if let Some((question, answer)) = extract_user_answered_question(&value) {
        entries.push(NormalizedEntry {
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            entry_type: NormalizedEntryType::UserAnsweredQuestions { question, answer },
            content: String::new(),
        });
    }

    entries
}

fn extract_token_usage(value: &Value) -> Option<(u64, u64, Option<u64>, Option<u64>)> {
    let usage_obj = [
        "/message/usage",
        "/event/message/usage",
        "/stream_event/event/message/usage",
        "/usage",
        "/result/usage",
        "/event/usage",
    ]
    .iter()
    .find_map(|path| value.pointer(path).and_then(Value::as_object));

    let usage = usage_obj?;

    let input_tokens = usage
        .get("input_tokens")
        .or_else(|| usage.get("inputTokens"))
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(value_as_u64)
        .unwrap_or(0);

    let output_tokens = usage
        .get("output_tokens")
        .or_else(|| usage.get("outputTokens"))
        .or_else(|| usage.get("completion_tokens"))
        .and_then(value_as_u64)
        .unwrap_or(0);

    let total_tokens = usage
        .get("total_tokens")
        .or_else(|| usage.get("totalTokens"))
        .and_then(value_as_u64);

    if input_tokens == 0 && output_tokens == 0 && total_tokens.unwrap_or(0) == 0 {
        return None;
    }

    let model_context_window = extract_model_context_window(value, usage);

    Some((
        input_tokens,
        output_tokens,
        total_tokens,
        model_context_window,
    ))
}

fn extract_model_context_window(
    value: &Value,
    usage: &serde_json::Map<String, Value>,
) -> Option<u64> {
    if let Some(window) = usage
        .get("model_context_window")
        .or_else(|| usage.get("modelContextWindow"))
        .or_else(|| usage.get("context_window"))
        .or_else(|| usage.get("contextWindow"))
        .and_then(value_as_u64)
        .filter(|window| *window > 0)
    {
        return Some(window);
    }

    for pointer in [
        "/message/model_context_window",
        "/event/message/model_context_window",
        "/stream_event/event/message/model_context_window",
        "/model_context_window",
        "/modelContextWindow",
        "/result/model_context_window",
        "/result/modelContextWindow",
    ] {
        if let Some(window) = value
            .pointer(pointer)
            .and_then(value_as_u64)
            .filter(|w| *w > 0)
        {
            return Some(window);
        }
    }

    for pointer in [
        "/result/model_usage",
        "/result/modelUsage",
        "/message/model_usage",
        "/message/modelUsage",
        "/event/message/model_usage",
        "/event/message/modelUsage",
        "/stream_event/event/message/model_usage",
        "/stream_event/event/message/modelUsage",
        "/model_usage",
        "/modelUsage",
    ] {
        let Some(model_usage) = value.pointer(pointer).and_then(Value::as_object) else {
            continue;
        };

        for usage in model_usage.values() {
            if let Some(window) = usage
                .get("context_window")
                .or_else(|| usage.get("contextWindow"))
                .and_then(value_as_u64)
                .filter(|w| *w > 0)
            {
                return Some(window);
            }
        }
    }

    None
}

fn extract_next_action_text(value: &Value) -> Option<String> {
    if let Some(explicit_next_action) = [
        "/message/next_action",
        "/event/message/next_action",
        "/stream_event/event/message/next_action",
        "/next_action",
        "/nextAction",
        "/data/next_action",
        "/data/nextAction",
    ]
    .iter()
    .find_map(|path| value.pointer(path).and_then(Value::as_str))
    {
        let text = explicit_next_action.trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }

    let stop_reason = [
        "/message/stop_reason",
        "/event/message/stop_reason",
        "/stream_event/event/message/stop_reason",
        "/stop_reason",
        "/stopReason",
    ]
    .iter()
    .find_map(|path| value.pointer(path).and_then(Value::as_str))
    .map(|s| s.to_ascii_lowercase())?;

    match stop_reason.as_str() {
        "tool_use" => Some("Continue by completing required tool actions".to_string()),
        "max_tokens" => Some("Continue generation to complete the response".to_string()),
        "pause_turn" => Some("Waiting for user follow-up input".to_string()),
        _ => None,
    }
}

fn extract_user_answered_question(value: &Value) -> Option<(String, String)> {
    // Best-effort support for hook payloads carrying explicit question/answer pairs.
    let obj = [
        "/question_answer",
        "/event/question_answer",
        "/stream_event/event/question_answer",
        "/data/question_answer",
        "/input",
        "/event/input",
        "/data/input",
    ]
    .iter()
    .find_map(|path| value.pointer(path).and_then(Value::as_object))?;

    let question = obj
        .get("question")
        .or_else(|| obj.get("prompt"))
        .or_else(|| obj.get("message"))
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    if question.is_empty() {
        return None;
    }

    let answer = obj
        .get("answer")
        .or_else(|| obj.get("response"))
        .or_else(|| obj.get("decision"))
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    if answer.is_empty() {
        return None;
    }

    Some((question, answer))
}

fn value_as_u64(value: &Value) -> Option<u64> {
    if let Some(v) = value.as_u64() {
        return Some(v);
    }
    if let Some(v) = value.as_i64() {
        return u64::try_from(v).ok();
    }
    value.as_str().and_then(|s| s.trim().parse::<u64>().ok())
}

impl ClaudeAgentClient {
    /// Extract human-readable content from SDK protocol messages
    /// Returns ExtractResult indicating what to do with the message
    async fn extract_sdk_content(&self, line: &str) -> ExtractResult {
        // Try to parse as JSON
        let json: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(
                    attempt_id = %self.attempt_id,
                    error = %e,
                    line_preview = %line.chars().take(100).collect::<String>(),
                    "SDK extract: Failed to parse JSON, returning Unknown"
                );
                return ExtractResult::Unknown;
            }
        };

        let msg_type = match json.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => {
                tracing::debug!(
                    attempt_id = %self.attempt_id,
                    json = %json,
                    "SDK extract: No 'type' field in JSON, returning Unknown"
                );
                return ExtractResult::Unknown;
            }
        };

        tracing::debug!(
            attempt_id = %self.attempt_id,
            msg_type = %msg_type,
            "SDK extract: Processing message type"
        );

        match msg_type {
            "content_block_start" => {
                tracing::debug!(attempt_id = %self.attempt_id, "SDK extract: Direct content_block_start");
                self.handle_content_block_start(&json).await
            }
            "content_block_delta" => self.handle_content_block_delta(&json).await,
            "content_block_stop" => {
                tracing::debug!(attempt_id = %self.attempt_id, "SDK extract: Direct content_block_stop");
                self.handle_content_block_stop(&json).await
            }
            "message_start" => ExtractResult::AssistantMessageStart,
            "message_stop" => ExtractResult::AssistantMessageStop,
            "stream_event" => {
                let event = match json.get("event") {
                    Some(e) => e,
                    None => {
                        tracing::debug!(
                            attempt_id = %self.attempt_id,
                            "SDK extract: stream_event has no 'event' field"
                        );
                        return ExtractResult::Skip;
                    }
                };
                let event_type = match event.get("type").and_then(|t| t.as_str()) {
                    Some(t) => t,
                    None => {
                        tracing::debug!(
                            attempt_id = %self.attempt_id,
                            "SDK extract: stream_event.event has no 'type' field"
                        );
                        return ExtractResult::Skip;
                    }
                };

                tracing::debug!(
                    attempt_id = %self.attempt_id,
                    event_type = %event_type,
                    "SDK extract: stream_event inner type"
                );

                match event_type {
                    "content_block_start" => self.handle_content_block_start(event).await,
                    "content_block_delta" => self.handle_content_block_delta(event).await,
                    "content_block_stop" => self.handle_content_block_stop(event).await,
                    "message_start" => ExtractResult::AssistantMessageStart,
                    "message_stop" => ExtractResult::AssistantMessageStop,
                    "message_delta" => ExtractResult::Skip,
                    _ => {
                        tracing::debug!(
                            attempt_id = %self.attempt_id,
                            event_type = %event_type,
                            "SDK extract: Unhandled stream_event type"
                        );
                        ExtractResult::Skip
                    }
                }
            }
            // Skip other protocol messages
            "init" | "result" | "error" | "ping" | "pong" => ExtractResult::Skip,
            _ => {
                tracing::debug!(
                    attempt_id = %self.attempt_id,
                    msg_type = %msg_type,
                    "SDK extract: Unhandled top-level message type"
                );
                ExtractResult::Skip
            }
        }
    }

    /// Handle content_block_start - capture tool_use info
    async fn handle_content_block_start(&self, json: &Value) -> ExtractResult {
        let content_block = match json.get("content_block") {
            Some(cb) => cb,
            None => {
                tracing::debug!(
                    attempt_id = %self.attempt_id,
                    json = %json,
                    "handle_content_block_start: No 'content_block' field"
                );
                return ExtractResult::Skip;
            }
        };
        let block_type = content_block.get("type").and_then(|t| t.as_str());

        tracing::debug!(
            attempt_id = %self.attempt_id,
            block_type = ?block_type,
            "handle_content_block_start: Processing block type"
        );

        match block_type {
            Some("tool_use") => {
                let id = content_block
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let name = content_block
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                tracing::info!(
                    attempt_id = %self.attempt_id,
                    tool_id = %id,
                    tool_name = %name,
                    "handle_content_block_start: TOOL_USE DETECTED - Creating normalized entry"
                );

                // Detect Task tool spawns for subagent tracking
                if name == "Task" {
                    tracing::info!(
                        attempt_id = %self.attempt_id,
                        tool_id = %id,
                        "handle_content_block_start: TASK TOOL DETECTED - Subagent spawn"
                    );
                    // Note: We'll need the input to extract task description
                    // which comes later in input_json_delta. Track this for now.
                }

                // Store partial tool call
                let started_at = chrono::Utc::now().to_rfc3339();
                {
                    let mut tool_calls = self.tool_calls.write().await;
                    tool_calls.insert(
                        id.to_string(),
                        PartialToolCall {
                            id: id.to_string(),
                            name: name.to_string(),
                            input_json: String::new(),
                            started_at: started_at.clone(),
                            timeline_log_id: None,
                        },
                    );
                }

                // Emit initial tool_use entry directly to the DB/timeline so we can update it in-place later.
                if let (Some(pool), Some(tx)) = (&self.db_pool, &self.broadcast_tx) {
                    let action_type = map_tool_to_action(name, None);
                    let entry = NormalizedEntry {
                        timestamp: Some(started_at.clone()),
                        entry_type: NormalizedEntryType::ToolUse {
                            tool_name: name.to_string(),
                            action_type,
                            status: ToolStatus::Created,
                        },
                        content: name.to_string(),
                    };

                    if let Ok(log_id) = StatusManager::log_normalized_entry_and_get_id(
                        pool,
                        tx,
                        self.attempt_id,
                        &entry,
                        Some(name.to_string()),
                    )
                    .await
                    {
                        let mut tool_calls = self.tool_calls.write().await;
                        if let Some(tc) = tool_calls.get_mut(id) {
                            tc.timeline_log_id = Some(log_id);
                        }
                    }
                }

                ExtractResult::Skip
            }
            Some("text") => {
                tracing::debug!(attempt_id = %self.attempt_id, "handle_content_block_start: text block, skipping");
                ExtractResult::Skip
            }
            Some("thinking") => {
                tracing::debug!(attempt_id = %self.attempt_id, "handle_content_block_start: thinking block, skipping");
                ExtractResult::Skip
            }
            _ => {
                tracing::debug!(
                    attempt_id = %self.attempt_id,
                    block_type = ?block_type,
                    "handle_content_block_start: Unknown block type"
                );
                ExtractResult::Skip
            }
        }
    }

    /// Handle content_block_delta - accumulate tool input or extract text
    async fn handle_content_block_delta(&self, json: &Value) -> ExtractResult {
        let delta = match json.get("delta") {
            Some(d) => d,
            None => {
                tracing::debug!(
                    attempt_id = %self.attempt_id,
                    "handle_content_block_delta: No 'delta' field"
                );
                return ExtractResult::Skip;
            }
        };
        let delta_type = delta.get("type").and_then(|t| t.as_str());

        match delta_type {
            Some("text_delta") => match delta.get("text").and_then(|t| t.as_str()) {
                Some(text) if !text.is_empty() => ExtractResult::Content(text.to_string()),
                _ => ExtractResult::Skip,
            },
            Some("thinking_delta") => {
                tracing::debug!(
                    attempt_id = %self.attempt_id,
                    "handle_content_block_delta: thinking_delta detected"
                );
                match delta.get("thinking").and_then(|t| t.as_str()) {
                    Some(thinking) if !thinking.is_empty() => {
                        ExtractResult::Normalized(Box::new(NormalizedEntry {
                            timestamp: Some(chrono::Utc::now().to_rfc3339()),
                            entry_type: NormalizedEntryType::Thinking,
                            content: thinking.to_string(),
                        }))
                    }
                    _ => ExtractResult::Skip,
                }
            }
            Some("input_json_delta") => {
                // Accumulate tool input JSON
                if let Some(partial) = delta.get("partial_json").and_then(|v| v.as_str()) {
                    let mut tool_calls = self.tool_calls.write().await;
                    // Find tool call by index (simplified - in practice match by tracking)
                    if let Some(tc) = tool_calls.values_mut().last() {
                        tc.input_json.push_str(partial);
                        tracing::debug!(
                            attempt_id = %self.attempt_id,
                            tool_name = %tc.name,
                            partial_len = partial.len(),
                            "handle_content_block_delta: Accumulating input_json_delta"
                        );
                    }
                }
                ExtractResult::Skip
            }
            _ => {
                tracing::debug!(
                    attempt_id = %self.attempt_id,
                    delta_type = ?delta_type,
                    "handle_content_block_delta: Unknown delta type"
                );
                ExtractResult::Skip
            }
        }
    }

    /// Handle content_block_stop - emit complete tool_use entry
    async fn handle_content_block_stop(&self, _json: &Value) -> ExtractResult {
        // Check if this is a tool_use block that just completed
        let tc = {
            let mut tool_calls = self.tool_calls.write().await;
            let tc = tool_calls.values().last().cloned();
            if let Some(ref tc) = tc {
                tool_calls.remove(&tc.id);
            }
            tc
        };

        if let Some(tc) = tc {
            tracing::info!(
                attempt_id = %self.attempt_id,
                tool_id = %tc.id,
                tool_name = %tc.name,
                input_json_len = tc.input_json.len(),
                "handle_content_block_stop: TOOL_USE COMPLETE - Creating normalized entry"
            );

            // Parse accumulated input JSON
            let input: Option<Value> = serde_json::from_str(&tc.input_json).ok();
            let action_type = map_tool_to_action(&tc.name, input.as_ref());
            let content = format_tool_content(&tc.name, input.as_ref());

            // Handle Task tool - create SubagentSpawn entry
            if tc.name == "Task" {
                self.handle_task_tool_spawn(&tc, input.as_ref()).await;
            }

            let entry = NormalizedEntry {
                timestamp: Some(tc.started_at.clone()),
                entry_type: NormalizedEntryType::ToolUse {
                    tool_name: tc.name.clone(),
                    action_type,
                    status: ToolStatus::Success,
                },
                content,
            };

            // Update the existing timeline entry in-place if we have a log id.
            if let (Some(log_id), Some(pool), Some(tx)) =
                (tc.timeline_log_id, &self.db_pool, &self.broadcast_tx)
            {
                if let Err(e) = StatusManager::update_normalized_entry(
                    pool,
                    tx,
                    self.attempt_id,
                    log_id,
                    &entry,
                    Some(tc.name.clone()),
                )
                .await
                {
                    tracing::warn!(
                        attempt_id = %self.attempt_id,
                        tool_name = %tc.name,
                        error = %e,
                        "Failed to update tool_use entry in-place; falling back to append"
                    );
                    return ExtractResult::Normalized(Box::new(entry));
                }
                return ExtractResult::Skip;
            }

            return ExtractResult::Normalized(Box::new(entry));
        }

        tracing::debug!(
            attempt_id = %self.attempt_id,
            "handle_content_block_stop: No pending tool call"
        );
        ExtractResult::Skip
    }

    /// Handle Task tool spawn - create SubagentSpawn normalized entry
    async fn handle_task_tool_spawn(&self, tool_call: &PartialToolCall, input: Option<&Value>) {
        tracing::info!(
            attempt_id = %self.attempt_id,
            tool_use_id = %tool_call.id,
            "handle_task_tool_spawn: Processing Task tool spawn"
        );

        // Extract task description from input
        let task_desc = input
            .and_then(|v| v.get("prompt"))
            .and_then(|v| v.as_str())
            .unwrap_or("Subtask");

        tracing::info!(
            attempt_id = %self.attempt_id,
            tool_use_id = %tool_call.id,
            task_description = %task_desc,
            "handle_task_tool_spawn: Task tool detected - will spawn subagent"
        );

        // For now, we'll use a placeholder UUID since we don't have the child attempt yet
        // In the future, this will be replaced by the actual child_attempt_id
        // created by the orchestrator
        let placeholder_child_id = Uuid::nil();

        // Create SubagentSpawn normalized entry
        let spawn_entry = NewNormalizedEntry::SubagentSpawn(SubagentSpawn {
            child_attempt_id: placeholder_child_id,
            task_description: task_desc.to_string(),
            tool_use_id: tool_call.id.clone(),
            timestamp: chrono::Utc::now(),
            line_number: 0, // Will be tracked properly later
        });

        // Store the entry directly in database if we have access
        if let Some(ref pool) = self.db_pool {
            let entry_type = spawn_entry.entry_type();
            let line_number = spawn_entry.line_number() as i32;

            match serde_json::to_value(&spawn_entry) {
                Ok(entry_data) => {
                    let result: Result<Uuid, sqlx::Error> = sqlx::query_scalar(
                        r#"INSERT INTO normalized_log_entries
                           (attempt_id, raw_log_id, entry_type, entry_data, line_number)
                           VALUES ($1, $2, $3, $4, $5)
                           RETURNING id"#,
                    )
                    .bind(self.attempt_id)
                    .bind(None as Option<Uuid>)
                    .bind(entry_type)
                    .bind(entry_data)
                    .bind(line_number)
                    .fetch_one(pool)
                    .await;

                    match result {
                        Ok(id) => {
                            tracing::info!(
                                attempt_id = %self.attempt_id,
                                entry_id = %id,
                                tool_use_id = %tool_call.id,
                                "handle_task_tool_spawn: SubagentSpawn entry stored successfully"
                            );
                        }
                        Err(e) => {
                            tracing::error!(
                                attempt_id = %self.attempt_id,
                                error = %e,
                                "handle_task_tool_spawn: Failed to store SubagentSpawn entry in database"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        attempt_id = %self.attempt_id,
                        error = %e,
                        "handle_task_tool_spawn: Failed to serialize SubagentSpawn entry"
                    );
                }
            }
        } else {
            tracing::warn!(
                attempt_id = %self.attempt_id,
                "handle_task_tool_spawn: No database pool available, cannot store SubagentSpawn entry"
            );
        }
    }
}

#[async_trait::async_trait]
impl ProtocolHandler for ClaudeAgentClient {
    async fn on_can_use_tool(
        &self,
        tool_name: String,
        input: Value,
        _permission_suggestions: Option<Vec<PermissionUpdate>>,
        tool_use_id: Option<String>,
    ) -> Result<PermissionResult> {
        if self.auto_approve {
            // No approval service - auto-approve all tools
            tracing::debug!(tool_name = %tool_name, "Auto-approving tool (no approval service)");
            return Ok(PermissionResult::Allow {
                updated_input: input,
                updated_permissions: None,
            });
        }

        // Get tool_use_id for tracking
        let tool_use_id = match tool_use_id {
            Some(id) => id,
            None => {
                // No tool_use_id - fallback to auto-approve
                tracing::warn!(
                    tool_name = %tool_name,
                    "No tool_use_id available, cannot request approval - auto-approving"
                );
                return Ok(PermissionResult::Allow {
                    updated_input: input,
                    updated_permissions: None,
                });
            }
        };

        // Request approval from service
        let Some(approval_service) = self.approval_service.as_ref() else {
            tracing::error!(
                tool_name = %tool_name,
                tool_use_id = %tool_use_id,
                "Approval service unavailable while auto_approve is disabled; denying tool call"
            );
            self.emit_tool_status_entry(
                &tool_name,
                &input,
                ToolStatus::Denied {
                    reason: Some("Approval service unavailable".to_string()),
                },
                None,
            )
            .await;
            return Ok(PermissionResult::Deny {
                message: "Approval service unavailable".to_string(),
                interrupt: Some(false),
            });
        };

        tracing::info!(
            tool_name = %tool_name,
            tool_use_id = %tool_use_id,
            "Requesting tool approval from user"
        );

        let requested_at = chrono::Utc::now();
        let timeout_at = requested_at + chrono::Duration::minutes(5);
        self.emit_tool_status_entry(
            &tool_name,
            &input,
            ToolStatus::PendingApproval {
                approval_id: tool_use_id.clone(),
                requested_at: requested_at.to_rfc3339(),
                timeout_at: timeout_at.to_rfc3339(),
            },
            Some(format!("Waiting for approval: {}", tool_name)),
        )
        .await;

        match approval_service
            .request_tool_approval(self.attempt_id, &tool_name, input.clone(), &tool_use_id)
            .await
        {
            Ok(ApprovalStatus::Approved) => {
                tracing::info!(tool_name = %tool_name, tool_use_id = %tool_use_id, "Tool approved");
                Ok(PermissionResult::Allow {
                    updated_input: input,
                    updated_permissions: None,
                })
            }
            Ok(ApprovalStatus::Denied { reason }) => {
                let message = reason.unwrap_or_else(|| "Denied by user".to_string());
                tracing::info!(
                    tool_name = %tool_name,
                    tool_use_id = %tool_use_id,
                    reason = %message,
                    "Tool denied"
                );
                self.emit_tool_status_entry(
                    &tool_name,
                    &input,
                    ToolStatus::Denied {
                        reason: Some(message.clone()),
                    },
                    None,
                )
                .await;
                Ok(PermissionResult::Deny {
                    message,
                    interrupt: Some(false), // Don't interrupt on deny
                })
            }
            Ok(ApprovalStatus::TimedOut) => {
                tracing::warn!(
                    tool_name = %tool_name,
                    tool_use_id = %tool_use_id,
                    "Tool approval timed out"
                );
                self.emit_tool_status_entry(
                    &tool_name,
                    &input,
                    ToolStatus::TimedOut,
                    Some(format!("Approval timed out: {}", tool_name)),
                )
                .await;
                Ok(PermissionResult::Deny {
                    message: "Approval request timed out".to_string(),
                    interrupt: Some(false),
                })
            }
            Ok(ApprovalStatus::Pending) => {
                // This shouldn't happen (request_tool_approval waits for resolution)
                tracing::error!(
                    tool_name = %tool_name,
                    tool_use_id = %tool_use_id,
                    "Approval still pending after request (unexpected)"
                );
                Ok(PermissionResult::Deny {
                    message: "Approval still pending (unexpected state)".to_string(),
                    interrupt: Some(false),
                })
            }
            Err(e) => {
                tracing::error!(
                    tool_name = %tool_name,
                    tool_use_id = %tool_use_id,
                    error = %e,
                    "Tool approval request failed"
                );
                Ok(PermissionResult::Deny {
                    message: format!("Tool approval request failed: {}", e),
                    interrupt: Some(false),
                })
            }
        }
    }

    async fn on_hook_callback(
        &self,
        callback_id: String,
        _input: Value,
        _tool_use_id: Option<String>,
    ) -> Result<Value> {
        // Hook callbacks can be used for plan mode approval
        // For now, we auto-approve (hooks not implemented yet)
        tracing::debug!(callback_id = %callback_id, "Auto-approving hook callback");
        Ok(serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "allow",
                "permissionDecisionReason": "Auto-approved by SDK"
            }
        }))
    }

    async fn on_non_control(&self, line: &str) -> Result<()> {
        // Parse SDK protocol message to extract human-readable content
        let extract_result = self.extract_sdk_content(line).await;

        // Save to database if available (primary storage)
        // Only save actual content, skip protocol noise
        if let (Some(pool), Some(tx)) = (&self.db_pool, &self.broadcast_tx) {
            match &extract_result {
                ExtractResult::Content(content) => {
                    let _ = (pool, tx);
                    self.emit_runtime_capable_assistant_content(content).await;
                }
                ExtractResult::Normalized(entry) => {
                    // Save normalized entry as JSON for vibe-kanban style display
                    match serde_json::to_string(entry) {
                        Ok(json) => {
                            tracing::info!(
                                attempt_id = %self.attempt_id,
                                entry_type = ?entry.entry_type,
                                json_len = json.len(),
                                "on_non_control: SAVING NORMALIZED ENTRY to database"
                            );
                            let result =
                                StatusManager::log(pool, tx, self.attempt_id, "normalized", &json)
                                    .await;
                            if let Err(e) = result {
                                tracing::error!(
                                    attempt_id = %self.attempt_id,
                                    error = %e,
                                    "on_non_control: FAILED to save normalized entry"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                attempt_id = %self.attempt_id,
                                error = %e,
                                "on_non_control: Failed to serialize normalized entry"
                            );
                        }
                    }
                }
                ExtractResult::AssistantMessageStart | ExtractResult::AssistantMessageStop => {
                    if matches!(extract_result, ExtractResult::AssistantMessageStop) {
                        self.flush_runtime_capable_assistant_content().await;
                    }
                    StatusManager::reset_assistant_accumulator(self.attempt_id).await;
                }
                ExtractResult::Skip => {
                    // Skip protocol noise - don't save to database
                }
                ExtractResult::Unknown => {
                    // Unknown format - log as raw for debugging (non-JSON stderr etc)
                    tracing::debug!(
                        attempt_id = %self.attempt_id,
                        line_preview = %line.chars().take(80).collect::<String>(),
                        "on_non_control: Unknown format, saving as stdout"
                    );
                    let _ = StatusManager::log(pool, tx, self.attempt_id, "stdout", line).await;
                }
            }
        } else {
            tracing::warn!(
                attempt_id = %self.attempt_id,
                has_db_pool = self.db_pool.is_some(),
                has_broadcast_tx = self.broadcast_tx.is_some(),
                "on_non_control: Missing db_pool or broadcast_tx, cannot save to database"
            );
            if let ExtractResult::Content(content) = &extract_result {
                self.emit_runtime_capable_assistant_content(content).await;
            } else if matches!(extract_result, ExtractResult::AssistantMessageStop) {
                self.flush_runtime_capable_assistant_content().await;
            }
        }

        // Extract metadata-style SDK entries (token usage, next action, user Q/A) from raw event line.
        self.emit_metadata_entries(line).await;

        // Also write to LogWriter (for potential file output or debugging)
        self.log_writer.log_raw(line).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_sdk_normalized_entry;

    #[test]
    fn parse_token_usage_from_message_stop() {
        let line = r#"{
            "type":"message_stop",
            "message":{"usage":{"input_tokens":210,"output_tokens":45,"total_tokens":255}}
        }"#;

        let entries = parse_claude_metadata_entries(line);
        assert_eq!(entries.len(), 1);
        match &entries[0].entry_type {
            NormalizedEntryType::TokenUsageInfo {
                input_tokens,
                output_tokens,
                total_tokens,
                model_context_window,
            } => {
                assert_eq!(*input_tokens, 210);
                assert_eq!(*output_tokens, 45);
                assert_eq!(*total_tokens, Some(255));
                assert_eq!(*model_context_window, None);
            }
            other => panic!("unexpected entry type: {:?}", other),
        }
    }

    #[test]
    fn parse_token_usage_extracts_model_context_window() {
        let line = r#"{
            "type":"message_stop",
            "message":{
                "usage":{
                    "input_tokens":210,
                    "output_tokens":45,
                    "total_tokens":255,
                    "model_context_window":"200000"
                }
            }
        }"#;

        let entries = parse_claude_metadata_entries(line);
        assert_eq!(entries.len(), 1);
        match &entries[0].entry_type {
            NormalizedEntryType::TokenUsageInfo {
                input_tokens,
                output_tokens,
                total_tokens,
                model_context_window,
            } => {
                assert_eq!(*input_tokens, 210);
                assert_eq!(*output_tokens, 45);
                assert_eq!(*total_tokens, Some(255));
                assert_eq!(*model_context_window, Some(200_000));
            }
            other => panic!("unexpected entry type: {:?}", other),
        }
    }

    #[test]
    fn derive_next_action_from_stop_reason() {
        let line = r#"{"type":"message_stop","message":{"stop_reason":"tool_use"}}"#;
        let entries = parse_claude_metadata_entries(line);
        assert_eq!(entries.len(), 1);
        match &entries[0].entry_type {
            NormalizedEntryType::NextAction { text } => {
                assert!(text.to_ascii_lowercase().contains("tool"));
            }
            other => panic!("unexpected entry type: {:?}", other),
        }
    }

    #[test]
    fn parse_user_answered_questions_payload() {
        let line = r#"{
            "type":"stream_event",
            "event":{"input":{"question":"Continue with deploy?","answer":"yes"}}
        }"#;
        let entries = parse_claude_metadata_entries(line);
        assert_eq!(entries.len(), 1);
        match &entries[0].entry_type {
            NormalizedEntryType::UserAnsweredQuestions { question, answer } => {
                assert_eq!(question, "Continue with deploy?");
                assert_eq!(answer, "yes");
            }
            other => panic!("unexpected entry type: {:?}", other),
        }
    }

    #[test]
    fn parse_combined_metadata_entries_with_camel_case_variants() {
        let line = r#"{
            "type":"stream_event",
            "nextAction":"Run integration tests",
            "usage":{"inputTokens":"12","outputTokens":"7","totalTokens":"19"},
            "question_answer":{"prompt":"Ship now?","response":"later"}
        }"#;

        let entries = parse_claude_metadata_entries(line);
        assert_eq!(entries.len(), 3);

        assert!(entries.iter().any(|entry| {
            matches!(
                entry.entry_type,
                NormalizedEntryType::TokenUsageInfo {
                    input_tokens: 12,
                    output_tokens: 7,
                    total_tokens: Some(19),
                    ..
                }
            )
        }));

        assert!(entries.iter().any(|entry| {
            matches!(
                entry.entry_type,
                NormalizedEntryType::NextAction { ref text } if text == "Run integration tests"
            )
        }));

        assert!(entries.iter().any(|entry| {
            matches!(
                entry.entry_type,
                NormalizedEntryType::UserAnsweredQuestions { ref question, ref answer }
                    if question == "Ship now?" && answer == "later"
            )
        }));
    }

    #[test]
    fn parse_next_action_prefers_explicit_text_over_stop_reason_fallback() {
        let line = r#"{
            "type":"message_stop",
            "message":{"next_action":"Explicit next step","stop_reason":"tool_use"}
        }"#;

        let entries = parse_claude_metadata_entries(line);
        assert_eq!(entries.len(), 1);
        match &entries[0].entry_type {
            NormalizedEntryType::NextAction { text } => {
                assert_eq!(text, "Explicit next step");
            }
            other => panic!("unexpected entry type: {:?}", other),
        }
    }

    #[test]
    fn parse_user_answered_questions_accepts_decision_alias() {
        let line = r#"{
            "type":"stream_event",
            "event":{"input":{"question":"Proceed with rebase?","decision":"no"}}
        }"#;

        let entries = parse_claude_metadata_entries(line);
        assert_eq!(entries.len(), 1);
        match &entries[0].entry_type {
            NormalizedEntryType::UserAnsweredQuestions { question, answer } => {
                assert_eq!(question, "Proceed with rebase?");
                assert_eq!(answer, "no");
            }
            other => panic!("unexpected entry type: {:?}", other),
        }
    }

    #[test]
    fn parse_token_usage_ignores_zero_only_counts() {
        let line = r#"{
            "type":"message_stop",
            "message":{"usage":{"input_tokens":0,"output_tokens":0,"total_tokens":0}}
        }"#;

        let entries = parse_claude_metadata_entries(line);
        assert!(
            entries.is_empty(),
            "expected no metadata entries for zero-only usage payload"
        );
    }

    #[test]
    fn parse_next_action_derives_pause_turn_and_max_tokens_stop_reasons() {
        let pause_line = r#"{"type":"message_stop","message":{"stop_reason":"pause_turn"}}"#;
        let max_tokens_line = r#"{"type":"message_stop","message":{"stop_reason":"max_tokens"}}"#;

        let pause_entries = parse_claude_metadata_entries(pause_line);
        let max_entries = parse_claude_metadata_entries(max_tokens_line);

        match &pause_entries[0].entry_type {
            NormalizedEntryType::NextAction { text } => {
                assert!(text.to_ascii_lowercase().contains("follow-up"));
            }
            other => panic!("unexpected pause_turn entry type: {:?}", other),
        }

        match &max_entries[0].entry_type {
            NormalizedEntryType::NextAction { text } => {
                assert!(text.to_ascii_lowercase().contains("continue generation"));
            }
            other => panic!("unexpected max_tokens entry type: {:?}", other),
        }
    }

    #[test]
    fn parse_user_answered_questions_accepts_prompt_response_aliases() {
        let line = r#"{
            "type":"stream_event",
            "question_answer":{"prompt":"Ship this now?","response":"not yet"}
        }"#;

        let entries = parse_claude_metadata_entries(line);
        assert_eq!(entries.len(), 1);
        match &entries[0].entry_type {
            NormalizedEntryType::UserAnsweredQuestions { question, answer } => {
                assert_eq!(question, "Ship this now?");
                assert_eq!(answer, "not yet");
            }
            other => panic!("unexpected entry type: {:?}", other),
        }
    }

    #[test]
    fn parse_user_answered_questions_accepts_data_question_answer_alias() {
        let line = r#"{
            "type":"stream_event",
            "data":{"question_answer":{"question":"Deploy now?","answer":"after QA"}}
        }"#;

        let entries = parse_claude_metadata_entries(line);
        assert_eq!(entries.len(), 1);
        match &entries[0].entry_type {
            NormalizedEntryType::UserAnsweredQuestions { question, answer } => {
                assert_eq!(question, "Deploy now?");
                assert_eq!(answer, "after QA");
            }
            other => panic!("unexpected entry type: {:?}", other),
        }
    }

    #[test]
    fn parse_next_action_accepts_data_alias() {
        let line = r#"{
            "type":"stream_event",
            "data":{"nextAction":"Request approval before deploy"}
        }"#;

        let entries = parse_claude_metadata_entries(line);
        assert_eq!(entries.len(), 1);
        match &entries[0].entry_type {
            NormalizedEntryType::NextAction { text } => {
                assert_eq!(text, "Request approval before deploy");
            }
            other => panic!("unexpected entry type: {:?}", other),
        }
    }

    #[test]
    fn parse_metadata_entries_emit_contract_valid_entries() {
        let line = r#"{
            "type":"stream_event",
            "nextAction":"Wait for QA sign-off",
            "usage":{"inputTokens":"8","outputTokens":"3","totalTokens":"11"},
            "question_answer":{"question":"Deploy now?","answer":"tomorrow morning"}
        }"#;

        let entries = parse_claude_metadata_entries(line);
        assert_eq!(entries.len(), 3);
        for entry in entries {
            assert!(
                validate_sdk_normalized_entry(&entry).is_ok(),
                "expected contract-valid metadata entry"
            );
        }
    }

    #[test]
    fn parse_metadata_entries_ignores_blank_or_incomplete_payloads() {
        let line = r#"{
            "type":"stream_event",
            "nextAction":"   ",
            "usage":{"input_tokens":0,"output_tokens":0,"total_tokens":0},
            "question_answer":{"question":"Deploy now?","answer":"   "}
        }"#;

        let entries = parse_claude_metadata_entries(line);
        assert!(
            entries.is_empty(),
            "expected no entries for blank/incomplete metadata payloads"
        );
    }

    #[test]
    fn parse_realistic_claude_metadata_fixture_lines() {
        let fixture = include_str!("../test-fixtures/claude_metadata_realistic.jsonl");
        let entries: Vec<NormalizedEntry> = fixture
            .lines()
            .filter(|line| !line.trim().is_empty())
            .flat_map(parse_claude_metadata_entries)
            .collect();

        assert!(entries.iter().any(|entry| {
            matches!(
                entry.entry_type,
                NormalizedEntryType::TokenUsageInfo {
                    input_tokens: 321,
                    output_tokens: 89,
                    total_tokens: Some(410),
                    ..
                }
            )
        }));

        assert!(entries.iter().any(|entry| {
            matches!(
                entry.entry_type,
                NormalizedEntryType::NextAction { ref text }
                    if text == "Run migration dry-run before apply"
            )
        }));

        assert!(entries.iter().any(|entry| {
            matches!(
                entry.entry_type,
                NormalizedEntryType::UserAnsweredQuestions { ref question, ref answer }
                    if question == "Apply DB migration now?" && answer == "after backup"
            )
        }));

        assert!(entries.iter().any(|entry| {
            matches!(
                entry.entry_type,
                NormalizedEntryType::UserAnsweredQuestions { ref question, ref answer }
                    if question == "Ship to production?" && answer == "not today"
            )
        }));

        for entry in entries {
            assert!(
                validate_sdk_normalized_entry(&entry).is_ok(),
                "fixture metadata entry must be contract-valid"
            );
        }
    }
}
