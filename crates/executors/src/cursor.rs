//! Cursor CLI client for spawning and managing agent sessions.
//!
//! Minimal integration:
//! - Spawn `agent -p` with --force, --output-format stream-json
//! - Parse stream-json NDJSON (type: system|user|assistant|tool_call|result)
//! - Map to normalized timeline events

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{Context, Result};
use command_group::{AsyncCommandGroup, AsyncGroupChild};
use regex::Regex;
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::claude::SpawnedAgent;
use crate::process::{InterruptReceiver, InterruptSender};

const EXEC_CURSOR_CMD_ENV: &str = "ACPMS_EXEC_CURSOR_CMD";
const OVERRIDE_CURSOR_BIN_ENV: &str = "ACPMS_AGENT_CURSOR_BIN";

pub struct CursorClient;

impl CursorClient {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorStreamEvent {
    /// type: "system", subtype: "init"
    SystemInit {
        cwd: String,
        model: String,
        session_id: String,
    },
    /// type: "assistant", message.content[0].text
    AgentMessage { text: String, is_final: bool },
    /// type: "thinking", subtype: "delta" — có thể bỏ qua hoặc log nhẹ
    ThinkingDelta { text: String },
    /// type: "thinking", subtype: "completed"
    ThinkingCompleted,
    /// type: "tool_call", subtype: "started"
    ToolCallStarted {
        call_id: String,
        tool_type: String,
        path: String,
    },
    /// type: "tool_call", subtype: "completed"
    ToolCallCompleted {
        call_id: String,
        tool_type: String,
        path: String,
        lines_added: Option<u64>,
        lines_created: Option<u64>,
        message: Option<String>,
    },
    /// type: "tool_call" — shellToolCall, globToolCall, etc. (non-edit tools)
    ToolCallStartedGeneric {
        call_id: String,
        tool_name: String,
        payload: String,
    },
    /// type: "tool_call", subtype: "completed" — for shell/glob
    ToolCallCompletedGeneric { call_id: String, success: bool },
    /// type: "result", subtype: "success"
    Result {
        duration_ms: u64,
        result: String,
        usage: Option<CursorUsage>,
    },
    /// Parsed but should not log as raw (user prompt, updateTodosToolCall)
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
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

fn resolve_cursor_command(env_vars: Option<&HashMap<String, String>>) -> Option<String> {
    if let Some(vars) = env_vars {
        if let Some(command) = read_non_empty_map_value(vars, EXEC_CURSOR_CMD_ENV) {
            return Some(command);
        }
    }
    resolve_command_with_override("agent", OVERRIDE_CURSOR_BIN_ENV)
}

fn parse_cursor_usage(v: &Value) -> Option<CursorUsage> {
    let o = v.as_object()?;
    Some(CursorUsage {
        input_tokens: o.get("inputTokens").and_then(Value::as_u64).unwrap_or(0),
        output_tokens: o.get("outputTokens").and_then(Value::as_u64).unwrap_or(0),
        cache_read_tokens: o.get("cacheReadTokens").and_then(Value::as_u64),
        cache_write_tokens: o.get("cacheWriteTokens").and_then(Value::as_u64),
    })
}

/// Split a line that may contain multiple concatenated JSON objects.
/// Handles: `}{`, `}\n{`, `} {"`, etc.
fn split_json_objects(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    // Normalize "}\s*{" (any whitespace between objects) to "}|{" so we can split (keep braces)
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

/// Parse one JSON object into CursorStreamEvent(s).
fn parse_one_cursor_json(value: &Value) -> Vec<CursorStreamEvent> {
    let mut events = Vec::new();
    let t = value.get("type").and_then(Value::as_str).unwrap_or("");
    let subtype = value.get("subtype").and_then(Value::as_str).unwrap_or("");

    match t {
        "system" if subtype == "init" => {
            if let (Some(cwd), Some(model), Some(sid)) = (
                value.get("cwd").and_then(Value::as_str),
                value.get("model").and_then(Value::as_str),
                value.get("session_id").and_then(Value::as_str),
            ) {
                events.push(CursorStreamEvent::SystemInit {
                    cwd: cwd.to_string(),
                    model: model.to_string(),
                    session_id: sid.to_string(),
                });
            }
        }
        "user" => {
            events.push(CursorStreamEvent::Skip);
        }
        "thinking" if subtype == "delta" => {
            if let Some(text) = value.get("text").and_then(Value::as_str) {
                events.push(CursorStreamEvent::ThinkingDelta {
                    text: text.to_string(),
                });
            }
        }
        "thinking" if subtype == "completed" => {
            events.push(CursorStreamEvent::ThinkingCompleted);
        }
        "assistant" => {
            if let Some(text) = value
                .pointer("/message/content/0/text")
                .and_then(Value::as_str)
            {
                events.push(CursorStreamEvent::AgentMessage {
                    text: text.to_string(),
                    is_final: false,
                });
            }
        }
        "tool_call" => {
            let call_id = value.get("call_id").and_then(Value::as_str).unwrap_or("");
            let tc = value.get("tool_call").and_then(Value::as_object);
            if subtype == "started" {
                if let Some(edit) = tc
                    .and_then(|o| o.get("editToolCall"))
                    .and_then(Value::as_object)
                {
                    let path = edit
                        .get("args")
                        .and_then(|a| a.get("path"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    events.push(CursorStreamEvent::ToolCallStarted {
                        call_id: call_id.to_string(),
                        tool_type: "editToolCall".to_string(),
                        path: path.to_string(),
                    });
                } else if let Some(shell) = tc
                    .and_then(|o| o.get("shellToolCall"))
                    .and_then(Value::as_object)
                {
                    let args = shell.get("args").and_then(Value::as_object);
                    let command = args
                        .and_then(|a| a.get("command"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    events.push(CursorStreamEvent::ToolCallStartedGeneric {
                        call_id: call_id.to_string(),
                        tool_name: "Bash".to_string(),
                        payload: command.to_string(),
                    });
                } else if let Some(glob) = tc
                    .and_then(|o| o.get("globToolCall"))
                    .and_then(Value::as_object)
                {
                    let args = glob.get("args").and_then(Value::as_object);
                    let pattern = args
                        .and_then(|a| a.get("pattern").or_else(|| a.get("glob")))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    events.push(CursorStreamEvent::ToolCallStartedGeneric {
                        call_id: call_id.to_string(),
                        tool_name: "Glob".to_string(),
                        payload: pattern.to_string(),
                    });
                } else {
                    events.push(CursorStreamEvent::Skip);
                }
            } else if subtype == "completed" {
                if let Some(edit) = tc
                    .and_then(|o| o.get("editToolCall"))
                    .and_then(Value::as_object)
                {
                    let res = edit.get("result").and_then(|r| r.get("success"));
                    let path = res
                        .and_then(|s| s.get("path"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let lines_added = res
                        .and_then(|s| s.get("linesAdded"))
                        .and_then(Value::as_u64);
                    let message = res.and_then(|s| s.get("message")).and_then(Value::as_str);
                    events.push(CursorStreamEvent::ToolCallCompleted {
                        call_id: call_id.to_string(),
                        tool_type: "editToolCall".to_string(),
                        path: path.to_string(),
                        lines_added,
                        lines_created: None,
                        message: message.map(String::from),
                    });
                } else if let Some(shell) = tc
                    .and_then(|o| o.get("shellToolCall"))
                    .and_then(Value::as_object)
                {
                    let res = shell.get("result").and_then(Value::as_object);
                    let success = res.map(|r| r.get("success").is_some()).unwrap_or(false);
                    events.push(CursorStreamEvent::ToolCallCompletedGeneric {
                        call_id: call_id.to_string(),
                        success,
                    });
                } else if let Some(glob) = tc
                    .and_then(|o| o.get("globToolCall"))
                    .and_then(Value::as_object)
                {
                    let res = glob.get("result").and_then(Value::as_object);
                    let success = res.map(|r| r.get("success").is_some()).unwrap_or(false);
                    events.push(CursorStreamEvent::ToolCallCompletedGeneric {
                        call_id: call_id.to_string(),
                        success,
                    });
                } else {
                    events.push(CursorStreamEvent::Skip);
                }
            } else {
                events.push(CursorStreamEvent::Skip);
            }
        }
        "result" if subtype == "success" => {
            let duration_ms = value
                .get("duration_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let result = value
                .get("result")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let usage = value.get("usage").and_then(parse_cursor_usage);
            events.push(CursorStreamEvent::Result {
                duration_ms,
                result,
                usage,
            });
        }
        _ => {}
    }
    events
}

/// Parse Cursor stream-json line(s) into structured events.
/// Handles multiple concatenated JSON objects per line (e.g. `}{"type":"user"...`).
/// Returns Skip for user prompts and non-edit tool calls so they are not logged as raw.
pub fn parse_cursor_json_events(line: &str) -> Vec<CursorStreamEvent> {
    let mut all = Vec::new();
    let objects = split_json_objects(line);
    for obj in objects {
        let value: Value = match serde_json::from_str(&obj) {
            Ok(v) => v,
            Err(_) => continue,
        };
        all.extend(parse_one_cursor_json(&value));
    }
    all
}

impl CursorClient {
    /// Spawns Cursor CLI in headless print mode.
    ///
    /// Notes:
    /// - `-p` runs headless (single prompt)
    /// - `--force` avoids interactive approval prompts
    /// - `--output-format stream-json` produces NDJSON events
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

        let command = resolve_cursor_command(env_vars.as_ref())
            .ok_or_else(|| anyhow::anyhow!("Cursor CLI not found in PATH or override"))?;

        let mut cmd = Command::new(&command);
        cmd.arg("-p")
            .arg(instruction)
            .arg("--force")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--workspace")
            .arg(worktree_path)
            .current_dir(worktree_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        cmd.env("NO_COLOR", "1");
        if let Some(vars) = env_vars {
            for (k, v) in vars {
                if k == EXEC_CURSOR_CMD_ENV {
                    continue;
                }
                cmd.env(k, v);
            }
        }

        let child: AsyncGroupChild = cmd
            .group_spawn()
            .with_context(|| format!("Failed to spawn Cursor CLI in {:?}", worktree_path))?;

        let (interrupt_tx, interrupt_rx): (InterruptSender, InterruptReceiver) = oneshot::channel();

        Ok(SpawnedAgent {
            child,
            interrupt_sender: Some(interrupt_tx),
            interrupt_receiver: Some(interrupt_rx),
            msg_store: None,
        })
    }
}

impl Default for CursorClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_system_init() {
        let line = r#"{"type":"system","subtype":"init","cwd":"/path","session_id":"sid-1","model":"Claude 4.6 Opus"}"#;
        let events = parse_cursor_json_events(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            CursorStreamEvent::SystemInit {
                cwd,
                model,
                session_id,
            } => {
                assert_eq!(cwd, "/path");
                assert_eq!(model, "Claude 4.6 Opus");
                assert_eq!(session_id, "sid-1");
            }
            _ => panic!("expected SystemInit"),
        }
    }

    #[test]
    fn parse_assistant_message() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello"}]},"session_id":"x"}"#;
        let events = parse_cursor_json_events(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            CursorStreamEvent::AgentMessage { text, is_final } => {
                assert_eq!(text, "Hello");
                assert!(!is_final);
            }
            _ => panic!("expected AgentMessage"),
        }
    }

    #[test]
    fn parse_user_skip() {
        let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Hello"}]}}"#;
        let events = parse_cursor_json_events(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], CursorStreamEvent::Skip));
    }

    #[test]
    fn parse_multiple_json_objects() {
        let line = r#"{"type":"user","message":{}}{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hi"}]},"session_id":"x"}"#;
        let events = parse_cursor_json_events(line);
        assert!(
            events.len() >= 1,
            "expected at least Skip, got {:?}",
            events
        );
        assert!(events.iter().any(|e| matches!(e, CursorStreamEvent::Skip)));
    }

    #[test]
    fn parse_shell_tool_call_started() {
        let line = r#"{"type":"tool_call","subtype":"started","call_id":"x","tool_call":{"shellToolCall":{"args":{"command":"npm install"}}}}"#;
        let events = parse_cursor_json_events(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            CursorStreamEvent::ToolCallStartedGeneric {
                call_id,
                tool_name,
                payload,
            } => {
                assert_eq!(call_id, "x");
                assert_eq!(tool_name, "Bash");
                assert_eq!(payload, "npm install");
            }
            _ => panic!("expected ToolCallStartedGeneric, got {:?}", events[0]),
        }
    }

    #[test]
    fn parse_shell_tool_call_completed() {
        let line = r#"{"type":"tool_call","subtype":"completed","call_id":"x","tool_call":{"shellToolCall":{"result":{"success":{"exitCode":0}}}}}"#;
        let events = parse_cursor_json_events(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            CursorStreamEvent::ToolCallCompletedGeneric { call_id, success } => {
                assert_eq!(call_id, "x");
                assert!(*success);
            }
            _ => panic!("expected ToolCallCompletedGeneric"),
        }
    }

    #[test]
    fn parse_multiple_json_objects_with_whitespace() {
        // Real Cursor output can have space between concatenated objects: "} {"
        // Use }{ (no space) - same as parse_multiple_json_objects, but with shellToolCall
        let line = r#"{"type":"tool_call","subtype":"started","call_id":"x","tool_call":{"shellToolCall":{"args":{"command":"echo hi"}}}}{"type":"tool_call","subtype":"completed","call_id":"x","tool_call":{"shellToolCall":{"result":{"success":{"exitCode":0}}}}}"#;
        let events = parse_cursor_json_events(line);
        assert_eq!(events.len(), 2, "got {:?}", events);
        match &events[0] {
            CursorStreamEvent::ToolCallStartedGeneric {
                call_id,
                tool_name,
                payload,
            } => {
                assert_eq!(call_id, "x");
                assert_eq!(tool_name, "Bash");
                assert_eq!(payload, "echo hi");
            }
            _ => panic!("expected ToolCallStartedGeneric"),
        }
        match &events[1] {
            CursorStreamEvent::ToolCallCompletedGeneric { call_id, success } => {
                assert_eq!(call_id, "x");
                assert!(*success);
            }
            _ => panic!("expected ToolCallCompletedGeneric"),
        }
    }

    #[test]
    fn parse_result_success() {
        let line = r#"{"type":"result","subtype":"success","duration_ms":1000,"result":"Done","usage":{"inputTokens":10,"outputTokens":5}}"#;
        let events = parse_cursor_json_events(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            CursorStreamEvent::Result {
                duration_ms,
                result,
                usage,
            } => {
                assert_eq!(*duration_ms, 1000);
                assert_eq!(result, "Done");
                assert!(usage.is_some());
                let u = usage.as_ref().unwrap();
                assert_eq!(u.input_tokens, 10);
                assert_eq!(u.output_tokens, 5);
            }
            _ => panic!("expected Result"),
        }
    }

    #[test]
    fn resolve_cursor_command_prefers_exec_env_command() {
        let mut vars = HashMap::new();
        vars.insert(
            EXEC_CURSOR_CMD_ENV.to_string(),
            "/tmp/custom-agent".to_string(),
        );

        let resolved = resolve_cursor_command(Some(&vars)).expect("expected command resolution");
        assert_eq!(resolved, "/tmp/custom-agent");
    }
}
