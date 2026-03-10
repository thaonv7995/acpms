use acpms_db::{models::ProjectDocument, PgPool};
use anyhow::{Context, Result};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UpsertProjectDocumentInput {
    pub title: String,
    pub filename: String,
    pub document_kind: String,
    pub content_type: String,
    pub storage_key: String,
    pub checksum: Option<String>,
    pub size_bytes: i64,
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateProjectDocumentInput {
    pub title: Option<String>,
    pub document_kind: Option<String>,
    pub content_type: Option<String>,
    pub storage_key: Option<String>,
    pub checksum: Option<Option<String>>,
    pub size_bytes: Option<i64>,
}

#[derive(Debug, Error)]
pub enum ProjectDocumentServiceError {
    #[error("Project document not found")]
    NotFound,
    #[error("A different filename already uses this title")]
    TitleConflict,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub struct ProjectDocumentService {
    pool: PgPool,
}

impl ProjectDocumentService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_project_documents(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<ProjectDocument>, ProjectDocumentServiceError> {
        let documents = sqlx::query_as::<_, ProjectDocument>(
            r#"
            SELECT id, project_id, title, filename, document_kind, content_type, storage_key,
                   checksum, size_bytes, source, version, ingestion_status, index_error,
                   indexed_at, created_by, updated_by, created_at, updated_at
            FROM project_documents
            WHERE project_id = $1
            ORDER BY updated_at DESC, created_at DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list project documents")?;

        Ok(documents)
    }

    pub async fn get_project_document(
        &self,
        project_id: Uuid,
        document_id: Uuid,
    ) -> Result<Option<ProjectDocument>, ProjectDocumentServiceError> {
        let document = sqlx::query_as::<_, ProjectDocument>(
            r#"
            SELECT id, project_id, title, filename, document_kind, content_type, storage_key,
                   checksum, size_bytes, source, version, ingestion_status, index_error,
                   indexed_at, created_by, updated_by, created_at, updated_at
            FROM project_documents
            WHERE project_id = $1 AND id = $2
            "#,
        )
        .bind(project_id)
        .bind(document_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch project document")?;

        Ok(document)
    }

    pub async fn create_or_upsert_project_document(
        &self,
        project_id: Uuid,
        actor_id: Uuid,
        input: UpsertProjectDocumentInput,
    ) -> Result<ProjectDocument, ProjectDocumentServiceError> {
        self.ensure_title_available(project_id, &input.title, Some(&input.filename), None)
            .await?;

        let existing = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id
            FROM project_documents
            WHERE project_id = $1 AND filename = $2
            "#,
        )
        .bind(project_id)
        .bind(&input.filename)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to look up project document by filename")?;

        let document = if let Some(document_id) = existing {
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
            .bind(project_id)
            .bind(document_id)
            .bind(&input.title)
            .bind(&input.document_kind)
            .bind(&input.content_type)
            .bind(&input.storage_key)
            .bind(&input.checksum)
            .bind(input.size_bytes)
            .bind(&input.source)
            .bind(actor_id)
            .fetch_one(&self.pool)
            .await
            .context("Failed to upsert existing project document")?
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
            .bind(project_id)
            .bind(&input.title)
            .bind(&input.filename)
            .bind(&input.document_kind)
            .bind(&input.content_type)
            .bind(&input.storage_key)
            .bind(&input.checksum)
            .bind(input.size_bytes)
            .bind(&input.source)
            .bind(actor_id)
            .fetch_one(&self.pool)
            .await
            .context("Failed to create project document")?
        };

        Ok(document)
    }

    pub async fn update_project_document(
        &self,
        project_id: Uuid,
        document_id: Uuid,
        actor_id: Uuid,
        input: UpdateProjectDocumentInput,
    ) -> Result<ProjectDocument, ProjectDocumentServiceError> {
        let existing = self
            .get_project_document(project_id, document_id)
            .await?
            .ok_or(ProjectDocumentServiceError::NotFound)?;

        let next_title = input
            .title
            .clone()
            .unwrap_or_else(|| existing.title.clone());
        self.ensure_title_available(
            project_id,
            &next_title,
            Some(&existing.filename),
            Some(document_id),
        )
        .await?;

        let next_document_kind = input
            .document_kind
            .clone()
            .unwrap_or_else(|| existing.document_kind.clone());
        let next_content_type = input
            .content_type
            .clone()
            .unwrap_or_else(|| existing.content_type.clone());
        let next_storage_key = input
            .storage_key
            .clone()
            .unwrap_or_else(|| existing.storage_key.clone());
        let next_checksum = input.checksum.clone().unwrap_or(existing.checksum.clone());
        let next_size_bytes = input.size_bytes.unwrap_or(existing.size_bytes);

        let content_changed = next_content_type != existing.content_type
            || next_storage_key != existing.storage_key
            || next_checksum != existing.checksum
            || next_size_bytes != existing.size_bytes;

        let version = if content_changed {
            existing.version + 1
        } else {
            existing.version
        };
        let ingestion_status = if content_changed {
            "pending".to_string()
        } else {
            existing.ingestion_status.clone()
        };
        let index_error = if content_changed {
            None
        } else {
            existing.index_error.clone()
        };
        let indexed_at = if content_changed {
            None
        } else {
            existing.indexed_at
        };

        let document = sqlx::query_as::<_, ProjectDocument>(
            r#"
            UPDATE project_documents
            SET title = $3,
                document_kind = $4,
                content_type = $5,
                storage_key = $6,
                checksum = $7,
                size_bytes = $8,
                version = $9,
                ingestion_status = $10,
                index_error = $11,
                indexed_at = $12,
                updated_by = $13,
                updated_at = NOW()
            WHERE project_id = $1 AND id = $2
            RETURNING id, project_id, title, filename, document_kind, content_type, storage_key,
                      checksum, size_bytes, source, version, ingestion_status, index_error,
                      indexed_at, created_by, updated_by, created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind(document_id)
        .bind(&next_title)
        .bind(&next_document_kind)
        .bind(&next_content_type)
        .bind(&next_storage_key)
        .bind(&next_checksum)
        .bind(next_size_bytes)
        .bind(version)
        .bind(&ingestion_status)
        .bind(&index_error)
        .bind(indexed_at)
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to update project document")?;

        Ok(document)
    }

    pub async fn delete_project_document(
        &self,
        project_id: Uuid,
        document_id: Uuid,
    ) -> Result<Option<ProjectDocument>, ProjectDocumentServiceError> {
        let deleted = sqlx::query_as::<_, ProjectDocument>(
            r#"
            DELETE FROM project_documents
            WHERE project_id = $1 AND id = $2
            RETURNING id, project_id, title, filename, document_kind, content_type, storage_key,
                      checksum, size_bytes, source, version, ingestion_status, index_error,
                      indexed_at, created_by, updated_by, created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind(document_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to delete project document")?;

        Ok(deleted)
    }

    async fn ensure_title_available(
        &self,
        project_id: Uuid,
        title: &str,
        filename: Option<&str>,
        exclude_document_id: Option<Uuid>,
    ) -> Result<(), ProjectDocumentServiceError> {
        let conflict = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT id, filename
            FROM project_documents
            WHERE project_id = $1
              AND title = $2
              AND ($3::uuid IS NULL OR id <> $3)
            LIMIT 1
            "#,
        )
        .bind(project_id)
        .bind(title)
        .bind(exclude_document_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to validate project document title")?;

        if let Some((_, existing_filename)) = conflict {
            if filename
                .map(|candidate| candidate != existing_filename)
                .unwrap_or(true)
            {
                return Err(ProjectDocumentServiceError::TitleConflict);
            }
        }

        Ok(())
    }
}
