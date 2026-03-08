use crate::assistant_log_buffer::AgentTextBuffer;
use crate::claude::{ClaudeRuntimeSkillConfig, SpawnedAgent};
use crate::msg_store::{LogMsg, MsgStore};
use crate::process::{kill_process_group, terminate_process, InterruptSender};
use crate::retry_handler::{RetryHandler, RetryScheduleResult};
use crate::session::ClaudeSessionManager;
use crate::worktree::{format_repository_clone_log, format_repository_sync_log, repo_url_matches};
use crate::{
    append_assistant_log, build_skill_instruction_context, format_loaded_skills_log_line,
    AgentEvent, AssistantLogMessage, AttemptSuccessHook, ClaudeClient, CodexClient, CursorClient,
    DeployContextPreparer, GeminiClient, GitOpsHandler, RuntimeSkillLoadResult,
    RuntimeSkillSearchResult, SkillInstructionContext, SkillKnowledgeHandle, SkillKnowledgeStatus,
    SkillRuntime, StatusManager, WorktreeManager,
};
use acpms_db::models::{
    project_repo_relative_path, AttemptStatus, InitSource, InitTaskMetadata, Project,
    ProjectSettings, ProjectType, RepositoryAccessMode, RepositoryContext, SystemSettings, Task,
    TaskAttempt, TaskType,
};
use acpms_db::ProjectTypeDetector;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use command_group::AsyncGroupChild;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::sync::{broadcast, watch};
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Default timeout for spawning an agent process (30 seconds).
const SPAWN_TIMEOUT: Duration = Duration::from_secs(30);

/// Default timeout for graceful shutdown (5 seconds).
const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Max time to wait for agent process exit after stream already ended.
/// Prevents attempts from hanging forever in "running" when provider process
/// keeps an idle session open.
const AGENT_EXIT_TIMEOUT_AFTER_STREAM: Duration = Duration::from_secs(20);

/// Maximum execution time for init tasks (30 minutes).
#[allow(dead_code)]
const INIT_TASK_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Maximum file size (in bytes) to store in file_diffs content columns.
/// Files larger than this will have their content omitted.
const MAX_DIFF_CONTENT_SIZE: usize = 1_048_576; // 1MB
/// Git empty tree object hash (used as base for first-commit diffs).
const GIT_EMPTY_TREE_HASH: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
const EXEC_CODEX_CMD_ENV: &str = "ACPMS_EXEC_CODEX_CMD";
const EXEC_CODEX_USE_NPX_ENV: &str = "ACPMS_EXEC_CODEX_USE_NPX";
const EXEC_GEMINI_CMD_ENV: &str = "ACPMS_EXEC_GEMINI_CMD";
const EXEC_GEMINI_USE_NPX_ENV: &str = "ACPMS_EXEC_GEMINI_USE_NPX";
const EXEC_CURSOR_CMD_ENV: &str = "ACPMS_EXEC_CURSOR_CMD";
const OVERRIDE_CODEX_BIN_ENV: &str = "ACPMS_AGENT_CODEX_BIN";
const OVERRIDE_GEMINI_BIN_ENV: &str = "ACPMS_AGENT_GEMINI_BIN";
const OVERRIDE_CURSOR_BIN_ENV: &str = "ACPMS_AGENT_CURSOR_BIN";
const OVERRIDE_NPX_BIN_ENV: &str = "ACPMS_AGENT_NPX_BIN";
const MAX_RUNTIME_SKILL_CONTENT_CHARS: usize = 12_000;
mod init_flow;
mod persistence_helpers;
mod post_init;

