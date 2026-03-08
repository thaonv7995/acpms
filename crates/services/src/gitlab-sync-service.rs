use crate::gitlab::GitLabService;
use crate::openclaw_gateway_events::OpenClawGatewayEventService;
use anyhow::{Context, Result};
use sqlx::{FromRow, PgPool};
use std::sync::Arc;
use uuid::Uuid;

#[derive(FromRow)]
struct MrRow {
    task_id: Uuid,
    attempt_id: Option<Uuid>,
    gitlab_mr_iid: i64,
    target_project_id: Option<i64>,
}

/// GitLab auto-sync service for incremental synchronization
pub struct GitLabSyncService {
    db: PgPool,
    gitlab_service: GitLabService,
    openclaw_event_service: Option<Arc<OpenClawGatewayEventService>>,
}

impl GitLabSyncService {
    pub fn new(db: PgPool, gitlab_service: GitLabService) -> Self {
        Self {
            db,
            gitlab_service,
            openclaw_event_service: None,
        }
    }

    pub fn with_openclaw_events(mut self, openclaw_event_service: Arc<OpenClawGatewayEventService>) -> Self {
        self.openclaw_event_service = Some(openclaw_event_service);
        self
    }

    /// Trigger incremental sync for a project
    ///
    /// ## Sync Strategy
    /// - Only sync changes since last_sync_at
    /// - Sync branches, merge requests, pipelines
    /// - Handle rate limiting (60 req/min per token)
    /// - Store sync metadata for next incremental sync
    pub async fn sync_project(&self, project_id: Uuid) -> Result<SyncResult> {
        // Mark sync as in progress
        self.mark_sync_status(project_id, "syncing").await?;

        let result = match self.perform_sync(project_id).await {
            Ok(sync_result) => {
                self.mark_sync_status(project_id, "idle").await?;
                sync_result
            }
            Err(e) => {
                self.mark_sync_error(project_id, &e).await?;
                return Err(e);
            }
        };

        Ok(result)
    }

    async fn perform_sync(&self, project_id: Uuid) -> Result<SyncResult> {
        // Get GitLab client
        let _client = self.gitlab_service.get_client(project_id).await?;

        // Get sync metadata
        let metadata = self.get_or_create_sync_metadata(project_id).await?;

        // TODO: Implement actual sync logic
        // 1. Sync branches (compare with last_branch_sync_at)
        // 2. Sync merge requests (compare with last_mr_sync_at)
        // 3. Sync pipelines (compare with last_pipeline_sync_at)
        // 4. Update sync metadata

        tracing::info!(
            "Syncing project {} (last sync: {:?})",
            project_id,
            metadata.last_sync_at
        );

        // Update sync timestamp
        self.update_sync_timestamp(project_id).await?;

        Ok(SyncResult {
            branches_synced: 0,
            merge_requests_synced: 0,
            pipelines_synced: 0,
        })
    }

    async fn get_or_create_sync_metadata(&self, project_id: Uuid) -> Result<SyncMetadata> {
        // Get GitLab configuration
        let config = self
            .gitlab_service
            .get_config(project_id)
            .await?
            .context("Project not linked to GitLab")?;

        // Get or create sync metadata
        let metadata = sqlx::query_as::<_, SyncMetadata>(
            r#"
            INSERT INTO gitlab_sync_metadata
            (project_id, gitlab_project_id, last_sync_at, sync_status)
            VALUES ($1, $2, NOW(), 'idle')
            ON CONFLICT (project_id) DO UPDATE SET updated_at = NOW()
            RETURNING
                project_id,
                gitlab_project_id,
                last_sync_at,
                last_branch_sync_at,
                last_mr_sync_at,
                last_pipeline_sync_at,
                sync_status
            "#,
        )
        .bind(project_id)
        .bind(config.gitlab_project_id)
        .fetch_one(&self.db)
        .await
        .context("Failed to get or create sync metadata")?;

        Ok(metadata)
    }

    async fn mark_sync_status(&self, project_id: Uuid, status: &str) -> Result<()> {
        sqlx::query("UPDATE gitlab_sync_metadata SET sync_status = $1 WHERE project_id = $2")
            .bind(status)
            .bind(project_id)
            .execute(&self.db)
            .await
            .context("Failed to update sync status")?;

        Ok(())
    }

