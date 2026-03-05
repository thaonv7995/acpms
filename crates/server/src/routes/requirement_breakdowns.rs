use acpms_db::{models::*, PgPool};
use acpms_services::{ProjectService, RequirementService, TaskService};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    api::{ApiResponse, TaskDto},
    error::{ApiError, ApiResult},
    middleware::{AuthUser, Permission, RbacChecker},
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

    let fallback_output = build_fallback_breakdown_output(
        &requirement,
        &project.name,
        project.project_type,
        active_sprint_id,
    );

    let mut proposal = match parse_and_validate_breakdown_output(
        &fallback_output,
        &requirement.title,
    ) {
        Ok(v) => v,
        Err(e) => {
            let _ = sqlx::query(
                r#"
                UPDATE requirement_breakdown_sessions
                SET status = $2, raw_output = $3, error_message = $4, completed_at = NOW(), updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(job.session_id)
            .bind(BREAKDOWN_STATUS_FAILED)
            .bind(fallback_output)
            .bind(format!("Breakdown parser failed: {}", e))
            .execute(&state.db)
            .await;
            return;
        }
    };

    // Keep lightweight execution context in analysis for review step.
    if let Some(obj) = proposal.analysis.as_object_mut() {
        obj.insert("project_type".to_string(), json!(project.project_type));
        obj.insert("existing_task_count".to_string(), json!(task_count));
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
    .bind(fallback_output)
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

    let mut created_tasks: Vec<Task> = Vec::with_capacity(proposed_tasks.len());
    for task in proposed_tasks {
        let task_type = parse_supported_task_type(&task.task_type).ok_or_else(|| {
            ApiError::BadRequest(format!(
                "Unsupported task_type '{}' in proposal",
                task.task_type
            ))
        })?;
        let metadata = json!({
            "breakdown_session_id": session.id,
            "breakdown_kind": if task.kind.trim().is_empty() { "implementation" } else { task.kind.trim() },
            "estimate": task.estimate
        });

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

    let mut created_tasks: Vec<Task> = Vec::with_capacity(payload.tasks.len());
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
}
