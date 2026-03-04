use crate::system_settings_service::SystemSettingsService;
use acpms_db::models::{GitLabConfiguration, LinkGitLabProjectRequest};
use acpms_gitlab::GitLabClient;
use anyhow::{bail, Context, Result};
use sqlx::PgPool;
use uuid::Uuid;

/// Parse repository URL (HTTPS or SSH) to project path (e.g. "group/repo").
fn parse_repo_path_from_url(repo_url: &str) -> Option<String> {
    let trimmed = repo_url.trim();
    if trimmed.is_empty() {
        return None;
    }
    // HTTPS/HTTP: https://host/group/repo(.git)
    if let Some(rest) = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
    {
        let without_auth = rest.rsplit('@').next().unwrap_or(rest);
        let (_, path) = without_auth.split_once('/')?;
        let path = path.trim().trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        if path.is_empty() {
            return None;
        }
        return Some(path.to_string());
    }
    // SSH: git@host:group/repo(.git)
    if let Some((_, right)) = trimmed.split_once(':') {
        let path = right.trim().trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        if path.is_empty() {
            return None;
        }
        return Some(path.to_string());
    }
    None
}

/// GitLabService manages per-project GitLab configuration.
///
/// ## Architecture (Phase 2)
/// - Global GitLab URL and PAT are stored in `system_settings` (singleton)
/// - Per-project config only stores `gitlab_project_id` reference
/// - Webhook secrets remain per-project for security isolation
#[derive(Clone)]
pub struct GitLabService {
    db: PgPool,
    settings_service: SystemSettingsService,
}

impl GitLabService {
    /// Create a new GitLabService.
    ///
    /// Uses SystemSettingsService for global GitLab credentials.
    pub fn new(db: PgPool) -> Result<Self> {
        let settings_service = SystemSettingsService::new(db.clone())?;
        Ok(Self {
            db,
            settings_service,
        })
    }

    /// Link a project to a GitLab project.
    ///
    /// Accepts either gitlab_project_id or repository_url. If repository_url is provided,
    /// resolves it to project ID via GitLab API (supports HTTPS and SSH URLs).
    pub async fn link_project(
        &self,
        project_id: Uuid,
        req: LinkGitLabProjectRequest,
    ) -> Result<GitLabConfiguration> {
        // Verify global GitLab is configured
        let settings = self.settings_service.get().await?;
        if settings.gitlab_pat_encrypted.is_none() {
            bail!("GitLab PAT not configured in system settings. Please configure GitLab in Settings first.");
        }

        let gitlab_project_id: i64 = match (&req.repository_url, req.gitlab_project_id) {
            (Some(url), _) => {
                let path = parse_repo_path_from_url(url)
                    .context("Invalid repository URL. Use format: https://gitlab.com/group/repo or git@gitlab.com:group/repo")?;
                let pat = self
                    .settings_service
                    .get_gitlab_pat()
                    .await?
                    .context("GitLab PAT not configured")?;
                let client = GitLabClient::new(&settings.gitlab_url, &pat)?;
                let project = client
                    .get_project_by_path(&path)
                    .await
                    .context("Failed to find GitLab project. Check URL and PAT permissions.")?;
                project.id as i64
            }
            (None, Some(id)) => id,
            (None, None) => bail!("Provide either repository_url or gitlab_project_id"),
        };

        // Generate a random webhook secret for this project
        let webhook_secret = Uuid::new_v4().to_string();

        let config = sqlx::query_as::<_, GitLabConfiguration>(
            r#"
            INSERT INTO gitlab_configurations
            (project_id, gitlab_project_id, base_url, pat_encrypted, webhook_secret)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (project_id)
            DO UPDATE SET
                gitlab_project_id = EXCLUDED.gitlab_project_id,
                base_url = EXCLUDED.base_url,
                updated_at = NOW()
            RETURNING *
            "#,
        )
        .bind(project_id)
        .bind(gitlab_project_id)
        .bind(&settings.gitlab_url)
        .bind("GLOBAL")
        .bind(webhook_secret)
        .fetch_one(&self.db)
        .await
        .context("Failed to save GitLab configuration")?;

        // Setup webhook so GitLab notifies us when MR is merged (task auto-complete)
        if let Err(e) = self.setup_webhook(project_id).await {
            tracing::warn!("Webhook setup failed (link succeeded): {}", e);
        }

        Ok(config)
    }

    /// Get GitLab configuration for a project.
    pub async fn get_config(&self, project_id: Uuid) -> Result<Option<GitLabConfiguration>> {
        let config = sqlx::query_as::<_, GitLabConfiguration>(
            "SELECT * FROM gitlab_configurations WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to fetch GitLab configuration")?;

        Ok(config)
    }

    /// Get GitLab client for a project using global PAT.
    pub async fn get_client(&self, project_id: Uuid) -> Result<GitLabClient> {
        let _config = self
            .get_config(project_id)
            .await?
            .context("Project is not linked to GitLab")?;

        // Get global PAT from system settings
        let settings = self.settings_service.get().await?;
        let pat = self
            .settings_service
            .get_gitlab_pat()
            .await?
            .context("GitLab PAT not configured in system settings")?;

        // Use base_url from system settings (not per-project)
        GitLabClient::new(&settings.gitlab_url, &pat)
    }

    /// Get merge requests for a task.
    pub async fn get_task_merge_requests(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<acpms_db::models::MergeRequestDb>> {
        let mrs = sqlx::query_as::<_, acpms_db::models::MergeRequestDb>(
            "SELECT * FROM merge_requests WHERE task_id = $1",
        )
        .bind(task_id)
        .fetch_all(&self.db)
        .await
        .context("Failed to fetch task merge requests")?;

        Ok(mrs)
    }

    /// Setup webhook for a project.
    pub async fn setup_webhook(&self, project_id: Uuid) -> Result<()> {
        let config = self
            .get_config(project_id)
            .await?
            .context("Project not linked to GitLab")?;

        // Get client using global PAT
        let client = self.get_client(project_id).await?;

        // Construct Webhook URL (must match route: /api/v1/webhooks/gitlab)
        let app_url =
            std::env::var("APP_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
        let webhook_url = format!("{}/api/v1/webhooks/gitlab", app_url.trim_end_matches('/'));

        // Register with GitLab
        client
            .create_webhook(
                config.gitlab_project_id as u64,
                &webhook_url,
                &config.webhook_secret,
            )
            .await?;

        // Record in DB
        sqlx::query(
            r#"
            INSERT INTO gitlab_webhooks
            (project_id, gitlab_id, url, events, secret_token)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(project_id)
        .bind(0i64) // Placeholder - webhook ID from GitLab
        .bind(&webhook_url)
        .bind(["push", "merge_request"])
        .bind(&config.webhook_secret)
        .execute(&self.db)
        .await
        .context("Failed to save webhook record")?;

        Ok(())
    }

    /// Check if GitLab is configured globally.
    pub async fn is_configured(&self) -> Result<bool> {
        let pat = self.settings_service.get_gitlab_pat().await?;
        Ok(pat.is_some())
    }

    /// Get global GitLab URL from system settings.
    pub async fn get_gitlab_url(&self) -> Result<String> {
        let settings = self.settings_service.get().await?;
        Ok(settings.gitlab_url)
    }
}
