use crate::models::TaskWithAttemptStatus;
use sqlx::PgPool;
use uuid::Uuid;

/// Get tasks with computed attempt status fields for kanban board display
/// Supports optional sprint filtering
pub async fn get_tasks_with_attempt_status(
    pool: &PgPool,
    project_id: Uuid,
    sprint_id: Option<Uuid>,
) -> Result<Vec<TaskWithAttemptStatus>, sqlx::Error> {
    let tasks = if let Some(sprint) = sprint_id {
        // Query with sprint filter
        sqlx::query_as::<_, TaskWithAttemptStatus>(
            r#"
            SELECT
                t.id,
                t.project_id,
                t.requirement_id,
                t.sprint_id,
                t.title,
                t.description,
                t.task_type,
                t.status,
                t.assigned_to,
                t.parent_task_id,
                t.gitlab_issue_id,
                t.metadata,
                t.created_by,
                t.created_at,
                t.updated_at,
                COALESCE(att.has_in_progress_attempt, false) as has_in_progress_attempt,
                COALESCE(att.last_attempt_failed, false) as last_attempt_failed,
                att.executor

            FROM tasks t
            LEFT JOIN LATERAL (
                SELECT
                    EXISTS(SELECT 1 FROM task_attempts ta WHERE ta.task_id = t.id AND ta.status = 'running') as has_in_progress_attempt,
                    (la.status = 'failed') as last_attempt_failed,
                    la.metadata->>'executor' as executor
                FROM (SELECT status, metadata FROM task_attempts WHERE task_id = t.id ORDER BY created_at DESC LIMIT 1) la
            ) att ON true
            WHERE t.project_id = $1 AND t.sprint_id = $2
            ORDER BY
                CASE lower(COALESCE(t.metadata->>'priority', 'normal'))
                    WHEN 'critical' THEN 0
                    WHEN 'high' THEN 1
                    WHEN 'normal' THEN 2
                    WHEN 'medium' THEN 2
                    WHEN 'low' THEN 3
                    ELSE 2
                END,
                t.updated_at DESC
            "#,
        )
        .bind(project_id)
        .bind(sprint)
        .fetch_all(pool)
        .await?
    } else {
        // Query without sprint filter
        sqlx::query_as::<_, TaskWithAttemptStatus>(
            r#"
            SELECT
                t.id,
                t.project_id,
                t.requirement_id,
                t.sprint_id,
                t.title,
                t.description,
                t.task_type,
                t.status,
                t.assigned_to,
                t.parent_task_id,
                t.gitlab_issue_id,
                t.metadata,
                t.created_by,
                t.created_at,
                t.updated_at,
                COALESCE(att.has_in_progress_attempt, false) as has_in_progress_attempt,
                COALESCE(att.last_attempt_failed, false) as last_attempt_failed,
                att.executor

            FROM tasks t
            LEFT JOIN LATERAL (
                SELECT
                    EXISTS(SELECT 1 FROM task_attempts ta WHERE ta.task_id = t.id AND ta.status = 'running') as has_in_progress_attempt,
                    (la.status = 'failed') as last_attempt_failed,
                    la.metadata->>'executor' as executor
                FROM (SELECT status, metadata FROM task_attempts WHERE task_id = t.id ORDER BY created_at DESC LIMIT 1) la
            ) att ON true
            WHERE t.project_id = $1
            ORDER BY
                CASE lower(COALESCE(t.metadata->>'priority', 'normal'))
                    WHEN 'critical' THEN 0
                    WHEN 'high' THEN 1
                    WHEN 'normal' THEN 2
                    WHEN 'medium' THEN 2
                    WHEN 'low' THEN 3
                    ELSE 2
                END,
                t.updated_at DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?
    };

    Ok(tasks)
}

/// Get tasks across all projects a user has access to, with computed attempt status fields
pub async fn get_all_user_tasks_with_attempt_status(
    pool: &PgPool,
    user_id: Uuid,
    is_admin: bool,
) -> Result<Vec<TaskWithAttemptStatus>, sqlx::Error> {
    let query = if is_admin {
        r#"
        SELECT
            t.id,
            t.project_id,
            t.requirement_id,
            t.sprint_id,
            t.title,
            t.description,
            t.task_type,
            t.status,
            t.assigned_to,
            t.parent_task_id,
            t.gitlab_issue_id,
            t.metadata,
            t.created_by,
            t.created_at,
            t.updated_at,
            COALESCE(att.has_in_progress_attempt, false) as has_in_progress_attempt,
            COALESCE(att.last_attempt_failed, false) as last_attempt_failed,
            att.executor
        FROM tasks t
        LEFT JOIN LATERAL (
            SELECT
                EXISTS(SELECT 1 FROM task_attempts ta WHERE ta.task_id = t.id AND ta.status = 'running') as has_in_progress_attempt,
                (la.status = 'failed') as last_attempt_failed,
                la.metadata->>'executor' as executor
            FROM (SELECT status, metadata FROM task_attempts WHERE task_id = t.id ORDER BY created_at DESC LIMIT 1) la
        ) att ON true
        ORDER BY
            CASE lower(COALESCE(t.metadata->>'priority', 'normal'))
                WHEN 'critical' THEN 0
                WHEN 'high' THEN 1
                WHEN 'normal' THEN 2
                WHEN 'medium' THEN 2
                WHEN 'low' THEN 3
                ELSE 2
            END,
            t.updated_at DESC
        "#
    } else {
        r#"
        SELECT
            t.id,
            t.project_id,
            t.requirement_id,
            t.sprint_id,
            t.title,
            t.description,
            t.task_type,
            t.status,
            t.assigned_to,
            t.parent_task_id,
            t.gitlab_issue_id,
            t.metadata,
            t.created_by,
            t.created_at,
            t.updated_at,
            COALESCE(att.has_in_progress_attempt, false) as has_in_progress_attempt,
            COALESCE(att.last_attempt_failed, false) as last_attempt_failed,
            att.executor
        FROM tasks t
        JOIN project_members pm ON t.project_id = pm.project_id
        LEFT JOIN LATERAL (
            SELECT
                EXISTS(SELECT 1 FROM task_attempts ta WHERE ta.task_id = t.id AND ta.status = 'running') as has_in_progress_attempt,
                (la.status = 'failed') as last_attempt_failed,
                la.metadata->>'executor' as executor
            FROM (SELECT status, metadata FROM task_attempts WHERE task_id = t.id ORDER BY created_at DESC LIMIT 1) la
        ) att ON true
        WHERE pm.user_id = $1
        ORDER BY
            CASE lower(COALESCE(t.metadata->>'priority', 'normal'))
                WHEN 'critical' THEN 0
                WHEN 'high' THEN 1
                WHEN 'normal' THEN 2
                WHEN 'medium' THEN 2
                WHEN 'low' THEN 3
                ELSE 2
            END,
            t.updated_at DESC
        "#
    };

    let tasks = if is_admin {
        sqlx::query_as::<_, TaskWithAttemptStatus>(query)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query_as::<_, TaskWithAttemptStatus>(query)
            .bind(user_id)
            .fetch_all(pool)
            .await?
    };

    Ok(tasks)
}
