//! Claude Code CLI SDK control protocol implementation.
//!
//! This module implements bidirectional JSON-RPC communication with Claude Code CLI
//! in SDK mode. It handles control requests (tool permissions, hooks) and sends
//! control responses back to the CLI.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;
use tokio::sync::{oneshot, Mutex};

/// Permission mode for tool execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    Default,           // Ask for each tool
    AcceptEdits,       // Auto-approve edit tools
    Plan,              // Plan mode (preview)
    BypassPermissions, // Auto-approve all
}

impl PermissionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::Plan => "plan",
            Self::BypassPermissions => "bypassPermissions",
        }
    }
}

impl std::fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Permission update for changing tool permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionUpdate {
    pub update_type: PermissionUpdateType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<PermissionMode>,
    pub destination: PermissionUpdateDestination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionUpdateType {
    SetMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PermissionUpdateDestination {
    Session,
}

/// Result of a permission check
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "lowercase")]
pub enum PermissionResult {
    Allow {
        updated_input: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_permissions: Option<Vec<PermissionUpdate>>,
    },
    Deny {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        interrupt: Option<bool>,
    },
}

/// Messages received from Claude CLI (stdout)
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CLIMessage {
    ControlRequest {
        request_id: String,
        request: ControlRequestType,
    },
    ControlResponse {
        response: ControlResponseType,
    },
    Result(Value),
    #[serde(untagged)]
    Other(Value),
}

/// Control requests from CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ControlRequestType {
    CanUseTool {
        tool_name: String,
        input: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        permission_suggestions: Option<Vec<PermissionUpdate>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
    },
    HookCallback {
        callback_id: String,
        input: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
    },
}

/// Control responses from CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ControlResponseType {
    Success {
        request_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        response: Option<Value>,
    },
    Error {
        request_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

/// Control response message wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlResponseMessage {
    #[serde(rename = "type")]
    message_type: String,
    pub response: ControlResponseType,
}

impl ControlResponseMessage {
    pub fn new(response: ControlResponseType) -> Self {
        Self {
            message_type: "control_response".to_string(),
            response,
        }
    }
}

/// SDK control request message wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SDKControlRequest {
    #[serde(rename = "type")]
    message_type: String,
    pub request_id: String,
    pub request: SDKControlRequestType,
}

impl SDKControlRequest {
    pub fn new(request: SDKControlRequestType) -> Self {
        Self {
            message_type: "control_request".to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            request,
        }
    }
}

/// SDK control request types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum SDKControlRequestType {
    SetPermissionMode {
        mode: PermissionMode,
    },
    Initialize {
        #[serde(skip_serializing_if = "Option::is_none")]
        hooks: Option<Value>,
    },
    Interrupt {},
}

/// User message wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    User { message: ClaudeUserMessage },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeUserMessage {
    role: String,
    content: String,
}

impl Message {
    pub fn new_user(content: String) -> Self {
        Self::User {
            message: ClaudeUserMessage {
                role: "user".to_string(),
                content,
            },
        }
    }
}

/// Trait for handling protocol callbacks
#[async_trait::async_trait]
pub trait ProtocolHandler: Send + Sync {
    /// Handle tool permission request
    async fn on_can_use_tool(
        &self,
        tool_name: String,
        input: Value,
        permission_suggestions: Option<Vec<PermissionUpdate>>,
        tool_use_id: Option<String>,
    ) -> Result<PermissionResult>;

    /// Handle hook callback
    async fn on_hook_callback(
        &self,
        callback_id: String,
        input: Value,
        tool_use_id: Option<String>,
    ) -> Result<Value>;

    /// Handle non-control messages (logs, tool calls, etc.)
    async fn on_non_control(&self, line: &str) -> Result<()>;
}

/// Bidirectional JSON-RPC protocol peer
#[derive(Clone)]
pub struct ProtocolPeer {
    stdin: Arc<Mutex<ChildStdin>>,
}

