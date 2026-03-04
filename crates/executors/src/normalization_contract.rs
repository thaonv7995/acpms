use crate::sdk_normalized_types::{ActionType, NormalizedEntry, NormalizedEntryType, ToolStatus};
use chrono::DateTime;

pub fn validate_sdk_normalized_entry(entry: &NormalizedEntry) -> Result<(), String> {
    if let Some(ts) = entry.timestamp.as_deref() {
        DateTime::parse_from_rfc3339(ts)
            .map_err(|e| format!("invalid timestamp '{}': {}", ts, e))?;
    }

    match &entry.entry_type {
        NormalizedEntryType::AssistantMessage
        | NormalizedEntryType::SystemMessage
        | NormalizedEntryType::Thinking => {
            if entry.content.trim().is_empty() {
                return Err("content must not be empty".to_string());
            }
        }
        NormalizedEntryType::ToolUse {
            tool_name,
            action_type,
            status,
        } => {
            if tool_name.trim().is_empty() {
                return Err("tool_use.tool_name must not be empty".to_string());
            }
            validate_action_type(action_type)?;
            validate_tool_status(status)?;
        }
        NormalizedEntryType::NextAction { text } => {
            if text.trim().is_empty() {
                return Err("next_action.text must not be empty".to_string());
            }
        }
        NormalizedEntryType::TokenUsageInfo {
            input_tokens,
            output_tokens,
            total_tokens,
            model_context_window,
        } => {
            if *input_tokens == 0 && *output_tokens == 0 && total_tokens.unwrap_or(0) == 0 {
                return Err(
                    "token_usage_info must contain at least one non-zero token count".to_string(),
                );
            }

            if let Some(context_window) = model_context_window {
                if *context_window == 0 {
                    return Err("token_usage_info.model_context_window must be > 0".to_string());
                }
            }
        }
        NormalizedEntryType::UserAnsweredQuestions { question, answer } => {
            if question.trim().is_empty() {
                return Err("user_answered_questions.question must not be empty".to_string());
            }
            if answer.trim().is_empty() {
                return Err("user_answered_questions.answer must not be empty".to_string());
            }
        }
    }

    Ok(())
}

fn validate_action_type(action: &ActionType) -> Result<(), String> {
    match action {
        ActionType::FileRead { path } | ActionType::FileEdit { path, .. } => {
            if path.trim().is_empty() {
                return Err("file action path must not be empty".to_string());
            }
        }
        ActionType::CommandRun { command, .. } => {
            if command.trim().is_empty() {
                return Err("command_run.command must not be empty".to_string());
            }
        }
        ActionType::Search { query } => {
            if query.trim().is_empty() {
                return Err("search.query must not be empty".to_string());
            }
        }
        ActionType::WebFetch { url } => {
            if url.trim().is_empty() {
                return Err("web_fetch.url must not be empty".to_string());
            }
        }
        ActionType::TaskCreate { description } => {
            if description.trim().is_empty() {
                return Err("task_create.description must not be empty".to_string());
            }
        }
        ActionType::PlanPresentation { plan } => {
            if plan.trim().is_empty() {
                return Err("plan_presentation.plan must not be empty".to_string());
            }
        }
        ActionType::TodoManagement { operation, .. } => {
            if operation.trim().is_empty() {
                return Err("todo_management.operation must not be empty".to_string());
            }
        }
        ActionType::Tool { tool_name, .. } => {
            if tool_name.trim().is_empty() {
                return Err("tool.tool_name must not be empty".to_string());
            }
        }
        ActionType::Other { .. } => {}
    }

    Ok(())
}

