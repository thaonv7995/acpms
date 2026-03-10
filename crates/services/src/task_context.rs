use acpms_db::{models::*, PgPool};
use anyhow::{anyhow, Context, Result};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

const ALLOWED_TASK_CONTEXT_CONTENT_TYPES: &[&str] = &["text/markdown", "text/plain"];
const ALLOWED_TASK_CONTEXT_SOURCES: &[&str] = &["user", "openclaw", "system"];

#[derive(Debug, Clone)]
pub struct TaskContextWithAttachments {
    pub context: TaskContext,
    pub attachments: Vec<TaskContextAttachment>,
}

#[derive(Debug, Clone)]
pub struct CreateTaskContextInput {
    pub title: Option<String>,
    pub content_type: String,
    pub raw_content: String,
    pub source: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateTaskContextInput {
    pub title: Option<Option<String>>,
    pub content_type: Option<String>,
    pub raw_content: Option<String>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct CreateTaskContextAttachmentInput {
    pub storage_key: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
}

pub struct TaskContextService {
    pool: PgPool,
}

impl TaskContextService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_task_contexts(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<TaskContextWithAttachments>> {
        let contexts = sqlx::query_as::<_, TaskContext>(
            r#"
            SELECT
                id, task_id, title, content_type, raw_content, source, sort_order,
                created_by, updated_by, created_at, updated_at
            FROM task_contexts
            WHERE task_id = $1
            ORDER BY sort_order ASC, created_at ASC
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list task contexts")?;

        if contexts.is_empty() {
            return Ok(Vec::new());
        }

        let attachments = sqlx::query_as::<_, TaskContextAttachment>(
            r#"
            SELECT
                a.id, a.task_context_id, a.storage_key, a.filename, a.content_type,
                a.size_bytes, a.checksum, a.created_by, a.created_at
            FROM task_context_attachments a
            INNER JOIN task_contexts c ON c.id = a.task_context_id
            WHERE c.task_id = $1
            ORDER BY a.created_at ASC
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list task context attachments")?;

        let mut by_context = std::collections::HashMap::<Uuid, Vec<TaskContextAttachment>>::new();
        for attachment in attachments {
            by_context
                .entry(attachment.task_context_id)
                .or_default()
                .push(attachment);
        }

        Ok(contexts
            .into_iter()
            .map(|context| TaskContextWithAttachments {
                attachments: by_context.remove(&context.id).unwrap_or_default(),
                context,
            })
            .collect())
    }

    pub async fn get_task_context(
        &self,
        task_id: Uuid,
        context_id: Uuid,
    ) -> Result<Option<TaskContextWithAttachments>> {
        let context = sqlx::query_as::<_, TaskContext>(
            r#"
            SELECT
                id, task_id, title, content_type, raw_content, source, sort_order,
                created_by, updated_by, created_at, updated_at
            FROM task_contexts
            WHERE id = $1 AND task_id = $2
            "#,
        )
        .bind(context_id)
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch task context")?;

        let Some(context) = context else {
            return Ok(None);
        };

        let attachments = sqlx::query_as::<_, TaskContextAttachment>(
            r#"
            SELECT
                id, task_context_id, storage_key, filename, content_type,
                size_bytes, checksum, created_by, created_at
            FROM task_context_attachments
            WHERE task_context_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(context_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch task context attachments")?;

        Ok(Some(TaskContextWithAttachments {
            context,
            attachments,
        }))
    }

    pub async fn count_task_contexts(&self, task_id: Uuid) -> Result<i64> {
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM task_contexts
            WHERE task_id = $1
            "#,
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to count task contexts")
    }

    pub async fn create_task_context(
        &self,
        task_id: Uuid,
        user_id: Uuid,
        input: CreateTaskContextInput,
    ) -> Result<TaskContext> {
        validate_content_type(&input.content_type)?;
        validate_source(&input.source)?;

        let task_context = sqlx::query_as::<_, TaskContext>(
            r#"
            INSERT INTO task_contexts (
                task_id, title, content_type, raw_content, source, sort_order, created_by, updated_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
            RETURNING
                id, task_id, title, content_type, raw_content, source, sort_order,
                created_by, updated_by, created_at, updated_at
            "#,
        )
        .bind(task_id)
        .bind(normalize_optional_title(input.title))
        .bind(input.content_type)
        .bind(input.raw_content)
        .bind(input.source)
        .bind(input.sort_order)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create task context")?;

        Ok(task_context)
    }

    pub async fn update_task_context(
        &self,
        task_id: Uuid,
        context_id: Uuid,
        user_id: Uuid,
        input: UpdateTaskContextInput,
    ) -> Result<TaskContext> {
        let existing = self
            .get_task_context(task_id, context_id)
            .await?
            .ok_or_else(|| anyhow!("Task context not found"))?;

        let title = input.title.unwrap_or(existing.context.title);
        let content_type = input.content_type.unwrap_or(existing.context.content_type);
        let raw_content = input.raw_content.unwrap_or(existing.context.raw_content);
        let sort_order = input.sort_order.unwrap_or(existing.context.sort_order);

        validate_content_type(&content_type)?;

        let task_context = sqlx::query_as::<_, TaskContext>(
            r#"
            UPDATE task_contexts
            SET title = $3,
                content_type = $4,
                raw_content = $5,
                sort_order = $6,
                updated_by = $7
            WHERE id = $1 AND task_id = $2
            RETURNING
                id, task_id, title, content_type, raw_content, source, sort_order,
                created_by, updated_by, created_at, updated_at
            "#,
        )
        .bind(context_id)
        .bind(task_id)
        .bind(normalize_optional_title(title))
        .bind(content_type)
        .bind(raw_content)
        .bind(sort_order)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to update task context")?;

        Ok(task_context)
    }

    pub async fn delete_task_context(&self, task_id: Uuid, context_id: Uuid) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin task context delete transaction")?;

        let deleted = sqlx::query(
            r#"
            DELETE FROM task_contexts
            WHERE id = $1 AND task_id = $2
            "#,
        )
        .bind(context_id)
        .bind(task_id)
        .execute(&mut *tx)
        .await
        .context("Failed to delete task context")?;

        if deleted.rows_affected() == 0 {
            return Err(anyhow!("Task context not found"));
        }

        self.sync_task_attachment_count_in_tx(&mut tx, task_id)
            .await?;
        tx.commit()
            .await
            .context("Failed to commit task context delete transaction")?;

        Ok(())
    }

    pub async fn create_attachment(
        &self,
        task_id: Uuid,
        context_id: Uuid,
        user_id: Uuid,
        input: CreateTaskContextAttachmentInput,
    ) -> Result<TaskContextAttachment> {
        self.get_task_context(task_id, context_id)
            .await?
            .ok_or_else(|| anyhow!("Task context not found"))?;

        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin task context attachment transaction")?;

        let attachment = sqlx::query_as::<_, TaskContextAttachment>(
            r#"
            INSERT INTO task_context_attachments (
                task_context_id, storage_key, filename, content_type, size_bytes, checksum, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id, task_context_id, storage_key, filename, content_type,
                size_bytes, checksum, created_by, created_at
            "#,
        )
        .bind(context_id)
        .bind(input.storage_key)
        .bind(input.filename)
        .bind(input.content_type)
        .bind(input.size_bytes)
        .bind(input.checksum)
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to create task context attachment")?;

        self.sync_task_attachment_count_in_tx(&mut tx, task_id)
            .await?;
        tx.commit()
            .await
            .context("Failed to commit task context attachment transaction")?;

        Ok(attachment)
    }

    pub async fn delete_attachment(
        &self,
        task_id: Uuid,
        context_id: Uuid,
        attachment_id: Uuid,
    ) -> Result<Option<String>> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin task context attachment delete transaction")?;

        let storage_key = sqlx::query_scalar::<_, String>(
            r#"
            DELETE FROM task_context_attachments
            WHERE id = $1
              AND task_context_id = $2
              AND EXISTS (
                  SELECT 1
                  FROM task_contexts
                  WHERE id = $2 AND task_id = $3
              )
            RETURNING storage_key
            "#,
        )
        .bind(attachment_id)
        .bind(context_id)
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await
        .context("Failed to delete task context attachment")?;

        if storage_key.is_none() {
            return Err(anyhow!("Task context attachment not found"));
        }

        self.sync_task_attachment_count_in_tx(&mut tx, task_id)
            .await?;
        tx.commit()
            .await
            .context("Failed to commit task context attachment delete transaction")?;

        Ok(storage_key)
    }

    pub async fn count_task_context_attachments(&self, task_id: Uuid) -> Result<i64> {
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM task_context_attachments a
            INNER JOIN task_contexts c ON c.id = a.task_context_id
            WHERE c.task_id = $1
            "#,
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to count task context attachments")
    }

    pub async fn sync_task_attachment_count(&self, task_id: Uuid) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin task attachment count sync transaction")?;
        self.sync_task_attachment_count_in_tx(&mut tx, task_id)
            .await?;
        tx.commit()
            .await
            .context("Failed to commit task attachment count sync transaction")?;
        Ok(())
    }

    async fn sync_task_attachment_count_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        task_id: Uuid,
    ) -> Result<()> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM task_context_attachments a
            INNER JOIN task_contexts c ON c.id = a.task_context_id
            WHERE c.task_id = $1
            "#,
        )
        .bind(task_id)
        .fetch_one(&mut **tx)
        .await
        .context("Failed to count task attachments during sync")?;

        sqlx::query(
            r#"
            UPDATE tasks
            SET metadata = COALESCE(metadata, '{}'::jsonb) || jsonb_build_object('attachments_count', $2),
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(task_id)
        .bind(count)
        .execute(&mut **tx)
        .await
        .context("Failed to sync task attachment count")?;

        Ok(())
    }
}

fn validate_content_type(value: &str) -> Result<()> {
    if ALLOWED_TASK_CONTEXT_CONTENT_TYPES.contains(&value) {
        Ok(())
    } else {
        Err(anyhow!("Unsupported task context content_type"))
    }
}

fn validate_source(value: &str) -> Result<()> {
    if ALLOWED_TASK_CONTEXT_SOURCES.contains(&value) {
        Ok(())
    } else {
        Err(anyhow!("Unsupported task context source"))
    }
}

fn normalize_optional_title(value: Option<String>) -> Option<String> {
    value
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
}
