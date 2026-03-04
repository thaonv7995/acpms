//! Normalized entry types for vibe-kanban style log display.
//!
//! These types match the frontend bindings and provide structured
//! tool_use, assistant_message, and thinking entries.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Normalized entry for frontend consumption (matches vibe-kanban format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEntry {
    pub timestamp: Option<String>,
    pub entry_type: NormalizedEntryType,
    pub content: String,
}

/// Entry type discriminated union
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NormalizedEntryType {
    AssistantMessage,
    ToolUse {
        tool_name: String,
        action_type: ActionType,
        status: ToolStatus,
    },
    NextAction {
        text: String,
    },
    TokenUsageInfo {
        input_tokens: u64,
        output_tokens: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        total_tokens: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model_context_window: Option<u64>,
    },
    UserAnsweredQuestions {
        question: String,
        answer: String,
    },
    SystemMessage,
    Thinking,
}

/// Action types for tool use entries
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
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
        arguments: Option<Value>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum FileChange {
    Write {
        content: String,
    },
    Delete,
    Edit {
        unified_diff: String,
        has_line_numbers: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRunResult {
    pub exit_status: Option<CommandExitStatus>,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandExitStatus {
    ExitCode { code: i32 },
    Success { success: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
    pub active_form: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ToolStatus {
    #[default]
    Created,
    Running,
    Success,
    Failed,
    Denied {
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    PendingApproval {
        approval_id: String,
        requested_at: String,
        timeout_at: String,
    },
    TimedOut,
}

/// Map tool name to ActionType based on Claude Code tool semantics
pub fn map_tool_to_action(tool_name: &str, input: Option<&Value>) -> ActionType {
    let normalized_tool = tool_name.to_lowercase();

    match normalized_tool.as_str() {
        "read" => {
            let path = input
                .and_then(|v| v.get("file_path").or_else(|| v.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ActionType::FileRead { path }
        }
        "edit" | "write" => {
            let path = input
                .and_then(|v| v.get("file_path").or_else(|| v.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ActionType::FileEdit {
                path,
                changes: vec![],
            }
        }
        "bash" => {
            let command = input
                .and_then(|v| v.get("command"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ActionType::CommandRun {
                command,
                result: None,
            }
        }
        "grep" | "glob" => {
            let query = input
                .and_then(|v| v.get("pattern").or(v.get("query")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ActionType::Search { query }
        }
        "webfetch" | "web_fetch" => {
            let url = input
                .and_then(|v| v.get("url"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ActionType::WebFetch { url }
        }
        "task" => {
            let description = input
                .and_then(|v| {
                    v.get("description")
                        .or_else(|| v.get("prompt"))
                        .or_else(|| v.get("task"))
                })
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ActionType::TaskCreate { description }
        }
        "todowrite" | "todo_write" | "todoread" | "todo_read" | "todo_management" => {
            let todos = parse_todos(input);
            let operation = input
                .and_then(|v| v.get("operation"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
                .unwrap_or_else(|| {
                    if normalized_tool.contains("read") {
                        "read".to_string()
                    } else {
                        "update".to_string()
                    }
                });

            ActionType::TodoManagement { todos, operation }
        }
        "exit_plan_mode" | "plan_presentation" => {
            let plan = input
                .and_then(|v| v.get("plan"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ActionType::PlanPresentation { plan }
        }
        _ => ActionType::Tool {
            tool_name: tool_name.to_string(),
            arguments: input.cloned(),
            result: None,
        },
    }
}

/// Format tool content for display
pub fn format_tool_content(tool_name: &str, input: Option<&Value>) -> String {
    let normalized_tool = tool_name.to_lowercase();

    match normalized_tool.as_str() {
        "read" | "edit" | "write" => input
            .and_then(|v| v.get("file_path"))
            .and_then(|v| v.as_str())
            .unwrap_or(tool_name)
            .to_string(),
        "bash" => input
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str())
            .map(|c| c.chars().take(80).collect::<String>())
            .unwrap_or_else(|| tool_name.to_string()),
        "grep" | "glob" => input
            .and_then(|v| v.get("pattern").or(v.get("query")))
            .and_then(|v| v.as_str())
            .unwrap_or(tool_name)
            .to_string(),
        "webfetch" | "web_fetch" => input
            .and_then(|v| v.get("url"))
            .and_then(|v| v.as_str())
            .unwrap_or(tool_name)
            .to_string(),
        "task" => {
            let description = input
                .and_then(|v| {
                    v.get("description")
                        .or_else(|| v.get("prompt"))
                        .or_else(|| v.get("task"))
                })
                .and_then(|v| v.as_str())
                .unwrap_or(tool_name);
            description.chars().take(120).collect()
        }
        "todowrite" | "todo_write" | "todoread" | "todo_read" | "todo_management" => {
            let count = parse_todos(input).len();
            if count > 0 {
                format!("Todo list updated ({})", count)
            } else {
                "Todo list updated".to_string()
            }
        }
        _ => tool_name.to_string(),
    }
}

fn parse_todos(input: Option<&Value>) -> Vec<TodoItem> {
    let Some(input) = input else {
        return Vec::new();
    };

    let Some(todos) = input.get("todos").and_then(|value| value.as_array()) else {
        return Vec::new();
    };

    todos
        .iter()
        .filter_map(|todo| {
            let content = todo
                .get("content")
                .and_then(|value| value.as_str())?
                .trim()
                .to_string();
            if content.is_empty() {
                return None;
            }

            let status = parse_todo_status(todo.get("status").and_then(|value| value.as_str()));
            let active_form = todo
                .get("active_form")
                .or_else(|| todo.get("activeForm"))
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_string();

            Some(TodoItem {
                content,
                status,
                active_form,
            })
        })
        .collect()
}

fn parse_todo_status(status: Option<&str>) -> TodoStatus {
    match status.unwrap_or("").to_lowercase().as_str() {
        "completed" | "done" => TodoStatus::Completed,
        "in_progress" | "inprogress" | "in-progress" | "running" | "active" => {
            TodoStatus::InProgress
        }
        _ => TodoStatus::Pending,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn map_todo_write_to_todo_management() {
        let input = json!({
            "todos": [
                {
                    "content": "Ship timeline UX",
                    "status": "in_progress",
                    "activeForm": "Shipping timeline UX"
                }
            ]
        });

        let action = map_tool_to_action("TodoWrite", Some(&input));

        match action {
            ActionType::TodoManagement { todos, operation } => {
                assert_eq!(operation, "update");
                assert_eq!(todos.len(), 1);
                assert_eq!(todos[0].content, "Ship timeline UX");
                assert!(matches!(todos[0].status, TodoStatus::InProgress));
                assert_eq!(todos[0].active_form, "Shipping timeline UX");
            }
            _ => panic!("expected todo_management action"),
        }
    }

    #[test]
    fn map_task_to_task_create() {
        let input = json!({ "prompt": "Initialize repository" });
        let action = map_tool_to_action("Task", Some(&input));

        match action {
            ActionType::TaskCreate { description } => {
                assert_eq!(description, "Initialize repository");
            }
            _ => panic!("expected task_create action"),
        }
    }

    #[test]
    fn serialize_pending_approval_status() {
        let status = ToolStatus::PendingApproval {
            approval_id: "toolu_123".to_string(),
            requested_at: "2026-02-08T10:00:00Z".to_string(),
            timeout_at: "2026-02-08T10:05:00Z".to_string(),
        };

        let value = serde_json::to_value(status).expect("serialize pending approval");
        assert_eq!(value["status"], "pending_approval");
        assert_eq!(value["approval_id"], "toolu_123");
    }
}
