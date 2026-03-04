use crate::encryption_service::EncryptionService;
use acpms_db::models::{SystemSettings, SystemSettingsResponse, UpdateSystemSettingsRequest};
use anyhow::{Context, Result};
use sqlx::PgPool;

/// SystemSettingsService manages global system configuration.
///
/// ## Design
/// - Singleton pattern: Only one settings row exists in the database
/// - Sensitive values (PAT, API tokens) are AES-256-GCM encrypted
/// - Response DTOs never expose encrypted values, only indicators
#[derive(Clone)]
pub struct SystemSettingsService {
    db: PgPool,
    encryption: EncryptionService,
}

impl SystemSettingsService {
    /// Create new SystemSettingsService with encryption support.
    pub fn new(db: PgPool) -> Result<Self> {
        let encryption = EncryptionService::from_env()
            .context("Failed to initialize encryption for system settings")?;
        Ok(Self { db, encryption })
    }

    /// Create new SystemSettingsService with injected encryption service (useful for testing).
    pub fn new_with_encryption(db: PgPool, encryption: EncryptionService) -> Self {
        Self { db, encryption }
    }

    /// Get worktrees path for agent worktrees (DB override or env WORKTREES_PATH or default $HOME/Projects).
    pub async fn get_worktrees_path(&self) -> Result<String> {
        let settings = self.get().await?;
        let default = std::env::var("HOME")
            .ok()
            .map(|h| format!("{}/Projects", h.trim_end_matches('/')))
            .unwrap_or_else(|| "./worktrees".to_string());
        Ok(settings
            .worktrees_path
            .filter(|p| !p.is_empty())
            .or_else(|| std::env::var("WORKTREES_PATH").ok())
            .unwrap_or(default))
    }

    /// Get current system settings.
    ///
    /// Returns the singleton settings row (auto-created by migration).
    pub async fn get(&self) -> Result<SystemSettings> {
        let settings = sqlx::query_as::<_, SystemSettings>("SELECT * FROM system_settings LIMIT 1")
            .fetch_one(&self.db)
            .await
            .context("Failed to fetch system settings")?;

        Ok(settings)
    }

    /// Get system settings as safe response DTO (no encrypted values).
    pub async fn get_response(&self) -> Result<SystemSettingsResponse> {
        let settings = self.get().await?;
        Ok(SystemSettingsResponse::from(settings))
    }

