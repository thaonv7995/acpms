use acpms_db::{models::*, PgPool};
use anyhow::{Context, Result};
use uuid::Uuid;

pub struct RequirementService {
    pool: PgPool,
}

impl RequirementService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_requirement(
        &self,
        created_by: Uuid,
        req: CreateRequirementRequest,
    ) -> Result<Requirement> {
        let metadata = req.metadata.unwrap_or_else(|| serde_json::json!({}));
        let priority = req.priority.unwrap_or(RequirementPriority::Medium);

        let requirement = sqlx::query_as::<_, Requirement>(
            r#"
            INSERT INTO requirements (project_id, title, content, priority, due_date, metadata, created_by, sprint_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, project_id, title, content, status, priority, due_date, metadata, created_by, created_at, updated_at, sprint_id
            "#
        )
        .bind(req.project_id)
        .bind(&req.title)
        .bind(&req.content)
        .bind(priority)
        .bind(req.due_date)
        .bind(metadata)
        .bind(created_by)
        .bind(req.sprint_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create requirement")?;

        Ok(requirement)
    }

    pub async fn get_project_requirements(&self, project_id: Uuid) -> Result<Vec<Requirement>> {
        let requirements = sqlx::query_as::<_, Requirement>(
            r#"
            SELECT id, project_id, title, content, status, priority, due_date, metadata, created_by, created_at, updated_at, sprint_id
            FROM requirements
            WHERE project_id = $1
            ORDER BY COALESCE(due_date, '9999-12-31'::date) ASC, created_at DESC
            "#
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch requirements")?;

        Ok(requirements)
    }

    pub async fn get_requirement(&self, requirement_id: Uuid) -> Result<Requirement> {
        let requirement = sqlx::query_as::<_, Requirement>(
            r#"
            SELECT id, project_id, title, content, status, priority, due_date, metadata, created_by, created_at, updated_at, sprint_id
            FROM requirements
            WHERE id = $1
            "#
        )
        .bind(requirement_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to fetch requirement")?;

        Ok(requirement)
    }

    pub async fn update_requirement(
        &self,
        requirement_id: Uuid,
        req: UpdateRequirementRequest,
    ) -> Result<Requirement> {
        let requirement = sqlx::query_as::<_, Requirement>(
            r#"
            UPDATE requirements
            SET title = COALESCE($2, title),
                content = COALESCE($3, content),
                status = COALESCE($4, status),
                priority = COALESCE($5, priority),
                metadata = COALESCE($6, metadata),
                sprint_id = COALESCE($7, sprint_id),
                due_date = COALESCE($8, due_date),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, title, content, status, priority, due_date, metadata, created_by, created_at, updated_at, sprint_id
            "#
        )
        .bind(requirement_id)
        .bind(req.title.as_ref())
        .bind(req.content.as_ref())
        .bind(req.status)
        .bind(req.priority)
        .bind(req.metadata.as_ref())
        .bind(req.sprint_id)
        .bind(req.due_date)
        .fetch_one(&self.pool)
        .await
        .context("Failed to update requirement")?;

        Ok(requirement)
    }

    pub async fn delete_requirement(&self, requirement_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM requirements WHERE id = $1")
            .bind(requirement_id)
            .execute(&self.pool)
            .await
            .context("Failed to delete requirement")?;

        Ok(())
    }
}
