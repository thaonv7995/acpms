use acpms_db::models::{CreateWorkspaceRepo, WorkspaceRepo};
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

/// Service for managing workspace repositories (multi-repo support)
pub struct WorkspaceRepoService {
    pool: PgPool,
}

impl WorkspaceRepoService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a repository association for a workspace
    pub async fn create_repo(
        &self,
        attempt_id: Uuid,
        project_id: Uuid,
        repo: CreateWorkspaceRepo,
    ) -> Result<WorkspaceRepo> {
        let row = sqlx::query_as::<_, WorkspaceRepo>(
            r#"INSERT INTO workspace_repos
               (attempt_id, project_id, repo_name, repo_url, worktree_path, relative_path,
                target_branch, base_branch, is_primary)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               RETURNING *"#,
        )
        .bind(attempt_id)
        .bind(project_id)
        .bind(&repo.repo_name)
        .bind(&repo.repo_url)
        .bind(&repo.worktree_path)
        .bind(&repo.relative_path)
        .bind(&repo.target_branch)
        .bind(&repo.base_branch)
        .bind(repo.is_primary)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// Get all repos for an attempt
    pub async fn get_repos_for_attempt(&self, attempt_id: Uuid) -> Result<Vec<WorkspaceRepo>> {
        let repos = sqlx::query_as::<_, WorkspaceRepo>(
            r#"SELECT * FROM workspace_repos
               WHERE attempt_id = $1
               ORDER BY is_primary DESC, created_at ASC"#,
        )
        .bind(attempt_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(repos)
    }

    /// Get primary repo for an attempt
    pub async fn get_primary_repo(&self, attempt_id: Uuid) -> Result<Option<WorkspaceRepo>> {
        let repo = sqlx::query_as::<_, WorkspaceRepo>(
            r#"SELECT * FROM workspace_repos
               WHERE attempt_id = $1 AND is_primary = true"#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(repo)
    }

    /// Update repo metadata
    pub async fn update_repo_metadata(
        &self,
        repo_id: Uuid,
        metadata: serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE workspace_repos
               SET metadata = metadata || $2, updated_at = now()
               WHERE id = $1"#,
        )
        .bind(repo_id)
        .bind(metadata)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a repo from workspace
    pub async fn delete_repo(&self, repo_id: Uuid) -> Result<u64> {
        let result = sqlx::query(r#"DELETE FROM workspace_repos WHERE id = $1"#)
            .bind(repo_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "../db/migrations")]
    async fn test_create_and_get_repos(pool: PgPool) {
        let service = WorkspaceRepoService::new(pool.clone());

        let attempt_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Setup
        sqlx::query("INSERT INTO users (id, email, name) VALUES ($1, $2, $3)")
            .bind(user_id)
            .bind("workspace-repos-test@example.com")
            .bind("Workspace Repos Test")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("INSERT INTO projects (id, name, created_by) VALUES ($1, 'Test', $2)")
            .bind(project_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO tasks (id, project_id, title, description, task_type, status, created_by)
             VALUES ($1, $2, 'Test', 'Test', 'feature', 'todo', $3)",
        )
        .bind(task_id)
        .bind(project_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO task_attempts (id, task_id, status)
             VALUES ($1, $2, 'queued')",
        )
        .bind(attempt_id)
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();

        // Create repos
        let repo1 = CreateWorkspaceRepo {
            repo_name: "backend".into(),
            repo_url: "https://gitlab.com/test/backend".into(),
            worktree_path: "/tmp/backend".into(),
            relative_path: "backend/".into(),
            target_branch: "feature-x".into(),
            base_branch: "main".into(),
            is_primary: true,
        };

        let created = service
            .create_repo(attempt_id, project_id, repo1)
            .await
            .unwrap();
        assert_eq!(created.repo_name, "backend");
        assert!(created.is_primary);

        // Get repos
        let repos = service.get_repos_for_attempt(attempt_id).await.unwrap();
        assert_eq!(repos.len(), 1);

        // Get primary
        let primary = service.get_primary_repo(attempt_id).await.unwrap();
        assert!(primary.is_some());
    }
}
