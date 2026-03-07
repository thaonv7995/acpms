//! Claude Code CLI client for spawning and managing agent sessions.

use anyhow::{Context, Result};
use command_group::{AsyncCommandGroup, AsyncGroupChild};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::agent_client::ClaudeAgentClient;
use crate::approval::ApprovalService;
use crate::follow_up_utils::wrap_trivial_follow_up;
use crate::log_writer::LogWriter;
use crate::msg_store::{LogMsg, MsgStore};
use crate::normalize_stderr_for_display;
use crate::orchestrator_status::StatusManager;
use crate::process::{InterruptReceiver, InterruptSender};
use crate::protocol::{PermissionMode, ProtocolPeer};
use crate::stdout_dup::create_stdout_pipe_writer;
use crate::{
    RuntimeSkillLoadResult, RuntimeSkillSearchResult, SkillKnowledgeHandle, SkillKnowledgeStatus,
    SkillRuntime,
};

const MAX_RUNTIME_SKILL_CONTENT_CHARS: usize = 12_000;

/// Result of spawning a Claude agent process.
pub struct SpawnedAgent {
    /// The spawned child process (as a process group for proper termination).
    pub child: AsyncGroupChild,
    /// Channel to request graceful shutdown.
    pub interrupt_sender: Option<InterruptSender>,
    /// Channel to receive interrupt signal (for the streaming task).
    pub interrupt_receiver: Option<InterruptReceiver>,
    /// Message store for log buffering and streaming (SDK mode).
    pub msg_store: Option<Arc<MsgStore>>,
}

#[derive(Clone)]
pub struct ClaudeRuntimeSkillConfig {
    pub repo_path: PathBuf,
    pub skill_knowledge: SkillKnowledgeHandle,
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

async fn handle_runtime_skill_tool_calls_for_sdk(
    attempt_id: Uuid,
    metadata: serde_json::Value,
    runtime_config: &ClaudeRuntimeSkillConfig,
    db_pool: Option<&sqlx::PgPool>,
    broadcast_tx: Option<&tokio::sync::broadcast::Sender<crate::AgentEvent>>,
    protocol_peer: &ProtocolPeer,
) {
    let Some(tool_calls) = metadata
        .get("tool_calls")
        .and_then(|value| value.as_array())
    else {
        return;
    };

    let runtime = SkillRuntime::new(Some(&runtime_config.skill_knowledge));
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
                if let (Some(pool), Some(tx)) = (db_pool, broadcast_tx) {
                    let _ = StatusManager::log(pool, tx, attempt_id, "system", &summary).await;
                }

                let follow_up = format_runtime_skill_search_follow_up(query, &result);
                if let Err(error) = protocol_peer.send_user_message(follow_up).await {
                    tracing::warn!(
                        attempt_id = %attempt_id,
                        error = %error,
                        query = %query,
                        "Failed to deliver runtime skill search response in SDK session"
                    );
                }
            }
            "load_skill" => {
                let Some(skill_id) = args.get("skill_id").and_then(|value| value.as_str()) else {
                    continue;
                };
                let result = runtime.load_runtime(skill_id, Some(&runtime_config.repo_path));
                let summary = format_runtime_skill_load_summary(skill_id, &result);
                if let (Some(pool), Some(tx)) = (db_pool, broadcast_tx) {
                    let _ = StatusManager::log(pool, tx, attempt_id, "system", &summary).await;
                }

                let follow_up = format_runtime_skill_load_follow_up(skill_id, &result);
                if let Err(error) = protocol_peer.send_user_message(follow_up).await {
                    tracing::warn!(
                        attempt_id = %attempt_id,
                        error = %error,
                        skill_id = %skill_id,
                        "Failed to deliver runtime skill load response in SDK session"
                    );
                }
            }
            _ => {}
        }
    }
}

pub struct ClaudeClient;

impl ClaudeClient {
    pub fn new() -> Self {
        Self
    }

