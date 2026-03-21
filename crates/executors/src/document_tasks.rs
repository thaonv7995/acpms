use std::sync::Arc;

use acpms_db::{
    models::{ProjectDocument, Task, TaskType},
    PgPool,
};
use acpms_utils::{
    build_project_document_chunks, is_indexable_project_document_content_type,
    normalize_project_document_text,
};
use anyhow::{Context, Result};
use sqlx::FromRow;
use uuid::Uuid;

use crate::DiffStorageUploader;

#[derive(Debug, Clone, FromRow)]
struct TaskContextRecord {
    raw_content: String,
}

#[derive(Debug, Clone, FromRow)]
struct TaskAttachmentRecord {
    storage_key: String,
    filename: String,
    content_type: String,
    size_bytes: Option<i64>,
    checksum: Option<String>,
}

struct PublishPayload {
    title: String,
    filename: String,
    document_kind: String,
    content_type: String,
    storage_key: String,
    checksum: Option<String>,
    size_bytes: i64,
    source: String,
    summary_text: String,
}

pub async fn publish_docs_task_to_vault(
    pool: &PgPool,
    storage: &Arc<dyn DiffStorageUploader>,
    task_id: Uuid,
) -> Result<Option<ProjectDocument>> {
    let task = sqlx::query_as::<_, Task>(
        r#"
        SELECT id, project_id, title, description, task_type, status, assigned_to, parent_task_id,
               requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
        FROM tasks
        WHERE id = $1
        "#,
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .context("Failed to load task for document publish")?;

    let Some(task) = task else {
        return Ok(None);
    };

    if task.task_type != TaskType::Docs {
        return Ok(None);
    }

    let publish_policy = task
        .metadata
        .get("document")
        .and_then(|value| value.get("publish_policy"))
        .and_then(|value| value.as_str())
        .unwrap_or("final_on_done");
    if !publish_policy.eq_ignore_ascii_case("final_on_done") {
        return Ok(None);
    }

    let contexts = sqlx::query_as::<_, TaskContextRecord>(
        r#"
        SELECT raw_content
        FROM task_contexts
        WHERE task_id = $1
        ORDER BY sort_order ASC, created_at ASC
        "#,
    )
    .bind(task.id)
    .fetch_all(pool)
    .await
    .context("Failed to load task contexts for document publish")?;

    let attachments = sqlx::query_as::<_, TaskAttachmentRecord>(
        r#"
        SELECT a.storage_key, a.filename, a.content_type, a.size_bytes, a.checksum
        FROM task_context_attachments a
        INNER JOIN task_contexts c ON c.id = a.task_context_id
        WHERE c.task_id = $1
        ORDER BY c.sort_order ASC, a.created_at ASC
        "#,
    )
    .bind(task.id)
    .fetch_all(pool)
    .await
    .context("Failed to load task attachments for document publish")?;

    let payload = build_publish_payload(&task, &contexts, &attachments, storage)
        .await
        .context("Failed to resolve document publish payload")?;

    let mut tx = pool
        .begin()
        .await
        .context("Failed to begin docs publish transaction")?;

    let existing_document_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id
        FROM project_documents
        WHERE project_id = $1 AND filename = $2
        "#,
    )
    .bind(task.project_id)
    .bind(&payload.filename)
    .fetch_optional(&mut *tx)
    .await
    .context("Failed to look up existing task-backed project document")?;

    let document = if let Some(document_id) = existing_document_id {
        sqlx::query_as::<_, ProjectDocument>(
            r#"
            UPDATE project_documents
            SET title = $3,
                document_kind = $4,
                content_type = $5,
                storage_key = $6,
                checksum = $7,
                size_bytes = $8,
                source = $9,
                version = version + 1,
                ingestion_status = 'pending',
                index_error = NULL,
                indexed_at = NULL,
                updated_by = $10,
                updated_at = NOW()
            WHERE project_id = $1 AND id = $2
            RETURNING id, project_id, title, filename, document_kind, content_type, storage_key,
                      checksum, size_bytes, source, version, ingestion_status, index_error,
                      indexed_at, created_by, updated_by, created_at, updated_at
            "#,
        )
        .bind(task.project_id)
        .bind(document_id)
        .bind(&payload.title)
        .bind(&payload.document_kind)
        .bind(&payload.content_type)
        .bind(&payload.storage_key)
        .bind(&payload.checksum)
        .bind(payload.size_bytes)
        .bind(&payload.source)
        .bind(task.created_by)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to update task-backed project document")?
    } else {
        sqlx::query_as::<_, ProjectDocument>(
            r#"
            INSERT INTO project_documents (
                project_id, title, filename, document_kind, content_type, storage_key,
                checksum, size_bytes, source, version, ingestion_status, created_by, updated_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1, 'pending', $10, $10)
            RETURNING id, project_id, title, filename, document_kind, content_type, storage_key,
                      checksum, size_bytes, source, version, ingestion_status, index_error,
                      indexed_at, created_by, updated_by, created_at, updated_at
            "#,
        )
        .bind(task.project_id)
        .bind(&payload.title)
        .bind(&payload.filename)
        .bind(&payload.document_kind)
        .bind(&payload.content_type)
        .bind(&payload.storage_key)
        .bind(&payload.checksum)
        .bind(payload.size_bytes)
        .bind(&payload.source)
        .bind(task.created_by)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to create task-backed project document")?
    };

    let indexed_document = index_document(pool, &mut tx, storage, document, &payload.summary_text)
        .await
        .context("Failed to index published task document")?;

    let mut next_metadata = task.metadata.clone();
    if !next_metadata.is_object() {
        next_metadata = serde_json::json!({});
    }
    let document_meta = next_metadata
        .get("document")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let mut document_meta = document_meta;
    document_meta.insert(
        "vault_document_id".into(),
        serde_json::Value::String(indexed_document.id.to_string()),
    );
    document_meta.insert(
        "preview_mode".into(),
        serde_json::Value::String("document".to_string()),
    );
    if document_meta
        .get("title")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        document_meta.insert(
            "title".into(),
            serde_json::Value::String(indexed_document.title.clone()),
        );
    }
    if let Some(object) = next_metadata.as_object_mut() {
        object.insert("document".into(), serde_json::Value::Object(document_meta));
    }

    sqlx::query(
        r#"
        UPDATE tasks
        SET metadata = $2,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(task.id)
    .bind(&next_metadata)
    .execute(&mut *tx)
    .await
    .context("Failed to store vault_document_id on task metadata")?;

    tx.commit()
        .await
        .context("Failed to commit docs publish transaction")?;

    Ok(Some(indexed_document))
}

