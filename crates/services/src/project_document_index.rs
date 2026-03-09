use std::sync::Arc;

use acpms_db::{
    models::{ProjectDocument, ProjectDocumentChunk},
    PgPool,
};
use acpms_utils::{
    build_project_document_chunks, is_indexable_project_document_content_type,
    normalize_project_document_text,
};
use anyhow::{Context, Result};
use uuid::Uuid;

use crate::StorageService;

pub struct ProjectDocumentIndexService {
    pool: PgPool,
    storage: Arc<StorageService>,
}

impl ProjectDocumentIndexService {
    pub fn new(pool: PgPool, storage: Arc<StorageService>) -> Self {
        Self { pool, storage }
    }

    pub async fn index_document(&self, document: &ProjectDocument) -> Result<ProjectDocument> {
        if !self
            .mark_document_indexing(document.id, document.version)
            .await?
        {
            return self.load_document(document.id).await;
        }

        let indexing_result = async {
            if !is_indexable_project_document_content_type(&document.content_type) {
                anyhow::bail!(
                    "Unsupported content type for v1 indexing: {}",
                    document.content_type
                );
            }

            let bytes = self
                .storage
                .download_object_bytes(&document.storage_key)
                .await
                .with_context(|| {
                    format!(
                        "Failed to download project document content from storage key {}",
                        document.storage_key
                    )
                })?;

            let text = normalize_project_document_text(&document.content_type, &bytes)?;
            if text.trim().is_empty() {
                anyhow::bail!("Document is empty after normalization");
            }

            let chunks = build_project_document_chunks(&text);
            if chunks.is_empty() {
                anyhow::bail!("Document produced zero searchable chunks");
            }

            self.replace_chunks_atomically(document, &chunks).await
        }
        .await;

        match indexing_result {
            Ok(updated) => Ok(updated),
            Err(error) => {
                self.mark_document_failed(document.id, &error.to_string())
                    .await?;
                Err(error)
            }
        }
    }

    async fn replace_chunks_atomically(
        &self,
        document: &ProjectDocument,
        chunks: &[acpms_utils::ProjectDocumentChunkDraft],
    ) -> Result<ProjectDocument> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to start indexing transaction")?;

        let current = sqlx::query_as::<_, ProjectDocument>(
            r#"
            SELECT id, project_id, title, filename, document_kind, content_type, storage_key,
                   checksum, size_bytes, source, version, ingestion_status, index_error,
                   indexed_at, created_by, updated_by, created_at, updated_at
            FROM project_documents
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(document.id)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to lock current project document row")?;

        if current.version != document.version {
            tx.rollback()
                .await
                .context("Failed to rollback stale indexing transaction")?;
            return Ok(current);
        }

        sqlx::query("DELETE FROM project_document_chunks WHERE document_id = $1")
            .bind(document.id)
            .execute(&mut *tx)
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
            .execute(&mut *tx)
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
        .fetch_one(&mut *tx)
        .await
        .context("Failed to mark project document indexed")?;

        tx.commit()
            .await
            .context("Failed to commit project document indexing transaction")?;

        Ok(updated)
    }

    async fn mark_document_indexing(
        &self,
        document_id: Uuid,
        expected_version: i32,
    ) -> Result<bool> {
        let rows = sqlx::query(
            r#"
            UPDATE project_documents
            SET ingestion_status = 'indexing',
                index_error = NULL,
                updated_at = NOW()
            WHERE id = $1 AND version = $2
            "#,
        )
        .bind(document_id)
        .bind(expected_version)
        .execute(&self.pool)
        .await
        .context("Failed to mark project document as indexing")?
        .rows_affected();

        Ok(rows > 0)
    }

    async fn mark_document_failed(&self, document_id: Uuid, error: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE project_documents
            SET ingestion_status = 'failed',
                index_error = $2,
                indexed_at = NULL,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(document_id)
        .bind(truncate_index_error(error))
        .execute(&self.pool)
        .await
        .context("Failed to mark project document as failed")?;

        Ok(())
    }

    async fn load_document(&self, document_id: Uuid) -> Result<ProjectDocument> {
        sqlx::query_as::<_, ProjectDocument>(
            r#"
            SELECT id, project_id, title, filename, document_kind, content_type, storage_key,
                   checksum, size_bytes, source, version, ingestion_status, index_error,
                   indexed_at, created_by, updated_by, created_at, updated_at
            FROM project_documents
            WHERE id = $1
            "#,
        )
        .bind(document_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to reload project document")
    }

    #[allow(dead_code)]
    pub async fn list_chunks_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<ProjectDocumentChunk>> {
        sqlx::query_as::<_, ProjectDocumentChunk>(
            r#"
            SELECT id, document_id, project_id, chunk_index, content, content_hash,
                   token_count, embedding, created_at
            FROM project_document_chunks
            WHERE document_id = $1
            ORDER BY chunk_index ASC
            "#,
        )
        .bind(document_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list project document chunks")
    }
}

fn truncate_index_error(error: &str) -> String {
    const MAX_ERROR_CHARS: usize = 500;
    let mut truncated = error.chars().take(MAX_ERROR_CHARS).collect::<String>();
    if error.chars().count() > MAX_ERROR_CHARS {
        truncated.push_str("...");
    }
    truncated
}
