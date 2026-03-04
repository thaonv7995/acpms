//! Google Gemini CLI client for spawning and managing agent sessions.
//!
//! Minimal integration:
//! - Spawn `gemini -p` in a worktree
//! - Use `--yolo` to auto-approve tool usage (non-interactive)
//! - Stream stdout/stderr into the attempt log pipeline

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use command_group::{AsyncCommandGroup, AsyncGroupChild};
use regex::Regex;
use serde_json::Value;
use std::path::PathBuf;
use tokio::process::Command;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::claude::SpawnedAgent;
use crate::process::{InterruptReceiver, InterruptSender};

pub struct GeminiClient;

const EXEC_GEMINI_CMD_ENV: &str = "ACPMS_EXEC_GEMINI_CMD";
const EXEC_GEMINI_USE_NPX_ENV: &str = "ACPMS_EXEC_GEMINI_USE_NPX";
const OVERRIDE_GEMINI_BIN_ENV: &str = "ACPMS_AGENT_GEMINI_BIN";
const OVERRIDE_NPX_BIN_ENV: &str = "ACPMS_AGENT_NPX_BIN";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeminiStreamEvent {
    AgentMessage {
        text: String,
        is_final: bool,
    },
    /// type: "message", role: "user" — skip, don't log raw
    Skip,
    /// type: "tool_use" — chat-format tool call (run_shell_command, etc.)
    ToolUseStarted {
        tool_id: String,
        tool_name: String,
        payload: String,
    },
    /// type: "tool_result" — chat-format tool result
    ToolResult {
        tool_id: String,
        success: bool,
        output: Option<String>,
    },
    CommandCompleted {
        command: String,
        exit_code: Option<i32>,
        output: Option<String>,
    },
    FileChanged {
        path: String,
        kind: String,
    },
    TokenUsage {
        input_tokens: u64,
        output_tokens: u64,
        total_tokens: Option<u64>,
        model_context_window: Option<u64>,
    },
    NextAction {
        text: String,
    },
    UserAnsweredQuestions {
        question: String,
        answer: String,
    },
}

#[derive(Debug, Clone)]
struct CommandResolution {
    command: String,
    use_npx: bool,
}

