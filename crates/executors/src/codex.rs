//! OpenAI Codex CLI client for spawning and managing agent sessions.
//!
//! This is intentionally minimal and mirrors the existing Claude integration:
//! - Spawn a non-interactive `codex exec` process in a worktree
//! - Stream stdout/stderr into the attempt log pipeline

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use command_group::{AsyncCommandGroup, AsyncGroupChild};
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use tokio::process::Command;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::claude::SpawnedAgent;
use crate::process::{InterruptReceiver, InterruptSender};

pub struct CodexClient;

const EXEC_CODEX_CMD_ENV: &str = "ACPMS_EXEC_CODEX_CMD";
const EXEC_CODEX_USE_NPX_ENV: &str = "ACPMS_EXEC_CODEX_USE_NPX";
const OVERRIDE_CODEX_BIN_ENV: &str = "ACPMS_AGENT_CODEX_BIN";
const OVERRIDE_NPX_BIN_ENV: &str = "ACPMS_AGENT_NPX_BIN";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexStreamEvent {
    AgentMessage {
        item_id: Option<String>,
        text: String,
        is_final: bool,
    },
    CommandStarted {
        command: String,
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

#[derive(Debug, Deserialize)]
struct CodexJsonLine {
    #[serde(rename = "type")]
    event_type: String,
    item: Option<CodexItem>,
}

#[derive(Debug, Deserialize)]
struct CodexItem {
    id: Option<String>,
    #[serde(rename = "type")]
    item_type: String,
    text: Option<String>,
    command: Option<String>,
    #[serde(alias = "aggregatedOutput")]
    aggregated_output: Option<String>,
    #[serde(alias = "exitCode")]
    exit_code: Option<i32>,
    status: Option<String>,
    changes: Option<Vec<CodexFileChange>>,
}

#[derive(Debug, Deserialize)]
struct CodexFileChange {
    path: String,
    kind: String,
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

fn resolve_codex_command(env_vars: Option<&HashMap<String, String>>) -> Option<CommandResolution> {
    if let Some(vars) = env_vars {
        if let Some(command) = read_non_empty_map_value(vars, EXEC_CODEX_CMD_ENV) {
            return Some(CommandResolution {
                command,
                use_npx: is_truthy(vars.get(EXEC_CODEX_USE_NPX_ENV)),
            });
        }
    }

    if let Some(command) = resolve_command_with_override("codex", OVERRIDE_CODEX_BIN_ENV) {
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

/// Parse a single `codex --json` line into a higher-level event we can map to our log schema.
pub fn parse_codex_json_events(line: &str) -> Vec<CodexStreamEvent> {
    let value: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    if event_type != "item.started"
        && event_type != "item.updated"
        && event_type != "item.completed"
    {
        return parse_codex_meta_events(&value);
    }

    let parsed: CodexJsonLine = match serde_json::from_value(value) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    // We only care about item lifecycle events.
    if parsed.event_type != "item.started"
        && parsed.event_type != "item.updated"
        && parsed.event_type != "item.completed"
    {
        return Vec::new();
    }

    let item = match parsed.item {
        Some(i) => i,
        None => return Vec::new(),
    };

    match (parsed.event_type.as_str(), item.item_type.as_str()) {
        ("item.updated", "agent_message") | ("item.completed", "agent_message") => {
            let Some(text) = item.text else {
                return Vec::new();
            };
            if text.trim().is_empty() {
                return Vec::new();
            }
            vec![CodexStreamEvent::AgentMessage {
                item_id: item.id,
                text,
                is_final: parsed.event_type == "item.completed",
            }]
        }
        ("item.started", "command_execution") => {
            let Some(command) = item.command else {
                return Vec::new();
            };
            let command = command.trim().to_string();
            if command.is_empty() {
                return Vec::new();
            }
            vec![CodexStreamEvent::CommandStarted { command }]
        }
        ("item.completed", "command_execution") => {
            let Some(command) = item.command else {
                return Vec::new();
            };
            let command = command.trim().to_string();
            if command.is_empty() {
                return Vec::new();
            }

            // Newer Codex JSON can omit explicit exit code while still providing a status.
            // Infer a synthetic exit code so downstream status mapping stays accurate.
            let mut exit_code = item.exit_code;
            if exit_code.is_none() {
                if let Some(status) = item.status.as_deref() {
                    let status = status.to_ascii_lowercase();
                    if matches!(status.as_str(), "completed" | "success" | "succeeded") {
                        exit_code = Some(0);
                    } else if matches!(
                        status.as_str(),
                        "failed" | "error" | "cancelled" | "canceled" | "killed"
                    ) {
                        exit_code = Some(1);
                    }
                }
            }

            let output = item.aggregated_output.and_then(|s| {
                let t = s.trim().to_string();
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            });
            vec![CodexStreamEvent::CommandCompleted {
                command,
                exit_code,
                output,
            }]
        }
        ("item.completed", "file_change") => {
            let Some(changes) = item.changes else {
                return Vec::new();
            };

            changes
                .into_iter()
                .filter_map(|c| {
                    let path = c.path.trim().to_string();
                    let kind = c.kind.trim().to_string();
                    if path.is_empty() || kind.is_empty() {
                        None
                    } else {
                        Some(CodexStreamEvent::FileChanged { path, kind })
                    }
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn parse_codex_meta_events(value: &Value) -> Vec<CodexStreamEvent> {
    let mut events = Vec::new();

    let usage = value
        .pointer("/response/usage")
        .or_else(|| value.get("usage"))
        .and_then(Value::as_object);

    if let Some(usage) = usage {
        let input_tokens = usage
            .get("input_tokens")
            .or_else(|| usage.get("prompt_tokens"))
            .or_else(|| usage.get("inputTokens"))
            .and_then(value_as_u64)
            .unwrap_or(0);
        let output_tokens = usage
            .get("output_tokens")
            .or_else(|| usage.get("completion_tokens"))
            .or_else(|| usage.get("outputTokens"))
            .and_then(value_as_u64)
            .unwrap_or(0);
        let total_tokens = usage
            .get("total_tokens")
            .or_else(|| usage.get("totalTokens"))
            .and_then(value_as_u64);
        let model_context_window = extract_model_context_window(value, Some(usage));

        if input_tokens > 0 || output_tokens > 0 || total_tokens.unwrap_or(0) > 0 {
            events.push(CodexStreamEvent::TokenUsage {
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
        .or_else(|| value.get("next_action").and_then(Value::as_str))
        .or_else(|| value.get("nextAction").and_then(Value::as_str))
        .or_else(|| value.pointer("/data/next_action").and_then(Value::as_str))
        .or_else(|| value.pointer("/data/nextAction").and_then(Value::as_str))
    {
        let text = text.trim().to_string();
        if !text.is_empty() {
            events.push(CodexStreamEvent::NextAction { text });
        }
    }

    if let Some((question, answer)) = extract_user_answered_questions(value) {
        events.push(CodexStreamEvent::UserAnsweredQuestions { question, answer });
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

fn value_as_u64(value: &Value) -> Option<u64> {
    if let Some(v) = value.as_u64() {
        return Some(v);
    }
    if let Some(v) = value.as_i64() {
        return u64::try_from(v).ok();
    }
    value.as_str().and_then(|s| s.trim().parse::<u64>().ok())
}

/// Best-effort extraction of text fragments that contain `REPO_URL`.
///
/// This is used as a fallback for Codex JSON events that are not covered by
/// `parse_codex_json_events` but still carry assistant output in nested fields.
pub fn extract_repo_url_hint_from_json_line(line: &str) -> Option<String> {
    if !line.to_ascii_lowercase().contains("repo_url") {
        return None;
    }

    if let Ok(value) = serde_json::from_str::<Value>(line) {
        let mut collected = Vec::new();
        collect_strings(&value, &mut collected);
        for text in collected {
            if text.to_ascii_lowercase().contains("repo_url") {
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }
    }

    let trimmed = line.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn collect_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => {
            for item in items {
                collect_strings(item, out);
            }
        }
        Value::Object(map) => {
            for v in map.values() {
                collect_strings(v, out);
            }
        }
        _ => {}
    }
}

/// Best-effort extraction of agent text from a JSON line.
/// Used when parse_codex_json_events returns empty but the line may contain assistant output.
pub fn extract_agent_text_from_json_line(line: &str) -> Option<String> {
    let value: Value = serde_json::from_str(line).ok()?;
    let obj = value.as_object()?;
    for key in ["text", "output", "output_text", "content"] {
        if let Some(v) = obj.get(key) {
            if let Some(s) = v.as_str() {
                let t = s.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    if let Some(item) = obj.get("item") {
        if let Some(s) = item.get("text").and_then(|v| v.as_str()) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    if let Some(msg) = obj.get("message") {
        if let Some(s) = msg.get("content").and_then(|v| v.as_str()) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

impl CodexClient {
    pub fn new() -> Self {
        Self
    }

    /// Spawns Codex CLI in non-interactive exec mode.
    ///
    /// Notes:
    /// - Uses `-a never` at the root command level to avoid interactive approval prompts
    /// - Uses `--sandbox danger-full-access` to match current "trusted local" behavior
    /// - Sets `NO_COLOR=1` to reduce ANSI noise in logs
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

        let command = resolve_codex_command(env_vars.as_ref())
            .ok_or_else(|| anyhow::anyhow!("Codex CLI not found and npx fallback unavailable"))?;

        let mut cmd = Command::new(&command.command);
        if command.use_npx {
            cmd.arg("-y").arg("@openai/codex");
        }
        // `-a/--ask-for-approval` is a *root* flag (not accepted by `codex exec` directly).
        // It must appear before the subcommand.
        cmd.arg("-a")
            .arg("never")
            .arg("exec")
            // Emit JSONL events (stdout) so we can parse + render logs cleanly (no stderr noise).
            .arg("--json")
            // From-scratch init starts in an empty directory; let Codex run before `git init`.
            .arg("--skip-git-repo-check")
            .arg("--sandbox")
            .arg("danger-full-access")
            .arg("--color")
            .arg("never")
            .arg("-C")
            .arg(worktree_path)
            .current_dir(worktree_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // Best-effort: disable color for cleaner log parsing
        cmd.env("NO_COLOR", "1");

        if let Some(vars) = env_vars {
            for (k, v) in vars {
                if k == EXEC_CODEX_CMD_ENV || k == EXEC_CODEX_USE_NPX_ENV {
                    continue;
                }
                cmd.env(k, v);
            }
        }

        // Ensure the prompt is treated as a positional argument even if it starts with `-`/`--`.
        // (Codex uses clap; without this, a prompt like `--foo` is parsed as an unknown flag.)
        cmd.arg("--");

        // Prompt as final arg (avoid shell escaping issues)
        cmd.arg(instruction);

        let child: AsyncGroupChild = cmd
            .group_spawn()
            .with_context(|| format!("Failed to spawn Codex CLI in {:?}", worktree_path))?;

        let (interrupt_tx, interrupt_rx): (InterruptSender, InterruptReceiver) = oneshot::channel();

        Ok(SpawnedAgent {
            child,
            interrupt_sender: Some(interrupt_tx),
            interrupt_receiver: Some(interrupt_rx),
            msg_store: None,
        })
    }
}

impl Default for CodexClient {
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

    fn metadata_event_to_entry(event: &CodexStreamEvent) -> Option<SdkNormalizedEntry> {
        let entry_type = match event {
            CodexStreamEvent::TokenUsage {
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
            CodexStreamEvent::NextAction { text } => {
                SdkNormalizedEntryType::NextAction { text: text.clone() }
            }
            CodexStreamEvent::UserAnsweredQuestions { question, answer } => {
                SdkNormalizedEntryType::UserAnsweredQuestions {
                    question: question.clone(),
                    answer: answer.clone(),
                }
            }
            CodexStreamEvent::AgentMessage { .. }
            | CodexStreamEvent::CommandStarted { .. }
            | CodexStreamEvent::CommandCompleted { .. }
            | CodexStreamEvent::FileChanged { .. } => {
                return None;
            }
        };

        Some(SdkNormalizedEntry {
            timestamp: Some("2026-02-27T12:30:00.000Z".to_string()),
            entry_type,
            content: String::new(),
        })
    }

    #[test]
    fn parse_agent_message() {
        let line = r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"OK"}}"#;
        assert_eq!(
            parse_codex_json_events(line),
            vec![CodexStreamEvent::AgentMessage {
                item_id: Some("item_0".to_string()),
                text: "OK".to_string(),
                is_final: true,
            }]
        );
    }

    #[test]
    fn parse_agent_message_updated_preserves_whitespace() {
        let line = r#"{"type":"item.updated","item":{"id":"item_0","type":"agent_message","text":"Hello "}}"#;
        assert_eq!(
            parse_codex_json_events(line),
            vec![CodexStreamEvent::AgentMessage {
                item_id: Some("item_0".to_string()),
                text: "Hello ".to_string(),
                is_final: false,
            }]
        );
    }

    #[test]
    fn parse_command_started() {
        let line = r#"{"type":"item.started","item":{"id":"item_1","type":"command_execution","command":"/bin/zsh -lc 'ls -la'","aggregated_output":"","exit_code":null,"status":"in_progress"}}"#;
        assert_eq!(
            parse_codex_json_events(line),
            vec![CodexStreamEvent::CommandStarted {
                command: "/bin/zsh -lc 'ls -la'".to_string()
            }]
        );
    }

    #[test]
    fn parse_command_completed() {
        let line = r#"{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"cmd","aggregated_output":"out","exit_code":0,"status":"completed"}}"#;
        assert_eq!(
            parse_codex_json_events(line),
            vec![CodexStreamEvent::CommandCompleted {
                command: "cmd".to_string(),
                exit_code: Some(0),
                output: Some("out".to_string())
            }]
        );
    }

    #[test]
    fn parse_command_completed_camel_case_and_missing_exit_code() {
        let line = r#"{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"cmd","aggregatedOutput":"out","status":"completed"}}"#;
        assert_eq!(
            parse_codex_json_events(line),
            vec![CodexStreamEvent::CommandCompleted {
                command: "cmd".to_string(),
                exit_code: Some(0),
                output: Some("out".to_string())
            }]
        );
    }

    #[test]
    fn parse_command_completed_infers_failed_exit_code_from_status() {
        let line = r#"{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"cmd","aggregatedOutput":"err","status":"failed"}}"#;
        assert_eq!(
            parse_codex_json_events(line),
            vec![CodexStreamEvent::CommandCompleted {
                command: "cmd".to_string(),
                exit_code: Some(1),
                output: Some("err".to_string())
            }]
        );
    }

    #[test]
    fn parse_file_change() {
        let line = r#"{"type":"item.completed","item":{"id":"item_5","type":"file_change","changes":[{"path":"/tmp/a.txt","kind":"update"}],"status":"completed"}}"#;
        assert_eq!(
            parse_codex_json_events(line),
            vec![CodexStreamEvent::FileChanged {
                path: "/tmp/a.txt".to_string(),
                kind: "update".to_string()
            }]
        );
    }

    #[test]
    fn extract_repo_url_hint_from_nested_json_text() {
        let line = r#"{"type":"response.completed","response":{"output_text":"Summary\nREPO_URL: https://gitlab.com/acme/demo.git"}}"#;
        let hint = extract_repo_url_hint_from_json_line(line);
        assert_eq!(
            hint,
            Some("Summary\nREPO_URL: https://gitlab.com/acme/demo.git".to_string())
        );
    }

    #[test]
    fn parse_response_usage_into_token_usage_event() {
        let line = r#"{
            "type":"response.completed",
            "response":{"usage":{"input_tokens":120,"output_tokens":34,"total_tokens":154}}
        }"#;

        assert_eq!(
            parse_codex_json_events(line),
            vec![CodexStreamEvent::TokenUsage {
                input_tokens: 120,
                output_tokens: 34,
                total_tokens: Some(154),
                model_context_window: None,
            }]
        );
    }

    #[test]
    fn parse_response_usage_extracts_model_context_window() {
        let line = r#"{
            "type":"response.completed",
            "response":{
                "usage":{
                    "input_tokens":120,
                    "output_tokens":34,
                    "total_tokens":154,
                    "model_context_window":200000
                }
            }
        }"#;

        assert_eq!(
            parse_codex_json_events(line),
            vec![CodexStreamEvent::TokenUsage {
                input_tokens: 120,
                output_tokens: 34,
                total_tokens: Some(154),
                model_context_window: Some(200_000),
            }]
        );
    }

    #[test]
    fn parse_response_usage_extracts_nested_model_usage_context_window() {
        let line = r#"{
            "type":"response.completed",
            "response":{
                "usage":{"input_tokens":12,"output_tokens":4},
                "model_usage":{
                    "gpt-5":{"context_window":272000}
                }
            }
        }"#;

        assert_eq!(
            parse_codex_json_events(line),
            vec![CodexStreamEvent::TokenUsage {
                input_tokens: 12,
                output_tokens: 4,
                total_tokens: None,
                model_context_window: Some(272_000),
            }]
        );
    }

    #[test]
    fn parse_next_action_event() {
        let line = r#"{
            "type":"response.completed",
            "response":{"next_action":"Run integration tests before merge"}
        }"#;

        assert_eq!(
            parse_codex_json_events(line),
            vec![CodexStreamEvent::NextAction {
                text: "Run integration tests before merge".to_string(),
            }]
        );
    }

    #[test]
    fn parse_user_answered_questions_event() {
        let line = r#"{
            "type":"response.completed",
            "response":{"user_answered_questions":{"question":"Deploy now?","answer":"Not yet"}}
        }"#;

        assert_eq!(
            parse_codex_json_events(line),
            vec![CodexStreamEvent::UserAnsweredQuestions {
                question: "Deploy now?".to_string(),
                answer: "Not yet".to_string(),
            }]
        );
    }

    #[test]
    fn parse_meta_events_support_camel_case_usage_and_direct_question_answer() {
        let line = r#"{
            "type":"response.completed",
            "usage":{"inputTokens":"10","outputTokens":"4","totalTokens":"14"},
            "question":"Run deploy?",
            "answer":"yes"
        }"#;

        assert_eq!(
            parse_codex_json_events(line),
            vec![
                CodexStreamEvent::TokenUsage {
                    input_tokens: 10,
                    output_tokens: 4,
                    total_tokens: Some(14),
                    model_context_window: None,
                },
                CodexStreamEvent::UserAnsweredQuestions {
                    question: "Run deploy?".to_string(),
                    answer: "yes".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_meta_events_support_data_next_action_and_nested_user_answered_questions() {
        let line = r#"{
            "type":"response.completed",
            "data":{
                "nextAction":"Ask for confirmation before deploy",
                "userAnsweredQuestions":{
                    "question":"Deploy now?",
                    "answer":"not yet"
                }
            }
        }"#;

        assert_eq!(
            parse_codex_json_events(line),
            vec![
                CodexStreamEvent::NextAction {
                    text: "Ask for confirmation before deploy".to_string(),
                },
                CodexStreamEvent::UserAnsweredQuestions {
                    question: "Deploy now?".to_string(),
                    answer: "not yet".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_meta_events_ignores_blank_or_incomplete_values() {
        let line = r#"{
            "type":"response.completed",
            "response":{
                "usage":{"input_tokens":0,"output_tokens":0,"total_tokens":0},
                "next_action":"   ",
                "user_answered_questions":{"question":"Deploy now?","answer":"   "}
            }
        }"#;

        assert_eq!(
            parse_codex_json_events(line),
            Vec::<CodexStreamEvent>::new()
        );
    }

    #[test]
    fn parsed_metadata_events_produce_contract_valid_normalized_entries() {
        let line = r#"{
            "type":"response.completed",
            "response":{
                "usage":{"input_tokens":9,"output_tokens":4,"total_tokens":13},
                "next_action":"Request confirmation before deploy",
                "user_answered_questions":{"question":"Deploy now?","answer":"after QA"}
            }
        }"#;

        for event in parse_codex_json_events(line) {
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
    fn parse_realistic_codex_fixture_stream() {
        let fixture = include_str!("../test-fixtures/codex_stream_realistic.jsonl");
        let events: Vec<CodexStreamEvent> = fixture
            .lines()
            .filter(|line| !line.trim().is_empty())
            .flat_map(parse_codex_json_events)
            .collect();

        assert!(events.iter().any(|event| {
            matches!(
                event,
                CodexStreamEvent::AgentMessage { text, is_final, .. }
                    if text.contains("Planning migration rollout") && !is_final
            )
        }));

        assert!(events.iter().any(|event| {
            matches!(
                event,
                CodexStreamEvent::CommandCompleted { command, exit_code, .. }
                    if command.contains("DATABASE_URL")
                        && *exit_code == Some(0)
            )
        }));

        assert!(events.iter().any(|event| {
            matches!(
                event,
                CodexStreamEvent::FileChanged { path, kind }
                    if path.ends_with("20260227_add_index.sql") && kind == "create"
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
    fn resolve_codex_command_prefers_exec_env_command() {
        let mut vars = HashMap::new();
        vars.insert(
            EXEC_CODEX_CMD_ENV.to_string(),
            "/tmp/custom-codex".to_string(),
        );

        let resolved = resolve_codex_command(Some(&vars)).expect("expected command resolution");
        assert_eq!(resolved.command, "/tmp/custom-codex");
        assert!(!resolved.use_npx);
    }

    #[test]
    fn resolve_codex_command_reads_exec_env_npx_flag() {
        let mut vars = HashMap::new();
        vars.insert(
            EXEC_CODEX_CMD_ENV.to_string(),
            "/tmp/custom-npx".to_string(),
        );
        vars.insert(EXEC_CODEX_USE_NPX_ENV.to_string(), "true".to_string());

        let resolved = resolve_codex_command(Some(&vars)).expect("expected command resolution");
        assert_eq!(resolved.command, "/tmp/custom-npx");
        assert!(resolved.use_npx);
    }
}