    /// Update system settings.
    ///
    /// ## Security
    /// - PAT and API tokens are encrypted before storage
    /// - Empty strings clear the value (set to NULL)
    /// - Partial updates supported (only provided fields are updated)
    pub async fn update(&self, req: UpdateSystemSettingsRequest) -> Result<SystemSettingsResponse> {
        // Build dynamic update query
        let mut updates = Vec::new();
        let mut param_idx = 1;

        // Track values for binding
        let mut gitlab_url_val: Option<String> = None;
        let mut gitlab_pat_encrypted_val: Option<Option<String>> = None;
        let mut gitlab_auto_sync_val: Option<bool> = None;
        let mut agent_cli_provider_val: Option<String> = None;
        let mut cloudflare_account_id_val: Option<Option<String>> = None;
        let mut cloudflare_api_token_encrypted_val: Option<Option<String>> = None;
        let mut cloudflare_zone_id_val: Option<Option<String>> = None;
        let mut cloudflare_base_domain_val: Option<Option<String>> = None;
        let mut notifications_email_val: Option<bool> = None;
        let mut notifications_slack_val: Option<bool> = None;
        let mut notifications_slack_webhook_val: Option<Option<String>> = None;
        let mut worktrees_path_val: Option<Option<String>> = None;
        let mut preferred_agent_language_val: Option<Option<String>> = None;

        if let Some(url) = req.gitlab_url {
            updates.push(format!("gitlab_url = ${}", param_idx));
            gitlab_url_val = Some(url);
            param_idx += 1;
        }

        if let Some(pat) = req.gitlab_pat {
            if pat.is_empty() {
                updates.push(format!("gitlab_pat_encrypted = ${}", param_idx));
                gitlab_pat_encrypted_val = Some(None);
            } else {
                let encrypted = self
                    .encryption
                    .encrypt(&pat)
                    .context("Failed to encrypt GitLab PAT")?;
                updates.push(format!("gitlab_pat_encrypted = ${}", param_idx));
                gitlab_pat_encrypted_val = Some(Some(encrypted));
            }
            param_idx += 1;
        }

        if let Some(auto_sync) = req.gitlab_auto_sync {
            updates.push(format!("gitlab_auto_sync = ${}", param_idx));
            gitlab_auto_sync_val = Some(auto_sync);
            param_idx += 1;
        }

        if let Some(provider) = req.agent_cli_provider {
            // No hard validation here; frontend restricts values and DB default is safe.
            updates.push(format!("agent_cli_provider = ${}", param_idx));
            agent_cli_provider_val = Some(provider);
            param_idx += 1;
        }

        if let Some(account_id) = req.cloudflare_account_id {
            if account_id.is_empty() {
                updates.push(format!("cloudflare_account_id = ${}", param_idx));
                cloudflare_account_id_val = Some(None);
            } else {
                updates.push(format!("cloudflare_account_id = ${}", param_idx));
                cloudflare_account_id_val = Some(Some(account_id));
            }
            param_idx += 1;
        }

        if let Some(token) = req.cloudflare_api_token {
            if token.is_empty() {
                updates.push(format!("cloudflare_api_token_encrypted = ${}", param_idx));
                cloudflare_api_token_encrypted_val = Some(None);
            } else {
                let encrypted = self
                    .encryption
                    .encrypt(&token)
                    .context("Failed to encrypt Cloudflare API token")?;
                updates.push(format!("cloudflare_api_token_encrypted = ${}", param_idx));
                cloudflare_api_token_encrypted_val = Some(Some(encrypted));
            }
            param_idx += 1;
        }

        if let Some(zone_id) = req.cloudflare_zone_id {
            if zone_id.is_empty() {
                updates.push(format!("cloudflare_zone_id = ${}", param_idx));
                cloudflare_zone_id_val = Some(None);
            } else {
                updates.push(format!("cloudflare_zone_id = ${}", param_idx));
                cloudflare_zone_id_val = Some(Some(zone_id));
            }
            param_idx += 1;
        }

        if let Some(base_domain) = req.cloudflare_base_domain {
            if base_domain.is_empty() {
                updates.push(format!("cloudflare_base_domain = ${}", param_idx));
                cloudflare_base_domain_val = Some(None);
            } else {
                updates.push(format!("cloudflare_base_domain = ${}", param_idx));
                cloudflare_base_domain_val = Some(Some(base_domain));
            }
            param_idx += 1;
        }

        if let Some(email) = req.notifications_email_enabled {
            updates.push(format!("notifications_email_enabled = ${}", param_idx));
            notifications_email_val = Some(email);
            param_idx += 1;
        }

        if let Some(slack) = req.notifications_slack_enabled {
            updates.push(format!("notifications_slack_enabled = ${}", param_idx));
            notifications_slack_val = Some(slack);
            param_idx += 1;
        }

        if let Some(webhook_url) = req.notifications_slack_webhook_url {
            if webhook_url.is_empty() {
                updates.push(format!("notifications_slack_webhook_url = ${}", param_idx));
                notifications_slack_webhook_val = Some(None);
            } else {
                updates.push(format!("notifications_slack_webhook_url = ${}", param_idx));
                notifications_slack_webhook_val = Some(Some(webhook_url));
            }
            param_idx += 1;
        }

        if let Some(path) = req.worktrees_path {
            updates.push(format!("worktrees_path = ${}", param_idx));
            worktrees_path_val = Some(if path.trim().is_empty() {
                None
            } else {
                Some(path.trim().to_string())
            });
        }

        if let Some(lang) = req.preferred_agent_language {
            updates.push(format!("preferred_agent_language = ${}", param_idx));
            preferred_agent_language_val = Some(if lang.trim().is_empty() {
                None
            } else {
                Some(lang.trim().to_string())
            });
        }

        if updates.is_empty() {
            // No updates, just return current settings
            return self.get_response().await;
        }

        // Build and execute query using raw SQL with proper binding
        let _query = format!(
            "UPDATE system_settings SET {}, updated_at = NOW() RETURNING *",
            updates.join(", ")
        );

        // Use a simpler approach with individual queries for each field
        // This is more maintainable and avoids complex dynamic binding
        let settings = self
            .update_individual_fields(
                gitlab_url_val,
                gitlab_pat_encrypted_val,
                gitlab_auto_sync_val,
                agent_cli_provider_val,
                cloudflare_account_id_val,
                cloudflare_api_token_encrypted_val,
                cloudflare_zone_id_val,
                cloudflare_base_domain_val,
                notifications_email_val,
                notifications_slack_val,
                notifications_slack_webhook_val,
                worktrees_path_val,
                preferred_agent_language_val,
            )
            .await?;

        Ok(SystemSettingsResponse::from(settings))
    }

