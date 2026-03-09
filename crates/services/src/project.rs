use acpms_db::{models::*, PgPool};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{load_project_summaries, ProjectComputedSummary};

fn parse_compact_stack_tokens(raw: &str) -> Vec<String> {
    raw.split(['|', ','])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            segment
                .rsplit_once(':')
                .map(|(_, stack)| stack)
                .unwrap_or(segment)
                .trim()
                .to_string()
        })
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn dedupe_stack_values(values: Vec<String>) -> Vec<String> {
    let mut deduped: Vec<String> = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        if deduped
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(trimmed))
        {
            continue;
        }
        deduped.push(trimmed.to_string());
    }
    deduped
}

fn stack_layer_key(layer: &ProjectStackLayer) -> &'static str {
    match layer {
        ProjectStackLayer::Frontend => "frontend",
        ProjectStackLayer::Backend => "backend",
        ProjectStackLayer::Database => "database",
        ProjectStackLayer::Auth => "auth",
        ProjectStackLayer::Cache => "cache",
        ProjectStackLayer::Queue => "queue",
    }
}

fn serialize_stack_selections(selections: &[ProjectStackSelection]) -> serde_json::Value {
    serde_json::Value::Array(
        selections
            .iter()
            .map(|selection| {
                serde_json::json!({
                    "layer": stack_layer_key(&selection.layer),
                    "stack": selection.stack,
                })
            })
            .collect(),
    )
}