async fn build_publish_payload(
    task: &Task,
    contexts: &[TaskContextRecord],
    attachments: &[TaskAttachmentRecord],
    storage: &Arc<dyn DiffStorageUploader>,
) -> Result<PublishPayload> {
    let document_meta = task
        .metadata
        .get("document")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let primary_context = contexts.first();
    let notes = primary_context
        .map(|context| context.raw_content.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or_default();

    let title = document_meta
        .get("title")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(task.title.as_str())
        .to_string();
    let document_kind = document_meta
        .get("kind")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("other")
        .to_string();
    let format = document_meta
        .get("format")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if document_meta
                .get("figma_url")
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .is_some()
            {
                "figma_link"
            } else {
                "markdown"
            }
        });

    if format.eq_ignore_ascii_case("figma_link") {
        let markdown = build_summary_markdown(task, &title, &document_kind, format, notes);
        let filename = stable_document_filename(task.id, &title, "md");
        let storage_key = project_document_storage_key(task.project_id, task.id, &filename);
        storage
            .upload_object_bytes(&storage_key, markdown.as_bytes(), "text/markdown")
            .await
            .context("Failed to upload figma link document")?;
        return Ok(PublishPayload {
            title,
            filename,
            document_kind,
            content_type: "text/markdown".to_string(),
            storage_key,
            checksum: None,
            size_bytes: markdown.len() as i64,
            source: "api".to_string(),
            summary_text: markdown,
        });
    }

    if format.eq_ignore_ascii_case("markdown") && !notes.is_empty() {
        let markdown = notes.to_string();
        let filename = stable_document_filename(task.id, &title, "md");
        let storage_key = project_document_storage_key(task.project_id, task.id, &filename);
        storage
            .upload_object_bytes(&storage_key, markdown.as_bytes(), "text/markdown")
            .await
            .context("Failed to upload markdown task document")?;
        return Ok(PublishPayload {
            title,
            filename,
            document_kind,
            content_type: "text/markdown".to_string(),
            storage_key,
            checksum: None,
            size_bytes: markdown.len() as i64,
            source: "api".to_string(),
            summary_text: markdown,
        });
    }

    if let Some(attachment) = select_best_attachment(format, attachments) {
        let bytes = storage
            .download_object_bytes(&attachment.storage_key)
            .await
            .with_context(|| {
                format!(
                    "Failed to download source attachment {} for document publish",
                    attachment.storage_key
                )
            })?;
        let extension = extension_for_attachment(&attachment.content_type, &attachment.filename);
        let filename = stable_document_filename(task.id, &title, extension);
        let storage_key = project_document_storage_key(task.project_id, task.id, &filename);
        storage
            .upload_object_bytes(&storage_key, &bytes, &attachment.content_type)
            .await
            .with_context(|| {
                format!(
                    "Failed to upload copied attachment {} into project documents",
                    attachment.storage_key
                )
            })?;
        let summary_text = build_summary_text(
            task,
            &title,
            &document_kind,
            format,
            notes,
            Some(attachment),
            None,
        );
        return Ok(PublishPayload {
            title,
            filename,
            document_kind,
            content_type: attachment.content_type.clone(),
            storage_key,
            checksum: attachment.checksum.clone(),
            size_bytes: attachment.size_bytes.unwrap_or(bytes.len() as i64),
            source: "upload".to_string(),
            summary_text,
        });
    }

    let markdown = build_summary_markdown(task, &title, &document_kind, format, notes);
    let filename = stable_document_filename(task.id, &title, "md");
    let storage_key = project_document_storage_key(task.project_id, task.id, &filename);
    storage
        .upload_object_bytes(&storage_key, markdown.as_bytes(), "text/markdown")
        .await
        .context("Failed to upload fallback markdown summary for docs task")?;

    Ok(PublishPayload {
        title,
        filename,
        document_kind,
        content_type: "text/markdown".to_string(),
        storage_key,
        checksum: None,
        size_bytes: markdown.len() as i64,
        source: "api".to_string(),
        summary_text: markdown,
    })
}