    async fn mark_sync_error(&self, project_id: Uuid, error: &anyhow::Error) -> Result<()> {
        sqlx::query(
            "UPDATE gitlab_sync_metadata SET sync_status = 'error', sync_error = $1 WHERE project_id = $2"
        )
        .bind(error.to_string())
        .bind(project_id)
        .execute(&self.db)
        .await
        .context("Failed to mark sync error")?;

        Ok(())
    }

    async fn update_sync_timestamp(&self, project_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE gitlab_sync_metadata SET last_sync_at = NOW() WHERE project_id = $1")
            .bind(project_id)
            .execute(&self.db)
            .await
            .context("Failed to update sync timestamp")?;

        Ok(())
    }

    /// Sync MR status for InReview tasks in project.
    /// When MR is merged on GitLab, update task to Done and merge_requests.
    /// Returns attempt_ids that were updated (for worktree cleanup).
    /// Called when loading Kanban so tasks reflect merged state without opening diff view.
    pub async fn sync_mr_status_for_project(&self, project_id: Uuid) -> Result<Vec<Uuid>> {
        let config = self
            .gitlab_service
            .get_config(project_id)
            .await?
            .context("Project not linked to GitLab")?;

        let rows: Vec<MrRow> = sqlx::query_as(
            r#"
            SELECT mr.task_id, mr.attempt_id, mr.gitlab_mr_iid, mr.target_project_id
            FROM merge_requests mr
            JOIN tasks t ON t.id = mr.task_id
            WHERE t.project_id = $1 AND t.status = 'in_review'
            AND LOWER(mr.status) != 'merged'
            AND mr.gitlab_mr_iid IS NOT NULL
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.db)
        .await
        .context("Failed to fetch InReview tasks with MRs")?;

        if rows.is_empty() {
            return Ok(vec![]);
        }

        let client = self.gitlab_service.get_client(project_id).await?;
        let mut updated_attempt_ids = Vec::new();

        for row in rows {
            let gitlab_project_id =
                row.target_project_id.unwrap_or(config.gitlab_project_id) as u64;
            match client
                .get_merge_request(gitlab_project_id, row.gitlab_mr_iid as u64)
                .await
            {
                Ok(mr) if mr.state.eq_ignore_ascii_case("merged") => {
                    let _ = sqlx::query(
                        "UPDATE merge_requests SET status = 'merged', updated_at = NOW() WHERE task_id = $1 AND gitlab_mr_iid = $2",
                    )
                    .bind(row.task_id)
                    .bind(row.gitlab_mr_iid)
                    .execute(&self.db)
                    .await;

                    let _ = sqlx::query(
                        "UPDATE tasks SET status = 'done', updated_at = NOW() WHERE id = $1",
                    )
                    .bind(row.task_id)
                    .execute(&self.db)
                    .await;

                    if let Some(openclaw_event_service) = &self.openclaw_event_service {
                        if let Err(error) = openclaw_event_service
                            .record_task_status_changed(
                                project_id,
                                row.task_id,
                                "in_review",
                                "done",
                                "services.gitlab_sync_service.sync_mr_status_for_project",
                            )
                            .await
                        {
                            tracing::warn!(
                                task_id = %row.task_id,
                                error = %error,
                                "Failed to emit OpenClaw task.status_changed event from GitLab sync"
                            );
                        }
                    }

                    if let Some(aid) = row.attempt_id {
                        updated_attempt_ids.push(aid);
                    }
                    tracing::info!(
                        "Synced task {} to Done (MR !{} merged on GitLab)",
                        row.task_id,
                        row.gitlab_mr_iid
                    );
                }
                _ => {}
            }
        }

        Ok(updated_attempt_ids)
    }
}

#[derive(Debug, FromRow)]
struct SyncMetadata {
    #[allow(dead_code)]
    project_id: Uuid,
    #[allow(dead_code)]
    gitlab_project_id: i64,
    last_sync_at: chrono::DateTime<chrono::Utc>,
    #[allow(dead_code)]
    last_branch_sync_at: Option<chrono::DateTime<chrono::Utc>>,
    #[allow(dead_code)]
    last_mr_sync_at: Option<chrono::DateTime<chrono::Utc>>,
    #[allow(dead_code)]
    last_pipeline_sync_at: Option<chrono::DateTime<chrono::Utc>>,
    #[allow(dead_code)]
    sync_status: String,
}

#[derive(Debug, Clone)]
pub struct SyncResult {
    pub branches_synced: u32,
    pub merge_requests_synced: u32,
    pub pipelines_synced: u32,
}
