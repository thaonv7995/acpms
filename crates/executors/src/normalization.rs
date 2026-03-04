use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use once_cell::sync::Lazy;

/// Normalized log entry extracted from raw agent logs
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum NormalizedEntry {
    Action(ActionType),
    AggregatedAction(AggregatedAction),
    SubagentSpawn(SubagentSpawn),
    FileChange(FileChange),
    TodoItem(TodoItem),
    ToolStatus(ToolStatus),
}

/// Tool action/invocation extracted from logs
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ActionType {
    pub tool_name: String,
    pub action: String,
    pub target: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub line_number: usize,
}

/// Aggregated consecutive operations
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AggregatedAction {
    pub tool_name: String,
    pub action: String,
    pub operations: Vec<ActionOperation>,
    pub start_line: usize,
    pub end_line: usize,
    pub timestamp_start: DateTime<Utc>,
    pub timestamp_end: DateTime<Utc>,
    pub total_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ActionOperation {
    pub target: Option<String>,
    pub line_number: usize,
    pub timestamp: DateTime<Utc>,
}

/// Subagent spawn tracking
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SubagentSpawn {
    pub child_attempt_id: Uuid,
    pub task_description: String,
    pub tool_use_id: String,
    pub timestamp: DateTime<Utc>,
    pub line_number: usize,
}

/// File change operation (create/modify/delete/rename)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FileChange {
    pub path: String,
    pub change_type: FileChangeType,
    pub lines_added: Option<usize>,
    pub lines_removed: Option<usize>,
    pub timestamp: DateTime<Utc>,
    pub line_number: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
    Renamed { from: String },
}

/// Todo item extracted from agent output
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TodoItem {
    pub status: TodoStatus,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub line_number: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

/// Tool execution status (success/failure)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ToolStatus {
    pub tool_name: String,
    pub status: ExecutionStatus,
    pub error_message: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub line_number: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ExecutionStatus {
    Success,
    Failed,
    Cancelled,
}

/// Trait for getting type information from normalized entries
pub trait NormalizedEntryType {
    fn entry_type(&self) -> &'static str;
    fn line_number(&self) -> usize;
}

impl NormalizedEntryType for NormalizedEntry {
    fn entry_type(&self) -> &'static str {
        match self {
            NormalizedEntry::Action(_) => "action",
            NormalizedEntry::AggregatedAction(_) => "aggregated_action",
            NormalizedEntry::SubagentSpawn(_) => "subagent_spawn",
            NormalizedEntry::FileChange(_) => "file_change",
            NormalizedEntry::TodoItem(_) => "todo_item",
            NormalizedEntry::ToolStatus(_) => "tool_status",
        }
    }

    fn line_number(&self) -> usize {
        match self {
            NormalizedEntry::Action(a) => a.line_number,
            NormalizedEntry::AggregatedAction(a) => a.start_line,
            NormalizedEntry::SubagentSpawn(s) => s.line_number,
            NormalizedEntry::FileChange(f) => f.line_number,
            NormalizedEntry::TodoItem(t) => t.line_number,
            NormalizedEntry::ToolStatus(s) => s.line_number,
        }
    }
}

// Regex patterns for log normalization
static ACTION_PATTERN: Lazy<Option<Regex>> =
    Lazy::new(|| Regex::new(r"Using tool:\s+(\w+)(?:\s+(.+))?").ok());

static FILE_CHANGE_PATTERN: Lazy<Option<Regex>> = Lazy::new(|| {
    Regex::new(r"(Created|Modified|Deleted|Renamed):\s+([^\s]+)(?:\s+\(\+(\d+),\s*-(\d+)\))?").ok()
});

static TODO_PATTERN: Lazy<Option<Regex>> =
    Lazy::new(|| Regex::new(r"^\s*-\s+\[([ xX])\]\s+(.+)$").ok());

static TOOL_STATUS_PATTERN: Lazy<Option<Regex>> =
    Lazy::new(|| Regex::new(r"^([✓✗])\s+(\w+)\s+(completed|failed|cancelled)(?::\s*(.+))?$").ok());