    /// Spawns the Claude Code CLI in the specified worktree.
    ///
    /// Returns a `SpawnedAgent` containing:
    /// - The child process (as a process group)
    /// - Interrupt channels for graceful shutdown
    ///
    /// ## Process Group
    /// The process is spawned as a process group leader, allowing proper
    /// termination of all child processes (e.g., spawned tools).
    pub async fn spawn_session(
        &self,
        worktree_path: &Path,
        instruction: &str,
        attempt_id: Uuid,
        env_vars: Option<HashMap<String, String>>,
        agent_settings: Option<&crate::AgentSettings>,
    ) -> Result<SpawnedAgent> {
        // Check if worktree path exists
        if !worktree_path.exists() {
            anyhow::bail!("Worktree path does not exist: {:?}", worktree_path);
        }

        let default_settings = crate::AgentSettings::default();
        let settings = agent_settings.unwrap_or(&default_settings);

        // Build command via sh -c (required for group_spawn compatibility with npx)
        let mut cmd = Command::new("sh");
        cmd.arg("-c");

        let base_cmd = if settings.enable_router_service {
            tracing::info!(
                attempt_id = %attempt_id,
                router_version = %settings.router_version,
                "Spawning agent with router service"
            );
            format!(
                "npx -y @musistudio/claude-code-router@{} code",
                settings.router_version
            )
        } else {
            "npx -y @anthropic-ai/claude-code".to_string()
        };

        // Escape shell special characters in instruction to prevent interpretation:
        // - double quotes (") -> \"
        // - backticks (`) -> \`  (prevents command substitution)
        // - dollar signs ($) -> \$ (prevents variable expansion)
        let escaped_instruction = instruction
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$");

        // Claude Code CLI flags:
        // --print (-p): Non-interactive mode, print response and exit
        // --dangerously-skip-permissions: Skip all permission prompts
        // --allowedTools '*': Allow all tools without prompting
        // --output-format text: Plain text output
        let full_cmd = format!(
            "{} --print --dangerously-skip-permissions --allowedTools '*' --output-format text \"{}\"",
            base_cmd,
            escaped_instruction
        );

        cmd.arg(&full_cmd)
            .current_dir(worktree_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // Inject environment variables
        if let Some(vars) = env_vars {
            for (key, value) in vars {
                cmd.env(key, value);
            }
        }

        // Router-specific env vars
        if settings.enable_router_service {
            if !settings.router_filters.is_empty() {
                if let Some(filters_json) = crate::serialize_filters(&settings.router_filters) {
                    cmd.env("ROUTER_FILTERS", filters_json);
                }
            }
            cmd.env("ROUTER_TIMEOUT", settings.router_timeout_ms.to_string());
        }

        // Spawn as process group for proper termination
        let child = cmd
            .group_spawn()
            .with_context(|| format!("Failed to spawn Claude Code CLI in {:?}", worktree_path))?;

        // Create interrupt channel for graceful shutdown
        let (interrupt_tx, interrupt_rx): (InterruptSender, InterruptReceiver) = oneshot::channel();

        Ok(SpawnedAgent {
            child,
            interrupt_sender: Some(interrupt_tx),
            interrupt_receiver: Some(interrupt_rx),
            msg_store: None,
        })
    }

    /// Spawn Claude Code CLI for Project Assistant session.
    ///
    /// We run in `--print` mode so the initial assistant response is emitted deterministically
    /// in non-TTY environments (web backend worker), instead of waiting on an interactive prompt.
    pub async fn spawn_assistant_session(
        &self,
        worktree_path: &Path,
        instruction: &str,
        _session_id: Uuid,
        agent_settings: Option<&crate::AgentSettings>,
    ) -> Result<SpawnedAgent> {
        if !worktree_path.exists() {
            anyhow::bail!("Worktree path does not exist: {:?}", worktree_path);
        }

        let default_settings = crate::AgentSettings::default();
        let settings = agent_settings.unwrap_or(&default_settings);

        let mut cmd = Command::new("sh");
        cmd.arg("-c");

        let base_cmd = if settings.enable_router_service {
            format!(
                "npx -y @musistudio/claude-code-router@{} code",
                settings.router_version
            )
        } else {
            "npx -y @anthropic-ai/claude-code".to_string()
        };

        let escaped = instruction
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$");

        let full_cmd = format!(
            "{} --print --dangerously-skip-permissions --allowedTools '*' --output-format text \"{}\"",
            base_cmd, escaped
        );

        cmd.arg(&full_cmd)
            .current_dir(worktree_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if settings.enable_router_service {
            if !settings.router_filters.is_empty() {
                if let Some(filters_json) = crate::serialize_filters(&settings.router_filters) {
                    cmd.env("ROUTER_FILTERS", filters_json);
                }
            }
            cmd.env("ROUTER_TIMEOUT", settings.router_timeout_ms.to_string());
        }

        let child = cmd
            .group_spawn()
            .with_context(|| format!("Failed to spawn assistant CLI in {:?}", worktree_path))?;

        let (interrupt_tx, interrupt_rx): (InterruptSender, InterruptReceiver) = oneshot::channel();

        Ok(SpawnedAgent {
            child,
            interrupt_sender: Some(interrupt_tx),
            interrupt_receiver: Some(interrupt_rx),
            msg_store: None,
        })
    }

    /// Spawn Claude Code CLI in SDK control mode (stream-json with bidirectional protocol).
    ///
    /// ## SDK Mode Features:
    /// - Bidirectional JSON-RPC communication
    /// - Tool permission requests (approval workflow)
    /// - Structured log format (JSON)
    /// - Graceful interruption via control protocol
    ///
    /// ## Returns:
    /// - SpawnedAgent with interrupt channel
    ///
    /// ## Note:
    /// This method spawns a background task to handle protocol communication.
    /// The task reads from stdout (control messages) and writes to stdin (responses).
    pub async fn spawn_session_sdk(
        &self,
        worktree_path: &Path,
        instruction: &str,
        attempt_id: Uuid,
        env_vars: Option<HashMap<String, String>>,
        approval_service: Option<Arc<dyn ApprovalService>>,
        db_pool: Option<sqlx::PgPool>,
        broadcast_tx: Option<tokio::sync::broadcast::Sender<crate::AgentEvent>>,
        agent_settings: Option<&crate::AgentSettings>,
        input_rx: Option<mpsc::UnboundedReceiver<String>>,
        runtime_skill_config: Option<ClaudeRuntimeSkillConfig>,
    ) -> Result<SpawnedAgent> {
        // Check if worktree path exists
        if !worktree_path.exists() {
            anyhow::bail!("Worktree path does not exist: {:?}", worktree_path);
        }

        let default_settings = crate::AgentSettings::default();
        let settings = agent_settings.unwrap_or(&default_settings);

        // Build command with optional router wrapper
        let mut cmd = Command::new("sh");
        cmd.arg("-c");

        let base_cmd = if settings.enable_router_service {
            tracing::info!(
                attempt_id = %attempt_id,
                router_version = %settings.router_version,
                "Spawning agent (SDK mode) with router service"
            );
            format!(
                "npx -y @musistudio/claude-code-router@{} code",
                settings.router_version
            )
        } else {
            "npx -y @anthropic-ai/claude-code".to_string()
        };

        // Claude Code SDK mode flags
        let full_cmd = format!(
            "{} -p --verbose --output-format=stream-json --input-format=stream-json --include-partial-messages --disallowedTools=AskUserQuestion",
            base_cmd
        );

        cmd.arg(&full_cmd)
            .current_dir(worktree_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // Inject environment variables
        if let Some(vars) = env_vars {
            for (key, value) in vars {
                cmd.env(key, value);
            }
        }

        // Router-specific env vars
        if settings.enable_router_service {
            if !settings.router_filters.is_empty() {
                if let Some(filters_json) = crate::serialize_filters(&settings.router_filters) {
                    cmd.env("ROUTER_FILTERS", filters_json);
                }
            }
            cmd.env("ROUTER_TIMEOUT", settings.router_timeout_ms.to_string());
        }

        // Spawn process
        let mut child = cmd.group_spawn().with_context(|| {
            format!(
                "Failed to spawn Claude Code CLI in SDK mode in {:?}",
                worktree_path
            )
        })?;

        // Extract stdio handles
        let child_stdin = child
            .inner()
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get stdin"))?;
        let child_stdout = child
            .inner()
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get stdout"))?;

        // Create fresh stdout pipe for logging
        let new_stdout = create_stdout_pipe_writer(&mut child)
            .context("Failed to create stdout pipe for logging")?;

        // Create interrupt channel
        let (interrupt_tx, interrupt_rx) = oneshot::channel();

        // Clone for stderr consumer (before protocol handler consumes them)
        let stderr_db_pool = db_pool.clone();
        let stderr_broadcast_tx = broadcast_tx.clone();
        let runtime_db_pool = db_pool.clone();
        let runtime_broadcast_tx = broadcast_tx.clone();
        let runtime_skill_config = runtime_skill_config.clone();

        let (runtime_tool_tx, mut runtime_tool_rx) = if runtime_skill_config.is_some() {
            let (tx, rx) = mpsc::unbounded_channel::<serde_json::Value>();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        // Clone instruction for 'static lifetime in spawn
        let instruction_owned = instruction.to_string();
        let mut input_rx = input_rx;

        // CRITICAL: Spawn task to consume LogWriter output and save to database
        // Without this, the pipe fills up and blocks!
        let stderr_pipe = child
            .inner()
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get stderr"))?;

        // Spawn protocol handler task
        tokio::spawn(async move {
            tracing::info!(attempt_id = %attempt_id, "Protocol handler started");

            // Create client with database logging
            let client = if let (Some(pool), Some(tx)) = (db_pool, broadcast_tx) {
                ClaudeAgentClient::with_database(
                    attempt_id,
                    LogWriter::new(new_stdout),
                    approval_service,
                    pool,
                    tx,
                    runtime_tool_tx.clone(),
                )
            } else {
                ClaudeAgentClient::new(
                    attempt_id,
                    LogWriter::new(new_stdout),
                    approval_service,
                    runtime_tool_tx.clone(),
                )
            };

            let protocol_peer =
                ProtocolPeer::spawn(child_stdin, child_stdout, client, interrupt_rx);

            // Initialize protocol
            if let Err(e) = protocol_peer.initialize(None).await {
                tracing::error!(attempt_id = %attempt_id, error = %e, "Failed to initialize control protocol");
                return;
            }
            tracing::info!(attempt_id = %attempt_id, "Protocol initialized successfully");

            // Set permission mode to bypass (auto-approve all tools, matches vibe-kanban)
            if let Err(e) = protocol_peer
                .set_permission_mode(PermissionMode::BypassPermissions)
                .await
            {
                tracing::warn!(attempt_id = %attempt_id, error = %e, "Failed to set permission mode");
            }

            // Send user message (main prompt)
            if let Err(e) = protocol_peer.send_user_message(instruction_owned).await {
                tracing::error!(attempt_id = %attempt_id, error = %e, "Failed to send prompt");
                return;
            }
            tracing::info!(attempt_id = %attempt_id, "User message sent successfully");

            if let (Some(config), Some(mut rx)) = (runtime_skill_config, runtime_tool_rx.take()) {
                let runtime_peer = protocol_peer.clone();
                let runtime_pool = runtime_db_pool.clone();
                let runtime_tx = runtime_broadcast_tx.clone();
                tokio::spawn(async move {
                    while let Some(metadata) = rx.recv().await {
                        handle_runtime_skill_tool_calls_for_sdk(
                            attempt_id,
                            metadata,
                            &config,
                            runtime_pool.as_ref(),
                            runtime_tx.as_ref(),
                            &runtime_peer,
                        )
                        .await;
                    }
                });
            }

            // Forward realtime user input messages (if provided) to the same SDK session.
            // Queue behavior: messages are delivered when agent reaches next turn boundary.
            // Avoids interrupting mid-execution (e.g. during git push) which could leave
            // the system in an inconsistent state.
            if let Some(mut rx) = input_rx.take() {
                while let Some(user_input) = rx.recv().await {
                    let trimmed = user_input.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    let to_send = wrap_trivial_follow_up(trimmed);
                    if let Err(e) = protocol_peer.send_user_message(to_send).await {
                        tracing::warn!(
                            attempt_id = %attempt_id,
                            error = %e,
                            "Failed to forward queued user input to SDK session"
                        );
                        break;
                    }

                    tracing::info!(
                        attempt_id = %attempt_id,
                        "Queued user input for SDK session (will process at next turn boundary)"
                    );
                }
            }
        });

        // Spawn stderr consumer (important: prevent stderr pipe from blocking)
        // Persist normalized stderr to attempt logs (filter noise, truncate technical dump)
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut reader = BufReader::new(stderr_pipe).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(normalized) = normalize_stderr_for_display(&line) {
                    tracing::error!(target: "claude_cli_stderr", attempt_id = %attempt_id, "{}", normalized);
                    if let (Some(ref pool), Some(ref tx)) =
                        (stderr_db_pool.as_ref(), stderr_broadcast_tx.as_ref())
                    {
                        let _ =
                            StatusManager::log(pool, tx, attempt_id, "stderr", &normalized).await;
                    }
                }
            }
        });

        // Create MsgStore for log buffering
        let msg_store = Arc::new(MsgStore::new());

        // Setup stdout/stderr consumers with MsgStore
        // Note: stdout was replaced with fresh pipe by create_stdout_pipe_writer()
        // The pipe reader is now child.stdout, we need to read from it
        if let Some(stdout) = child.inner().stdout.take() {
            let store = msg_store.clone();
            tokio::spawn(async move {
                use tokio::io::BufReader;
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    store.push(LogMsg::Stdout(line));
                }
                store.push(LogMsg::Finished);
            });
        }

        Ok(SpawnedAgent {
            child,
            interrupt_sender: Some(interrupt_tx),
            interrupt_receiver: None,
            msg_store: Some(msg_store),
        })
    }

