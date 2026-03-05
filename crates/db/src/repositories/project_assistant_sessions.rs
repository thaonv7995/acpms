use crate::models::ProjectAssistantSession;
use chrono::Utc;
use sqlx::{PgPool, Postgres};
use uuid::Uuid;

/// Find active session for project and user
pub async fn find_active_by_project_and_user(
    pool: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<Option<ProjectAssistantSession>, sqlx::Error> {
    sqlx::query_as::<_, ProjectAssistantSession>(
        r#"
        SELECT id, project_id, user_id, status, s3_log_key, created_at, ended_at
        FROM project_assistant_sessions
        WHERE project_id = $1 AND user_id = $2 AND status = 'active'
        "#,
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// List sessions for project and user, sorted by created_at desc
pub async fn list_by_project_and_user(
    pool: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<ProjectAssistantSession>, sqlx::Error> {
    sqlx::query_as::<_, ProjectAssistantSession>(
        r#"
        SELECT id, project_id, user_id, status, s3_log_key, created_at, ended_at
        FROM project_assistant_sessions
        WHERE project_id = $1 AND user_id = $2
        ORDER BY created_at DESC
        LIMIT $3
        "#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Get session by id
pub async fn get_by_id(
    pool: &PgPool,
    session_id: Uuid,
) -> Result<Option<ProjectAssistantSession>, sqlx::Error> {
    sqlx::query_as::<_, ProjectAssistantSession>(
        r#"
        SELECT id, project_id, user_id, status, s3_log_key, created_at, ended_at
        FROM project_assistant_sessions
        WHERE id = $1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
}

/// Create new session
pub async fn create(
    pool: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<ProjectAssistantSession, sqlx::Error> {
    create_tx(pool, project_id, user_id).await
}

/// End active sessions for user in project (for force_new flow)
pub async fn end_active_for_user(
    executor: impl sqlx::Executor<'_, Database = Postgres>,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE project_assistant_sessions
        SET status = 'ended', ended_at = $1
        WHERE project_id = $2 AND user_id = $3 AND status = 'active'
        "#,
    )
    .bind(Utc::now())
    .bind(project_id)
    .bind(user_id)
    .execute(executor)
    .await?;

    Ok(result.rows_affected())
}

/// Create session (for use in transaction)
pub async fn create_tx(
    executor: impl sqlx::Executor<'_, Database = Postgres>,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<ProjectAssistantSession, sqlx::Error> {
    sqlx::query_as::<_, ProjectAssistantSession>(
        r#"
        INSERT INTO project_assistant_sessions (project_id, user_id, status)
        VALUES ($1, $2, 'active')
        RETURNING id, project_id, user_id, status, s3_log_key, created_at, ended_at
        "#,
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_one(executor)
    .await
}

/// Delete sessions beyond the N most recent per project/user (retention: keep only recent)
pub async fn delete_beyond_recent(
    pool: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
    keep: i64,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        WITH keep_ids AS (
            SELECT id FROM project_assistant_sessions
            WHERE project_id = $1 AND user_id = $2
            ORDER BY created_at DESC
            LIMIT $3
        )
        DELETE FROM project_assistant_sessions
        WHERE project_id = $1 AND user_id = $2
        AND id NOT IN (SELECT id FROM keep_ids)
        "#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(keep)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Update only the s3_log_key on a session (e.g. after background S3 upload).
pub async fn update_s3_log_key(
    pool: &PgPool,
    session_id: Uuid,
    s3_log_key: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE project_assistant_sessions
        SET s3_log_key = $1
        WHERE id = $2
        "#,
    )
    .bind(s3_log_key)
    .bind(session_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// End session and set s3_log_key
pub async fn end_session(
    pool: &PgPool,
    session_id: Uuid,
    s3_log_key: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE project_assistant_sessions
        SET status = 'ended', ended_at = $1, s3_log_key = $2
        WHERE id = $3 AND status = 'active'
        "#,
    )
    .bind(Utc::now())
    .bind(s3_log_key)
    .bind(session_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
