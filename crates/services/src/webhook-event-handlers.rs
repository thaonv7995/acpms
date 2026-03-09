use acpms_executors::webhook_job::WebhookJob;
use anyhow::{Context, Result};
use sqlx::PgPool;
use std::sync::Arc;

use crate::openclaw_gateway_events::OpenClawGatewayEventService;

/// Handlers for different GitLab webhook event types
#[derive(Clone)]
pub struct WebhookEventHandlers {
    db: PgPool,
    openclaw_event_service: Option<Arc<OpenClawGatewayEventService>>,
}

impl WebhookEventHandlers {
    pub fn new(db: PgPool) -> Self {
        Self {
            db,
            openclaw_event_service: None,
        }
    }

    pub fn with_openclaw_events(
        mut self,
        openclaw_event_service: Arc<OpenClawGatewayEventService>,
    ) -> Self {
        self.openclaw_event_service = Some(openclaw_event_service);
        self
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
            previous_status: String,
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
            WITH matched_task AS (
                SELECT tasks.id, tasks.status::text AS previous_status
                FROM tasks
                JOIN merge_requests ON tasks.id = merge_requests.task_id
                WHERE merge_requests.gitlab_mr_iid = $1
                  AND (merge_requests.target_project_id = $3 OR merge_requests.target_project_id IS NULL)
                  AND tasks.project_id = $2
                  AND tasks.status <> 'done'
                LIMIT 1
            )
            UPDATE tasks
            SET status = 'done', updated_at = NOW()
            FROM matched_task
            WHERE tasks.id = matched_task.id
            RETURNING tasks.id, matched_task.previous_status
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

            if let Some(openclaw_event_service) = &self.openclaw_event_service {
                if let Err(error) = openclaw_event_service
                    .record_task_status_changed(
                        project_id,
                        record.id,
                        &record.previous_status,
                        "done",
                        "services.webhook_event_handlers.auto_complete_task_on_merge",
                    )
                    .await
                {
                    tracing::warn!(
                        task_id = %record.id,
                        error = %error,
                        "Failed to emit OpenClaw task.status_changed event from webhook merge"
                    );
                }
            }

            // TODO: Send notification to task assignee
            // - Create notification record
            // - Send email if enabled
            // - Push to WebSocket for real-time update
        }

        Ok(())
    }
}