async fn index_document(
    _pool: &PgPool,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    storage: &Arc<dyn DiffStorageUploader>,
    document: ProjectDocument,
    summary_text: &str,
) -> Result<ProjectDocument> {
    let text = if is_indexable_project_document_content_type(&document.content_type) {
        let bytes = storage
            .download_object_bytes(&document.storage_key)
            .await
            .with_context(|| format!("Failed to download {}", document.storage_key))?;
        normalize_project_document_text(&document.content_type, &bytes)
            .unwrap_or_else(|_| summary_text.to_string())
    } else {
        summary_text.to_string()
    };

    let chunks = build_project_document_chunks(&text);

    sqlx::query("DELETE FROM project_document_chunks WHERE document_id = $1")
        .bind(document.id)
        .execute(&mut **tx)
        .await
        .context("Failed to delete stale project document chunks")?;

    for chunk in chunks {
        sqlx::query(
            r#"
            INSERT INTO project_document_chunks (
                document_id, project_id, chunk_index, content, content_hash, token_count, embedding
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(document.id)
        .bind(document.project_id)
        .bind(chunk.chunk_index as i32)
        .bind(&chunk.content)
        .bind(&chunk.content_hash)
        .bind(chunk.token_count as i32)
        .bind(&chunk.embedding)
        .execute(&mut **tx)
        .await
        .context("Failed to insert project document chunk")?;
    }

    let updated = sqlx::query_as::<_, ProjectDocument>(
        r#"
        UPDATE project_documents
        SET ingestion_status = 'indexed',
            index_error = NULL,
            indexed_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, project_id, title, filename, document_kind, content_type, storage_key,
                  checksum, size_bytes, source, version, ingestion_status, index_error,
                  indexed_at, created_by, updated_by, created_at, updated_at
        "#,
    )
    .bind(document.id)
    .fetch_one(&mut **tx)
    .await
    .context("Failed to mark project document indexed")?;

    Ok(updated)
}

fn select_best_attachment<'a>(
    format: &str,
    attachments: &'a [TaskAttachmentRecord],
) -> Option<&'a TaskAttachmentRecord> {
    let normalized = format.trim().to_ascii_lowercase();
    attachments
        .iter()
        .find(|attachment| match normalized.as_str() {
            "pdf" => attachment
                .content_type
                .eq_ignore_ascii_case("application/pdf"),
            "image" => attachment.content_type.starts_with("image/"),
            "markdown" => {
                attachment.content_type.starts_with("text/")
                    || matches!(
                        attachment.content_type.as_str(),
                        "application/json"
                            | "application/yaml"
                            | "application/x-yaml"
                            | "application/xml"
                            | "application/toml"
                    )
            }
            "binary" => true,
            _ => true,
        })
}