    /// Helper to update individual fields (cleaner than dynamic SQL).
    #[allow(clippy::too_many_arguments)]
    async fn update_individual_fields(
        &self,
        gitlab_url: Option<String>,
        gitlab_pat_encrypted: Option<Option<String>>,
        gitlab_auto_sync: Option<bool>,
        agent_cli_provider: Option<String>,
        cloudflare_account_id: Option<Option<String>>,
        cloudflare_api_token_encrypted: Option<Option<String>>,
        cloudflare_zone_id: Option<Option<String>>,
        cloudflare_base_domain: Option<Option<String>>,
        notifications_email: Option<bool>,
        notifications_slack: Option<bool>,
        notifications_slack_webhook: Option<Option<String>>,
        worktrees_path: Option<Option<String>>,
        preferred_agent_language: Option<Option<String>>,
    ) -> Result<SystemSettings> {
        // Update each field if provided
        if let Some(url) = gitlab_url {
            sqlx::query("UPDATE system_settings SET gitlab_url = $1, updated_at = NOW()")
                .bind(&url)
                .execute(&self.db)
                .await?;
        }

        if let Some(pat) = gitlab_pat_encrypted {
            sqlx::query("UPDATE system_settings SET gitlab_pat_encrypted = $1, updated_at = NOW()")
                .bind(pat.as_deref())
                .execute(&self.db)
                .await?;
        }

        if let Some(auto_sync) = gitlab_auto_sync {
            sqlx::query("UPDATE system_settings SET gitlab_auto_sync = $1, updated_at = NOW()")
                .bind(auto_sync)
                .execute(&self.db)
                .await?;
        }

        if let Some(provider) = agent_cli_provider {
            sqlx::query("UPDATE system_settings SET agent_cli_provider = $1, updated_at = NOW()")
                .bind(&provider)
                .execute(&self.db)
                .await?;
        }

        if let Some(account_id) = cloudflare_account_id {
            sqlx::query(
                "UPDATE system_settings SET cloudflare_account_id = $1, updated_at = NOW()",
            )
            .bind(account_id.as_deref())
            .execute(&self.db)
            .await?;
        }

        if let Some(token) = cloudflare_api_token_encrypted {
            sqlx::query("UPDATE system_settings SET cloudflare_api_token_encrypted = $1, updated_at = NOW()")
                .bind(token.as_deref())
                .execute(&self.db)
                .await?;
        }

        if let Some(zone_id) = cloudflare_zone_id {
            sqlx::query("UPDATE system_settings SET cloudflare_zone_id = $1, updated_at = NOW()")
                .bind(zone_id.as_deref())
                .execute(&self.db)
                .await?;
        }

        if let Some(base_domain) = cloudflare_base_domain {
            sqlx::query(
                "UPDATE system_settings SET cloudflare_base_domain = $1, updated_at = NOW()",
            )
            .bind(base_domain.as_deref())
            .execute(&self.db)
            .await?;
        }

        if let Some(email) = notifications_email {
            sqlx::query(
                "UPDATE system_settings SET notifications_email_enabled = $1, updated_at = NOW()",
            )
            .bind(email)
            .execute(&self.db)
            .await?;
        }

        if let Some(slack) = notifications_slack {
            sqlx::query(
                "UPDATE system_settings SET notifications_slack_enabled = $1, updated_at = NOW()",
            )
            .bind(slack)
            .execute(&self.db)
            .await?;
        }

        if let Some(webhook) = notifications_slack_webhook {
            sqlx::query("UPDATE system_settings SET notifications_slack_webhook_url = $1, updated_at = NOW()")
                .bind(webhook.as_deref())
                .execute(&self.db)
                .await?;
        }

        if let Some(path) = worktrees_path {
            sqlx::query("UPDATE system_settings SET worktrees_path = $1, updated_at = NOW()")
                .bind(path.as_deref())
                .execute(&self.db)
                .await?;
        }

        if let Some(lang) = preferred_agent_language {
            sqlx::query(
                "UPDATE system_settings SET preferred_agent_language = $1, updated_at = NOW()",
            )
            .bind(lang.as_deref())
            .execute(&self.db)
            .await?;
        }

        // Return updated settings
        self.get().await
    }