impl ProtocolPeer {
    /// Spawn protocol handler with stdin/stdout
    ///
    /// This spawns a background task that reads from stdout, parses JSON messages,
    /// and routes them to the handler. It returns immediately with a ProtocolPeer
    /// that can be used to send messages to the CLI.
    pub fn spawn(
        stdin: ChildStdin,
        stdout: tokio::process::ChildStdout,
        handler: Arc<dyn ProtocolHandler>,
        interrupt_rx: oneshot::Receiver<()>,
    ) -> Self {
        let peer = Self {
            stdin: Arc::new(Mutex::new(stdin)),
        };

        let reader_peer = peer.clone();
        tokio::spawn(async move {
            if let Err(e) = reader_peer.read_loop(stdout, handler, interrupt_rx).await {
                tracing::error!("Protocol reader loop error: {}", e);
            }
        });

        peer
    }

    /// Main read loop - parses JSON from stdout
    async fn read_loop(
        &self,
        stdout: tokio::process::ChildStdout,
        handler: Arc<dyn ProtocolHandler>,
        interrupt_rx: oneshot::Receiver<()>,
    ) -> Result<()> {
        let mut reader = BufReader::new(stdout);
        let mut buffer = String::new();
        let mut interrupt_rx = Some(interrupt_rx);

        loop {
            buffer.clear();

            tokio::select! {
                line_result = reader.read_line(&mut buffer) => {
                    match line_result {
                        Ok(0) => {
                            // EOF - process exited
                            tracing::debug!("Protocol peer: EOF received, exiting read loop");
                            break;
                        }
                        Ok(_) => {
                            let line = buffer.trim();
                            if line.is_empty() {
                                continue;
                            }

                            // Parse JSON message
                            match serde_json::from_str::<CLIMessage>(line) {
                                Ok(CLIMessage::ControlRequest {
                                    request_id,
                                    request,
                                }) => {
                                    tracing::debug!(
                                        request_id = %request_id,
                                        "Protocol: Received ControlRequest"
                                    );
                                    self.handle_control_request(&handler, request_id, request)
                                        .await;
                                }
                                Ok(CLIMessage::ControlResponse { .. }) => {
                                    // We don't expect control responses from CLI in this direction
                                    tracing::debug!("Received control response from CLI (unexpected)");
                                }
                                Ok(CLIMessage::Result(ref value)) => {
                                    tracing::info!(
                                        result_type = ?value.get("type"),
                                        "Protocol: Received Result message, forwarding to handler"
                                    );
                                    // Final result - forward to handler
                                    if let Err(e) = handler.on_non_control(line).await {
                                        tracing::error!("Error handling result message: {}", e);
                                    }
                                    // CLI is done - exit read loop
                                    break;
                                }
                                Ok(CLIMessage::Other(ref value)) => {
                                    // Log the message type for debugging
                                    let msg_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                                    tracing::debug!(
                                        msg_type = %msg_type,
                                        line_len = line.len(),
                                        "Protocol: Received Other message, forwarding to handler"
                                    );
                                    // Regular log/tool call/result - forward to handler
                                    if let Err(e) = handler.on_non_control(line).await {
                                        tracing::error!("Error handling non-control message: {}", e);
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        error = %e,
                                        line_preview = %line.chars().take(150).collect::<String>(),
                                        "Protocol: Failed to parse CLI message as JSON"
                                    );
                                    // Don't break - continue processing other messages
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error reading stdout: {}", e);
                            break;
                        }
                    }
                }
                _ = async {
                    if let Some(rx) = interrupt_rx.take() {
                        rx.await.ok()
                    } else {
                        // No interrupt channel or already fired - never resolves
                        std::future::pending::<Option<()>>().await
                    }
                } => {
                    tracing::debug!("Received interrupt signal, sending interrupt to Claude");
                    if let Err(e) = self.interrupt().await {
                        tracing::debug!("Failed to send interrupt to Claude: {}", e);
                    }
                    // Continue read loop to process remaining messages
                }
            }
        }

        Ok(())
    }

    /// Handle control requests from CLI
    async fn handle_control_request(
        &self,
        handler: &Arc<dyn ProtocolHandler>,
        request_id: String,
        request: ControlRequestType,
    ) {
        match request {
            ControlRequestType::CanUseTool {
                tool_name,
                input,
                permission_suggestions,
                tool_use_id,
            } => {
                match handler
                    .on_can_use_tool(
                        tool_name.clone(),
                        input,
                        permission_suggestions,
                        tool_use_id,
                    )
                    .await
                {
                    Ok(result) => match serde_json::to_value(result) {
                        Ok(payload) => {
                            if let Err(e) = self.send_hook_response(request_id, payload).await {
                                tracing::error!("Failed to send permission result: {}", e);
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to serialize permission result for {}: {}",
                                tool_name,
                                e
                            );
                            if let Err(e2) = self
                                .send_error(
                                    request_id,
                                    "Failed to serialize permission result".to_string(),
                                )
                                .await
                            {
                                tracing::error!("Failed to send serialization error: {}", e2);
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!("Error in on_can_use_tool for {}: {}", tool_name, e);
                        if let Err(e2) = self.send_error(request_id, e.to_string()).await {
                            tracing::error!("Failed to send error response: {}", e2);
                        }
                    }
                }
            }
            ControlRequestType::HookCallback {
                callback_id,
                input,
                tool_use_id,
            } => {
                match handler
                    .on_hook_callback(callback_id.clone(), input, tool_use_id)
                    .await
                {
                    Ok(hook_output) => {
                        if let Err(e) = self.send_hook_response(request_id, hook_output).await {
                            tracing::error!("Failed to send hook callback result: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error in on_hook_callback for {}: {}", callback_id, e);
                        if let Err(e2) = self.send_error(request_id, e.to_string()).await {
                            tracing::error!("Failed to send error response: {}", e2);
                        }
                    }
                }
            }
        }
    }

    /// Send JSON message to stdin
    async fn send_json<T: Serialize>(&self, message: &T) -> Result<()> {
        let json = serde_json::to_string(message).context("Failed to serialize message")?;
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(json.as_bytes())
            .await
            .context("Failed to write to stdin")?;
        stdin
            .write_all(b"\n")
            .await
            .context("Failed to write newline")?;
        stdin.flush().await.context("Failed to flush stdin")?;
        Ok(())
    }

    /// Send hook response
    pub async fn send_hook_response(&self, request_id: String, hook_output: Value) -> Result<()> {
        self.send_json(&ControlResponseMessage::new(ControlResponseType::Success {
            request_id,
            response: Some(hook_output),
        }))
        .await
    }

    /// Send error response
    async fn send_error(&self, request_id: String, error: String) -> Result<()> {
        self.send_json(&ControlResponseMessage::new(ControlResponseType::Error {
            request_id,
            error: Some(error),
        }))
        .await
    }

    /// Initialize protocol (must be called first)
    pub async fn initialize(&self, hooks: Option<Value>) -> Result<()> {
        self.send_json(&SDKControlRequest::new(SDKControlRequestType::Initialize {
            hooks,
        }))
        .await
    }

    /// Set permission mode
    pub async fn set_permission_mode(&self, mode: PermissionMode) -> Result<()> {
        self.send_json(&SDKControlRequest::new(
            SDKControlRequestType::SetPermissionMode { mode },
        ))
        .await
    }

    /// Send user message (the main prompt)
    pub async fn send_user_message(&self, content: String) -> Result<()> {
        self.send_json(&Message::new_user(content)).await
    }

    /// Send interrupt signal
    pub async fn interrupt(&self) -> Result<()> {
        self.send_json(&SDKControlRequest::new(SDKControlRequestType::Interrupt {}))
            .await
    }
}
