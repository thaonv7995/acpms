use acpms_db::{models::*, PgPool};
use acpms_services::{ProjectService, RequirementService, TaskAttemptService, TaskService};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use crate::{
    api::{ApiResponse, TaskDto},
    error::{ApiError, ApiResult},
    middleware::{AuthUser, Permission, RbacChecker},
    routes::task_attempts,
    AppState,
};

const BREAKDOWN_STATUS_QUEUED: &str = "queued";
const BREAKDOWN_STATUS_RUNNING: &str = "running";
const BREAKDOWN_STATUS_REVIEW: &str = "review";
const BREAKDOWN_STATUS_CONFIRMED: &str = "confirmed";
const BREAKDOWN_STATUS_FAILED: &str = "failed";
const BREAKDOWN_STATUS_CANCELLED: &str = "cancelled";

#[derive(Debug, Clone, FromRow)]
struct RequirementBreakdownSessionRow {
    id: Uuid,
    project_id: Uuid,
    requirement_id: Uuid,
    created_by: Uuid,
    status: String,
    analysis: Option<Value>,
    impact: Option<Value>,
    plan: Option<Value>,
    proposed_tasks: Option<Value>,
    suggested_sprint_id: Option<Uuid>,
    error_message: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    confirmed_at: Option<DateTime<Utc>>,
    cancelled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct RequirementBreakdownSessionDto {
    pub id: Uuid,
    pub project_id: Uuid,
    pub requirement_id: Uuid,
    pub created_by: Uuid,
    pub status: String,
    pub analysis: Option<Value>,
    pub impact: Option<Value>,
    pub plan: Option<Value>,
    pub proposed_tasks: Option<Value>,
    pub suggested_sprint_id: Option<Uuid>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub confirmed_at: Option<String>,
    pub cancelled_at: Option<String>,
}

impl From<RequirementBreakdownSessionRow> for RequirementBreakdownSessionDto {
    fn from(row: RequirementBreakdownSessionRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            requirement_id: row.requirement_id,
            created_by: row.created_by,
            status: row.status,
            analysis: row.analysis,
            impact: row.impact,
            plan: row.plan,
            proposed_tasks: row.proposed_tasks,
            suggested_sprint_id: row.suggested_sprint_id,
            error_message: row.error_message,
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
            started_at: row.started_at.map(|v| v.to_rfc3339()),
            completed_at: row.completed_at.map(|v| v.to_rfc3339()),
            confirmed_at: row.confirmed_at.map(|v| v.to_rfc3339()),
            cancelled_at: row.cancelled_at.map(|v| v.to_rfc3339()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ConfirmRequirementBreakdownResponse {
    pub session: RequirementBreakdownSessionDto,
    pub tasks: Vec<TaskDto>,
}

#[derive(Debug, Serialize)]
pub struct ConfirmManualRequirementBreakdownResponse {
    pub tasks: Vec<TaskDto>,
}

#[derive(Debug, Deserialize, Default)]
pub struct StartRequirementTaskSequenceRequest {
    #[serde(default)]
    pub continue_on_failure: bool,
}

#[derive(Debug, Serialize)]
pub struct StartRequirementTaskSequenceResponse {
    pub run_id: Uuid,
    pub task_ids: Vec<Uuid>,
    pub total_tasks: usize,
    pub continue_on_failure: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakdownSprintAssignmentMode {
    Active,
    Selected,
    Backlog,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmRequirementBreakdownRequest {
    pub assignment_mode: BreakdownSprintAssignmentMode,
    pub sprint_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ManualBreakdownTaskDraftRequest {
    pub title: String,
    pub description: Option<String>,
    pub task_type: String,
    pub priority: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub kind: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmManualRequirementBreakdownRequest {
    pub assignment_mode: BreakdownSprintAssignmentMode,
    pub sprint_id: Option<Uuid>,
    pub tasks: Vec<ManualBreakdownTaskDraftRequest>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct BreakdownImpactItem {
    area: String,
    impact: String,
    risk: String,
    mitigation: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct BreakdownTaskDraft {
    title: String,
    description: String,
    task_type: String,
    #[serde(default)]
    estimate: Option<String>,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    kind: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct BreakdownProposal {
    analysis: Value,
    impact: Vec<BreakdownImpactItem>,
    plan: Value,
    tasks: Vec<BreakdownTaskDraft>,
    suggested_sprint_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct RawBreakdownProposal {
    analysis: Value,
    impact: Vec<BreakdownImpactItem>,
    plan: Value,
    tasks: Vec<RawBreakdownTask>,
    suggested_sprint_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct RawBreakdownTask {
    title: String,
    description: Option<String>,
    task_type: Option<String>,
    estimate: Option<String>,
    #[serde(default)]
    depends_on: Vec<String>,
    kind: Option<String>,
}

#[derive(Debug)]
struct BreakdownJob {
    session_id: Uuid,
    project_id: Uuid,
    requirement_id: Uuid,
    created_by: Uuid,
}

fn parse_supported_task_type(value: &str) -> Option<TaskType> {
    match value.trim().to_ascii_lowercase().as_str() {
        "feature" => Some(TaskType::Feature),
        "bug" => Some(TaskType::Bug),
        "refactor" => Some(TaskType::Refactor),
        "docs" => Some(TaskType::Docs),
        "test" => Some(TaskType::Test),
        "chore" => Some(TaskType::Chore),
        "hotfix" => Some(TaskType::Hotfix),
        "spike" => Some(TaskType::Spike),
        "small_task" => Some(TaskType::SmallTask),
        _ => None,
    }
}

fn parse_supported_priority(value: Option<&str>) -> Option<&'static str> {
    match value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("medium")
        .to_ascii_lowercase()
        .as_str()
    {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "critical" => Some("critical"),
        _ => None,
    }
}

fn task_type_as_str(task_type: TaskType) -> &'static str {
    match task_type {
        TaskType::Feature => "feature",
        TaskType::Bug => "bug",
        TaskType::Refactor => "refactor",
        TaskType::Docs => "docs",
        TaskType::Test => "test",
        TaskType::Chore => "chore",
        TaskType::Hotfix => "hotfix",
        TaskType::Spike => "spike",
        TaskType::SmallTask => "small_task",
        TaskType::Init => "spike",
        TaskType::Deploy => "chore",
    }
}

fn with_requirement_execution_metadata(
    metadata: &mut Value,
    requirement_id: Uuid,
    order: usize,
    total: usize,
    depends_on: Option<&[String]>,
) {
    let mut normalized = if metadata.is_object() {
        metadata.clone()
    } else {
        json!({ "manual_payload": metadata.clone() })
    };

    if let Some(metadata_obj) = normalized.as_object_mut() {
        metadata_obj.insert(
            "execution_group".to_string(),
            json!(format!("requirement:{}", requirement_id)),
        );
        metadata_obj.insert("execution_policy".to_string(), json!("sequential"));
        metadata_obj.insert("execution_order".to_string(), json!(order));
        metadata_obj.insert("execution_total".to_string(), json!(total));

        let mut execution = metadata_obj
            .get("execution")
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        execution.insert(
            "group".to_string(),
            json!(format!("requirement:{}", requirement_id)),
        );
        execution.insert("policy".to_string(), json!("sequential"));
        execution.insert("order".to_string(), json!(order));
        execution.insert("total".to_string(), json!(total));
        if let Some(depends_on) = depends_on.filter(|items| !items.is_empty()) {
            execution.insert("depends_on".to_string(), json!(depends_on));
        }
        metadata_obj.insert("execution".to_string(), Value::Object(execution));
    }

    *metadata = normalized;
}

fn is_breakdown_analysis_task(task: &Task) -> bool {
    let mode = task
        .metadata
        .get("breakdown_mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let kind = task
        .metadata
        .get("breakdown_kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    mode == "ai_support"
        || kind == "analysis_session"
        || task.title.trim_start().starts_with("[Breakdown]")
}

fn parse_execution_order(task: &Task) -> Option<u64> {
    let parse_value = |value: Option<&Value>| -> Option<u64> {
        match value {
            Some(Value::Number(num)) => num.as_u64(),
            Some(Value::String(raw)) => raw.trim().parse::<u64>().ok(),
            _ => None,
        }
    };

    parse_value(task.metadata.get("execution_order")).or_else(|| {
        task.metadata
            .get("execution")
            .and_then(Value::as_object)
            .and_then(|execution| parse_value(execution.get("order")))
    })
}

fn sort_requirement_tasks_for_sequence(tasks: &mut [Task]) {
    tasks.sort_by(|a, b| {
        let a_order = parse_execution_order(a).unwrap_or(u64::MAX);
        let b_order = parse_execution_order(b).unwrap_or(u64::MAX);
        a_order
            .cmp(&b_order)
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn task_status_is_eligible_for_requirement_sequence(status: TaskStatus) -> bool {
    matches!(status, TaskStatus::Todo | TaskStatus::InProgress)
}

fn attempt_status_is_terminal(status: AttemptStatus) -> bool {
    matches!(
        status,
        AttemptStatus::Success | AttemptStatus::Failed | AttemptStatus::Cancelled
    )
}

async fn wait_for_attempt_terminal_status(
    attempt_service: &TaskAttemptService,
    attempt_id: Uuid,
) -> Option<AttemptStatus> {
    const MAX_POLLS: usize = 7200; // 6h with 3s interval.
    for _ in 0..MAX_POLLS {
        let attempt = attempt_service
            .get_attempt(attempt_id)
            .await
            .ok()
            .flatten()?;
        if attempt_status_is_terminal(attempt.status) {
            return Some(attempt.status);
        }
        sleep(Duration::from_secs(3)).await;
    }
    None
}

async fn run_requirement_task_sequence(
    state: AppState,
    auth_user: AuthUser,
    project_id: Uuid,
    requirement_id: Uuid,
    run_id: Uuid,
    task_ids: Vec<Uuid>,
    continue_on_failure: bool,
) {
    let task_service = TaskService::new(state.db.clone());
    let attempt_service = TaskAttemptService::new(state.db.clone());

    tracing::info!(
        run_id = %run_id,
        project_id = %project_id,
        requirement_id = %requirement_id,
        task_count = task_ids.len(),
        continue_on_failure,
        "Starting requirement task sequence run"
    );

    for (idx, task_id) in task_ids.into_iter().enumerate() {
        let maybe_task = match task_service.get_task(task_id).await {
            Ok(task) => task,
            Err(error) => {
                tracing::error!(
                    run_id = %run_id,
                    task_id = %task_id,
                    error = %error,
                    "Failed to fetch task during sequence run"
                );
                if continue_on_failure {
                    continue;
                }
                break;
            }
        };

        let task = match maybe_task {
            Some(task) => task,
            None => {
                tracing::warn!(
                    run_id = %run_id,
                    task_id = %task_id,
                    "Task no longer exists, skipping sequence item"
                );
                continue;
            }
        };

        if task.project_id != project_id || task.requirement_id != Some(requirement_id) {
            tracing::warn!(
                run_id = %run_id,
                task_id = %task_id,
                "Task no longer belongs to target project/requirement, skipping"
            );
            continue;
        }

        if !task_status_is_eligible_for_requirement_sequence(task.status) {
            tracing::info!(
                run_id = %run_id,
                task_id = %task_id,
                status = ?task.status,
                "Skipping task due to non-eligible status"
            );
            continue;
        }

        let active_attempt = attempt_service
            .get_task_attempts(task_id)
            .await
            .ok()
            .and_then(|attempts| {
                attempts
                    .into_iter()
                    .find(|attempt| {
                        matches!(
                            attempt.status,
                            AttemptStatus::Queued | AttemptStatus::Running
                        )
                    })
                    .map(|attempt| attempt.id)
            });

        let attempt_id = if let Some(existing_attempt_id) = active_attempt {
            tracing::info!(
                run_id = %run_id,
                task_id = %task_id,
                attempt_id = %existing_attempt_id,
                index = idx + 1,
                "Waiting for existing active attempt in requirement sequence"
            );
            existing_attempt_id
        } else {
            match task_attempts::create_task_attempt(
                State(state.clone()),
                auth_user.clone(),
                Path(task_id),
            )
            .await
            {
                Ok((_status, Json(response))) => {
                    if let Some(created_attempt) = response.data {
                        tracing::info!(
                            run_id = %run_id,
                            task_id = %task_id,
                            attempt_id = %created_attempt.id,
                            index = idx + 1,
                            "Started task attempt in requirement sequence"
                        );
                        created_attempt.id
                    } else {
                        tracing::error!(
                            run_id = %run_id,
                            task_id = %task_id,
                            index = idx + 1,
                            "Attempt creation response missing data"
                        );
                        if continue_on_failure {
                            continue;
                        }
                        break;
                    }
                }
                Err(error) => {
                    tracing::error!(
                        run_id = %run_id,
                        task_id = %task_id,
                        index = idx + 1,
                        error = %error,
                        "Failed to start task attempt in requirement sequence"
                    );
                    if continue_on_failure {
                        continue;
                    }
                    break;
                }
            }
        };

        let terminal_status = wait_for_attempt_terminal_status(&attempt_service, attempt_id).await;
        match terminal_status {
            Some(status) if status == AttemptStatus::Success => {
                tracing::info!(
                    run_id = %run_id,
                    task_id = %task_id,
                    attempt_id = %attempt_id,
                    index = idx + 1,
                    "Requirement sequence item completed successfully"
                );
            }
            Some(status) => {
                tracing::warn!(
                    run_id = %run_id,
                    task_id = %task_id,
                    attempt_id = %attempt_id,
                    index = idx + 1,
                    status = ?status,
                    continue_on_failure,
                    "Requirement sequence item completed with non-success status"
                );
                if !continue_on_failure {
                    break;
                }
            }
            None => {
                tracing::error!(
                    run_id = %run_id,
                    task_id = %task_id,
                    attempt_id = %attempt_id,
                    index = idx + 1,
                    continue_on_failure,
                    "Requirement sequence timed out while waiting for attempt terminal status"
                );
                if !continue_on_failure {
                    break;
                }
            }
        }
    }

    tracing::info!(
        run_id = %run_id,
        project_id = %project_id,
        requirement_id = %requirement_id,
        "Finished requirement task sequence run"
    );
}

fn extract_json_object(raw: &str) -> Result<Value, ApiError> {
    if let Ok(value) = serde_json::from_str::<Value>(raw) {
        return Ok(value);
    }

    if let Some(start_idx) = raw.find('{') {
        if let Some(end_idx) = raw.rfind('}') {
            if end_idx > start_idx {
                let candidate = &raw[start_idx..=end_idx];
                if let Ok(value) = serde_json::from_str::<Value>(candidate) {
                    return Ok(value);
                }
            }
        }
    }

    Err(ApiError::Internal(
        "Agent output is not valid JSON object".to_string(),
    ))
}

fn normalize_breakdown_proposal(
    raw: RawBreakdownProposal,
    requirement_title: &str,
) -> Result<BreakdownProposal, ApiError> {
    if raw.impact.is_empty() {
        return Err(ApiError::Internal(
            "Breakdown output validation failed: impact is empty".to_string(),
        ));
    }
    if raw.tasks.is_empty() {
        return Err(ApiError::Internal(
            "Breakdown output validation failed: tasks is empty".to_string(),
        ));
    }
    if raw.tasks.len() > 20 {
        return Err(ApiError::Internal(
            "Breakdown output validation failed: too many tasks".to_string(),
        ));
    }

    let mut tasks = Vec::with_capacity(raw.tasks.len() + 1);
    let mut has_breakdown_task = false;

    for task in raw.tasks {
        let title = task.title.trim();
        if title.is_empty() {
            return Err(ApiError::Internal(
                "Breakdown output validation failed: task title is empty".to_string(),
            ));
        }
        let kind = task
            .kind
            .as_deref()
            .unwrap_or("implementation")
            .trim()
            .to_ascii_lowercase();

        let resolved_task_type = match task.task_type.as_deref() {
            Some(value) => parse_supported_task_type(value).ok_or_else(|| {
                ApiError::Internal(format!(
                    "Breakdown output validation failed: unsupported task_type '{}'",
                    value
                ))
            })?,
            None => {
                if kind == "analysis_session" {
                    TaskType::Spike
                } else {
                    TaskType::Feature
                }
            }
        };
        let mut normalized_kind = if kind == "analysis_session" {
            "analysis_session".to_string()
        } else {
            "implementation".to_string()
        };

        if normalized_kind == "analysis_session" {
            has_breakdown_task = true;
        }
        if title.starts_with("[Breakdown]") {
            normalized_kind = "analysis_session".to_string();
            has_breakdown_task = true;
        }

        tasks.push(BreakdownTaskDraft {
            title: title.to_string(),
            description: task
                .description
                .as_deref()
                .unwrap_or("Implement scoped task from requirement breakdown")
                .trim()
                .to_string(),
            task_type: task_type_as_str(resolved_task_type).to_string(),
            estimate: task.estimate.map(|v| v.trim().to_string()),
            depends_on: task
                .depends_on
                .into_iter()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect(),
            kind: normalized_kind,
        });
    }

    if !has_breakdown_task {
        tasks.insert(
            0,
            BreakdownTaskDraft {
                title: format!("[Breakdown] {}", requirement_title.trim()),
                description:
                    "AI analysis session for requirement scope, impact, and execution plan."
                        .to_string(),
                task_type: "spike".to_string(),
                estimate: Some("S".to_string()),
                depends_on: vec![],
                kind: "analysis_session".to_string(),
            },
        );
    }

    Ok(BreakdownProposal {
        analysis: raw.analysis,
        impact: raw.impact,
        plan: raw.plan,
        tasks,
        suggested_sprint_id: raw.suggested_sprint_id,
    })
}

fn parse_and_validate_breakdown_output(
    raw_output: &str,
    requirement_title: &str,
) -> Result<BreakdownProposal, ApiError> {
    let value = extract_json_object(raw_output)?;
    let parsed: RawBreakdownProposal = serde_json::from_value(value)
        .map_err(|e| ApiError::Internal(format!("Breakdown JSON schema mismatch: {}", e)))?;
    normalize_breakdown_proposal(parsed, requirement_title)
}

fn build_breakdown_attempt_instruction(requirement: &Requirement) -> String {
    format!(
        r#"
You are an AI business/system analyst helping break one requirement into implementable tasks.

Requirement title:
{title}

Requirement content:
{content}

STRICT RULES:
1) Analysis only. Do NOT edit files. Do NOT apply code changes.
2) Propose 3-12 implementation tasks. Keep each task small and reviewable.
3) Allowed task_type: feature, bug, refactor, docs, test, chore, hotfix, spike, small_task.
4) As each task is ready, emit one line in EXACT format:
BREAKDOWN_TASK {{"title":"...","description":"...","task_type":"feature","priority":"medium","kind":"implementation"}}
5) After emitting all BREAKDOWN_TASK lines, output one FINAL JSON object with schema:
{{
  "analysis": {{"summary":"...","current_system_findings":["..."],"assumptions":["..."]}},
  "impact": [{{"area":"backend|frontend|database|api|infra|security|testing|ops|other","impact":"...","risk":"low|medium|high","mitigation":"..."}}],
  "plan": {{"summary":"...","steps":["..."]}},
  "tasks": [
    {{"title":"...","description":"...","task_type":"feature","depends_on":[],"kind":"implementation"}}
  ],
  "suggested_sprint_id": null
}}
6) Do not include markdown fences around the final JSON.
7) Keep language concise and practical.
"#,
        title = requirement.title,
        content = requirement.content
    )
}

fn extract_streamed_breakdown_tasks_from_logs(logs: &[AgentLog]) -> Vec<RawBreakdownTask> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for log in logs {
        for line in log.content.lines() {
            let Some(marker_index) = line.find("BREAKDOWN_TASK") else {
                continue;
            };
            let Some(json_start_rel) = line[marker_index..].find('{') else {
                continue;
            };
            let json_start = marker_index + json_start_rel;
            let Some(json_end) = line.rfind('}') else {
                continue;
            };
            if json_end <= json_start {
                continue;
            }

            let candidate = &line[json_start..=json_end];
            let Ok(value) = serde_json::from_str::<Value>(candidate) else {
                continue;
            };

            let title = value
                .get("title")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .unwrap_or("");
            if title.is_empty() {
                continue;
            }

            let description = value
                .get("description")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned);
            let task_type = value
                .get("task_type")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned);
            let estimate = value
                .get("estimate")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned);
            let kind = value
                .get("kind")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned);
            let depends_on = value
                .get("depends_on")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| item.as_str().map(str::trim))
                        .filter(|item| !item.is_empty())
                        .map(ToOwned::to_owned)
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();

            let signature = format!(
                "{}|{}|{}",
                task_type
                    .as_deref()
                    .unwrap_or("feature")
                    .to_ascii_lowercase(),
                title.to_ascii_lowercase(),
                description
                    .as_deref()
                    .unwrap_or_default()
                    .to_ascii_lowercase()
            );
            if !seen.insert(signature) {
                continue;
            }

            out.push(RawBreakdownTask {
                title: title.to_string(),
                description,
                task_type,
                estimate,
                depends_on,
                kind,
            });
        }
    }

    out
}

async fn load_attempt_logs_for_breakdown(state: &AppState, attempt: &TaskAttempt) -> Vec<AgentLog> {
    let local_bytes = acpms_executors::read_attempt_log_file(attempt.id)
        .await
        .unwrap_or_default();

    let bytes = if local_bytes.is_empty() {
        if let Some(ref s3_key) = attempt.s3_log_key {
            state
                .storage_service
                .get_log_bytes_tail(s3_key, 20_000_000)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        local_bytes
    };

    acpms_executors::parse_jsonl_to_agent_logs(&bytes)
}

fn choose_breakdown_proposal_from_logs(
    logs: &[AgentLog],
    requirement: &Requirement,
    project_name: &str,
    project_type: ProjectType,
    suggested_sprint_id: Option<Uuid>,
) -> Result<(BreakdownProposal, String), ApiError> {
    let mut raw_candidates: Vec<String> = logs
        .iter()
        .rev()
        .map(|log| log.content.trim())
        .filter(|content| !content.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    let combined = logs
        .iter()
        .map(|log| log.content.as_str())
        .collect::<Vec<&str>>()
        .join("\n");
    if !combined.trim().is_empty() {
        raw_candidates.push(combined.clone());
    }

    for candidate in &raw_candidates {
        if let Ok(mut proposal) = parse_and_validate_breakdown_output(candidate, &requirement.title)
        {
            proposal.tasks.retain(|task| {
                let kind = task.kind.trim().to_ascii_lowercase();
                !(kind == "analysis_session" || task.title.trim().starts_with("[Breakdown]"))
            });
            if proposal.tasks.is_empty() {
                continue;
            }
            return Ok((proposal, candidate.clone()));
        }
    }

    let streamed_tasks = extract_streamed_breakdown_tasks_from_logs(logs);
    if !streamed_tasks.is_empty() {
        let raw = RawBreakdownProposal {
            analysis: json!({
                "summary": format!(
                    "Breakdown proposal generated from streamed agent output for requirement '{}' in project '{}' ({})",
                    requirement.title,
                    project_name,
                    project_type.display_name()
                ),
                "current_system_findings": [
                    "Agent ran analysis-only requirement breakdown attempt.",
                    "Tasks were extracted from BREAKDOWN_TASK stream lines."
                ],
                "assumptions": [
                    "Final task wording may be refined by user before confirmation."
                ]
            }),
            impact: infer_impact_areas(&requirement.content),
            plan: json!({
                "summary": "Use streamed breakdown output as draft plan, then confirm sprint assignment before task creation.",
                "steps": [
                    "Review streamed task proposals and adjust scope.",
                    "Confirm sprint assignment mode.",
                    "Create todo tasks in one batch operation."
                ]
            }),
            tasks: streamed_tasks,
            suggested_sprint_id,
        };
        let mut proposal = normalize_breakdown_proposal(raw, &requirement.title)?;
        proposal.tasks.retain(|task| {
            let kind = task.kind.trim().to_ascii_lowercase();
            !(kind == "analysis_session" || task.title.trim().starts_with("[Breakdown]"))
        });
        if !proposal.tasks.is_empty() {
            return Ok((proposal, combined));
        }
    }

    Err(ApiError::Internal(
        "Agent output did not produce valid breakdown proposal".to_string(),
    ))
}

fn infer_impact_areas(content: &str) -> Vec<BreakdownImpactItem> {
    let lower = content.to_ascii_lowercase();
    let mut areas = Vec::new();

    if lower.contains("ui")
        || lower.contains("frontend")
        || lower.contains("react")
        || lower.contains("screen")
    {
        areas.push(BreakdownImpactItem {
            area: "frontend".to_string(),
            impact: "UI flows and client-side validations need updates for this requirement."
                .to_string(),
            risk: "medium".to_string(),
            mitigation: "Add focused UI tests for the changed user path.".to_string(),
        });
    }
    if lower.contains("api")
        || lower.contains("endpoint")
        || lower.contains("server")
        || lower.contains("backend")
    {
        areas.push(BreakdownImpactItem {
            area: "backend".to_string(),
            impact: "Server-side handlers and business rules likely need changes.".to_string(),
            risk: "medium".to_string(),
            mitigation: "Add request/response contract tests and guard validations.".to_string(),
        });
    }
    if lower.contains("db")
        || lower.contains("database")
        || lower.contains("migration")
        || lower.contains("schema")
    {
        areas.push(BreakdownImpactItem {
            area: "database".to_string(),
            impact: "Data model or query layer may require migration and compatibility checks."
                .to_string(),
            risk: "high".to_string(),
            mitigation: "Use backward-compatible migration and add rollback-safe checks."
                .to_string(),
        });
    }

    if areas.is_empty() {
        areas.push(BreakdownImpactItem {
            area: "backend".to_string(),
            impact: "Core application logic needs scoped implementation updates.".to_string(),
            risk: "medium".to_string(),
            mitigation: "Implement incrementally and validate with targeted tests.".to_string(),
        });
        areas.push(BreakdownImpactItem {
            area: "testing".to_string(),
            impact: "Regression coverage required to protect existing behavior.".to_string(),
            risk: "low".to_string(),
            mitigation: "Add tests for happy path and one key edge case.".to_string(),
        });
    }

    areas
}

fn build_fallback_breakdown_output(
    requirement: &Requirement,
    project_name: &str,
    project_type: ProjectType,
    suggested_sprint_id: Option<Uuid>,
) -> String {
    let impact = infer_impact_areas(&requirement.content);
    let summary = format!(
        "Requirement '{}' for project '{}' ({}) was analyzed with current repository context.",
        requirement.title,
        project_name,
        project_type.display_name()
    );
    let output = json!({
        "analysis": {
            "summary": summary,
            "current_system_findings": [
                "Requirement content and existing project scope were inspected.",
                "Task breakdown keeps execution tasks in todo and separated from analysis."
            ],
            "assumptions": [
                "Detailed implementation will be validated during coding tasks.",
                "Sprint assignment will be confirmed before task creation."
            ]
        },
        "impact": impact,
        "plan": {
            "summary": "Run analysis, confirm sprint assignment, then execute small tasks in sequence.",
            "steps": [
                "Capture requirement analysis and impact in a dedicated breakdown task.",
                "Implement core code changes in focused tasks.",
                "Validate with tests and integration checks."
            ]
        },
        "tasks": [
            {
                "title": format!("[Breakdown] {}", requirement.title),
                "description": "AI analysis session for requirement scope, impact, and execution plan.",
                "task_type": "spike",
                "estimate": "S",
                "depends_on": [],
                "kind": "analysis_session"
            },
            {
                "title": format!("Implement core changes for {}", requirement.title),
                "description": "Apply the main implementation updates required by this requirement.",
                "task_type": "feature",
                "estimate": "M",
                "depends_on": [],
                "kind": "implementation"
            },
            {
                "title": format!("Add validations and edge-case handling for {}", requirement.title),
                "description": "Harden data validation and error handling for updated flows.",
                "task_type": "refactor",
                "estimate": "S",
                "depends_on": [
                    format!("Implement core changes for {}", requirement.title)
                ],
                "kind": "implementation"
            },
            {
                "title": format!("Add tests for {}", requirement.title),
                "description": "Add automated tests covering new behavior and key regressions.",
                "task_type": "test",
                "estimate": "S",
                "depends_on": [
                    format!("Implement core changes for {}", requirement.title)
                ],
                "kind": "implementation"
            }
        ],
        "suggested_sprint_id": suggested_sprint_id
    });
    output.to_string()
}

async fn get_breakdown_session(
    pool: &PgPool,
    project_id: Uuid,
    requirement_id: Uuid,
    session_id: Uuid,
) -> Result<Option<RequirementBreakdownSessionRow>, ApiError> {
    sqlx::query_as::<_, RequirementBreakdownSessionRow>(
        r#"
        SELECT
            id, project_id, requirement_id, created_by, status,
            analysis, impact, plan, proposed_tasks, suggested_sprint_id,
            error_message,
            created_at, updated_at, started_at, completed_at, confirmed_at, cancelled_at
        FROM requirement_breakdown_sessions
        WHERE id = $1 AND project_id = $2 AND requirement_id = $3
        "#,
    )
    .bind(session_id)
    .bind(project_id)
    .bind(requirement_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

async fn process_breakdown_job(state: AppState, job: BreakdownJob) {
    let update_running = sqlx::query(
        r#"
        UPDATE requirement_breakdown_sessions
        SET status = $2, started_at = NOW(), updated_at = NOW(), error_message = NULL
        WHERE id = $1 AND status = $3
        "#,
    )
    .bind(job.session_id)
    .bind(BREAKDOWN_STATUS_RUNNING)
    .bind(BREAKDOWN_STATUS_QUEUED)
    .execute(&state.db)
    .await;

    if let Err(e) = update_running {
        tracing::error!(
            session_id = %job.session_id,
            error = %e,
            "Failed to move requirement breakdown session to running"
        );
        return;
    }

    let project_service = ProjectService::new(state.db.clone());
    let requirement_service = RequirementService::new(state.db.clone());
    let task_service = TaskService::new(state.db.clone());

    let project = match project_service.get_project(job.project_id).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            let _ = sqlx::query(
                r#"
                UPDATE requirement_breakdown_sessions
                SET status = $2, error_message = $3, completed_at = NOW(), updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(job.session_id)
            .bind(BREAKDOWN_STATUS_FAILED)
            .bind("Project not found")
            .execute(&state.db)
            .await;
            return;
        }
        Err(e) => {
            let _ = sqlx::query(
                r#"
                UPDATE requirement_breakdown_sessions
                SET status = $2, error_message = $3, completed_at = NOW(), updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(job.session_id)
            .bind(BREAKDOWN_STATUS_FAILED)
            .bind(format!("Failed to load project: {}", e))
            .execute(&state.db)
            .await;
            return;
        }
    };

    let requirement = match requirement_service
        .get_requirement(job.requirement_id)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            let _ = sqlx::query(
                r#"
                UPDATE requirement_breakdown_sessions
                SET status = $2, error_message = $3, completed_at = NOW(), updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(job.session_id)
            .bind(BREAKDOWN_STATUS_FAILED)
            .bind(format!("Failed to load requirement: {}", e))
            .execute(&state.db)
            .await;
            return;
        }
    };

    // Fetch basic context to ground proposal (counts and active sprint recommendation).
    let task_count = task_service
        .get_project_tasks(job.project_id)
        .await
        .map(|v| v.len())
        .unwrap_or(0);
    let active_sprint_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM sprints WHERE project_id = $1 AND status = 'active' ORDER BY sequence ASC LIMIT 1",
    )
    .bind(job.project_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let analysis_task = match task_service
        .create_task(
            job.created_by,
            CreateTaskRequest {
                project_id: job.project_id,
                requirement_id: Some(job.requirement_id),
                sprint_id: active_sprint_id,
                title: format!("[Breakdown] {}", requirement.title.trim()),
                description: Some(build_breakdown_attempt_instruction(&requirement)),
                task_type: TaskType::Spike,
                assigned_to: None,
                metadata: Some(json!({
                    "priority": "medium",
                    "breakdown_session_id": job.session_id,
                    "breakdown_mode": "ai_support",
                    "breakdown_kind": "analysis_session",
                    "requirement_id": job.requirement_id,
                    "no_code_changes": true,
                    "execution": {
                        "no_code_changes": true,
                        "run_build_and_tests": false,
                        "require_review": false,
                        "auto_deploy": false,
                    },
                    "skills": ["requirement-breakdown"]
                })),
                parent_task_id: None,
            },
        )
        .await
    {
        Ok(task) => task,
        Err(e) => {
            let _ = sqlx::query(
                r#"
                UPDATE requirement_breakdown_sessions
                SET status = $2, error_message = $3, completed_at = NOW(), updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(job.session_id)
            .bind(BREAKDOWN_STATUS_FAILED)
            .bind(format!("Failed to create breakdown analysis task: {}", e))
            .execute(&state.db)
            .await;
            return;
        }
    };

    let attempt_id = match task_attempts::create_task_attempt(
        State(state.clone()),
        AuthUser {
            id: job.created_by,
            jti: format!("breakdown-session-{}", job.session_id),
        },
        Path(analysis_task.id),
    )
    .await
    {
        Ok((_status, Json(response))) => match response.data {
            Some(dto) => dto.id,
            None => {
                let _ = sqlx::query(
                    r#"
                    UPDATE requirement_breakdown_sessions
                    SET status = $2, error_message = $3, completed_at = NOW(), updated_at = NOW()
                    WHERE id = $1
                    "#,
                )
                .bind(job.session_id)
                .bind(BREAKDOWN_STATUS_FAILED)
                .bind("Breakdown attempt started but response did not return attempt id")
                .execute(&state.db)
                .await;
                return;
            }
        },
        Err(e) => {
            let _ = sqlx::query(
                r#"
                UPDATE requirement_breakdown_sessions
                SET status = $2, error_message = $3, completed_at = NOW(), updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(job.session_id)
            .bind(BREAKDOWN_STATUS_FAILED)
            .bind(format!("Failed to start breakdown attempt: {}", e))
            .execute(&state.db)
            .await;
            return;
        }
    };

    let attempt_service = TaskAttemptService::new(state.db.clone());
    let max_wait = Duration::from_secs(15 * 60);
    let poll_interval = Duration::from_secs(2);
    let mut waited = Duration::from_secs(0);
    let terminal_attempt = loop {
        let attempt = match attempt_service.get_attempt(attempt_id).await {
            Ok(Some(a)) => a,
            Ok(None) => {
                let _ = sqlx::query(
                    r#"
                    UPDATE requirement_breakdown_sessions
                    SET status = $2, error_message = $3, completed_at = NOW(), updated_at = NOW()
                    WHERE id = $1
                    "#,
                )
                .bind(job.session_id)
                .bind(BREAKDOWN_STATUS_FAILED)
                .bind("Breakdown attempt not found after start")
                .execute(&state.db)
                .await;
                return;
            }
            Err(e) => {
                let _ = sqlx::query(
                    r#"
                    UPDATE requirement_breakdown_sessions
                    SET status = $2, error_message = $3, completed_at = NOW(), updated_at = NOW()
                    WHERE id = $1
                    "#,
                )
                .bind(job.session_id)
                .bind(BREAKDOWN_STATUS_FAILED)
                .bind(format!("Failed to read breakdown attempt status: {}", e))
                .execute(&state.db)
                .await;
                return;
            }
        };

        if matches!(
            attempt.status,
            AttemptStatus::Success | AttemptStatus::Failed | AttemptStatus::Cancelled
        ) {
            break attempt;
        }

        if waited >= max_wait {
            let _ = sqlx::query(
                r#"
                UPDATE requirement_breakdown_sessions
                SET status = $2, error_message = $3, completed_at = NOW(), updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(job.session_id)
            .bind(BREAKDOWN_STATUS_FAILED)
            .bind("Breakdown attempt timeout while waiting for completion")
            .execute(&state.db)
            .await;
            return;
        }

        sleep(poll_interval).await;
        waited += poll_interval;
    };

    let attempt_logs = load_attempt_logs_for_breakdown(&state, &terminal_attempt).await;
    let fallback_output = build_fallback_breakdown_output(
        &requirement,
        &project.name,
        project.project_type,
        active_sprint_id,
    );

    let (mut proposal, raw_output, parse_source) = match choose_breakdown_proposal_from_logs(
        &attempt_logs,
        &requirement,
        &project.name,
        project.project_type,
        active_sprint_id,
    ) {
        Ok((proposal, raw)) => (proposal, raw, "agent_output"),
        Err(parse_err) => {
            if terminal_attempt.status == AttemptStatus::Success {
                match parse_and_validate_breakdown_output(&fallback_output, &requirement.title) {
                    Ok(proposal) => (proposal, fallback_output, "fallback"),
                    Err(fallback_err) => {
                        let _ = sqlx::query(
                            r#"
                            UPDATE requirement_breakdown_sessions
                            SET status = $2, raw_output = $3, error_message = $4, completed_at = NOW(), updated_at = NOW()
                            WHERE id = $1
                            "#,
                        )
                        .bind(job.session_id)
                        .bind(BREAKDOWN_STATUS_FAILED)
                        .bind(
                            attempt_logs
                                .iter()
                                .map(|log| log.content.as_str())
                                .collect::<Vec<&str>>()
                                .join("\n"),
                        )
                        .bind(format!(
                            "Breakdown parser failed: {}; fallback failed: {}",
                            parse_err, fallback_err
                        ))
                        .execute(&state.db)
                        .await;
                        return;
                    }
                }
            } else {
                let _ = sqlx::query(
                    r#"
                    UPDATE requirement_breakdown_sessions
                    SET status = $2, raw_output = $3, error_message = $4, completed_at = NOW(), updated_at = NOW()
                    WHERE id = $1
                    "#,
                )
                .bind(job.session_id)
                .bind(BREAKDOWN_STATUS_FAILED)
                .bind(
                    attempt_logs
                        .iter()
                        .map(|log| log.content.as_str())
                        .collect::<Vec<&str>>()
                        .join("\n"),
                )
                .bind(format!(
                    "Breakdown attempt ended with status {:?}: {}. Parser error: {}",
                    terminal_attempt.status,
                    terminal_attempt
                        .error_message
                        .unwrap_or_else(|| "no provider error message".to_string()),
                    parse_err
                ))
                .execute(&state.db)
                .await;
                return;
            }
        }
    };

    // Dedicated analysis task is already created and executed; proposal should keep implementation tasks only.
    proposal.tasks.retain(|task| {
        let kind = task.kind.trim().to_ascii_lowercase();
        !(kind == "analysis_session" || task.title.trim().starts_with("[Breakdown]"))
    });
    if proposal.tasks.is_empty() {
        proposal.tasks.push(BreakdownTaskDraft {
            title: format!("Implement core changes for {}", requirement.title),
            description: "Apply the main implementation updates required by this requirement."
                .to_string(),
            task_type: "feature".to_string(),
            estimate: Some("M".to_string()),
            depends_on: vec![],
            kind: "implementation".to_string(),
        });
    }

    // Keep lightweight execution context in analysis for review step.
    if let Some(obj) = proposal.analysis.as_object_mut() {
        obj.insert("project_type".to_string(), json!(project.project_type));
        obj.insert("existing_task_count".to_string(), json!(task_count));
        obj.insert(
            "breakdown_session_task_id".to_string(),
            json!(analysis_task.id),
        );
        obj.insert("breakdown_attempt_id".to_string(), json!(attempt_id));
        obj.insert("breakdown_parse_source".to_string(), json!(parse_source));
    }

    let proposed_tasks_value = serde_json::to_value(&proposal.tasks).ok();
    let _ = sqlx::query(
        r#"
        UPDATE requirement_breakdown_sessions
        SET
            status = $2,
            analysis = $3,
            impact = $4,
            plan = $5,
            proposed_tasks = $6,
            suggested_sprint_id = $7,
            raw_output = $8,
            completed_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(job.session_id)
    .bind(BREAKDOWN_STATUS_REVIEW)
    .bind(proposal.analysis)
    .bind(json!(proposal.impact))
    .bind(proposal.plan)
    .bind(proposed_tasks_value)
    .bind(proposal.suggested_sprint_id.or(active_sprint_id))
    .bind(raw_output)
    .execute(&state.db)
    .await;
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/requirements/{requirement_id}/breakdown/start",
    tag = "Requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("requirement_id" = Uuid, Path, description = "Requirement ID")
    ),
    responses(
        (status = 200, description = "Breakdown session already active"),
        (status = 202, description = "Breakdown session started"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Requirement not found")
    )
)]
pub async fn start_requirement_breakdown(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, requirement_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<(
    StatusCode,
    Json<ApiResponse<RequirementBreakdownSessionDto>>,
)> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ModifyRequirement,
        &state.db,
    )
    .await?;
    RbacChecker::check_permission(auth_user.id, project_id, Permission::CreateTask, &state.db)
        .await?;
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ExecuteTask, &state.db)
        .await?;

    let requirement_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM requirements WHERE id = $1 AND project_id = $2)",
    )
    .bind(requirement_id)
    .bind(project_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    if !requirement_exists {
        return Err(ApiError::NotFound("Requirement not found".to_string()));
    }

    let existing_active = sqlx::query_as::<_, RequirementBreakdownSessionRow>(
        r#"
        SELECT
            id, project_id, requirement_id, created_by, status,
            analysis, impact, plan, proposed_tasks, suggested_sprint_id,
            error_message,
            created_at, updated_at, started_at, completed_at, confirmed_at, cancelled_at
        FROM requirement_breakdown_sessions
        WHERE
            project_id = $1
            AND requirement_id = $2
            AND created_by = $3
            AND status IN ($4, $5, $6)
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(project_id)
    .bind(requirement_id)
    .bind(auth_user.id)
    .bind(BREAKDOWN_STATUS_QUEUED)
    .bind(BREAKDOWN_STATUS_RUNNING)
    .bind(BREAKDOWN_STATUS_REVIEW)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if let Some(session) = existing_active {
        let response = ApiResponse::success(
            RequirementBreakdownSessionDto::from(session),
            "Requirement breakdown session already active",
        );
        return Ok((StatusCode::OK, Json(response)));
    }

    let session = sqlx::query_as::<_, RequirementBreakdownSessionRow>(
        r#"
        INSERT INTO requirement_breakdown_sessions (
            project_id, requirement_id, created_by, status
        )
        VALUES ($1, $2, $3, $4)
        RETURNING
            id, project_id, requirement_id, created_by, status,
            analysis, impact, plan, proposed_tasks, suggested_sprint_id,
            error_message,
            created_at, updated_at, started_at, completed_at, confirmed_at, cancelled_at
        "#,
    )
    .bind(project_id)
    .bind(requirement_id)
    .bind(auth_user.id)
    .bind(BREAKDOWN_STATUS_QUEUED)
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let job = BreakdownJob {
        session_id: session.id,
        project_id,
        requirement_id,
        created_by: auth_user.id,
    };
    let state_clone = state.clone();
    tokio::spawn(async move {
        process_breakdown_job(state_clone, job).await;
    });

    let response = ApiResponse::success(
        RequirementBreakdownSessionDto::from(session),
        "Requirement breakdown session started",
    );
    Ok((StatusCode::ACCEPTED, Json(response)))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/requirements/{requirement_id}/breakdown/{session_id}",
    tag = "Requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("requirement_id" = Uuid, Path, description = "Requirement ID"),
        ("session_id" = Uuid, Path, description = "Breakdown session ID")
    ),
    responses(
        (status = 200, description = "Breakdown session"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Session not found")
    )
)]
pub async fn get_requirement_breakdown_session(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, requirement_id, session_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<RequirementBreakdownSessionDto>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let session = get_breakdown_session(&state.db, project_id, requirement_id, session_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Breakdown session not found".to_string()))?;

    Ok(Json(ApiResponse::success(
        RequirementBreakdownSessionDto::from(session),
        "Requirement breakdown session retrieved",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/requirements/{requirement_id}/breakdown/{session_id}/confirm",
    tag = "Requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("requirement_id" = Uuid, Path, description = "Requirement ID"),
        ("session_id" = Uuid, Path, description = "Breakdown session ID")
    ),
    responses(
        (status = 200, description = "Breakdown confirmed and tasks created"),
        (status = 400, description = "Invalid payload"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Session not found")
    )
)]
pub async fn confirm_requirement_breakdown(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, requirement_id, session_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(payload): Json<ConfirmRequirementBreakdownRequest>,
) -> ApiResult<Json<ApiResponse<ConfirmRequirementBreakdownResponse>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::CreateTask, &state.db)
        .await?;

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let session = sqlx::query_as::<_, RequirementBreakdownSessionRow>(
        r#"
        SELECT
            id, project_id, requirement_id, created_by, status,
            analysis, impact, plan, proposed_tasks, suggested_sprint_id,
            error_message,
            created_at, updated_at, started_at, completed_at, confirmed_at, cancelled_at
        FROM requirement_breakdown_sessions
        WHERE id = $1 AND project_id = $2 AND requirement_id = $3
        FOR UPDATE
        "#,
    )
    .bind(session_id)
    .bind(project_id)
    .bind(requirement_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("Breakdown session not found".to_string()))?;

    if session.status != BREAKDOWN_STATUS_REVIEW {
        return Err(ApiError::BadRequest(format!(
            "Breakdown session is not ready for confirmation (status: {})",
            session.status
        )));
    }

    let proposed_tasks_value = session
        .proposed_tasks
        .clone()
        .ok_or_else(|| ApiError::BadRequest("No proposed tasks to confirm".to_string()))?;
    let proposed_tasks: Vec<BreakdownTaskDraft> = serde_json::from_value(proposed_tasks_value)
        .map_err(|e| ApiError::BadRequest(format!("Invalid proposed tasks payload: {}", e)))?;

    if proposed_tasks.is_empty() {
        return Err(ApiError::BadRequest(
            "No proposed tasks to confirm".to_string(),
        ));
    }

    let resolved_sprint_id = match payload.assignment_mode {
        BreakdownSprintAssignmentMode::Backlog => None,
        BreakdownSprintAssignmentMode::Active => sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM sprints WHERE project_id = $1 AND status = 'active' ORDER BY sequence ASC LIMIT 1",
        )
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::BadRequest("No active sprint found for this project".to_string()))?
        .into(),
        BreakdownSprintAssignmentMode::Selected => {
            let sprint_id = payload
                .sprint_id
                .ok_or_else(|| ApiError::BadRequest("sprint_id is required for selected mode".to_string()))?;
            let sprint_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM sprints WHERE id = $1 AND project_id = $2)",
            )
            .bind(sprint_id)
            .bind(project_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
            if !sprint_exists {
                return Err(ApiError::BadRequest(
                    "Selected sprint does not belong to this project".to_string(),
                ));
            }
            Some(sprint_id)
        }
    };

    let total_tasks = proposed_tasks.len();
    let mut created_tasks: Vec<Task> = Vec::with_capacity(total_tasks);
    for (idx, task) in proposed_tasks.into_iter().enumerate() {
        let task_type = parse_supported_task_type(&task.task_type).ok_or_else(|| {
            ApiError::BadRequest(format!(
                "Unsupported task_type '{}' in proposal",
                task.task_type
            ))
        })?;
        let mut metadata = json!({
            "breakdown_session_id": session.id,
            "breakdown_mode": "ai",
            "breakdown_kind": if task.kind.trim().is_empty() { "implementation" } else { task.kind.trim() },
            "estimate": task.estimate
        });
        with_requirement_execution_metadata(
            &mut metadata,
            requirement_id,
            idx + 1,
            total_tasks,
            Some(&task.depends_on),
        );

        let created = sqlx::query_as::<_, Task>(
            r#"
            INSERT INTO tasks (
                project_id, title, description, task_type, status,
                requirement_id, sprint_id, created_by, metadata
            )
            VALUES ($1, $2, $3, $4::task_type, $5::task_status, $6, $7, $8, $9)
            RETURNING id, project_id, title, description, task_type, status, assigned_to, parent_task_id, requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind(task.title.trim())
        .bind(Some(task.description.trim().to_string()))
        .bind(task_type)
        .bind(TaskStatus::Todo)
        .bind(requirement_id)
        .bind(resolved_sprint_id)
        .bind(auth_user.id)
        .bind(metadata)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
        created_tasks.push(created);
    }

    let updated_session = sqlx::query_as::<_, RequirementBreakdownSessionRow>(
        r#"
        UPDATE requirement_breakdown_sessions
        SET
            status = $2,
            confirmed_at = NOW(),
            completed_at = COALESCE(completed_at, NOW()),
            suggested_sprint_id = $3,
            updated_at = NOW()
        WHERE id = $1
        RETURNING
            id, project_id, requirement_id, created_by, status,
            analysis, impact, plan, proposed_tasks, suggested_sprint_id,
            error_message,
            created_at, updated_at, started_at, completed_at, confirmed_at, cancelled_at
        "#,
    )
    .bind(session.id)
    .bind(BREAKDOWN_STATUS_CONFIRMED)
    .bind(resolved_sprint_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = ConfirmRequirementBreakdownResponse {
        session: RequirementBreakdownSessionDto::from(updated_session),
        tasks: created_tasks.into_iter().map(TaskDto::from).collect(),
    };

    Ok(Json(ApiResponse::success(
        response,
        "Breakdown confirmed and tasks created",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/requirements/{requirement_id}/breakdown/manual/confirm",
    tag = "Requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("requirement_id" = Uuid, Path, description = "Requirement ID")
    ),
    responses(
        (status = 200, description = "Manual breakdown tasks created"),
        (status = 400, description = "Invalid payload"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Requirement not found")
    )
)]
pub async fn confirm_requirement_breakdown_manual(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, requirement_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<ConfirmManualRequirementBreakdownRequest>,
) -> ApiResult<Json<ApiResponse<ConfirmManualRequirementBreakdownResponse>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::CreateTask, &state.db)
        .await?;

    if payload.tasks.is_empty() {
        return Err(ApiError::BadRequest(
            "tasks must contain at least one item".to_string(),
        ));
    }
    if payload.tasks.len() > 100 {
        return Err(ApiError::BadRequest(
            "tasks exceeds maximum allowed items (100)".to_string(),
        ));
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let requirement_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM requirements WHERE id = $1 AND project_id = $2)",
    )
    .bind(requirement_id)
    .bind(project_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    if !requirement_exists {
        return Err(ApiError::NotFound("Requirement not found".to_string()));
    }

    let resolved_sprint_id = match payload.assignment_mode {
        BreakdownSprintAssignmentMode::Backlog => None,
        BreakdownSprintAssignmentMode::Active => sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM sprints WHERE project_id = $1 AND status = 'active' ORDER BY sequence ASC LIMIT 1",
        )
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::BadRequest("No active sprint found for this project".to_string()))?
        .into(),
        BreakdownSprintAssignmentMode::Selected => {
            let sprint_id = payload
                .sprint_id
                .ok_or_else(|| ApiError::BadRequest("sprint_id is required for selected mode".to_string()))?;
            let sprint_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM sprints WHERE id = $1 AND project_id = $2)",
            )
            .bind(sprint_id)
            .bind(project_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
            if !sprint_exists {
                return Err(ApiError::BadRequest(
                    "Selected sprint does not belong to this project".to_string(),
                ));
            }
            Some(sprint_id)
        }
    };

    let total_tasks = payload.tasks.len();
    let mut created_tasks: Vec<Task> = Vec::with_capacity(total_tasks);
    for (idx, task) in payload.tasks.iter().enumerate() {
        let title = task.title.trim();
        if title.is_empty() {
            return Err(ApiError::BadRequest(format!(
                "tasks[{}].title must not be empty",
                idx
            )));
        }
        let task_type = parse_supported_task_type(&task.task_type).ok_or_else(|| {
            ApiError::BadRequest(format!(
                "tasks[{}].task_type '{}' is not supported",
                idx, task.task_type
            ))
        })?;
        let priority = parse_supported_priority(task.priority.as_deref()).ok_or_else(|| {
            ApiError::BadRequest(format!(
                "tasks[{}].priority '{}' is not supported",
                idx,
                task.priority.as_deref().unwrap_or_default()
            ))
        })?;

        if let Some(assignee_id) = task.assigned_to {
            let is_member: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM project_members WHERE project_id = $1 AND user_id = $2)",
            )
            .bind(project_id)
            .bind(assignee_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
            if !is_member {
                return Err(ApiError::BadRequest(format!(
                    "tasks[{}].assigned_to must belong to the project",
                    idx
                )));
            }
        }

        let description = task
            .description
            .as_deref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);

        let normalized_kind = task
            .kind
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("implementation");

        let mut metadata = task.metadata.clone().unwrap_or_else(|| json!({}));
        if !metadata.is_object() {
            metadata = json!({ "manual_payload": metadata });
        }
        if let Some(metadata_obj) = metadata.as_object_mut() {
            metadata_obj.insert("breakdown_mode".to_string(), json!("manual"));
            metadata_obj.insert("breakdown_kind".to_string(), json!(normalized_kind));
            metadata_obj.insert("priority".to_string(), json!(priority));
        }
        with_requirement_execution_metadata(
            &mut metadata,
            requirement_id,
            idx + 1,
            total_tasks,
            None,
        );

        let created = sqlx::query_as::<_, Task>(
            r#"
            INSERT INTO tasks (
                project_id, title, description, task_type, status, assigned_to,
                requirement_id, sprint_id, created_by, metadata
            )
            VALUES ($1, $2, $3, $4::task_type, $5::task_status, $6, $7, $8, $9, $10)
            RETURNING id, project_id, title, description, task_type, status, assigned_to, parent_task_id, requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind(title)
        .bind(description)
        .bind(task_type)
        .bind(TaskStatus::Todo)
        .bind(task.assigned_to)
        .bind(requirement_id)
        .bind(resolved_sprint_id)
        .bind(auth_user.id)
        .bind(metadata)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        created_tasks.push(created);
    }

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = ConfirmManualRequirementBreakdownResponse {
        tasks: created_tasks.into_iter().map(TaskDto::from).collect(),
    };

    Ok(Json(ApiResponse::success(
        response,
        "Manual breakdown tasks created",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/requirements/{requirement_id}/tasks/start-sequential",
    tag = "Requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("requirement_id" = Uuid, Path, description = "Requirement ID")
    ),
    responses(
        (status = 200, description = "Requirement task sequence started"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Requirement not found")
    )
)]
pub async fn start_requirement_task_sequence(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, requirement_id)): Path<(Uuid, Uuid)>,
    payload: Option<Json<StartRequirementTaskSequenceRequest>>,
) -> ApiResult<Json<ApiResponse<StartRequirementTaskSequenceResponse>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ExecuteTask, &state.db)
        .await?;

