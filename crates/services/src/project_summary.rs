use acpms_db::{models::Project, PgPool};
use anyhow::Result;
use serde::Serialize;
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectComputedSummary {
    pub lifecycle_status: String,
    pub execution_status: String,
    pub progress: i64,
    pub total_tasks: i64,
    pub completed_tasks: i64,
    pub active_tasks: i64,
    pub review_tasks: i64,
    pub blocked_tasks: i64,
}

impl Default for ProjectComputedSummary {
    fn default() -> Self {
        Self {
            lifecycle_status: "planning".to_string(),
            execution_status: "idle".to_string(),
            progress: 0,
            total_tasks: 0,
            completed_tasks: 0,
            active_tasks: 0,
            review_tasks: 0,
            blocked_tasks: 0,
        }
    }
}

#[derive(Debug, FromRow)]
struct ProjectSummaryRow {
    project_id: Uuid,
    total_tasks: i64,
    completed_tasks: i64,
    active_tasks: i64,
    review_tasks: i64,
    blocked_tasks: i64,
    latest_attempt_status: Option<String>,
}

fn project_status_override(metadata: &serde_json::Value) -> Option<&'static str> {
    let status = metadata
        .get("status")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_ascii_lowercase();

    match status.as_str() {
        "paused" => Some("paused"),
        "archived" => Some("archived"),
        _ => None,
    }
}

pub fn derive_project_progress(total_tasks: i64, completed_tasks: i64) -> i64 {
    if total_tasks <= 0 {
        return 0;
    }

    ((completed_tasks as f64 / total_tasks as f64) * 100.0)
        .round()
        .clamp(0.0, 100.0) as i64
}

pub fn derive_project_execution_status(latest_attempt_status: Option<&str>) -> String {
    match latest_attempt_status.map(|status| status.trim().to_ascii_lowercase()) {
        Some(status) if status == "queued" => "queued".to_string(),
        Some(status) if status == "running" => "running".to_string(),
        Some(status) if status == "success" => "success".to_string(),
        Some(status) if status == "failed" => "failed".to_string(),
        Some(status) if status == "cancelled" || status == "canceled" => "cancelled".to_string(),
        _ => "idle".to_string(),
    }
}

pub fn derive_project_lifecycle_status(
    metadata: &serde_json::Value,
    total_tasks: i64,
    completed_tasks: i64,
    active_tasks: i64,
    review_tasks: i64,
    blocked_tasks: i64,
) -> String {
    if let Some(status) = project_status_override(metadata) {
        return status.to_string();
    }

    if total_tasks <= 0 {
        return "planning".to_string();
    }

    if completed_tasks >= total_tasks {
        return "completed".to_string();
    }

    if active_tasks > 0 {
        return "active".to_string();
    }

    if review_tasks > 0 {
        return "reviewing".to_string();
    }

    if blocked_tasks > 0 {
        return "blocked".to_string();
    }

    if completed_tasks > 0 {
        return "active".to_string();
    }

    "planning".to_string()
}

pub fn summarize_project(
    metadata: &serde_json::Value,
    total_tasks: i64,
    completed_tasks: i64,
    active_tasks: i64,
    review_tasks: i64,
    blocked_tasks: i64,
    latest_attempt_status: Option<&str>,
) -> ProjectComputedSummary {
    ProjectComputedSummary {
        lifecycle_status: derive_project_lifecycle_status(
            metadata,
            total_tasks,
            completed_tasks,
            active_tasks,
            review_tasks,
            blocked_tasks,
        ),
        execution_status: derive_project_execution_status(latest_attempt_status),
        progress: derive_project_progress(total_tasks, completed_tasks),
        total_tasks,
        completed_tasks,
        active_tasks,
        review_tasks,
        blocked_tasks,
    }
}