fn validate_tool_status(status: &ToolStatus) -> Result<(), String> {
    if let ToolStatus::PendingApproval {
        approval_id,
        requested_at,
        timeout_at,
    } = status
    {
        if approval_id.trim().is_empty() {
            return Err("pending_approval.approval_id must not be empty".to_string());
        }
        if requested_at.trim().is_empty() {
            return Err("pending_approval.requested_at must not be empty".to_string());
        }
        if timeout_at.trim().is_empty() {
            return Err("pending_approval.timeout_at must not be empty".to_string());
        }
        DateTime::parse_from_rfc3339(requested_at).map_err(|e| {
            format!(
                "pending_approval.requested_at must be RFC3339 timestamp: {}",
                e
            )
        })?;
        DateTime::parse_from_rfc3339(timeout_at).map_err(|e| {
            format!(
                "pending_approval.timeout_at must be RFC3339 timestamp: {}",
                e
            )
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk_normalized_types::{
        ActionType, NormalizedEntry, NormalizedEntryType, ToolStatus,
    };

    #[test]
    fn validate_assistant_message_requires_content() {
        let entry = NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::AssistantMessage,
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_token_usage_accepts_non_zero() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::TokenUsageInfo {
                input_tokens: 100,
                output_tokens: 20,
                total_tokens: Some(120),
                model_context_window: Some(200_000),
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_ok());
    }

    #[test]
    fn validate_tool_use_rejects_empty_command() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "Bash".to_string(),
                action_type: ActionType::CommandRun {
                    command: String::new(),
                    result: None,
                },
                status: ToolStatus::Success,
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_tool_use_accepts_search_action() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "Grep".to_string(),
                action_type: ActionType::Search {
                    query: "TODO".to_string(),
                },
                status: ToolStatus::Success,
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_ok());
    }

    #[test]
    fn validate_tool_use_accepts_file_read_action() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "Read".to_string(),
                action_type: ActionType::FileRead {
                    path: "src/main.rs".to_string(),
                },
                status: ToolStatus::Success,
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_ok());
    }

    #[test]
    fn validate_tool_use_rejects_empty_file_read_path() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "Read".to_string(),
                action_type: ActionType::FileRead {
                    path: "   ".to_string(),
                },
                status: ToolStatus::Success,
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_tool_use_rejects_empty_file_edit_path() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "Edit".to_string(),
                action_type: ActionType::FileEdit {
                    path: "".to_string(),
                    changes: vec![],
                },
                status: ToolStatus::Success,
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_tool_use_rejects_empty_task_create_description() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "Task".to_string(),
                action_type: ActionType::TaskCreate {
                    description: "   ".to_string(),
                },
                status: ToolStatus::Success,
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_tool_use_rejects_empty_plan_presentation_text() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "Plan".to_string(),
                action_type: ActionType::PlanPresentation {
                    plan: "".to_string(),
                },
                status: ToolStatus::Success,
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_tool_use_rejects_empty_todo_operation() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "TodoWrite".to_string(),
                action_type: ActionType::TodoManagement {
                    todos: vec![],
                    operation: "  ".to_string(),
                },
                status: ToolStatus::Success,
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_tool_use_rejects_empty_generic_tool_name() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "Custom".to_string(),
                action_type: ActionType::Tool {
                    tool_name: "".to_string(),
                    arguments: None,
                    result: None,
                },
                status: ToolStatus::Success,
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_tool_use_rejects_empty_search_query() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "Grep".to_string(),
                action_type: ActionType::Search {
                    query: "  ".to_string(),
                },
                status: ToolStatus::Success,
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_tool_use_rejects_empty_web_fetch_url() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "WebFetch".to_string(),
                action_type: ActionType::WebFetch {
                    url: "   ".to_string(),
                },
                status: ToolStatus::Success,
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_next_action_rejects_empty_text() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::NextAction {
                text: "   ".to_string(),
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_user_answered_questions_rejects_empty_values() {
        let missing_question = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::UserAnsweredQuestions {
                question: "".to_string(),
                answer: "yes".to_string(),
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&missing_question).is_err());

        let missing_answer = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::UserAnsweredQuestions {
                question: "Deploy now?".to_string(),
                answer: " ".to_string(),
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&missing_answer).is_err());
    }

    #[test]
    fn validate_pending_approval_rejects_missing_fields() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "Bash".to_string(),
                action_type: ActionType::CommandRun {
                    command: "npm test".to_string(),
                    result: None,
                },
                status: ToolStatus::PendingApproval {
                    approval_id: "".to_string(),
                    requested_at: "".to_string(),
                    timeout_at: "".to_string(),
                },
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_pending_approval_accepts_complete_fields() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "Bash".to_string(),
                action_type: ActionType::CommandRun {
                    command: "npm test".to_string(),
                    result: None,
                },
                status: ToolStatus::PendingApproval {
                    approval_id: "approval-123".to_string(),
                    requested_at: "2026-02-26T10:00:00Z".to_string(),
                    timeout_at: "2026-02-26T10:05:00Z".to_string(),
                },
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_ok());
    }

    #[test]
    fn validate_pending_approval_rejects_invalid_requested_at_timestamp() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "Bash".to_string(),
                action_type: ActionType::CommandRun {
                    command: "echo hello".to_string(),
                    result: None,
                },
                status: ToolStatus::PendingApproval {
                    approval_id: "approval-1".to_string(),
                    requested_at: "not-a-timestamp".to_string(),
                    timeout_at: "2026-02-26T10:05:00Z".to_string(),
                },
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_pending_approval_rejects_invalid_timeout_at_timestamp() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-02-26T10:00:00Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "Bash".to_string(),
                action_type: ActionType::CommandRun {
                    command: "echo hello".to_string(),
                    result: None,
                },
                status: ToolStatus::PendingApproval {
                    approval_id: "approval-1".to_string(),
                    requested_at: "2026-02-26T10:00:00Z".to_string(),
                    timeout_at: "invalid-timeout".to_string(),
                },
            },
            content: String::new(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }

    #[test]
    fn validate_rejects_invalid_timestamp() {
        let entry = NormalizedEntry {
            timestamp: Some("not-a-timestamp".to_string()),
            entry_type: NormalizedEntryType::AssistantMessage,
            content: "hello".to_string(),
        };
        assert!(validate_sdk_normalized_entry(&entry).is_err());
    }
}