#[derive(Debug, Clone, Deserialize)]
struct TaskAttachmentMetadata {
    key: String,
    filename: Option<String>,
    content_type: Option<String>,
    size: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct PreparedReferenceFile {
    key: String,
    filename: String,
    local_path: String,
    content_type: Option<String>,
    size: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct PreparedReferenceFailure {
    key: Option<String>,
    filename: Option<String>,
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
struct PreparedReferenceManifest {
    attempt_id: Uuid,
    task_id: Uuid,
    total_requested: usize,
    downloaded: usize,
    failed: usize,
    manifest_path: String,
    files: Vec<PreparedReferenceFile>,
    failures: Vec<PreparedReferenceFailure>,
}

static GITLAB_PAT_REGEX: Lazy<Option<Regex>> =
    Lazy::new(|| Regex::new(r"glpat-[A-Za-z0-9_-]{20,}").ok());
static EMAIL_REGEX: Lazy<Option<Regex>> =
    Lazy::new(|| Regex::new(r"(?i)\b[a-z0-9._%+\-]+@[a-z0-9.\-]+\.[a-z]{2,}\b").ok());

/// Sanitize log content to redact sensitive information.
///
/// ## Security
/// - Redacts GitLab PAT patterns (glpat-*)
/// - Prevents credential leakage in logs and broadcasts
pub fn sanitize_log(line: &str) -> String {
    let mut sanitized = line.to_string();

    if let Some(pat_regex) = GITLAB_PAT_REGEX.as_ref() {
        sanitized = pat_regex
            .replace_all(&sanitized, "***GITLAB_PAT_REDACTED***")
            .to_string();
    }
    if let Some(email_regex) = EMAIL_REGEX.as_ref() {
        sanitized = email_regex
            .replace_all(&sanitized, "***EMAIL_REDACTED***")
            .to_string();
    }

    sanitized
}

/// Drop verbose telemetry/debug lines that do not add user-facing value.
pub fn should_skip_log_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return true;
    }

    // Codex telemetry
    if trimmed.contains("codex_otel::traces::otel_manager")
        || trimmed.contains("event.name=\"codex.sse_event\"")
        || trimmed.starts_with("DEBUG codex_exec: Received event:")
        || trimmed.contains("terminal.type=")
        || trimmed.contains("user.account_id=")
    {
        return true;
    }

    // Claude/Node router service noise
    if trimmed.starts_with("Service not running, starting service")
        || trimmed.contains("claude code router service has been successfully stopped")
    {
        return true;
    }

    // Node/npm verbose internal noise (not user-actionable)
    if trimmed.starts_with("npm timing ")
        || trimmed.starts_with("npm sill ")
        || trimmed.starts_with("npm verb ")
        || trimmed.contains("node:internal/")
        || (trimmed.starts_with("at ") && trimmed.contains("node:internal"))
    {
        return true;
    }

    // Cursor stream-json: raw JSON lines (tool_call, user, etc.) — normalize via parser, don't log raw
    if (trimmed.starts_with(r#"{"type":"#) || trimmed.starts_with(r#"{"type": "#))
        && (trimmed.contains(r#""call_id""#)
            || trimmed.contains(r#""tool_call""#)
            || trimmed.contains(r#""session_id""#))
    {
        return true;
    }

    // Gemini stream-json: raw JSON (type: message|tool_use|tool_result|result) — normalize via parser, don't log raw
    if (trimmed.starts_with(r#"{"type":"#) || trimmed.starts_with(r#"{"type": "#))
        && (trimmed.contains(r#""type":"message""#)
            || trimmed.contains(r#""type": "message""#)
            || trimmed.contains(r#""type":"tool_use""#)
            || trimmed.contains(r#""type": "tool_use""#)
            || trimmed.contains(r#""type":"tool_result""#)
            || trimmed.contains(r#""type": "tool_result""#)
            || trimmed.contains(r#""type":"result""#)
            || trimmed.contains(r#""type": "result""#))
    {
        return true;
    }
    // Gemini result JSON may appear mid-line (e.g. after assistant text)
    if (trimmed.contains(r#""type":"result""#) || trimmed.contains(r#""type": "result""#))
        && trimmed.contains(r#""stats""#)
    {
        return true;
    }

    false
}

/// Normalize stderr for user-facing display: sanitize, filter noise, truncate long lines.
/// Returns None if line should be skipped; otherwise Some(normalized_string).
pub fn normalize_stderr_for_display(line: &str) -> Option<String> {
    let sanitized = sanitize_log(line.trim());
    if sanitized.is_empty() || should_skip_log_line(&sanitized) {
        return None;
    }
    // Truncate very long lines to keep UI readable (stack traces, minified errors)
    const MAX_STDERR_CHARS: usize = 600;
    if sanitized.len() > MAX_STDERR_CHARS {
        let mut cut = MAX_STDERR_CHARS - 20; // "... (truncated)"
        while cut > 0 && !sanitized.is_char_boundary(cut) {
            cut -= 1;
        }
        return Some(format!("{}... (truncated)", &sanitized[..cut]));
    }
    Some(sanitized)
}

fn normalize_assistant_text_for_dedupe(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_immediate_duplicate_assistant_text(last: Option<&str>, current: &str) -> bool {
    let normalized_current = normalize_assistant_text_for_dedupe(current);
    if normalized_current.is_empty() {
        return false;
    }
    let Some(last_text) = last else {
        return false;
    };
    normalize_assistant_text_for_dedupe(last_text) == normalized_current
}

fn strip_matching_quotes(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.len() < 2 {
        return trimmed;
    }

    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return trimmed;
    };
    let Some(last) = trimmed.chars().last() else {
        return trimmed;
    };

    if (first == '\'' && last == '\'') || (first == '"' && last == '"') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

fn normalize_shell_command(command: &str) -> String {
    let trimmed = command.trim();
    for marker in [" -lc ", " -c "] {
        if let Some(idx) = trimmed.find(marker) {
            let inner = &trimmed[idx + marker.len()..];
            return strip_matching_quotes(inner).trim().to_string();
        }
    }
    strip_matching_quotes(trimmed).trim().to_string()
}

fn tokenize_shell_command(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(|part| {
            part.trim_matches(|c: char| c == '"' || c == '\'' || c == '`' || c == ';' || c == ',')
                .to_string()
        })
        .filter(|part| !part.is_empty())
        .collect()
}

fn command_segment_tokens(tokens: &[String], start_idx: usize) -> &[String] {
    let end_idx = tokens
        .iter()
        .enumerate()
        .skip(start_idx)
        .find_map(|(idx, token)| {
            matches!(
                token.as_str(),
                "|" | "||" | "&&" | ">" | ">>" | "<" | "2>" | "2>>" | "1>" | "1>>"
            )
            .then_some(idx)
        })
        .unwrap_or(tokens.len());

    &tokens[start_idx.min(tokens.len())..end_idx]
}

fn first_non_flag_argument(tokens: &[String], start_idx: usize) -> Option<String> {
    command_segment_tokens(tokens, start_idx)
        .iter()
        .find(|token| token.as_str() != "--" && !token.starts_with('-'))
        .cloned()
}

fn last_non_flag_argument(tokens: &[String], start_idx: usize) -> Option<String> {
    command_segment_tokens(tokens, start_idx)
        .iter()
        .rev()
        .find(|token| token.as_str() != "--" && !token.starts_with('-'))
        .cloned()
}

fn classify_successful_shell_command(
    command: &str,
) -> Option<(String, crate::sdk_normalized_types::ActionType)> {
    use crate::sdk_normalized_types::ActionType;

    let normalized = normalize_shell_command(command);
    if normalized.is_empty() {
        return None;
    }

    let tokens = tokenize_shell_command(&normalized);
    if tokens.is_empty() {
        return None;
    }

    let mut command_idx = 0usize;
    if tokens[0].eq_ignore_ascii_case("sudo") && tokens.len() > 1 {
        command_idx = 1;
    }

    let primary = tokens[command_idx]
        .rsplit('/')
        .next()
        .unwrap_or(tokens[command_idx].as_str())
        .to_ascii_lowercase();
    let argument_start = command_idx.saturating_add(1);

    match primary.as_str() {
        "rg" | "grep" | "ag" | "ack" | "fd" | "fdfind" | "ls" | "tree" => {
            let query = first_non_flag_argument(&tokens, argument_start)
                .unwrap_or_else(|| normalized.clone());
            Some(("Grep".to_string(), ActionType::Search { query }))
        }
        "find" => Some((
            "Grep".to_string(),
            ActionType::Search {
                query: normalized.clone(),
            },
        )),
        "cat" | "less" | "more" | "head" | "tail" => {
            let path = first_non_flag_argument(&tokens, argument_start)?;
            if path.trim().is_empty() {
                return None;
            }
            Some(("Read".to_string(), ActionType::FileRead { path }))
        }
        "sed" => {
            let path = last_non_flag_argument(&tokens, argument_start)?;
            if path.trim().is_empty() {
                return None;
            }
            // Common sed script arg pattern, not a file target.
            if path
                .chars()
                .all(|ch| ch.is_ascii_digit() || ch == ',' || ch == 'p')
            {
                return None;
            }
            Some(("Read".to_string(), ActionType::FileRead { path }))
        }
        "awk" => {
            let path = last_non_flag_argument(&tokens, argument_start)?;
            if path.trim().is_empty() {
                return None;
            }
            // awk script fragments are often mistaken as positional args by whitespace tokenization.
            if path.contains('{')
                || path.contains('}')
                || path.contains('$')
                || path.eq_ignore_ascii_case("print")
            {
                return None;
            }
            Some(("Read".to_string(), ActionType::FileRead { path }))
        }
        "curl" | "wget" => {
            let url = tokens
                .iter()
                .skip(argument_start)
                .find(|token| token.starts_with("http://") || token.starts_with("https://"))
                .cloned()?;
            if url.trim().is_empty() {
                return None;
            }
            Some(("WebFetch".to_string(), ActionType::WebFetch { url }))
        }
        _ => None,
    }
}

/// Stores active agent session info for termination control.
struct ActiveSession {
    /// Interrupt sender for graceful shutdown.
    interrupt_sender: Option<InterruptSender>,
    /// Child process for force termination.
    child: Arc<Mutex<Option<AsyncGroupChild>>>,
    /// Realtime user input channel (if supported by current executor mode).
    input_sender: Option<mpsc::UnboundedSender<String>>,
}

/// Active Project Assistant session (session_id keyed).
struct AssistantActiveSession {
    interrupt_sender: Option<InterruptSender>,
    child: Arc<Mutex<Option<AsyncGroupChild>>>,
    input_sender: Option<mpsc::UnboundedSender<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentCliProvider {
    ClaudeCode,
    OpenAiCodex,
    GeminiCli,
    CursorCli,
}

impl AgentCliProvider {
    const ALL: [Self; 4] = [
        Self::ClaudeCode,
        Self::OpenAiCodex,
        Self::GeminiCli,
        Self::CursorCli,
    ];

    fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "claude-code" => Self::ClaudeCode,
            "openai-codex" | "codex" => Self::OpenAiCodex,
            "gemini-cli" | "gemini" => Self::GeminiCli,
            "cursor-cli" | "cursor" => Self::CursorCli,
            _ => Self::ClaudeCode,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::OpenAiCodex => "openai-codex",
            Self::GeminiCli => "gemini-cli",
            Self::CursorCli => "cursor-cli",
        }
    }

    fn display_name(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::OpenAiCodex => "OpenAI Codex",
            Self::GeminiCli => "Google Gemini",
            Self::CursorCli => "Cursor CLI",
        }
    }

    fn fallback_order(default_provider: Self) -> Vec<Self> {
        let mut ordered = Vec::with_capacity(Self::ALL.len());
        ordered.push(default_provider);
        ordered.extend(
            Self::ALL
                .into_iter()
                .filter(|provider| *provider != default_provider),
        );
        ordered
    }
}

fn normalize_assistant_plain_stdout_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return None;
    }

    let sanitized = sanitize_log(trimmed);
    if sanitized.is_empty() || should_skip_log_line(&sanitized) {
        return None;
    }

    Some(sanitized)
}

fn detect_provider_auth_blocker(provider: AgentCliProvider, line: &str) -> Option<&'static str> {
    let normalized = line.to_ascii_lowercase();

    match provider {
        AgentCliProvider::GeminiCli => {
            if normalized.contains("enter the authorization code")
                || normalized.contains("please visit the following url to authorize")
                || normalized.contains("opening authentication page in your browser")
                || normalized.contains("do you want to continue? [y/n]")
            {
                return Some(
                    "Gemini CLI requires authentication. Open Settings -> Agent Provider, run Sign in/Re-auth for Gemini CLI, then retry.",
                );
            }
            None
        }
        AgentCliProvider::CursorCli => {
            if normalized.contains("run `agent login`")
                || normalized.contains("run agent login")
                || normalized.contains("not logged in")
                || normalized.contains("not authenticated")
            {
                return Some(
                    "Cursor CLI requires authentication. Open Settings -> Agent Provider, run Sign in/Re-auth for Cursor CLI, then retry.",
                );
            }
            None
        }
        _ => None,
    }
}

fn flatten_runtime_detail(detail: Option<&str>) -> String {
    detail
        .unwrap_or("No detail available.")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn runtime_status_label(status: &SkillKnowledgeStatus) -> &'static str {
    match status {
        SkillKnowledgeStatus::Disabled => "disabled",
        SkillKnowledgeStatus::Pending => "pending",
        SkillKnowledgeStatus::Ready => "ready",
        SkillKnowledgeStatus::Failed => "failed",
        SkillKnowledgeStatus::NoMatches => "no_matches",
    }
}

fn truncate_runtime_skill_content(content: &str, max_chars: usize) -> (String, bool) {
    if content.chars().count() <= max_chars {
        return (content.to_string(), false);
    }

    let truncated = content.chars().take(max_chars).collect::<String>();
    (format!("{truncated}\n... (truncated)"), true)
}

fn format_runtime_skill_search_summary(query: &str, result: &RuntimeSkillSearchResult) -> String {
    match result.status {
        SkillKnowledgeStatus::Ready if !result.matches.is_empty() => {
            let items = result
                .matches
                .iter()
                .map(|skill| {
                    format!(
                        "{}@{} ({}%)",
                        skill.skill_id,
                        skill.origin,
                        (skill.score * 100.0).round() as i32
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Runtime skill search: query=\"{}\" -> [{}]",
                query.trim(),
                items
            )
        }
        _ => format!(
            "Runtime skill search: query=\"{}\" -> {} ({})",
            query.trim(),
            runtime_status_label(&result.status),
            flatten_runtime_detail(result.detail.as_deref())
        ),
    }
}

fn format_runtime_skill_search_follow_up(query: &str, result: &RuntimeSkillSearchResult) -> String {
    let mut lines = vec![format!(
        "ACPMS runtime skill search results for query: \"{}\"",
        query.trim()
    )];

    match result.status {
        SkillKnowledgeStatus::Ready if !result.matches.is_empty() => {
            lines.push(
                "If one is useful, load it by printing exactly one JSON object on its own line with no markdown fences:".to_string(),
            );
            lines.push(r#"{"tool":"load_skill","args":{"skill_id":"<skill-id>"}}"#.to_string());
            for (idx, skill) in result.matches.iter().enumerate() {
                lines.push(format!(
                    "{}. {} | origin={} | relevance={}% | source={}",
                    idx + 1,
                    skill.skill_id,
                    skill.origin,
                    (skill.score * 100.0).round() as i32,
                    skill.source_path
                ));
                if !skill.description.trim().is_empty() {
                    lines.push(format!("   {}", skill.description.trim()));
                }
            }
        }
        _ => {
            lines.push(flatten_runtime_detail(result.detail.as_deref()));
            lines.push(
                "Continue with the currently loaded skills unless you want to retry later."
                    .to_string(),
            );
        }
    }

    lines.join("\n")
}

fn format_runtime_skill_load_summary(skill_id: &str, result: &RuntimeSkillLoadResult) -> String {
    match (&result.status, &result.skill) {
        (SkillKnowledgeStatus::Ready, Some(skill)) => format!(
            "Runtime skill loaded: {}@{}",
            skill.skill_id,
            skill.origin.as_deref().unwrap_or("unknown")
        ),
        _ => format!(
            "Runtime skill load: skill_id=\"{}\" -> {} ({})",
            skill_id.trim(),
            runtime_status_label(&result.status),
            flatten_runtime_detail(result.detail.as_deref())
        ),
    }
}

fn format_runtime_skill_load_follow_up(skill_id: &str, result: &RuntimeSkillLoadResult) -> String {
    match &result.skill {
        Some(skill) if result.status == SkillKnowledgeStatus::Ready => {
            let (content, was_truncated) =
                truncate_runtime_skill_content(&skill.content, MAX_RUNTIME_SKILL_CONTENT_CHARS);
            let mut lines = vec![
                "ACPMS runtime skill loaded.".to_string(),
                format!("skill_id: {}", skill.skill_id),
                format!("origin: {}", skill.origin.as_deref().unwrap_or("unknown")),
            ];
            if let Some(source_path) = &skill.source_path {
                lines.push(format!("source: {}", source_path));
            }
            if was_truncated {
                lines.push(
                    "The skill body was truncated to fit runtime context. Follow the visible guidance first.".to_string(),
                );
            }
            lines.push("Apply this skill where relevant for the rest of the attempt.".to_string());
            lines.push(String::new());
            lines.push(content);
            lines.join("\n")
        }
        _ => format!(
            "ACPMS runtime skill load failed for `{}`.\n{}",
            skill_id.trim(),
            flatten_runtime_detail(result.detail.as_deref())
        ),
    }
}

pub struct ExecutorOrchestrator {
    db_pool: PgPool,
    worktree_manager: WorktreeManager,
    claude_client: ClaudeClient,
    codex_client: CodexClient,
    gemini_client: GeminiClient,
    cursor_client: CursorClient,
    session_manager: ClaudeSessionManager,
    broadcast_tx: broadcast::Sender<AgentEvent>,
    /// Active agent sessions mapped by attempt_id for termination control.
    active_sessions: Arc<Mutex<HashMap<Uuid, ActiveSession>>>,
    #[allow(dead_code)]
    preview_enabled: bool,
    /// AES-256-GCM cipher for decrypting sensitive data (PATs, tokens).
    cipher: Aes256Gcm,
    /// Approval service for SDK mode tool permissions.
    approval_service: Arc<dyn crate::approval::ApprovalService>,
    /// S3 storage service for persisting diff snapshots.
    storage_service: Arc<dyn crate::diff_snapshot::DiffStorageUploader>,
    /// Optional pre-success hook (deployment/report finalization).
    attempt_success_hook: Option<Arc<dyn AttemptSuccessHook>>,
    /// Optional deploy context preparer (SSH key + config for agent to deploy directly).
    deploy_context_preparer: Option<Arc<dyn DeployContextPreparer>>,
    /// Active Project Assistant sessions (session_id -> session).
    active_assistant_sessions: Arc<Mutex<HashMap<Uuid, AssistantActiveSession>>>,
    /// Shared global skill knowledge handle for RAG suggestions.
    skill_knowledge: SkillKnowledgeHandle,
}

impl ExecutorOrchestrator {
    fn is_claude_sdk_turn_complete_line(line: &str) -> bool {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            return false;
        };

        let message_type = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map(|value| value.to_ascii_lowercase());

        if matches!(message_type.as_deref(), Some("result")) {
            return true;
        }

        if !matches!(message_type.as_deref(), Some("message_stop")) {
            return false;
        }

        let stop_reason = [
            "/message/stop_reason",
            "/event/message/stop_reason",
            "/stream_event/event/message/stop_reason",
            "/stop_reason",
            "/stopReason",
        ]
        .iter()
        .find_map(|path| value.pointer(path).and_then(serde_json::Value::as_str))
        .map(|value| value.to_ascii_lowercase());

        matches!(stop_reason.as_deref(), Some("end_turn"))
    }

    pub(super) async fn wait_for_claude_sdk_turn_completion(
        &self,
        msg_store: Arc<MsgStore>,
        timeout: Duration,
    ) -> Result<()> {
        let stream = msg_store.history_plus_stream();
        tokio::pin!(stream);

        tokio::time::timeout(timeout, async {
            while let Some(message) = stream.next().await {
                match message? {
                    LogMsg::Stdout(line) | LogMsg::Stderr(line) => {
                        if Self::is_claude_sdk_turn_complete_line(&line) {
                            return Ok(());
                        }
                    }
                    LogMsg::Finished => return Ok(()),
                }
            }

            Ok(())
        })
        .await
        .map_err(|_| anyhow::anyhow!("Claude SDK turn did not finish within {:?}", timeout))?
    }

    pub fn new(
        db_pool: PgPool,
        worktrees_path: std::sync::Arc<tokio::sync::RwLock<std::path::PathBuf>>,
        broadcast_tx: broadcast::Sender<AgentEvent>,
        storage_service: Arc<dyn crate::diff_snapshot::DiffStorageUploader>,
    ) -> Result<Self> {
        let session_manager = ClaudeSessionManager::new()?;

        // Initialize encryption cipher from ENCRYPTION_KEY env var
        let key_base64 = std::env::var("ENCRYPTION_KEY")
            .context("ENCRYPTION_KEY environment variable not set")?;
        let key_bytes = BASE64
            .decode(&key_base64)
            .context("Failed to decode base64 encryption key")?;
        if key_bytes.len() != 32 {
            bail!(
                "Invalid encryption key length: expected 32 bytes, got {}",
                key_bytes.len()
            );
        }
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to create AES-256-GCM cipher: {}", e))?;

        // Initialize approval service for SDK mode (uses main broadcast_tx)
        let approval_service =
            crate::approval::DatabaseApprovalService::new(db_pool.clone(), broadcast_tx.clone());

        Ok(Self {
            db_pool,
            worktree_manager: WorktreeManager::new(worktrees_path.clone()),
            claude_client: ClaudeClient::new(),
            codex_client: CodexClient::new(),
            gemini_client: GeminiClient::new(),
            cursor_client: CursorClient::new(),
            session_manager,
            broadcast_tx,
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
            preview_enabled: false, // Can be configured via env var
            cipher,
            approval_service,
            storage_service,
            attempt_success_hook: None,
            deploy_context_preparer: None,
            active_assistant_sessions: Arc::new(Mutex::new(HashMap::new())),
            skill_knowledge: SkillKnowledgeHandle::disabled(),
        })
    }

    pub fn with_attempt_success_hook(mut self, hook: Arc<dyn AttemptSuccessHook>) -> Self {
        self.attempt_success_hook = Some(hook);
        self
    }

    pub fn with_deploy_context_preparer(
        mut self,
        preparer: Arc<dyn DeployContextPreparer>,
    ) -> Self {
        self.deploy_context_preparer = Some(preparer);
        self
    }

    pub fn with_skill_knowledge(mut self, handle: SkillKnowledgeHandle) -> Self {
        self.skill_knowledge = handle;
        self
    }

    pub fn skill_knowledge(&self) -> SkillKnowledgeHandle {
        self.skill_knowledge.clone()
    }

    fn build_skill_instruction_context(
        &self,
        task: &Task,
        settings: &ProjectSettings,
        project_type: ProjectType,
        repo_path: Option<&Path>,
    ) -> SkillInstructionContext {
        build_skill_instruction_context(
            task,
            settings,
            project_type,
            repo_path,
            Some(&self.skill_knowledge),
        )
    }

    async fn log_loaded_skills(
        &self,
        attempt_id: Uuid,
        context: &SkillInstructionContext,
    ) -> Result<()> {
        let message = format_loaded_skills_log_line(context);
        self.log(attempt_id, "system", &message).await
    }

    async fn emit_runtime_capable_assistant_chunk(
        &self,
        attempt_id: Uuid,
        repo_path: &Path,
        buffer: &mut AgentTextBuffer,
        text: &str,
    ) -> Result<()> {
        buffer.push(text);

        let mut emitted_any = false;
        while let Some((content, metadata)) = buffer.pop_next() {
            emitted_any = true;
            self.handle_runtime_capable_agent_output(
                attempt_id,
                repo_path,
                &content,
                metadata.as_ref(),
            )
            .await?;
        }

        if !emitted_any {
            if let Some((content, metadata)) = buffer.pop_partial_text_for_display() {
                self.handle_runtime_capable_agent_output(
                    attempt_id,
                    repo_path,
                    &content,
                    metadata.as_ref(),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn flush_runtime_capable_assistant_buffer(
        &self,
        attempt_id: Uuid,
        repo_path: &Path,
        buffer: &mut AgentTextBuffer,
    ) -> Result<()> {
        if let Some((content, metadata)) = buffer.flush() {
            self.handle_runtime_capable_agent_output(
                attempt_id,
                repo_path,
                &content,
                metadata.as_ref(),
            )
            .await?;
        }

        Ok(())
    }

    async fn handle_runtime_capable_agent_output(
        &self,
        attempt_id: Uuid,
        repo_path: &Path,
        content: &str,
        metadata: Option<&serde_json::Value>,
    ) -> Result<()> {
        if !content.trim().is_empty() {
            StatusManager::log_assistant_delta(
                &self.db_pool,
                &self.broadcast_tx,
                attempt_id,
                content,
            )
            .await?;
        }

        if let Some(metadata) = metadata {
            self.handle_runtime_skill_tool_calls(attempt_id, repo_path, metadata)
                .await?;
        }

        Ok(())
    }

    async fn handle_runtime_skill_tool_calls(
        &self,
        attempt_id: Uuid,
        repo_path: &Path,
        metadata: &serde_json::Value,
    ) -> Result<()> {
        let Some(tool_calls) = metadata
            .get("tool_calls")
            .and_then(|value| value.as_array())
        else {
            return Ok(());
        };

        let runtime = SkillRuntime::new(Some(&self.skill_knowledge));
        for tool_call in tool_calls {
            let Some(name) = tool_call.get("name").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(args) = tool_call.get("args").and_then(|value| value.as_object()) else {
                continue;
            };

            match name {
                "search_skills" => {
                    let Some(query) = args.get("query").and_then(|value| value.as_str()) else {
                        continue;
                    };
                    let top_k = args
                        .get("top_k")
                        .and_then(|value| value.as_u64())
                        .map(|value| value as usize)
                        .unwrap_or(5);
                    let result = runtime.search_runtime(query, top_k);
                    let summary = format_runtime_skill_search_summary(query, &result);
                    self.log(attempt_id, "system", &summary).await?;

                    let follow_up = format_runtime_skill_search_follow_up(query, &result);
                    if let Err(error) = self.send_input(attempt_id, &follow_up).await {
                        warn!(
                            attempt_id = %attempt_id,
                            error = %error,
                            query = %query,
                            "Failed to deliver runtime skill search response"
                        );
                        let message = format!(
                            "Failed to deliver runtime skill search response to agent: {}",
                            error
                        );
                        let _ = self.log(attempt_id, "stderr", &message).await;
                    }
                }
                "load_skill" => {
                    let Some(skill_id) = args.get("skill_id").and_then(|value| value.as_str())
                    else {
                        continue;
                    };
                    let result = runtime.load_runtime(skill_id, Some(repo_path));
                    let summary = format_runtime_skill_load_summary(skill_id, &result);
                    self.log(attempt_id, "system", &summary).await?;

                    let follow_up = format_runtime_skill_load_follow_up(skill_id, &result);
                    if let Err(error) = self.send_input(attempt_id, &follow_up).await {
                        warn!(
                            attempt_id = %attempt_id,
                            error = %error,
                            skill_id = %skill_id,
                            "Failed to deliver runtime skill load response"
                        );
                        let message = format!(
                            "Failed to deliver runtime skill load response to agent: {}",
                            error
                        );
                        let _ = self.log(attempt_id, "stderr", &message).await;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Verify Claude session exists before execution.
    /// Uses spawn_blocking for has_any_project to avoid blocking tokio runtime
    /// (fs::read_dir can be slow on large dirs).
    async fn verify_claude_session(&self, attempt_id: Option<Uuid>) -> Result<()> {
        let manager = self.session_manager.clone();
        let has_projects = tokio::task::spawn_blocking(move || manager.has_any_project())
            .await
            .context("has_any_project task panicked")?
            .context("Failed to check Claude sessions")?;

        if !has_projects {
            let error_msg =
                "No Claude session found. Please login via: npx @anthropic-ai/claude-code";
            if let Some(attempt_id) = attempt_id {
                let _ = self.log(attempt_id, "system", error_msg).await;
            }
            anyhow::bail!(error_msg);
        }

        info!("Claude project(s) available");
        Ok(())
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

    fn is_executable_file(path: &Path) -> bool {
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
            if Self::is_executable_file(&path) {
                return Some(path.to_string_lossy().to_string());
            }
            return None;
        }

        let path_var = std::env::var_os("PATH")?;
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(trimmed);
            if Self::is_executable_file(&candidate) {
                return Some(candidate.to_string_lossy().to_string());
            }
        }

        None
    }

    fn resolve_command_with_override(default_cmd: &str, override_env: &str) -> Option<String> {
        if let Some(override_cmd) = Self::read_non_empty_env(override_env) {
            return Self::resolve_command_in_path(&override_cmd);
        }
        Self::resolve_command_in_path(default_cmd)
    }

    fn resolve_npx_command() -> Option<String> {
        if let Some(override_cmd) = Self::read_non_empty_env(OVERRIDE_NPX_BIN_ENV) {
            return Self::resolve_command_in_path(&override_cmd);
        }
        Self::resolve_command_in_path("npx")
    }

    fn resolve_codex_command() -> Option<(String, bool)> {
        if let Some(cmd) = Self::resolve_command_with_override("codex", OVERRIDE_CODEX_BIN_ENV) {
            return Some((cmd, false));
        }
        Self::resolve_npx_command().map(|cmd| (cmd, true))
    }

    fn resolve_gemini_command() -> Option<(String, bool)> {
        if let Some(cmd) = Self::resolve_command_with_override("gemini", OVERRIDE_GEMINI_BIN_ENV) {
            return Some((cmd, false));
        }
        Self::resolve_npx_command().map(|cmd| (cmd, true))
    }

    fn resolve_cursor_command() -> Option<String> {
        Self::resolve_command_with_override("agent", OVERRIDE_CURSOR_BIN_ENV)
    }

    fn is_truthy_flag(value: Option<&String>) -> bool {
        value
            .map(|v| v.trim().to_ascii_lowercase())
            .map(|v| !matches!(v.as_str(), "0" | "false" | "off" | "no" | ""))
            .unwrap_or(false)
    }

    fn first_non_empty_line(text: &str) -> Option<String> {
        text.lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(ToString::to_string)
    }

    async fn run_non_interactive_cli_probe(
        command: &str,
        args: &[String],
        timeout_secs: u64,
    ) -> Result<(bool, String, String, bool)> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if std::env::var_os("TERM").is_none() {
            cmd.env("TERM", "xterm-256color");
        }
        // Hint CLIs to avoid interactive UI mode during health probe.
        cmd.env("CI", "1");

        let output =
            match tokio::time::timeout(Duration::from_secs(timeout_secs), cmd.output()).await {
                Ok(result) => result.context("Failed to execute CLI probe")?,
                Err(_) => return Ok((false, String::new(), String::new(), true)),
            };

        Ok((
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
            false,
        ))
    }

    async fn verify_gemini_auth_readiness(
        provider_env: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        let env =
            provider_env.ok_or_else(|| anyhow::anyhow!("Missing Gemini provider runtime env"))?;
        let command = env
            .get(EXEC_GEMINI_CMD_ENV)
            .map(String::as_str)
            .ok_or_else(|| anyhow::anyhow!("Gemini command path is missing"))?;
        let use_npx = Self::is_truthy_flag(env.get(EXEC_GEMINI_USE_NPX_ENV));
        let args: Vec<String> = if use_npx {
            vec![
                "-y".to_string(),
                "@google/gemini-cli".to_string(),
                "--list-sessions".to_string(),
            ]
        } else {
            vec!["--list-sessions".to_string()]
        };

        let (success, stdout, stderr, timed_out) =
            Self::run_non_interactive_cli_probe(command, &args, 10).await?;
        if timed_out {
            bail!("Gemini CLI auth probe timed out. Please authenticate Gemini in Settings and retry.");
        }

        let combined = format!("{}\n{}", stdout, stderr);
        if let Some(hint) = detect_provider_auth_blocker(AgentCliProvider::GeminiCli, &combined) {
            bail!(hint);
        }

        if !success {
            let reason = Self::first_non_empty_line(&stdout)
                .or_else(|| Self::first_non_empty_line(&stderr))
                .unwrap_or_else(|| "Gemini CLI auth probe failed".to_string());
            bail!("Gemini CLI is not ready: {}", reason);
        }

        Ok(())
    }

    fn resolve_provider_command_env(
        provider: AgentCliProvider,
    ) -> Result<Option<HashMap<String, String>>> {
        let mut provider_env: HashMap<String, String> = HashMap::new();

        match provider {
            AgentCliProvider::ClaudeCode => {}
            AgentCliProvider::OpenAiCodex => {
                let Some((cmd, use_npx)) = Self::resolve_codex_command() else {
                    bail!("Codex CLI not found. Install: npm i -g @openai/codex");
                };
                provider_env.insert(EXEC_CODEX_CMD_ENV.to_string(), cmd);
                if use_npx {
                    provider_env.insert(EXEC_CODEX_USE_NPX_ENV.to_string(), "1".to_string());
                }
            }
            AgentCliProvider::GeminiCli => {
                let Some((cmd, use_npx)) = Self::resolve_gemini_command() else {
                    bail!(
                        "Gemini CLI not found. Install: npx @google/gemini-cli or npm i -g @google/gemini-cli (macOS: brew install gemini-cli)"
                    );
                };
                provider_env.insert(EXEC_GEMINI_CMD_ENV.to_string(), cmd);
                if use_npx {
                    provider_env.insert(EXEC_GEMINI_USE_NPX_ENV.to_string(), "1".to_string());
                }
            }
            AgentCliProvider::CursorCli => {
                let Some(cmd) = Self::resolve_cursor_command() else {
                    bail!("Cursor CLI not found. Install: curl https://cursor.com/install -fsS | bash");
                };
                provider_env.insert(EXEC_CURSOR_CMD_ENV.to_string(), cmd);
            }
        }

        Ok(if provider_env.is_empty() {
            None
        } else {
            Some(provider_env)
        })
    }

    async fn resolve_provider_readiness(
        &self,
        provider: AgentCliProvider,
        attempt_id_for_log: Option<Uuid>,
    ) -> Result<Option<HashMap<String, String>>> {
        if matches!(provider, AgentCliProvider::ClaudeCode) {
            self.verify_claude_session(attempt_id_for_log).await?;
        }

        let provider_env = Self::resolve_provider_command_env(provider)?;

        if matches!(provider, AgentCliProvider::GeminiCli) {
            if let Err(err) = Self::verify_gemini_auth_readiness(provider_env.as_ref()).await {
                if let Some(attempt_id) = attempt_id_for_log {
                    let _ = self.log(attempt_id, "system", &err.to_string()).await;
                }
                return Err(err);
            }
        }

        Ok(provider_env)
    }

    async fn resolve_selected_agent_cli_with_fallback(
        &self,
        attempt_id_for_log: Option<Uuid>,
    ) -> Result<(AgentCliProvider, Option<HashMap<String, String>>)> {
        let settings = self.fetch_system_settings().await?;
        let selected_provider = AgentCliProvider::from_str(&settings.agent_cli_provider);
        let mut failure_reasons: Vec<String> = Vec::new();

        for provider in AgentCliProvider::fallback_order(selected_provider) {
            match self
                .resolve_provider_readiness(provider, attempt_id_for_log)
                .await
            {
                Ok(provider_env) => {
                    if provider != selected_provider {
                        let fallback_msg = format!(
                            "Default agent provider {} is unavailable. Falling back to {}.",
                            selected_provider.display_name(),
                            provider.display_name()
                        );
                        if let Some(attempt_id) = attempt_id_for_log {
                            let _ = self.log(attempt_id, "system", &fallback_msg).await;
                        }
                        warn!(
                            selected_provider = selected_provider.as_str(),
                            fallback_provider = provider.as_str(),
                            "Agent provider fallback activated"
                        );
                    }
                    return Ok((provider, provider_env));
                }
                Err(err) => {
                    let reason = format!("{} unavailable: {}", provider.display_name(), err);
                    if let Some(attempt_id) = attempt_id_for_log {
                        let _ = self.log(attempt_id, "system", &reason).await;
                    }
                    warn!(
                        provider = provider.as_str(),
                        error = %err,
                        "Agent provider unavailable during runtime selection"
                    );
                    failure_reasons.push(reason);
                }
            }
        }

        if failure_reasons.is_empty() {
            bail!("No available agent provider found");
        }
        bail!(
            "No available agent provider found. {}",
            failure_reasons.join(" | ")
        );
    }

    async fn resolve_agent_cli(
        &self,
        attempt_id: Uuid,
    ) -> Result<(AgentCliProvider, Option<HashMap<String, String>>)> {
        self.resolve_selected_agent_cli_with_fallback(Some(attempt_id))
            .await
    }

    async fn resolve_agent_cli_for_assistant(
        &self,
    ) -> Result<(AgentCliProvider, Option<HashMap<String, String>>)> {
        self.resolve_selected_agent_cli_with_fallback(None).await
    }

    async fn set_attempt_executor(
        &self,
        attempt_id: Uuid,
        provider: AgentCliProvider,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE task_attempts
               SET metadata = metadata || jsonb_build_object('executor', $2::text)
               WHERE id = $1"#,
        )
        .bind(attempt_id)
        .bind(provider.as_str())
        .execute(&self.db_pool)
        .await
        .context("Failed to set attempt executor metadata")?;

        Ok(())
    }

    fn sanitize_reference_filename(filename: &str) -> String {
        let mut out = String::with_capacity(filename.len());
        for ch in filename.chars() {
            let c = ch.to_ascii_lowercase();
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                out.push(c);
            } else {
                out.push('_');
            }
        }
        let trimmed = out.trim_matches('_');
        if trimmed.is_empty() {
            "attachment.bin".to_string()
        } else {
            trimmed.to_string()
        }
    }

    fn fallback_attachment_filename(key: &str) -> String {
        key.rsplit('/')
            .next()
            .map(Self::sanitize_reference_filename)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "attachment.bin".to_string())
    }

    fn append_reference_context_to_instruction(
        instruction: String,
        manifest: &PreparedReferenceManifest,
    ) -> String {
        if manifest.files.is_empty() && manifest.failures.is_empty() {
            return instruction;
        }

        let mut section = String::new();
        section.push_str("\n\n## Attached References\n");
        if manifest.files.is_empty() {
            section
                .push_str("Task has attached references, but none were downloaded successfully.\n");
        } else {
            section.push_str(
                "The following files are available locally. Read them before implementing changes:\n",
            );

            for file in &manifest.files {
                if let Some(content_type) = &file.content_type {
                    section.push_str(&format!(
                        "- `{}` ({}, {} bytes)\n",
                        file.local_path,
                        content_type,
                        file.size.unwrap_or(0)
                    ));
                } else {
                    section.push_str(&format!(
                        "- `{}` ({} bytes)\n",
                        file.local_path,
                        file.size.unwrap_or(0)
                    ));
                }
            }
        }

        section.push_str(&format!(
            "\nReference manifest: `{}`\n",
            manifest.manifest_path
        ));

        if !manifest.failures.is_empty() {
            section.push_str(
                "Note: Some references could not be downloaded. Mention this in your final summary.\n",
            );
        }

        format!("{}{}", instruction, section)
    }

    async fn prepare_task_references(
        &self,
        attempt_id: Uuid,
        task_id: Uuid,
        worktree_path: &Path,
    ) -> Result<Option<PreparedReferenceManifest>> {
        let (task_metadata, requirement_id): (serde_json::Value, Option<Uuid>) =
            sqlx::query_as("SELECT metadata, requirement_id FROM tasks WHERE id = $1")
                .bind(task_id)
                .fetch_optional(&self.db_pool)
                .await
                .context("Failed to fetch task for references")?
                .map(|row: (serde_json::Value, Option<Uuid>)| row)
                .unwrap_or((serde_json::json!({}), None));

        let mut attachments: Vec<TaskAttachmentMetadata> = task_metadata
            .get("attachments")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        serde_json::from_value::<TaskAttachmentMetadata>(item.clone()).ok()
                    })
                    .filter(|item| !item.key.trim().is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // When task is linked to a requirement, also include requirement's attachments
        // (file references are often stored on the requirement, not the task)
        if let Some(rid) = requirement_id {
            if let Some(req_metadata) = sqlx::query_scalar::<_, Option<serde_json::Value>>(
                "SELECT metadata FROM requirements WHERE id = $1",
            )
            .bind(rid)
            .fetch_optional(&self.db_pool)
            .await
            .context("Failed to fetch requirement metadata for references")?
            .flatten()
            {
                let req_attachments: Vec<TaskAttachmentMetadata> = req_metadata
                    .get("attachments")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| {
                                serde_json::from_value::<TaskAttachmentMetadata>(item.clone()).ok()
                            })
                            .filter(|item| !item.key.trim().is_empty())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                let mut seen: std::collections::HashSet<String> =
                    attachments.iter().map(|a| a.key.clone()).collect();
                for a in req_attachments {
                    if seen.insert(a.key.clone()) {
                        attachments.push(a);
                    }
                }
            }
        }

        if attachments.is_empty() {
            return Ok(None);
        }

        let refs_root = worktree_path.join(".acpms").join("references");
        tokio::fs::create_dir_all(&refs_root)
            .await
            .with_context(|| format!("Failed to create references directory: {:?}", refs_root))?;

        let mut files: Vec<PreparedReferenceFile> = Vec::new();
        let mut failures: Vec<PreparedReferenceFailure> = Vec::new();

        for (idx, attachment) in attachments.iter().enumerate() {
            let raw_name = attachment
                .filename
                .as_deref()
                .map(Self::sanitize_reference_filename)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| Self::fallback_attachment_filename(&attachment.key));
            let local_name = format!("{:02}_{}", idx + 1, raw_name);
            let local_path = refs_root.join(&local_name);
            let relative_path = format!(".acpms/references/{}", local_name);

            match self
                .storage_service
                .download_object_bytes(&attachment.key)
                .await
            {
                Ok(bytes) => {
                    if let Err(err) = tokio::fs::write(&local_path, &bytes).await {
                        failures.push(PreparedReferenceFailure {
                            key: Some(attachment.key.clone()),
                            filename: attachment.filename.clone(),
                            reason: format!("Failed to write local file: {}", err),
                        });
                        continue;
                    }

                    files.push(PreparedReferenceFile {
                        key: attachment.key.clone(),
                        filename: raw_name,
                        local_path: relative_path,
                        content_type: attachment.content_type.clone(),
                        size: attachment.size.or(Some(bytes.len() as u64)),
                    });
                }
                Err(err) => {
                    failures.push(PreparedReferenceFailure {
                        key: Some(attachment.key.clone()),
                        filename: attachment.filename.clone(),
                        reason: err.to_string(),
                    });
                }
            }
        }

        let manifest = PreparedReferenceManifest {
            attempt_id,
            task_id,
            total_requested: attachments.len(),
            downloaded: files.len(),
            failed: failures.len(),
            manifest_path: ".acpms/references/refs_manifest.json".to_string(),
            files,
            failures,
        };

        let manifest_json = serde_json::to_vec_pretty(&manifest)
            .context("Failed to serialize references manifest")?;
        tokio::fs::write(refs_root.join("refs_manifest.json"), manifest_json)
            .await
            .context("Failed to write references manifest")?;

        let references_value =
            serde_json::to_value(&manifest).context("Failed to convert references to JSON")?;
        sqlx::query(
            r#"
            UPDATE task_attempts
            SET metadata = metadata || jsonb_build_object('references', $2::jsonb)
            WHERE id = $1
            "#,
        )
        .bind(attempt_id)
        .bind(references_value)
        .execute(&self.db_pool)
        .await
        .context("Failed to persist reference manifest in attempt metadata")?;

        Ok(Some(manifest))
    }

    /// Patch task metadata with has_issue flag for kanban display
    async fn patch_task_has_issue(
        &self,
        task_id: Uuid,
        has_issue: bool,
        reason: Option<&str>,
    ) -> Result<()> {
        let mut patch = serde_json::Map::new();
        patch.insert("has_issue".to_string(), serde_json::json!(has_issue));
        if let Some(r) = reason {
            patch.insert("has_issue_reason".to_string(), serde_json::json!(r));
        } else {
            patch.insert("has_issue_reason".to_string(), serde_json::Value::Null);
        }
        sqlx::query(
            r#"
            UPDATE tasks
            SET metadata = COALESCE(metadata, '{}'::jsonb) || $2::jsonb,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(task_id)
        .bind(serde_json::Value::Object(patch))
        .execute(&self.db_pool)
        .await
        .context("Failed to patch task has_issue metadata")?;
        Ok(())
    }

    /// Execute task with cancellation support
    pub async fn execute_task_with_cancel(
        &self,
        attempt_id: Uuid,
        repo_path: PathBuf,
        instruction: String,
        cancel_rx: watch::Receiver<bool>,
    ) -> Result<()> {
        // Check for early cancellation
        if *cancel_rx.borrow() {
            self.cancel_attempt(attempt_id, "Cancelled before execution started")
                .await?;
            anyhow::bail!("Task cancelled before execution");
        }

        // Update status to RUNNING
        self.update_status(attempt_id, AttemptStatus::Running)
            .await?;

        // Resolve selected agent CLI and verify it is ready
        let (provider, provider_env) = match self.resolve_agent_cli(attempt_id).await {
            Ok(v) => v,
            Err(e) => {
                self.fail_attempt(attempt_id, &e.to_string()).await?;
                return Err(e);
            }
        };

        // Verify Claude session exists
        if matches!(provider, AgentCliProvider::ClaudeCode) {
            // Already verified by resolve_agent_cli
        }

        // Clone repository if needed
        if let Err(e) = self.ensure_repo_cloned(attempt_id, &repo_path).await {
            self.fail_attempt(attempt_id, &e.to_string()).await?;
            return Err(e);
        }

        // Create worktree
        let worktree_path = match self.create_worktree(attempt_id, &repo_path).await {
            Ok(path) => path,
            Err(e) => {
                self.fail_attempt(attempt_id, &e.to_string()).await?;
                return Err(e);
            }
        };

        // Check cancellation before spawning agent
        if *cancel_rx.borrow() {
            self.cleanup_and_cancel(attempt_id, &repo_path, "Cancelled before agent spawn")
                .await?;
            anyhow::bail!("Task cancelled before agent spawn");
        }

        // Spawn and execute agent
        let execution_result = self
            .execute_agent(
                attempt_id,
                &worktree_path,
                &instruction,
                cancel_rx.clone(),
                provider,
                provider_env,
            )
            .await;

        // Handle execution result
        match execution_result {
            Ok(_) => {
                self.log(
                    attempt_id,
                    "system",
                    "Agent execution completed. Capturing diff snapshot...",
                )
                .await?;

                // Save file diffs to S3 before any GitOps branch operations.
                info!(
                    "📸 [DIFF CAPTURE TRIGGER] About to save diffs for attempt {}",
                    attempt_id
                );
                if let Err(e) = self.save_diffs_to_s3(attempt_id, &worktree_path).await {
                    warn!(
                        "📸 [DIFF CAPTURE ERROR] Failed to save diffs for attempt {}: {}",
                        attempt_id, e
                    );
                    // Don't fail the attempt if diff capture fails
                }

                self.log(attempt_id, "system", "Syncing changes with repository...")
                    .await?;

                // Persist structured outputs (including MR_TITLE, MR_DESCRIPTION) before GitOps
                if let Err(err) = self
                    .persist_structured_outputs_from_attempt_logs(attempt_id, Some(&worktree_path))
                    .await
                {
                    warn!(
                        "Failed to persist structured outputs for attempt {}: {}",
                        attempt_id, err
                    );
                    self.log(
                        attempt_id,
                        "stderr",
                        &format!(
                            "Warning: failed to persist structured deployment/report outputs: {}",
                            err
                        ),
                    )
                    .await?;
                }

                // GitOps: Create MR (agent already pushed)
                if let Err(e) = self.handle_gitops(attempt_id).await {
                    self.log(
                        attempt_id,
                        "stderr",
                        &format!("Could not sync with repository: {}", e),
                    )
                    .await?;
                }

                if let Err(e) = self.run_before_success_hook(attempt_id).await {
                    tracing::error!(
                        attempt_id = %attempt_id,
                        error = %e,
                        "Deployment hook failed (Cloudflare/preview)"
                    );
                    let user_msg = Self::format_deployment_hook_failure_user_message(&e);
                    self.log(attempt_id, "system", &user_msg).await?;
                    // Do NOT fail attempt: Cloudflare/deployment errors are not attempt failures
                }

                self.update_status(attempt_id, AttemptStatus::Success)
                    .await?;
                self.log(attempt_id, "system", "✅ Task completed successfully")
                    .await?;
            }
            Err(e) => {
                self.log(
                    attempt_id,
                    "system",
                    &format!("Failed to start agent: {}", e),
                )
                .await?;
                self.fail_attempt(attempt_id, &e.to_string()).await?;
            }
        }

        // Cleanup worktree
        self.log(attempt_id, "system", "Cleaning up...").await?;
        if let Err(e) = self.cleanup_attempt_worktree(&repo_path, attempt_id).await {
            self.log(
                attempt_id,
                "system",
                &format!("Warning: Cleanup failed: {}", e),
            )
            .await?;
        }

        Ok(())
    }

    /// Execute task with review flow support
    /// - If require_review=true: Don't cleanup worktree, set task to InReview
    /// - If require_review=false: Cleanup worktree, create MR, set task to Done
    pub async fn execute_task_with_cancel_review(
        &self,
        attempt_id: Uuid,
        task_id: Uuid,
        repo_path: PathBuf,
        instruction: String,
        cancel_rx: watch::Receiver<bool>,
        require_review: bool,
    ) -> Result<()> {
        // Check for early cancellation
        if *cancel_rx.borrow() {
            self.cancel_attempt(attempt_id, "Cancelled before execution started")
                .await?;
            anyhow::bail!("Task cancelled before execution");
        }

        // Update status to RUNNING
        self.update_status(attempt_id, AttemptStatus::Running)
            .await?;

        // Resolve selected agent CLI and verify it is ready
        let (provider, provider_env) = match self.resolve_agent_cli(attempt_id).await {
            Ok(v) => v,
            Err(e) => {
                self.fail_attempt_with_retry(attempt_id, task_id, &e.to_string())
                    .await?;
                return Err(e);
            }
        };

        // Clone repository if needed
        if let Err(e) = self.ensure_repo_cloned(attempt_id, &repo_path).await {
            self.fail_attempt_with_retry(attempt_id, task_id, &e.to_string())
                .await?;
            return Err(e);
        }

        // Create worktree
        let worktree_path = match self.create_worktree(attempt_id, &repo_path).await {
            Ok(path) => path,
            Err(e) => {
                self.fail_attempt_with_retry(attempt_id, task_id, &e.to_string())
                    .await?;
                return Err(e);
            }
        };

        // Store worktree path in attempt metadata for later use (review/approve)
        self.store_worktree_path(attempt_id, &worktree_path).await?;

        // Materialize task attachments into the worktree and append local reference paths
        // to the instruction so the agent can read them directly.
        let mut effective_instruction = instruction;
        match self
            .prepare_task_references(attempt_id, task_id, &worktree_path)
            .await
        {
            Ok(Some(manifest)) => {
                self.log(
                    attempt_id,
                    "system",
                    &format!(
                        "Prepared {} reference file(s) for this attempt.",
                        manifest.downloaded
                    ),
                )
                .await?;
                if manifest.failed > 0 {
                    self.log(
                        attempt_id,
                        "stderr",
                        &format!(
                            "Warning: {} reference file(s) could not be downloaded.",
                            manifest.failed
                        ),
                    )
                    .await?;
                    // Mark task as having issues for kanban display
                    let reason: String = manifest
                        .failures
                        .iter()
                        .map(|f| f.reason.as_str())
                        .collect::<Vec<_>>()
                        .join("; ");
                    self.patch_task_has_issue(task_id, true, Some(&reason))
                        .await?;
                } else {
                    // Clear has_issue when references succeeded
                    self.patch_task_has_issue(task_id, false, None).await?;
                }
                effective_instruction =
                    Self::append_reference_context_to_instruction(effective_instruction, &manifest);
            }
            Ok(None) => {}
            Err(err) => {
                warn!(
                    "Failed to prepare references for attempt {} task {}: {}",
                    attempt_id, task_id, err
                );
                self.log(
                    attempt_id,
                    "stderr",
                    &format!("Warning: failed to prepare references: {}", err),
                )
                .await?;
                self.patch_task_has_issue(task_id, true, Some(&err.to_string()))
                    .await?;
            }
        }

        // For Deploy tasks or tasks that mention cancel/stop/cleanup: prepare SSH key + config
        if let Some(preparer) = &self.deploy_context_preparer {
            let task: Option<Task> = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
                .bind(task_id)
                .fetch_optional(&self.db_pool)
                .await
                .ok()
                .flatten();
            let needs_deploy_context =
                matches!(task.as_ref().map(|t| t.task_type), Some(TaskType::Deploy))
                    || task.as_ref().map_or(false, |t| {
                        let mut haystack = t.title.to_lowercase();
                        haystack.push(' ');
                        if let Some(d) = &t.description {
                            haystack.push_str(&d.to_lowercase());
                        }
                        let needles = [
                            "cancel deploy",
                            "dừng deploy",
                            "stop deploy",
                            "stop container",
                            "dừng container",
                            "docker down",
                            "cleanup deploy",
                        ];
                        needles.iter().any(|n| haystack.contains(n))
                    });
            if needs_deploy_context {
                if let Err(e) = preparer.prepare(attempt_id, &worktree_path).await {
                    warn!(
                        attempt_id = %attempt_id,
                        error = %e,
                        "Deploy context preparation failed"
                    );
                    self.log(
                        attempt_id,
                        "stderr",
                        &format!(
                            "Warning: could not prepare deploy context (SSH key/config). \
                             Deploy may fail: {}",
                            e
                        ),
                    )
                    .await?;
                } else {
                    self.log(
                        attempt_id,
                        "system",
                        "Deploy context prepared (.acpms/deploy/). Use it to SSH and deploy.",
                    )
                    .await?;
                }
            }
        }

        // Check cancellation before spawning agent
        if *cancel_rx.borrow() {
            self.cleanup_and_cancel(attempt_id, &repo_path, "Cancelled before agent spawn")
                .await?;
            anyhow::bail!("Task cancelled before agent spawn");
        }

        // Spawn and execute agent
        let agent_env_for_followup = provider_env.clone().unwrap_or_default();

        let execution_result = self
            .execute_agent(
                attempt_id,
                &worktree_path,
                &effective_instruction,
                cancel_rx.clone(),
                provider,
                provider_env,
            )
            .await;

        // Handle execution result
        match execution_result {
            Ok(_) => {
                // If user cancelled (e.g. via terminate_session), attempt may already be cancelled.
                // Cleanup worktree and return early — do not run success-path logic.
                let current_status: Option<String> =
                    sqlx::query_scalar("SELECT status::text FROM task_attempts WHERE id = $1")
                        .bind(attempt_id)
                        .fetch_optional(&self.db_pool)
                        .await
                        .ok()
                        .flatten();
                if current_status.as_deref() == Some("cancelled") {
                    self.log(
                        attempt_id,
                        "system",
                        "Attempt was cancelled. Cleaning up worktree...",
                    )
                    .await?;
                    if let Err(e) = self.cleanup_attempt_worktree(&repo_path, attempt_id).await {
                        self.log(
                            attempt_id,
                            "stderr",
                            &format!("Worktree cleanup failed after cancel: {}", e),
                        )
                        .await?;
                    }
                    return Ok(());
                }

                let mut preview_target_from_deploy_validation: Option<String> = None;

                match self
                    .maybe_run_agent_driven_deploy_validation(
                        attempt_id,
                        task_id,
                        &worktree_path,
                        provider,
                        &agent_env_for_followup,
                    )
                    .await
                {
                    Ok(Some(preview_target)) => {
                        preview_target_from_deploy_validation = Some(preview_target.clone());
                        self.log(
                            attempt_id,
                            "system",
                            &format!("🌐 PREVIEW_TARGET: {}", preview_target),
                        )
                        .await?;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let msg = format!("Deployment validation failed: {}", e);
                        self.log(attempt_id, "stderr", &msg).await?;
                        self.fail_attempt_with_retry(attempt_id, task_id, &msg)
                            .await?;
                        self.log(
                            attempt_id,
                            "system",
                            "Deployment validation failed. Cleaning up failed worktree...",
                        )
                        .await?;
                        if let Err(cleanup_err) =
                            self.cleanup_attempt_worktree(&repo_path, attempt_id).await
                        {
                            self.log(
                                attempt_id,
                                "stderr",
                                &format!(
                                    "Worktree cleanup failed after deployment validation error: {}",
                                    cleanup_err
                                ),
                            )
                            .await?;
                        }
                        return Err(anyhow::anyhow!(msg));
                    }
                }

                match self
                    .persist_structured_outputs_from_attempt_logs(
                        attempt_id,
                        Some(worktree_path.as_path()),
                    )
                    .await
                {
                    Ok(structured) => {
                        if let Some(preview_target) = structured.preview_target {
                            let already_logged = preview_target_from_deploy_validation
                                .as_ref()
                                .map(|logged| logged == &preview_target)
                                .unwrap_or(false);
                            if !already_logged {
                                self.log(
                                    attempt_id,
                                    "system",
                                    &format!("🌐 PREVIEW_TARGET: {}", preview_target),
                                )
                                .await?;
                            }
                        }
                        if let Some(preview_url) = structured.preview_url {
                            self.log(
                                attempt_id,
                                "system",
                                &format!("🔗 PREVIEW_URL: {}", preview_url),
                            )
                            .await?;
                        }
                        if structured.deployment_report.is_some() {
                            self.log(
                                attempt_id,
                                "system",
                                "📋 Parsed deployment report fields from agent output.",
                            )
                            .await?;
                        }
                        if structured.mr_title.is_some() || structured.mr_description.is_some() {
                            self.log(
                                attempt_id,
                                "system",
                                "📝 Parsed MR title/description from agent output.",
                            )
                            .await?;
                        }
                    }
                    Err(err) => {
                        warn!(
                            "Failed to persist structured outputs for attempt {}: {}",
                            attempt_id, err
                        );
                        self.log(
                            attempt_id,
                            "stderr",
                            &format!(
                                "Warning: failed to persist structured deployment/report outputs: {}",
                                err
                            ),
                        )
                        .await?;
                    }
                }

                if require_review {
                    // Review required: set task to InReview, keep worktree for review
                    self.log(
                        attempt_id,
                        "system",
                        "Agent execution completed. Awaiting human review...",
                    )
                    .await?;
                    self.log(
                        attempt_id,
                        "system",
                        "Changes saved for review. Will cleanup after you approve or reject.",
                    )
                    .await?;
                    if let Err(e) = self.run_before_success_hook(attempt_id).await {
                        tracing::error!(
                            attempt_id = %attempt_id,
                            error = %e,
                            "Deployment hook failed (Cloudflare/preview)"
                        );
                        let user_msg = Self::format_deployment_hook_failure_user_message(&e);
                        self.log(attempt_id, "system", &user_msg).await?;
                        // Do NOT fail attempt: Cloudflare/deployment errors are not attempt failures
                    }

                    self.update_status(attempt_id, AttemptStatus::Success)
                        .await?;
                    self.mark_task_in_review(task_id).await?;

                    // Save file diffs to S3 (worktree still exists)
                    info!("📸 [DIFF CAPTURE TRIGGER] About to save diffs for attempt {} (review mode)", attempt_id);
                    if let Err(e) = self.save_diffs_to_s3(attempt_id, &worktree_path).await {
                        warn!(
                            "📸 [DIFF CAPTURE ERROR] Failed to save diffs for attempt {}: {}",
                            attempt_id, e
                        );
                        // Don't fail the attempt if diff capture fails
                    }

                    // NOTE: Worktree is NOT cleaned up here - it will be cleaned up after approve/reject
                } else {
                    // No review: GitOps + auto-merge + cleanup only after merge succeeds
                    self.log(
                        attempt_id,
                        "system",
                        "Agent execution completed. Finalizing branch...",
                    )
                    .await?;

                    let branch_ready = match self
                        .finalize_branch_for_no_review(attempt_id, &worktree_path)
                        .await
                    {
                        Ok(_) => true,
                        Err(e) => {
                            self.log(
                                attempt_id,
                                "stderr",
                                &format!(
                                    "Failed to finalize branch commit/push for no-review flow: {}",
                                    e
                                ),
                            )
                            .await?;
                            false
                        }
                    };

                    self.log(attempt_id, "system", "Saving changes...").await?;

                    // Save file diffs to S3 from the final branch state.
                    info!("📸 [DIFF CAPTURE TRIGGER] About to save diffs for attempt {} (no-review mode)", attempt_id);
                    if let Err(e) = self.save_diffs_to_s3(attempt_id, &worktree_path).await {
                        warn!(
                            "📸 [DIFF CAPTURE ERROR] Failed to save diffs for attempt {}: {}",
                            attempt_id, e
                        );
                        // Don't fail the attempt if diff capture fails
                    }

                    if branch_ready {
                        self.log(attempt_id, "system", "Syncing changes with repository...")
                            .await?;
                    }

                    if let Err(e) = self.run_before_success_hook(attempt_id).await {
                        tracing::error!(
                            attempt_id = %attempt_id,
                            error = %e,
                            "Deployment hook failed (Cloudflare/preview)"
                        );
                        let user_msg = Self::format_deployment_hook_failure_user_message(&e);
                        self.log(attempt_id, "system", &user_msg).await?;
                        // Do NOT fail attempt: Cloudflare/deployment errors are not attempt failures
                    }

                    self.update_status(attempt_id, AttemptStatus::Success)
                        .await?;

                    let auto_merged = if branch_ready {
                        match self.handle_gitops(attempt_id).await {
                            Ok(_) => match self.handle_gitops_merge(attempt_id).await {
                                Ok(merged) => merged,
                                Err(e) => {
                                    self.log(
                                        attempt_id,
                                        "stderr",
                                        &format!("Auto-merge failed: {}", e),
                                    )
                                    .await?;
                                    false
                                }
                            },
                            Err(e) => {
                                self.log(
                                    attempt_id,
                                    "stderr",
                                    &format!("Could not sync with repository: {}", e),
                                )
                                .await?;
                                false
                            }
                        }
                    } else {
                        false
                    };

                    if auto_merged {
                        self.mark_task_completed(task_id).await?;

                        // Cleanup worktree only after merge is confirmed.
                        self.log(attempt_id, "system", "Cleaning up...").await?;
                        if let Err(e) = self.cleanup_attempt_worktree(&repo_path, attempt_id).await
                        {
                            self.log(
                                attempt_id,
                                "system",
                                &format!("Warning: Cleanup failed: {}", e),
                            )
                            .await?;
                        }

                        // Emit final report for the user after cleanup.
                        if let Err(report_err) = self.emit_completion_report(attempt_id).await {
                            self.log(
                                attempt_id,
                                "stderr",
                                &format!("Failed to emit final attempt report: {}", report_err),
                            )
                            .await?;
                        }
                    } else {
                        // When diff is 0/0, no MR was created - nothing to review. Mark Done and cleanup.
                        let (additions, deletions) = sqlx::query_as::<_, (Option<i32>, Option<i32>)>(
                            "SELECT diff_total_additions, diff_total_deletions FROM task_attempts WHERE id = $1",
                        )
                        .bind(attempt_id)
                        .fetch_optional(&self.db_pool)
                        .await?
                        .unwrap_or((None, None));
                        let has_code_changes =
                            additions.unwrap_or(0) > 0 || deletions.unwrap_or(0) > 0;

                        if !has_code_changes {
                            self.log(
                                attempt_id,
                                "system",
                                "No code changes to merge. Marking task complete.",
                            )
                            .await?;
                            self.mark_task_completed(task_id).await?;
                            self.log(attempt_id, "system", "Cleaning up...").await?;
                            if let Err(e) =
                                self.cleanup_attempt_worktree(&repo_path, attempt_id).await
                            {
                                self.log(
                                    attempt_id,
                                    "system",
                                    &format!("Warning: Cleanup failed: {}", e),
                                )
                                .await?;
                            }
                            if let Err(report_err) = self.emit_completion_report(attempt_id).await {
                                self.log(
                                    attempt_id,
                                    "stderr",
                                    &format!("Failed to emit final attempt report: {}", report_err),
                                )
                                .await?;
                            }
                        } else {
                            // Preserve changes for manual review.
                            self.mark_task_in_review(task_id).await?;
                            let in_review_msg = if !branch_ready {
                                "Task moved to review. Repository sync hit a recoverable Git issue; your changes are preserved. Approve after fixing repository state, or send a follow-up to continue from the same attempt."
                            } else {
                                "Task moved to review. Please approve to merge and complete."
                            };
                            self.log(attempt_id, "system", in_review_msg).await?;
                        }
                    }
                }
            }
            Err(e) => {
                self.log(
                    attempt_id,
                    "system",
                    &format!("Failed to start agent: {}", e),
                )
                .await?;
                self.fail_attempt_with_retry(attempt_id, task_id, &e.to_string())
                    .await?;
                // Cleanup on failure
                self.log(
                    attempt_id,
                    "system",
                    "Task failed. Cleaning up failed worktree...",
                )
                .await?;
                if let Err(cleanup_err) =
                    self.cleanup_attempt_worktree(&repo_path, attempt_id).await
                {
                    self.log(
                        attempt_id,
                        "stderr",
                        &format!("Worktree cleanup failed after task error: {}", cleanup_err),
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }

    async fn execute_agent(
        &self,
        attempt_id: Uuid,
        worktree_path: &Path,
        instruction: &str,
        mut cancel_rx: watch::Receiver<bool>,
        provider: AgentCliProvider,
        provider_env: Option<HashMap<String, String>>,
    ) -> Result<()> {
        self.set_attempt_executor(attempt_id, provider).await?;

        self.log(
            attempt_id,
            "system",
            &format!("Starting {} Agent...", provider.display_name()),
        )
        .await?;

        // Load agent settings (Claude-only for now)
        let agent_settings = self.load_agent_settings(attempt_id).await?;
        // Live input queue for this session.
        // - Claude SDK: forwarded via protocol `send_user_message`.
        // - Codex/Gemini: forwarded to process stdin (best effort).
        let (session_input_sender, provider_input_rx) = mpsc::unbounded_channel::<String>();
        let (claude_input_rx, mut stdio_input_rx) =
            if matches!(provider, AgentCliProvider::ClaudeCode) {
                (Some(provider_input_rx), None)
            } else {
                (None, Some(provider_input_rx))
            };

        // Spawn agent with timeout
        let spawned = tokio::time::timeout(SPAWN_TIMEOUT, async {
            match provider {
                AgentCliProvider::ClaudeCode => {
                    self.claude_client
                        .spawn_session_sdk(
                            worktree_path,
                            instruction,
                            attempt_id,
                            provider_env,
                            Some(self.approval_service.clone()),
                            Some(self.db_pool.clone()),
                            Some(self.broadcast_tx.clone()),
                            Some(&agent_settings),
                            claude_input_rx,
                            Some(ClaudeRuntimeSkillConfig {
                                repo_path: worktree_path.to_path_buf(),
                                skill_knowledge: self.skill_knowledge.clone(),
                            }),
                        )
                        .await
                }
                AgentCliProvider::OpenAiCodex => {
                    self.codex_client
                        .spawn_session(worktree_path, instruction, attempt_id, provider_env)
                        .await
                }
                AgentCliProvider::GeminiCli => {
                    self.gemini_client
                        .spawn_session(worktree_path, instruction, attempt_id, provider_env)
                        .await
                }
                AgentCliProvider::CursorCli => {
                    self.cursor_client
                        .spawn_session(worktree_path, instruction, attempt_id, provider_env)
                        .await
                }
            }
        })
        .await
        .map_err(|_| {
            anyhow::anyhow!("Timeout: agent took more than {:?} to start", SPAWN_TIMEOUT)
        })??;

        let SpawnedAgent {
            child,
            interrupt_sender,
            interrupt_receiver,
            msg_store,
        } = spawned;

        // Store session for termination control (with cleanup guard)
        let child_arc = Arc::new(Mutex::new(Some(child)));
        {
            let session = ActiveSession {
                interrupt_sender,
                child: child_arc.clone(),
                input_sender: Some(session_input_sender),
            };
            self.active_sessions
                .lock()
                .await
                .insert(attempt_id, session);
        }

        // Ensure cleanup happens even on error
        let cleanup_guard = scopeguard::guard((), {
            let sessions = self.active_sessions.clone();
            move |_| {
                // Synchronous cleanup - session will be removed
                tokio::spawn(async move {
                    sessions.lock().await.remove(&attempt_id);
                    debug!("Cleaned up session for attempt {}", attempt_id);
                });
            }
        });

        // Take child from Arc for streaming
        let mut child_opt = child_arc.lock().await.take();
        let child_ref = child_opt
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Child process not available"))?;

        self.attach_execution_process_pid(attempt_id, child_ref)
            .await;

        // For non-SDK providers, drain queued live input into child stdin.
        if !matches!(provider, AgentCliProvider::ClaudeCode) {
            if let Some(mut rx) = stdio_input_rx.take() {
                if let Some(mut stdin) = child_ref.inner().stdin.take() {
                    let pool = self.db_pool.clone();
                    let tx = self.broadcast_tx.clone();
                    tokio::spawn(async move {
                        while let Some(message) = rx.recv().await {
                            let trimmed = message.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            let to_send = crate::follow_up_utils::wrap_trivial_follow_up(trimmed);
                            let line = format!("{}\n", to_send);
                            if let Err(e) = stdin.write_all(line.as_bytes()).await {
                                let _ = StatusManager::log(
                                    &pool,
                                    &tx,
                                    attempt_id,
                                    "stderr",
                                    &format!("Failed to forward live input to stdin: {}", e),
                                )
                                .await;
                                break;
                            }
                            if let Err(e) = stdin.flush().await {
                                let _ = StatusManager::log(
                                    &pool,
                                    &tx,
                                    attempt_id,
                                    "stderr",
                                    &format!("Failed to flush live input to stdin: {}", e),
                                )
                                .await;
                                break;
                            }
                        }
                    });
                }
            }
        }

        // Create streaming task with cancel check.
        // Prefer cancel branch so we detect user cancel before stream EOF (when process is killed).
        let stream_result = tokio::select! {
            biased;
            // Check for cancellation
            _ = async {
                loop {
                    cancel_rx.changed().await.ok();
                    if *cancel_rx.borrow() {
                        break;
                    }
                }
            } => {
                self.log(attempt_id, "system", "Agent cancelled by user").await?;
                Err(anyhow::anyhow!("Agent cancelled"))
            }

            // Stream logs
            result = async {
                match provider {
                    AgentCliProvider::OpenAiCodex => {
                        self.stream_codex_json_with_interrupt(
                            child_ref,
                            attempt_id,
                            worktree_path,
                            interrupt_receiver,
                        ).await
                    }
                    AgentCliProvider::GeminiCli => {
                        self.stream_gemini_json_with_interrupt(
                            child_ref,
                            attempt_id,
                            worktree_path,
                            interrupt_receiver,
                        ).await
                    }
                    AgentCliProvider::CursorCli => {
                        self.stream_cursor_json_with_interrupt(
                            child_ref,
                            attempt_id,
                            worktree_path,
                            interrupt_receiver,
                        ).await
                    }
                    // Claude SDK mode logs are persisted via ProtocolPeer + ClaudeAgentClient.
                    AgentCliProvider::ClaudeCode => {
                        if let Some(store) = msg_store.clone() {
                            self.wait_for_claude_sdk_turn_completion(store, INIT_TASK_TIMEOUT)
                                .await
                        } else {
                            Ok(())
                        }
                    }
                }
            } => {
                result
            }
        };

        // If cancelled: terminate process before returning to avoid hang (wait() would block forever)
        if stream_result.is_err() {
            let interrupt_sender = self
                .active_sessions
                .lock()
                .await
                .remove(&attempt_id)
                .and_then(|s| s.interrupt_sender);
            let _ = terminate_process(child_ref, interrupt_sender, GRACEFUL_SHUTDOWN_TIMEOUT).await;
            drop(cleanup_guard);
            return stream_result;
        }

        // Stream completed successfully, close runtime input channel so providers can
        // end single-turn sessions cleanly instead of waiting for more input forever.
        if let Some(session) = self.active_sessions.lock().await.get_mut(&attempt_id) {
            session.input_sender = None;
        }

        // Wait for process to complete, but never block attempt finalization forever.
        match tokio::time::timeout(AGENT_EXIT_TIMEOUT_AFTER_STREAM, child_ref.wait()).await {
            Ok(Ok(status)) => {
                if !status.success() {
                    self.log(
                        attempt_id,
                        "system",
                        &format!("Agent exited with status: {}", status),
                    )
                    .await?;
                }
            }
            Ok(Err(err)) => {
                let msg = format!("Failed to wait for agent process exit: {}", err);
                self.log(attempt_id, "stderr", &msg).await?;
                drop(cleanup_guard);
                return Err(anyhow::anyhow!(msg));
            }
            Err(_) => {
                self.log(
                    attempt_id,
                    "stderr",
                    &format!(
                        "Agent process did not exit after stream completion (>{:?}). Forcing shutdown to avoid hang.",
                        AGENT_EXIT_TIMEOUT_AFTER_STREAM
                    ),
                )
                .await?;

                let _ = terminate_process(child_ref, None, GRACEFUL_SHUTDOWN_TIMEOUT).await;
            }
        }

        // Explicitly drop cleanup guard (cleanup will run)
        drop(cleanup_guard);

        stream_result
    }

    /// Stream Codex `--json` stdout events into our existing log schema.
    ///
    /// Why: Codex emits rich JSONL events on stdout, and otherwise prints verbose UI-ish text to
    /// stderr. By using `--json` and parsing here, we keep timeline logs clean and we can
    /// generate normalized entries (tool calls + file changes) that persist in `agent_logs`
    /// and render consistently after page reloads (Vibe Kanban-style).
    async fn stream_codex_json_with_interrupt(
        &self,
        child: &mut AsyncGroupChild,
        attempt_id: Uuid,
        worktree_path: &Path,
        mut interrupt_rx: Option<crate::process::InterruptReceiver>,
    ) -> Result<()> {
        const MAX_CMD_CHARS: usize = 400;
        const MAX_OUTPUT_CHARS: usize = 100_000; // Codex/Gemini command output (was 20K, too aggressive)

        let stdout = child.inner().stdout.take().context("No stdout captured")?;
        let stderr = child.inner().stderr.take().context("No stderr captured")?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        // Best-effort truncation that preserves UTF-8 boundaries.
        let truncate = |mut s: String, max: usize| -> String {
            if s.len() <= max {
                return s;
            }
            let mut cut = max;
            while cut > 0 && !s.is_char_boundary(cut) {
                cut -= 1;
            }
            s.truncate(cut);
            s.push_str("\n... (truncated)");
            s
        };

        let rel_path = |p: &str| -> String {
            let abs = std::path::Path::new(p);
            let rel = abs.strip_prefix(worktree_path).unwrap_or(abs);
            rel.to_string_lossy().to_string()
        };

        let mut stdout_done = false;
        let mut stderr_done = false;
        let mut assistant_snapshots: HashMap<String, String> = HashMap::new();
        let mut agent_buffer = AgentTextBuffer::new();

        loop {
            tokio::select! {
                // Interrupt support (pause/cancel) for long-running streams.
                _ = async {
                    if let Some(ref mut rx) = interrupt_rx {
                        rx.await.ok()
                    } else {
                        std::future::pending::<Option<()>>().await
                    }
                } => {
                    self.log(attempt_id, "system", "Agent interrupted by user").await?;
                    break;
                }

                line = stdout_reader.next_line(), if !stdout_done => {
                    match line {
                        Ok(Some(raw_line)) => {
                            let events = crate::codex::parse_codex_json_events(&raw_line);
                            if events.is_empty() {
                                // Some Codex JSON event variants are not part of the typed mapper,
                                // but can still contain the final assistant output (including REPO_URL).
                                if let Some(hint) = crate::codex::extract_repo_url_hint_from_json_line(&raw_line) {
                                    self.log(attempt_id, "stdout", &hint).await?;
                                }
                                continue;
                            }

                            for ev in events {
                                match ev {
                                    crate::codex::CodexStreamEvent::AgentMessage { item_id, text, is_final } => {
                                        // Codex `item.updated` for agent_message is often snapshot-style.
                                        // Convert snapshot -> delta so timeline can update progressively
                                        // instead of appending full sentence-sized blocks.
                                        let key = item_id.unwrap_or_else(|| "__default__".to_string());
                                        let previous = assistant_snapshots.get(&key).cloned();

                                        let fragment_to_emit = match previous {
                                            Some(prev) if text.starts_with(&prev) => {
                                                let delta = text[prev.len()..].to_string();
                                                if delta.is_empty() { None } else { Some(delta) }
                                            }
                                            Some(prev) if prev.starts_with(&text) => {
                                                // Out-of-order or duplicated partial update.
                                                None
                                            }
                                            Some(_) => {
                                                // Snapshot diverged from previous content. Start a fresh assistant card.
                                                StatusManager::reset_assistant_accumulator(attempt_id).await;
                                                Some(text.clone())
                                            }
                                            None => Some(text.clone()),
                                        };

                                        assistant_snapshots.insert(key.clone(), text);

                                        if let Some(fragment) = fragment_to_emit {
                                            self.emit_runtime_capable_assistant_chunk(
                                                attempt_id,
                                                worktree_path,
                                                &mut agent_buffer,
                                                &fragment,
                                            )
                                            .await?;
                                        }

                                        if is_final {
                                            assistant_snapshots.remove(&key);
                                            StatusManager::reset_assistant_accumulator(attempt_id).await;
                                        }
                                    }
                                    // Log CommandStarted so user sees "Running grep..." / "Searching..." immediately
                                    // instead of generic "Generating..." while command runs.
                                    crate::codex::CodexStreamEvent::CommandStarted { command } => {
                                        StatusManager::reset_assistant_accumulator(attempt_id).await;
                                        let cmd = truncate(command, MAX_CMD_CHARS);
                                        let (tool_name, action_type) = classify_successful_shell_command(&cmd)
                                            .map(|(name, at)| {
                                                let at_no_result = match at {
                                                    crate::sdk_normalized_types::ActionType::CommandRun {
                                                        command: c,
                                                        result: _,
                                                    } => crate::sdk_normalized_types::ActionType::CommandRun {
                                                        command: c,
                                                        result: None,
                                                    },
                                                    other => other,
                                                };
                                                (name, at_no_result)
                                            })
                                            .unwrap_or_else(|| {
                                                (
                                                    "Bash".to_string(),
                                                    crate::sdk_normalized_types::ActionType::CommandRun {
                                                        command: cmd.clone(),
                                                        result: None,
                                                    },
                                                )
                                            });
                                        let entry = crate::sdk_normalized_types::NormalizedEntry {
                                            timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                            entry_type: crate::sdk_normalized_types::NormalizedEntryType::ToolUse {
                                                tool_name,
                                                action_type,
                                                status: crate::sdk_normalized_types::ToolStatus::Running,
                                            },
                                            content: String::new(),
                                        };
                                        if let Ok(json) = serde_json::to_string(&entry) {
                                            self.log(attempt_id, "normalized", &json).await?;
                                        }
                                    }
                                    crate::codex::CodexStreamEvent::CommandCompleted { command, exit_code, output } => {
                                        StatusManager::reset_assistant_accumulator(attempt_id).await;
                                        use crate::sdk_normalized_types::{
                                            ActionType, CommandExitStatus, CommandRunResult, NormalizedEntry,
                                            NormalizedEntryType, ToolStatus,
                                        };

                                        let cmd = truncate(command, MAX_CMD_CHARS);
                                        let ok = exit_code == Some(0);
                                        let status = if ok { ToolStatus::Success } else { ToolStatus::Failed };

                                        // Keep the timeline non-verbose: only attach command output on failures.
                                        let output = if !ok { output } else { None };
                                        let output = output.map(|out| truncate(out, MAX_OUTPUT_CHARS));

                                        let result = CommandRunResult {
                                            exit_status: exit_code.map(|code| CommandExitStatus::ExitCode { code }),
                                            output,
                                        };
                                        let (tool_name, action_type) = if ok {
                                            classify_successful_shell_command(&cmd).unwrap_or_else(|| {
                                                (
                                                    "Bash".to_string(),
                                                    ActionType::CommandRun {
                                                        command: cmd.clone(),
                                                        result: Some(result.clone()),
                                                    },
                                                )
                                            })
                                        } else {
                                            (
                                                "Bash".to_string(),
                                                ActionType::CommandRun {
                                                    command: cmd.clone(),
                                                    result: Some(result),
                                                },
                                            )
                                        };

                                        let entry = NormalizedEntry {
                                            timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                            entry_type: NormalizedEntryType::ToolUse {
                                                tool_name,
                                                action_type,
                                                status,
                                            },
                                            content: String::new(),
                                        };

                                        if let Ok(json) = serde_json::to_string(&entry) {
                                            self.log(attempt_id, "normalized", &json).await?;
                                        }
                                    }
                                    crate::codex::CodexStreamEvent::FileChanged { path, kind } => {
                                        StatusManager::reset_assistant_accumulator(attempt_id).await;
                                        let path = rel_path(&path);
                                        use crate::normalization::{FileChange, FileChangeType, NormalizedEntry};

                                        let kind_lc = kind.to_ascii_lowercase();
                                        let mut change_type = match kind_lc.as_str() {
                                            "create" => FileChangeType::Created,
                                            "delete" | "remove" => FileChangeType::Deleted,
                                            // Codex doesn't give us the "from" path; treat rename as modified.
                                            "rename" => FileChangeType::Modified,
                                            _ => FileChangeType::Modified,
                                        };

                                        // Codex often reports new files as "update".
                                        // If the path is not tracked yet, surface it as Created.
                                        if kind_lc == "update"
                                            && matches!(change_type, FileChangeType::Modified)
                                            && !Self::is_git_tracked_path(worktree_path, &path).await
                                        {
                                            change_type = FileChangeType::Created;
                                        }

                                        let entry = NormalizedEntry::FileChange(FileChange {
                                            path,
                                            change_type,
                                            lines_added: None,
                                            lines_removed: None,
                                            timestamp: chrono::Utc::now(),
                                            line_number: 0,
                                        });

                                        if let Ok(json) = serde_json::to_string(&entry) {
                                            self.log(attempt_id, "normalized", &json).await?;
                                        }
                                    }
                                    crate::codex::CodexStreamEvent::TokenUsage {
                                        input_tokens,
                                        output_tokens,
                                        total_tokens,
                                        model_context_window,
                                    } => {
                                        use crate::sdk_normalized_types::{
                                            NormalizedEntry, NormalizedEntryType,
                                        };

                                        let entry = NormalizedEntry {
                                            timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                            entry_type: NormalizedEntryType::TokenUsageInfo {
                                                input_tokens,
                                                output_tokens,
                                                total_tokens,
                                                model_context_window,
                                            },
                                            content: String::new(),
                                        };

                                        if let Ok(json) = serde_json::to_string(&entry) {
                                            self.log(attempt_id, "normalized", &json).await?;
                                        }
                                    }
                                    crate::codex::CodexStreamEvent::NextAction { text } => {
                                        use crate::sdk_normalized_types::{
                                            NormalizedEntry, NormalizedEntryType,
                                        };

                                        let entry = NormalizedEntry {
                                            timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                            entry_type: NormalizedEntryType::NextAction { text },
                                            content: String::new(),
                                        };

                                        if let Ok(json) = serde_json::to_string(&entry) {
                                            self.log(attempt_id, "normalized", &json).await?;
                                        }
                                    }
                                    crate::codex::CodexStreamEvent::UserAnsweredQuestions {
                                        question,
                                        answer,
                                    } => {
                                        use crate::sdk_normalized_types::{
                                            NormalizedEntry, NormalizedEntryType,
                                        };

                                        let entry = NormalizedEntry {
                                            timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                            entry_type: NormalizedEntryType::UserAnsweredQuestions {
                                                question,
                                                answer,
                                            },
                                            content: String::new(),
                                        };

                                        if let Ok(json) = serde_json::to_string(&entry) {
                                            self.log(attempt_id, "normalized", &json).await?;
                                        }
                                    }
                                }
                            }
                        }
                        Ok(None) => stdout_done = true, // EOF
                        Err(e) => return Err(anyhow::anyhow!("Error reading Codex stdout: {}", e)),
                    }
                }

                // If Codex prints anything to stderr in --json mode, surface it as stderr.
                line = stderr_reader.next_line(), if !stderr_done => {
                    match line {
                        Ok(Some(raw_line)) => {
                            if let Some(normalized) = normalize_stderr_for_display(&raw_line) {
                                self.log(attempt_id, "stderr", &normalized).await?;
                            }
                        }
                        Ok(None) => stderr_done = true, // EOF
                        Err(e) => return Err(anyhow::anyhow!("Error reading Codex stderr: {}", e)),
                    }
                }

                else => break,
            }

            if stdout_done && stderr_done {
                break;
            }
        }

        self.flush_runtime_capable_assistant_buffer(attempt_id, worktree_path, &mut agent_buffer)
            .await?;

        Ok(())
    }

    /// Stream Gemini `--output-format stream-json` stdout events into normalized timeline entries.
    async fn stream_gemini_json_with_interrupt(
        &self,
        child: &mut AsyncGroupChild,
        attempt_id: Uuid,
        worktree_path: &Path,
        mut interrupt_rx: Option<crate::process::InterruptReceiver>,
    ) -> Result<()> {
        const MAX_CMD_CHARS: usize = 400;
        const MAX_OUTPUT_CHARS: usize = 100_000; // Codex/Gemini command output (was 20K, too aggressive)

        let stdout = child.inner().stdout.take().context("No stdout captured")?;
        let stderr = child.inner().stderr.take().context("No stderr captured")?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let truncate = |mut s: String, max: usize| -> String {
            if s.len() <= max {
                return s;
            }
            let mut cut = max;
            while cut > 0 && !s.is_char_boundary(cut) {
                cut -= 1;
            }
            s.truncate(cut);
            s.push_str("\n... (truncated)");
            s
        };

        let rel_path = |p: &str| -> String {
            let abs = std::path::Path::new(p);
            let rel = abs.strip_prefix(worktree_path).unwrap_or(abs);
            rel.to_string_lossy().to_string()
        };

        let mut stdout_done = false;
        let mut stderr_done = false;
        let mut agent_buffer = AgentTextBuffer::new();

        loop {
            tokio::select! {
                _ = async {
                    if let Some(ref mut rx) = interrupt_rx {
                        rx.await.ok()
                    } else {
                        std::future::pending::<Option<()>>().await
                    }
                } => {
                    self.log(attempt_id, "system", "Agent interrupted by user").await?;
                    break;
                }
                line = stdout_reader.next_line(), if !stdout_done => {
                    match line {
                        Ok(Some(raw_line)) => {
                            let events = crate::gemini::parse_gemini_json_events(&raw_line);
                            if events.is_empty() {
                                let msg = sanitize_log(&raw_line);
                                if !should_skip_log_line(&msg) {
                                    self.log(attempt_id, "stdout", &msg).await?;
                                    if let Some(hint) =
                                        detect_provider_auth_blocker(AgentCliProvider::GeminiCli, &msg)
                                    {
                                        self.log(attempt_id, "system", hint).await?;
                                        return Err(anyhow::anyhow!(hint));
                                    }
                                }
                                continue;
                            }

                            for event in events {
                                match event {
                                    crate::gemini::GeminiStreamEvent::AgentMessage { text, is_final } => {
                                        if text.trim().is_empty() {
                                            continue;
                                        }
                                        self.emit_runtime_capable_assistant_chunk(
                                            attempt_id,
                                            worktree_path,
                                            &mut agent_buffer,
                                            &text,
                                        )
                                        .await?;

                                        if is_final {
                                            StatusManager::reset_assistant_accumulator(attempt_id).await;
                                        }
                                    }
                                    crate::gemini::GeminiStreamEvent::CommandCompleted { command, exit_code, output } => {
                                        StatusManager::reset_assistant_accumulator(attempt_id).await;
                                        use crate::sdk_normalized_types::{
                                            ActionType, CommandExitStatus, CommandRunResult, NormalizedEntry,
                                            NormalizedEntryType, ToolStatus,
                                        };

                                        let cmd = truncate(command, MAX_CMD_CHARS);
                                        let ok = exit_code == Some(0);
                                        let status = if ok { ToolStatus::Success } else { ToolStatus::Failed };
                                        let output = if !ok { output } else { None };
                                        let output = output.map(|out| truncate(out, MAX_OUTPUT_CHARS));
                                        let result = CommandRunResult {
                                            exit_status: exit_code.map(|code| CommandExitStatus::ExitCode { code }),
                                            output,
                                        };
                                        let (tool_name, action_type) = if ok {
                                            classify_successful_shell_command(&cmd).unwrap_or_else(|| {
                                                (
                                                    "Bash".to_string(),
                                                    ActionType::CommandRun {
                                                        command: cmd.clone(),
                                                        result: Some(result.clone()),
                                                    },
                                                )
                                            })
                                        } else {
                                            (
                                                "Bash".to_string(),
                                                ActionType::CommandRun {
                                                    command: cmd.clone(),
                                                    result: Some(result),
                                                },
                                            )
                                        };

                                        let entry = NormalizedEntry {
                                            timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                            entry_type: NormalizedEntryType::ToolUse {
                                                tool_name,
                                                action_type,
                                                status,
                                            },
                                            content: String::new(),
                                        };

                                        if let Ok(json) = serde_json::to_string(&entry) {
                                            self.log(attempt_id, "normalized", &json).await?;
                                        }
                                    }
                                    crate::gemini::GeminiStreamEvent::FileChanged { path, kind } => {
                                        StatusManager::reset_assistant_accumulator(attempt_id).await;
                                        let path = rel_path(&path);
                                        use crate::normalization::{FileChange, FileChangeType, NormalizedEntry};

                                        let kind_lc = kind.to_ascii_lowercase();
                                        let mut change_type = match kind_lc.as_str() {
                                            "create" => FileChangeType::Created,
                                            "delete" | "remove" => FileChangeType::Deleted,
                                            _ => FileChangeType::Modified,
                                        };

                                        if kind_lc == "update"
                                            && matches!(change_type, FileChangeType::Modified)
                                            && !Self::is_git_tracked_path(worktree_path, &path).await
                                        {
                                            change_type = FileChangeType::Created;
                                        }

                                        let entry = NormalizedEntry::FileChange(FileChange {
                                            path,
                                            change_type,
                                            lines_added: None,
                                            lines_removed: None,
                                            timestamp: chrono::Utc::now(),
                                            line_number: 0,
                                        });

                                        if let Ok(json) = serde_json::to_string(&entry) {
                                            self.log(attempt_id, "normalized", &json).await?;
                                        }
                                    }
                                    crate::gemini::GeminiStreamEvent::TokenUsage {
                                        input_tokens,
                                        output_tokens,
                                        total_tokens,
                                        model_context_window,
                                    } => {
                                        use crate::sdk_normalized_types::{
                                            NormalizedEntry, NormalizedEntryType,
                                        };

                                        let entry = NormalizedEntry {
                                            timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                            entry_type: NormalizedEntryType::TokenUsageInfo {
                                                input_tokens,
                                                output_tokens,
                                                total_tokens,
                                                model_context_window,
                                            },
                                            content: String::new(),
                                        };

                                        if let Ok(json) = serde_json::to_string(&entry) {
                                            self.log(attempt_id, "normalized", &json).await?;
                                        }
                                    }
                                    crate::gemini::GeminiStreamEvent::NextAction { text } => {
                                        use crate::sdk_normalized_types::{
                                            NormalizedEntry, NormalizedEntryType,
                                        };

                                        let entry = NormalizedEntry {
                                            timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                            entry_type: NormalizedEntryType::NextAction { text },
                                            content: String::new(),
                                        };

                                        if let Ok(json) = serde_json::to_string(&entry) {
                                            self.log(attempt_id, "normalized", &json).await?;
                                        }
                                    }
                                    crate::gemini::GeminiStreamEvent::UserAnsweredQuestions {
                                        question,
                                        answer,
                                    } => {
                                        use crate::sdk_normalized_types::{
                                            NormalizedEntry, NormalizedEntryType,
                                        };

                                        let entry = NormalizedEntry {
                                            timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                            entry_type: NormalizedEntryType::UserAnsweredQuestions {
                                                question,
                                                answer,
                                            },
                                            content: String::new(),
                                        };

                                        if let Ok(json) = serde_json::to_string(&entry) {
                                            self.log(attempt_id, "normalized", &json).await?;
                                        }
                                    }
                                    crate::gemini::GeminiStreamEvent::ToolUseStarted {
                                        tool_id,
                                        tool_name,
                                        payload,
                                    } => {
                                        StatusManager::reset_assistant_accumulator(attempt_id).await;
                                        if let Err(e) = StatusManager::create_gemini_tool_start(
                                            &self.db_pool,
                                            &self.broadcast_tx,
                                            attempt_id,
                                            &tool_id,
                                            &tool_name,
                                            &payload,
                                        )
                                        .await
                                        {
                                            warn!(
                                                "Failed to create Gemini tool start for {}: {}",
                                                tool_id, e
                                            );
                                        }
                                    }
                                    crate::gemini::GeminiStreamEvent::ToolResult {
                                        tool_id,
                                        success,
                                        ..
                                    } => {
                                        if let Err(e) = StatusManager::complete_gemini_tool(
                                            &self.db_pool,
                                            &self.broadcast_tx,
                                            attempt_id,
                                            &tool_id,
                                            success,
                                        )
                                        .await
                                        {
                                            warn!(
                                                "Failed to complete Gemini tool {}: {}",
                                                tool_id, e
                                            );
                                        }
                                    }
                                    crate::gemini::GeminiStreamEvent::Skip => {}
                                }
                            }
                        }
                        Ok(None) => stdout_done = true,
                        Err(e) => return Err(anyhow::anyhow!("Error reading Gemini stdout: {}", e)),
                    }
                }
                line = stderr_reader.next_line(), if !stderr_done => {
                    match line {
                        Ok(Some(raw_line)) => {
                            if let Some(normalized) = normalize_stderr_for_display(&raw_line) {
                                self.log(attempt_id, "stderr", &normalized).await?;
                                if let Some(hint) = detect_provider_auth_blocker(
                                    AgentCliProvider::GeminiCli,
                                    &normalized,
                                ) {
                                    self.log(attempt_id, "system", hint).await?;
                                    return Err(anyhow::anyhow!(hint));
                                }
                            }
                        }
                        Ok(None) => stderr_done = true,
                        Err(e) => return Err(anyhow::anyhow!("Error reading Gemini stderr: {}", e)),
                    }
                }
                else => break,
            }

            if stdout_done && stderr_done {
                break;
            }
        }

        self.flush_runtime_capable_assistant_buffer(attempt_id, worktree_path, &mut agent_buffer)
            .await?;

        Ok(())
    }

    /// Stream Cursor CLI `--output-format stream-json` stdout events into normalized timeline entries.
    async fn stream_cursor_json_with_interrupt(
        &self,
        child: &mut AsyncGroupChild,
        attempt_id: Uuid,
        worktree_path: &Path,
        mut interrupt_rx: Option<crate::process::InterruptReceiver>,
    ) -> Result<()> {
        let stdout = child.inner().stdout.take().context("No stdout captured")?;
        let stderr = child.inner().stderr.take().context("No stderr captured")?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let rel_path = |p: &str| -> String {
            let abs = std::path::Path::new(p);
            let rel = abs.strip_prefix(worktree_path).unwrap_or(abs);
            rel.to_string_lossy().to_string()
        };

        let mut stdout_done = false;
        let mut stderr_done = false;
        let mut agent_buffer = AgentTextBuffer::new();

        loop {
            tokio::select! {
                _ = async {
                    if let Some(ref mut rx) = interrupt_rx {
                        rx.await.ok()
                    } else {
                        std::future::pending::<Option<()>>().await
                    }
                } => {
                    self.log(attempt_id, "system", "Agent interrupted by user").await?;
                    break;
                }
                line = stdout_reader.next_line(), if !stdout_done => {
                    match line {
                        Ok(Some(raw_line)) => {
                            let events = crate::cursor::parse_cursor_json_events(&raw_line);
                            if events.is_empty() {
                                let msg = sanitize_log(&raw_line);
                                if !should_skip_log_line(&msg) {
                                    self.log(attempt_id, "stdout", &msg).await?;
                                    if let Some(hint) =
                                        detect_provider_auth_blocker(AgentCliProvider::CursorCli, &msg)
                                    {
                                        self.log(attempt_id, "system", hint).await?;
                                        return Err(anyhow::anyhow!(hint));
                                    }
                                }
                                continue;
                            }

                            for event in events {
                                match event {
                                    crate::cursor::CursorStreamEvent::AgentMessage { text, is_final } => {
                                        if text.trim().is_empty() {
                                            continue;
                                        }
                                        self.emit_runtime_capable_assistant_chunk(
                                            attempt_id,
                                            worktree_path,
                                            &mut agent_buffer,
                                            &text,
                                        )
                                        .await?;

                                        if is_final {
                                            StatusManager::reset_assistant_accumulator(attempt_id).await;
                                        }
                                    }
                                    crate::cursor::CursorStreamEvent::ToolCallCompleted {
                                        path,
                                        lines_added,
                                        ..
                                    } => {
                                        StatusManager::reset_assistant_accumulator(attempt_id).await;
                                        let path = rel_path(&path);
                                        use crate::normalization::{FileChange, FileChangeType, NormalizedEntry};

                                        let change_type = if !Self::is_git_tracked_path(worktree_path, &path).await {
                                            FileChangeType::Created
                                        } else {
                                            FileChangeType::Modified
                                        };

                                        let lines_added = lines_added.map(|n| n as usize);

                                        let entry = NormalizedEntry::FileChange(FileChange {
                                            path,
                                            change_type,
                                            lines_added,
                                            lines_removed: None,
                                            timestamp: chrono::Utc::now(),
                                            line_number: 0,
                                        });

                                        if let Ok(json) = serde_json::to_string(&entry) {
                                            self.log(attempt_id, "normalized", &json).await?;
                                        }
                                    }
                                    crate::cursor::CursorStreamEvent::Result {
                                        result,
                                        usage,
                                        ..
                                    } => {
                                        StatusManager::reset_assistant_accumulator(attempt_id).await;
                                        if !result.trim().is_empty() {
                                            self.emit_runtime_capable_assistant_chunk(
                                                attempt_id,
                                                worktree_path,
                                                &mut agent_buffer,
                                                &result,
                                            )
                                            .await?;
                                        }
                                        if let Some(u) = usage {
                                            use crate::sdk_normalized_types::{
                                                NormalizedEntry, NormalizedEntryType,
                                            };
                                            let total = u.input_tokens + u.output_tokens;
                                            let entry = NormalizedEntry {
                                                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                                entry_type: NormalizedEntryType::TokenUsageInfo {
                                                    input_tokens: u.input_tokens,
                                                    output_tokens: u.output_tokens,
                                                    total_tokens: Some(total),
                                                    model_context_window: None,
                                                },
                                                content: String::new(),
                                            };
                                            if let Ok(json) = serde_json::to_string(&entry) {
                                                self.log(attempt_id, "normalized", &json).await?;
                                            }
                                        }
                                    }
                                    crate::cursor::CursorStreamEvent::ToolCallStartedGeneric {
                                        call_id,
                                        tool_name,
                                        payload,
                                    } => {
                                        StatusManager::reset_assistant_accumulator(attempt_id).await;
                                        if let Err(e) = StatusManager::create_cursor_tool_start(
                                            &self.db_pool,
                                            &self.broadcast_tx,
                                            attempt_id,
                                            &call_id,
                                            &tool_name,
                                            &payload,
                                        )
                                        .await
                                        {
                                            warn!(
                                                "Failed to create Cursor tool start for {}: {}",
                                                call_id, e
                                            );
                                        }
                                    }
                                    crate::cursor::CursorStreamEvent::ToolCallCompletedGeneric {
                                        call_id,
                                        success,
                                    } => {
                                        if let Err(e) = StatusManager::complete_cursor_tool(
                                            &self.db_pool,
                                            &self.broadcast_tx,
                                            attempt_id,
                                            &call_id,
                                            success,
                                        )
                                        .await
                                        {
                                            warn!(
                                                "Failed to complete Cursor tool {}: {}",
                                                call_id, e
                                            );
                                        }
                                    }
                                    crate::cursor::CursorStreamEvent::SystemInit { .. }
                                    | crate::cursor::CursorStreamEvent::ThinkingDelta { .. }
                                    | crate::cursor::CursorStreamEvent::ThinkingCompleted
                                    | crate::cursor::CursorStreamEvent::ToolCallStarted { .. }
                                    | crate::cursor::CursorStreamEvent::Skip => {}
                                }
                            }
                        }
                        Ok(None) => stdout_done = true,
                        Err(e) => return Err(anyhow::anyhow!("Error reading Cursor stdout: {}", e)),
                    }
                }
                line = stderr_reader.next_line(), if !stderr_done => {
                    match line {
                        Ok(Some(raw_line)) => {
                            if let Some(normalized) = normalize_stderr_for_display(&raw_line) {
                                self.log(attempt_id, "stderr", &normalized).await?;
                                if let Some(hint) = detect_provider_auth_blocker(
                                    AgentCliProvider::CursorCli,
                                    &normalized,
                                ) {
                                    self.log(attempt_id, "system", hint).await?;
                                    return Err(anyhow::anyhow!(hint));
                                }
                            }
                        }
                        Ok(None) => stderr_done = true,
                        Err(e) => return Err(anyhow::anyhow!("Error reading Cursor stderr: {}", e)),
                    }
                }
                else => break,
            }

            if stdout_done && stderr_done {
                break;
            }
        }

        self.flush_runtime_capable_assistant_buffer(attempt_id, worktree_path, &mut agent_buffer)
            .await?;

        Ok(())
    }

    /// Store worktree path in attempt metadata
    async fn store_worktree_path(&self, attempt_id: Uuid, worktree_path: &Path) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE task_attempts
            SET metadata = metadata || jsonb_build_object('worktree_path', $2)
            WHERE id = $1
            "#,
        )
        .bind(attempt_id)
        .bind(worktree_path.to_string_lossy().to_string())
        .execute(&self.db_pool)
        .await
        .context("Failed to store worktree path")?;
        Ok(())
    }

    /// Detect current HEAD commit hash for a repo path.
    async fn detect_current_head_commit(repo_path: &Path) -> Option<String> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["rev-parse", "--verify", "HEAD"])
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if hash.is_empty() {
            None
        } else {
            Some(hash)
        }
    }

    /// Best-effort check whether a path is currently tracked by git.
    async fn is_git_tracked_path(repo_path: &Path, rel_path: &str) -> bool {
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["ls-files", "--error-unmatch", "--", rel_path])
            .output()
            .await;

        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }

    /// Persist a fixed diff base reference in attempt metadata.
    async fn store_diff_base_commit_value(&self, attempt_id: Uuid, base_ref: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE task_attempts
            SET metadata = COALESCE(metadata, '{}'::jsonb) || jsonb_build_object('diff_base_commit', $2::text)
            WHERE id = $1
            "#,
        )
        .bind(attempt_id)
        .bind(base_ref)
        .execute(&self.db_pool)
        .await
        .context("Failed to store diff base commit")?;

        Ok(())
    }

    /// Capture and persist a diff base commit for this attempt.
    ///
    /// If the repository has no commit yet (e.g. from-scratch init), fall back to
    /// the Git empty-tree hash so first-commit scaffolding still yields a full diff.
    async fn store_diff_base_commit_from_repo(
        &self,
        attempt_id: Uuid,
        repo_path: &Path,
    ) -> Result<()> {
        let base_ref = Self::detect_current_head_commit(repo_path)
            .await
            .unwrap_or_else(|| GIT_EMPTY_TREE_HASH.to_string());

        self.store_diff_base_commit_value(attempt_id, &base_ref)
            .await
    }

    /// Persist a project slug in `projects.metadata.slug`.
    ///
    /// We use a stable, URL-safe slug for:
    /// - local worktree directory naming
    /// - GitLab project `path` when creating repos from scratch
    async fn store_project_slug(&self, project_id: Uuid, slug: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE projects
            SET metadata = COALESCE(metadata, '{}'::jsonb) || jsonb_build_object('slug', $2::text),
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(project_id)
        .bind(slug)
        .execute(&self.db_pool)
        .await
        .context("Failed to store project slug")?;

        Ok(())
    }

    /// Capture and save file diffs when agent completes successfully.
    /// **DEPRECATED**: Use `save_diffs_to_database()` instead, which saves full file content.
    ///
    /// ## Behavior
    /// - Runs git diff to get file changes between base and feature branches
    /// - Parses numstat output for additions/deletions (stats only, no content)
    /// - Saves to file_diffs table
    /// - Updates attempt summary statistics
    /// - Gracefully handles git errors (no-op if diff fails)
    #[allow(dead_code)]
    async fn capture_and_save_diff(
        &self,
        attempt_id: Uuid,
        task_id: Uuid,
        worktree_path: &Path,
    ) -> Result<()> {
        info!(
            "📸 [DIFF CAPTURE START] attempt={}, task={}, path={:?}",
            attempt_id, task_id, worktree_path
        );

        // Check if worktree exists
        if !worktree_path.exists() {
            warn!(
                "📸 [DIFF CAPTURE SKIP] Worktree doesn't exist: {:?}",
                worktree_path
            );
            return Ok(());
        }
        info!("📸 [DIFF CAPTURE] Worktree exists, proceeding...");

        // Get base branch from attempt metadata or default to "main"
        let base_branch = sqlx::query_scalar::<_, Option<serde_json::Value>>(
            "SELECT metadata->'base_branch' FROM task_attempts WHERE id = $1",
        )
        .bind(attempt_id)
        .fetch_optional(&self.db_pool)
        .await?
        .flatten()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "main".to_string());

        let _feature_branch = format!("feature/task-{}", attempt_id);

        // Run git diff --numstat to get file statistics
        info!(
            "📸 [DIFF CAPTURE] Running git diff --numstat {} HEAD",
            base_branch
        );
        let diff_output = tokio::process::Command::new("git")
            .args(["diff", "--numstat", &base_branch, "HEAD"])
            .current_dir(worktree_path)
            .output()
            .await
            .context("Failed to run git diff command")?;

        if !diff_output.status.success() {
            warn!(
                "📸 [DIFF CAPTURE FAIL] Git diff failed for attempt {}: {}",
                attempt_id,
                String::from_utf8_lossy(&diff_output.stderr)
            );
            return Ok(());
        }

        let diff_text = String::from_utf8_lossy(&diff_output.stdout);
        info!(
            "📸 [DIFF CAPTURE] Git output ({} bytes): {:?}",
            diff_text.len(),
            diff_text.chars().take(200).collect::<String>()
        );

        if diff_text.trim().is_empty() {
            info!(
                "📸 [DIFF CAPTURE EMPTY] No file changes detected for attempt {}",
                attempt_id
            );
            return Ok(());
        }

        // Parse numstat output (format: additions deletions filename)
        let mut total_files = 0;
        let mut total_additions = 0i32;
        let mut total_deletions = 0i32;
        let line_count = diff_text.lines().count();
        info!(
            "📸 [DIFF CAPTURE] Parsing {} lines of diff output",
            line_count
        );

        for line in diff_text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }

            let additions = parts[0].parse::<i32>().unwrap_or(0);
            let deletions = parts[1].parse::<i32>().unwrap_or(0);
            let file_path = parts[2].to_string();

            // Determine change type (simple heuristic)
            let change_type = if additions > 0 && deletions == 0 {
                "added"
            } else if additions == 0 && deletions > 0 {
                "deleted"
            } else {
                "modified"
            };

            // Save to file_diffs table
            sqlx::query(
                r#"
                INSERT INTO file_diffs (attempt_id, file_path, change_type, additions, deletions)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(attempt_id)
            .bind(file_path)
            .bind(change_type)
            .bind(additions)
            .bind(deletions)
            .execute(&self.db_pool)
            .await
            .context("Failed to insert file diff")?;

            total_files += 1;
            total_additions += additions;
            total_deletions += deletions;
        }

        // Update attempt with diff summary
        sqlx::query(
            r#"
            UPDATE task_attempts
            SET diff_total_files = $2,
                diff_total_additions = $3,
                diff_total_deletions = $4,
                diff_saved_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(attempt_id)
        .bind(total_files)
        .bind(total_additions)
        .bind(total_deletions)
        .execute(&self.db_pool)
        .await
        .context("Failed to update attempt diff summary")?;

        info!(
            "📸 [DIFF CAPTURE SUCCESS] attempt={}: {} files, +{} -{} lines",
            attempt_id, total_files, total_additions, total_deletions
        );

        self.log(
            attempt_id,
            "system",
            &format!(
                "Diff captured: {} files changed (+{} -{} lines)",
                total_files, total_additions, total_deletions
            ),
        )
        .await?;

        Ok(())
    }

    /// Check if content appears to be binary (contains null bytes in first 8KB).
    fn is_binary_content(bytes: &[u8]) -> bool {
        let check_len = bytes.len().min(8192);
        bytes[..check_len].contains(&0)
    }

    /// Sanitize content for DB storage: skip binary, cap at MAX_DIFF_CONTENT_SIZE.
    fn sanitize_diff_content(content: String) -> Option<String> {
        if Self::is_binary_content(content.as_bytes()) {
            return None;
        }
        if content.len() > MAX_DIFF_CONTENT_SIZE {
            // Truncate to nearest char boundary
            let mut end = MAX_DIFF_CONTENT_SIZE;
            while !content.is_char_boundary(end) && end > 0 {
                end -= 1;
            }
            Some(content[..end].to_string())
        } else {
            Some(content)
        }
    }

    /// Save file diffs to S3 (MinIO) for persistent storage.
    ///
    /// Collects full file content via `collect_diffs_for_s3`, uploads the
    /// JSON snapshot to S3, and updates attempt metadata with the S3 key.
    ///
    /// ## Safety Guards
    /// - Binary files are detected and skipped (content stored as None)
    /// - Files larger than 1MB have their content truncated
    /// - Errors are logged but do not propagate to fail the attempt
    async fn save_diffs_to_s3(&self, attempt_id: Uuid, worktree_path: &Path) -> Result<()> {
        // Get task_id from attempt
        let task_id: Uuid = sqlx::query_scalar("SELECT task_id FROM task_attempts WHERE id = $1")
            .bind(attempt_id)
            .fetch_one(&self.db_pool)
            .await?;

        // Collect diffs into snapshot
        let mut snapshot = self
            .collect_diffs_for_s3(attempt_id, task_id, worktree_path)
            .await?;

        // Apply binary/size guards to collected files
        for file in &mut snapshot.files {
            file.old_content = file
                .old_content
                .take()
                .and_then(Self::sanitize_diff_content);
            file.new_content = file
                .new_content
                .take()
                .and_then(Self::sanitize_diff_content);
        }

        // Recalculate totals after sanitization
        snapshot.total_additions = snapshot.files.iter().map(|f| f.additions).sum();
        snapshot.total_deletions = snapshot.files.iter().map(|f| f.deletions).sum();

        if snapshot.files.is_empty() {
            info!(
                "📸 [DIFF S3] No file diffs to save for attempt {}",
                attempt_id
            );
            sqlx::query(
                r#"
                UPDATE task_attempts
                SET diff_total_files = 0,
                    diff_total_additions = 0,
                    diff_total_deletions = 0,
                    diff_saved_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(attempt_id)
            .execute(&self.db_pool)
            .await?;
            return Ok(());
        }

        // Generate S3 key and upload
        let s3_key = crate::diff_snapshot::AttemptDiffSnapshot::generate_s3_key(
            attempt_id,
            snapshot.saved_at,
        );
        let total_size = snapshot.calculate_total_size();

        self.storage_service
            .upload_diff_snapshot(&s3_key, &snapshot)
            .await?;

        // Update attempt with S3 metadata
        sqlx::query(
            r#"
            UPDATE task_attempts
            SET s3_diff_key = $2,
                s3_diff_size = $3,
                s3_diff_saved_at = NOW(),
                diff_total_files = $4,
                diff_total_additions = $5,
                diff_total_deletions = $6,
                diff_saved_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(attempt_id)
        .bind(&s3_key)
        .bind(total_size)
        .bind(snapshot.total_files as i32)
        .bind(snapshot.total_additions)
        .bind(snapshot.total_deletions)
        .execute(&self.db_pool)
        .await?;

        info!(
            "📸 [DIFF S3] Saved {} file diffs for attempt {} ({})",
            snapshot.total_files, attempt_id, s3_key
        );

        Ok(())
    }

    /// Save file diffs from worktree to database for persistent storage.
    /// **DEPRECATED**: Use `save_diffs_to_s3()` instead.
    #[allow(dead_code)]
    async fn save_diffs_to_database(&self, attempt_id: Uuid, worktree_path: &Path) -> Result<()> {
        // Get base branch from attempt metadata or default to "main"
        // Use Option<serde_json::Value> to handle NULL when key doesn't exist
        let base_branch = sqlx::query_scalar::<_, Option<serde_json::Value>>(
            "SELECT metadata->'base_branch' FROM task_attempts WHERE id = $1",
        )
        .bind(attempt_id)
        .fetch_optional(&self.db_pool)
        .await?
        .flatten() // Option<Option<Value>> -> Option<Value>
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "main".to_string());

        // Collect file diffs
        struct FileDiffData {
            file_path: String,
            old_path: Option<String>,
            change_type: String,
            additions: i32,
            deletions: i32,
            old_content: Option<String>,
            new_content: Option<String>,
        }

        let mut file_diffs: Vec<FileDiffData> = Vec::new();

        // Get list of changed files (tracked)
        let name_status = tokio::process::Command::new("git")
            .current_dir(worktree_path)
            .args(["diff", "--name-status", &base_branch])
            .output()
            .await?;

        let name_status_str = String::from_utf8_lossy(&name_status.stdout);

        for line in name_status_str.lines() {
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.is_empty() {
                continue;
            }

            let status = parts[0];
            let (change_type, old_path, file_path) = match status.chars().next() {
                Some('M') => (
                    "modified",
                    parts.get(1).map(|s| s.to_string()),
                    parts.get(1).map(|s| s.to_string()).unwrap_or_default(),
                ),
                Some('A') => (
                    "added",
                    None,
                    parts.get(1).map(|s| s.to_string()).unwrap_or_default(),
                ),
                Some('D') => (
                    "deleted",
                    parts.get(1).map(|s| s.to_string()),
                    parts.get(1).map(|s| s.to_string()).unwrap_or_default(),
                ),
                Some('R') => (
                    "renamed",
                    parts.get(1).map(|s| s.to_string()),
                    parts.get(2).map(|s| s.to_string()).unwrap_or_default(),
                ),
                _ => continue,
            };

            // Get old content from base branch (skip binary, cap size)
            let old_content = if change_type != "added" {
                let old_ref = format!(
                    "{}:{}",
                    base_branch,
                    old_path.as_ref().unwrap_or(&file_path)
                );
                tokio::process::Command::new("git")
                    .current_dir(worktree_path)
                    .args(["show", &old_ref])
                    .output()
                    .await
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                    .and_then(Self::sanitize_diff_content)
            } else {
                None
            };

            // Get new content from working directory (skip binary, cap size)
            let new_content = if change_type != "deleted" {
                let full_path = worktree_path.join(&file_path);
                tokio::fs::read_to_string(&full_path)
                    .await
                    .ok()
                    .and_then(Self::sanitize_diff_content)
            } else {
                None
            };

            // Count additions/deletions
            let (additions, deletions) = Self::count_line_changes(&old_content, &new_content);

            file_diffs.push(FileDiffData {
                file_path,
                old_path,
                change_type: change_type.to_string(),
                additions,
                deletions,
                old_content,
                new_content,
            });
        }

        // Handle untracked files (new files not yet staged)
        let untracked_output = tokio::process::Command::new("git")
            .current_dir(worktree_path)
            .args(["ls-files", "--others", "--exclude-standard"])
            .output()
            .await?;

        let untracked_str = String::from_utf8_lossy(&untracked_output.stdout);

        for file in untracked_str.lines().filter(|l| !l.is_empty()) {
            let full_path = worktree_path.join(file);
            if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
                let sanitized = Self::sanitize_diff_content(content);
                let line_count = sanitized.as_ref().map_or(0, |c| c.lines().count() as i32);
                file_diffs.push(FileDiffData {
                    file_path: file.to_string(),
                    old_path: None,
                    change_type: "added".to_string(),
                    additions: line_count,
                    deletions: 0,
                    old_content: None,
                    new_content: sanitized,
                });
            }
        }

        // Save to database using direct SQL
        if !file_diffs.is_empty() {
            let diff_count = file_diffs.len();
            let mut total_additions = 0i32;
            let mut total_deletions = 0i32;

            // Delete any existing diffs for this attempt
            sqlx::query("DELETE FROM file_diffs WHERE attempt_id = $1")
                .bind(attempt_id)
                .execute(&self.db_pool)
                .await?;

            // Insert each file diff
            for diff in &file_diffs {
                sqlx::query(
                    r#"
                    INSERT INTO file_diffs (attempt_id, file_path, old_path, change_type, additions, deletions, old_content, new_content)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                    "#
                )
                .bind(attempt_id)
                .bind(&diff.file_path)
                .bind(&diff.old_path)
                .bind(&diff.change_type)
                .bind(diff.additions)
                .bind(diff.deletions)
                .bind(&diff.old_content)
                .bind(&diff.new_content)
                .execute(&self.db_pool)
                .await?;

                total_additions += diff.additions;
                total_deletions += diff.deletions;
            }

            // Update attempt with diff summary
            sqlx::query(
                r#"
                UPDATE task_attempts
                SET diff_total_files = $2,
                    diff_total_additions = $3,
                    diff_total_deletions = $4,
                    diff_saved_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(attempt_id)
            .bind(diff_count as i32)
            .bind(total_additions)
            .bind(total_deletions)
            .execute(&self.db_pool)
            .await?;

            info!(
                "📸 [DIFF DB] Saved {} file diffs for attempt {}",
                diff_count, attempt_id
            );
        }

        Ok(())
    }

    /// Count line additions and deletions between old and new content
    fn count_line_changes(
        old_content: &Option<String>,
        new_content: &Option<String>,
    ) -> (i32, i32) {
        match (old_content, new_content) {
            (None, Some(new)) => (new.lines().count() as i32, 0),
            (Some(old), None) => (0, old.lines().count() as i32),
            (Some(old), Some(new)) => {
                use std::collections::HashSet;
                let old_lines: HashSet<&str> = old.lines().collect();
                let new_lines: HashSet<&str> = new.lines().collect();
                let additions = new_lines.difference(&old_lines).count() as i32;
                let deletions = old_lines.difference(&new_lines).count() as i32;
                (additions, deletions)
            }
            (None, None) => (0, 0),
        }
    }

    /// Collect file diffs from worktree and create a JSON snapshot for S3 storage
    ///
    /// Returns the AttemptDiffSnapshot that can be uploaded to S3 by the caller.
    /// This method collects all changed files but does not upload - the caller
    /// should use StorageService::upload_json() to complete the operation.
    pub async fn collect_diffs_for_s3(
        &self,
        attempt_id: Uuid,
        task_id: Uuid,
        worktree_path: &Path,
    ) -> Result<crate::diff_snapshot::AttemptDiffSnapshot> {
        use crate::diff_snapshot::{AttemptDiffSnapshot, FileDiffData};
        use chrono::Utc;
        use std::collections::HashSet;

        // Read branch metadata + fixed diff base checkpoint.
        let attempt_metadata = sqlx::query_scalar::<_, Option<serde_json::Value>>(
            "SELECT metadata FROM task_attempts WHERE id = $1",
        )
        .bind(attempt_id)
        .fetch_optional(&self.db_pool)
        .await?
        .flatten()
        .unwrap_or_else(|| serde_json::json!({}));

        let base_branch = attempt_metadata
            .get("base_branch")
            .and_then(|v| v.as_str())
            .unwrap_or("main")
            .to_string();
        let feature_branch = attempt_metadata
            .get("feature_branch")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("feature/task-{}", attempt_id));
        let diff_base_ref = attempt_metadata
            .get("diff_base_commit")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&base_branch)
            .to_string();
        let diff_range = format!("{}..HEAD", diff_base_ref);

        // Collect file diffs from committed range and working tree changes.
        let mut file_diffs: Vec<FileDiffData> = Vec::new();

        // Get list of changed files (tracked)
        let name_status = tokio::process::Command::new("git")
            .current_dir(worktree_path)
            .args(["diff", "--name-status", "--find-renames", &diff_range])
            .output()
            .await?;

        let name_status_str = String::from_utf8_lossy(&name_status.stdout);

        for line in name_status_str.lines() {
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.is_empty() {
                continue;
            }

            let status = parts[0];
            let (change_type, old_path, file_path) = match status.chars().next() {
                Some('M') => (
                    "modified",
                    parts.get(1).map(|s| s.to_string()),
                    parts.get(1).map(|s| s.to_string()).unwrap_or_default(),
                ),
                Some('A') => (
                    "added",
                    None,
                    parts.get(1).map(|s| s.to_string()).unwrap_or_default(),
                ),
                Some('D') => (
                    "deleted",
                    parts.get(1).map(|s| s.to_string()),
                    parts.get(1).map(|s| s.to_string()).unwrap_or_default(),
                ),
                Some('R') => (
                    "renamed",
                    parts.get(1).map(|s| s.to_string()),
                    parts.get(2).map(|s| s.to_string()).unwrap_or_default(),
                ),
                _ => continue,
            };

            // Get old content from base branch
            let old_content = if change_type != "added" {
                let old_ref = format!(
                    "{}:{}",
                    diff_base_ref,
                    old_path.as_ref().unwrap_or(&file_path)
                );
                tokio::process::Command::new("git")
                    .current_dir(worktree_path)
                    .args(["show", &old_ref])
                    .output()
                    .await
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            } else {
                None
            };

            // Get new content from working directory
            let new_content = if change_type != "deleted" {
                let full_path = worktree_path.join(&file_path);
                tokio::fs::read_to_string(&full_path).await.ok()
            } else {
                None
            };

            // Count additions/deletions
            let (additions, deletions) = Self::count_line_changes(&old_content, &new_content);

            file_diffs.push(FileDiffData {
                change: change_type.to_string(),
                path: file_path,
                old_path,
                additions,
                deletions,
                old_content,
                new_content,
            });
        }

        // Track collected paths to avoid duplicate diff entries.
        let mut seen_paths: HashSet<String> = file_diffs.iter().map(|f| f.path.clone()).collect();

        // Include tracked working-tree changes (unstaged + staged).
        // This covers cases where the agent edited files but did not commit.
        for args in [
            ["diff", "--name-status", "--find-renames"].as_slice(),
            ["diff", "--cached", "--name-status", "--find-renames"].as_slice(),
        ] {
            let output = tokio::process::Command::new("git")
                .current_dir(worktree_path)
                .args(args)
                .output()
                .await?;

            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                if line.is_empty() {
                    continue;
                }

                let parts: Vec<&str> = line.split('\t').collect();
                if parts.is_empty() {
                    continue;
                }

                let status = parts[0];
                let (change_type, old_path, file_path) = match status.chars().next() {
                    Some('M') => (
                        "modified",
                        parts.get(1).map(|s| s.to_string()),
                        parts.get(1).map(|s| s.to_string()).unwrap_or_default(),
                    ),
                    Some('A') => (
                        "added",
                        None,
                        parts.get(1).map(|s| s.to_string()).unwrap_or_default(),
                    ),
                    Some('D') => (
                        "deleted",
                        parts.get(1).map(|s| s.to_string()),
                        parts.get(1).map(|s| s.to_string()).unwrap_or_default(),
                    ),
                    Some('R') => (
                        "renamed",
                        parts.get(1).map(|s| s.to_string()),
                        parts.get(2).map(|s| s.to_string()).unwrap_or_default(),
                    ),
                    _ => continue,
                };

                if file_path.is_empty() || seen_paths.contains(&file_path) {
                    continue;
                }

                let old_content = if change_type != "added" {
                    let old_ref = format!(
                        "{}:{}",
                        diff_base_ref,
                        old_path.as_ref().unwrap_or(&file_path)
                    );
                    tokio::process::Command::new("git")
                        .current_dir(worktree_path)
                        .args(["show", &old_ref])
                        .output()
                        .await
                        .ok()
                        .filter(|o| o.status.success())
                        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                } else {
                    None
                };

                let new_content = if change_type != "deleted" {
                    let full_path = worktree_path.join(&file_path);
                    tokio::fs::read_to_string(&full_path).await.ok()
                } else {
                    None
                };

                let (additions, deletions) = Self::count_line_changes(&old_content, &new_content);

                file_diffs.push(FileDiffData {
                    change: change_type.to_string(),
                    path: file_path.clone(),
                    old_path,
                    additions,
                    deletions,
                    old_content,
                    new_content,
                });
                seen_paths.insert(file_path);
            }
        }

        // Handle untracked files (new files not yet staged)
        let untracked_output = tokio::process::Command::new("git")
            .current_dir(worktree_path)
            .args(["ls-files", "--others", "--exclude-standard"])
            .output()
            .await?;

        let untracked_str = String::from_utf8_lossy(&untracked_output.stdout);

        for file in untracked_str.lines().filter(|l| !l.is_empty()) {
            if seen_paths.contains(file) {
                continue;
            }
            let full_path = worktree_path.join(file);
            if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
                let line_count = content.lines().count() as i32;
                file_diffs.push(FileDiffData {
                    change: "added".to_string(),
                    path: file.to_string(),
                    old_path: None,
                    additions: line_count,
                    deletions: 0,
                    old_content: None,
                    new_content: Some(content),
                });
                seen_paths.insert(file.to_string());
            }
        }

        // Calculate totals (even if empty)
        let total_additions: i32 = file_diffs.iter().map(|f| f.additions).sum();
        let total_deletions: i32 = file_diffs.iter().map(|f| f.deletions).sum();

        // Build and return snapshot
        let snapshot = AttemptDiffSnapshot {
            attempt_id,
            task_id,
            saved_at: Utc::now(),
            base_branch,
            feature_branch,
            total_files: file_diffs.len(),
            total_additions,
            total_deletions,
            files: file_diffs,
            metadata: serde_json::json!({
                "saved_from": "worktree",
                "worktree_path": worktree_path.to_string_lossy(),
                "diff_base_ref": diff_base_ref,
            }),
        };

        Ok(snapshot)
    }

    /// Mark task as InReview (awaiting human review)
    async fn mark_task_in_review(&self, task_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE tasks SET status = 'in_review', updated_at = NOW() WHERE id = $1")
            .bind(task_id)
            .execute(&self.db_pool)
            .await
            .context("Failed to mark task as in_review")?;
        Ok(())
    }

    async fn ensure_repo_cloned(&self, attempt_id: Uuid, repo_path: &Path) -> Result<()> {
        let project_info = self.fetch_project_info(attempt_id).await?;

        if let Some(info) = &project_info {
            if let Some(repo_url) = repository_origin_url(info) {
                // Use project PAT if available (must decrypt), otherwise fallback to system PAT (GitLab or GitHub)
                let pat = match info.pat_encrypted.as_deref() {
                    Some(enc) if !enc.is_empty() => match self.decrypt_value(enc) {
                        Ok(decrypted) => decrypted,
                        Err(e) => {
                            tracing::warn!(
                                "Failed to decrypt project PAT, falling back to system PAT: {}",
                                e
                            );
                            self.get_system_pat_for_repo(repo_url)
                                .await
                                .unwrap_or_default()
                        }
                    },
                    _ => self
                        .get_system_pat_for_repo(repo_url)
                        .await
                        .unwrap_or_default(),
                };

                // Check if repo exists to log appropriate message
                if repo_path.join(".git").exists() {
                    self.log(attempt_id, "system", &format_repository_sync_log(repo_url))
                        .await?;
                } else {
                    self.log(attempt_id, "system", &format_repository_clone_log(repo_url))
                        .await?;
                }

                self.worktree_manager
                    .ensure_cloned_with_upstream(
                        repo_path,
                        repo_url,
                        repository_upstream_url(info).as_deref(),
                        &pat,
                    )
                    .await
                    .context("Failed to clone/sync repository")?;

                self.log(attempt_id, "system", "Repository ready with latest code")
                    .await?;
            }
        }

        Ok(())
    }

    async fn create_worktree(&self, attempt_id: Uuid, repo_path: &Path) -> Result<PathBuf> {
        let project_info = self.fetch_project_info(attempt_id).await?;
        let base_ref_override = project_info.as_ref().and_then(repository_base_ref_override);
        let info = self
            .worktree_manager
            .create_worktree(repo_path, attempt_id, base_ref_override.as_deref())
            .await?;
        self.log(attempt_id, "system", "Ready to work").await?;

        // Store base_branch and feature_branch in attempt metadata for diff computation
        sqlx::query(
            r#"
            UPDATE task_attempts
            SET metadata = metadata || jsonb_build_object(
                'base_branch', $2::text,
                'feature_branch', $3::text
            )
            WHERE id = $1
            "#,
        )
        .bind(attempt_id)
        .bind(&info.base_branch)
        .bind(&info.feature_branch)
        .execute(&self.db_pool)
        .await
        .context("Failed to store branch metadata")?;

        // Persist a fixed diff base commit so we can always compute diffs
        // against the exact pre-attempt state (independent of branch names).
        self.store_diff_base_commit_from_repo(attempt_id, &info.path)
            .await
            .context("Failed to store diff base commit")?;

        Ok(info.path)
    }

    /// Terminate an active agent session gracefully, then force if needed.
    pub async fn terminate_session(&self, attempt_id: Uuid) -> Result<()> {
        let session = self.active_sessions.lock().await.remove(&attempt_id);

        if let Some(session) = session {
            debug!("Terminating session for attempt {}", attempt_id);

            // Take ownership of interrupt sender
            let interrupt_sender = session.interrupt_sender;

            // Get child process
            let mut child_guard = session.child.lock().await;
            if let Some(ref mut child) = *child_guard {
                terminate_process(child, interrupt_sender, GRACEFUL_SHUTDOWN_TIMEOUT).await?;
            }

            self.log(attempt_id, "system", "Agent stopped").await?;
        }

        Ok(())
    }

    async fn handle_gitops(&self, attempt_id: Uuid) -> Result<()> {
        let log_fn = |msg: &str| -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
            let pool = self.db_pool.clone();
            let tx = self.broadcast_tx.clone();
            let msg = msg.to_string();
            Box::pin(
                async move { StatusManager::log(&pool, &tx, attempt_id, "system", &msg).await },
            )
        };

        let system_pat = self.get_system_pat().await;
        let system_gitlab_url = self
            .fetch_system_settings()
            .await
            .ok()
            .map(|s| s.gitlab_url);

        // Agent already pushed changes, just create MR
        GitOpsHandler::create_mr(
            &self.db_pool,
            attempt_id,
            system_pat.as_deref(),
            system_gitlab_url.as_deref(),
            log_fn,
        )
        .await
    }

    async fn handle_gitops_merge(&self, attempt_id: Uuid) -> Result<bool> {
        let log_fn = |msg: &str| -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
            let pool = self.db_pool.clone();
            let tx = self.broadcast_tx.clone();
            let msg = msg.to_string();
            Box::pin(
                async move { StatusManager::log(&pool, &tx, attempt_id, "system", &msg).await },
            )
        };

        let system_pat = self.get_system_pat().await;
        let system_gitlab_url = self
            .fetch_system_settings()
            .await
            .ok()
            .map(|s| s.gitlab_url);

        GitOpsHandler::merge_mr_for_attempt(
            &self.db_pool,
            attempt_id,
            system_pat.as_deref(),
            system_gitlab_url.as_deref(),
            log_fn,
        )
        .await
    }

    async fn finalize_branch_for_no_review(
        &self,
        attempt_id: Uuid,
        worktree_path: &Path,
    ) -> Result<()> {
        self.log(
            attempt_id,
            "system",
            "Finalizing branch: ensuring commit and push before GitOps...",
        )
        .await?;

        let committed = self
            .worktree_manager
            .commit_worktree(
                worktree_path,
                &format!("chore: finalize attempt {}", attempt_id),
            )
            .await?;

        if committed {
            self.log(
                attempt_id,
                "system",
                "Committed local changes to attempt branch.",
            )
            .await?;
        } else {
            self.log(
                attempt_id,
                "system",
                "No new local changes to commit; using current branch HEAD.",
            )
            .await?;
        }

        self.worktree_manager.push_worktree(worktree_path).await?;
        self.log(
            attempt_id,
            "system",
            "Branch pushed to remote successfully.",
        )
        .await?;

        Ok(())
    }

    /// Emit a final user-facing attempt report in timeline after execution ends.
    async fn emit_completion_report(&self, attempt_id: Uuid) -> Result<()> {
        let row = sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                Option<i32>,
                Option<i32>,
                Option<i32>,
                Option<String>,
                serde_json::Value,
                serde_json::Value,
            ),
        >(
            r#"
            SELECT ta.task_id,
                   t.title,
                   ta.diff_total_files,
                   ta.diff_total_additions,
                   ta.diff_total_deletions,
                   ta.s3_diff_key,
                   ta.metadata,
                   t.metadata
            FROM task_attempts ta
            JOIN tasks t ON t.id = ta.task_id
            WHERE ta.id = $1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db_pool)
        .await?;

        let Some((
            task_id,
            task_title,
            diff_files,
            diff_additions,
            diff_deletions,
            s3_diff_key,
            attempt_metadata,
            task_metadata,
        )) = row
        else {
            return Ok(());
        };

        let mr_url: Option<String> = sqlx::query_scalar(
            r#"
            SELECT web_url
            FROM merge_requests
            WHERE attempt_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db_pool)
        .await?
        .or(sqlx::query_scalar(
            r#"
                SELECT web_url
                FROM merge_requests
                WHERE task_id = $1
                ORDER BY created_at DESC
                LIMIT 1
                "#,
        )
        .bind(task_id)
        .fetch_optional(&self.db_pool)
        .await?);

        let preview_url = task_metadata
            .get("preview_url")
            .and_then(|v| v.as_str())
            .or_else(|| {
                attempt_metadata
                    .get("preview_url_agent")
                    .and_then(|v| v.as_str())
            });

        let mut changed_files_preview: Option<String> = None;
        let mut snapshot_summary: Option<(i32, i32, i32)> = None;
        if let Some(key) = s3_diff_key.as_ref() {
            match self.storage_service.download_object_bytes(key).await {
                Ok(bytes) => match serde_json::from_slice::<crate::AttemptDiffSnapshot>(&bytes) {
                    Ok(snapshot) => {
                        snapshot_summary = Some((
                            snapshot.total_files as i32,
                            snapshot.total_additions,
                            snapshot.total_deletions,
                        ));
                        if !snapshot.files.is_empty() {
                            let max_files = 8usize;
                            let names: Vec<String> = snapshot
                                .files
                                .iter()
                                .take(max_files)
                                .map(|f| f.path.clone())
                                .collect();
                            let extra = snapshot.files.len().saturating_sub(names.len());
                            changed_files_preview = Some(if extra > 0 {
                                format!("{} (+{} more)", names.join(", "), extra)
                            } else {
                                names.join(", ")
                            });
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to parse diff snapshot {} for attempt {} report: {}",
                            key, attempt_id, e
                        );
                    }
                },
                Err(e) => {
                    warn!(
                        "Failed to download diff snapshot {} for attempt {} report: {}",
                        key, attempt_id, e
                    );
                }
            }
        }

        let mut lines = vec![
            "## Final Attempt Report".to_string(),
            format!("- Task: {}", task_title),
        ];

        if let Some(files) = diff_files {
            lines.push(format!(
                "- Code changes: {} file(s) (+{} -{})",
                files,
                diff_additions.unwrap_or(0),
                diff_deletions.unwrap_or(0)
            ));
        } else if let Some((files, additions, deletions)) = snapshot_summary {
            lines.push(format!(
                "- Code changes: {} file(s) (+{} -{})",
                files, additions, deletions
            ));
        } else {
            lines.push("- Code changes: no diff summary available".to_string());
        }

        if let Some(url) = mr_url {
            lines.push(format!("- Merge Request: {}", url));
        }

        if let Some(url) = preview_url {
            lines.push(format!("- Preview: {}", url));
        }

        if let Some(files) = changed_files_preview {
            lines.push(format!("- Files changed: {}", files));
        }

        lines.push("".to_string());
        lines.push(
            "If you want any update, send a follow-up input and the agent will continue on a fresh worktree."
                .to_string(),
        );

        self.log(attempt_id, "system", &lines.join("\n")).await?;
        Ok(())
    }

    async fn fetch_project_info(&self, attempt_id: Uuid) -> Result<Option<ProjectInfo>> {
        let info = sqlx::query_as::<_, ProjectInfo>(
            r#"
            SELECT p.repository_url,
                   p.repository_context,
                   g.pat_encrypted,
                   g.gitlab_project_id,
                   g.base_url
            FROM task_attempts ta
            JOIN tasks t ON t.id = ta.task_id
            JOIN projects p ON p.id = t.project_id
            LEFT JOIN gitlab_configurations g ON g.project_id = p.id
            WHERE ta.id = $1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(info)
    }

    /// Load agent settings from project (for router configuration)
    async fn load_agent_settings(&self, attempt_id: Uuid) -> Result<crate::AgentSettings> {
        let settings_json = sqlx::query_scalar::<_, Option<serde_json::Value>>(
            r#"
            SELECT p.agent_settings
            FROM task_attempts ta
            JOIN tasks t ON t.id = ta.task_id
            JOIN projects p ON p.id = t.project_id
            WHERE ta.id = $1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db_pool)
        .await?
        .flatten();

        match settings_json {
            Some(json) => {
                serde_json::from_value(json).with_context(|| "Failed to parse agent_settings JSON")
            }
            None => Ok(crate::AgentSettings::default()),
        }
    }

    /// Get system-level GitLab PAT (decrypted) from system_settings.
    /// Prefer `get_system_pat_for_repo` when repo_url is available (supports GitHub).
    async fn get_system_pat(&self) -> Option<String> {
        let result = sqlx::query_scalar::<_, Option<String>>(
            "SELECT gitlab_pat_encrypted FROM system_settings LIMIT 1",
        )
        .fetch_optional(&self.db_pool)
        .await
        .ok()
        .flatten()
        .flatten();

        if let Some(encrypted) = result {
            self.decrypt_value(&encrypted).ok()
        } else {
            None
        }
    }

    /// Get system PAT for a given repo URL when host matches configured URL (GitLab hoặc GitHub).
    async fn get_system_pat_for_repo(&self, repo_url: &str) -> Option<String> {
        use crate::orchestrator::init_flow::{parse_host_from_urlish, parse_repo_host_and_path};

        let settings = self.fetch_system_settings().await.ok()?;
        let (repo_host, _) = parse_repo_host_and_path(repo_url)?;
        let configured = parse_host_from_urlish(&settings.gitlab_url)?;

        if repo_host.eq_ignore_ascii_case(&configured) {
            if let Some(enc) = settings.gitlab_pat_encrypted.as_ref() {
                return self.decrypt_value(enc).ok();
            }
        }

        None
    }

    /// Send input to an active agent session.
    ///
    /// Queues input for delivery to the agent and broadcasts UserMessage event.
    pub async fn send_input(&self, attempt_id: Uuid, input: &str) -> Result<()> {
        // Get active session
        let sessions = self.active_sessions.lock().await;
        let session = sessions
            .get(&attempt_id)
            .ok_or_else(|| anyhow::anyhow!("No active session for attempt {}", attempt_id))?;

        // Send input to active realtime channel (if executor/mode supports it).
        if let Some(input_sender) = &session.input_sender {
            input_sender.send(input.to_string()).map_err(|_| {
                anyhow::anyhow!("Live input channel is closed for attempt {}", attempt_id)
            })?;
        } else {
            anyhow::bail!("Live input channel is not available for this session");
        }

        // Broadcast as UserMessage event
        let event = crate::AgentEvent::UserMessage(crate::UserMessageEvent {
            attempt_id,
            content: input.to_string(),
            timestamp: chrono::Utc::now(),
        });

        let _ = self.broadcast_tx.send(event);

        Ok(())
    }

    /// Attach or replace the live input channel for an existing attempt session.
    ///
    /// This is used by runtimes that establish input transport out of band and by
    /// integration tests that need to exercise `/attempts/{id}/input` without a full
    /// provider subprocess.
    pub async fn attach_input_sender_for_attempt(
        &self,
        attempt_id: Uuid,
        input_sender: mpsc::UnboundedSender<String>,
    ) {
        let mut sessions = self.active_sessions.lock().await;
        let session = sessions.entry(attempt_id).or_insert_with(|| ActiveSession {
            interrupt_sender: None,
            child: Arc::new(Mutex::new(None)),
            input_sender: None,
        });
        session.input_sender = Some(input_sender);
    }

    /// Send input to an active Project Assistant session (follow-up message).
    pub async fn send_input_to_assistant_session(
        &self,
        session_id: Uuid,
        input: &str,
    ) -> Result<()> {
        let sessions = self.active_assistant_sessions.lock().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| anyhow::anyhow!("No active assistant session for {}", session_id))?;

        if let Some(input_sender) = &session.input_sender {
            input_sender.send(input.to_string()).map_err(|_| {
                anyhow::anyhow!("Live input channel is closed for session {}", session_id)
            })?;
        } else {
            anyhow::bail!("Live input channel is not available for this session");
        }

        Ok(())
    }

    async fn force_stop_assistant_runtime_session(
        active_sessions: Arc<Mutex<HashMap<Uuid, AssistantActiveSession>>>,
        session_id: Uuid,
    ) {
        let Some(session) = active_sessions.lock().await.remove(&session_id) else {
            return;
        };

        let mut child_opt = session.child.lock().await.take();
        if let Some(ref mut child) = child_opt {
            let _ =
                terminate_process(child, session.interrupt_sender, GRACEFUL_SHUTDOWN_TIMEOUT).await;
        }
    }

    /// Spawn Project Assistant CLI session. Streams output to assistant JSONL.
    /// PA chạy trong folder dự án (repo clone) để agent có context code khi trả lời.
    pub async fn spawn_project_assistant_session(
        &self,
        session_id: Uuid,
        project_id: Uuid,
        worktree_path: PathBuf,
        instruction: String,
    ) -> Result<()> {
        let project = self.fetch_project(project_id).await?;

        // Fast path: if the repo already exists locally, use it as-is without
        // any network I/O (fetch/pull). The assistant works on whatever code is
        // on disk — syncing is not required and would add 5-15s of latency.
        // Only clone when the directory doesn't exist yet.
        if worktree_path.join(".git").exists() {
            tracing::info!(
                "Assistant repo already exists at {:?}, using local code",
                worktree_path
            );
        } else if let Some(ref repo_url) = project.repository_url {
            if !repo_url.trim().is_empty() {
                let pat = self.get_system_pat_for_repo(repo_url).await;
                let pat = pat.as_deref().unwrap_or("");
                self.worktree_manager
                    .ensure_repo_exists(
                        &worktree_path,
                        repo_url,
                        project
                            .repository_context
                            .upstream_repository_url
                            .as_deref()
                            .filter(|upstream| !repo_url_matches(upstream, repo_url)),
                        pat,
                    )
                    .await
                    .context("Failed to clone project repo for assistant")?;
            } else {
                tokio::fs::create_dir_all(&worktree_path)
                    .await
                    .context("Failed to create assistant worktree")?;
            }
        } else {
            tokio::fs::create_dir_all(&worktree_path)
                .await
                .context("Failed to create assistant worktree")?;
        }

        let agent_settings = self.load_agent_settings_for_project(project_id).await?;

        // Resolve provider + runtime env with same strategy as attempt-task execution.
        let (provider, provider_env) = self.resolve_agent_cli_for_assistant().await?;

        // Log spawn progress (like attempt task)
        let start_msg = format!("Starting {} Agent...", provider.display_name());
        for content in [
            "Spawning agent for Project Assistant session...",
            start_msg.as_str(),
        ] {
            let created_at = chrono::Utc::now();
            if let Ok(id) = append_assistant_log(session_id, "system", content, None).await {
                let _ = self
                    .broadcast_tx
                    .send(AgentEvent::AssistantLog(AssistantLogMessage {
                        session_id,
                        id,
                        role: "system".to_string(),
                        content: content.to_string(),
                        metadata: None,
                        created_at,
                    }));
            }
        }

        // Claude Project Assistant runs in one-shot `--print` mode, so live stdin follow-up
        // is not exposed for that provider. Follow-up messages are handled via new turns.
        let (session_input_sender, mut stdio_input_rx) =
            if matches!(provider, AgentCliProvider::ClaudeCode) {
                (None, None)
            } else {
                let (tx, rx) = mpsc::unbounded_channel::<String>();
                (Some(tx), Some(rx))
            };
        let provider_env_for_spawn = provider_env.clone();

        let spawn_fut = async {
            match provider {
                AgentCliProvider::ClaudeCode => {
                    self.claude_client
                        .spawn_assistant_session(
                            &worktree_path,
                            &instruction,
                            session_id,
                            Some(&agent_settings),
                        )
                        .await
                }
                AgentCliProvider::OpenAiCodex => {
                    self.codex_client
                        .spawn_session(
                            &worktree_path,
                            &instruction,
                            session_id,
                            provider_env_for_spawn.clone(),
                        )
                        .await
                }
                AgentCliProvider::GeminiCli => {
                    self.gemini_client
                        .spawn_session(
                            &worktree_path,
                            &instruction,
                            session_id,
                            provider_env_for_spawn.clone(),
                        )
                        .await
                }
                AgentCliProvider::CursorCli => {
                    self.cursor_client
                        .spawn_session(
                            &worktree_path,
                            &instruction,
                            session_id,
                            provider_env_for_spawn.clone(),
                        )
                        .await
                }
            }
        };
        let spawned = tokio::time::timeout(SPAWN_TIMEOUT, spawn_fut)
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "Timeout: assistant took more than {:?} to start",
                    SPAWN_TIMEOUT
                )
            })??;

        let SpawnedAgent {
            child,
            interrupt_sender,
            interrupt_receiver: _,
            msg_store: _,
        } = spawned;

        let child_arc = Arc::new(Mutex::new(Some(child)));
        {
            let session = AssistantActiveSession {
                interrupt_sender,
                child: child_arc.clone(),
                input_sender: session_input_sender,
            };
            self.active_assistant_sessions
                .lock()
                .await
                .insert(session_id, session);
        }

        let (stdout, stderr, stdin_opt) = {
            let mut child_guard = child_arc.lock().await;
            let child_ref = child_guard
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("Child process not available"))?;

            let stdout = child_ref.inner().stdout.take().context("No stdout")?;
            let stderr = child_ref.inner().stderr.take().context("No stderr")?;
            let stdin = child_ref.inner().stdin.take();
            (stdout, stderr, stdin)
        };

        // Forward live input to stdin
        if let (Some(mut stdin), Some(mut input_rx)) = (stdin_opt, stdio_input_rx.take()) {
            let session_id = session_id;
            tokio::spawn(async move {
                while let Some(message) = input_rx.recv().await {
                    let trimmed = message.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let line = format!(
                        "{}\n",
                        crate::follow_up_utils::wrap_trivial_follow_up(trimmed)
                    );
                    if let Err(e) = stdin.write_all(line.as_bytes()).await {
                        let _ = append_assistant_log(
                            session_id,
                            "system",
                            &format!("Failed to forward input: {}", e),
                            None,
                        )
                        .await;
                        break;
                    }
                    let _ = stdin.flush().await;
                }
            });
        }

        // Stream stdout to assistant log
        let session_id_stdout = session_id;
        let broadcast_tx_stdout = self.broadcast_tx.clone();
        let provider_stdout = provider;

        match provider_stdout {
            AgentCliProvider::ClaudeCode => {
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stdout);
                    let mut chunk = vec![0_u8; 4096];
                    let mut agent_buffer = AgentTextBuffer::new();
                    loop {
                        let read = match reader.read(&mut chunk).await {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(_) => break,
                        };
                        let text = String::from_utf8_lossy(&chunk[..read]);
                        if text.trim().is_empty() {
                            continue;
                        }

                        agent_buffer.push(&text);
                        let mut emitted_any = false;
                        while let Some((content, metadata)) = agent_buffer.pop_next() {
                            emitted_any = true;
                            let created_at = chrono::Utc::now();
                            if let Ok(id) = append_assistant_log(
                                session_id_stdout,
                                "assistant",
                                &content,
                                metadata.as_ref(),
                            )
                            .await
                            {
                                let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                    AssistantLogMessage {
                                        session_id: session_id_stdout,
                                        id,
                                        role: "assistant".to_string(),
                                        content,
                                        metadata,
                                        created_at,
                                    },
                                ));
                            }
                        }
                        if !emitted_any {
                            if let Some((content, metadata)) =
                                agent_buffer.pop_partial_text_for_display()
                            {
                                let created_at = chrono::Utc::now();
                                if let Ok(id) = append_assistant_log(
                                    session_id_stdout,
                                    "assistant",
                                    &content,
                                    metadata.as_ref(),
                                )
                                .await
                                {
                                    let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                        AssistantLogMessage {
                                            session_id: session_id_stdout,
                                            id,
                                            role: "assistant".to_string(),
                                            content,
                                            metadata,
                                            created_at,
                                        },
                                    ));
                                }
                            }
                        }
                    }
                    if let Some((content, metadata)) = agent_buffer.flush() {
                        let created_at = chrono::Utc::now();
                        if let Ok(id) = append_assistant_log(
                            session_id_stdout,
                            "assistant",
                            &content,
                            metadata.as_ref(),
                        )
                        .await
                        {
                            let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                AssistantLogMessage {
                                    session_id: session_id_stdout,
                                    id,
                                    role: "assistant".to_string(),
                                    content,
                                    metadata,
                                    created_at,
                                },
                            ));
                        }
                    }
                });
            }
            AgentCliProvider::OpenAiCodex => {
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stdout).lines();
                    let mut agent_buffer = AgentTextBuffer::new();
                    while let Ok(Some(line)) = reader.next_line().await {
                        let events = crate::codex::parse_codex_json_events(&line);
                        let mut had_event = false;
                        for ev in events {
                            let text = match ev {
                                crate::codex::CodexStreamEvent::AgentMessage { text, .. }
                                | crate::codex::CodexStreamEvent::NextAction { text } => text,
                                crate::codex::CodexStreamEvent::CommandCompleted { .. } => continue,
                                _ => continue,
                            };
                            if text.trim().is_empty() {
                                continue;
                            }
                            had_event = true;
                            agent_buffer.push(&text);
                            let mut emitted_any = false;
                            while let Some((content, metadata)) = agent_buffer.pop_next() {
                                emitted_any = true;
                                let created_at = chrono::Utc::now();
                                if let Ok(id) = append_assistant_log(
                                    session_id_stdout,
                                    "assistant",
                                    &content,
                                    metadata.as_ref(),
                                )
                                .await
                                {
                                    let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                        AssistantLogMessage {
                                            session_id: session_id_stdout,
                                            id,
                                            role: "assistant".to_string(),
                                            content,
                                            metadata,
                                            created_at,
                                        },
                                    ));
                                }
                            }
                            if !emitted_any {
                                if let Some((content, metadata)) =
                                    agent_buffer.pop_partial_text_for_display()
                                {
                                    let created_at = chrono::Utc::now();
                                    if let Ok(id) = append_assistant_log(
                                        session_id_stdout,
                                        "assistant",
                                        &content,
                                        metadata.as_ref(),
                                    )
                                    .await
                                    {
                                        let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                            AssistantLogMessage {
                                                session_id: session_id_stdout,
                                                id,
                                                role: "assistant".to_string(),
                                                content,
                                                metadata,
                                                created_at,
                                            },
                                        ));
                                    }
                                }
                            }
                        }
                        if !had_event {
                            if let Some(text) =
                                crate::codex::extract_agent_text_from_json_line(&line)
                            {
                                agent_buffer.push(&text);
                                let mut emitted_any = false;
                                while let Some((content, metadata)) = agent_buffer.pop_next() {
                                    emitted_any = true;
                                    let created_at = chrono::Utc::now();
                                    if let Ok(id) = append_assistant_log(
                                        session_id_stdout,
                                        "assistant",
                                        &content,
                                        metadata.as_ref(),
                                    )
                                    .await
                                    {
                                        let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                            AssistantLogMessage {
                                                session_id: session_id_stdout,
                                                id,
                                                role: "assistant".to_string(),
                                                content,
                                                metadata,
                                                created_at,
                                            },
                                        ));
                                    }
                                }
                                if !emitted_any {
                                    if let Some((content, metadata)) =
                                        agent_buffer.pop_partial_text_for_display()
                                    {
                                        let created_at = chrono::Utc::now();
                                        if let Ok(id) = append_assistant_log(
                                            session_id_stdout,
                                            "assistant",
                                            &content,
                                            metadata.as_ref(),
                                        )
                                        .await
                                        {
                                            let _ = broadcast_tx_stdout.send(
                                                AgentEvent::AssistantLog(AssistantLogMessage {
                                                    session_id: session_id_stdout,
                                                    id,
                                                    role: "assistant".to_string(),
                                                    content,
                                                    metadata,
                                                    created_at,
                                                }),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some((content, metadata)) = agent_buffer.flush() {
                        let created_at = chrono::Utc::now();
                        if let Ok(id) = append_assistant_log(
                            session_id_stdout,
                            "assistant",
                            &content,
                            metadata.as_ref(),
                        )
                        .await
                        {
                            let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                AssistantLogMessage {
                                    session_id: session_id_stdout,
                                    id,
                                    role: "assistant".to_string(),
                                    content,
                                    metadata,
                                    created_at,
                                },
                            ));
                        }
                    }
                });
            }
            AgentCliProvider::GeminiCli => {
                let active_sessions_stdout = self.active_assistant_sessions.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stdout).lines();
                    let mut agent_buffer = AgentTextBuffer::new();
                    while let Ok(Some(line)) = reader.next_line().await {
                        let mut emitted_for_line = false;
                        for ev in crate::gemini::parse_gemini_json_events(&line) {
                            let text = match ev {
                                crate::gemini::GeminiStreamEvent::AgentMessage { text, .. }
                                | crate::gemini::GeminiStreamEvent::NextAction { text } => text,
                                _ => continue,
                            };
                            if text.trim().is_empty() {
                                continue;
                            }
                            emitted_for_line = true;
                            agent_buffer.push(&text);
                            let mut emitted_any = false;
                            while let Some((content, metadata)) = agent_buffer.pop_next() {
                                emitted_any = true;
                                let created_at = chrono::Utc::now();
                                if let Ok(id) = append_assistant_log(
                                    session_id_stdout,
                                    "assistant",
                                    &content,
                                    metadata.as_ref(),
                                )
                                .await
                                {
                                    let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                        AssistantLogMessage {
                                            session_id: session_id_stdout,
                                            id,
                                            role: "assistant".to_string(),
                                            content,
                                            metadata,
                                            created_at,
                                        },
                                    ));
                                }
                            }
                            if !emitted_any {
                                if let Some((content, metadata)) =
                                    agent_buffer.pop_partial_text_for_display()
                                {
                                    let created_at = chrono::Utc::now();
                                    if let Ok(id) = append_assistant_log(
                                        session_id_stdout,
                                        "assistant",
                                        &content,
                                        metadata.as_ref(),
                                    )
                                    .await
                                    {
                                        let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                            AssistantLogMessage {
                                                session_id: session_id_stdout,
                                                id,
                                                role: "assistant".to_string(),
                                                content,
                                                metadata,
                                                created_at,
                                            },
                                        ));
                                    }
                                }
                            }
                        }

                        if !emitted_for_line {
                            if let Some(fallback_text) =
                                normalize_assistant_plain_stdout_line(&line)
                            {
                                let auth_hint = detect_provider_auth_blocker(
                                    AgentCliProvider::GeminiCli,
                                    &fallback_text,
                                );
                                let role = if auth_hint.is_some() {
                                    "system"
                                } else {
                                    "assistant"
                                };
                                let display_content =
                                    auth_hint.unwrap_or(&fallback_text).to_string();
                                let created_at = chrono::Utc::now();
                                if let Ok(id) = append_assistant_log(
                                    session_id_stdout,
                                    role,
                                    &display_content,
                                    None,
                                )
                                .await
                                {
                                    let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                        AssistantLogMessage {
                                            session_id: session_id_stdout,
                                            id,
                                            role: role.to_string(),
                                            content: display_content.clone(),
                                            metadata: None,
                                            created_at,
                                        },
                                    ));
                                }

                                if auth_hint.is_some() {
                                    ExecutorOrchestrator::force_stop_assistant_runtime_session(
                                        active_sessions_stdout.clone(),
                                        session_id_stdout,
                                    )
                                    .await;
                                    break;
                                }
                            }
                        }
                    }
                    if let Some((content, metadata)) = agent_buffer.flush() {
                        let created_at = chrono::Utc::now();
                        if let Ok(id) = append_assistant_log(
                            session_id_stdout,
                            "assistant",
                            &content,
                            metadata.as_ref(),
                        )
                        .await
                        {
                            let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                AssistantLogMessage {
                                    session_id: session_id_stdout,
                                    id,
                                    role: "assistant".to_string(),
                                    content,
                                    metadata,
                                    created_at,
                                },
                            ));
                        }
                    }
                });
            }
            AgentCliProvider::CursorCli => {
                let active_sessions_stdout = self.active_assistant_sessions.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stdout).lines();
                    let mut agent_buffer = AgentTextBuffer::new();
                    let mut last_emitted_assistant_text: Option<String> = None;
                    while let Ok(Some(line)) = reader.next_line().await {
                        let mut emitted_for_line = false;
                        for ev in crate::cursor::parse_cursor_json_events(&line) {
                            let text = match &ev {
                                crate::cursor::CursorStreamEvent::AgentMessage { text, .. } => {
                                    text.clone()
                                }
                                crate::cursor::CursorStreamEvent::Result { result, .. } => {
                                    result.clone()
                                }
                                _ => continue,
                            };
                            if text.trim().is_empty() {
                                continue;
                            }
                            if is_immediate_duplicate_assistant_text(
                                last_emitted_assistant_text.as_deref(),
                                &text,
                            ) {
                                continue;
                            }
                            emitted_for_line = true;
                            agent_buffer.push(&text);
                            let mut emitted_any = false;
                            while let Some((content, metadata)) = agent_buffer.pop_next() {
                                if is_immediate_duplicate_assistant_text(
                                    last_emitted_assistant_text.as_deref(),
                                    &content,
                                ) {
                                    continue;
                                }
                                emitted_any = true;
                                let created_at = chrono::Utc::now();
                                if let Ok(id) = append_assistant_log(
                                    session_id_stdout,
                                    "assistant",
                                    &content,
                                    metadata.as_ref(),
                                )
                                .await
                                {
                                    last_emitted_assistant_text = Some(content.clone());
                                    let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                        AssistantLogMessage {
                                            session_id: session_id_stdout,
                                            id,
                                            role: "assistant".to_string(),
                                            content,
                                            metadata,
                                            created_at,
                                        },
                                    ));
                                }
                            }
                            if !emitted_any {
                                if let Some((content, metadata)) =
                                    agent_buffer.pop_partial_text_for_display()
                                {
                                    if is_immediate_duplicate_assistant_text(
                                        last_emitted_assistant_text.as_deref(),
                                        &content,
                                    ) {
                                        continue;
                                    }
                                    let created_at = chrono::Utc::now();
                                    if let Ok(id) = append_assistant_log(
                                        session_id_stdout,
                                        "assistant",
                                        &content,
                                        metadata.as_ref(),
                                    )
                                    .await
                                    {
                                        last_emitted_assistant_text = Some(content.clone());
                                        let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                            AssistantLogMessage {
                                                session_id: session_id_stdout,
                                                id,
                                                role: "assistant".to_string(),
                                                content,
                                                metadata,
                                                created_at,
                                            },
                                        ));
                                    }
                                }
                            }
                        }

                        if !emitted_for_line {
                            if let Some(fallback_text) =
                                normalize_assistant_plain_stdout_line(&line)
                            {
                                let auth_hint = detect_provider_auth_blocker(
                                    AgentCliProvider::CursorCli,
                                    &fallback_text,
                                );
                                let role = if auth_hint.is_some() {
                                    "system"
                                } else {
                                    "assistant"
                                };
                                let display_content =
                                    auth_hint.unwrap_or(&fallback_text).to_string();

                                if role == "assistant"
                                    && is_immediate_duplicate_assistant_text(
                                        last_emitted_assistant_text.as_deref(),
                                        &display_content,
                                    )
                                {
                                    continue;
                                }

                                let created_at = chrono::Utc::now();
                                if let Ok(id) = append_assistant_log(
                                    session_id_stdout,
                                    role,
                                    &display_content,
                                    None,
                                )
                                .await
                                {
                                    if role == "assistant" {
                                        last_emitted_assistant_text = Some(display_content.clone());
                                    }
                                    let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                        AssistantLogMessage {
                                            session_id: session_id_stdout,
                                            id,
                                            role: role.to_string(),
                                            content: display_content.clone(),
                                            metadata: None,
                                            created_at,
                                        },
                                    ));
                                }

                                if auth_hint.is_some() {
                                    ExecutorOrchestrator::force_stop_assistant_runtime_session(
                                        active_sessions_stdout.clone(),
                                        session_id_stdout,
                                    )
                                    .await;
                                    break;
                                }
                            }
                        }
                    }
                    if let Some((content, metadata)) = agent_buffer.flush() {
                        if is_immediate_duplicate_assistant_text(
                            last_emitted_assistant_text.as_deref(),
                            &content,
                        ) {
                            return;
                        }
                        let created_at = chrono::Utc::now();
                        if let Ok(id) = append_assistant_log(
                            session_id_stdout,
                            "assistant",
                            &content,
                            metadata.as_ref(),
                        )
                        .await
                        {
                            let _ = broadcast_tx_stdout.send(AgentEvent::AssistantLog(
                                AssistantLogMessage {
                                    session_id: session_id_stdout,
                                    id,
                                    role: "assistant".to_string(),
                                    content,
                                    metadata,
                                    created_at,
                                },
                            ));
                        }
                    }
                });
            }
        }

        // Stream stderr to assistant log
        let session_id_stderr = session_id;
        let provider_stderr = provider;
        let active_sessions_stderr = self.active_assistant_sessions.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            let mut auth_blocked = false;
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = append_assistant_log(session_id_stderr, "stderr", &line, None).await;

                if auth_blocked {
                    continue;
                }

                let auth_hint = match provider_stderr {
                    AgentCliProvider::GeminiCli => {
                        detect_provider_auth_blocker(AgentCliProvider::GeminiCli, &line)
                    }
                    AgentCliProvider::CursorCli => {
                        detect_provider_auth_blocker(AgentCliProvider::CursorCli, &line)
                    }
                    _ => None,
                };

                if let Some(hint) = auth_hint {
                    auth_blocked = true;
                    let _ = append_assistant_log(session_id_stderr, "system", hint, None).await;
                    ExecutorOrchestrator::force_stop_assistant_runtime_session(
                        active_sessions_stderr.clone(),
                        session_id_stderr,
                    )
                    .await;
                }
            }
        });

        let child_arc_wait = child_arc.clone();
        let active_sessions = self.active_assistant_sessions.clone();
        tokio::spawn(async move {
            loop {
                let should_cleanup = {
                    let mut child_guard = child_arc_wait.lock().await;
                    match child_guard.as_mut() {
                        Some(child) => match child.inner().try_wait() {
                            Ok(Some(status)) => {
                                info!(
                                    session_id = %session_id,
                                    exit_status = ?status,
                                    "Project Assistant session exited"
                                );
                                let _ = child_guard.take();
                                true
                            }
                            Ok(None) => false,
                            Err(err) => {
                                warn!(
                                    session_id = %session_id,
                                    error = %err,
                                    "Failed while checking Project Assistant session status"
                                );
                                let _ = child_guard.take();
                                true
                            }
                        },
                        None => true,
                    }
                };

                if should_cleanup {
                    active_sessions.lock().await.remove(&session_id);
                    break;
                }

                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        });

        Ok(())
    }

    /// Terminate an active Project Assistant session.
    pub async fn terminate_assistant_session(&self, session_id: Uuid) -> Result<()> {
        let session = self
            .active_assistant_sessions
            .lock()
            .await
            .remove(&session_id)
            .ok_or_else(|| anyhow::anyhow!("No active assistant session for {}", session_id))?;

        let mut child_opt = session.child.lock().await.take();
        if let Some(ref mut child) = child_opt {
            let _ =
                terminate_process(child, session.interrupt_sender, GRACEFUL_SHUTDOWN_TIMEOUT).await;
        }

        Ok(())
    }

    /// Check if assistant session is active.
    pub async fn is_assistant_session_active(&self, session_id: Uuid) -> bool {
        self.active_assistant_sessions
            .lock()
            .await
            .contains_key(&session_id)
    }

    async fn load_agent_settings_for_project(
        &self,
        project_id: Uuid,
    ) -> Result<crate::AgentSettings> {
        let settings_json = sqlx::query_scalar::<_, Option<serde_json::Value>>(
            r#"SELECT agent_settings FROM projects WHERE id = $1"#,
        )
        .bind(project_id)
        .fetch_optional(&self.db_pool)
        .await?
        .flatten();

        match settings_json {
            Some(json) => {
                serde_json::from_value(json).with_context(|| "Failed to parse agent_settings JSON")
            }
            None => Ok(crate::AgentSettings::default()),
        }
    }

    async fn update_status(&self, attempt_id: Uuid, status: AttemptStatus) -> Result<()> {
        StatusManager::update_status(&self.db_pool, &self.broadcast_tx, attempt_id, status).await
    }

    async fn set_latest_execution_process_pid(
        &self,
        attempt_id: Uuid,
        process_id: i32,
    ) -> Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE execution_processes
            SET process_id = $1
            WHERE id = (
                SELECT id
                FROM execution_processes
                WHERE attempt_id = $2
                  AND process_id IS NULL
                ORDER BY created_at DESC, id DESC
                LIMIT 1
            )
            "#,
        )
        .bind(process_id)
        .bind(attempt_id)
        .execute(&self.db_pool)
        .await
        .context("Failed to set execution process PID")?;

        if result.rows_affected() == 0 {
            warn!(
                attempt_id = %attempt_id,
                process_id,
                "No pending execution process record found while attaching PID"
            );
        }

        Ok(())
    }

    async fn attach_execution_process_pid(&self, attempt_id: Uuid, child: &mut AsyncGroupChild) {
        let Some(process_id_raw) = child.inner().id() else {
            warn!(
                attempt_id = %attempt_id,
                "Spawned child process has no PID; skipping execution process linkage"
            );
            return;
        };

        let Ok(process_id) = i32::try_from(process_id_raw) else {
            warn!(
                attempt_id = %attempt_id,
                process_id = process_id_raw,
                "Process ID exceeded i32 range; skipping execution process linkage"
            );
            return;
        };

        if let Err(error) = self
            .set_latest_execution_process_pid(attempt_id, process_id)
            .await
        {
            warn!(
                attempt_id = %attempt_id,
                process_id,
                error = %error,
                "Failed to persist process ID on execution process record"
            );
        }
    }

    /// Returns a user-friendly message when deployment hook fails.
    /// Cloudflare/tunnel errors get a specific message; others get a generic one.
    fn format_deployment_hook_failure_user_message(e: &anyhow::Error) -> String {
        let err_lower = e.to_string().to_lowercase();
        if err_lower.contains("cloudflare") || err_lower.contains("tunnel") {
            "Cloudflare tunnel could not be configured. In System Settings (/settings), ensure Cloudflare Account ID, API Token, Zone ID, and Base Domain are all set. Session completed successfully.".to_string()
        } else {
            "Preview deployment could not be completed. Session completed successfully.".to_string()
        }
    }

    async fn run_before_success_hook(&self, attempt_id: Uuid) -> Result<()> {
        let Some(hook) = &self.attempt_success_hook else {
            return Ok(());
        };

        hook.before_mark_success(attempt_id).await?;

        Ok(())
    }

    async fn fail_attempt(&self, attempt_id: Uuid, error: &str) -> Result<()> {
        StatusManager::fail_attempt(&self.db_pool, &self.broadcast_tx, attempt_id, error).await
    }

    /// Fail attempt with specific failure reason (for router crash, timeout, etc.)
    async fn fail_attempt_with_reason(
        &self,
        attempt_id: Uuid,
        failure_reason: &str,
        error_message: &str,
    ) -> Result<()> {
        let result = sqlx::query(
            r#"UPDATE task_attempts
               SET status = 'failed',
                   completed_at = now(),
                   error_message = $2,
                   failure_reason = $3
               WHERE id = $1
                 AND status != 'cancelled'"#,
        )
        .bind(attempt_id)
        .bind(error_message)
        .bind(failure_reason)
        .execute(&self.db_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(());
        }

        // Broadcast failure event
        let _ = self
            .broadcast_tx
            .send(crate::AgentEvent::Status(crate::StatusMessage {
                attempt_id,
                status: acpms_db::models::AttemptStatus::Failed,
                timestamp: chrono::Utc::now(),
            }));

        // Reset task status
        sqlx::query(
            r#"UPDATE tasks
               SET status = 'todo'
               WHERE id = (SELECT task_id FROM task_attempts WHERE id = $1)
               AND status = 'in_progress'"#,
        )
        .bind(attempt_id)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Fail attempt with auto-retry support.
    ///
    /// Checks project settings for auto_retry flag and schedules retry if:
    /// - auto_retry is enabled
    /// - retry count < max_retries
    /// - error is retriable (not auth/permission errors)
    async fn fail_attempt_with_retry(
        &self,
        attempt_id: Uuid,
        task_id: Uuid,
        error: &str,
    ) -> Result<()> {
        // First, mark the current attempt as failed
        self.fail_attempt(attempt_id, error).await?;

        let current_status: Option<String> =
            sqlx::query_scalar("SELECT status::text FROM task_attempts WHERE id = $1")
                .bind(attempt_id)
                .fetch_optional(&self.db_pool)
                .await
                .ok()
                .flatten();

        if current_status.as_deref() == Some("cancelled") {
            return Ok(());
        }

        // Get project_id from task
        let project_id: Uuid =
            match sqlx::query_scalar("SELECT project_id FROM tasks WHERE id = $1")
                .bind(task_id)
                .fetch_one(&self.db_pool)
                .await
            {
                Ok(id) => id,
                Err(e) => {
                    warn!("Failed to fetch project_id for task {}: {}", task_id, e);
                    return Ok(());
                }
            };

        // Try to fetch project settings for retry logic
        let settings = match self.fetch_project_settings(project_id).await {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to fetch project settings for retry: {}", e);
                return Ok(()); // Continue without retry
            }
        };

        let retry_handler = RetryHandler::new(&settings);

        // Check if auto-retry is enabled
        if !retry_handler.is_auto_retry_enabled() {
            info!("Auto-retry disabled for project {}", project_id);
            return Ok(());
        }

        // Check if error is retriable
        if !retry_handler.is_retriable_error(error) {
            info!("Error is not retriable: {}", error);
            return Ok(());
        }

        // Fetch current attempt to check retry count
        let attempt = match self.fetch_attempt(attempt_id).await {
            Ok(a) => a,
            Err(e) => {
                warn!("Failed to fetch attempt for retry: {}", e);
                return Ok(());
            }
        };

        // Check if we should retry
        if !retry_handler.should_retry(&attempt) {
            info!(
                "Max retries ({}) exceeded for task {}",
                settings.max_retries, task_id
            );
            return Ok(());
        }

        // Schedule retry
        match retry_handler
            .schedule_retry(&self.db_pool, task_id, &attempt, error)
            .await
        {
            Ok(RetryScheduleResult::Scheduled {
                attempt_id: new_attempt_id,
                retry_count,
                backoff,
                ..
            }) => {
                self.log(
                    attempt_id,
                    "system",
                    &format!(
                        "🔄 Auto-retry scheduled (attempt {}/{}), backoff: {:?}",
                        retry_count, settings.max_retries, backoff
                    ),
                )
                .await?;

                // Mark original attempt with retry info
                let _ = retry_handler
                    .mark_attempt_for_retry(&self.db_pool, attempt_id, new_attempt_id)
                    .await;

                // Update task status back to todo (ready for retry)
                sqlx::query("UPDATE tasks SET status = 'todo', updated_at = NOW() WHERE id = $1")
                    .bind(task_id)
                    .execute(&self.db_pool)
                    .await?;

                info!(
                    "Retry attempt {} created for task {}",
                    new_attempt_id, task_id
                );
            }
            Ok(RetryScheduleResult::MaxRetriesExceeded {
                retry_count,
                max_retries,
            }) => {
                self.log(
                    attempt_id,
                    "system",
                    &format!("❌ Max retries exceeded ({}/{})", retry_count, max_retries),
                )
                .await?;
            }
            Ok(RetryScheduleResult::NonRetriableError { error }) => {
                self.log(
                    attempt_id,
                    "system",
                    &format!("⚠️ Non-retriable error: {}", error),
                )
                .await?;
            }
            Err(e) => {
                warn!("Failed to schedule retry: {}", e);
            }
        }

        Ok(())
    }

    /// Fetch project settings by project_id.
    async fn fetch_project_settings(&self, project_id: Uuid) -> Result<ProjectSettings> {
        let settings_json: serde_json::Value = sqlx::query_scalar(
            "SELECT COALESCE(settings, '{}'::jsonb) FROM projects WHERE id = $1",
        )
        .bind(project_id)
        .fetch_one(&self.db_pool)
        .await
        .context("Failed to fetch project settings")?;

        let settings: ProjectSettings = serde_json::from_value(settings_json).unwrap_or_default();

        Ok(settings)
    }

    /// Fetch task attempt by attempt_id.
    async fn fetch_attempt(&self, attempt_id: Uuid) -> Result<TaskAttempt> {
        let attempt = sqlx::query_as::<_, TaskAttempt>("SELECT * FROM task_attempts WHERE id = $1")
            .bind(attempt_id)
            .fetch_one(&self.db_pool)
            .await
            .context("Failed to fetch task attempt")?;

        Ok(attempt)
    }

    async fn cancel_attempt(&self, attempt_id: Uuid, reason: &str) -> Result<()> {
        StatusManager::cancel_attempt(&self.db_pool, &self.broadcast_tx, attempt_id, reason).await
    }

    async fn log(&self, attempt_id: Uuid, role: &str, content: &str) -> Result<()> {
        let sanitized_content = sanitize_log(content);
        StatusManager::log(
            &self.db_pool,
            &self.broadcast_tx,
            attempt_id,
            role,
            &sanitized_content,
        )
        .await
    }

    async fn cleanup_and_cancel(
        &self,
        attempt_id: Uuid,
        repo_path: &Path,
        reason: &str,
    ) -> Result<()> {
        self.cancel_attempt(attempt_id, reason).await?;
        if let Err(e) = self.cleanup_attempt_worktree(repo_path, attempt_id).await {
            warn!("Cleanup failed for cancelled attempt {}: {}", attempt_id, e);
        }
        Ok(())
    }

    /// Spawn agent with GitLab PAT for init tasks.
    ///
    /// ## Security
    /// - Accepts GitLab PAT and URL as parameters (fetched by caller)
    /// - Injects PAT as environment variable (GITLAB_PAT)
    /// - Used for from-scratch project initialization
    ///
    /// ## Usage
    /// ```ignore
    /// let settings_service = SystemSettingsService::new(pool)?;
    /// let gitlab_pat = settings_service.get_gitlab_pat().await?
    ///     .context("GitLab PAT not configured")?;
    /// let config = settings_service.get().await?;
    ///
    /// orchestrator.spawn_agent_with_gitlab_pat(
    ///     &worktree_path,
    ///     &instruction,
    ///     attempt_id,
    ///     &gitlab_pat,
    ///     &config.gitlab_url
    /// ).await?;
    /// ```
    pub async fn spawn_agent_with_gitlab_pat(
        &self,
        worktree_path: &Path,
        instruction: &str,
        attempt_id: Uuid,
        gitlab_pat: &str,
        gitlab_url: &str,
    ) -> Result<SpawnedAgent> {
        let (provider, provider_env) = self.resolve_agent_cli(attempt_id).await?;

        // Prepare environment variables
        let mut env_vars = HashMap::new();
        env_vars.insert("GITLAB_PAT".to_string(), gitlab_pat.to_string());
        env_vars.insert("GITLAB_URL".to_string(), gitlab_url.to_string());
        if let Some(extra_env) = provider_env {
            env_vars.extend(extra_env);
        }
        self.extend_agent_env_with_cloudflare_settings(&mut env_vars)
            .await;

        // Spawn with timeout and env vars
        tokio::time::timeout(SPAWN_TIMEOUT, async {
            match provider {
                AgentCliProvider::ClaudeCode => {
                    let agent_settings = self.load_agent_settings(attempt_id).await?;
                    self.claude_client
                        .spawn_session(
                            worktree_path,
                            instruction,
                            attempt_id,
                            Some(env_vars),
                            Some(&agent_settings),
                        )
                        .await
                }
                AgentCliProvider::OpenAiCodex => {
                    self.codex_client
                        .spawn_session(worktree_path, instruction, attempt_id, Some(env_vars))
                        .await
                }
                AgentCliProvider::GeminiCli => {
                    self.gemini_client
                        .spawn_session(worktree_path, instruction, attempt_id, Some(env_vars))
                        .await
                }
                AgentCliProvider::CursorCli => {
                    self.cursor_client
                        .spawn_session(worktree_path, instruction, attempt_id, Some(env_vars))
                        .await
                }
            }
        })
        .await
        .map_err(|_| {
            anyhow::anyhow!("Timeout: agent took more than {:?} to start", SPAWN_TIMEOUT)
        })?
    }

    /// Execute task with routing based on task type (init vs regular).
    ///
    /// ## Task Type Routing
    /// - **Init tasks**: Route to execute_init_task() for project initialization
    /// - **Regular tasks**: Use existing execute_task_with_cancel() for feature/bug/refactor
    pub async fn execute_task(&self, task_id: Uuid) -> Result<()> {
        self.execute_task_with_attempt(task_id, None).await
    }

    /// Execute task with an existing attempt_id (avoids creating duplicate attempts).
    ///
    /// ## Parameters
    /// - `task_id`: The task to execute
    /// - `attempt_id`: Optional existing attempt_id to use (if None, creates new attempt)
    pub async fn execute_task_with_attempt(
        &self,
        task_id: Uuid,
        attempt_id: Option<Uuid>,
    ) -> Result<()> {
        // Fetch task from database
        let task = self.fetch_task(task_id).await?;

        match task.task_type {
            TaskType::Init => {
                self.execute_init_task(task_id, &task, attempt_id).await?;
            }
            TaskType::Feature
            | TaskType::Bug
            | TaskType::Refactor
            | TaskType::Docs
            | TaskType::Test
            | TaskType::Hotfix
            | TaskType::Chore
            | TaskType::Spike
            | TaskType::SmallTask
            | TaskType::Deploy => {
                // For regular tasks, use provided attempt_id when available.
                let attempt_id = match attempt_id {
                    Some(existing_attempt_id) => existing_attempt_id,
                    None => self.create_attempt(task_id).await?,
                };
                let project = self.fetch_project(task.project_id).await?;
                let settings = self
                    .fetch_project_settings(task.project_id)
                    .await
                    .unwrap_or_default();
                let require_review = task
                    .metadata
                    .get("execution")
                    .and_then(|v| v.get("require_review"))
                    .and_then(|v| v.as_bool())
                    .or_else(|| {
                        task.metadata
                            .get("require_review")
                            .and_then(|v| v.as_bool())
                    })
                    .unwrap_or(settings.require_review);
                let run_build_and_tests = task
                    .metadata
                    .get("execution")
                    .and_then(|v| v.get("run_build_and_tests"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let repo_path = if project.repository_url.is_some() {
                    self.worktree_manager
                        .base_path()
                        .await
                        .join(project_repo_relative_path(
                            project.id,
                            &project.metadata,
                            &project.name,
                        ))
                } else {
                    bail!("Project {} has no repository URL", project.id);
                };

                let task_desc = task.description.as_deref().unwrap_or(&task.title);
                let skill_context = self.build_skill_instruction_context(
                    &task,
                    &settings,
                    project.project_type,
                    Some(repo_path.as_path()),
                );
                if let Err(error) = self
                    .persist_skill_instruction_context(
                        attempt_id,
                        &skill_context,
                        "orchestrator_execute_task",
                    )
                    .await
                {
                    warn!(
                        attempt_id = %attempt_id,
                        error = %error,
                        "Failed to persist skill instruction metadata from orchestrator"
                    );
                }
                if let Err(error) = self.log_loaded_skills(attempt_id, &skill_context).await {
                    warn!(
                        attempt_id = %attempt_id,
                        error = %error,
                        "Failed to append skill timeline log from orchestrator"
                    );
                }
                let skill_block = skill_context.block.clone();
                let verification_rule = if run_build_and_tests {
                    "2. Run verification (build, lint, tests as appropriate)."
                } else {
                    "2. Keep changes focused and lightweight. Skip expensive build/test runs unless absolutely necessary."
                };

                // Build instruction based on require_review setting
                let instruction = if require_review {
                    // Review required: Agent only implements, no commit/push
                    format!(
                        r#"## Task
{}

## Workflow
1. Implement the task above
{}
3. Only modify files necessary for this task
4. Prepare a review handoff summary with changed files, risks, and verification coverage.

IMPORTANT: Do NOT commit or push changes. Changes will be reviewed by a human before committing.{}"#,
                        task_desc, verification_rule, skill_block
                    )
                } else {
                    // No review: Agent handles full workflow including commit/push
                    format!(
                        r#"## Task
{}

## Workflow
1. Implement the task above
{}
3. Only modify files necessary for this task
4. Stage ONLY the files you changed: `git add <specific-files>`
5. Commit with descriptive message: `git commit -m "feat: <description>"`
6. Push to remote: `git push origin HEAD`
7. Include deployment/report details required by active skills.

IMPORTANT: Do not commit unrelated files. Verify changes work before committing.{}"#,
                        task_desc, verification_rule, skill_block
                    )
                };

                let (_tx, rx) = watch::channel(false);
                self.execute_task_with_cancel_review(
                    attempt_id,
                    task_id,
                    repo_path,
                    instruction,
                    rx,
                    require_review,
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Resume execution for an existing attempt with a new instruction.
    /// Re-spawns the agent in the repo directory and handles completion.
    /// Used for follow-up messages on completed attempts.
    pub async fn execute_agent_for_attempt(
        &self,
        attempt_id: Uuid,
        repo_path: &Path,
        instruction: &str,
    ) -> Result<()> {
        // Get task_id from attempt
        let attempt = sqlx::query_as::<_, TaskAttempt>("SELECT * FROM task_attempts WHERE id = $1")
            .bind(attempt_id)
            .fetch_optional(&self.db_pool)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Attempt not found: {}", attempt_id))?;

        let task_id = attempt.task_id;

        // Resolve execution path for follow-up:
        // - Reuse existing worktree if still present
        // - If cleaned already, create a fresh worktree (safer than running in repo root)
        let existing_worktree = attempt
            .metadata
            .get("worktree_path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from);

        let effective_path = if let Some(path) = existing_worktree {
            if path.exists() {
                path
            } else {
                self.log(
                    attempt_id,
                    "system",
                    "Previous worktree was cleaned. Creating a fresh worktree for follow-up...",
                )
                .await?;
                // Best-effort stale cleanup (branch/worktree metadata) before recreation.
                let _ = self.cleanup_attempt_worktree(repo_path, attempt_id).await;
                let fresh = self
                    .create_worktree(attempt_id, repo_path)
                    .await
                    .context("Failed to create fresh follow-up worktree")?;
                self.store_worktree_path(attempt_id, &fresh).await?;
                fresh
            }
        } else {
            self.log(
                attempt_id,
                "system",
                "No worktree metadata found. Creating a fresh worktree for follow-up...",
            )
            .await?;
            let fresh = self
                .create_worktree(attempt_id, repo_path)
                .await
                .context("Failed to create follow-up worktree")?;
            self.store_worktree_path(attempt_id, &fresh).await?;
            fresh
        };

        // Load agent settings and project settings
        let agent_settings = self.load_agent_settings(attempt_id).await?;
        let task = self.fetch_task(task_id).await?;
        let project_settings = self
            .fetch_project_settings(task.project_id)
            .await
            .unwrap_or_default();
        let require_review = task
            .metadata
            .get("execution")
            .and_then(|v| v.get("require_review"))
            .and_then(|v| v.as_bool())
            .or_else(|| {
                task.metadata
                    .get("require_review")
                    .and_then(|v| v.as_bool())
            })
            .unwrap_or(project_settings.require_review);
        let task_timeout = Duration::from_secs(project_settings.timeout_mins as u64 * 60);

        // Resume uses the currently selected provider from system settings.
        let (provider, provider_env) = self.resolve_agent_cli(attempt_id).await?;
        self.set_attempt_executor(attempt_id, provider).await?;
        // Live input queue for this resumed session.
        let (session_input_sender, provider_input_rx) = mpsc::unbounded_channel::<String>();
        let (claude_input_rx, mut stdio_input_rx) =
            if matches!(provider, AgentCliProvider::ClaudeCode) {
                (Some(provider_input_rx), None)
            } else {
                (None, Some(provider_input_rx))
            };

        // Mark running for the follow-up run
        self.update_status(attempt_id, AttemptStatus::Running)
            .await?;

        let spawned = match provider {
            AgentCliProvider::ClaudeCode => {
                // Use SDK mode (stream-json) for real-time log streaming
                // Print mode buffers all output until completion, causing apparent hangs
                self.claude_client
                    .spawn_session_sdk(
                        &effective_path,
                        instruction,
                        attempt_id,
                        provider_env,
                        Some(self.approval_service.clone()),
                        Some(self.db_pool.clone()),
                        Some(self.broadcast_tx.clone()),
                        Some(&agent_settings),
                        claude_input_rx,
                        Some(ClaudeRuntimeSkillConfig {
                            repo_path: effective_path.clone(),
                            skill_knowledge: self.skill_knowledge.clone(),
                        }),
                    )
                    .await
                    .context("Failed to spawn Claude agent for resume")?
            }
            AgentCliProvider::OpenAiCodex => self
                .codex_client
                .spawn_session(&effective_path, instruction, attempt_id, provider_env)
                .await
                .context("Failed to spawn Codex agent for resume")?,
            AgentCliProvider::GeminiCli => self
                .gemini_client
                .spawn_session(&effective_path, instruction, attempt_id, provider_env)
                .await
                .context("Failed to spawn Gemini agent for resume")?,
            AgentCliProvider::CursorCli => self
                .cursor_client
                .spawn_session(&effective_path, instruction, attempt_id, provider_env)
                .await
                .context("Failed to spawn Cursor agent for resume")?,
        };

        let SpawnedAgent {
            child,
            interrupt_sender,
            interrupt_receiver,
            msg_store,
        } = spawned;

        let _ = msg_store;

        // Store child in Arc for cleanup guard access
        let child_arc = Arc::new(Mutex::new(Some(child)));

        // Store active session for runtime input and termination control.
        {
            let session = ActiveSession {
                interrupt_sender,
                child: child_arc.clone(),
                input_sender: Some(session_input_sender),
            };
            self.active_sessions
                .lock()
                .await
                .insert(attempt_id, session);
        }

        let _cleanup_guard = scopeguard::guard(child_arc.clone(), {
            let child_clone = child_arc.clone();
            let sessions = self.active_sessions.clone();
            move |_| {
                tokio::spawn(async move {
                    sessions.lock().await.remove(&attempt_id);
                    if let Some(mut c) = child_clone.lock().await.take() {
                        debug!("Resume cleanup guard triggered, killing orphan process");
                        let _ = kill_process_group(&mut c).await;
                    }
                });
            }
        });

        // Wait for process completion with timeout
        let mut child_opt = child_arc.lock().await.take();
        let child_ref = child_opt
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Child process not available"))?;

        self.attach_execution_process_pid(attempt_id, child_ref)
            .await;

        // For non-SDK providers, drain queued live input into child stdin.
        if !matches!(provider, AgentCliProvider::ClaudeCode) {
            if let Some(mut rx) = stdio_input_rx.take() {
                if let Some(mut stdin) = child_ref.inner().stdin.take() {
                    let pool = self.db_pool.clone();
                    let tx = self.broadcast_tx.clone();
                    tokio::spawn(async move {
                        while let Some(message) = rx.recv().await {
                            let trimmed = message.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            let to_send = crate::follow_up_utils::wrap_trivial_follow_up(trimmed);
                            let line = format!("{}\n", to_send);
                            if let Err(e) = stdin.write_all(line.as_bytes()).await {
                                let _ = StatusManager::log(
                                    &pool,
                                    &tx,
                                    attempt_id,
                                    "stderr",
                                    &format!("Failed to forward live input to stdin: {}", e),
                                )
                                .await;
                                break;
                            }
                            if let Err(e) = stdin.flush().await {
                                let _ = StatusManager::log(
                                    &pool,
                                    &tx,
                                    attempt_id,
                                    "stderr",
                                    &format!("Failed to flush live input to stdin: {}", e),
                                )
                                .await;
                                break;
                            }
                        }
                    });
                }
            }
        }

        let db_pool = self.db_pool.clone();
        let tx = self.broadcast_tx.clone();

        let wait_future = async {
            // Claude SDK mode logs are handled by ProtocolPeer; other providers stream logs here.
            if !matches!(provider, AgentCliProvider::ClaudeCode) {
                match provider {
                    AgentCliProvider::OpenAiCodex => {
                        self.stream_codex_json_with_interrupt(
                            child_ref,
                            attempt_id,
                            &effective_path,
                            interrupt_receiver,
                        )
                        .await?;
                    }
                    AgentCliProvider::GeminiCli => {
                        ClaudeClient::stream_logs_with_interrupt(
                            child_ref,
                            interrupt_receiver,
                            move |line, is_stderr| {
                                let pool = db_pool.clone();
                                let tx = tx.clone();
                                let role = if is_stderr { "stderr" } else { "stdout" };
                                if should_skip_log_line(&line) {
                                    return;
                                }
                                let log_content = sanitize_log(&line);
                                tokio::spawn(async move {
                                    let _ = StatusManager::log(
                                        &pool,
                                        &tx,
                                        attempt_id,
                                        role,
                                        &log_content,
                                    )
                                    .await;
                                });
                            },
                        )
                        .await?;
                    }
                    AgentCliProvider::CursorCli => {
                        self.stream_cursor_json_with_interrupt(
                            child_ref,
                            attempt_id,
                            &effective_path,
                            interrupt_receiver,
                        )
                        .await?;
                    }
                    AgentCliProvider::ClaudeCode => unreachable!(),
                }
            }

            Ok::<_, anyhow::Error>(child_ref.wait().await?)
        };

        let result = tokio::time::timeout(task_timeout, wait_future).await;

        let status = match result {
            Ok(Ok(status)) => {
                if let Some(ref mut c) = child_opt {
                    let _ = kill_process_group(c).await;
                }
                status
            }
            Ok(Err(e)) => {
                if let Some(ref mut c) = child_opt {
                    let _ = kill_process_group(c).await;
                }
                let error_msg = format!("Follow-up execution failed: {}", e);
                self.fail_attempt_with_retry(attempt_id, task_id, &error_msg)
                    .await?;
                let _ = self.cleanup_attempt_worktree(repo_path, attempt_id).await;
                return Err(anyhow::anyhow!("Failed to wait for agent: {}", e));
            }
            Err(_) => {
                self.log(
                    attempt_id,
                    "system",
                    &format!(
                        "Follow-up timed out after {} mins, terminating agent...",
                        project_settings.timeout_mins
                    ),
                )
                .await?;
                if let Some(ref mut c) = child_opt {
                    let _ = terminate_process(c, None, GRACEFUL_SHUTDOWN_TIMEOUT).await;
                }
                self.fail_attempt_with_retry(attempt_id, task_id, "Follow-up execution timed out")
                    .await?;
                let _ = self.cleanup_attempt_worktree(repo_path, attempt_id).await;
                bail!("Follow-up execution timed out after {:?}", task_timeout);
            }
        };

        if status.success() {
            if let Err(err) = self
                .persist_structured_outputs_from_attempt_logs(
                    attempt_id,
                    Some(effective_path.as_path()),
                )
                .await
            {
                warn!(
                    "Failed to persist structured outputs for follow-up attempt {}: {}",
                    attempt_id, err
                );
                self.log(
                    attempt_id,
                    "stderr",
                    &format!(
                        "Warning: failed to persist structured deployment/report outputs: {}",
                        err
                    ),
                )
                .await?;
            }

            let branch_ready = if require_review {
                true
            } else {
                match self
                    .finalize_branch_for_no_review(attempt_id, &effective_path)
                    .await
                {
                    Ok(_) => true,
                    Err(e) => {
                        self.log(
                            attempt_id,
                            "stderr",
                            &format!(
                                "Failed to finalize branch commit/push after follow-up: {}",
                                e
                            ),
                        )
                        .await?;
                        false
                    }
                }
            };

            // Save file diffs to S3 from final branch state.
            info!(
                "📸 [DIFF CAPTURE TRIGGER] About to save diffs for attempt {} (resume mode)",
                attempt_id
            );
            if let Err(e) = self.save_diffs_to_s3(attempt_id, &effective_path).await {
                warn!(
                    "📸 [DIFF CAPTURE ERROR] Failed to save diffs for attempt {}: {}",
                    attempt_id, e
                );
                // Don't fail the attempt if diff capture fails
            }

            if let Err(e) = self.run_before_success_hook(attempt_id).await {
                tracing::error!(
                    attempt_id = %attempt_id,
                    error = %e,
                    "Deployment hook failed (Cloudflare/preview)"
                );
                let user_msg = Self::format_deployment_hook_failure_user_message(&e);
                self.log(attempt_id, "system", &user_msg).await?;
                // Do NOT fail attempt: Cloudflare/deployment errors are not attempt failures
            }

            self.update_status(attempt_id, AttemptStatus::Success)
                .await?;
            if require_review {
                self.mark_task_in_review(task_id).await?;
                // Auto-retry merge after Request Changes (e.g. conflict resolution). Agent may have pushed.
                match self.handle_gitops_merge(attempt_id).await {
                    Ok(true) => {
                        self.mark_task_completed(task_id).await?;
                        self.log(
                            attempt_id,
                            "system",
                            "Follow-up completed. MR merged successfully. Cleaning up worktree...",
                        )
                        .await?;
                        if let Err(cleanup_err) =
                            self.cleanup_attempt_worktree(repo_path, attempt_id).await
                        {
                            self.log(
                                attempt_id,
                                "stderr",
                                &format!("Worktree cleanup failed: {}", cleanup_err),
                            )
                            .await?;
                        }
                        if let Err(report_err) = self.emit_completion_report(attempt_id).await {
                            self.log(
                                attempt_id,
                                "stderr",
                                &format!("Failed to emit final report: {}", report_err),
                            )
                            .await?;
                        }
                    }
                    Ok(false) | Err(_) => {
                        self.log(
                            attempt_id,
                            "system",
                            "MR merge not completed. Please approve again to retry merge.",
                        )
                        .await?;
                    }
                }
            } else if !branch_ready {
                self.mark_task_in_review(task_id).await?;
                self.log(
                    attempt_id,
                    "system",
                    "Follow-up moved to review. Repository sync hit a recoverable Git issue; the branch and worktree were preserved so you can send another follow-up or approve after fixing repository state.",
                )
                .await?;
            } else {
                self.log(
                    attempt_id,
                    "system",
                    "Follow-up completed. Starting GitOps sync...",
                )
                .await?;

                let auto_merged = match self.handle_gitops(attempt_id).await {
                    Ok(_) => match self.handle_gitops_merge(attempt_id).await {
                        Ok(merged) => merged,
                        Err(e) => {
                            self.log(
                                attempt_id,
                                "stderr",
                                &format!("Auto-merge failed after follow-up: {}", e),
                            )
                            .await?;
                            false
                        }
                    },
                    Err(e) => {
                        self.log(
                            attempt_id,
                            "stderr",
                            &format!("GitOps failed after follow-up: {}", e),
                        )
                        .await?;
                        false
                    }
                };

                if auto_merged {
                    self.mark_task_completed(task_id).await?;
                    self.log(
                        attempt_id,
                        "system",
                        "Follow-up completed. Cleaning up worktree...",
                    )
                    .await?;
                    if let Err(cleanup_err) =
                        self.cleanup_attempt_worktree(repo_path, attempt_id).await
                    {
                        self.log(
                            attempt_id,
                            "stderr",
                            &format!("Worktree cleanup failed after follow-up: {}", cleanup_err),
                        )
                        .await?;
                    }
                    if let Err(report_err) = self.emit_completion_report(attempt_id).await {
                        self.log(
                            attempt_id,
                            "stderr",
                            &format!("Failed to emit final follow-up report: {}", report_err),
                        )
                        .await?;
                    }
                } else {
                    self.mark_task_in_review(task_id).await?;
                    self.log(
                        attempt_id,
                        "system",
                        "Follow-up auto-merge was not completed. The branch and worktree were preserved, so you can approve for manual merge or send another follow-up to continue.",
                    )
                    .await?;
                }
            }
        } else {
            let error_msg = format!("Follow-up agent exited with status: {}", status);
            self.fail_attempt_with_retry(attempt_id, task_id, &error_msg)
                .await?;
            let _ = self.cleanup_attempt_worktree(repo_path, attempt_id).await;
            bail!("{}", error_msg);
        }

        Ok(())
    }
}

/// Extract repository URL from agent stdout.
///
/// ## Expected Format
/// Agent should output a line like: `REPO_URL: https://gitlab.com/user/repo`
///
/// ## Returns
/// - `Some(url)` if found
/// - `None` if not found
fn extract_repo_url(lines: &[String]) -> Option<String> {
    for line in lines {
        // Some timeline rows are persisted as normalized JSON entries:
        // {"entry_type": {...}, "content":"...REPO_URL: ..."}
        // Parse and inspect the `content` field as a fallback.
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(content) = parsed.get("content").and_then(|v| v.as_str()) {
                if let Some(url) = extract_repo_url_from_text(content) {
                    return Some(url);
                }
            }
        }

        // Raw text logs (process_stdout/system/etc)
        if let Some(url) = extract_repo_url_from_text(line) {
            return Some(url);
        }
    }
    None
}

fn extract_repo_url_from_text(text: &str) -> Option<String> {
    // Primary form: REPO_URL: https://... or REPO_URL = ... or REPO_URL | ... (table)
    let labeled_regex =
        Regex::new(r#"(?i)\brepo_url\b\s*[:=\s|]+\s*(https?://[^\s\]>}"'`|]+)"#).ok()?;
    if let Some(caps) = labeled_regex.captures(text) {
        if let Some(url) = caps.get(1) {
            let candidate = trim_repo_url_candidate(url.as_str());
            if parse_repo_host_and_path_for_extraction(&candidate).is_some() {
                return Some(candidate);
            }
        }
    }

    // Fallback: when "repo_url" appears, collect all URLs and pick the first with valid host+path
    // (avoids picking GITLAB_URL or other host-only URLs that appear earlier in the text)
    if text.to_ascii_lowercase().contains("repo_url") {
        let url_regex = Regex::new(r#"https?://[^\s\]>}"'`|]+"#).ok()?;
        for url_match in url_regex.find_iter(text) {
            let candidate = trim_repo_url_candidate(url_match.as_str());
            if let Some((_, path)) = parse_repo_host_and_path_for_extraction(&candidate) {
                if !path.is_empty() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

/// Same as init_flow::parse_repo_host_and_path but accessible from orchestrator for extraction.
fn parse_repo_host_and_path_for_extraction(repo_url: &str) -> Option<(String, String)> {
    let trimmed = repo_url.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
    {
        let without_auth = rest.rsplit('@').next().unwrap_or(rest);
        let (host, path) = without_auth.split_once('/')?;
        let host = host.trim().to_ascii_lowercase();
        let path = path.trim().trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path).to_string();
        if host.is_empty() || path.is_empty() {
            return None;
        }
        return Some((host, path));
    }
    if let Some((left, right)) = trimmed.split_once(':') {
        if let Some(host) = left.split('@').nth(1) {
            let host = host.trim().to_ascii_lowercase();
            let path = right.trim().trim_matches('/');
            let path = path.strip_suffix(".git").unwrap_or(path).to_string();
            if host.is_empty() || path.is_empty() {
                return None;
            }
            return Some((host, path));
        }
    }
    None
}

fn extract_preview_target(lines: &[String]) -> Option<String> {
    for line in lines {
        // Handle normalized JSON log rows:
        // {"entry_type": {...}, "content":"...PREVIEW_TARGET: ..."}
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(content) = parsed.get("content").and_then(|v| v.as_str()) {
                if let Some(target) = extract_preview_target_from_text(content) {
                    return Some(target);
                }
            }
        }

        if let Some(target) = extract_preview_target_from_text(line) {
            return Some(target);
        }
    }
    None
}

fn extract_preview_target_from_text(text: &str) -> Option<String> {
    // Supported forms:
    // PREVIEW_TARGET: http://127.0.0.1:5173
    // PREVIEW_TARGET = http://localhost:3000
    let labeled_regex = Regex::new(r#"(?i)\bpreview_target\b\s*[:=]\s*(https?://\S+)"#).ok()?;
    if let Some(caps) = labeled_regex.captures(text) {
        if let Some(value) = caps.get(1) {
            let candidate = trim_repo_url_candidate(value.as_str());
            if is_placeholder_preview_target(&candidate) {
                return None;
            }
            return Some(candidate);
        }
    }

    None
}

fn is_placeholder_preview_target(candidate: &str) -> bool {
    candidate.contains('<')
        || candidate.contains('>')
        || candidate.contains('{')
        || candidate.contains('}')
}

fn extract_preview_url(lines: &[String]) -> Option<String> {
    for line in lines {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(content) = parsed.get("content").and_then(|v| v.as_str()) {
                if let Some(url) = extract_preview_url_from_text(content) {
                    return Some(url);
                }
            }
        }

        if let Some(url) = extract_preview_url_from_text(line) {
            return Some(url);
        }
    }
    None
}

fn extract_preview_url_from_text(text: &str) -> Option<String> {
    // Supported forms:
    // PREVIEW_URL: https://task-abcd.preview.example.com
    // PREVIEW_URL = https://xxxx.trycloudflare.com
    let labeled_regex = Regex::new(r#"(?i)\bpreview_url\b\s*[:=]\s*(https?://\S+)"#).ok()?;
    if let Some(caps) = labeled_regex.captures(text) {
        if let Some(value) = caps.get(1) {
            return Some(trim_repo_url_candidate(value.as_str()));
        }
    }
    None
}

fn extract_labeled_value(lines: &[String], label: &str) -> Option<String> {
    for line in lines {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(content) = parsed.get("content").and_then(|v| v.as_str()) {
                if let Some(value) = extract_labeled_value_from_text(content, label) {
                    return Some(value);
                }
            }
        }

        if let Some(value) = extract_labeled_value_from_text(line, label) {
            return Some(value);
        }
    }

    None
}

fn extract_labeled_value_from_text(text: &str, label: &str) -> Option<String> {
    // Supported forms:
    // deployment_status: active
    // deployment_status = active
    // - deployment_status: active
    // `deployment_status`: active
    let pattern = format!(
        r#"(?im)^\s*(?:[-*]\s*)?(?:`)?{}\b(?:`)?\s*[:=]\s*(.+?)\s*$"#,
        regex::escape(label)
    );
    let labeled_regex = Regex::new(&pattern).ok()?;
    let captures = labeled_regex.captures(text)?;
    let value = captures.get(1)?.as_str();
    let trimmed = trim_labeled_value_candidate(value);
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed)
}

fn extract_mr_title(lines: &[String]) -> Option<String> {
    extract_labeled_value(lines, "mr_title").or_else(|| extract_labeled_value(lines, "MR_TITLE"))
}

fn extract_mr_description(lines: &[String]) -> Option<String> {
    extract_labeled_value(lines, "mr_description")
        .or_else(|| extract_labeled_value(lines, "MR_DESCRIPTION"))
}

fn extract_deployment_report(lines: &[String]) -> Option<serde_json::Value> {
    let mut report = serde_json::Map::new();
    let fields = [
        "deploy_precheck",
        "deploy_precheck_reason",
        "deployment_status",
        "deployment_error",
        "deployment_kind",
        "production_deployment_status",
        "production_deployment_error",
        "production_deployment_url",
        "production_deployment_type",
        "production_deployment_id",
        "smoke_status",
        "rollback_recommended",
        "delivery_status",
    ];

    for field in fields {
        if let Some(value) = extract_labeled_value(lines, field) {
            report.insert(field.to_string(), serde_json::Value::String(value));
        }
    }

    if report.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(report))
    }
}

fn trim_labeled_value_candidate(value: &str) -> String {
    let trimmed = value.trim();

    let unwrapped = if (trimmed.starts_with('`') && trimmed.ends_with('`'))
        || (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed[1..trimmed.len().saturating_sub(1)].trim()
    } else {
        trimmed
    };

    unwrapped
        .trim_end_matches([',', ';', ')', ']', '}', '"', '\'', '`'])
        .trim()
        .to_string()
}

fn trim_repo_url_candidate(url: &str) -> String {
    let mut candidate = url;
    if let Some(idx) = candidate.find(|c: char| {
        c.is_whitespace() || c == '`' || c == '"' || c == '\'' || c == '*' || c == '|'
    }) {
        candidate = &candidate[..idx];
    }
    let lower_candidate = candidate.to_ascii_lowercase();
    let truncation_markers = [
        "preview_url:",
        "preview_target:",
        "**summary:**",
        "summary:",
        "what was done:",
        "what was built:",
        "deploy_precheck:",
        "deployment_failure_reason:",
    ];
    let marker_index = truncation_markers
        .iter()
        .filter_map(|marker| lower_candidate.find(marker))
        .filter(|idx| *idx > 0)
        .min();
    if let Some(idx) = marker_index {
        candidate = &candidate[..idx];
    }
    candidate
        .trim_end_matches(['.', ',', ';', ':', ')', ']', '>', '}', '\n', '\r'])
        .trim()
        .to_string()
}

fn normalize_repo_url(url: &str) -> String {
    let trimmed = trim_repo_url_candidate(url);
    for scheme in ["https://", "http://"] {
        if let Some(rest) = trimmed.strip_prefix(scheme) {
            let slash_index = rest.find('/').unwrap_or(rest.len());
            let (authority, path) = rest.split_at(slash_index);
            // Never persist embedded credentials in repository URL.
            let sanitized_authority = authority.rsplit('@').next().unwrap_or(authority);
            let normalized = format!("{}{}{}", scheme, sanitized_authority, path);
            return normalized.trim_end_matches('/').to_string();
        }
    }
    trimmed
}

#[derive(sqlx::FromRow)]
struct ProjectInfo {
    repository_url: Option<String>,
    repository_context: serde_json::Value,
    pat_encrypted: Option<String>,
    #[allow(dead_code)]
    gitlab_project_id: Option<i64>,
    #[allow(dead_code)]
    base_url: Option<String>,
}

impl ProjectInfo {
    fn repository_context(&self) -> RepositoryContext {
        serde_json::from_value(self.repository_context.clone()).unwrap_or_default()
    }
}

fn repository_origin_url(info: &ProjectInfo) -> Option<&str> {
    info.repository_url
        .as_deref()
        .filter(|value| !value.trim().is_empty())
}

fn repository_upstream_url(info: &ProjectInfo) -> Option<String> {
    let context = info.repository_context();
    let origin = repository_origin_url(info);
    context
        .upstream_repository_url
        .filter(|value| !value.trim().is_empty())
        .filter(|upstream| {
            origin
                .map(|origin| !repo_url_matches(upstream, origin))
                .unwrap_or(true)
        })
}

fn repository_base_ref_override(info: &ProjectInfo) -> Option<String> {
    let context = info.repository_context();
    let default_branch = context
        .default_branch
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    let remote = if context.access_mode == RepositoryAccessMode::ForkGitops
        && context
            .upstream_repository_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .is_some()
    {
        "upstream"
    } else {
        "origin"
    };

    Some(format!("{}/{}", remote, default_branch))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk_normalized_types::ActionType;
    use std::sync::{LazyLock, Mutex};

    static ENV_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    struct EnvVarGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: String) -> Self {
            let original = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(original) = &self.original {
                std::env::set_var(self.key, original);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn resolve_provider_command_env_returns_none_for_claude() {
        let env = ExecutorOrchestrator::resolve_provider_command_env(AgentCliProvider::ClaudeCode)
            .expect("claude provider should not fail");
        assert!(env.is_none(), "claude provider should not inject exec env");
    }

    #[test]
    fn detects_claude_sdk_turn_completion_from_end_turn_message_stop() {
        let line = r#"{"type":"message_stop","message":{"stop_reason":"end_turn"}}"#;
        assert!(ExecutorOrchestrator::is_claude_sdk_turn_complete_line(line));
    }

    #[test]
    fn does_not_treat_tool_use_message_stop_as_completion() {
        let line = r#"{"type":"message_stop","message":{"stop_reason":"tool_use"}}"#;
        assert!(!ExecutorOrchestrator::is_claude_sdk_turn_complete_line(
            line
        ));
    }

    #[test]
    fn resolve_provider_command_env_codex_uses_override_binary() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock poisoned");
        let current_exe = std::env::current_exe().expect("failed to read current executable path");
        let current_exe = current_exe.to_string_lossy().to_string();
        let _override_guard = EnvVarGuard::set(OVERRIDE_CODEX_BIN_ENV, current_exe.clone());

        let env = ExecutorOrchestrator::resolve_provider_command_env(AgentCliProvider::OpenAiCodex)
            .expect("codex provider should resolve")
            .expect("codex provider should inject exec env");

        assert_eq!(env.get(EXEC_CODEX_CMD_ENV), Some(&current_exe));
        assert!(
            !env.contains_key(EXEC_CODEX_USE_NPX_ENV),
            "override binary path should disable npx fallback mode"
        );
    }

    #[test]
    fn resolve_provider_command_env_gemini_uses_override_binary() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock poisoned");
        let current_exe = std::env::current_exe().expect("failed to read current executable path");
        let current_exe = current_exe.to_string_lossy().to_string();
        let _override_guard = EnvVarGuard::set(OVERRIDE_GEMINI_BIN_ENV, current_exe.clone());

        let env = ExecutorOrchestrator::resolve_provider_command_env(AgentCliProvider::GeminiCli)
            .expect("gemini provider should resolve")
            .expect("gemini provider should inject exec env");

        assert_eq!(env.get(EXEC_GEMINI_CMD_ENV), Some(&current_exe));
        assert!(
            !env.contains_key(EXEC_GEMINI_USE_NPX_ENV),
            "override binary path should disable npx fallback mode"
        );
    }

    #[test]
    fn resolve_provider_command_env_cursor_fails_when_override_is_missing_binary() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock poisoned");
        let missing = "/tmp/acpms-missing-cursor-bin-for-test";
        let _override_guard = EnvVarGuard::set(OVERRIDE_CURSOR_BIN_ENV, missing.to_string());

        let err = ExecutorOrchestrator::resolve_provider_command_env(AgentCliProvider::CursorCli)
            .expect_err("cursor provider should fail when override binary is missing");

        assert!(
            err.to_string().contains("Cursor CLI not found"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn agent_cli_provider_fallback_order_keeps_selected_first() {
        let ordered = AgentCliProvider::fallback_order(AgentCliProvider::GeminiCli);
        assert_eq!(
            ordered,
            vec![
                AgentCliProvider::GeminiCli,
                AgentCliProvider::ClaudeCode,
                AgentCliProvider::OpenAiCodex,
                AgentCliProvider::CursorCli
            ]
        );
    }

    #[test]
    fn agent_cli_provider_fallback_order_contains_all_providers_once() {
        let ordered = AgentCliProvider::fallback_order(AgentCliProvider::CursorCli);
        assert_eq!(ordered.len(), AgentCliProvider::ALL.len());
        for provider in AgentCliProvider::ALL {
            assert!(
                ordered.contains(&provider),
                "fallback order should include provider {}",
                provider.as_str()
            );
        }
    }

    #[test]
    fn is_immediate_duplicate_assistant_text_ignores_whitespace_differences() {
        assert!(is_immediate_duplicate_assistant_text(
            Some("Hello   world"),
            "  Hello world  "
        ));
    }

    #[test]
    fn is_immediate_duplicate_assistant_text_detects_distinct_messages() {
        assert!(!is_immediate_duplicate_assistant_text(
            Some("Hello world"),
            "Hello world again"
        ));
    }

    #[test]
    fn normalize_assistant_plain_stdout_line_ignores_json_lines() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#;
        assert!(
            normalize_assistant_plain_stdout_line(line).is_none(),
            "JSON lines should be ignored by plain-text fallback"
        );
    }

    #[test]
    fn normalize_assistant_plain_stdout_line_keeps_plain_text() {
        let line = "Open this URL to authenticate.";
        let normalized =
            normalize_assistant_plain_stdout_line(line).expect("expected plain-text fallback");
        assert_eq!(normalized, line);
    }

    #[test]
    fn detect_provider_auth_blocker_matches_gemini_auth_prompts() {
        let hint = detect_provider_auth_blocker(
            AgentCliProvider::GeminiCli,
            "Enter the authorization code:",
        );
        assert!(
            hint.is_some(),
            "gemini auth prompt should map to a remediation hint"
        );
    }

    #[test]
    fn detect_provider_auth_blocker_matches_cursor_auth_prompts() {
        let hint = detect_provider_auth_blocker(
            AgentCliProvider::CursorCli,
            "Not logged in. Please run agent login",
        );
        assert!(
            hint.is_some(),
            "cursor auth prompt should map to a remediation hint"
        );
    }

    #[test]
    fn classify_successful_shell_command_maps_search_tools() {
        let classified = classify_successful_shell_command("/bin/zsh -lc 'rg \"TODO\" src'");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for ripgrep");
        };

        assert_eq!(tool_name, "Grep");
        match action_type {
            ActionType::Search { query } => assert_eq!(query, "TODO"),
            other => panic!("expected search action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_maps_file_read_tools() {
        let classified = classify_successful_shell_command("cat src/main.rs");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for cat");
        };

        assert_eq!(tool_name, "Read");
        match action_type {
            ActionType::FileRead { path } => assert_eq!(path, "src/main.rs"),
            other => panic!("expected file_read action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_maps_sed_to_file_read() {
        let classified = classify_successful_shell_command("sed -n '1,40p' src/main.rs");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for sed");
        };

        assert_eq!(tool_name, "Read");
        match action_type {
            ActionType::FileRead { path } => assert_eq!(path, "src/main.rs"),
            other => panic!("expected file_read action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_maps_sed_to_file_read_with_pipeline_suffix() {
        let classified =
            classify_successful_shell_command("sed -n '1,40p' src/main.rs | head -n 5");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for piped sed");
        };

        assert_eq!(tool_name, "Read");
        match action_type {
            ActionType::FileRead { path } => assert_eq!(path, "src/main.rs"),
            other => panic!("expected file_read action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_maps_awk_to_file_read_when_file_is_present() {
        let classified = classify_successful_shell_command("awk 'NR==1{print $1}' src/main.rs");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for awk");
        };

        assert_eq!(tool_name, "Read");
        match action_type {
            ActionType::FileRead { path } => assert_eq!(path, "src/main.rs"),
            other => panic!("expected file_read action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_maps_awk_to_file_read_with_redirection_suffix() {
        let classified =
            classify_successful_shell_command("awk 'NR==1{print $1}' src/main.rs > /tmp/out.txt");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for redirected awk");
        };

        assert_eq!(tool_name, "Read");
        match action_type {
            ActionType::FileRead { path } => assert_eq!(path, "src/main.rs"),
            other => panic!("expected file_read action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_maps_fd_to_search() {
        let classified = classify_successful_shell_command("fd TODO src");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for fd");
        };

        assert_eq!(tool_name, "Grep");
        match action_type {
            ActionType::Search { query } => assert_eq!(query, "TODO"),
            other => panic!("expected search action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_maps_fdfind_alias_to_search() {
        let classified = classify_successful_shell_command("fdfind TODO src");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for fdfind");
        };

        assert_eq!(tool_name, "Grep");
        match action_type {
            ActionType::Search { query } => assert_eq!(query, "TODO"),
            other => panic!("expected search action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_maps_find_to_search_with_full_query() {
        let classified = classify_successful_shell_command("find src -name '*.rs'");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for find");
        };

        assert_eq!(tool_name, "Grep");
        match action_type {
            ActionType::Search { query } => assert!(query.starts_with("find src -name")),
            other => panic!("expected search action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_maps_ls_to_search_with_target_directory() {
        let classified = classify_successful_shell_command("ls -la src/components");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for ls");
        };

        assert_eq!(tool_name, "Grep");
        match action_type {
            ActionType::Search { query } => assert_eq!(query, "src/components"),
            other => panic!("expected search action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_maps_tree_with_pipeline_suffix() {
        let classified = classify_successful_shell_command("tree -a | head -n 20");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for tree");
        };

        assert_eq!(tool_name, "Grep");
        match action_type {
            ActionType::Search { query } => assert!(query.starts_with("tree -a")),
            other => panic!("expected search action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_maps_web_fetch_tools() {
        let classified = classify_successful_shell_command("curl -s https://example.com/data.json");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for curl");
        };

        assert_eq!(tool_name, "WebFetch");
        match action_type {
            ActionType::WebFetch { url } => assert_eq!(url, "https://example.com/data.json"),
            other => panic!("expected web_fetch action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_maps_wget_to_web_fetch() {
        let classified =
            classify_successful_shell_command("wget https://example.com/archive.tar.gz");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for wget");
        };

        assert_eq!(tool_name, "WebFetch");
        match action_type {
            ActionType::WebFetch { url } => assert_eq!(url, "https://example.com/archive.tar.gz"),
            other => panic!("expected web_fetch action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_maps_curl_with_redirection_suffix() {
        let classified = classify_successful_shell_command(
            "curl -s https://example.com/api.json > /tmp/api.json",
        );
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for redirected curl");
        };

        assert_eq!(tool_name, "WebFetch");
        match action_type {
            ActionType::WebFetch { url } => assert_eq!(url, "https://example.com/api.json"),
            other => panic!("expected web_fetch action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_handles_sudo_prefix_for_read_tools() {
        let classified = classify_successful_shell_command("sudo cat /etc/hosts");
        let Some((tool_name, action_type)) = classified else {
            panic!("expected command classification for sudo cat");
        };

        assert_eq!(tool_name, "Read");
        match action_type {
            ActionType::FileRead { path } => assert_eq!(path, "/etc/hosts"),
            other => panic!("expected file_read action, got {:?}", other),
        }
    }

    #[test]
    fn classify_successful_shell_command_returns_none_for_unknown_commands() {
        let classified = classify_successful_shell_command("pnpm test --filter web");
        assert!(classified.is_none());
    }

    #[test]
    fn classify_successful_shell_command_returns_none_for_read_without_target() {
        let classified = classify_successful_shell_command("cat");
        assert!(classified.is_none());
    }

    #[test]
    fn classify_successful_shell_command_returns_none_for_sed_without_target_file() {
        let classified = classify_successful_shell_command("sed -n '1,40p'");
        assert!(classified.is_none());
    }

    #[test]
    fn classify_successful_shell_command_returns_none_for_sed_numeric_script_without_file() {
        let classified = classify_successful_shell_command("sed -n 1,40p");
        assert!(classified.is_none());
    }

    #[test]
    fn classify_successful_shell_command_returns_none_for_awk_without_target_file() {
        let classified = classify_successful_shell_command("awk 'NR==1{print $1}'");
        assert!(classified.is_none());
    }

    #[test]
    fn classify_successful_shell_command_returns_none_for_web_fetch_without_url() {
        let classified = classify_successful_shell_command("curl -sS");
        assert!(classified.is_none());
    }

    #[test]
    fn test_sanitize_log_redacts_gitlab_pat() {
        let input = "Using GitLab PAT: glpat-1234567890abcdefghij";
        let output = sanitize_log(input);
        assert_eq!(output, "Using GitLab PAT: ***GITLAB_PAT_REDACTED***");
    }

    #[test]
    fn test_sanitize_log_redacts_multiple_pats() {
        let input = "PAT1: glpat-aaaaaaaaaaaaaaaaaaaaa and PAT2: glpat-bbbbbbbbbbbbbbbbbbbb";
        let output = sanitize_log(input);
        assert_eq!(
            output,
            "PAT1: ***GITLAB_PAT_REDACTED*** and PAT2: ***GITLAB_PAT_REDACTED***"
        );
    }

    #[test]
    fn test_sanitize_log_leaves_normal_text() {
        let input = "Normal log message without secrets";
        let output = sanitize_log(input);
        assert_eq!(output, "Normal log message without secrets");
    }

    #[test]
    fn test_sanitize_log_handles_long_pats() {
        let input = "Token: glpat-1234567890abcdefghijklmnopqrstuvwxyz";
        let output = sanitize_log(input);
        assert_eq!(output, "Token: ***GITLAB_PAT_REDACTED***");
    }

    #[test]
    fn test_extract_repo_url() {
        let lines = vec![
            "Creating repository...".to_string(),
            "REPO_URL: https://gitlab.com/user/repo".to_string(),
            "Done".to_string(),
        ];
        let url = extract_repo_url(&lines);
        assert_eq!(url, Some("https://gitlab.com/user/repo".to_string()));
    }

    #[test]
    fn test_extract_repo_url_not_found() {
        let lines = vec!["Creating repository...".to_string(), "Done".to_string()];
        let url = extract_repo_url(&lines);
        assert_eq!(url, None);
    }

    #[test]
    fn test_extract_repo_url_in_multiline_assistant_message() {
        let lines = vec![r#"Key decisions
- Use Vite
- Use ESLint
REPO_URL: https://gitlab.com/user/repo"#
            .to_string()];
        let url = extract_repo_url(&lines);
        assert_eq!(url, Some("https://gitlab.com/user/repo".to_string()));
    }

    #[test]
    fn test_extract_repo_url_from_normalized_json_content() {
        let lines = vec![
            r#"{"timestamp":"2026-02-08T00:00:00Z","entry_type":{"type":"assistant_message"},"content":"Summary\nREPO_URL: https://gitlab.com/user/repo"}"#.to_string(),
        ];
        let url = extract_repo_url(&lines);
        assert_eq!(url, Some("https://gitlab.com/user/repo".to_string()));
    }

    #[test]
    fn test_extract_repo_url_prefers_full_url_over_host_only() {
        let lines = vec![
            "GITLAB_URL: https://gitlab.example.com\nREPO_URL: https://gitlab.example.com/org/repo"
                .to_string(),
        ];
        let url = extract_repo_url(&lines);
        assert_eq!(url, Some("https://gitlab.example.com/org/repo".to_string()));
    }

    #[test]
    fn test_extract_preview_target_raw_line() {
        let lines = vec![
            "Some log".to_string(),
            "PREVIEW_TARGET: http://127.0.0.1:5173".to_string(),
        ];
        let target = extract_preview_target(&lines);
        assert_eq!(target, Some("http://127.0.0.1:5173".to_string()));
    }

    #[test]
    fn test_extract_preview_target_from_normalized_json_content() {
        let lines = vec![
            r#"{"timestamp":"2026-02-08T00:00:00Z","entry_type":{"type":"assistant_message"},"content":"Deploy summary\nPREVIEW_TARGET: http://localhost:8080"}"#.to_string(),
        ];
        let target = extract_preview_target(&lines);
        assert_eq!(target, Some("http://localhost:8080".to_string()));
    }

    #[test]
    fn test_extract_preview_target_ignores_placeholder_values() {
        let lines = vec![
            "Expected output format: PREVIEW_TARGET: http://127.0.0.1:<port>".to_string(),
            "PREVIEW_TARGET: http://127.0.0.1:4173".to_string(),
        ];
        let target = extract_preview_target(&lines);
        assert_eq!(target, Some("http://127.0.0.1:4173".to_string()));
    }

    #[test]
    fn test_extract_preview_url_raw_line() {
        let lines = vec![
            "Logs...".to_string(),
            "PREVIEW_URL: https://task-abcd.preview.example.com".to_string(),
        ];
        let url = extract_preview_url(&lines);
        assert_eq!(
            url,
            Some("https://task-abcd.preview.example.com".to_string())
        );
    }

    #[test]
    fn test_extract_preview_url_from_normalized_json_content() {
        let lines = vec![
            r#"{"timestamp":"2026-02-08T00:00:00Z","entry_type":{"type":"assistant_message"},"content":"Deploy summary\nPREVIEW_URL: https://demo.trycloudflare.com"}"#.to_string(),
        ];
        let url = extract_preview_url(&lines);
        assert_eq!(url, Some("https://demo.trycloudflare.com".to_string()));
    }

    #[test]
    fn test_extract_preview_target_strips_trailing_json_artifacts() {
        let lines = vec![r#"PREVIEW_TARGET: http://127.0.0.1:8080"}"#.to_string()];
        let target = extract_preview_target(&lines);
        assert_eq!(target, Some("http://127.0.0.1:8080".to_string()));
    }

    #[test]
    fn test_extract_preview_url_strips_trailing_json_artifacts() {
        let lines = vec![r#"PREVIEW_URL: http://127.0.0.1:8080"}"#.to_string()];
        let url = extract_preview_url(&lines);
        assert_eq!(url, Some("http://127.0.0.1:8080".to_string()));
    }

    #[test]
    fn test_extract_preview_target_strips_trailing_markdown_summary() {
        let lines = vec![
            r#"PREVIEW_TARGET: http://localhost:4174**Summary:**- Built successfully"#.to_string(),
        ];
        let target = extract_preview_target(&lines);
        assert_eq!(target, Some("http://localhost:4174".to_string()));
    }

    #[test]
    fn test_extract_preview_url_strips_trailing_markdown_summary() {
        let lines = vec![
            r#"PREVIEW_URL: http://localhost:4174**Summary:**- Built successfully"#.to_string(),
        ];
        let url = extract_preview_url(&lines);
        assert_eq!(url, Some("http://localhost:4174".to_string()));
    }

    #[test]
    fn test_extract_preview_target_strips_concatenated_preview_url_marker() {
        let lines = vec![
            r#"PREVIEW_TARGET: http://localhost:8081PREVIEW_URL: http://localhost:8081What was done:- Built successfully"#.to_string(),
        ];
        let target = extract_preview_target(&lines);
        assert_eq!(target, Some("http://localhost:8081".to_string()));
    }

    #[test]
    fn test_extract_preview_url_strips_concatenated_preview_target_marker() {
        let lines = vec![
            r#"PREVIEW_URL: http://localhost:8081PREVIEW_TARGET: http://localhost:8081Summary:- Built successfully"#.to_string(),
        ];
        let url = extract_preview_url(&lines);
        assert_eq!(url, Some("http://localhost:8081".to_string()));
    }

    #[test]
    fn test_normalize_repo_url_removes_credentials() {
        let url = "https://oauth2:token@gitlab.example.com/group/repo.git";
        let normalized = normalize_repo_url(url);
        assert_eq!(normalized, "https://gitlab.example.com/group/repo.git");
    }

    #[test]
    fn test_normalize_repo_url_keeps_ssh_style() {
        let url = "git@gitlab.example.com:group/repo.git";
        let normalized = normalize_repo_url(url);
        assert_eq!(normalized, url);
    }

    #[test]
    fn test_extract_labeled_value_supports_markdown_bullet_and_backticks() {
        let lines = vec![
            "- `deployment_status`: active".to_string(),
            "- production_deployment_status = deploy_failed".to_string(),
        ];

        let deployment_status = extract_labeled_value(&lines, "deployment_status");
        let production_status = extract_labeled_value(&lines, "production_deployment_status");
        assert_eq!(deployment_status, Some("active".to_string()));
        assert_eq!(production_status, Some("deploy_failed".to_string()));
    }

    #[test]
    fn test_extract_deployment_report_from_normalized_json_content() {
        let lines = vec![
            r#"{"timestamp":"2026-02-09T00:00:00Z","entry_type":{"type":"assistant_message"},"content":"Deployment\ndeployment_status: active\nproduction_deployment_status: active\nproduction_deployment_url: https://api.example.com\ndelivery_status: complete"}"#.to_string(),
        ];

        let report = extract_deployment_report(&lines).and_then(|v| v.as_object().cloned());
        let Some(report) = report else {
            panic!("expected deployment report object");
        };
        assert_eq!(
            report
                .get("deployment_status")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "active"
        );
        assert_eq!(
            report
                .get("production_deployment_url")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "https://api.example.com"
        );
        assert_eq!(
            report
                .get("delivery_status")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "complete"
        );
    }

    #[test]
    fn test_extract_deployment_report_returns_none_without_structured_fields() {
        let lines = vec!["Deployment completed successfully".to_string()];
        assert!(extract_deployment_report(&lines).is_none());
    }

    #[test]
    fn test_extract_mr_title_raw_line() {
        let lines = vec![
            "Some log".to_string(),
            "MR_TITLE: feat: Add user authentication".to_string(),
        ];
        let title = extract_mr_title(&lines);
        assert_eq!(title, Some("feat: Add user authentication".to_string()));
    }

    #[test]
    fn test_extract_mr_title_from_normalized_json_content() {
        let lines = vec![
            r#"{"timestamp":"2026-02-08T00:00:00Z","entry_type":{"type":"assistant_message"},"content":"Final report\nMR_TITLE: fix: Resolve login bug\nMR_DESCRIPTION: ..."}"#
                .to_string(),
        ];
        let title = extract_mr_title(&lines);
        assert_eq!(title, Some("fix: Resolve login bug".to_string()));
    }

    #[test]
    fn test_extract_mr_title_lowercase_label() {
        let lines = vec!["mr_title: chore: Update dependencies".to_string()];
        let title = extract_mr_title(&lines);
        assert_eq!(title, Some("chore: Update dependencies".to_string()));
    }

    #[test]
    fn test_extract_mr_description_raw_line() {
        let lines = vec![
            "Some log".to_string(),
            "MR_DESCRIPTION: ## Summary - Implemented OAuth2 flow".to_string(),
        ];
        let desc = extract_mr_description(&lines);
        assert_eq!(
            desc,
            Some("## Summary - Implemented OAuth2 flow".to_string())
        );
    }

    #[test]
    fn test_extract_mr_description_from_normalized_json_content() {
        let lines = vec![
            r#"{"content":"Deployment summary\nMR_DESCRIPTION: ## Changes\n- Added auth routes\n- Updated config"}"#
                .to_string(),
        ];
        let desc = extract_mr_description(&lines);
        // extract_labeled_value captures to end of line; multiline goes to next line
        assert_eq!(desc, Some("## Changes".to_string()));
    }

    #[test]
    fn test_extract_mr_description_returns_none_without_label() {
        let lines = vec!["Merge request created successfully".to_string()];
        assert!(extract_mr_description(&lines).is_none());
    }
}