fn read_non_empty_env(var: &str) -> Option<String> {
    let value = std::env::var(var).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn read_non_empty_map_value(map: &HashMap<String, String>, key: &str) -> Option<String> {
    map.get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn is_truthy(value: Option<&String>) -> bool {
    value
        .map(|v| v.trim().to_ascii_lowercase())
        .map(|v| !matches!(v.as_str(), "0" | "false" | "off" | "no" | ""))
        .unwrap_or(false)
}

fn is_executable_file(path: &std::path::Path) -> bool {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn resolve_command_in_path(command: &str) -> Option<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains(std::path::MAIN_SEPARATOR) {
        let path = PathBuf::from(trimmed);
        if is_executable_file(&path) {
            return Some(path.to_string_lossy().to_string());
        }
        return None;
    }

    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(trimmed);
        if is_executable_file(&candidate) {
            return Some(candidate.to_string_lossy().to_string());
        }
    }

    None
}

fn resolve_command_with_override(default_cmd: &str, override_env: &str) -> Option<String> {
    if let Some(override_cmd) = read_non_empty_env(override_env) {
        return resolve_command_in_path(&override_cmd);
    }
    resolve_command_in_path(default_cmd)
}

fn resolve_npx_command() -> Option<String> {
    if let Some(override_cmd) = read_non_empty_env(OVERRIDE_NPX_BIN_ENV) {
        return resolve_command_in_path(&override_cmd);
    }
    resolve_command_in_path("npx")
}

fn resolve_gemini_command(env_vars: Option<&HashMap<String, String>>) -> Option<CommandResolution> {
    if let Some(vars) = env_vars {
        if let Some(command) = read_non_empty_map_value(vars, EXEC_GEMINI_CMD_ENV) {
            return Some(CommandResolution {
                command,
                use_npx: is_truthy(vars.get(EXEC_GEMINI_USE_NPX_ENV)),
            });
        }
    }

    if let Some(command) = resolve_command_with_override("gemini", OVERRIDE_GEMINI_BIN_ENV) {
        return Some(CommandResolution {
            command,
            use_npx: false,
        });
    }
    resolve_npx_command().map(|command| CommandResolution {
        command,
        use_npx: true,
    })
}

/// Split a line that may contain multiple concatenated JSON objects.
fn split_json_objects(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let normalized = trimmed.replace("}\n{", "}|{").replace("}{", "}|{");
    let normalized = Regex::new(r"}\s+{")
        .ok()
        .map(|re| re.replace_all(&normalized, "}|{").to_string())
        .unwrap_or(normalized);
    let parts: Vec<&str> = normalized.split('|').collect();
    if parts.len() == 1 {
        return vec![trimmed.to_string()];
    }
    parts
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect()
}

/// Parse chat-format events: type=message|tool_use|tool_result (Gemini CLI stream-json).
fn extract_gemini_chat_format(value: &Value) -> Option<Vec<GeminiStreamEvent>> {
    let obj = value.as_object()?;
    let t = obj.get("type").and_then(Value::as_str).unwrap_or("");

    match t {
        "message" => {
            let role = obj.get("role").and_then(Value::as_str).unwrap_or("");
            if role == "user" {
                return Some(vec![GeminiStreamEvent::Skip]);
            }
            if role == "assistant" {
                if let Some(content) = obj.get("content").and_then(Value::as_str) {
                    let text = content.to_string();
                    if !text.trim().is_empty() {
                        let is_final = !obj.get("delta").and_then(Value::as_bool).unwrap_or(false);
                        return Some(vec![GeminiStreamEvent::AgentMessage { text, is_final }]);
                    }
                }
            }
            Some(vec![GeminiStreamEvent::Skip])
        }
        "tool_use" => {
            let tool_id = obj.get("tool_id").and_then(Value::as_str).unwrap_or("");
            let tool_name = obj.get("tool_name").and_then(Value::as_str).unwrap_or("");
            let params = obj.get("parameters").and_then(Value::as_object);
            let payload = params
                .and_then(|p| {
                    p.get("command")
                        .and_then(Value::as_str)
                        .map(String::from)
                        .or_else(|| {
                            p.get("file_path")
                                .or_else(|| p.get("path"))
                                .and_then(Value::as_str)
                                .map(|s| serde_json::json!({"file_path": s}).to_string())
                        })
                })
                .unwrap_or_else(|| {
                    serde_json::to_string(params.unwrap_or(&serde_json::Map::new()))
                        .unwrap_or_default()
                });
            if tool_id.is_empty() || tool_name.is_empty() {
                return Some(vec![GeminiStreamEvent::Skip]);
            }
            Some(vec![GeminiStreamEvent::ToolUseStarted {
                tool_id: tool_id.to_string(),
                tool_name: tool_name.to_string(),
                payload,
            }])
        }
        "tool_result" => {
            let tool_id = obj.get("tool_id").and_then(Value::as_str).unwrap_or("");
            if tool_id.is_empty() {
                return None;
            }
            let status = obj.get("status").and_then(Value::as_str).unwrap_or("");
            let success = status.to_ascii_lowercase() == "success";
            let output = obj.get("output").and_then(Value::as_str).map(String::from);
            Some(vec![GeminiStreamEvent::ToolResult {
                tool_id: tool_id.to_string(),
                success,
                output,
            }])
        }
        "result" => Some(vec![GeminiStreamEvent::Skip]),
        _ => None,
    }
}

/// Parse one Gemini stream-json line into structured events for timeline normalization.
pub fn parse_gemini_json_events(line: &str) -> Vec<GeminiStreamEvent> {
    let mut all = Vec::new();
    let objects = split_json_objects(line);

    for obj_str in objects {
        let value: Value = match serde_json::from_str(&obj_str) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(chat_events) = extract_gemini_chat_format(&value) {
            all.extend(chat_events);
            continue;
        }

        let mut events = Vec::new();
        events.extend(extract_gemini_command_event(&value));
        events.extend(extract_gemini_file_change_events(&value));
        events.extend(extract_gemini_meta_events(&value));

        let texts = extract_gemini_text_fragments(&value);
        if !texts.is_empty() {
            let is_final = infer_gemini_final_message(&value);
            let text_count = texts.len();
            for (idx, text) in texts.into_iter().enumerate() {
                let trimmed = text.trim().to_string();
                if trimmed.is_empty() {
                    continue;
                }
                let final_fragment = is_final && idx + 1 == text_count;
                events.push(GeminiStreamEvent::AgentMessage {
                    text: trimmed,
                    is_final: final_fragment,
                });
            }
        }

        all.extend(events);
    }

    all
}

fn extract_gemini_meta_events(value: &Value) -> Vec<GeminiStreamEvent> {
    let mut events = Vec::new();

    let usage = value
        .pointer("/response/usage")
        .or_else(|| value.pointer("/usage"))
        .or_else(|| value.pointer("/data/usage"))
        .or_else(|| value.pointer("/event/usage"))
        .and_then(Value::as_object);

    if let Some(usage) = usage {
        let input_tokens = usage
            .get("input_tokens")
            .or_else(|| usage.get("prompt_tokens"))
            .or_else(|| usage.get("inputTokenCount"))
            .or_else(|| usage.get("inputTokens"))
            .and_then(value_as_u64)
            .unwrap_or(0);

        let output_tokens = usage
            .get("output_tokens")
            .or_else(|| usage.get("completion_tokens"))
            .or_else(|| usage.get("outputTokenCount"))
            .or_else(|| usage.get("outputTokens"))
            .or_else(|| usage.get("candidates_token_count"))
            .and_then(value_as_u64)
            .unwrap_or(0);

        let total_tokens = usage
            .get("total_tokens")
            .or_else(|| usage.get("totalTokenCount"))
            .or_else(|| usage.get("totalTokens"))
            .and_then(value_as_u64);
        let model_context_window = extract_model_context_window(value, Some(usage));

        if input_tokens > 0 || output_tokens > 0 || total_tokens.unwrap_or(0) > 0 {
            events.push(GeminiStreamEvent::TokenUsage {
                input_tokens,
                output_tokens,
                total_tokens,
                model_context_window,
            });
        }
    }

    if let Some(text) = value
        .pointer("/response/next_action")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .pointer("/response/nextAction")
                .and_then(Value::as_str)
        })
        .or_else(|| value.pointer("/next_action").and_then(Value::as_str))
        .or_else(|| value.pointer("/nextAction").and_then(Value::as_str))
        .or_else(|| value.pointer("/data/next_action").and_then(Value::as_str))
        .or_else(|| value.pointer("/data/nextAction").and_then(Value::as_str))
    {
        let text = text.trim().to_string();
        if !text.is_empty() {
            events.push(GeminiStreamEvent::NextAction { text });
        }
    }

    if let Some((question, answer)) = extract_user_answered_questions(value) {
        events.push(GeminiStreamEvent::UserAnsweredQuestions { question, answer });
    }

    events
}

