use crate::types::normalized_entry::*;
use acpms_db::models::AgentLog;
use regex::Regex;

#[allow(dead_code)]
pub struct LogNormalizer;

impl LogNormalizer {
    /// Transform raw agent logs to normalized entries
    #[allow(dead_code)]
    pub fn normalize_logs(logs: Vec<AgentLog>) -> Vec<NormalizedEntry> {
        logs.into_iter()
            .filter_map(|log| Self::parse_log_entry(&log))
            .collect()
    }

    #[allow(dead_code)]
    fn parse_log_entry(log: &AgentLog) -> Option<NormalizedEntry> {
        let content = &log.content;

        // Detect entry type from content patterns
        let entry_type = if Self::is_user_message(content) {
            NormalizedEntryType::UserMessage
        } else if Self::is_assistant_message(content) {
            NormalizedEntryType::AssistantMessage
        } else if let Some(tool_use) = Self::parse_tool_use(content) {
            tool_use
        } else if Self::is_system_message(content) {
            NormalizedEntryType::SystemMessage
        } else if let Some(error) = Self::parse_error(content) {
            error
        } else if Self::is_thinking(content) {
            NormalizedEntryType::Thinking
        } else {
            // Unknown format - skip
            return None;
        };

        Some(NormalizedEntry {
            timestamp: Some(log.created_at.to_rfc3339()),
            entry_type,
            content: content.clone(),
        })
    }

    #[allow(dead_code)]
    fn is_user_message(content: &str) -> bool {
        content.starts_with("[USER]")
            || content.starts_with("User:")
            || content.contains(">>> User input:")
    }

    #[allow(dead_code)]
    fn is_assistant_message(content: &str) -> bool {
        content.starts_with("[ASSISTANT]")
            || content.starts_with("Assistant:")
            || content.contains("<<< Assistant response:")
    }

    #[allow(dead_code)]
    fn is_system_message(content: &str) -> bool {
        content.starts_with("[SYSTEM]") || content.contains("<system>")
    }

    #[allow(dead_code)]
    fn is_thinking(content: &str) -> bool {
        content.starts_with("[THINKING]") || content.contains("<thinking>")
    }

    #[allow(dead_code)]
    fn parse_tool_use(content: &str) -> Option<NormalizedEntryType> {
        // Pattern: [TOOL: Read] /path/to/file.rs
        let re = Regex::new(r"\[TOOL:\s*(\w+)\]").ok()?;
        let caps = re.captures(content)?;
        let tool_name = caps.get(1)?.as_str().to_string();

        // Determine action type based on tool name
        let action_type = match tool_name.as_str() {
            "Read" => Self::parse_file_read(content)?,
            "Edit" | "Write" => Self::parse_file_edit(content)?,
            "Bash" => Self::parse_command_run(content)?,
            "Grep" | "Glob" => Self::parse_search(content)?,
            _ => ActionType::Other {
                description: tool_name.clone(),
            },
        };

        // Parse status from content
        let status = if content.contains("✓") || content.contains("success") {
            ToolStatus::Success
        } else if content.contains("✗") || content.contains("failed") {
            ToolStatus::Failed
        } else {
            ToolStatus::Created
        };

        Some(NormalizedEntryType::ToolUse {
            tool_name,
            action_type,
            status,
        })
    }

    #[allow(dead_code)]
    fn parse_file_read(content: &str) -> Option<ActionType> {
        // Extract file path - pattern: Read] /path/to/file or Read /path/to/file
        let re = Regex::new(r"Read\]\s+(.+?)(?:\s+[✓✗]|$)").ok()?;
        let path = re
            .captures(content)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().trim().to_string())?;

        Some(ActionType::FileRead { path })
    }

    #[allow(dead_code)]
    fn parse_file_edit(content: &str) -> Option<ActionType> {
        // Extract file path - pattern: Edit /path/to/file
        let re = Regex::new(r"(?:Edit|Write)\s+(.+)").ok()?;
        let path = re.captures(content)?.get(1)?.as_str().trim().to_string();

        // TODO: Parse actual file changes from subsequent lines
        // For now, return empty changes array
        Some(ActionType::FileEdit {
            path,
            changes: vec![],
        })
    }

    #[allow(dead_code)]
    fn parse_command_run(content: &str) -> Option<ActionType> {
        // Extract command - pattern: Bash: command here
        let re = Regex::new(r"Bash:\s*(.+)").ok()?;
        let command = re.captures(content)?.get(1)?.as_str().trim().to_string();

        Some(ActionType::CommandRun {
            command,
            result: None,
        })
    }

    #[allow(dead_code)]
    fn parse_search(content: &str) -> Option<ActionType> {
        // Extract query - pattern: Grep: query or Glob: query
        let re = Regex::new(r"(?:Grep|Glob):\s*(.+)").ok()?;
        let query = re.captures(content)?.get(1)?.as_str().trim().to_string();

        Some(ActionType::Search { query })
    }

    #[allow(dead_code)]
    fn parse_error(content: &str) -> Option<NormalizedEntryType> {
        if !content.contains("[ERROR]") && !content.contains("Error:") {
            return None;
        }

        let error_type = if content.contains("API") {
            NormalizedEntryError::ApiError
        } else if content.contains("Tool") {
            NormalizedEntryError::ToolError
        } else {
            NormalizedEntryError::Unknown
        };

        Some(NormalizedEntryType::ErrorMessage { error_type })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_user_message_detection() {
        let content = "[USER] Fix the login bug";
        assert!(LogNormalizer::is_user_message(content));
    }

    #[test]
    fn test_assistant_message_detection() {
        let content = "[ASSISTANT] I'll help you fix that";
        assert!(LogNormalizer::is_assistant_message(content));
    }

    #[test]
    fn test_tool_use_parsing() {
        let content = "[TOOL: Read] src/auth/login.rs ✓";
        let result = LogNormalizer::parse_tool_use(content);

        assert!(result.is_some());
        if let Some(NormalizedEntryType::ToolUse {
            tool_name,
            action_type,
            status,
        }) = result
        {
            assert_eq!(tool_name, "Read");
            assert!(matches!(action_type, ActionType::FileRead { .. }));
            assert!(matches!(status, ToolStatus::Success));
        }
    }
}