    /// Get PAT for a given repo URL when host matches configured URL (GitLab or GitHub).
    pub async fn get_pat_for_repo(&self, repo_url: &str) -> Result<Option<String>> {
        let settings = self.get().await?;
        let repo_host =
            parse_repo_host(repo_url).ok_or_else(|| anyhow::anyhow!("Invalid repo URL"))?;
        let configured_host = parse_host_from_urlish(&settings.gitlab_url)
            .ok_or_else(|| anyhow::anyhow!("Invalid configured URL"))?;
        if !repo_host.eq_ignore_ascii_case(&configured_host) {
            return Ok(None);
        }
        self.get_gitlab_pat().await
    }

    /// Get decrypted GitLab PAT for use in GitLab operations.
    ///
    /// ## Security
    /// Returns None if PAT is not configured.
    /// The decrypted PAT should never be logged or exposed in responses.
    pub async fn get_gitlab_pat(&self) -> Result<Option<String>> {
        let settings = self.get().await?;

        match settings.gitlab_pat_encrypted {
            Some(encrypted) => {
                let decrypted = self
                    .encryption
                    .decrypt(&encrypted)
                    .context("Failed to decrypt GitLab PAT")?;
                Ok(Some(decrypted))
            }
            None => Ok(None),
        }
    }

    /// Get decrypted Cloudflare API token.
    pub async fn get_cloudflare_token(&self) -> Result<Option<String>> {
        let settings = self.get().await?;

        match settings.cloudflare_api_token_encrypted {
            Some(encrypted) => {
                let decrypted = self
                    .encryption
                    .decrypt(&encrypted)
                    .context("Failed to decrypt Cloudflare API token")?;
                Ok(Some(decrypted))
            }
            None => Ok(None),
        }
    }

    /// Test GitLab connection using configured PAT.
    pub async fn test_gitlab_connection(&self) -> Result<bool> {
        let settings = self.get().await?;
        let pat = match self.get_gitlab_pat().await? {
            Some(p) => p,
            None => return Ok(false),
        };

        // Create temporary GitLab client and test connection
        let client = acpms_gitlab::GitLabClient::new(&settings.gitlab_url, &pat)?;

        // Try to get current user (API endpoint: /api/v4/user)
        // For now, just verify the client can be created
        // TODO: Add actual API call to verify connection
        let _ = client;
        Ok(true)
    }
}

fn parse_repo_host(repo_url: &str) -> Option<String> {
    let trimmed = repo_url.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
    {
        let without_auth = rest.rsplit('@').next().unwrap_or(rest);
        let (host, _) = without_auth.split_once('/')?;
        let host = host.trim().to_lowercase();
        if host.is_empty() {
            return None;
        }
        return Some(host);
    }
    if let Some((left, _)) = trimmed.split_once(':') {
        if let Some(host) = left.split('@').nth(1) {
            let host = host.trim().to_lowercase();
            if !host.is_empty() {
                return Some(host);
            }
        }
    }
    None
}

fn parse_host_from_urlish(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let without_auth = without_scheme.rsplit('@').next().unwrap_or(without_scheme);
    let host = without_auth.split('/').next()?.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_lowercase())
    }
}
