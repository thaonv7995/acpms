//! Prepares deploy context (SSH key, config) in worktree for Deploy tasks.
//! Agent uses these to SSH directly and deploy—no API call.

use acpms_db::models::DeploymentEnvironment;
use acpms_executors::DeployContextPreparer;
use acpms_services::EncryptionService;
use async_trait::async_trait;
use serde::Serialize;
use sqlx::PgPool;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Serialize)]
struct DeployConfig {
    host: String,
    port: u16,
    username: String,
    deploy_path: String,
}

fn normalize_multiline_ssh_secret(secret: &str) -> Option<String> {
    let normalized = secret.replace("\r\n", "\n");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(format!("{}\n", trimmed))
}

pub struct ServerDeployContextPreparer {
    db: PgPool,
    encryption: Arc<EncryptionService>,
}

impl ServerDeployContextPreparer {
    pub fn new(db: PgPool, encryption: Arc<EncryptionService>) -> Self {
        Self { db, encryption }
    }

    async fn load_decrypted_private_key(
        &self,
        environment_id: Uuid,
    ) -> anyhow::Result<Option<String>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            ciphertext: String,
        }

        let rows = sqlx::query_as::<_, Row>(
            r#"
            SELECT ciphertext
            FROM deployment_environment_secrets
            WHERE environment_id = $1 AND secret_type = 'ssh_private_key'
            "#,
        )
        .bind(environment_id)
        .fetch_all(&self.db)
        .await?;

        for row in rows {
            let decrypted = self
                .encryption
                .decrypt(&row.ciphertext)
                .map_err(|e| anyhow::anyhow!("Failed to decrypt SSH key: {}", e))?;
            if let Some(v) = normalize_multiline_ssh_secret(&decrypted) {
                return Ok(Some(v));
            }
        }
        Ok(None)
    }

    fn extract_ssh_config(
        target_config: &serde_json::Value,
        deploy_path: &str,
    ) -> anyhow::Result<DeployConfig> {
        let obj = target_config
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("target_config must be an object"))?;

        let host = obj
            .get("host")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        if host.is_empty() {
            anyhow::bail!("target_config.host is required");
        }

        let username = obj
            .get("username")
            .or_else(|| obj.get("user"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        if username.is_empty() {
            anyhow::bail!("target_config.username is required");
        }

        let port = obj.get("port").and_then(|v| v.as_u64()).unwrap_or(22);
        if port == 0 || port > 65535 {
            anyhow::bail!("target_config.port must be 1-65535");
        }

        Ok(DeployConfig {
            host,
            port: port as u16,
            username,
            deploy_path: deploy_path.to_string(),
        })
    }
}

#[async_trait]
impl DeployContextPreparer for ServerDeployContextPreparer {
    async fn prepare(&self, attempt_id: Uuid, worktree_path: &Path) -> anyhow::Result<()> {
        let project_id: Uuid = sqlx::query_scalar(
            "SELECT t.project_id FROM task_attempts ta JOIN tasks t ON t.id = ta.task_id WHERE ta.id = $1",
        )
        .bind(attempt_id)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Attempt {} not found", attempt_id))?;

        let env: Option<DeploymentEnvironment> = sqlx::query_as(
            r#"
            SELECT *
            FROM deployment_environments
            WHERE project_id = $1
              AND target_type = 'ssh_remote'
              AND is_default = true
              AND is_enabled = true
            ORDER BY created_at ASC
            LIMIT 1
            "#,
        )
        .bind(project_id)
        .fetch_optional(&self.db)
        .await?;

        let env = env
            .ok_or_else(|| anyhow::anyhow!("No default SSH deployment environment for project"))?;

        let private_key = self
            .load_decrypted_private_key(env.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No SSH private key configured for environment"))?;

        let config = Self::extract_ssh_config(&env.target_config, &env.deploy_path)?;

        let deploy_dir = worktree_path.join(".acpms").join("deploy");
        tokio::fs::create_dir_all(&deploy_dir).await?;

        let key_path = deploy_dir.join("ssh_key");
        tokio::fs::write(&key_path, &private_key).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&key_path).await?.permissions();
            perms.set_mode(0o600);
            tokio::fs::set_permissions(&key_path, perms).await?;
        }

        let config_path = deploy_dir.join("config.json");
        let config_json = serde_json::to_string_pretty(&config)?;
        tokio::fs::write(&config_path, config_json).await?;

        tracing::info!(
            attempt_id = %attempt_id,
            deploy_dir = %deploy_dir.display(),
            "Deploy context prepared for agent"
        );

        Ok(())
    }
}
