use acpms_db::models::NormalizedLogEntry;
use acpms_executors::normalization::LogNormalizer;
use acpms_executors::{NormalizedEntry, NormalizedEntryType};
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

/// Service for storing and retrieving normalized log entries
pub struct NormalizedLogService {
    pool: PgPool,
}

impl NormalizedLogService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Store a normalized entry in the database
    pub async fn store_entry(
        &self,
        attempt_id: Uuid,
        raw_log_id: Option<Uuid>,
        entry: &NormalizedEntry,
    ) -> Result<Uuid> {
        let entry_type = entry.entry_type();
        let entry_data = serde_json::to_value(entry)?;
        let line_number = entry.line_number() as i32;

        let id = sqlx::query_scalar(
            r#"INSERT INTO normalized_log_entries
               (attempt_id, raw_log_id, entry_type, entry_data, line_number)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id"#,
        )
        .bind(attempt_id)
        .bind(raw_log_id)
        .bind(entry_type)
        .bind(entry_data)
        .bind(line_number)
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    /// Store multiple normalized entries in a batch
    pub async fn store_entries_batch(
        &self,
        attempt_id: Uuid,
        raw_log_id: Option<Uuid>,
        entries: &[NormalizedEntry],
    ) -> Result<Vec<Uuid>> {
        let mut ids = Vec::with_capacity(entries.len());

        for entry in entries {
            let id = self.store_entry(attempt_id, raw_log_id, entry).await?;
            ids.push(id);
        }

        Ok(ids)
    }

    /// Store normalized entries with aggregation enabled
    ///
    /// This method aggregates consecutive Read/Grep/Glob operations (≥3 operations)
    /// into `AggregatedAction` entries before storing them in the database.
    ///
    /// # Arguments
    ///
    /// * `attempt_id` - The task attempt ID these entries belong to
    /// * `raw_log_id` - Optional raw log ID for traceability
    /// * `entries` - Slice of normalized entries to aggregate and store
    ///
    /// # Returns
    ///
    /// Vector of database IDs for the stored entries (aggregated entries count as one)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use acpms_services::NormalizedLogService;
    /// # use acpms_executors::NormalizedEntry;
    /// # use uuid::Uuid;
    /// # async fn example(service: NormalizedLogService, entries: Vec<NormalizedEntry>) {
    /// let attempt_id = Uuid::new_v4();
    /// let ids = service
    ///     .store_entries_with_aggregation(attempt_id, None, &entries)
    ///     .await
    ///     .expect("Failed to store entries");
    /// # }
    /// ```
    pub async fn store_entries_with_aggregation(
        &self,
        attempt_id: Uuid,
        raw_log_id: Option<Uuid>,
        entries: &[NormalizedEntry],
    ) -> Result<Vec<Uuid>> {
        // Create normalizer and aggregate consecutive actions
        let normalizer = LogNormalizer::new();
        let aggregated = normalizer.aggregate_consecutive_actions(entries);

        // Store the aggregated entries using the existing batch method
        self.store_entries_batch(attempt_id, raw_log_id, &aggregated)
            .await
    }

    /// Get all normalized entries for an attempt
    pub async fn get_entries_for_attempt(
        &self,
        attempt_id: Uuid,
    ) -> Result<Vec<NormalizedLogEntry>> {
        let entries = sqlx::query_as::<_, NormalizedLogEntry>(
            r#"SELECT * FROM normalized_log_entries
               WHERE attempt_id = $1
               ORDER BY created_at ASC, line_number ASC"#,
        )
        .bind(attempt_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(entries)
    }

    /// Get normalized entries by type for an attempt
    pub async fn get_entries_by_type(
        &self,
        attempt_id: Uuid,
        entry_type: &str,
    ) -> Result<Vec<NormalizedLogEntry>> {
        let entries = sqlx::query_as::<_, NormalizedLogEntry>(
            r#"SELECT * FROM normalized_log_entries
               WHERE attempt_id = $1 AND entry_type = $2
               ORDER BY created_at ASC, line_number ASC"#,
        )
        .bind(attempt_id)
        .bind(entry_type)
        .fetch_all(&self.pool)
        .await?;

        Ok(entries)
    }

    /// Get file changes for an attempt
    pub async fn get_file_changes(&self, attempt_id: Uuid) -> Result<Vec<NormalizedLogEntry>> {
        self.get_entries_by_type(attempt_id, "file_change").await
    }

    /// Get todo items for an attempt
    pub async fn get_todo_items(&self, attempt_id: Uuid) -> Result<Vec<NormalizedLogEntry>> {
        self.get_entries_by_type(attempt_id, "todo_item").await
    }

    /// Get tool actions for an attempt
    pub async fn get_actions(&self, attempt_id: Uuid) -> Result<Vec<NormalizedLogEntry>> {
        self.get_entries_by_type(attempt_id, "action").await
    }

    /// Get tool statuses for an attempt
    pub async fn get_tool_statuses(&self, attempt_id: Uuid) -> Result<Vec<NormalizedLogEntry>> {
        self.get_entries_by_type(attempt_id, "tool_status").await
    }

    /// Count normalized entries for an attempt
    pub async fn count_entries(&self, attempt_id: Uuid) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) as count
               FROM normalized_log_entries
               WHERE attempt_id = $1"#,
        )
        .bind(attempt_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Get paginated normalized entries with filters (server-side pagination, O(page_size)).
    /// Use this instead of loading all entries and paginating in memory.
    ///
    /// - `attempt_ids`: single or multiple attempt IDs (for include_subagents)
    /// - `entry_types`: filter by entry_type (None = no filter)
    /// - `tool_names`: for entry_type='action', filter by tool_name (None = no filter)
    pub async fn get_entries_paginated(
        &self,
        attempt_ids: &[Uuid],
        entry_types: Option<&[String]>,
        tool_names: Option<&[String]>,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<NormalizedLogEntry>, i64)> {
        if attempt_ids.is_empty() {
            return Ok((Vec::new(), 0));
        }

        let offset = (page.saturating_sub(1)) * page_size;
        let limit = page_size.max(1).min(500) as i64;

        // Build filter conditions for entry_types and tool_names
        // entry_types: (NULL or empty) => no filter; otherwise entry_type = ANY($entry_types)
        // tool_names: (NULL or empty) => no filter; otherwise for action entries: entry_data->>'tool_name' = ANY($tool_names)
        let entry_types_arr: Option<Vec<String>> =
            entry_types.filter(|s| !s.is_empty()).map(|s| s.to_vec());
        let tool_names_arr: Option<Vec<String>> =
            tool_names.filter(|s| !s.is_empty()).map(|s| s.to_vec());

        let total: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM normalized_log_entries
            WHERE attempt_id = ANY($1)
              AND ($2::text[] IS NULL OR coalesce(array_length($2::text[], 1), 0) = 0 OR entry_type = ANY($2))
              AND (
                $3::text[] IS NULL OR coalesce(array_length($3::text[], 1), 0) = 0
                OR entry_type != 'action'
                OR (entry_type = 'action' AND entry_data->>'tool_name' = ANY($3))
              )
            "#,
        )
        .bind(attempt_ids)
        .bind(&entry_types_arr)
        .bind(&tool_names_arr)
        .fetch_one(&self.pool)
        .await?;

        let entries = sqlx::query_as::<_, NormalizedLogEntry>(
            r#"
            SELECT id, attempt_id, raw_log_id, entry_type, entry_data, line_number, created_at
            FROM normalized_log_entries
            WHERE attempt_id = ANY($1)
              AND ($2::text[] IS NULL OR coalesce(array_length($2::text[], 1), 0) = 0 OR entry_type = ANY($2))
              AND (
                $3::text[] IS NULL OR coalesce(array_length($3::text[], 1), 0) = 0
                OR entry_type != 'action'
                OR (entry_type = 'action' AND entry_data->>'tool_name' = ANY($3))
              )
            ORDER BY created_at ASC, line_number ASC
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(attempt_ids)
        .bind(&entry_types_arr)
        .bind(&tool_names_arr)
        .bind(limit)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok((entries, total))
    }

    /// Delete all normalized entries for an attempt
    pub async fn delete_entries_for_attempt(&self, attempt_id: Uuid) -> Result<u64> {
        let result = sqlx::query(r#"DELETE FROM normalized_log_entries WHERE attempt_id = $1"#)
            .bind(attempt_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acpms_executors::{ActionType, FileChange, FileChangeType, TodoItem, TodoStatus};
    use chrono::Utc;

    #[sqlx::test(migrations = "../db/migrations")]
    async fn test_store_and_retrieve_entry(pool: PgPool) {
        let service = NormalizedLogService::new(pool.clone());

        // Create a test attempt
        let attempt_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Setup - Create user first (foreign key dependency)
        sqlx::query(
            "INSERT INTO users (id, email, name, global_roles)
             VALUES ($1, 'test@example.com', 'Test User', ARRAY['developer']::system_role[])",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO projects (id, name, created_by) VALUES ($1, 'Test', $2)")
            .bind(project_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO tasks (id, project_id, title, description, task_type, status, created_by)
             VALUES ($1, $2, 'Test', 'Test', 'feature', 'todo', $3)",
        )
        .bind(task_id)
        .bind(project_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO task_attempts (id, task_id, status)
             VALUES ($1, $2, 'queued')",
        )
        .bind(attempt_id)
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();

        // Create and store a normalized entry
        let entry = NormalizedEntry::Action(ActionType {
            tool_name: "Read".into(),
            action: "read".into(),
            target: Some("/path/file".into()),
            timestamp: Utc::now(),
            line_number: 10,
        });

        let entry_id = service.store_entry(attempt_id, None, &entry).await.unwrap();
        assert!(entry_id != Uuid::nil());

        // Retrieve entries
        let entries = service.get_entries_for_attempt(attempt_id).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entry_type, "action");
        assert_eq!(entries[0].line_number, 10);
    }

    #[sqlx::test(migrations = "../db/migrations")]
    async fn test_batch_store(pool: PgPool) {
        let service = NormalizedLogService::new(pool.clone());

        let attempt_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Setup - Create user first (foreign key dependency)
        sqlx::query(
            "INSERT INTO users (id, email, name, global_roles)
             VALUES ($1, 'test@example.com', 'Test User', ARRAY['developer']::system_role[])",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO projects (id, name, created_by) VALUES ($1, 'Test', $2)")
            .bind(project_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO tasks (id, project_id, title, description, task_type, status, created_by)
             VALUES ($1, $2, 'Test', 'Test', 'feature', 'todo', $3)",
        )
        .bind(task_id)
        .bind(project_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO task_attempts (id, task_id, status)
             VALUES ($1, $2, 'queued')",
        )
        .bind(attempt_id)
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();

        // Create multiple entries
        let entries = vec![
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("/file1".into()),
                timestamp: Utc::now(),
                line_number: 1,
            }),
            NormalizedEntry::FileChange(FileChange {
                path: "/file1".into(),
                change_type: FileChangeType::Modified,
                lines_added: Some(10),
                lines_removed: Some(5),
                timestamp: Utc::now(),
                line_number: 2,
            }),
            NormalizedEntry::TodoItem(TodoItem {
                status: TodoStatus::Pending,
                content: "Fix bug".into(),
                timestamp: Utc::now(),
                line_number: 3,
            }),
        ];

        let ids = service
            .store_entries_batch(attempt_id, None, &entries)
            .await
            .unwrap();

        assert_eq!(ids.len(), 3);

        // Verify count
        let count = service.count_entries(attempt_id).await.unwrap();
        assert_eq!(count, 3);
    }

    #[sqlx::test(migrations = "../db/migrations")]
    async fn test_store_with_aggregation(pool: PgPool) {
        let service = NormalizedLogService::new(pool.clone());

        let attempt_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Setup - Create user first (foreign key dependency)
        sqlx::query(
            "INSERT INTO users (id, email, name, global_roles)
             VALUES ($1, 'test@example.com', 'Test User', ARRAY['developer']::system_role[])",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO projects (id, name, created_by) VALUES ($1, 'Test', $2)")
            .bind(project_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO tasks (id, project_id, title, description, task_type, status, created_by)
             VALUES ($1, $2, 'Test', 'Test', 'feature', 'todo', $3)",
        )
        .bind(task_id)
        .bind(project_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO task_attempts (id, task_id, status)
             VALUES ($1, $2, 'queued')",
        )
        .bind(attempt_id)
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();

        // Create 5 consecutive Read actions (should aggregate)
        let entries = vec![
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("/file1.rs".into()),
                timestamp: Utc::now(),
                line_number: 1,
            }),
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("/file2.rs".into()),
                timestamp: Utc::now(),
                line_number: 2,
            }),
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("/file3.rs".into()),
                timestamp: Utc::now(),
                line_number: 3,
            }),
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("/file4.rs".into()),
                timestamp: Utc::now(),
                line_number: 4,
            }),
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("/file5.rs".into()),
                timestamp: Utc::now(),
                line_number: 5,
            }),
            // Non-aggregatable action
            NormalizedEntry::TodoItem(TodoItem {
                status: TodoStatus::Pending,
                content: "Review code".into(),
                timestamp: Utc::now(),
                line_number: 6,
            }),
        ];

        // Store with aggregation
        let ids = service
            .store_entries_with_aggregation(attempt_id, None, &entries)
            .await
            .unwrap();

        // Should have 2 entries: 1 AggregatedAction (5 Reads) + 1 TodoItem
        assert_eq!(ids.len(), 2);

        // Verify the stored entries
        let stored = service.get_entries_for_attempt(attempt_id).await.unwrap();
        assert_eq!(stored.len(), 2);

        // First entry should be aggregated_action
        assert_eq!(stored[0].entry_type, "aggregated_action");

        // Second entry should be todo_item
        assert_eq!(stored[1].entry_type, "todo_item");
    }

    #[sqlx::test(migrations = "../db/migrations")]
    async fn test_no_aggregation_for_few_actions(pool: PgPool) {
        let service = NormalizedLogService::new(pool.clone());

        let attempt_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Setup - Create user first (foreign key dependency)
        sqlx::query(
            "INSERT INTO users (id, email, name, global_roles)
             VALUES ($1, 'test@example.com', 'Test User', ARRAY['developer']::system_role[])",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO projects (id, name, created_by) VALUES ($1, 'Test', $2)")
            .bind(project_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO tasks (id, project_id, title, description, task_type, status, created_by)
             VALUES ($1, $2, 'Test', 'Test', 'feature', 'todo', $3)",
        )
        .bind(task_id)
        .bind(project_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO task_attempts (id, task_id, status)
             VALUES ($1, $2, 'queued')",
        )
        .bind(attempt_id)
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();

        // Create only 2 Read actions (should NOT aggregate, threshold is ≥3)
        let entries = vec![
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("/file1.rs".into()),
                timestamp: Utc::now(),
                line_number: 1,
            }),
            NormalizedEntry::Action(ActionType {
                tool_name: "Read".into(),
                action: "read".into(),
                target: Some("/file2.rs".into()),
                timestamp: Utc::now(),
                line_number: 2,
            }),
        ];

        // Store with aggregation
        let ids = service
            .store_entries_with_aggregation(attempt_id, None, &entries)
            .await
            .unwrap();

        // Should still have 2 individual actions (no aggregation)
        assert_eq!(ids.len(), 2);

        // Verify both are action type
        let stored = service.get_entries_for_attempt(attempt_id).await.unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].entry_type, "action");
        assert_eq!(stored[1].entry_type, "action");
    }
}
