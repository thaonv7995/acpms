//! PA-302: Parse agent output for tool_calls.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const VALID_TASK_TYPES: &[&str] = &[
    "feature",
    "bug",
    "refactor",
    "docs",
    "test",
    "chore",
    "hotfix",
    "spike",
    "small_task",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct RawToolLine {
    tool: String,
    args: serde_json::Value,
}

/// Parse a line of output for tool call JSON. Returns Some if valid tool call.
pub fn parse_tool_call_line(line: &str) -> Option<ToolCall> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let v: RawToolLine = serde_json::from_str(trimmed).ok()?;
    if v.tool != "create_requirement" && v.tool != "create_task" {
        return None;
    }
    let args = validate_and_normalize_args(&v.tool, &v.args)?;
    Some(ToolCall {
        id: format!("tc_{}", Uuid::new_v4()),
        name: v.tool,
        args,
    })
}

fn validate_and_normalize_args(tool: &str, args: &serde_json::Value) -> Option<serde_json::Value> {
    let obj = args.as_object()?;
    let mut out = serde_json::Map::new();

    if tool == "create_requirement" {
        let title = obj.get("title")?.as_str()?.to_string();
        let content = obj
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let priority = obj
            .get("priority")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase())
            .filter(|s| matches!(s.as_str(), "low" | "medium" | "high" | "critical"))
            .unwrap_or_else(|| "medium".to_string());
        out.insert("title".to_string(), serde_json::json!(title));
        out.insert("content".to_string(), serde_json::json!(content));
        out.insert("priority".to_string(), serde_json::json!(priority));
    } else if tool == "create_task" {
        let title = obj.get("title")?.as_str()?.to_string();
        let description = obj
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let task_type = obj
            .get("task_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase())
            .filter(|s| VALID_TASK_TYPES.contains(&s.as_str()))
            .unwrap_or_else(|| "feature".to_string());
        let requirement_id = obj
            .get("requirement_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(|u| u.to_string());
        let sprint_id = obj
            .get("sprint_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(|u| u.to_string());
        out.insert("title".to_string(), serde_json::json!(title));
        out.insert(
            "description".to_string(),
            serde_json::json!(description.unwrap_or_default()),
        );
        out.insert("task_type".to_string(), serde_json::json!(task_type));
        out.insert(
            "requirement_id".to_string(),
            serde_json::json!(requirement_id),
        );
        out.insert("sprint_id".to_string(), serde_json::json!(sprint_id));
    } else {
        return None;
    }

    Some(serde_json::Value::Object(out))
}
