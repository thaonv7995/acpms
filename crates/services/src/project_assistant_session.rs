use acpms_db::{
    models::ProjectAssistantSession,
    repositories::{
        create, delete_beyond_recent, end_session as repo_end_session,
        find_active_by_project_and_user, get_by_id, list_by_project_and_user,
    },
    PgPool,
};
use anyhow::{Context, Result};
use uuid::Uuid;

pub struct ProjectAssistantSessionService {
    pool: PgPool,
}

impl ProjectAssistantSessionService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new active session row.
    pub async fn create_session(
        &self,
        project_id: Uuid,
        user_id: Uuid,
    ) -> Result<ProjectAssistantSession> {
        create(&self.pool, project_id, user_id)
            .await
            .context("Failed to create session")
    }

    /// Return the current active session for this user/project if one exists.
    pub async fn find_active_session(
        &self,
        project_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<ProjectAssistantSession>> {
        find_active_by_project_and_user(&self.pool, project_id, user_id)
            .await
            .context("Failed to find active session")
    }

    /// Reuse the active session if present, otherwise create a new one.
    pub async fn get_or_create_session(
        &self,
        project_id: Uuid,
        user_id: Uuid,
    ) -> Result<ProjectAssistantSession> {
        if let Some(session) = self.find_active_session(project_id, user_id).await? {
            return Ok(session);
        }
        self.create_session(project_id, user_id).await
    }

    /// List sessions for user in project. Keeps only 3 most recent; deletes older ones.
    pub async fn list_sessions(
        &self,
        project_id: Uuid,
        user_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ProjectAssistantSession>> {
        let _ = delete_beyond_recent(&self.pool, project_id, user_id, limit)
            .await
            .context("Failed to prune old sessions");
        list_by_project_and_user(&self.pool, project_id, user_id, limit)
            .await
            .context("Failed to list sessions")
    }

    /// Get session by id
    pub async fn get_session(&self, session_id: Uuid) -> Result<Option<ProjectAssistantSession>> {
        get_by_id(&self.pool, session_id)
            .await
            .context("Failed to get session")
    }

    /// End session (set status=ended, s3_log_key). Caller must upload JSONL first.
    pub async fn end_session(&self, session_id: Uuid, s3_log_key: &str) -> Result<u64> {
        repo_end_session(&self.pool, session_id, s3_log_key)
            .await
            .context("Failed to end session")
    }
}
