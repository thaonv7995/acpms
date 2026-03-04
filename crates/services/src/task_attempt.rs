use acpms_db::{models::*, PgPool};
use anyhow::{Context, Result};
use uuid::Uuid;

pub struct TaskAttemptService {
    pool: PgPool,
}

impl TaskAttemptService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn has_active_attempt(&self, task_id: Uuid) -> Result<bool> {
        let has_active: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM task_attempts
                WHERE task_id = $1
                  AND status IN ('queued', 'running')
            )
            "#,
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check active task attempts")?;

        Ok(has_active)
    }

    pub async fn create_attempt_with_metadata(
        &self,
        task_id: Uuid,
        metadata: serde_json::Value,
    ) -> Result<TaskAttempt> {
        if self.has_active_attempt(task_id).await? {
            anyhow::bail!("Task already has an active attempt (queued or running)");
        }

        let attempt = sqlx::query_as::<_, TaskAttempt>(
            r#"
            INSERT INTO task_attempts (task_id, metadata)
            VALUES ($1, $2)
            RETURNING id, task_id, status, started_at, completed_at, error_message, metadata, created_at,
                      diff_total_files, diff_total_additions, diff_total_deletions, diff_saved_at,
                      s3_diff_key, s3_diff_size, s3_diff_saved_at, s3_log_key
            "#,
        )
        .bind(task_id)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create task attempt")?;

        Ok(attempt)
    }

    pub async fn create_attempt(&self, task_id: Uuid) -> Result<TaskAttempt> {
        self.create_attempt_with_metadata(task_id, serde_json::json!({}))
            .await
    }

    pub async fn create_attempt_with_status_and_metadata(
        &self,
        task_id: Uuid,
        status: AttemptStatus,
        metadata: serde_json::Value,
    ) -> Result<TaskAttempt> {
        if self.has_active_attempt(task_id).await? {
            anyhow::bail!("Task already has an active attempt (queued or running)");
        }

        let attempt = sqlx::query_as::<_, TaskAttempt>(
            r#"
            INSERT INTO task_attempts (task_id, status, metadata)
            VALUES ($1, $2, $3)
            RETURNING id, task_id, status, started_at, completed_at, error_message, metadata, created_at,
                      diff_total_files, diff_total_additions, diff_total_deletions, diff_saved_at,
                      s3_diff_key, s3_diff_size, s3_diff_saved_at, s3_log_key
            "#,
        )
        .bind(task_id)
        .bind(status)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create task attempt")?;

        Ok(attempt)
    }

    pub async fn get_attempt(&self, attempt_id: Uuid) -> Result<Option<TaskAttempt>> {
        let attempt = sqlx::query_as::<_, TaskAttempt>(
            r#"
            SELECT id, task_id, status, started_at, completed_at, error_message, metadata, created_at,
                   diff_total_files, diff_total_additions, diff_total_deletions, diff_saved_at,
                   s3_diff_key, s3_diff_size, s3_diff_saved_at, s3_log_key
            FROM task_attempts
            WHERE id = $1
            "#
        )
        .bind(attempt_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch task attempt")?;

        Ok(attempt)
    }

    pub async fn get_task_attempts(&self, task_id: Uuid) -> Result<Vec<TaskAttempt>> {
        let attempts = sqlx::query_as::<_, TaskAttempt>(
            r#"
            SELECT id, task_id, status, started_at, completed_at, error_message, metadata, created_at,
                   diff_total_files, diff_total_additions, diff_total_deletions, diff_saved_at,
                   s3_diff_key, s3_diff_size, s3_diff_saved_at, s3_log_key
            FROM task_attempts
            WHERE task_id = $1
            ORDER BY created_at DESC
            "#
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch task attempts")?;

        Ok(attempts)
    }

    /// Atomically transition a completed attempt (success/failed/cancelled) to running.
    /// Returns Some(attempt) if transition succeeded, None if 0 rows (already transitioned or wrong state).
    /// Returns Err on unique constraint violation (another attempt for same task is already queued/running).
    pub async fn transition_completed_to_running(
        &self,
        attempt_id: Uuid,
    ) -> Result<Option<TaskAttempt>> {
        let now = chrono::Utc::now();
        let result = sqlx::query_as::<_, TaskAttempt>(
            r#"
            UPDATE task_attempts
            SET status = 'running',
                started_at = $2,
                completed_at = NULL,
                error_message = NULL
            WHERE id = $1
              AND status IN ('success', 'failed', 'cancelled')
            RETURNING id, task_id, status, started_at, completed_at, error_message, metadata, created_at,
                      diff_total_files, diff_total_additions, diff_total_deletions, diff_saved_at,
                      s3_diff_key, s3_diff_size, s3_diff_saved_at, s3_log_key
            "#,
        )
        .bind(attempt_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await;

        match result {
            Ok(Some(attempt)) => Ok(Some(attempt)),
            Ok(None) => Ok(None),
            Err(e) => {
                if let sqlx::Error::Database(db_err) = &e {
                    if db_err.is_unique_violation() {
                        anyhow::bail!("Task already has an active attempt (queued or running)");
                    }
                }
                Err(e.into())
            }
        }
    }

    pub async fn update_status(
        &self,
        attempt_id: Uuid,
        status: AttemptStatus,
        error_message: Option<String>,
    ) -> Result<TaskAttempt> {
        let now = chrono::Utc::now();
        let (started_at, completed_at) = match status {
            AttemptStatus::Running => (Some(now), None),
            AttemptStatus::Success | AttemptStatus::Failed | AttemptStatus::Cancelled => {
                (None, Some(now))
            }
            _ => (None, None),
        };

        let attempt = sqlx::query_as::<_, TaskAttempt>(
            r#"
            UPDATE task_attempts
            SET status = $2,
                started_at = COALESCE($3, started_at),
                completed_at = COALESCE($4, completed_at),
                error_message = $5
            WHERE id = $1
            RETURNING id, task_id, status, started_at, completed_at, error_message, metadata, created_at,
                      diff_total_files, diff_total_additions, diff_total_deletions, diff_saved_at,
                      s3_diff_key, s3_diff_size, s3_diff_saved_at, s3_log_key
            "#
        )
        .bind(attempt_id)
        .bind(status)
        .bind(started_at)
        .bind(completed_at)
        .bind(error_message)
        .fetch_one(&self.pool)
        .await
        .context("Failed to update task attempt status")?;

        Ok(attempt)
    }

    /// Update content of a user message log (JSONL append - no agent_logs).
    /// Appends update line; when loading, last per id wins.
    pub async fn update_log_content(
        &self,
        log_id: Uuid,
        attempt_id: Uuid,
        content: &str,
    ) -> Result<AgentLog> {
        let created_at = chrono::Utc::now();
        acpms_executors::append_log_to_jsonl(attempt_id, "user", content, log_id, created_at)
            .await
            .context("Failed to append log update to JSONL")?;
        Ok(AgentLog {
            id: log_id,
            attempt_id,
            log_type: "user".to_string(),
            content: content.to_string(),
            created_at,
        })
    }

    /// Update s3_log_key for an attempt (after uploading JSONL to S3).
    pub async fn update_s3_log_key(&self, attempt_id: Uuid, s3_log_key: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE task_attempts
            SET s3_log_key = $1
            WHERE id = $2
            "#,
        )
        .bind(s3_log_key)
        .bind(attempt_id)
        .execute(&self.pool)
        .await
        .context("Failed to update s3_log_key")?;
        Ok(())
    }

    /// Cursor-based pagination for attempt logs (newest page first, returned in ASC order for UI append/prepend).
    /// Save file diffs to database for an attempt
    pub async fn save_file_diffs(&self, attempt_id: Uuid, diffs: Vec<FileDiffInput>) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to start transaction")?;

        // Delete any existing diffs for this attempt (in case of re-save)
        sqlx::query("DELETE FROM file_diffs WHERE attempt_id = $1")
            .bind(attempt_id)
            .execute(&mut *tx)
            .await
            .context("Failed to delete existing diffs")?;

        let mut total_additions = 0i32;
        let mut total_deletions = 0i32;

        // Insert each file diff
        for diff in &diffs {
            sqlx::query(
                r#"
                INSERT INTO file_diffs (attempt_id, file_path, old_path, change_type, additions, deletions, old_content, new_content)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#
            )
            .bind(attempt_id)
            .bind(&diff.file_path)
            .bind(&diff.old_path)
            .bind(&diff.change_type)
            .bind(diff.additions)
            .bind(diff.deletions)
            .bind(&diff.old_content)
            .bind(&diff.new_content)
            .execute(&mut *tx)
            .await
            .context("Failed to insert file diff")?;

            total_additions += diff.additions;
            total_deletions += diff.deletions;
        }

        // Update attempt with diff summary
        sqlx::query(
            r#"
            UPDATE task_attempts
            SET diff_total_files = $2,
                diff_total_additions = $3,
                diff_total_deletions = $4,
                diff_saved_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(attempt_id)
        .bind(diffs.len() as i32)
        .bind(total_additions)
        .bind(total_deletions)
        .execute(&mut *tx)
        .await
        .context("Failed to update attempt diff summary")?;

        tx.commit().await.context("Failed to commit transaction")?;

        Ok(())
    }

    /// Retrieve saved file diffs from database
    pub async fn get_saved_diffs(&self, attempt_id: Uuid) -> Result<Vec<FileDiff>> {
        let diffs = sqlx::query_as::<_, FileDiff>(
            r#"
            SELECT id, attempt_id, file_path, old_path, change_type, additions, deletions, old_content, new_content, created_at
            FROM file_diffs
            WHERE attempt_id = $1
            ORDER BY file_path ASC
            "#
        )
        .bind(attempt_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch saved file diffs")?;

        Ok(diffs)
    }

    /// Check if diffs are saved in database for an attempt
    pub async fn has_saved_diffs(&self, attempt_id: Uuid) -> Result<bool> {
        let result =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM file_diffs WHERE attempt_id = $1")
                .bind(attempt_id)
                .fetch_one(&self.pool)
                .await
                .context("Failed to check saved diffs")?;

        Ok(result > 0)
    }
}

/// Input struct for saving file diffs (without id and created_at)
#[derive(Debug, Clone)]
pub struct FileDiffInput {
    pub file_path: String,
    pub old_path: Option<String>,
    pub change_type: String,
    pub additions: i32,
    pub deletions: i32,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
}