    /// Stream logs from stdout/stderr with interrupt support.
    ///
    /// ## Arguments
    /// * `child` - The child process to stream from
    /// * `interrupt_rx` - Optional receiver to signal early termination
    /// * `callback` - Function called for each log line (line, is_stderr)
    ///
    /// ## Interruption
    /// If `interrupt_rx` receives a signal, streaming will stop gracefully
    /// and the function will return `Ok(())`.
    pub async fn stream_logs_with_interrupt<F>(
        child: &mut AsyncGroupChild,
        mut interrupt_rx: Option<InterruptReceiver>,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(String, bool),
    {
        let stdout = child.inner().stdout.take().context("No stdout captured")?;
        let stderr = child.inner().stderr.take().context("No stderr captured")?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        loop {
            tokio::select! {
                // Check for interrupt signal
                _ = async {
                    if let Some(ref mut rx) = interrupt_rx {
                        rx.await.ok()
                    } else {
                        // No interrupt channel, never resolves
                        std::future::pending::<Option<()>>().await
                    }
                } => {
                    tracing::debug!("Received interrupt signal, stopping log stream");
                    callback("Agent interrupted by user".to_string(), true);
                    break;
                }

                line = stdout_reader.next_line() => {
                    match line {
                        Ok(Some(completed_line)) => callback(completed_line, false),
                        Ok(None) => break, // EOF
                        Err(e) => return Err(anyhow::anyhow!("Error reading stdout: {}", e)),
                    }
                }

                line = stderr_reader.next_line() => {
                    match line {
                        Ok(Some(completed_line)) => callback(completed_line, true),
                        Ok(None) => break, // EOF
                        Err(e) => return Err(anyhow::anyhow!("Error reading stderr: {}", e)),
                    }
                }

                else => break,
            }
        }

        Ok(())
    }

    /// Legacy helper to stream logs from stdout/stderr to a callback.
    ///
    /// Note: This method takes ownership of the child and waits for it to complete.
    /// For more control, use `stream_logs_with_interrupt` instead.
    pub async fn stream_logs<F>(mut child: AsyncGroupChild, mut callback: F) -> Result<()>
    where
        F: FnMut(String, bool),
    {
        Self::stream_logs_with_interrupt(&mut child, None, &mut callback).await?;

        let status = child.wait().await?;
        if !status.success() {
            callback(format!("Process exited with status: {}", status), true);
        }

        Ok(())
    }
}

impl Default for ClaudeClient {
    fn default() -> Self {
        Self::new()
    }
}