fn persist_stack_metadata(
    metadata: &mut serde_json::Map<String, serde_json::Value>,
    tech_stack: Option<&str>,
    stack_selections: Option<&[ProjectStackSelection]>,
) {
    let normalized_tech_stack = tech_stack.map(str::trim).filter(|value| !value.is_empty());
    if let Some(value) = normalized_tech_stack {
        metadata.insert(
            "tech_stack".to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }

    if let Some(selections) = stack_selections.filter(|items| !items.is_empty()) {
        let serialized = serialize_stack_selections(selections);
        metadata.insert("stack_selections".to_string(), serialized.clone());
        metadata.insert("stackSelections".to_string(), serialized);
    }

    let from_layered = stack_selections
        .unwrap_or_default()
        .iter()
        .map(|selection| selection.stack.trim().to_string())
        .collect::<Vec<_>>();
    let from_compact = normalized_tech_stack
        .map(parse_compact_stack_tokens)
        .unwrap_or_default();
    let display_stack = if from_layered.is_empty() {
        from_compact
    } else {
        from_layered
    };

    let deduped = dedupe_stack_values(display_stack);
    if !deduped.is_empty() {
        metadata.insert(
            "techStack".to_string(),
            serde_json::Value::Array(
                deduped
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
    }
}

pub struct ProjectService {
    pool: PgPool,
}

impl ProjectService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_project(
        &self,
        user_id: Uuid,
        req: CreateProjectRequest,
    ) -> Result<Project> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin project creation transaction")?;

        let project = Self::create_project_in_tx(&mut tx, user_id, req).await?;

        tx.commit()
            .await
            .context("Failed to commit project creation transaction")?;

        Ok(project)
    }

    pub async fn create_project_in_tx(
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
        req: CreateProjectRequest,
    ) -> Result<Project> {
        let mut metadata = req.metadata.unwrap_or_else(|| serde_json::json!({}));
        if !metadata.is_object() {
            metadata = serde_json::json!({});
        }

        // Ensure a stable URL-safe slug is always present for worktree naming and GitLab paths.
        let needs_slug = metadata
            .get("slug")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        if needs_slug {
            let slug = slugify_project_name(&req.name);
            if let Some(obj) = metadata.as_object_mut() {
                obj.insert("slug".to_string(), serde_json::Value::String(slug));
            }
        }

        if let Some(metadata_obj) = metadata.as_object_mut() {
            metadata_obj
                .entry("repo_path_version".to_string())
                .or_insert_with(|| serde_json::Value::from(3));
            persist_stack_metadata(
                metadata_obj,
                req.tech_stack.as_deref(),
                req.stack_selections.as_deref(),
            );
        }

        let require_review = req.require_review.unwrap_or(true);
        let project_type = req.project_type.unwrap_or_default();
        let preview_enabled = req
            .preview_enabled
            .unwrap_or_else(|| project_type.default_preview_enabled());
        let repository_url = req.repository_url.clone();
        let repository_context = req.repository_context.unwrap_or_else(|| RepositoryContext {
            upstream_repository_url: repository_url.clone(),
            effective_clone_url: repository_url.clone(),
            ..RepositoryContext::default()
        });

        // Create initial settings with require_review synced and type defaults
        let settings = ProjectSettings {
            require_review,
            preview_enabled,
            ..ProjectSettings::default()
        };
        let settings_json =
            serde_json::to_value(&settings).context("Failed to serialize project settings")?;

        let project = sqlx::query_as::<_, Project>(
            r#"
            INSERT INTO projects (name, description, repository_url, repository_context, created_by, metadata, require_review, settings, project_type)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
            "#
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(&repository_url)
        .bind(serde_json::to_value(&repository_context).context("Failed to serialize repository context")?)
        .bind(user_id)
        .bind(&metadata)
        .bind(require_review)
        .bind(settings_json)
        .bind(project_type)
        .fetch_one(&mut **tx)
        .await
        .context("Failed to create project")?;

        // Auto-assign creator as OWNER
        sqlx::query(
            r#"
            INSERT INTO project_members (project_id, user_id, roles)
            VALUES ($1, $2, ARRAY['owner']::project_role[])
            "#,
        )
        .bind(project.id)
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .context("Failed to assign owner role")?;

        // Create Default "Sprint 0" (Backlog)
        sqlx::query(
            r#"
            INSERT INTO sprints (project_id, sequence, name, description, goal, status, start_date, end_date)
            VALUES ($1, 1, 'Sprint 0 (Backlog)', 'Backlog for tasks and requirements', NULL, 'active', NOW(), NULL::timestamptz)
            "#
        )
        .bind(project.id)
        .execute(&mut **tx)
        .await
        .context("Failed to create Sprint 0")?;

        Ok(project)
    }

    pub async fn get_user_projects(&self, user_id: Uuid) -> Result<Vec<Project>> {
        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT p.*
            FROM projects p
            INNER JOIN project_members pm ON p.id = pm.project_id
            WHERE pm.user_id = $1
            ORDER BY p.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch user projects")?;

        Ok(projects)
    }

    pub async fn get_all_projects(&self) -> Result<Vec<Project>> {
        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT p.*
            FROM projects p
            ORDER BY p.created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch all projects")?;

        Ok(projects)
    }

    /// Paginated version of get_user_projects with cursor-based pagination.
    pub async fn get_user_projects_paginated(
        &self,
        user_id: Uuid,
        limit: i64,
        before: Option<DateTime<Utc>>,
        before_id: Option<Uuid>,
        search: Option<&str>,
    ) -> Result<Vec<Project>> {
        let bounded_limit = limit.clamp(1, 100);

        let projects = if let Some(before_ts) = before {
            if let Some(bid) = before_id {
                sqlx::query_as::<_, Project>(
                    r#"
                    SELECT p.*
                    FROM projects p
                    INNER JOIN project_members pm ON p.id = pm.project_id
                    WHERE pm.user_id = $1
                      AND (p.created_at < $2 OR (p.created_at = $2 AND p.id < $3))
                      AND ($4::text IS NULL OR p.name ILIKE '%' || $4 || '%')
                    ORDER BY p.created_at DESC, p.id DESC
                    LIMIT $5
                    "#,
                )
                .bind(user_id)
                .bind(before_ts)
                .bind(bid)
                .bind(search)
                .bind(bounded_limit)
                .fetch_all(&self.pool)
                .await
                .context("Failed to fetch paginated user projects")?
            } else {
                sqlx::query_as::<_, Project>(
                    r#"
                    SELECT p.*
                    FROM projects p
                    INNER JOIN project_members pm ON p.id = pm.project_id
                    WHERE pm.user_id = $1
                      AND p.created_at < $2
                      AND ($3::text IS NULL OR p.name ILIKE '%' || $3 || '%')
                    ORDER BY p.created_at DESC, p.id DESC
                    LIMIT $4
                    "#,
                )
                .bind(user_id)
                .bind(before_ts)
                .bind(search)
                .bind(bounded_limit)
                .fetch_all(&self.pool)
                .await
                .context("Failed to fetch paginated user projects")?
            }
        } else {
            sqlx::query_as::<_, Project>(
                r#"
                SELECT p.*
                FROM projects p
                INNER JOIN project_members pm ON p.id = pm.project_id
                WHERE pm.user_id = $1
                  AND ($2::text IS NULL OR p.name ILIKE '%' || $2 || '%')
                ORDER BY p.created_at DESC, p.id DESC
                LIMIT $3
                "#,
            )
            .bind(user_id)
            .bind(search)
            .bind(bounded_limit)
            .fetch_all(&self.pool)
            .await
            .context("Failed to fetch paginated user projects")?
        };

        Ok(projects)
    }

    /// Paginated version of get_all_projects (admin) with cursor-based pagination.
    pub async fn get_all_projects_paginated(
        &self,
        limit: i64,
        before: Option<DateTime<Utc>>,
        before_id: Option<Uuid>,
        search: Option<&str>,
    ) -> Result<Vec<Project>> {
        let bounded_limit = limit.clamp(1, 100);

        let projects = if let Some(before_ts) = before {
            if let Some(bid) = before_id {
                sqlx::query_as::<_, Project>(
                    r#"
                    SELECT p.*
                    FROM projects p
                    WHERE (p.created_at < $1 OR (p.created_at = $1 AND p.id < $2))
                      AND ($3::text IS NULL OR p.name ILIKE '%' || $3 || '%')
                    ORDER BY p.created_at DESC, p.id DESC
                    LIMIT $4
                    "#,
                )
                .bind(before_ts)
                .bind(bid)
                .bind(search)
                .bind(bounded_limit)
                .fetch_all(&self.pool)
                .await
                .context("Failed to fetch paginated projects")?
            } else {
                sqlx::query_as::<_, Project>(
                    r#"
                    SELECT p.*
                    FROM projects p
                    WHERE p.created_at < $1
                      AND ($2::text IS NULL OR p.name ILIKE '%' || $2 || '%')
                    ORDER BY p.created_at DESC, p.id DESC
                    LIMIT $3
                    "#,
                )
                .bind(before_ts)
                .bind(search)
                .bind(bounded_limit)
                .fetch_all(&self.pool)
                .await
                .context("Failed to fetch paginated projects")?
            }
        } else {
            sqlx::query_as::<_, Project>(
                r#"
                SELECT p.*
                FROM projects p
                WHERE ($1::text IS NULL OR p.name ILIKE '%' || $1 || '%')
                ORDER BY p.created_at DESC, p.id DESC
                LIMIT $2
                "#,
            )
            .bind(search)
            .bind(bounded_limit)
            .fetch_all(&self.pool)
            .await
            .context("Failed to fetch paginated projects")?
        };

        Ok(projects)
    }

    /// Offset-based paginated version of get_all_projects (admin).
    pub async fn get_all_projects_page(
        &self,
        limit: i64,
        offset: i64,
        search: Option<&str>,
    ) -> Result<Vec<Project>> {
        let bounded_limit = limit.clamp(1, 100);
        let valid_offset = offset.max(0);

        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT p.*
            FROM projects p
            WHERE ($1::text IS NULL OR p.name ILIKE '%' || $1 || '%')
            ORDER BY p.created_at DESC, p.id DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(search)
        .bind(bounded_limit)
        .bind(valid_offset)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch paginated projects (offset)")?;

        Ok(projects)
    }

    /// Offset-based paginated version of get_user_projects.
    pub async fn get_user_projects_page(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        search: Option<&str>,
    ) -> Result<Vec<Project>> {
        let bounded_limit = limit.clamp(1, 100);
        let valid_offset = offset.max(0);

        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT p.*
            FROM projects p
            INNER JOIN project_members pm ON p.id = pm.project_id
            WHERE pm.user_id = $1
              AND ($2::text IS NULL OR p.name ILIKE '%' || $2 || '%')
            ORDER BY p.created_at DESC, p.id DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(user_id)
        .bind(search)
        .bind(bounded_limit)
        .bind(valid_offset)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch paginated user projects (offset)")?;

        Ok(projects)
    }

    pub async fn count_all_projects(&self, search: Option<&str>) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM projects p
            WHERE ($1::text IS NULL OR p.name ILIKE '%' || $1 || '%')
            "#,
        )
        .bind(search)
        .fetch_one(&self.pool)
        .await
        .context("Failed to count all projects")?;

        Ok(count)
    }

    pub async fn count_user_projects(&self, user_id: Uuid, search: Option<&str>) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM projects p
            INNER JOIN project_members pm ON p.id = pm.project_id
            WHERE pm.user_id = $1
              AND ($2::text IS NULL OR p.name ILIKE '%' || $2 || '%')
            "#,
        )
        .bind(user_id)
        .bind(search)
        .fetch_one(&self.pool)
        .await
        .context("Failed to count user projects")?;

        Ok(count)
    }

    pub async fn get_project(&self, project_id: Uuid) -> Result<Option<Project>> {
        let project = sqlx::query_as::<_, Project>(
            r#"
            SELECT *
            FROM projects
            WHERE id = $1
            "#,
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch project")?;

        Ok(project)
    }

    pub async fn load_project_summaries(
        &self,
        projects: &[Project],
    ) -> Result<HashMap<Uuid, ProjectComputedSummary>> {
        load_project_summaries(&self.pool, projects).await
    }

    pub async fn load_project_summary(&self, project: &Project) -> Result<ProjectComputedSummary> {
        Ok(self
            .load_project_summaries(std::slice::from_ref(project))
            .await?
            .remove(&project.id)
            .unwrap_or_default())
    }

    pub async fn update_project(
        &self,
        project_id: Uuid,
        req: UpdateProjectRequest,
    ) -> Result<Project> {
        // If require_review is being updated, also sync it to settings
        let project = if let Some(require_review) = req.require_review {
            sqlx::query_as::<_, Project>(
                r#"
                UPDATE projects
                SET name = COALESCE($2, name),
                    description = COALESCE($3, description),
                    repository_url = COALESCE($4, repository_url),
                    metadata = COALESCE($5, metadata),
                    repository_context = COALESCE($6, repository_context),
                    require_review = $7,
                    settings = settings || jsonb_build_object('require_review', $7),
                    updated_at = NOW()
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(project_id)
            .bind(&req.name)
            .bind(&req.description)
            .bind(&req.repository_url)
            .bind(&req.metadata)
            .bind(
                req.repository_context
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()
                    .context("Failed to serialize repository context")?,
            )
            .bind(require_review)
            .fetch_one(&self.pool)
            .await
            .context("Failed to update project")?
        } else {
            sqlx::query_as::<_, Project>(
                r#"
                UPDATE projects
                SET name = COALESCE($2, name),
                    description = COALESCE($3, description),
                    repository_url = COALESCE($4, repository_url),
                    metadata = COALESCE($5, metadata),
                    repository_context = COALESCE($6, repository_context),
                    updated_at = NOW()
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(project_id)
            .bind(&req.name)
            .bind(&req.description)
            .bind(&req.repository_url)
            .bind(&req.metadata)
            .bind(
                req.repository_context
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()
                    .context("Failed to serialize repository context")?,
            )
            .fetch_one(&self.pool)
            .await
            .context("Failed to update project")?
        };

        Ok(project)
    }

    pub async fn delete_project(&self, project_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM projects
            WHERE id = $1
            "#,
        )
        .bind(project_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete project")?;

        Ok(())
    }

    pub async fn check_user_role(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        required_role: ProjectRole,
    ) -> Result<bool> {
        let has_role: bool = sqlx::query_scalar(
            r#"
            SELECT user_has_role($1, $2, $3)
            "#,
        )
        .bind(user_id)
        .bind(project_id)
        .bind(required_role as ProjectRole)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check user role")?;

        Ok(has_role)
    }

    // ===== Project Settings Methods =====

    /// Get project settings by project ID
    pub async fn get_settings(&self, project_id: Uuid) -> Result<ProjectSettings> {
        let settings: serde_json::Value =
            sqlx::query_scalar(r#"SELECT settings FROM projects WHERE id = $1"#)
                .bind(project_id)
                .fetch_one(&self.pool)
                .await
                .context("Failed to fetch project settings")?;

        let project_settings: ProjectSettings =
            serde_json::from_value(settings).unwrap_or_default();

        Ok(project_settings)
    }

    /// Update project settings (full replacement)
    pub async fn update_settings(
        &self,
        project_id: Uuid,
        settings: ProjectSettings,
    ) -> Result<ProjectSettings> {
        let settings_json =
            serde_json::to_value(&settings).context("Failed to serialize settings")?;

        // Also sync require_review to the legacy column for backward compatibility
        let updated_settings: serde_json::Value = sqlx::query_scalar(
            r#"
            UPDATE projects
            SET settings = $2,
                require_review = ($2->>'require_review')::boolean,
                updated_at = NOW()
            WHERE id = $1
            RETURNING settings
            "#,
        )
        .bind(project_id)
        .bind(&settings_json)
        .fetch_one(&self.pool)
        .await
        .context("Failed to update project settings")?;

        let result: ProjectSettings = serde_json::from_value(updated_settings).unwrap_or_default();

        Ok(result)
    }

    /// Update a single setting by key (partial update)
    pub async fn update_single_setting(
        &self,
        project_id: Uuid,
        key: &str,
        value: serde_json::Value,
    ) -> Result<ProjectSettings> {
        // Build the JSONB update
        let update_obj = serde_json::json!({ key: value });

        // Special handling for require_review to sync with legacy column
        let updated_settings: serde_json::Value = if key == "require_review" {
            let require_review = value.as_bool().unwrap_or(true);
            sqlx::query_scalar(
                r#"
                UPDATE projects
                SET settings = settings || $2,
                    require_review = $3,
                    updated_at = NOW()
                WHERE id = $1
                RETURNING settings
                "#,
            )
            .bind(project_id)
            .bind(&update_obj)
            .bind(require_review)
            .fetch_one(&self.pool)
            .await
            .context("Failed to update project setting")?
        } else {
            sqlx::query_scalar(
                r#"
                UPDATE projects
                SET settings = settings || $2,
                    updated_at = NOW()
                WHERE id = $1
                RETURNING settings
                "#,
            )
            .bind(project_id)
            .bind(&update_obj)
            .fetch_one(&self.pool)
            .await
            .context("Failed to update project setting")?
        };

        let result: ProjectSettings = serde_json::from_value(updated_settings).unwrap_or_default();

        Ok(result)
    }

    // ===== Settings Helper Methods =====

    /// Check if a project requires human review for agent changes
    pub async fn should_require_review(&self, project_id: Uuid) -> Result<bool> {
        let settings = self.get_settings(project_id).await?;
        Ok(settings.require_review)
    }

    /// Check if a project should auto-deploy approved changes
    pub async fn should_auto_deploy(&self, project_id: Uuid) -> Result<bool> {
        let settings = self.get_settings(project_id).await?;
        Ok(settings.auto_deploy)
    }

    /// Check if a project should create preview environments
    pub async fn should_create_preview(&self, project_id: Uuid) -> Result<bool> {
        let settings = self.get_settings(project_id).await?;
        Ok(settings.preview_enabled)
    }

    /// Check if a project uses GitOps workflow (MRs)
    pub async fn should_use_gitops(&self, project_id: Uuid) -> Result<bool> {
        let settings = self.get_settings(project_id).await?;
        Ok(settings.gitops_enabled)
    }

    /// Check if a project should auto-execute tasks
    pub async fn should_auto_execute(
        &self,
        project_id: Uuid,
        task_type: Option<&str>,
    ) -> Result<bool> {
        let settings = self.get_settings(project_id).await?;

        if !settings.auto_execute {
            return Ok(false);
        }

        // If auto_execute_types is empty, auto-execute all tasks
        if settings.auto_execute_types.is_empty() {
            return Ok(true);
        }

        // Otherwise check if this task type is in the list
        match task_type {
            Some(t) => Ok(settings.auto_execute_types.iter().any(|tt| tt == t)),
            None => Ok(false),
        }
    }

    /// Check if a project should auto-retry failed tasks
    pub async fn should_auto_retry(&self, project_id: Uuid) -> Result<bool> {
        let settings = self.get_settings(project_id).await?;
        Ok(settings.auto_retry)
    }

    /// Get the max retry attempts for a project
    pub async fn get_max_retries(&self, project_id: Uuid) -> Result<i32> {
        let settings = self.get_settings(project_id).await?;
        Ok(settings.max_retries)
    }

    /// Get the execution timeout in minutes for a project
    pub async fn get_timeout_mins(&self, project_id: Uuid) -> Result<i32> {
        let settings = self.get_settings(project_id).await?;
        Ok(settings.timeout_mins)
    }
}