/// Parse a log line and extract all possible normalized entries
pub fn normalize_log_line(
    line: &str,
    line_number: usize,
    _is_stderr: bool,
) -> Vec<NormalizedEntry> {
    // Quick bailout for empty or very short lines
    if line.is_empty() || line.len() < 10 {
        return Vec::new();
    }

    let mut entries = Vec::new();

    // Try each pattern
    if let Some(action) = parse_action(line, line_number) {
        entries.push(NormalizedEntry::Action(action));
    }

    if let Some(file_change) = parse_file_change(line, line_number) {
        entries.push(NormalizedEntry::FileChange(file_change));
    }

    if let Some(todo) = parse_todo_item(line, line_number) {
        entries.push(NormalizedEntry::TodoItem(todo));
    }

    if let Some(status) = parse_tool_status(line, line_number) {
        entries.push(NormalizedEntry::ToolStatus(status));
    }

    entries
}

fn parse_action(line: &str, line_number: usize) -> Option<ActionType> {
    let caps = ACTION_PATTERN.as_ref()?.captures(line)?;

    let tool_name = caps.get(1)?.as_str().to_string();
    let target = caps.get(2).map(|m| m.as_str().trim().to_string());

    Some(ActionType {
        tool_name: tool_name.clone(),
        action: tool_name.to_lowercase(),
        target,
        timestamp: Utc::now(),
        line_number,
    })
}

fn parse_file_change(line: &str, line_number: usize) -> Option<FileChange> {
    let caps = FILE_CHANGE_PATTERN.as_ref()?.captures(line)?;

    let change_str = caps.get(1)?.as_str();
    let path = caps.get(2)?.as_str().to_string();

    let change_type = match change_str {
        "Created" => FileChangeType::Created,
        "Modified" => FileChangeType::Modified,
        "Deleted" => FileChangeType::Deleted,
        "Renamed" => {
            // For renamed, we'd need more info. For now, treat as modified
            FileChangeType::Modified
        }
        _ => FileChangeType::Modified,
    };

    let lines_added = caps.get(3).and_then(|m| m.as_str().parse::<usize>().ok());
    let lines_removed = caps.get(4).and_then(|m| m.as_str().parse::<usize>().ok());

    Some(FileChange {
        path,
        change_type,
        lines_added,
        lines_removed,
        timestamp: Utc::now(),
        line_number,
    })
}

fn parse_todo_item(line: &str, line_number: usize) -> Option<TodoItem> {
    let caps = TODO_PATTERN.as_ref()?.captures(line)?;

    let status_char = caps.get(1)?.as_str();
    let content = caps.get(2)?.as_str().to_string();

    let status = match status_char {
        "x" | "X" => TodoStatus::Completed,
        " " => TodoStatus::Pending,
        _ => TodoStatus::Pending,
    };

    Some(TodoItem {
        status,
        content,
        timestamp: Utc::now(),
        line_number,
    })
}

fn parse_tool_status(line: &str, line_number: usize) -> Option<ToolStatus> {
    let caps = TOOL_STATUS_PATTERN.as_ref()?.captures(line)?;

    let status_symbol = caps.get(1)?.as_str();
    let tool_name = caps.get(2)?.as_str().to_string();
    let status_str = caps.get(3)?.as_str();
    let error_message = caps.get(4).map(|m| m.as_str().trim().to_string());

    let status = match (status_symbol, status_str) {
        ("✓", "completed") => ExecutionStatus::Success,
        ("✗", "failed") => ExecutionStatus::Failed,
        (_, "cancelled") => ExecutionStatus::Cancelled,
        _ => ExecutionStatus::Failed,
    };

    Some(ToolStatus {
        tool_name,
        status,
        error_message,
        timestamp: Utc::now(),
        line_number,
    })
}

/// LogNormalizer aggregates consecutive similar operations
pub struct LogNormalizer;

impl LogNormalizer {
    pub fn new() -> Self {
        Self
    }