fn build_summary_markdown(
    task: &Task,
    title: &str,
    document_kind: &str,
    format: &str,
    notes: &str,
) -> String {
    let mut lines = vec![
        format!("# {}", title),
        String::new(),
        format!("- Task ID: `{}`", task.id),
        format!("- Task Type: `{}`", task_type_label(task.task_type)),
        format!("- Document Kind: `{}`", document_kind),
        format!("- Format: `{}`", format),
    ];

    if let Some(source_url) = task
        .metadata
        .get("document")
        .and_then(|value| value.get("source_url"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("- Source URL: {}", source_url));
    }

    if let Some(figma_url) = task
        .metadata
        .get("document")
        .and_then(|value| value.get("figma_url"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("- Figma URL: {}", figma_url));
    }

    if let Some(figma_node_id) = task
        .metadata
        .get("document")
        .and_then(|value| value.get("figma_node_id"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("- Figma Node ID: `{}`", figma_node_id));
    }

    if let Some(description) = task
        .description
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(String::new());
        lines.push("## Task Summary".to_string());
        lines.push(String::new());
        lines.push(description.trim().to_string());
    }

    if !notes.trim().is_empty() {
        lines.push(String::new());
        lines.push("## Notes".to_string());
        lines.push(String::new());
        lines.push(notes.trim().to_string());
    }

    lines.join("\n")
}

fn build_summary_text(
    task: &Task,
    title: &str,
    document_kind: &str,
    format: &str,
    notes: &str,
    attachment: Option<&TaskAttachmentRecord>,
    extra_caption: Option<&str>,
) -> String {
    let mut lines = vec![
        format!("Document title: {}", title),
        format!("Task id: {}", task.id),
        format!("Task type: docs"),
        format!("Document kind: {}", document_kind),
        format!("Document format: {}", format),
    ];

    if let Some(attachment) = attachment {
        lines.push(format!("Attachment filename: {}", attachment.filename));
        lines.push(format!(
            "Attachment content type: {}",
            attachment.content_type
        ));
    }

    if let Some(source_url) = task
        .metadata
        .get("document")
        .and_then(|value| value.get("source_url"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("Source URL: {}", source_url));
    }

    if let Some(figma_url) = task
        .metadata
        .get("document")
        .and_then(|value| value.get("figma_url"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("Figma URL: {}", figma_url));
    }

    if let Some(caption) = extra_caption.filter(|value| !value.trim().is_empty()) {
        lines.push(format!("Caption: {}", caption.trim()));
    }

    if let Some(description) = task
        .description
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(String::new());
        lines.push(format!("Task summary: {}", description.trim()));
    }

    if !notes.trim().is_empty() {
        lines.push(String::new());
        lines.push("Notes:".to_string());
        lines.push(notes.trim().to_string());
    }

    lines.join("\n")
}

fn stable_document_filename(task_id: Uuid, title: &str, extension: &str) -> String {
    let safe_title = sanitize_filename(title);
    let normalized_extension = extension.trim().trim_start_matches('.');
    if normalized_extension.is_empty() {
        format!("task-{}-{}", task_id, safe_title)
    } else {
        format!("task-{}-{}.{}", task_id, safe_title, normalized_extension)
    }
}

fn project_document_storage_key(project_id: Uuid, task_id: Uuid, filename: &str) -> String {
    format!(
        "project-documents/{}/task-docs/{}-{}",
        project_id,
        task_id,
        sanitize_filename(filename)
    )
}

fn extension_for_attachment<'a>(content_type: &str, filename: &'a str) -> &'a str {
    if let Some((_, ext)) = filename.rsplit_once('.') {
        return ext;
    }

    match content_type {
        "application/pdf" => "pdf",
        value if value.starts_with("image/png") => "png",
        value if value.starts_with("image/jpeg") => "jpg",
        value if value.starts_with("image/webp") => "webp",
        "text/markdown" => "md",
        "text/plain" => "txt",
        _ => "bin",
    }
}

fn sanitize_filename(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
            out.push(c);
        } else {
            out.push('_');
        }
    }

    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "document".to_string()
    } else {
        trimmed.to_string()
    }
}

fn task_type_label(task_type: TaskType) -> &'static str {
    match task_type {
        TaskType::Feature => "feature",
        TaskType::Bug => "bug",
        TaskType::Refactor => "refactor",
        TaskType::Docs => "docs",
        TaskType::Test => "test",
        TaskType::Init => "init",
        TaskType::Hotfix => "hotfix",
        TaskType::Chore => "chore",
        TaskType::Spike => "spike",
        TaskType::SmallTask => "small_task",
        TaskType::Deploy => "deploy",
    }
}
