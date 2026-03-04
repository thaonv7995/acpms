//! JSONL log storage for Project Assistant chat sessions.
//! Path: {ACPMS_LOG_DIR}/assistant/{session_id}.jsonl
//! Format: { id, session_id, role, content, metadata, created_at }

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

fn log_dir() -> PathBuf {
    std::env::var("ACPMS_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("acpms-logs"))
}

/// Path to the JSONL log file for an assistant session.
pub fn get_assistant_log_file_path(session_id: Uuid) -> PathBuf {
    log_dir()
        .join("assistant")
        .join(format!("{}.jsonl", session_id))
}

/// Parsed message from assistant JSONL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Process agent text: extract tool call JSON lines. Returns (content_without_json, metadata).
/// Used when text is a complete line (e.g. ClaudeCode raw stdout).
pub fn process_agent_text_for_tool_calls(text: &str) -> (String, Option<serde_json::Value>) {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut content_parts = Vec::new();
    let mut metadata = None;
    for line in lines {
        if let Some(m) = parse_tool_call_metadata(line) {
            metadata = Some(m);
        } else if let Some(m) = extract_tool_call_from_streaming_text(line) {
            metadata = Some(m);
        } else {
            content_parts.push(line);
        }
    }
    let content = content_parts.join("\n").trim().to_string();
    (content, metadata)
}

/// Buffer for streaming agent text. Accumulates chunks and extracts complete tool call JSON
/// even when split across multiple chunks.
#[derive(Default)]
pub struct AgentTextBuffer {
    buffer: String,
}

impl AgentTextBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a text chunk. Call pop_next() to get emitted content/metadata.
    pub fn push(&mut self, text: &str) {
        self.buffer.push_str(text);
    }

    /// Pop the next (content, metadata) if we have a complete tool call or complete line.
    pub fn pop_next(&mut self) -> Option<(String, Option<serde_json::Value>)> {
        if let Some(out) = self.try_extract_tool_call() {
            return Some(out);
        }
        if let Some((line, _)) = self.try_take_complete_line() {
            return Some((line, None));
        }
        None
    }

    /// Flush remaining buffer as content (no tool call). Call when stream ends.
    pub fn flush(&mut self) -> Option<(String, Option<serde_json::Value>)> {
        let content = std::mem::take(&mut self.buffer);
        let trimmed = content.trim().to_string();
        if trimmed.is_empty() {
            return None;
        }
        Some((trimmed, None))
    }

    /// Emit a partial text chunk even when there is no trailing newline.
    ///
    /// This is useful for providers that stream assistant text fragments without
    /// newline terminators. We keep possible tool-call JSON fragments in the buffer
    /// to avoid breaking metadata extraction.
    pub fn pop_partial_text_for_display(&mut self) -> Option<(String, Option<serde_json::Value>)> {
        let trimmed = self.buffer.trim();
        if trimmed.is_empty() {
            self.buffer.clear();
            return None;
        }

        for prefix in [
            r#"{"tool":"create_task""#,
            r#"{"tool":"create_requirement""#,
        ] {
            if let Some(start) = trimmed.find(prefix) {
                if start == 0 {
                    // The buffer starts with an incomplete tool-call candidate; keep it.
                    return None;
                }

                // Emit safe text before tool-call fragment and keep fragment buffered.
                let content_before = trimmed[..start].trim().to_string();
                self.buffer = trimmed[start..].to_string();
                if content_before.is_empty() {
                    return None;
                }
                return Some((content_before, None));
            }
        }

        let content = trimmed.to_string();
        self.buffer.clear();
        Some((content, None))
    }

    fn try_take_complete_line(&mut self) -> Option<(String, Option<serde_json::Value>)> {
        if let Some(pos) = self.buffer.find('\n') {
            let line = self.buffer[..pos].trim().to_string();
            self.buffer = self.buffer[pos + 1..].to_string();
            if line.is_empty() {
                return self.try_take_complete_line();
            }
            if parse_tool_call_metadata(&line).is_some() {
                self.buffer = format!("{}\n{}", line, self.buffer);
                return None;
            }
            return Some((line, None));
        }
        None
    }

    fn try_extract_tool_call(&mut self) -> Option<(String, Option<serde_json::Value>)> {
        let trimmed = self.buffer.trim();
        for prefix in [
            r#"{"tool":"create_task""#,
            r#"{"tool":"create_requirement""#,
        ] {
            if let Some(start) = trimmed.find(prefix) {
                let slice = &trimmed[start..];
                if let Some(end) = find_matching_json_end(slice) {
                    let json_str = &slice[..=end];
                    if let Some(metadata) = parse_tool_call_metadata(json_str) {
                        let content_before = trimmed[..start].trim().to_string();
                        let after_end = start + end + 1;
                        self.buffer = if after_end < trimmed.len() {
                            trimmed[after_end..].trim().to_string()
                        } else {
                            String::new()
                        };
                        return Some((content_before, Some(metadata)));
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pop_partial_text_for_display_emits_without_newline() {
        let mut buffer = AgentTextBuffer::new();
        buffer.push("Hello from codex");

        let out = buffer.pop_partial_text_for_display();
        assert_eq!(out, Some(("Hello from codex".to_string(), None)));
        assert_eq!(buffer.pop_partial_text_for_display(), None);
    }

    #[test]
    fn pop_partial_text_for_display_keeps_tool_call_fragment() {
        let mut buffer = AgentTextBuffer::new();
        buffer.push(r#"{"tool":"create_task","args":{"title":"Do A""#);

        let out = buffer.pop_partial_text_for_display();
        assert_eq!(out, None);
    }

    #[test]
    fn pop_partial_text_for_display_emits_prefix_before_tool_call_fragment() {
        let mut buffer = AgentTextBuffer::new();
        buffer.push(r#"Here is the plan {"tool":"create_task","args":{"title":"Do A""#);

        let out = buffer.pop_partial_text_for_display();
        assert_eq!(out, Some(("Here is the plan".to_string(), None)));
    }
}

/// Extract complete tool call JSON from text that may be streamed (partial) or embedded.
fn extract_tool_call_from_streaming_text(text: &str) -> Option<serde_json::Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    for prefix in [
        r#"{"tool":"create_task""#,
        r#"{"tool":"create_requirement""#,
    ] {
        if let Some(start) = trimmed.find(prefix) {
            let slice = &trimmed[start..];
            if let Some(end) = find_matching_json_end(slice) {
                let json_str = &slice[..=end];
                if let Some(m) = parse_tool_call_metadata(json_str) {
                    return Some(m);
                }
            }
        }
    }
    None
}

/// Find the index of the final '}' that closes the root JSON object.
fn find_matching_json_end(s: &str) -> Option<usize> {
    let mut depth = 0u32;
    let mut i = 0;
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    while i < n {
        let c = chars[i];
        if c == '"' {
            i += 1;
            while i < n {
                let x = chars[i];
                if x == '\\' {
                    i += 2;
                    continue;
                }
                if x == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if c == '{' {
            depth += 1;
        } else if c == '}' {
            if depth == 1 {
                return Some(i);
            }
            depth = depth.saturating_sub(1);
        }
        i += 1;
    }
    None
}

/// Detect if a line is a tool call JSON ({"tool":"create_requirement"|"create_task","args":{...}}).
/// Returns metadata with tool_calls for append_assistant_log if valid.
pub fn parse_tool_call_metadata(line: &str) -> Option<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let obj = v.as_object()?;
    let tool = obj.get("tool")?.as_str()?;
    if tool != "create_requirement" && tool != "create_task" {
        return None;
    }
    let args = obj.get("args")?.as_object()?;
    if args.get("title").and_then(|x| x.as_str()).is_none() {
        return None;
    }
    let id = format!("tc_{}", Uuid::new_v4());
    let tool_calls = serde_json::json!([{
        "id": id,
        "name": tool,
        "args": obj.get("args").cloned().unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
    }]);
    Some(serde_json::json!({ "tool_calls": tool_calls }))
}

/// Append a single log entry to assistant JSONL file.
pub async fn append_assistant_log(
    session_id: Uuid,
    role: &str,
    content: &str,
    metadata: Option<&serde_json::Value>,
) -> Result<Uuid> {
    let id = Uuid::new_v4();
    let created_at = Utc::now();

    let dir = log_dir().join("assistant");
    tokio::fs::create_dir_all(&dir)
        .await
        .context("Failed to create assistant log dir")?;

    let path = dir.join(format!("{}.jsonl", session_id));
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .context("Failed to open assistant log file for append")?;

    let line = serde_json::json!({
        "id": id,
        "session_id": session_id,
        "role": role,
        "content": content,
        "metadata": metadata,
        "created_at": created_at,
    });
    let s = serde_json::to_string(&line).context("Failed to serialize assistant log entry")?;
    tokio::io::AsyncWriteExt::write_all(&mut file, s.as_bytes()).await?;
    tokio::io::AsyncWriteExt::write_all(&mut file, b"\n").await?;

    Ok(id)
}

/// Read full JSONL log file content. Returns empty vec if file does not exist.
pub async fn read_assistant_log_file(session_id: Uuid) -> Result<Vec<u8>> {
    let path = get_assistant_log_file_path(session_id);
    match tokio::fs::read(&path).await {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

/// Parse JSONL bytes to AssistantMessages. Skips invalid lines.
pub fn parse_jsonl_to_messages(bytes: &[u8]) -> Vec<AssistantMessage> {
    let mut messages = Vec::new();
    for line in bytes.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let v: serde_json::Value = match serde_json::from_slice(line) {
            Ok(x) => x,
            _ => continue,
        };
        let id = match v
            .get("id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            Some(x) => x,
            None => continue,
        };
        let session_id = match v
            .get("session_id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            Some(x) => x,
            None => continue,
        };
        let role = match v.get("role").and_then(|x| x.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let content = match v.get("content").and_then(|x| x.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let metadata = v.get("metadata").cloned().filter(|m| !m.is_null());
        let created_at = match v
            .get("created_at")
            .and_then(|x| x.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        {
            Some(dt) => dt.with_timezone(&Utc),
            None => continue,
        };
        messages.push(AssistantMessage {
            id,
            session_id,
            role,
            content,
            metadata,
            created_at,
        });
    }
    messages.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    messages
}