fn extract_user_answered_questions(value: &Value) -> Option<(String, String)> {
    for (question_ptr, answer_ptr) in [
        (
            "/response/user_answered_questions/question",
            "/response/user_answered_questions/answer",
        ),
        (
            "/response/userAnsweredQuestions/question",
            "/response/userAnsweredQuestions/answer",
        ),
        (
            "/user_answered_questions/question",
            "/user_answered_questions/answer",
        ),
        (
            "/userAnsweredQuestions/question",
            "/userAnsweredQuestions/answer",
        ),
        (
            "/data/user_answered_questions/question",
            "/data/user_answered_questions/answer",
        ),
        (
            "/data/userAnsweredQuestions/question",
            "/data/userAnsweredQuestions/answer",
        ),
        ("/response/question", "/response/answer"),
        ("/question", "/answer"),
        ("/data/question", "/data/answer"),
    ] {
        let question = value.pointer(question_ptr).and_then(Value::as_str);
        let answer = value.pointer(answer_ptr).and_then(Value::as_str);
        let Some(question) = question else {
            continue;
        };
        let Some(answer) = answer else {
            continue;
        };

        let question = question.trim().to_string();
        let answer = answer.trim().to_string();
        if question.is_empty() || answer.is_empty() {
            continue;
        }

        return Some((question, answer));
    }

    None
}

