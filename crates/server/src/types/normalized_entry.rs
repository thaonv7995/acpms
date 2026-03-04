use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Normalized log entry for frontend consumption
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[allow(dead_code)]
pub struct NormalizedEntry {
    pub timestamp: Option<String>,
    pub entry_type: NormalizedEntryType,
    pub content: String,
}

/// Entry type discriminated union
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
#[allow(dead_code)]
pub enum NormalizedEntryType {
    UserMessage,

    UserFeedback {
        denied_tool: String,
    },

    AssistantMessage,

    ToolUse {
        tool_name: String,
        action_type: ActionType,
        status: ToolStatus,
    },

    SystemMessage,

    ErrorMessage {
        error_type: NormalizedEntryError,
    },

    Thinking,

    Loading,

    NextAction {
        failed: bool,
        execution_processes: i32,
        needs_setup: bool,
    },
}

/// Action types for tool use entries
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "action", rename_all = "snake_case")]
#[ts(export)]
#[allow(dead_code)]
pub enum ActionType {
    FileRead {
        path: String,
    },

    FileEdit {
        path: String,
        changes: Vec<FileChange>,
    },

    CommandRun {
        command: String,
        result: Option<CommandRunResult>,
    },

    Search {
        query: String,
    },

    WebFetch {
        url: String,
    },

    Tool {
        tool_name: String,
        arguments: Option<serde_json::Value>,
        result: Option<ToolResult>,
    },

    TaskCreate {
        description: String,
    },

    PlanPresentation {
        plan: String,
    },

    TodoManagement {
        todos: Vec<TodoItem>,
        operation: String,
    },

    Other {
        description: String,
    },
}

/// File change operations
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "action", rename_all = "snake_case")]
#[ts(export)]
#[allow(dead_code)]
pub enum FileChange {
    Write {
        content: String,
    },
    Delete,
    Rename {
        new_path: String,
    },
    Edit {
        unified_diff: String,
        has_line_numbers: bool,
    },
}

/// Tool execution status
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
#[ts(export)]
#[allow(dead_code)]
pub enum ToolStatus {
    Created,
    Success,
    Failed,
    Denied {
        reason: Option<String>,
    },
    PendingApproval {
        approval_id: String,
        requested_at: String,
        timeout_at: String,
    },
    TimedOut,
}

/// Command run result
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[allow(dead_code)]
pub struct CommandRunResult {
    pub exit_status: Option<CommandExitStatus>,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
#[allow(dead_code)]
pub enum CommandExitStatus {
    ExitCode { code: i32 },
    Success { success: bool },
}

/// Tool result wrapper
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[allow(dead_code)]
pub struct ToolResult {
    pub success: bool,
    pub output: Option<String>,
}

/// Todo item for TodoManagement action
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[allow(dead_code)]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
    pub active_form: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
#[allow(dead_code)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

/// Error types for ErrorMessage entries
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
#[allow(dead_code)]
pub enum NormalizedEntryError {
    ToolError,
    ApiError,
    SystemError,
    Unknown,
}