    /// Aggregate consecutive Read/Grep/Glob operations
    /// Groups ≥3 consecutive operations of same type into AggregatedAction
    pub fn aggregate_consecutive_actions(
        &self,
        entries: &[NormalizedEntry],
    ) -> Vec<NormalizedEntry> {
        let mut result = Vec::new();
        let mut buffer: Vec<ActionType> = Vec::new();
        let mut current_tool: Option<String> = None;

        for entry in entries {
            match entry {
                NormalizedEntry::Action(action) => {
                    let tool_name = &action.tool_name;

                    // Only aggregate Read, Grep, Glob (file/search operations)
                    if matches!(tool_name.as_str(), "Read" | "Grep" | "Glob") {
                        if Some(tool_name) == current_tool.as_ref() {
                            // Same tool - add to buffer
                            buffer.push(action.clone());
                        } else {
                            // Tool changed - flush previous buffer
                            Self::flush_buffer(&mut result, &mut buffer, &current_tool);
                            buffer = vec![action.clone()];
                            current_tool = Some(tool_name.clone());
                        }
                    } else {
                        // Non-aggregatable action - flush buffer and add directly
                        Self::flush_buffer(&mut result, &mut buffer, &current_tool);
                        buffer.clear();
                        current_tool = None;
                        result.push(NormalizedEntry::Action(action.clone()));
                    }
                }
                other => {
                    // Non-action entry - flush buffer
                    Self::flush_buffer(&mut result, &mut buffer, &current_tool);
                    buffer.clear();
                    current_tool = None;
                    result.push(other.clone());
                }
            }
        }

        // Final flush
        Self::flush_buffer(&mut result, &mut buffer, &current_tool);

        result
    }

    fn flush_buffer(
        result: &mut Vec<NormalizedEntry>,
        buffer: &mut Vec<ActionType>,
        _current_tool: &Option<String>,
    ) {
        if buffer.is_empty() {
            return;
        }

        // Only aggregate if ≥3 operations
        if buffer.len() >= 3 {
            if let Some(aggregated) = Self::create_aggregated_action(buffer.clone()) {
                result.push(aggregated);
            } else {
                for action in buffer.drain(..) {
                    result.push(NormalizedEntry::Action(action));
                }
                return;
            }
        } else {
            // Keep individual entries if <3
            for action in buffer.drain(..) {
                result.push(NormalizedEntry::Action(action));
            }
        }

        buffer.clear();
    }

    fn create_aggregated_action(actions: Vec<ActionType>) -> Option<NormalizedEntry> {
        let first = actions.first()?;
        let last = actions.last()?;
        let operations: Vec<ActionOperation> = actions
            .iter()
            .map(|a| ActionOperation {
                target: a.target.clone(),
                line_number: a.line_number,
                timestamp: a.timestamp,
            })
            .collect();

        Some(NormalizedEntry::AggregatedAction(AggregatedAction {
            tool_name: first.tool_name.clone(),
            action: first.action.clone(),
            operations,
            start_line: first.line_number,
            end_line: last.line_number,
            timestamp_start: first.timestamp,
            timestamp_end: last.timestamp,
            total_count: actions.len(),
        }))
    }
}

impl Default for LogNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_type_matching() {
        let action = NormalizedEntry::Action(ActionType {
            tool_name: "Read".into(),
            action: "read".into(),
            target: Some("/path/file".into()),
            timestamp: Utc::now(),
            line_number: 1,
        });

