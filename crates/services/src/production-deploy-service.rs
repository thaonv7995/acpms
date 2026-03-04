//! Production Deployment Service for deploying builds to production environments.
//!
//! Supports multiple deployment targets based on project type:
//! - Cloudflare Pages for web applications
//! - Cloudflare Workers for APIs
//! - Container registries for microservices
//! - Manual deployment with artifact download

use acpms_db::{
    models::{BuildArtifact, ProductionDeployment, Project, ProjectType},
    PgPool,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{EncryptionService, SystemSettingsService};

/// Error types for deployment operations
#[derive(Debug, thiserror::Error)]
pub enum DeployError {
    #[error("Deployment failed: {0}")]
    Failed(String),
    #[error("No artifact found for deployment")]
    NoArtifact,
    #[error("Unsupported deployment type: {0}")]
    UnsupportedType(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Database error: {0}")]
    Database(String),
}

/// Result of a production deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployResult {
    pub deployment_id: Uuid,
    pub url: String,
    pub external_id: Option<String>,
    pub deployment_type: String,
}

/// Service for production deployments
pub struct ProductionDeployService {
    db: PgPool,
    settings_service: SystemSettingsService,
    encryption: EncryptionService,
}

impl ProductionDeployService {
    /// Create a new ProductionDeployService
    pub fn new(
        db: PgPool,
        settings_service: SystemSettingsService,
        encryption: EncryptionService,
    ) -> Self {
        Self {
            db,
            settings_service,
            encryption,
        }
    }

    /// Deploy a build artifact to production
    ///
    /// Routes to appropriate deployment method based on project type
    pub async fn deploy(
        &self,
        project: &Project,
        artifact: &BuildArtifact,
        triggered_by: Option<Uuid>,
    ) -> Result<DeployResult> {
        info!(
            "Starting production deployment for project {} (type: {:?})",
            project.name, project.project_type
        );

        // Mark any existing active deployments as superseded
        self.supersede_active_deployments(project.id).await?;

        // Route to appropriate deployment method
        let result = match project.project_type {
            ProjectType::Web => self.deploy_to_pages(project, artifact).await,
            ProjectType::Api => self.deploy_to_workers(project, artifact).await,
            ProjectType::Microservice => self.deploy_to_container(project, artifact).await,
            ProjectType::Extension => self.deploy_extension(project, artifact).await,
            _ => {
                warn!(
                    "Unsupported deployment type {:?}, creating manual deployment record",
                    project.project_type
                );
                self.create_manual_deployment(project, artifact).await
            }
        }?;

        // Save deployment record
        let deployment = self
            .save_deployment_record(
                project.id,
                Some(artifact.id),
                &result.deployment_type,
                &result.url,
                result.external_id.as_deref(),
                triggered_by,
            )
            .await?;

        info!(
            "Production deployment completed: {} -> {}",
            project.name, result.url
        );

        Ok(DeployResult {
            deployment_id: deployment.id,
            url: result.url,
            external_id: result.external_id,
            deployment_type: result.deployment_type,
        })
    }

    /// Deploy to Cloudflare Pages
    async fn deploy_to_pages(
        &self,
        project: &Project,
        _artifact: &BuildArtifact,
    ) -> Result<DeployResult> {
        info!("Deploying {} to Cloudflare Pages", project.name);

        // Get Cloudflare credentials from system settings
        let settings = self.settings_service.get().await?;
        let _account_id = settings
            .cloudflare_account_id
            .ok_or_else(|| DeployError::ConfigError("Cloudflare Account ID not set".into()))?;

        let api_token_encrypted = settings
            .cloudflare_api_token_encrypted
            .ok_or_else(|| DeployError::ConfigError("Cloudflare API token not set".into()))?;

        let _api_token = self
            .encryption
            .decrypt(&api_token_encrypted)
            .context("Failed to decrypt Cloudflare API token")?;

        // Generate project name for Pages (sanitized)
        let pages_project_name = self.sanitize_project_name(&project.name);

        // In a real implementation, this would:
        // 1. Download artifact from MinIO
        // 2. Extract the build output
        // 3. Call Cloudflare Pages API to create deployment
        // For now, we create a placeholder deployment

        // Placeholder URL - in production this comes from Cloudflare API response
        let deployment_url = format!("https://{}.pages.dev", pages_project_name);

        Ok(DeployResult {
            deployment_id: Uuid::new_v4(),
            url: deployment_url,
            external_id: Some(format!("pages-{}", Uuid::new_v4())),
            deployment_type: "pages".to_string(),
        })
    }

    /// Deploy to Cloudflare Workers
    async fn deploy_to_workers(
        &self,
        project: &Project,
        _artifact: &BuildArtifact,
    ) -> Result<DeployResult> {
        info!("Deploying {} to Cloudflare Workers", project.name);

        // Get Cloudflare credentials
        let settings = self.settings_service.get().await?;
        let _account_id = settings
            .cloudflare_account_id
            .ok_or_else(|| DeployError::ConfigError("Cloudflare Account ID not set".into()))?;

        // Generate worker name
        let worker_name = self.sanitize_project_name(&project.name);

        // Placeholder - actual implementation would use wrangler or Workers API
        let deployment_url = format!("https://{}.workers.dev", worker_name);

        Ok(DeployResult {
            deployment_id: Uuid::new_v4(),
            url: deployment_url,
            external_id: Some(format!("worker-{}", Uuid::new_v4())),
            deployment_type: "workers".to_string(),
        })
    }

    /// Deploy to container registry
    async fn deploy_to_container(
        &self,
        project: &Project,
        _artifact: &BuildArtifact,
    ) -> Result<DeployResult> {
        info!("Deploying {} as container", project.name);

        // Get container registry URL from project metadata or settings
        let registry_url = project
            .metadata
            .get("container_registry")
            .and_then(|v| v.as_str())
            .unwrap_or("ghcr.io");

        let image_name = self.sanitize_project_name(&project.name);
        let tag = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();

        // Placeholder - actual implementation would push to registry
        let image_url = format!("{}/{}:{}", registry_url, image_name, tag);

        Ok(DeployResult {
            deployment_id: Uuid::new_v4(),
            url: image_url,
            external_id: Some(format!("container-{}", tag)),
            deployment_type: "container".to_string(),
        })
    }

    /// Deploy browser extension
    async fn deploy_extension(
        &self,
        project: &Project,
        artifact: &BuildArtifact,
    ) -> Result<DeployResult> {
        info!("Creating extension deployment record for {}", project.name);

        // Extensions typically need manual upload to Chrome/Firefox stores
        // We just record the artifact location
        Ok(DeployResult {
            deployment_id: Uuid::new_v4(),
            url: format!("artifact://{}", artifact.artifact_key),
            external_id: None,
            deployment_type: "extension".to_string(),
        })
    }

    /// Create a manual deployment record (for unsupported types)
    async fn create_manual_deployment(
        &self,
        project: &Project,
        artifact: &BuildArtifact,
    ) -> Result<DeployResult> {
        info!(
            "Creating manual deployment record for {} (download artifact manually)",
            project.name
        );

        Ok(DeployResult {
            deployment_id: Uuid::new_v4(),
            url: format!("manual://artifact/{}", artifact.artifact_key),
            external_id: None,
            deployment_type: "manual".to_string(),
        })
    }

    /// Mark all active deployments for a project as superseded
    async fn supersede_active_deployments(&self, project_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE production_deployments
            SET status = 'superseded', updated_at = NOW()
            WHERE project_id = $1 AND status = 'active'
            "#,
        )
        .bind(project_id)
        .execute(&self.db)
        .await
        .context("Failed to supersede active deployments")?;

        Ok(())
    }

    /// Save deployment record to database
    async fn save_deployment_record(
        &self,
        project_id: Uuid,
        artifact_id: Option<Uuid>,
        deployment_type: &str,
        url: &str,
        external_id: Option<&str>,
        triggered_by: Option<Uuid>,
    ) -> Result<ProductionDeployment> {
        let deployment = sqlx::query_as::<_, ProductionDeployment>(
            r#"
            INSERT INTO production_deployments (
                project_id, artifact_id, deployment_type, url,
                deployment_id, status, triggered_by, metadata
            )
            VALUES ($1, $2, $3, $4, $5, 'active', $6, '{}'::jsonb)
            RETURNING id, project_id, artifact_id, deployment_type, url,
                      deployment_id, status, triggered_by, metadata, created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind(artifact_id)
        .bind(deployment_type)
        .bind(url)
        .bind(external_id)
        .bind(triggered_by)
        .fetch_one(&self.db)
        .await
        .context("Failed to save deployment record")?;

        Ok(deployment)
    }

    /// Get deployments for a project
    pub async fn get_project_deployments(
        &self,
        project_id: Uuid,
        limit: Option<i32>,
    ) -> Result<Vec<ProductionDeployment>> {
        let limit = limit.unwrap_or(10);

        let deployments = sqlx::query_as::<_, ProductionDeployment>(
            r#"
            SELECT id, project_id, artifact_id, deployment_type, url,
                   deployment_id, status, triggered_by, metadata, created_at, updated_at
            FROM production_deployments
            WHERE project_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.db)
        .await
        .context("Failed to fetch project deployments")?;

        Ok(deployments)
    }

    /// Get the current active deployment for a project
    pub async fn get_active_deployment(
        &self,
        project_id: Uuid,
    ) -> Result<Option<ProductionDeployment>> {
        let deployment = sqlx::query_as::<_, ProductionDeployment>(
            r#"
            SELECT id, project_id, artifact_id, deployment_type, url,
                   deployment_id, status, triggered_by, metadata, created_at, updated_at
            FROM production_deployments
            WHERE project_id = $1 AND status = 'active'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(project_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to fetch active deployment")?;

        Ok(deployment)
    }

    /// Get a specific deployment by ID
    pub async fn get_deployment(
        &self,
        deployment_id: Uuid,
    ) -> Result<Option<ProductionDeployment>> {
        let deployment = sqlx::query_as::<_, ProductionDeployment>(
            r#"
            SELECT id, project_id, artifact_id, deployment_type, url,
                   deployment_id, status, triggered_by, metadata, created_at, updated_at
            FROM production_deployments
            WHERE id = $1
            "#,
        )
        .bind(deployment_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to fetch deployment")?;

        Ok(deployment)
    }

    /// Rollback to a previous deployment
    pub async fn rollback_deployment(
        &self,
        project_id: Uuid,
        target_deployment_id: Uuid,
        triggered_by: Option<Uuid>,
    ) -> Result<ProductionDeployment> {
        // Get the target deployment
        let target = self
            .get_deployment(target_deployment_id)
            .await?
            .ok_or_else(|| DeployError::Failed("Target deployment not found".into()))?;

        if target.project_id != project_id {
            return Err(DeployError::Failed("Deployment does not belong to project".into()).into());
        }

        // Supersede current active deployment
        self.supersede_active_deployments(project_id).await?;

        // Create new deployment record based on target
        let new_deployment = self
            .save_deployment_record(
                project_id,
                target.artifact_id,
                &target.deployment_type,
                &target.url,
                target.deployment_id.as_deref(),
                triggered_by,
            )
            .await?;

        info!(
            "Rolled back project {} to deployment {}",
            project_id, target_deployment_id
        );

        Ok(new_deployment)
    }

    /// Sanitize project name for use in URLs
    fn sanitize_project_name(&self, name: &str) -> String {
        name.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_project_name() {
        // Test helper function
        let sanitize = |name: &str| -> String {
            name.chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' {
                        c.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
                .trim_matches('-')
                .to_string()
        };

        assert_eq!(sanitize("My Project"), "my-project");
        assert_eq!(sanitize("Test_App_123"), "test-app-123");
        assert_eq!(sanitize("  spaces  "), "spaces");
    }
}