    let requirement_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM requirements WHERE id = $1 AND project_id = $2)",
    )
    .bind(requirement_id)
    .bind(project_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    if !requirement_exists {
        return Err(ApiError::NotFound("Requirement not found".to_string()));
    }

    let continue_on_failure = payload
        .map(|body| body.0.continue_on_failure)
        .unwrap_or(false);

    let mut requirement_tasks = sqlx::query_as::<_, Task>(
        r#"
        SELECT id, project_id, title, description, task_type, status, assigned_to, parent_task_id,
               requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
        FROM tasks
        WHERE project_id = $1
          AND requirement_id = $2
          AND task_type <> 'init'
        "#,
    )
    .bind(project_id)
    .bind(requirement_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    requirement_tasks.retain(|task| {
        !is_breakdown_analysis_task(task)
            && task_status_is_eligible_for_requirement_sequence(task.status)
    });
    sort_requirement_tasks_for_sequence(&mut requirement_tasks);

    let task_ids: Vec<Uuid> = requirement_tasks.into_iter().map(|task| task.id).collect();
    let run_id = Uuid::new_v4();

    if !task_ids.is_empty() {
        let state_clone = state.clone();
        let auth_user_clone = auth_user.clone();
        let task_ids_clone = task_ids.clone();
        tokio::spawn(async move {
            run_requirement_task_sequence(
                state_clone,
                auth_user_clone,
                project_id,
                requirement_id,
                run_id,
                task_ids_clone,
                continue_on_failure,
            )
            .await;
        });
    }

    let response = StartRequirementTaskSequenceResponse {
        run_id,
        total_tasks: task_ids.len(),
        task_ids,
        continue_on_failure,
    };

    Ok(Json(ApiResponse::success(
        response,
        "Requirement task sequence started",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/requirements/{requirement_id}/breakdown/{session_id}/cancel",
    tag = "Requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project ID"),
        ("requirement_id" = Uuid, Path, description = "Requirement ID"),
        ("session_id" = Uuid, Path, description = "Breakdown session ID")
    ),
    responses(
        (status = 200, description = "Breakdown session cancelled"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Session not found")
    )
)]
pub async fn cancel_requirement_breakdown(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, requirement_id, session_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<RequirementBreakdownSessionDto>>> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ModifyRequirement,
        &state.db,
    )
    .await?;

    let session = get_breakdown_session(&state.db, project_id, requirement_id, session_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Breakdown session not found".to_string()))?;

    if matches!(
        session.status.as_str(),
        BREAKDOWN_STATUS_CONFIRMED | BREAKDOWN_STATUS_CANCELLED
    ) {
        return Err(ApiError::BadRequest(format!(
            "Session cannot be cancelled in status '{}'",
            session.status
        )));
    }

    let updated = sqlx::query_as::<_, RequirementBreakdownSessionRow>(
        r#"
        UPDATE requirement_breakdown_sessions
        SET status = $2, cancelled_at = NOW(), updated_at = NOW()
        WHERE id = $1
        RETURNING
            id, project_id, requirement_id, created_by, status,
            analysis, impact, plan, proposed_tasks, suggested_sprint_id,
            error_message,
            created_at, updated_at, started_at, completed_at, confirmed_at, cancelled_at
        "#,
    )
    .bind(session.id)
    .bind(BREAKDOWN_STATUS_CANCELLED)
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(ApiResponse::success(
        RequirementBreakdownSessionDto::from(updated),
        "Breakdown session cancelled",
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_task_with_metadata(
        title: &str,
        status: TaskStatus,
        metadata: Value,
        created_at: DateTime<Utc>,
    ) -> Task {
        Task {
            id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            requirement_id: Some(Uuid::new_v4()),
            sprint_id: None,
            title: title.to_string(),
            description: None,
            task_type: TaskType::Feature,
            status,
            assigned_to: None,
            parent_task_id: None,
            gitlab_issue_id: None,
            metadata,
            created_by: Uuid::new_v4(),
            created_at,
            updated_at: created_at,
        }
    }

    #[test]
    fn parse_breakdown_output_accepts_json_with_missing_breakdown_task() {
        let raw = r#"{
            "analysis": {"summary":"ok"},
            "impact": [{"area":"backend","impact":"x","risk":"medium","mitigation":"y"}],
            "plan": {"summary":"plan","steps":["a","b"]},
            "tasks": [
                {"title":"Implement A","description":"d1","task_type":"feature"}
            ],
            "suggested_sprint_id": null
        }"#;
        let parsed = parse_and_validate_breakdown_output(raw, "Requirement A")
            .expect("expected parser to succeed");
        assert!(!parsed.tasks.is_empty());
        assert_eq!(parsed.tasks[0].kind, "analysis_session");
        assert!(parsed.tasks[0].title.starts_with("[Breakdown]"));
    }

    #[test]
    fn parse_breakdown_output_rejects_non_json() {
        let raw = "not-a-json";
        let err = parse_and_validate_breakdown_output(raw, "Req")
            .expect_err("expected parser to reject invalid json");
        assert!(
            format!("{}", err).to_ascii_lowercase().contains("json"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn parse_breakdown_output_rejects_unsupported_task_type() {
        let raw = r#"{
            "analysis": {"summary":"ok"},
            "impact": [{"area":"backend","impact":"x","risk":"medium","mitigation":"y"}],
            "plan": {"summary":"plan","steps":["a","b"]},
            "tasks": [
                {"title":"Bad task","description":"d1","task_type":"deploy"}
            ]
        }"#;
        let parsed = parse_and_validate_breakdown_output(raw, "Req");
        assert!(parsed.is_err(), "unsupported type should fail validation");
    }

    #[test]
    fn extract_streamed_breakdown_tasks_parses_breakdown_task_lines() {
        let attempt_id = Uuid::new_v4();
        let logs = vec![
            AgentLog {
                id: Uuid::new_v4(),
                attempt_id,
                log_type: "stdout".to_string(),
                content: r#"BREAKDOWN_TASK {"title":"Create API endpoint","description":"Expose requirement breakdown endpoint","task_type":"feature","priority":"high","kind":"implementation"}"#.to_string(),
                created_at: Utc::now(),
            },
            AgentLog {
                id: Uuid::new_v4(),
                attempt_id,
                log_type: "stdout".to_string(),
                content: r#"noise
BREAKDOWN_TASK {"title":"Add tests","description":"Cover breakdown parser and RBAC","task_type":"test","priority":"medium","kind":"implementation"}"#.to_string(),
                created_at: Utc::now(),
            },
        ];

        let tasks = extract_streamed_breakdown_tasks_from_logs(&logs);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].title, "Create API endpoint");
        assert_eq!(tasks[0].task_type.as_deref(), Some("feature"));
        assert_eq!(tasks[1].title, "Add tests");
        assert_eq!(tasks[1].task_type.as_deref(), Some("test"));
    }

    #[test]
    fn requirement_execution_metadata_sets_sequential_fields() {
        let requirement_id = Uuid::new_v4();
        let mut metadata = json!({
            "breakdown_mode": "manual",
            "execution": {
                "no_code_changes": true
            }
        });
        let depends_on = vec!["Task A".to_string(), "Task B".to_string()];

        with_requirement_execution_metadata(&mut metadata, requirement_id, 2, 6, Some(&depends_on));

        let execution_group = metadata
            .get("execution_group")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert_eq!(
            execution_group,
            format!("requirement:{}", requirement_id),
            "execution_group should target requirement"
        );
        assert_eq!(metadata.get("execution_order"), Some(&json!(2)));
        assert_eq!(metadata.get("execution_total"), Some(&json!(6)));

        let execution = metadata
            .get("execution")
            .and_then(Value::as_object)
            .expect("execution object should be present");
        assert_eq!(execution.get("policy"), Some(&json!("sequential")));
        assert_eq!(execution.get("order"), Some(&json!(2)));
        assert_eq!(execution.get("total"), Some(&json!(6)));
        assert_eq!(execution.get("depends_on"), Some(&json!(depends_on)));
        assert_eq!(
            execution.get("no_code_changes"),
            Some(&json!(true)),
            "existing execution fields should be preserved"
        );
    }

    #[test]
    fn requirement_execution_metadata_normalizes_non_object_metadata() {
        let requirement_id = Uuid::new_v4();
        let mut metadata = json!("raw-metadata");

        with_requirement_execution_metadata(&mut metadata, requirement_id, 1, 3, None);

        let metadata_obj = metadata
            .as_object()
            .expect("metadata should be normalized into object");
        assert!(metadata_obj.contains_key("manual_payload"));
        assert_eq!(
            metadata_obj.get("execution_policy"),
            Some(&json!("sequential"))
        );
        assert_eq!(metadata_obj.get("execution_order"), Some(&json!(1)));
        assert_eq!(metadata_obj.get("execution_total"), Some(&json!(3)));
    }

    #[test]
    fn parse_execution_order_reads_root_and_nested_fields() {
        let now = Utc::now();
        let root = make_task_with_metadata(
            "Root order",
            TaskStatus::Todo,
            json!({ "execution_order": "5" }),
            now,
        );
        let nested = make_task_with_metadata(
            "Nested order",
            TaskStatus::Todo,
            json!({ "execution": { "order": 2 } }),
            now,
        );

        assert_eq!(parse_execution_order(&root), Some(5));
        assert_eq!(parse_execution_order(&nested), Some(2));
    }

    #[test]
    fn sort_requirement_tasks_for_sequence_prefers_execution_order_then_created_at() {
        let base = Utc::now();
        let mut tasks = vec![
            make_task_with_metadata(
                "No order",
                TaskStatus::Todo,
                json!({}),
                base + chrono::Duration::seconds(30),
            ),
            make_task_with_metadata(
                "Order 2",
                TaskStatus::Todo,
                json!({ "execution_order": 2 }),
                base + chrono::Duration::seconds(10),
            ),
            make_task_with_metadata(
                "Order 1",
                TaskStatus::Todo,
                json!({ "execution": { "order": 1 } }),
                base + chrono::Duration::seconds(20),
            ),
        ];

        sort_requirement_tasks_for_sequence(&mut tasks);

        let titles: Vec<&str> = tasks.iter().map(|task| task.title.as_str()).collect();
        assert_eq!(titles, vec!["Order 1", "Order 2", "No order"]);
    }

    #[test]
    fn breakdown_analysis_tasks_are_excluded_from_sequence() {
        let analysis = make_task_with_metadata(
            "[Breakdown][AI] Analyze",
            TaskStatus::Todo,
            json!({ "breakdown_kind": "analysis_session" }),
            Utc::now(),
        );
        let execution = make_task_with_metadata(
            "Implement endpoint",
            TaskStatus::Todo,
            json!({ "breakdown_kind": "implementation" }),
            Utc::now(),
        );

        assert!(is_breakdown_analysis_task(&analysis));
        assert!(!is_breakdown_analysis_task(&execution));
        assert!(task_status_is_eligible_for_requirement_sequence(
            TaskStatus::Todo
        ));
        assert!(!task_status_is_eligible_for_requirement_sequence(
            TaskStatus::InReview
        ));
    }
}
