//! Project Template Service
//!
//! Manages project templates for quick scaffolding with predefined tech stacks and settings.

use acpms_db::models::*;
use acpms_db::PgPool;
use anyhow::{Context, Result};
use uuid::Uuid;

/// Service for managing project templates
pub struct ProjectTemplateService {
    pool: PgPool,
}

impl ProjectTemplateService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// List all templates with optional filtering
    pub async fn list_templates(&self, query: ListTemplatesQuery) -> Result<Vec<ProjectTemplate>> {
        let templates = match (query.project_type, query.official_only) {
            (Some(project_type), Some(true)) => sqlx::query_as::<_, ProjectTemplate>(
                r#"
                    SELECT id, name, description, project_type, repository_url,
                           tech_stack, default_settings, is_official, created_by,
                           created_at, updated_at
                    FROM project_templates
                    WHERE project_type = $1 AND is_official = true
                    ORDER BY name ASC
                    "#,
            )
            .bind(project_type)
            .fetch_all(&self.pool)
            .await
            .context("Failed to fetch templates")?,
            (Some(project_type), _) => sqlx::query_as::<_, ProjectTemplate>(
                r#"
                    SELECT id, name, description, project_type, repository_url,
                           tech_stack, default_settings, is_official, created_by,
                           created_at, updated_at
                    FROM project_templates
                    WHERE project_type = $1
                    ORDER BY is_official DESC, name ASC
                    "#,
            )
            .bind(project_type)
            .fetch_all(&self.pool)
            .await
            .context("Failed to fetch templates")?,
            (None, Some(true)) => sqlx::query_as::<_, ProjectTemplate>(
                r#"
                    SELECT id, name, description, project_type, repository_url,
                           tech_stack, default_settings, is_official, created_by,
                           created_at, updated_at
                    FROM project_templates
                    WHERE is_official = true
                    ORDER BY project_type, name ASC
                    "#,
            )
            .fetch_all(&self.pool)
            .await
            .context("Failed to fetch templates")?,
            (None, _) => sqlx::query_as::<_, ProjectTemplate>(
                r#"
                    SELECT id, name, description, project_type, repository_url,
                           tech_stack, default_settings, is_official, created_by,
                           created_at, updated_at
                    FROM project_templates
                    ORDER BY is_official DESC, project_type, name ASC
                    "#,
            )
            .fetch_all(&self.pool)
            .await
            .context("Failed to fetch templates")?,
        };

        Ok(templates)
    }

    /// Get a template by ID
    pub async fn get_template(&self, id: Uuid) -> Result<Option<ProjectTemplate>> {
        let template = sqlx::query_as::<_, ProjectTemplate>(
            r#"
            SELECT id, name, description, project_type, repository_url,
                   tech_stack, default_settings, is_official, created_by,
                   created_at, updated_at
            FROM project_templates
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch template")?;

        Ok(template)
    }

    /// Create a new template
    pub async fn create_template(
        &self,
        user_id: Uuid,
        req: CreateProjectTemplateRequest,
    ) -> Result<ProjectTemplate> {
        let tech_stack = req
            .tech_stack
            .map(|ts| serde_json::json!(ts))
            .unwrap_or_else(|| serde_json::json!([]));

        let default_settings = req
            .default_settings
            .unwrap_or_else(|| serde_json::json!({}));

        let is_official = req.is_official.unwrap_or(false);

        let template = sqlx::query_as::<_, ProjectTemplate>(
            r#"
            INSERT INTO project_templates (
                name, description, project_type, repository_url,
                tech_stack, default_settings, is_official, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, name, description, project_type, repository_url,
                      tech_stack, default_settings, is_official, created_by,
                      created_at, updated_at
            "#,
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(req.project_type)
        .bind(&req.repository_url)
        .bind(&tech_stack)
        .bind(&default_settings)
        .bind(is_official)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create template")?;

        Ok(template)
    }

    /// Update an existing template
    pub async fn update_template(
        &self,
        id: Uuid,
        req: UpdateProjectTemplateRequest,
    ) -> Result<ProjectTemplate> {
        let tech_stack = req.tech_stack.map(|ts| serde_json::json!(ts));

        let template = sqlx::query_as::<_, ProjectTemplate>(
            r#"
            UPDATE project_templates
            SET name = COALESCE($2, name),
                description = COALESCE($3, description),
                project_type = COALESCE($4, project_type),
                repository_url = COALESCE($5, repository_url),
                tech_stack = COALESCE($6, tech_stack),
                default_settings = COALESCE($7, default_settings),
                is_official = COALESCE($8, is_official),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, description, project_type, repository_url,
                      tech_stack, default_settings, is_official, created_by,
                      created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&req.name)
        .bind(&req.description)
        .bind(req.project_type)
        .bind(&req.repository_url)
        .bind(&tech_stack)
        .bind(&req.default_settings)
        .bind(req.is_official)
        .fetch_one(&self.pool)
        .await
        .context("Failed to update template")?;

        Ok(template)
    }

    /// Delete a template
    pub async fn delete_template(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM project_templates WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to delete template")?;

        Ok(())
    }

    /// Get default settings for a template, merged with project type defaults
    pub fn get_merged_settings(&self, template: &ProjectTemplate) -> ProjectSettings {
        let mut settings = ProjectSettings {
            preview_enabled: template.project_type.default_preview_enabled(),
            ..ProjectSettings::default()
        };

        // Override with template-specific settings
        if let Ok(template_settings) =
            serde_json::from_value::<ProjectSettings>(template.default_settings.clone())
        {
            settings = template_settings;
        } else {
            // Partial merge if full deserialization fails
            settings.merge(&template.default_settings);
        }

        settings
    }
}