        assert_eq!(action.entry_type(), "action");
        assert_eq!(action.line_number(), 1);
    }

    #[test]
    fn test_file_change_variants() {
        let created = FileChange {
            path: "src/main.rs".into(),
            change_type: FileChangeType::Created,
            lines_added: Some(10),
            lines_removed: None,
            timestamp: Utc::now(),
            line_number: 5,
        };

        assert_eq!(created.path, "src/main.rs");
    }

    #[test]
    fn test_parse_action() {
        let entries = normalize_log_line("Using tool: Read /path/to/file.rs", 10, false);
        assert_eq!(entries.len(), 1);

        if let NormalizedEntry::Action(action) = &entries[0] {
            assert_eq!(action.tool_name, "Read");
            assert_eq!(action.target, Some("/path/to/file.rs".to_string()));
            assert_eq!(action.line_number, 10);
        } else {
            panic!("Expected Action entry");
        }
    }

    #[test]
    fn test_parse_file_change() {
        let entries = normalize_log_line("Modified: src/main.rs (+15, -3)", 20, false);
        assert_eq!(entries.len(), 1);

        if let NormalizedEntry::FileChange(fc) = &entries[0] {
            assert_eq!(fc.path, "src/main.rs");
            assert_eq!(fc.lines_added, Some(15));
            assert_eq!(fc.lines_removed, Some(3));
        } else {
            panic!("Expected FileChange entry");
        }
    }

    #[test]
    fn test_parse_todo_pending() {
        let entries = normalize_log_line("- [ ] Implement feature X", 30, false);
        assert_eq!(entries.len(), 1);

        if let NormalizedEntry::TodoItem(todo) = &entries[0] {
            assert_eq!(todo.content, "Implement feature X");
            assert!(matches!(todo.status, TodoStatus::Pending));
        } else {
            panic!("Expected TodoItem entry");
        }
    }

    #[test]
    fn test_parse_todo_completed() {
        let entries = normalize_log_line("- [x] Complete task Y", 40, false);
        assert_eq!(entries.len(), 1);

        if let NormalizedEntry::TodoItem(todo) = &entries[0] {
            assert_eq!(todo.content, "Complete task Y");
            assert!(matches!(todo.status, TodoStatus::Completed));
        } else {
            panic!("Expected TodoItem entry");
        }
    }

    #[test]
    fn test_parse_tool_success() {
        let entries = normalize_log_line("✓ Bash completed", 50, false);
        assert_eq!(entries.len(), 1);

        if let NormalizedEntry::ToolStatus(status) = &entries[0] {
            assert_eq!(status.tool_name, "Bash");
            assert!(matches!(status.status, ExecutionStatus::Success));
            assert!(status.error_message.is_none());
        } else {
            panic!("Expected ToolStatus entry");
        }
    }

    #[test]
    fn test_parse_tool_failure() {
        let entries = normalize_log_line("✗ Edit failed: permission denied", 60, false);
        assert_eq!(entries.len(), 1);

        if let NormalizedEntry::ToolStatus(status) = &entries[0] {
            assert_eq!(status.tool_name, "Edit");
            assert!(matches!(status.status, ExecutionStatus::Failed));
            assert_eq!(status.error_message, Some("permission denied".to_string()));
        } else {
            panic!("Expected ToolStatus entry");
        }
    }

    #[test]
    fn test_bailout_short_lines() {
        assert_eq!(normalize_log_line("", 1, false).len(), 0);
        assert_eq!(normalize_log_line("short", 1, false).len(), 0);
    }

    #[test]
    fn test_no_match() {
        let entries = normalize_log_line("This is just a regular log line", 70, false);
        assert_eq!(entries.len(), 0);
    }
}

#[cfg(test)]
mod aggregation_tests {
    use super::*;

    #[test]
    fn test_aggregate_consecutive_reads() {
        let normalizer = LogNormalizer::new();

        let entries = vec![
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("file1.rs".into()),
                timestamp: Utc::now(),
                line_number: 1,
            }),
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("file2.rs".into()),
                timestamp: Utc::now(),
                line_number: 2,
            }),
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("file3.rs".into()),
                timestamp: Utc::now(),
                line_number: 3,
            }),
        ];

        let result = normalizer.aggregate_consecutive_actions(&entries);

        assert_eq!(result.len(), 1);
        if let NormalizedEntry::AggregatedAction(agg) = &result[0] {
            assert_eq!(agg.total_count, 3);
            assert_eq!(agg.tool_name, "Read");
            assert_eq!(agg.operations.len(), 3);
        } else {
            panic!("Expected AggregatedAction");
        }
    }

    #[test]
    fn test_no_aggregate_when_less_than_three() {
        let normalizer = LogNormalizer::new();

        let entries = vec![
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("file1.rs".into()),
                timestamp: Utc::now(),
                line_number: 1,
            }),
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("file2.rs".into()),
                timestamp: Utc::now(),
                line_number: 2,
            }),
        ];

        let result = normalizer.aggregate_consecutive_actions(&entries);

        // Should keep as 2 individual actions
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], NormalizedEntry::Action(_)));
        assert!(matches!(result[1], NormalizedEntry::Action(_)));
    }

    #[test]
    fn test_flush_on_tool_change() {
        let normalizer = LogNormalizer::new();

        let entries = vec![
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("file1.rs".into()),
                timestamp: Utc::now(),
                line_number: 1,
            }),
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("file2.rs".into()),
                timestamp: Utc::now(),
                line_number: 2,
            }),
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("file3.rs".into()),
                timestamp: Utc::now(),
                line_number: 3,
            }),
            NormalizedEntry::Action(ActionType {
                tool_name: "Grep".into(), // Tool changed
                action: "grep".into(),
                target: Some("pattern".into()),
                timestamp: Utc::now(),
                line_number: 4,
            }),
        ];

        let result = normalizer.aggregate_consecutive_actions(&entries);

        // Should have: 1 AggregatedAction (3 Reads) + 1 Grep
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], NormalizedEntry::AggregatedAction(_)));
        assert!(matches!(result[1], NormalizedEntry::Action(_)));
    }
}
