use acpms_executors::webhook_job::WebhookJob;
use anyhow::{Context, Result};
use sqlx::PgPool;

/// Handlers for different GitLab webhook event types
#[derive(Clone)]
pub struct WebhookEventHandlers {
    db: PgPool,
}

impl WebhookEventHandlers {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Handle GitLab push events
    pub async fn handle_push(&self, job: &WebhookJob) -> Result<()> {
        let push: acpms_gitlab::PushEvent =
            serde_json::from_value(job.payload.clone()).context("Failed to parse push event")?;

        tracing::info!(
            "Processing push event: {} -> {} by {} ({} commits)",
            push.before,
            push.after,
            push.user_name,
            push.total_commits_count
        );

        // TODO: Implement auto-sync logic
        // - Update branch tracking in database
        // - Trigger incremental sync if needed
        // - Notify relevant users about changes

        Ok(())
    }

    /// Handle GitLab merge request events
    pub async fn handle_merge_request(&self, job: &WebhookJob) -> Result<()> {
        let mr_event: acpms_gitlab::MergeRequestEvent = serde_json::from_value(job.payload.clone())
            .context("Failed to parse merge request event")?;

        let mr = &mr_event.object_attributes;
        let target_project_id = mr_event.project.id as i64;

        tracing::info!(
            "Processing MR event: {} ({}) - {}",
            mr.title,
            mr.state,
            mr.action.as_deref().unwrap_or("unknown")
        );

        // Handle merged MRs - auto-complete tasks
        if mr.state == "merged" {
            self.auto_complete_task_on_merge(job.project_id, mr.iid as i64, target_project_id)
                .await?;
        }

        Ok(())
    }

    /// Handle GitLab pipeline events
    pub async fn handle_pipeline(&self, job: &WebhookJob) -> Result<()> {
        tracing::info!("Processing pipeline event for project {}", job.project_id);

        // TODO: Implement pipeline event handling
        // - Parse pipeline event
        // - Update CI/CD status in task_attempts table
        // - Send notifications on pipeline failures
        // - Track deployment status

        Ok(())
    }

    /// Auto-complete task when MR is merged
    async fn auto_complete_task_on_merge(
        &self,
        project_id: uuid::Uuid,
        mr_iid: i64,
        target_project_id: i64,
    ) -> Result<()> {
        #[derive(sqlx::FromRow)]
        struct TaskId {
            id: uuid::Uuid,
        }

        // Update merge_requests status first for consistency
        let _ = sqlx::query(
            r#"
            UPDATE merge_requests
            SET status = 'merged', updated_at = NOW()
            WHERE gitlab_mr_iid = $1
            AND (target_project_id = $3 OR target_project_id IS NULL)
            AND task_id IN (SELECT id FROM tasks WHERE project_id = $2)
            "#,
        )
        .bind(mr_iid)
        .bind(project_id)
        .bind(target_project_id)
        .execute(&self.db)
        .await;

        let updated = sqlx::query_as::<_, TaskId>(
            r#"
            UPDATE tasks
            SET status = 'done', updated_at = NOW()
            FROM merge_requests
            WHERE tasks.id = merge_requests.task_id
            AND merge_requests.gitlab_mr_iid = $1
            AND (merge_requests.target_project_id = $3 OR merge_requests.target_project_id IS NULL)
            AND tasks.project_id = $2
            RETURNING tasks.id
            "#,
        )
        .bind(mr_iid)
        .bind(project_id)
        .bind(target_project_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to update task status")?;

        if let Some(record) = updated {
            tracing::info!(
                "Auto-completed task {} due to MR merge (webhook)",
                record.id
            );

            // TODO: Send notification to task assignee
            // - Create notification record
            // - Send email if enabled
            // - Push to WebSocket for real-time update
        }

        Ok(())
    }
}
