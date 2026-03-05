use acpms_db::{models::*, repositories, PgPool};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

// Re-export metadata types for convenience
pub use acpms_db::models::{InitSource, InitTaskMetadata};

/// Task with latest attempt ID for efficient kanban queries
#[derive(Debug, FromRow)]
pub struct TaskWithLatestAttempt {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub assigned_to: Option<Uuid>,
    pub parent_task_id: Option<Uuid>,
    pub requirement_id: Option<Uuid>,
    pub sprint_id: Option<Uuid>,
    pub gitlab_issue_id: Option<i32>,
    pub metadata: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub latest_attempt_id: Option<Uuid>,
}

pub struct TaskService {
    pool: PgPool,
}

impl TaskService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn ensure_project_member(&self, project_id: Uuid, user_id: Uuid) -> Result<()> {
        let is_member: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM project_members
                WHERE project_id = $1 AND user_id = $2
            )
            "#,
        )
        .bind(project_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to validate project member")?;

        if !is_member {
            anyhow::bail!("Selected assignee is not a member of this project");
        }

        Ok(())
    }

    async fn ensure_parent_task_in_project(
        &self,
        project_id: Uuid,
        parent_task_id: Uuid,
    ) -> Result<()> {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM tasks
                WHERE id = $1 AND project_id = $2
            )
            "#,
        )
        .bind(parent_task_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to validate parent task")?;

        if !exists {
            anyhow::bail!("Parent task must belong to the same project");
        }

        Ok(())
    }

    async fn ensure_requirement_in_project(
        &self,
        project_id: Uuid,
        requirement_id: Uuid,
    ) -> Result<()> {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM requirements
                WHERE id = $1 AND project_id = $2
            )
            "#,
        )
        .bind(requirement_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to validate requirement")?;

        if !exists {
            anyhow::bail!("Requirement must belong to the same project");
        }

        Ok(())
    }

    async fn ensure_sprint_in_project(&self, project_id: Uuid, sprint_id: Uuid) -> Result<()> {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM sprints
                WHERE id = $1 AND project_id = $2
            )
            "#,
        )
        .bind(sprint_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to validate sprint")?;

        if !exists {
            anyhow::bail!("Sprint must belong to the same project");
        }

        Ok(())
    }

    pub async fn create_task(&self, user_id: Uuid, req: CreateTaskRequest) -> Result<Task> {
        if let Some(assignee_id) = req.assigned_to {
            self.ensure_project_member(req.project_id, assignee_id)
                .await?;
        }

        if let Some(parent_task_id) = req.parent_task_id {
            self.ensure_parent_task_in_project(req.project_id, parent_task_id)
                .await?;
        }

        if let Some(requirement_id) = req.requirement_id {
            self.ensure_requirement_in_project(req.project_id, requirement_id)
                .await?;
        }

        if let Some(sprint_id) = req.sprint_id {
            self.ensure_sprint_in_project(req.project_id, sprint_id)
                .await?;
        }

        let metadata = req.metadata.unwrap_or_else(|| serde_json::json!({}));

        let task = sqlx::query_as::<_, Task>(
            r#"
            INSERT INTO tasks (
                project_id, title, description, task_type, assigned_to, parent_task_id,
                requirement_id, sprint_id, created_by, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, project_id, title, description, task_type, status, assigned_to, parent_task_id, requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
            "#
        )
        .bind(req.project_id)
        .bind(req.title)
        .bind(req.description)
        .bind(req.task_type)
        .bind(req.assigned_to)
        .bind(req.parent_task_id)
        .bind(req.requirement_id)
        .bind(req.sprint_id)
        .bind(user_id)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create task")?;

        Ok(task)
    }

    pub async fn get_project_tasks(&self, project_id: Uuid) -> Result<Vec<TaskWithLatestAttempt>> {
        let tasks = sqlx::query_as::<_, TaskWithLatestAttempt>(
            r#"
            SELECT
                t.id, t.project_id, t.title, t.description, t.task_type, t.status,
                t.assigned_to, t.parent_task_id, t.requirement_id, t.sprint_id,
                t.gitlab_issue_id, t.metadata, t.created_by, t.created_at, t.updated_at,
                (
                    SELECT ta.id
                    FROM task_attempts ta
                    WHERE ta.task_id = t.id
                    ORDER BY ta.created_at DESC
                    LIMIT 1
                ) as latest_attempt_id
            FROM tasks t
            WHERE t.project_id = $1
            ORDER BY
                CASE lower(COALESCE(t.metadata->>'priority', 'normal'))
                    WHEN 'critical' THEN 0
                    WHEN 'high' THEN 1
                    WHEN 'normal' THEN 2
                    WHEN 'medium' THEN 2
                    WHEN 'low' THEN 3
                    ELSE 2
                END,
                t.created_at DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch project tasks")?;

        Ok(tasks)
    }

    /// Get tasks with computed attempt status fields for kanban board display
    /// Supports optional sprint filtering
    pub async fn get_project_tasks_with_attempt_status(
        &self,
        project_id: Uuid,
        sprint_id: Option<Uuid>,
    ) -> Result<Vec<TaskWithAttemptStatus>> {
        repositories::tasks::get_tasks_with_attempt_status(&self.pool, project_id, sprint_id)
            .await
            .context("Failed to fetch tasks with attempt status")
    }

    /// Get tasks across all projects a user has access to
    pub async fn get_all_user_tasks_with_attempt_status(
        &self,
        user_id: Uuid,
        is_admin: bool,
    ) -> Result<Vec<TaskWithAttemptStatus>> {
        repositories::tasks::get_all_user_tasks_with_attempt_status(&self.pool, user_id, is_admin)
            .await
            .context("Failed to fetch all user tasks with attempt status")
    }

    /// Get tasks for a project filtered by sprint
    pub async fn get_project_tasks_by_sprint(
        &self,
        project_id: Uuid,
        sprint_id: Option<Uuid>,
    ) -> Result<Vec<TaskWithLatestAttempt>> {
        let tasks = match sprint_id {
            Some(sid) => sqlx::query_as::<_, TaskWithLatestAttempt>(
                r#"
                    SELECT
                        t.id, t.project_id, t.title, t.description, t.task_type, t.status,
                        t.assigned_to, t.parent_task_id, t.requirement_id, t.sprint_id,
                        t.gitlab_issue_id, t.metadata, t.created_by, t.created_at, t.updated_at,
                        (
                            SELECT ta.id
                            FROM task_attempts ta
                            WHERE ta.task_id = t.id
                            ORDER BY ta.created_at DESC
                            LIMIT 1
                        ) as latest_attempt_id
                    FROM tasks t
                    WHERE t.project_id = $1 AND t.sprint_id = $2
                    ORDER BY
                        CASE lower(COALESCE(t.metadata->>'priority', 'normal'))
                            WHEN 'critical' THEN 0
                            WHEN 'high' THEN 1
                            WHEN 'normal' THEN 2
                            WHEN 'medium' THEN 2
                            WHEN 'low' THEN 3
                            ELSE 2
                        END,
                        t.created_at DESC
                    "#,
            )
            .bind(project_id)
            .bind(sid)
            .fetch_all(&self.pool)
            .await
            .context("Failed to fetch tasks by sprint")?,
            None => {
                // If no sprint_id, return tasks without sprint assignment (backlog)
                sqlx::query_as::<_, TaskWithLatestAttempt>(
                    r#"
                    SELECT
                        t.id, t.project_id, t.title, t.description, t.task_type, t.status,
                        t.assigned_to, t.parent_task_id, t.requirement_id, t.sprint_id,
                        t.gitlab_issue_id, t.metadata, t.created_by, t.created_at, t.updated_at,
                        (
                            SELECT ta.id
                            FROM task_attempts ta
                            WHERE ta.task_id = t.id
                            ORDER BY ta.created_at DESC
                            LIMIT 1
                        ) as latest_attempt_id
                    FROM tasks t
                    WHERE t.project_id = $1 AND t.sprint_id IS NULL
                    ORDER BY
                        CASE lower(COALESCE(t.metadata->>'priority', 'normal'))
                            WHEN 'critical' THEN 0
                            WHEN 'high' THEN 1
                            WHEN 'normal' THEN 2
                            WHEN 'medium' THEN 2
                            WHEN 'low' THEN 3
                            ELSE 2
                        END,
                        t.created_at DESC
                    "#,
                )
                .bind(project_id)
                .fetch_all(&self.pool)
                .await
                .context("Failed to fetch unassigned tasks")?
            }
        };

        Ok(tasks)
    }

    /// Assign a task to a sprint
    pub async fn assign_to_sprint(&self, task_id: Uuid, sprint_id: Option<Uuid>) -> Result<Task> {
        let existing = self.get_task(task_id).await?.context("Task not found")?;
        if let Some(sid) = sprint_id {
            self.ensure_sprint_in_project(existing.project_id, sid)
                .await?;
        }

        let task = sqlx::query_as::<_, Task>(
            r#"
            UPDATE tasks
            SET sprint_id = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, title, description, task_type, status, assigned_to, parent_task_id, requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
            "#
        )
        .bind(task_id)
        .bind(sprint_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to assign task to sprint")?;

        Ok(task)
    }

    pub async fn get_task(&self, task_id: Uuid) -> Result<Option<Task>> {
        let task = sqlx::query_as::<_, Task>(
            r#"
            SELECT id, project_id, title, description, task_type, status, assigned_to, parent_task_id, requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
            FROM tasks
            WHERE id = $1
            "#
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch task")?;

        Ok(task)
    }

    pub async fn update_task(&self, task_id: Uuid, req: UpdateTaskRequest) -> Result<Task> {
        let existing = self.get_task(task_id).await?.context("Task not found")?;

        if let Some(new_status) = req.status {
            self.validate_status_transition(existing.status, new_status)?;
        }

        if let Some(assignee_id) = req.assigned_to {
            self.ensure_project_member(existing.project_id, assignee_id)
                .await?;
        }

        if let Some(sprint_id) = req.sprint_id {
            self.ensure_sprint_in_project(existing.project_id, sprint_id)
                .await?;
        }

        let task = sqlx::query_as::<_, Task>(
            r#"
            UPDATE tasks
            SET title = COALESCE($2, title),
                description = COALESCE($3, description),
                status = COALESCE($4, status),
                assigned_to = COALESCE($5, assigned_to),
                sprint_id = COALESCE($6, sprint_id),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, title, description, task_type, status, assigned_to, parent_task_id, requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
            "#
        )
        .bind(task_id)
        .bind(req.title)
        .bind(req.description)
        .bind(req.status)
        .bind(req.assigned_to)
        .bind(req.sprint_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to update task")?;

        Ok(task)
    }

    pub async fn delete_task(&self, task_id: Uuid) -> Result<()> {
        // Nullify children first
        sqlx::query(
            r#"
            UPDATE tasks
            SET parent_task_id = NULL
            WHERE parent_task_id = $1
            "#,
        )
        .bind(task_id)
        .execute(&self.pool)
        .await
        .context("Failed to nullify child tasks")?;

        // Delete the task
        sqlx::query(
            r#"
            DELETE FROM tasks
            WHERE id = $1
            "#,
        )
        .bind(task_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete task")?;

        Ok(())
    }

    /// Validate status transition
    pub fn validate_status_transition(&self, from: TaskStatus, to: TaskStatus) -> Result<()> {
        use TaskStatus::*;

        let valid = match (from, to) {
            // Allow staying in same status
            (a, b) if a == b => true,
            // From BACKLOG
            (Backlog, Todo) => true,
            (Backlog, InProgress) => true,
            // From TODO
            (Todo, Backlog) => true,
            (Todo, InProgress) => true,
            (Todo, Done) => true,
            (Todo, Archived) => true,
            // From IN_PROGRESS
            (InProgress, Backlog) => true,
            (InProgress, Todo) => true,
            (InProgress, InReview) => true,
            (InProgress, Done) => true,
            // From IN_REVIEW
            (InReview, InProgress) => true,
            (InReview, Done) => true,
            // From BLOCKED
            (Blocked, Backlog) => true,    // Allow de-prioritizing
            (Blocked, Todo) => true,       // Allow resetting
            (Blocked, InProgress) => true, // Allow retry
            // From DONE
            (Done, Archived) => true,
            (Done, InProgress) => true, // Allow reopening
            // From ARCHIVED
            (Archived, Backlog) => true, // Allow unarchiving to backlog
            (Archived, InProgress) => true, // Allow unarchiving
            // All other transitions forbidden
            _ => false,
        };

        if !valid {
            anyhow::bail!("Invalid status transition from {:?} to {:?}", from, to);
        }

        Ok(())
    }

    /// Update task status with validation
    pub async fn update_task_status(&self, task_id: Uuid, new_status: TaskStatus) -> Result<Task> {
        // Get current task
        let task = self.get_task(task_id).await?.context("Task not found")?;

        // Validate transition
        self.validate_status_transition(task.status, new_status)?;

        // Update status
        let updated_task = sqlx::query_as::<_, Task>(
            r#"
            UPDATE tasks
            SET status = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, title, description, task_type, status, assigned_to, parent_task_id, requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
            "#
        )
        .bind(task_id)
        .bind(new_status)
        .fetch_one(&self.pool)
        .await
        .context("Failed to update task status")?;

        Ok(updated_task)
    }

    /// Get child tasks
    pub async fn get_children(&self, parent_id: Uuid) -> Result<Vec<Task>> {
        let children = sqlx::query_as::<_, Task>(
            r#"
            SELECT id, project_id, title, description, task_type, status, assigned_to, parent_task_id, requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
            FROM tasks
            WHERE parent_task_id = $1
            ORDER BY created_at ASC
            "#
        )
        .bind(parent_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch child tasks")?;

        Ok(children)
    }

    /// Assign user to task
    pub async fn assign_task(&self, task_id: Uuid, user_id: Option<Uuid>) -> Result<Task> {
        let existing = self.get_task(task_id).await?.context("Task not found")?;
        if let Some(assignee_id) = user_id {
            self.ensure_project_member(existing.project_id, assignee_id)
                .await?;
        }

        let task = sqlx::query_as::<_, Task>(
            r#"
            UPDATE tasks
            SET assigned_to = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, title, description, task_type, status, assigned_to, parent_task_id, requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
            "#
        )
        .bind(task_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to assign task")?;

        Ok(task)
    }

    /// Update task metadata
    pub async fn update_metadata(
        &self,
        task_id: Uuid,
        metadata: serde_json::Value,
    ) -> Result<Task> {
        let task = sqlx::query_as::<_, Task>(
            r#"
            UPDATE tasks
            SET metadata = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, title, description, task_type, status, assigned_to, parent_task_id, requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
            "#
        )
        .bind(task_id)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await
        .context("Failed to update task metadata")?;

        Ok(task)
    }

    /// Create init task for GitLab import
    pub async fn create_gitlab_import_task(
        &self,
        project_id: Uuid,
        user_id: Uuid,
        repository_url: &str,
        project_type: Option<ProjectType>,
    ) -> Result<Task> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin task transaction")?;

        let task = Self::create_gitlab_import_task_in_tx_inner(
            &mut tx,
            project_id,
            user_id,
            repository_url,
            project_type,
        )
        .await?;

        tx.commit()
            .await
            .context("Failed to commit GitLab import task transaction")?;

        Ok(task)
    }

    pub async fn create_gitlab_import_task_in_tx(
        tx: &mut Transaction<'_, Postgres>,
        project_id: Uuid,
        user_id: Uuid,
        repository_url: &str,
        project_type: Option<ProjectType>,
    ) -> Result<Task> {
        Self::create_gitlab_import_task_in_tx_inner(
            tx,
            project_id,
            user_id,
            repository_url,
            project_type,
        )
        .await
    }

    async fn create_gitlab_import_task_in_tx_inner(
        tx: &mut Transaction<'_, Postgres>,
        project_id: Uuid,
        user_id: Uuid,
        repository_url: &str,
        project_type: Option<ProjectType>,
    ) -> Result<Task> {
        let metadata = InitTaskMetadata::gitlab_import(repository_url.to_string(), project_type);

        let task = sqlx::query_as::<_, Task>(
            r#"
            INSERT INTO tasks (
                project_id, title, description, task_type,
                status, created_by, metadata
            )
            VALUES ($1, $2, $3, $4::task_type, $5::task_status, $6, $7)
            RETURNING id, project_id, title, description, task_type, status, assigned_to, parent_task_id, requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind("Initialize Local Repository")
        .bind(format!("Clone repository from {}", repository_url))
        .bind(TaskType::Init)
        .bind(TaskStatus::Todo)
        .bind(user_id)
        .bind(metadata)
        .fetch_one(&mut **tx)
        .await
        .context("Failed to create GitLab import task")?;

        Ok(task)
    }

    /// Create an init task for a from-scratch project.
    /// Type-specific scaffolding is provided via skills (init-web-scaffold, init-api-scaffold, etc.).
    pub async fn create_from_scratch_task(
        &self,
        project_id: Uuid,
        user_id: Uuid,
        project_type: ProjectType,
        project_name: &str,
        description: &str,
        preferred_stack: Option<&str>,
        stack_selections: Option<&[ProjectStackSelection]>,
        visibility: &str,
        reference_keys: Option<&[String]>,
    ) -> Result<Task> {
        let ref_keys = reference_keys.map(|k| k.to_vec()).filter(|k| !k.is_empty());
        let stack_selections_vec = stack_selections.map(|s| s.to_vec());
        let metadata = InitTaskMetadata::from_scratch(
            visibility.to_string(),
            ref_keys,
            preferred_stack.map(str::to_string),
            stack_selections_vec,
        );

        let task = sqlx::query_as::<_, Task>(
            r#"
            INSERT INTO tasks (
                project_id, title, description, task_type,
                status, created_by, metadata
            )
            VALUES ($1, $2, $3, $4::task_type, $5::task_status, $6, $7)
            RETURNING id, project_id, title, description, task_type, status, assigned_to, parent_task_id, requirement_id, sprint_id, gitlab_issue_id, metadata, created_by, created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind(format!("Initialize {} Project: {}", project_type.display_name(), project_name))
        .bind(description)  // User's project description; type-specific content comes from skills
        .bind(TaskType::Init)
        .bind(TaskStatus::Todo)
        .bind(user_id)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create from-scratch task")?;

        Ok(task)
    }
}