fn extract_gemini_command_event(value: &Value) -> Option<GeminiStreamEvent> {
    // Typical shape: {"command":"npm test","exit_code":1,"output":"..."}
    // or nested in item/data payloads.
    let obj = value.as_object()?;

    let command = obj
        .get("command")
        .and_then(Value::as_str)
        .or_else(|| {
            obj.get("item")
                .and_then(Value::as_object)
                .and_then(|item| item.get("command"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            obj.get("data")
                .and_then(Value::as_object)
                .and_then(|data| data.get("command"))
                .and_then(Value::as_str)
        })?;

    let command = command.trim().to_string();
    if command.is_empty() {
        return None;
    }

    let exit_code = obj
        .get("exit_code")
        .and_then(Value::as_i64)
        .or_else(|| obj.get("exitCode").and_then(Value::as_i64))
        .or_else(|| {
            obj.get("item").and_then(Value::as_object).and_then(|item| {
                item.get("exit_code")
                    .and_then(Value::as_i64)
                    .or_else(|| item.get("exitCode").and_then(Value::as_i64))
            })
        })
        .or_else(|| {
            obj.get("status")
                .and_then(Value::as_str)
                .map(|status| status.to_ascii_lowercase())
                .and_then(|status| match status.as_str() {
                    "completed" | "success" | "succeeded" => Some(0),
                    "failed" | "error" | "cancelled" | "canceled" => Some(1),
                    _ => None,
                })
        })
        .and_then(|code| i32::try_from(code).ok());

    let output = obj
        .get("output")
        .and_then(Value::as_str)
        .or_else(|| obj.get("result").and_then(Value::as_str))
        .or_else(|| {
            obj.get("item").and_then(Value::as_object).and_then(|item| {
                item.get("output")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("result").and_then(Value::as_str))
            })
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Some(GeminiStreamEvent::CommandCompleted {
        command,
        exit_code,
        output,
    })
}

fn extract_gemini_file_change_events(value: &Value) -> Vec<GeminiStreamEvent> {
    let mut out = Vec::new();
    let Some(obj) = value.as_object() else {
        return out;
    };

    let mut push_change = |path: &str, kind: &str| {
        let path = path.trim();
        let kind = kind.trim();
        if path.is_empty() || kind.is_empty() {
            return;
        }
        out.push(GeminiStreamEvent::FileChanged {
            path: path.to_string(),
            kind: kind.to_string(),
        });
    };

    if let Some(changes) = obj.get("changes").and_then(Value::as_array) {
        for change in changes {
            let Some(change_obj) = change.as_object() else {
                continue;
            };
            let path = change_obj.get("path").and_then(Value::as_str).unwrap_or("");
            let kind = change_obj.get("kind").and_then(Value::as_str).unwrap_or("");
            push_change(path, kind);
        }
    }

    if let Some(file_change) = obj.get("file_change").and_then(Value::as_object) {
        let path = file_change
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("");
        let kind = file_change
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("");
        push_change(path, kind);
    }

    out
}

fn extract_gemini_text_fragments(value: &Value) -> Vec<String> {
    let mut texts: Vec<String> = Vec::new();

    // Common direct fields.
    for key in ["text", "message", "output_text", "delta"] {
        if let Some(text) = value.get(key).and_then(Value::as_str) {
            let text = text.trim();
            if !text.is_empty() {
                texts.push(text.to_string());
            }
        }
    }

    // Gemini candidate parts shape.
    for pointer in [
        "/candidates",
        "/response/candidates",
        "/data/candidates",
        "/item/candidates",
    ] {
        let Some(candidates) = value.pointer(pointer).and_then(Value::as_array) else {
            continue;
        };

        for candidate in candidates {
            let Some(parts) = candidate
                .pointer("/content/parts")
                .and_then(Value::as_array)
            else {
                continue;
            };
            for part in parts {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    let text = text.trim();
                    if !text.is_empty() {
                        texts.push(text.to_string());
                    }
                }
            }
        }
    }

    // Best-effort dedupe while preserving order.
    let mut deduped = Vec::new();
    for text in texts {
        if deduped.last() == Some(&text) {
            continue;
        }
        deduped.push(text);
    }

    deduped
}

fn infer_gemini_final_message(value: &Value) -> bool {
    let type_value = value
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/event/type").and_then(Value::as_str))
        .unwrap_or("")
        .to_ascii_lowercase();
    if type_value.contains("completed") || type_value.contains("final") || type_value == "response"
    {
        return true;
    }

    value
        .get("status")
        .and_then(Value::as_str)
        .map(|status| {
            matches!(
                status.to_ascii_lowercase().as_str(),
                "completed" | "success" | "succeeded" | "done"
            )
        })
        .unwrap_or(false)
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

fn extract_model_context_window(
    value: &Value,
    usage: Option<&serde_json::Map<String, Value>>,
) -> Option<u64> {
    if let Some(usage) = usage {
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
    }

    for pointer in [
        "/response/model_context_window",
        "/response/modelContextWindow",
        "/model_context_window",
        "/modelContextWindow",
        "/data/model_context_window",
        "/data/modelContextWindow",
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
        "/response/model_usage",
        "/response/modelUsage",
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

impl GeminiClient {
    pub fn new() -> Self {
        Self
    }

    /// Spawns Gemini CLI in headless prompt mode.
    ///
    /// Notes:
    /// - `-p` runs headless (single prompt)
    /// - `--yolo` avoids interactive approval prompts
    /// - `--output-format stream-json` produces JSONL events (useful for future normalization)
    pub async fn spawn_session(
        &self,
        worktree_path: &Path,
        instruction: &str,
        _attempt_id: Uuid,
        env_vars: Option<HashMap<String, String>>,
    ) -> Result<SpawnedAgent> {
        if !worktree_path.exists() {
            anyhow::bail!("Worktree path does not exist: {:?}", worktree_path);
        }

        let command = resolve_gemini_command(env_vars.as_ref())
            .ok_or_else(|| anyhow::anyhow!("Gemini CLI not found and npx fallback unavailable"))?;

        let mut cmd = Command::new(&command.command);
        if command.use_npx {
            cmd.arg("-y").arg("@google/gemini-cli");
        }
        cmd.arg("-p")
            .arg(instruction)
            .arg("--yolo")
            .arg("--output-format")
            .arg("stream-json")
            .current_dir(worktree_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // Best-effort: disable color for cleaner log parsing
        cmd.env("NO_COLOR", "1");

        if let Some(vars) = env_vars {
            for (k, v) in vars {
                if k == EXEC_GEMINI_CMD_ENV || k == EXEC_GEMINI_USE_NPX_ENV {
                    continue;
                }
                cmd.env(k, v);
            }
        }

        let child: AsyncGroupChild = cmd
            .group_spawn()
            .with_context(|| format!("Failed to spawn Gemini CLI in {:?}", worktree_path))?;

        let (interrupt_tx, interrupt_rx): (InterruptSender, InterruptReceiver) = oneshot::channel();

        Ok(SpawnedAgent {
            child,
            interrupt_sender: Some(interrupt_tx),
            interrupt_receiver: Some(interrupt_rx),
            msg_store: None,
        })
    }
}

impl Default for GeminiClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk_normalized_types::{
        NormalizedEntry as SdkNormalizedEntry, NormalizedEntryType as SdkNormalizedEntryType,
    };
    use crate::validate_sdk_normalized_entry;

    fn metadata_event_to_entry(event: &GeminiStreamEvent) -> Option<SdkNormalizedEntry> {
        let entry_type = match event {
            GeminiStreamEvent::TokenUsage {
                input_tokens,
                output_tokens,
                total_tokens,
                model_context_window,
            } => SdkNormalizedEntryType::TokenUsageInfo {
                input_tokens: *input_tokens,
                output_tokens: *output_tokens,
                total_tokens: *total_tokens,
                model_context_window: *model_context_window,
            },
            GeminiStreamEvent::NextAction { text } => {
                SdkNormalizedEntryType::NextAction { text: text.clone() }
            }
            GeminiStreamEvent::UserAnsweredQuestions { question, answer } => {
                SdkNormalizedEntryType::UserAnsweredQuestions {
                    question: question.clone(),
                    answer: answer.clone(),
                }
            }
            GeminiStreamEvent::AgentMessage { .. }
            | GeminiStreamEvent::Skip
            | GeminiStreamEvent::ToolUseStarted { .. }
            | GeminiStreamEvent::ToolResult { .. }
            | GeminiStreamEvent::CommandCompleted { .. }
            | GeminiStreamEvent::FileChanged { .. } => {
                return None;
            }
        };

        Some(SdkNormalizedEntry {
            timestamp: Some("2026-02-27T12:35:00.000Z".to_string()),
            entry_type,
            content: String::new(),
        })
    }

    #[test]
    fn parse_agent_message_from_candidates() {
        let line = r#"{
            "type":"response.completed",
            "response":{
                "candidates":[
                    {"content":{"parts":[{"text":"Implemented changes"}]}}
                ]
            }
        }"#;

        assert_eq!(
            parse_gemini_json_events(line),
            vec![GeminiStreamEvent::AgentMessage {
                text: "Implemented changes".to_string(),
                is_final: true,
            }]
        );
    }

    #[test]
    fn parse_command_completed_event() {
        let line = r#"{
            "type":"tool_result",
            "command":"npm test",
            "exit_code":1,
            "output":"failing test"
        }"#;

        assert_eq!(
            parse_gemini_json_events(line),
            vec![GeminiStreamEvent::CommandCompleted {
                command: "npm test".to_string(),
                exit_code: Some(1),
                output: Some("failing test".to_string()),
            }]
        );
    }

    #[test]
    fn parse_command_completed_event_infers_exit_code_from_status() {
        let line = r#"{
            "type":"tool_result",
            "command":"npm test",
            "status":"failed",
            "output":"failing test"
        }"#;

        assert_eq!(
            parse_gemini_json_events(line),
            vec![GeminiStreamEvent::CommandCompleted {
                command: "npm test".to_string(),
                exit_code: Some(1),
                output: Some("failing test".to_string()),
            }]
        );
    }

    #[test]
    fn parse_file_change_events() {
        let line = r#"{
            "type":"file_change",
            "changes":[
                {"path":"src/main.rs","kind":"update"},
                {"path":"README.md","kind":"create"}
            ]
        }"#;

        assert_eq!(
            parse_gemini_json_events(line),
            vec![
                GeminiStreamEvent::FileChanged {
                    path: "src/main.rs".to_string(),
                    kind: "update".to_string(),
                },
                GeminiStreamEvent::FileChanged {
                    path: "README.md".to_string(),
                    kind: "create".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_usage_and_next_action_meta_events() {
        let line = r#"{
            "type":"response.completed",
            "response":{
                "usage":{"inputTokenCount":"120","outputTokenCount":34,"totalTokenCount":154},
                "next_action":"Run integration tests before merge"
            }
        }"#;

        assert_eq!(
            parse_gemini_json_events(line),
            vec![
                GeminiStreamEvent::TokenUsage {
                    input_tokens: 120,
                    output_tokens: 34,
                    total_tokens: Some(154),
                    model_context_window: None,
                },
                GeminiStreamEvent::NextAction {
                    text: "Run integration tests before merge".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_usage_meta_events_extracts_model_context_window() {
        let line = r#"{
            "type":"response.completed",
            "response":{
                "usage":{
                    "inputTokenCount":"120",
                    "outputTokenCount":34,
                    "totalTokenCount":154,
                    "modelContextWindow":"200000"
                }
            }
        }"#;

        assert_eq!(
            parse_gemini_json_events(line),
            vec![GeminiStreamEvent::TokenUsage {
                input_tokens: 120,
                output_tokens: 34,
                total_tokens: Some(154),
                model_context_window: Some(200_000),
            }]
        );
    }

    #[test]
    fn parse_usage_meta_events_extracts_nested_model_usage_context_window() {
        let line = r#"{
            "type":"response.completed",
            "response":{
                "usage":{"inputTokenCount":12,"outputTokenCount":4},
                "modelUsage":{"gemini-2.5":{"contextWindow":1048576}}
            }
        }"#;

        assert_eq!(
            parse_gemini_json_events(line),
            vec![GeminiStreamEvent::TokenUsage {
                input_tokens: 12,
                output_tokens: 4,
                total_tokens: None,
                model_context_window: Some(1_048_576),
            }]
        );
    }

    #[test]
    fn parse_user_answered_questions_meta_event() {
        let line = r#"{
            "type":"response.completed",
            "response":{
                "user_answered_questions":{
                    "question":"Deploy now?",
                    "answer":"Not yet"
                }
            }
        }"#;

        assert_eq!(
            parse_gemini_json_events(line),
            vec![GeminiStreamEvent::UserAnsweredQuestions {
                question: "Deploy now?".to_string(),
                answer: "Not yet".to_string(),
            }]
        );
    }

    #[test]
    fn parse_meta_events_support_data_next_action_and_direct_question_answer() {
        let line = r#"{
            "type":"response.completed",
            "data":{"nextAction":"Ask for approval before deploy"},
            "question":"Proceed deploy?",
            "answer":"yes"
        }"#;

        assert_eq!(
            parse_gemini_json_events(line),
            vec![
                GeminiStreamEvent::NextAction {
                    text: "Ask for approval before deploy".to_string(),
                },
                GeminiStreamEvent::UserAnsweredQuestions {
                    question: "Proceed deploy?".to_string(),
                    answer: "yes".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_meta_events_support_data_nested_user_answered_questions() {
        let line = r#"{
            "type":"response.completed",
            "data":{
                "userAnsweredQuestions":{
                    "question":"Ship now?",
                    "answer":"after QA"
                }
            }
        }"#;

        assert_eq!(
            parse_gemini_json_events(line),
            vec![GeminiStreamEvent::UserAnsweredQuestions {
                question: "Ship now?".to_string(),
                answer: "after QA".to_string(),
            }]
        );
    }

    #[test]
    fn parse_meta_events_ignores_blank_or_incomplete_values() {
        let line = r#"{
            "type":"response.completed",
            "response":{
                "usage":{"inputTokenCount":0,"outputTokenCount":0,"totalTokenCount":0},
                "next_action":"   ",
                "user_answered_questions":{"question":"Deploy now?","answer":""}
            }
        }"#;

        assert_eq!(
            parse_gemini_json_events(line),
            Vec::<GeminiStreamEvent>::new()
        );
    }

    #[test]
    fn parsed_metadata_events_produce_contract_valid_normalized_entries() {
        let line = r#"{
            "type":"response.completed",
            "response":{
                "usage":{"inputTokenCount":"11","outputTokenCount":"5","totalTokenCount":"16"},
                "next_action":"Wait for product sign-off",
                "user_answered_questions":{"question":"Deploy now?","answer":"tomorrow"}
            }
        }"#;

        for event in parse_gemini_json_events(line) {
            let Some(entry) = metadata_event_to_entry(&event) else {
                continue;
            };
            assert!(
                validate_sdk_normalized_entry(&entry).is_ok(),
                "expected contract-valid entry for event: {:?}",
                event
            );
        }
    }

    #[test]
    fn parse_chat_format_message_and_tool_events() {
        let msg_user = r#"{"type":"message","timestamp":"2026-03-01T15:41:29.712Z","role":"user","content":"add to readme"}"#;
        assert_eq!(
            parse_gemini_json_events(msg_user),
            vec![GeminiStreamEvent::Skip]
        );

        let msg_assistant = r#"{"type":"message","timestamp":"2026-03-01T15:41:37.071Z","role":"assistant","content":"I will begin by performing a preflight check.\n\n","delta":true}"#;
        assert_eq!(
            parse_gemini_json_events(msg_assistant),
            vec![GeminiStreamEvent::AgentMessage {
                text: "I will begin by performing a preflight check.\n\n".to_string(),
                is_final: false,
            }]
        );

        let tool_use_read = r#"{"type":"tool_use","timestamp":"2026-03-01T15:41:38.000Z","tool_name":"read_file","tool_id":"read_file_123","parameters":{"file_path":"README.md"}}"#;
        assert_eq!(
            parse_gemini_json_events(tool_use_read),
            vec![GeminiStreamEvent::ToolUseStarted {
                tool_id: "read_file_123".to_string(),
                tool_name: "read_file".to_string(),
                payload: r#"{"file_path":"README.md"}"#.to_string(),
            }]
        );

        let tool_use = r#"{"type":"tool_use","timestamp":"2026-03-01T15:41:37.348Z","tool_name":"run_shell_command","tool_id":"run_shell_command_1772379697348_0","parameters":{"description":"Checking for preflight manifest and git status.","command":"ls -a .acpms/references/refs_manifest.json && git status"}}"#;
        assert_eq!(
            parse_gemini_json_events(tool_use),
            vec![GeminiStreamEvent::ToolUseStarted {
                tool_id: "run_shell_command_1772379697348_0".to_string(),
                tool_name: "run_shell_command".to_string(),
                payload: "ls -a .acpms/references/refs_manifest.json && git status".to_string(),
            }]
        );

        let tool_result = r#"{"type":"tool_result","timestamp":"2026-03-01T15:41:37.407Z","tool_id":"run_shell_command_1772379697348_0","status":"success","output":"ls: .acpms/references/refs_manifest.json: No such file or directory"}"#;
        assert_eq!(
            parse_gemini_json_events(tool_result),
            vec![GeminiStreamEvent::ToolResult {
                tool_id: "run_shell_command_1772379697348_0".to_string(),
                success: true,
                output: Some(
                    "ls: .acpms/references/refs_manifest.json: No such file or directory"
                        .to_string()
                ),
            }]
        );

        let result = r#"{"type":"result","timestamp":"2026-03-01T15:51:26.000Z","status":"success","stats":{"total_tokens":121482,"input_tokens":117934,"output_tokens":1202}}"#;
        assert_eq!(
            parse_gemini_json_events(result),
            vec![GeminiStreamEvent::Skip]
        );
    }

    #[test]
    fn parse_realistic_gemini_fixture_stream() {
        let fixture = include_str!("../test-fixtures/gemini_stream_realistic.jsonl");
        let events: Vec<GeminiStreamEvent> = fixture
            .lines()
            .filter(|line| !line.trim().is_empty())
            .flat_map(parse_gemini_json_events)
            .collect();

        assert!(events.iter().any(|event| {
            matches!(
                event,
                GeminiStreamEvent::AgentMessage { text, is_final }
                    if text.contains("Prepared deployment checklist") && *is_final
            )
        }));

        assert!(events.iter().any(|event| {
            matches!(
                event,
                GeminiStreamEvent::CommandCompleted { command, exit_code, .. }
                    if command.contains("https://status.example.com/health")
                        && *exit_code == Some(0)
            )
        }));

        assert!(events.iter().any(|event| {
            matches!(
                event,
                GeminiStreamEvent::FileChanged { path, kind }
                    if path == "docs/runbook.md" && kind == "update"
            )
        }));

        for event in &events {
            let Some(entry) = metadata_event_to_entry(event) else {
                continue;
            };
            assert!(
                validate_sdk_normalized_entry(&entry).is_ok(),
                "fixture metadata entry must be contract-valid"
            );
        }
    }

    #[test]
    fn resolve_gemini_command_prefers_exec_env_command() {
        let mut vars = HashMap::new();
        vars.insert(
            EXEC_GEMINI_CMD_ENV.to_string(),
            "/tmp/custom-gemini".to_string(),
        );

        let resolved = resolve_gemini_command(Some(&vars)).expect("expected command resolution");
        assert_eq!(resolved.command, "/tmp/custom-gemini");
        assert!(!resolved.use_npx);
    }

    #[test]
    fn resolve_gemini_command_reads_exec_env_npx_flag() {
        let mut vars = HashMap::new();
        vars.insert(
            EXEC_GEMINI_CMD_ENV.to_string(),
            "/tmp/custom-npx".to_string(),
        );
        vars.insert(EXEC_GEMINI_USE_NPX_ENV.to_string(), "1".to_string());

        let resolved = resolve_gemini_command(Some(&vars)).expect("expected command resolution");
        assert_eq!(resolved.command, "/tmp/custom-npx");
        assert!(resolved.use_npx);
    }
}