pub async fn load_project_summaries(
    pool: &PgPool,
    projects: &[Project],
) -> Result<HashMap<Uuid, ProjectComputedSummary>> {
    if projects.is_empty() {
        return Ok(HashMap::new());
    }

    let project_ids: Vec<Uuid> = projects.iter().map(|project| project.id).collect();
    let rows = sqlx::query_as::<_, ProjectSummaryRow>(
        r#"
        WITH task_stats AS (
            SELECT
                t.project_id,
                COUNT(*) FILTER (WHERE LOWER(t.status::text) <> 'archived')::bigint AS total_tasks,
                COUNT(*) FILTER (WHERE LOWER(t.status::text) = 'done')::bigint AS completed_tasks,
                COUNT(*) FILTER (WHERE LOWER(t.status::text) = 'in_progress')::bigint AS active_tasks,
                COUNT(*) FILTER (WHERE LOWER(t.status::text) = 'in_review')::bigint AS review_tasks,
                COUNT(*) FILTER (WHERE LOWER(t.status::text) = 'blocked')::bigint AS blocked_tasks
            FROM tasks t
            WHERE t.project_id = ANY($1)
            GROUP BY t.project_id
        ),
        latest_attempts AS (
            SELECT DISTINCT ON (t.project_id)
                t.project_id,
                ta.status::text AS latest_attempt_status
            FROM tasks t
            JOIN task_attempts ta ON ta.task_id = t.id
            WHERE t.project_id = ANY($1)
            ORDER BY t.project_id, COALESCE(ta.started_at, ta.created_at) DESC, ta.id DESC
        )
        SELECT
            p.id AS project_id,
            COALESCE(ts.total_tasks, 0)::bigint AS total_tasks,
            COALESCE(ts.completed_tasks, 0)::bigint AS completed_tasks,
            COALESCE(ts.active_tasks, 0)::bigint AS active_tasks,
            COALESCE(ts.review_tasks, 0)::bigint AS review_tasks,
            COALESCE(ts.blocked_tasks, 0)::bigint AS blocked_tasks,
            la.latest_attempt_status
        FROM projects p
        LEFT JOIN task_stats ts ON ts.project_id = p.id
        LEFT JOIN latest_attempts la ON la.project_id = p.id
        WHERE p.id = ANY($1)
        "#,
    )
    .bind(&project_ids)
    .fetch_all(pool)
    .await?;

    let rows_by_project: HashMap<Uuid, ProjectSummaryRow> =
        rows.into_iter().map(|row| (row.project_id, row)).collect();

    Ok(projects
        .iter()
        .map(|project| {
            let summary = rows_by_project
                .get(&project.id)
                .map(|row| {
                    summarize_project(
                        &project.metadata,
                        row.total_tasks,
                        row.completed_tasks,
                        row.active_tasks,
                        row.review_tasks,
                        row.blocked_tasks,
                        row.latest_attempt_status.as_deref(),
                    )
                })
                .unwrap_or_else(|| summarize_project(&project.metadata, 0, 0, 0, 0, 0, None));

            (project.id, summary)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn progress_is_zero_without_tasks() {
        assert_eq!(derive_project_progress(0, 0), 0);
    }

    #[test]
    fn summary_marks_project_completed_when_all_counted_tasks_are_done() {
        let summary = summarize_project(&json!({}), 4, 4, 0, 0, 0, Some("success"));

        assert_eq!(summary.lifecycle_status, "completed");
        assert_eq!(summary.execution_status, "success");
        assert_eq!(summary.progress, 100);
    }

    #[test]
    fn summary_marks_project_active_when_work_is_in_progress() {
        let summary = summarize_project(&json!({}), 6, 2, 1, 0, 0, Some("running"));

        assert_eq!(summary.lifecycle_status, "active");
        assert_eq!(summary.execution_status, "running");
        assert_eq!(summary.progress, 33);
    }

    #[test]
    fn summary_marks_project_reviewing_when_only_review_work_remains() {
        let summary = summarize_project(&json!({}), 3, 1, 0, 2, 0, Some("success"));

        assert_eq!(summary.lifecycle_status, "reviewing");
        assert_eq!(summary.execution_status, "success");
    }

    #[test]
    fn summary_marks_project_blocked_when_no_active_or_review_work_exists() {
        let summary = summarize_project(&json!({}), 5, 2, 0, 0, 1, Some("failed"));

        assert_eq!(summary.lifecycle_status, "blocked");
        assert_eq!(summary.execution_status, "failed");
    }

    #[test]
    fn summary_uses_metadata_pause_override() {
        let summary = summarize_project(&json!({ "status": "paused" }), 10, 10, 0, 0, 0, None);

        assert_eq!(summary.lifecycle_status, "paused");
        assert_eq!(summary.progress, 100);
    }

    #[test]
    fn summary_defaults_to_planning_when_only_backlog_like_work_exists() {
        let summary = summarize_project(&json!({}), 4, 0, 0, 0, 0, None);

        assert_eq!(summary.lifecycle_status, "planning");
        assert_eq!(summary.execution_status, "idle");
        assert_eq!(summary.progress, 0);
    }
}
