use acpms_db::models::{
    CloudflareTunnel, PreviewDeployment, PreviewInfo, Project, ProjectType, SystemSettings,
    TunnelStatus,
};
use acpms_deployment::CloudflareClient;
use acpms_services::{EncryptionService, SystemSettingsService};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sqlx::PgPool;
use std::{
    collections::BTreeMap,
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::Command,
};
use tokio::time::{sleep, Duration as TokioDuration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Manages preview environment lifecycle
pub struct PreviewManager {
    #[allow(dead_code)]
    cloudflare: CloudflareClient,
    encryption: EncryptionService,
    settings_service: SystemSettingsService,
    db: PgPool,
    preview_ttl_days: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

const LOCAL_PREVIEW_TUNNEL_PREFIX: &str = "local-preview-";

enum PreviewExposureMode {
    Cloudflare {
        credentials_path: PathBuf,
        tunnel_id: String,
    },
    Local {
        host_port: u16,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewRuntimeStatus {
    pub attempt_id: Uuid,
    pub runtime_enabled: bool,
    pub worktree_path: Option<String>,
    pub compose_file_exists: bool,
    pub docker_project_name: Option<String>,
    pub compose_file_path: Option<String>,
    pub running_services: Vec<String>,
    pub runtime_ready: bool,
    pub last_error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewRuntimeLogs {
    pub attempt_id: Uuid,
    pub runtime_enabled: bool,
    pub docker_project_name: Option<String>,
    pub compose_file_path: Option<String>,
    pub tail: u32,
    pub logs: String,
    pub message: Option<String>,
}

impl PreviewManager {
    /// Create a new preview manager
    pub fn new(
        cloudflare: CloudflareClient,
        encryption: EncryptionService,
        settings_service: SystemSettingsService,
        db: PgPool,
        preview_ttl_days: Option<i64>,
    ) -> Self {
        Self {
            cloudflare,
            encryption,
            settings_service,
            db,
            preview_ttl_days: preview_ttl_days.unwrap_or(7),
        }
    }

    /// Initializes a CloudflareClient using current system settings.
    async fn get_client(&self) -> Result<CloudflareClient> {
        let settings = self
            .settings_service
            .get()
            .await
            .context("Failed to retrieve system settings for Cloudflare client")?;

        let account_id = settings
            .cloudflare_account_id
            .context("Cloudflare Account ID not set")?;

        let token_encrypted = settings
            .cloudflare_api_token_encrypted
            .context("Cloudflare API token not set")?;

        let api_token = self
            .encryption
            .decrypt(&token_encrypted)
            .context("Failed to decrypt Cloudflare API token")?;

        CloudflareClient::new(api_token, account_id)
    }

    /// Whether Docker preview runtime is enabled via environment flag.
    pub fn runtime_enabled(&self) -> bool {
        is_docker_runtime_enabled()
    }

    /// Create a preview environment for a task attempt
    pub async fn create_preview(&self, attempt_id: Uuid, task_name: &str) -> Result<PreviewInfo> {
        info!("Creating preview for attempt {}", attempt_id);

        if let Some(existing) = self.get_preview(attempt_id).await? {
            info!(
                "Preview already exists for attempt {}, returning existing URL: {}",
                attempt_id, existing.preview_url
            );
            return Ok(existing);
        }

        if !self.cloudflare_preview_configured().await? {
            info!(
                "Cloudflare preview config missing; falling back to local preview URL for attempt {}",
                attempt_id
            );
            return self.create_local_preview(attempt_id, task_name).await;
        }

        // Generate tunnel name (sanitized, with timestamp)
        let tunnel_name = self.generate_tunnel_name(task_name, attempt_id);

        // Initialize Cloudflare client
        let cloudflare = self
            .get_client()
            .await
            .context("Failed to initialize Cloudflare client")?;

        // Create tunnel via Cloudflare API
        let credentials = cloudflare
            .create_tunnel(&tunnel_name)
            .await
            .context("Failed to create Cloudflare tunnel")?;

        debug!("Created tunnel with ID: {}", credentials.tunnel_id);

        // Fetch settings for DNS configuration
        let settings = self.settings_service.get().await.ok();
        let zone_id = settings
            .as_ref()
            .and_then(|s| s.cloudflare_zone_id.clone())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let base_domain = settings
            .as_ref()
            .and_then(|s| s.cloudflare_base_domain.clone())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        // Generate preview URL and capture DNS record ID
        let (preview_url, dns_record_id) =
            if let (Some(zone_id_value), Some(base_domain_value)) = (&zone_id, &base_domain) {
                // New Flow: Create DNS record
                let subdomain = format!("task-{}", attempt_id);
                let full_domain = format!("{}.{}", subdomain, base_domain_value);
                let target = format!("{}.cfargotunnel.com", credentials.tunnel_id);

                info!("Creating DNS record for {}", full_domain);
                let record_id_result = cloudflare
                    .create_dns_record(zone_id_value, &subdomain, &target, "CNAME", true)
                    .await;
                let record_id = match record_id_result {
                    Ok(record_id) => record_id,
                    Err(error) => {
                        self.rollback_cloudflare_resources(
                            &cloudflare,
                            Some(zone_id_value.as_str()),
                            None,
                            &credentials.tunnel_id,
                            "DNS creation failed in create_preview",
                        )
                        .await;
                        return Err(error).context("Failed to create DNS record");
                    }
                };

                (format!("https://{}", full_domain), Some(record_id))
            } else {
                // Fallback: Use default Cloudflare URL
                warn!("Missing Zone ID or Base Domain settings. Using default Cloudflare URL.");
                (
                    cloudflare.generate_preview_url(&credentials.tunnel_id),
                    None,
                )
            };

        // Encrypt credentials
        let credentials_encrypted = match self.encryption.encrypt(&credentials.credentials_file) {
            Ok(value) => value,
            Err(error) => {
                self.rollback_cloudflare_resources(
                    &cloudflare,
                    zone_id.as_deref(),
                    dns_record_id.as_deref(),
                    &credentials.tunnel_id,
                    "Credential encryption failed in create_preview",
                )
                .await;
                return Err(error).context("Failed to encrypt tunnel credentials");
            }
        };

        // Calculate expiration time
        let expires_at = Utc::now() + Duration::days(self.preview_ttl_days);

        // Store in database
        let tunnel_result = sqlx::query_as::<_, CloudflareTunnel>(
            r#"
            INSERT INTO cloudflare_tunnels (
                attempt_id, tunnel_id, tunnel_name, credentials_encrypted,
                preview_url, status, expires_at, dns_record_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
        )
        .bind(attempt_id)
        .bind(&credentials.tunnel_id)
        .bind(&tunnel_name)
        .bind(&credentials_encrypted)
        .bind(&preview_url)
        .bind(TunnelStatus::Active)
        .bind(expires_at)
        .bind(&dns_record_id)
        .fetch_one(&self.db)
        .await;
        let tunnel = match tunnel_result {
            Ok(tunnel) => tunnel,
            Err(error) => {
                self.rollback_cloudflare_resources(
                    &cloudflare,
                    zone_id.as_deref(),
                    dns_record_id.as_deref(),
                    &credentials.tunnel_id,
                    "Database insert failed in create_preview",
                )
                .await;
                return Err(error).context("Failed to insert tunnel into database");
            }
        };

        info!(
            "Preview created successfully: {} (expires: {})",
            preview_url, expires_at
        );

        Ok(PreviewInfo {
            id: tunnel.id,
            attempt_id: tunnel.attempt_id,
            preview_url: tunnel.preview_url,
            status: tunnel.status,
            created_at: tunnel.created_at,
            expires_at: tunnel.expires_at,
        })
    }

    async fn cloudflare_preview_configured(&self) -> Result<bool> {
        let settings = self
            .settings_service
            .get()
            .await
            .context("Failed to load system settings for preview mode decision")?;
        Ok(has_complete_cloudflare_config(&settings))
    }

    pub async fn create_local_preview(
        &self,
        attempt_id: Uuid,
        task_name: &str,
    ) -> Result<PreviewInfo> {
        if let Some(existing) = self.get_preview(attempt_id).await? {
            return Ok(existing);
        }

        let tunnel = self
            .create_local_preview_tunnel(attempt_id, task_name, self.preview_ttl_days)
            .await?;

        Ok(PreviewInfo {
            id: tunnel.id,
            attempt_id: tunnel.attempt_id,
            preview_url: tunnel.preview_url,
            status: tunnel.status,
            created_at: tunnel.created_at,
            expires_at: tunnel.expires_at,
        })
    }

    async fn create_local_preview_tunnel(
        &self,
        attempt_id: Uuid,
        task_name: &str,
        ttl_days: i64,
    ) -> Result<CloudflareTunnel> {
        if let Some(existing_tunnel) = sqlx::query_as::<_, CloudflareTunnel>(
            r#"
            SELECT * FROM cloudflare_tunnels
            WHERE attempt_id = $1 AND deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to query existing preview metadata before local preview creation")?
        {
            return Ok(existing_tunnel);
        }

        let tunnel_name = format!("local-{}", self.generate_tunnel_name(task_name, attempt_id));
        let tunnel_id = format!("{}{}", LOCAL_PREVIEW_TUNNEL_PREFIX, Uuid::new_v4());
        let preview_url = local_preview_url(allocate_preview_local_public_port(attempt_id)?);
        let credentials_encrypted = self
            .encryption
            .encrypt("{}")
            .context("Failed to encrypt local preview placeholder credentials")?;
        let expires_at = Utc::now() + Duration::days(ttl_days);

        let tunnel = sqlx::query_as::<_, CloudflareTunnel>(
            r#"
            INSERT INTO cloudflare_tunnels (
                attempt_id, tunnel_id, tunnel_name, credentials_encrypted,
                preview_url, status, expires_at, dns_record_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, NULL)
            RETURNING *
            "#,
        )
        .bind(attempt_id)
        .bind(&tunnel_id)
        .bind(&tunnel_name)
        .bind(&credentials_encrypted)
        .bind(&preview_url)
        .bind(TunnelStatus::Active)
        .bind(expires_at)
        .fetch_one(&self.db)
        .await
        .with_context(|| {
            format!(
                "Failed to insert local preview metadata for attempt {}",
                attempt_id
            )
        })?;

        info!(
            "Local preview metadata created for attempt {} at {}",
            attempt_id, preview_url
        );

        Ok(tunnel)
    }

    /// Mark preview as deleted in DB (fast). Returns tunnel info for resource cleanup.
    /// Caller should spawn cleanup_preview_resources in background.
    pub async fn mark_preview_deleted(
        &self,
        attempt_id: Uuid,
    ) -> Result<Option<(String, Option<String>)>> {
        let tunnel = sqlx::query_as::<_, CloudflareTunnel>(
            r#"
            SELECT * FROM cloudflare_tunnels
            WHERE attempt_id = $1 AND deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to query tunnel from database")?;

        let Some(tunnel) = tunnel else {
            warn!("No active tunnel found for attempt {}", attempt_id);
            return Ok(None);
        };

        sqlx::query(
            r#"
            UPDATE cloudflare_tunnels
            SET status = $1, deleted_at = NOW(), stopped_at = COALESCE(stopped_at, NOW())
            WHERE id = $2
            "#,
        )
        .bind(TunnelStatus::Deleted)
        .bind(tunnel.id)
        .execute(&self.db)
        .await
        .context("Failed to update tunnel status")?;

        info!("Preview marked deleted for attempt {}", attempt_id);
        Ok(Some((tunnel.tunnel_id, tunnel.dns_record_id)))
    }

    /// Clean up Docker + Cloudflare resources (best effort). Run in background after mark_preview_deleted.
    pub async fn cleanup_preview_resources(
        &self,
        attempt_id: Uuid,
        tunnel_id: String,
        dns_record_id: Option<String>,
    ) {
        if let Err(e) = self.stop_preview_runtime(attempt_id).await {
            warn!(
                "Failed to stop preview runtime for attempt {} during cleanup: {}",
                attempt_id, e
            );
        }

        if is_local_preview_tunnel_id(&tunnel_id) {
            debug!(
                "Skip Cloudflare cleanup for local preview tunnel {} (attempt {})",
                tunnel_id, attempt_id
            );
            return;
        }

        let cloudflare = match self.get_client().await {
            Ok(c) => Some(c),
            Err(e) => {
                error!("Failed to initialize Cloudflare client for cleanup: {}", e);
                None
            }
        };

        if let Some(cf) = &cloudflare {
            if let Err(e) = cf.delete_tunnel(&tunnel_id).await {
                error!(
                    "Failed to delete tunnel {} from Cloudflare: {}",
                    tunnel_id, e
                );
            }
        }

        if let Some(record_id) = dns_record_id {
            if let Ok(settings) = self.settings_service.get().await {
                if let Some(zone_id) = settings.cloudflare_zone_id {
                    if let Some(cf) = &cloudflare {
                        info!("Deleting DNS record {}", record_id);
                        if let Err(e) = cf.delete_dns_record(&zone_id, &record_id).await {
                            error!("Failed to delete DNS record {}: {}", record_id, e);
                        }
                    }
                }
            }
        }
    }

    /// Cleanup a preview environment (full inline - use mark_preview_deleted + cleanup_preview_resources for non-blocking)
    pub async fn cleanup_preview(&self, attempt_id: Uuid) -> Result<()> {
        let Some((tunnel_id, dns_record_id)) = self.mark_preview_deleted(attempt_id).await? else {
            return Ok(());
        };
        self.cleanup_preview_resources(attempt_id, tunnel_id, dns_record_id)
            .await;
        Ok(())
    }

    /// List all active previews
    pub async fn list_active_previews(&self) -> Result<Vec<PreviewInfo>> {
        let previews = sqlx::query_as::<_, CloudflareTunnel>(
            r#"
            SELECT * FROM cloudflare_tunnels
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.db)
        .await
        .context("Failed to query active previews")?;

        Ok(previews
            .into_iter()
            .map(|t| PreviewInfo {
                id: t.id,
                attempt_id: t.attempt_id,
                preview_url: t.preview_url,
                status: t.status,
                created_at: t.created_at,
                expires_at: t.expires_at,
            })
            .collect())
    }

    /// Get preview info for a specific attempt
    pub async fn get_preview(&self, attempt_id: Uuid) -> Result<Option<PreviewInfo>> {
        let tunnel = sqlx::query_as::<_, CloudflareTunnel>(
            r#"
            SELECT * FROM cloudflare_tunnels
            WHERE attempt_id = $1 AND deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to query preview from database")?;

        Ok(tunnel.map(|t| PreviewInfo {
            id: t.id,
            attempt_id: t.attempt_id,
            preview_url: t.preview_url,
            status: t.status,
            created_at: t.created_at,
            expires_at: t.expires_at,
        }))
    }

    /// Create a preview environment if enabled in project settings
    ///
    /// This method checks project settings before creating a preview:
    /// - If preview_enabled is false, returns None
    /// - Uses preview_ttl_days from settings for expiration
    /// - Stores the preview deployment record in database
    pub async fn create_preview_if_enabled(
        &self,
        project: &Project,
        attempt_id: Uuid,
        task_name: &str,
        artifact_id: Option<Uuid>,
        preview_target: Option<&str>,
    ) -> Result<Option<PreviewInfo>> {
        // Check if preview is enabled (auto_deploy = preview when task completes; preview_enabled is legacy alias)
        let preview_wanted = project.settings.auto_deploy || project.settings.preview_enabled;
        if !preview_wanted {
            info!(
                "Preview disabled for project {} (auto_deploy and preview_enabled both off), skipping creation",
                project.name
            );
            return Ok(None);
        }

        // Use project-specific TTL or fall back to default
        let ttl_days = project.settings.preview_ttl_days as i64;

        info!(
            "Creating preview for project {} (TTL: {} days)",
            project.name, ttl_days
        );

        if !self.cloudflare_preview_configured().await? {
            info!(
                "Cloudflare preview config missing; using local preview URL for project {} attempt {}",
                project.name, attempt_id
            );

            let tunnel = self
                .create_local_preview_tunnel(attempt_id, task_name, ttl_days)
                .await?;
            let preview_url = tunnel.preview_url.clone();

            let preview_deployment_result = sqlx::query_as::<_, PreviewDeployment>(
                r#"
                INSERT INTO preview_deployments (
                    attempt_id, project_id, artifact_id, url, tunnel_id,
                    dns_record_id, status, expires_at, metadata
                )
                VALUES ($1, $2, $3, $4, $5, $6, 'active', $7, $8)
                RETURNING *
                "#,
            )
            .bind(attempt_id)
            .bind(project.id)
            .bind(artifact_id)
            .bind(&preview_url)
            .bind(&tunnel.tunnel_id)
            .bind(Option::<String>::None)
            .bind(tunnel.expires_at)
            .bind(serde_json::json!({
                "preview_target": preview_target,
                "target_source": if preview_target.is_some() { "agent_output" } else { "unspecified" },
                "delivery_mode": "local_runtime",
            }))
            .fetch_one(&self.db)
            .await;

            if let Err(error) = preview_deployment_result {
                let _ = sqlx::query(
                    r#"
                    UPDATE cloudflare_tunnels
                    SET status = $1, deleted_at = NOW(), stopped_at = COALESCE(stopped_at, NOW())
                    WHERE id = $2
                    "#,
                )
                .bind(TunnelStatus::Deleted)
                .bind(tunnel.id)
                .execute(&self.db)
                .await;

                return Err(error).context("Failed to insert local preview deployment record");
            }

            return Ok(Some(PreviewInfo {
                id: tunnel.id,
                attempt_id: tunnel.attempt_id,
                preview_url: tunnel.preview_url,
                status: tunnel.status,
                created_at: tunnel.created_at,
                expires_at: tunnel.expires_at,
            }));
        }

        // Generate tunnel name
        let tunnel_name = self.generate_tunnel_name(task_name, attempt_id);

        // Initialize Cloudflare client
        let cloudflare = self
            .get_client()
            .await
            .context("Failed to initialize Cloudflare client")?;

        // Create tunnel via Cloudflare API
        let credentials = cloudflare
            .create_tunnel(&tunnel_name)
            .await
            .context("Failed to create Cloudflare tunnel")?;

        debug!("Created tunnel with ID: {}", credentials.tunnel_id);

        // Fetch settings for DNS configuration
        let settings = self.settings_service.get().await.ok();
        let zone_id = settings
            .as_ref()
            .and_then(|s| s.cloudflare_zone_id.clone())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let base_domain = settings
            .as_ref()
            .and_then(|s| s.cloudflare_base_domain.clone())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        // Generate preview URL and capture DNS record ID
        let (preview_url, dns_record_id) =
            if let (Some(zone_id_value), Some(base_domain_value)) = (&zone_id, &base_domain) {
                let subdomain = format!("task-{}", &attempt_id.to_string()[..8]);
                let full_domain = format!("{}.{}", subdomain, base_domain_value);
                let target = format!("{}.cfargotunnel.com", credentials.tunnel_id);

                info!("Creating DNS record for {}", full_domain);
                let record_id_result = cloudflare
                    .create_dns_record(zone_id_value, &subdomain, &target, "CNAME", true)
                    .await;
                let record_id = match record_id_result {
                    Ok(record_id) => record_id,
                    Err(error) => {
                        self.rollback_cloudflare_resources(
                            &cloudflare,
                            Some(zone_id_value.as_str()),
                            None,
                            &credentials.tunnel_id,
                            "DNS creation failed in create_preview_if_enabled",
                        )
                        .await;
                        return Err(error).context("Failed to create DNS record");
                    }
                };

                (format!("https://{}", full_domain), Some(record_id))
            } else {
                warn!("Missing Zone ID or Base Domain settings. Using default Cloudflare URL.");
                (
                    cloudflare.generate_preview_url(&credentials.tunnel_id),
                    None,
                )
            };

        // Encrypt credentials
        let credentials_encrypted = match self.encryption.encrypt(&credentials.credentials_file) {
            Ok(value) => value,
            Err(error) => {
                self.rollback_cloudflare_resources(
                    &cloudflare,
                    zone_id.as_deref(),
                    dns_record_id.as_deref(),
                    &credentials.tunnel_id,
                    "Credential encryption failed in create_preview_if_enabled",
                )
                .await;
                return Err(error).context("Failed to encrypt tunnel credentials");
            }
        };

        // Calculate expiration time using project settings
        let expires_at = Utc::now() + Duration::days(ttl_days);

        // Store in cloudflare_tunnels table (existing table)
        let tunnel_result = sqlx::query_as::<_, CloudflareTunnel>(
            r#"
            INSERT INTO cloudflare_tunnels (
                attempt_id, tunnel_id, tunnel_name, credentials_encrypted,
                preview_url, status, expires_at, dns_record_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
        )
        .bind(attempt_id)
        .bind(&credentials.tunnel_id)
        .bind(&tunnel_name)
        .bind(&credentials_encrypted)
        .bind(&preview_url)
        .bind(TunnelStatus::Active)
        .bind(expires_at)
        .bind(&dns_record_id)
        .fetch_one(&self.db)
        .await;
        let tunnel = match tunnel_result {
            Ok(tunnel) => tunnel,
            Err(error) => {
                self.rollback_cloudflare_resources(
                    &cloudflare,
                    zone_id.as_deref(),
                    dns_record_id.as_deref(),
                    &credentials.tunnel_id,
                    "Database insert failed in create_preview_if_enabled",
                )
                .await;
                return Err(error).context("Failed to insert tunnel into database");
            }
        };

        // Also store in preview_deployments table for deployment tracking
        let preview_deployment_result = sqlx::query_as::<_, PreviewDeployment>(
            r#"
            INSERT INTO preview_deployments (
                attempt_id, project_id, artifact_id, url, tunnel_id,
                dns_record_id, status, expires_at, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, 'active', $7, $8)
            RETURNING *
            "#,
        )
        .bind(attempt_id)
        .bind(project.id)
        .bind(artifact_id)
        .bind(&preview_url)
        .bind(&credentials.tunnel_id)
        .bind(&dns_record_id)
        .bind(expires_at)
        .bind(serde_json::json!({
            "preview_target": preview_target,
            "target_source": if preview_target.is_some() { "agent_output" } else { "unspecified" },
        }))
        .fetch_one(&self.db)
        .await;
        if let Err(error) = preview_deployment_result {
            self.rollback_cloudflare_resources(
                &cloudflare,
                zone_id.as_deref(),
                dns_record_id.as_deref(),
                &credentials.tunnel_id,
                "Preview deployment insert failed in create_preview_if_enabled",
            )
            .await;

            let _ = sqlx::query(
                r#"
                UPDATE cloudflare_tunnels
                SET status = $1, deleted_at = NOW(), stopped_at = COALESCE(stopped_at, NOW())
                WHERE id = $2
                "#,
            )
            .bind(TunnelStatus::Deleted)
            .bind(tunnel.id)
            .execute(&self.db)
            .await;

            return Err(error).context("Failed to insert preview deployment record");
        }

        info!(
            "Preview created successfully: {} (expires: {})",
            preview_url, expires_at
        );

        Ok(Some(PreviewInfo {
            id: tunnel.id,
            attempt_id: tunnel.attempt_id,
            preview_url: tunnel.preview_url,
            status: tunnel.status,
            created_at: tunnel.created_at,
            expires_at: tunnel.expires_at,
        }))
    }

    /// Get preview deployment from the preview_deployments table
    pub async fn get_preview_deployment(
        &self,
        attempt_id: Uuid,
    ) -> Result<Option<PreviewDeployment>> {
        let deployment = sqlx::query_as::<_, PreviewDeployment>(
            r#"
            SELECT id, attempt_id, project_id, artifact_id, url, tunnel_id,
                   dns_record_id, status, expires_at, destroyed_at, created_at, updated_at
            FROM preview_deployments
            WHERE attempt_id = $1 AND status = 'active'
            LIMIT 1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to query preview deployment")?;

        Ok(deployment)
    }

    /// List preview deployments for a project
    pub async fn list_project_previews(&self, project_id: Uuid) -> Result<Vec<PreviewDeployment>> {
        let deployments = sqlx::query_as::<_, PreviewDeployment>(
            r#"
            SELECT id, attempt_id, project_id, artifact_id, url, tunnel_id,
                   dns_record_id, status, expires_at, destroyed_at, created_at, updated_at
            FROM preview_deployments
            WHERE project_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.db)
        .await
        .context("Failed to query project preview deployments")?;

        Ok(deployments)
    }

    /// Destroy a preview deployment
    pub async fn destroy_preview(&self, attempt_id: Uuid) -> Result<()> {
        // First cleanup the Cloudflare resources
        self.cleanup_preview(attempt_id).await?;

        // Update the preview_deployments table
        sqlx::query(
            r#"
            UPDATE preview_deployments
            SET status = 'destroyed', destroyed_at = NOW(), updated_at = NOW()
            WHERE attempt_id = $1
            "#,
        )
        .bind(attempt_id)
        .execute(&self.db)
        .await
        .context("Failed to update preview deployment status")?;

        Ok(())
    }

    /// Cleanup expired previews (background job)
    pub async fn cleanup_expired_previews(&self) -> Result<usize> {
        info!("Running cleanup job for expired previews");

        // Find expired tunnels
        let expired_tunnels = sqlx::query_as::<_, CloudflareTunnel>(
            r#"
            SELECT * FROM cloudflare_tunnels
            WHERE deleted_at IS NULL
              AND expires_at < NOW()
            "#,
        )
        .fetch_all(&self.db)
        .await
        .context("Failed to query expired tunnels")?;

        let count = expired_tunnels.len();
        info!("Found {} expired previews to cleanup", count);

        let requires_cloudflare_cleanup = expired_tunnels
            .iter()
            .any(|tunnel| !is_local_preview_tunnel_id(&tunnel.tunnel_id));
        let cloudflare = if requires_cloudflare_cleanup {
            match self.get_client().await {
                Ok(c) => Some(c),
                Err(e) => {
                    error!(
                        "Failed to initialize Cloudflare client for cleanup job: {}",
                        e
                    );
                    // We should probably abort or skip Cloudflare deletion but still clean DB?
                    // For now, let's treat it as None and skip Cloudflare ops.
                    None
                }
            }
        } else {
            None
        };

        for tunnel in expired_tunnels {
            if let Err(e) = self.stop_preview_runtime(tunnel.attempt_id).await {
                warn!(
                    "Failed to stop preview runtime for expired attempt {}: {}",
                    tunnel.attempt_id, e
                );
            }

            if !is_local_preview_tunnel_id(&tunnel.tunnel_id) {
                if let Some(cf) = &cloudflare {
                    // Delete from Cloudflare (Tunnel)
                    match cf.delete_tunnel(&tunnel.tunnel_id).await {
                        Ok(_) => {
                            debug!("Deleted expired tunnel {}", tunnel.tunnel_id);
                        }
                        Err(e) => {
                            error!(
                                "Failed to delete expired tunnel {}: {}",
                                tunnel.tunnel_id, e
                            );
                            // Continue with next tunnel
                            // continue; // Actually we want to try DNS too
                        }
                    }

                    // Delete DNS record if exists
                    if let Some(record_id) = &tunnel.dns_record_id {
                        if let Ok(settings) = self.settings_service.get().await {
                            if let Some(zone_id) = settings.cloudflare_zone_id {
                                if let Err(e) = cf.delete_dns_record(&zone_id, record_id).await {
                                    error!(
                                        "Failed to delete expired DNS record {}: {}",
                                        record_id, e
                                    );
                                } else {
                                    debug!("Deleted expired DNS record {}", record_id);
                                }
                            }
                        }
                    }
                }
            } else {
                debug!(
                    "Skip Cloudflare cleanup for expired local preview tunnel {}",
                    tunnel.tunnel_id
                );
            }

            // Mark as deleted in database
            if let Err(e) = sqlx::query(
                r#"
                UPDATE cloudflare_tunnels
                SET status = $1, deleted_at = NOW(), stopped_at = COALESCE(stopped_at, NOW())
                WHERE id = $2
                "#,
            )
            .bind(TunnelStatus::Deleted)
            .bind(tunnel.id)
            .execute(&self.db)
            .await
            {
                error!("Failed to update tunnel {} status: {}", tunnel.id, e);
            }
        }

        info!("Cleanup job completed: {} previews deleted", count);

        Ok(count)
    }

    /// Start Docker-based runtime for preview (best effort controlled by env flag).
    pub async fn start_preview_runtime(
        &self,
        attempt_id: Uuid,
        project_type: ProjectType,
    ) -> Result<()> {
        if !is_docker_runtime_enabled() {
            debug!(
                "Preview Docker runtime disabled; skip starting runtime for attempt {}",
                attempt_id
            );
            return Ok(());
        }

        let tunnel = sqlx::query_as::<_, CloudflareTunnel>(
            r#"
            SELECT * FROM cloudflare_tunnels
            WHERE attempt_id = $1 AND deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to load tunnel for preview runtime startup")?
        .context("No active tunnel found for runtime startup")?;
        let local_only_exposure = is_local_preview_tunnel_id(&tunnel.tunnel_id);

        let worktree_path = self
            .resolve_attempt_worktree_path(attempt_id)
            .await
            .context("Unable to resolve attempt worktree path for preview runtime")?;

        if !worktree_path.exists() {
            anyhow::bail!(
                "Worktree path does not exist for attempt {}: {}",
                attempt_id,
                worktree_path.display()
            );
        }

        let runtime_dir = worktree_path
            .join(".acpms")
            .join("preview")
            .join(attempt_id.to_string());
        fs::create_dir_all(&runtime_dir).with_context(|| {
            format!(
                "Failed to create preview runtime directory: {}",
                runtime_dir.display()
            )
        })?;

        let container_port = preview_dev_port();
        let dev_command = resolve_preview_dev_command(&worktree_path, project_type, container_port)
            .context("Failed to resolve preview dev command")?;
        let dev_image = resolve_preview_dev_image(&worktree_path, &dev_command);
        let compose_path = runtime_dir.join("docker-compose.preview.yml");
        let exposure = if local_only_exposure {
            let host_port = extract_local_preview_port(&tunnel.preview_url)
                .unwrap_or_else(|| preview_local_public_port(attempt_id));
            PreviewExposureMode::Local {
                host_port,
            }
        } else {
            let credentials_json = self
                .encryption
                .decrypt(&tunnel.credentials_encrypted)
                .context("Failed to decrypt tunnel credentials for runtime startup")?;

            let credentials_path = runtime_dir.join("tunnel-credentials.json");
            fs::write(&credentials_path, credentials_json).with_context(|| {
                format!(
                    "Failed to write tunnel credentials file: {}",
                    credentials_path.display()
                )
            })?;

            PreviewExposureMode::Cloudflare {
                credentials_path,
                tunnel_id: tunnel.tunnel_id.clone(),
            }
        };

        let compose_content = build_compose_content(
            &worktree_path,
            &dev_image,
            &dev_command,
            container_port,
            exposure,
        )
        .context("Failed to build docker compose content")?;
        fs::write(&compose_path, compose_content)
            .with_context(|| format!("Failed to write compose file: {}", compose_path.display()))?;

        let project_name = preview_docker_project_name(attempt_id);
        self.mark_runtime_preparing(attempt_id, &project_name, &compose_path, &worktree_path)
            .await
            .context("Failed to persist runtime metadata before docker startup")?;

        let output_result = Command::new(preview_docker_command())
            .arg("compose")
            .arg("-p")
            .arg(&project_name)
            .arg("-f")
            .arg(&compose_path)
            .arg("up")
            .arg("-d")
            .arg("--remove-orphans")
            .current_dir(&runtime_dir)
            .output();

        let output = match output_result {
            Ok(output) => output,
            Err(error) => {
                let message = format!(
                    "Failed to execute docker compose up for attempt {}: {}",
                    attempt_id, error
                );
                let _ = self.record_runtime_error(attempt_id, &message).await;
                anyhow::bail!(message);
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let message = format!(
                "docker compose up failed for attempt {} (status: {:?})\nstdout: {}\nstderr: {}",
                attempt_id,
                output.status.code(),
                stdout,
                stderr
            );
            let _ = self.record_runtime_error(attempt_id, &message).await;
            anyhow::bail!(message);
        }

        if let Err(error) = self
            .wait_for_runtime_ready(
                attempt_id,
                &runtime_dir,
                &compose_path,
                &project_name,
                !local_only_exposure,
            )
            .await
        {
            let message = format!(
                "Preview runtime did not become ready for attempt {}: {}",
                attempt_id, error
            );
            self.stop_runtime_containers_best_effort(
                attempt_id,
                &runtime_dir,
                &compose_path,
                &project_name,
            );
            let _ = self.record_runtime_error(attempt_id, &message).await;
            anyhow::bail!(message);
        }

        self.mark_runtime_started(attempt_id)
            .await
            .context("Failed to mark runtime as started")?;

        info!(
            "Preview runtime started for attempt {} at worktree {}",
            attempt_id,
            worktree_path.display()
        );
        Ok(())
    }

    /// Stop Docker-based preview runtime (best effort).
    pub async fn stop_preview_runtime(&self, attempt_id: Uuid) -> Result<()> {
        if !is_docker_runtime_enabled() {
            return Ok(());
        }

        let worktree_path = match self.resolve_attempt_worktree_path(attempt_id).await {
            Ok(path) => path,
            Err(e) => {
                warn!(
                    "Skip runtime stop for attempt {}: cannot resolve worktree path ({})",
                    attempt_id, e
                );
                let _ = self
                    .record_runtime_error(
                        attempt_id,
                        &format!(
                            "Skip runtime stop: cannot resolve worktree path for attempt {} ({})",
                            attempt_id, e
                        ),
                    )
                    .await;
                return Ok(());
            }
        };
        let runtime_dir = worktree_path
            .join(".acpms")
            .join("preview")
            .join(attempt_id.to_string());
        let compose_path = runtime_dir.join("docker-compose.preview.yml");
        if !compose_path.exists() {
            debug!(
                "No compose file found for attempt {} at {}; skip docker down",
                attempt_id,
                compose_path.display()
            );
            return Ok(());
        }

        let project_name = preview_docker_project_name(attempt_id);
        let output = Command::new(preview_docker_command())
            .arg("compose")
            .arg("-p")
            .arg(&project_name)
            .arg("-f")
            .arg(&compose_path)
            .arg("down")
            .arg("--remove-orphans")
            .current_dir(&runtime_dir)
            .output()
            .with_context(|| {
                format!(
                    "Failed to execute docker compose down for attempt {}",
                    attempt_id
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let message = format!(
                "docker compose down failed for attempt {} (status: {:?})\nstdout: {}\nstderr: {}",
                attempt_id,
                output.status.code(),
                stdout,
                stderr
            );
            let _ = self.record_runtime_error(attempt_id, &message).await;
            anyhow::bail!(message);
        }

        self.mark_runtime_stopped(attempt_id)
            .await
            .context("Failed to mark runtime as stopped")?;

        info!("Preview runtime stopped for attempt {}", attempt_id);
        Ok(())
    }

    /// Get preview Docker runtime status for an attempt.
    pub async fn get_preview_runtime_status(
        &self,
        attempt_id: Uuid,
    ) -> Result<PreviewRuntimeStatus> {
        let runtime_enabled = self.runtime_enabled();
        let runtime_metadata = sqlx::query_as::<_, CloudflareTunnel>(
            r#"
            SELECT * FROM cloudflare_tunnels
            WHERE attempt_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to query runtime metadata from cloudflare_tunnels")?;

        let docker_project_name = runtime_metadata
            .as_ref()
            .and_then(|metadata| metadata.docker_project_name.clone());
        let compose_file_path_from_db = runtime_metadata
            .as_ref()
            .and_then(|metadata| metadata.compose_file_path.clone());
        let last_error = runtime_metadata
            .as_ref()
            .and_then(|metadata| metadata.last_error.clone());
        let started_at = runtime_metadata
            .as_ref()
            .and_then(|metadata| metadata.started_at);
        let stopped_at = runtime_metadata
            .as_ref()
            .and_then(|metadata| metadata.stopped_at);
        let requires_cloudflared = runtime_metadata
            .as_ref()
            .map(|metadata| !is_local_preview_tunnel_id(&metadata.tunnel_id))
            .unwrap_or(true);

        let worktree_path = match self.resolve_attempt_worktree_path(attempt_id).await {
            Ok(path) => path,
            Err(_) => {
                return Ok(PreviewRuntimeStatus {
                    attempt_id,
                    runtime_enabled,
                    worktree_path: None,
                    compose_file_exists: false,
                    docker_project_name,
                    compose_file_path: compose_file_path_from_db,
                    running_services: Vec::new(),
                    runtime_ready: false,
                    last_error,
                    started_at,
                    stopped_at,
                    message: Some(
                        "Worktree path not found for attempt (execution_processes/metadata)"
                            .to_string(),
                    ),
                });
            }
        };

        let runtime_dir = worktree_path
            .join(".acpms")
            .join("preview")
            .join(attempt_id.to_string());
        let compose_path = runtime_dir.join("docker-compose.preview.yml");
        let compose_file_path = compose_file_path_from_db
            .clone()
            .unwrap_or_else(|| compose_path.to_string_lossy().to_string());
        let compose_file_exists = compose_path.exists();

        if !runtime_enabled {
            return Ok(PreviewRuntimeStatus {
                attempt_id,
                runtime_enabled,
                worktree_path: Some(worktree_path.to_string_lossy().to_string()),
                compose_file_exists,
                docker_project_name,
                compose_file_path: Some(compose_file_path),
                running_services: Vec::new(),
                runtime_ready: false,
                last_error,
                started_at,
                stopped_at,
                message: Some("Docker preview runtime is disabled".to_string()),
            });
        }

        if !compose_file_exists {
            return Ok(PreviewRuntimeStatus {
                attempt_id,
                runtime_enabled,
                worktree_path: Some(worktree_path.to_string_lossy().to_string()),
                compose_file_exists,
                docker_project_name,
                compose_file_path: Some(compose_file_path),
                running_services: Vec::new(),
                runtime_ready: false,
                last_error,
                started_at,
                stopped_at,
                message: Some("Compose file not found for this attempt runtime".to_string()),
            });
        }

        let project_name = docker_project_name
            .clone()
            .unwrap_or_else(|| preview_docker_project_name(attempt_id));
        let output = Command::new(preview_docker_command())
            .arg("compose")
            .arg("-p")
            .arg(&project_name)
            .arg("-f")
            .arg(&compose_path)
            .arg("ps")
            .arg("--services")
            .arg("--status")
            .arg("running")
            .current_dir(&runtime_dir)
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let services_stdout = String::from_utf8_lossy(&output.stdout);
                let running_services = parse_running_services_output(&services_stdout);

                Ok(PreviewRuntimeStatus {
                    attempt_id,
                    runtime_enabled,
                    worktree_path: Some(worktree_path.to_string_lossy().to_string()),
                    compose_file_exists,
                    docker_project_name,
                    compose_file_path: Some(compose_file_path),
                    runtime_ready: runtime_services_ready(&running_services, requires_cloudflared),
                    running_services,
                    last_error,
                    started_at,
                    stopped_at,
                    message: None,
                })
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Ok(PreviewRuntimeStatus {
                    attempt_id,
                    runtime_enabled,
                    worktree_path: Some(worktree_path.to_string_lossy().to_string()),
                    compose_file_exists,
                    docker_project_name,
                    compose_file_path: Some(compose_file_path),
                    running_services: Vec::new(),
                    runtime_ready: false,
                    last_error,
                    started_at,
                    stopped_at,
                    message: Some(format!(
                        "docker compose ps failed (status: {:?}): {}",
                        output.status.code(),
                        stderr
                    )),
                })
            }
            Err(error) => Ok(PreviewRuntimeStatus {
                attempt_id,
                runtime_enabled,
                worktree_path: Some(worktree_path.to_string_lossy().to_string()),
                compose_file_exists,
                docker_project_name,
                compose_file_path: Some(compose_file_path),
                running_services: Vec::new(),
                runtime_ready: false,
                last_error,
                started_at,
                stopped_at,
                message: Some(format!("Failed to execute docker compose ps: {}", error)),
            }),
        }
    }

    /// Get Docker compose logs for preview runtime (debug endpoint).
    pub async fn get_preview_runtime_logs(
        &self,
        attempt_id: Uuid,
        tail: Option<u32>,
    ) -> Result<PreviewRuntimeLogs> {
        let runtime_enabled = self.runtime_enabled();
        let tail = tail.unwrap_or(200).clamp(1, 2000);

        let runtime_metadata = sqlx::query_as::<_, CloudflareTunnel>(
            r#"
            SELECT * FROM cloudflare_tunnels
            WHERE attempt_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to query runtime metadata from cloudflare_tunnels")?;

        let docker_project_name = runtime_metadata
            .as_ref()
            .and_then(|metadata| metadata.docker_project_name.clone());

        if !runtime_enabled {
            return Ok(PreviewRuntimeLogs {
                attempt_id,
                runtime_enabled,
                docker_project_name,
                compose_file_path: runtime_metadata.and_then(|metadata| metadata.compose_file_path),
                tail,
                logs: String::new(),
                message: Some("Docker preview runtime is disabled".to_string()),
            });
        }

        let worktree_path = match self.resolve_attempt_worktree_path(attempt_id).await {
            Ok(path) => path,
            Err(_) => {
                return Ok(PreviewRuntimeLogs {
                    attempt_id,
                    runtime_enabled,
                    docker_project_name,
                    compose_file_path: runtime_metadata
                        .and_then(|metadata| metadata.compose_file_path),
                    tail,
                    logs: String::new(),
                    message: Some(
                        "Worktree path not found for attempt (execution_processes/metadata)"
                            .to_string(),
                    ),
                });
            }
        };

        let runtime_dir = worktree_path
            .join(".acpms")
            .join("preview")
            .join(attempt_id.to_string());

        let compose_file_path = runtime_metadata
            .as_ref()
            .and_then(|metadata| metadata.compose_file_path.clone())
            .unwrap_or_else(|| {
                runtime_dir
                    .join("docker-compose.preview.yml")
                    .to_string_lossy()
                    .to_string()
            });
        let compose_path = PathBuf::from(&compose_file_path);
        if !compose_path.exists() {
            return Ok(PreviewRuntimeLogs {
                attempt_id,
                runtime_enabled,
                docker_project_name,
                compose_file_path: Some(compose_file_path),
                tail,
                logs: String::new(),
                message: Some("Compose file not found for this attempt runtime".to_string()),
            });
        }

        let project_name = docker_project_name
            .clone()
            .unwrap_or_else(|| preview_docker_project_name(attempt_id));
        let output = Command::new(preview_docker_command())
            .arg("compose")
            .arg("-p")
            .arg(&project_name)
            .arg("-f")
            .arg(&compose_path)
            .arg("logs")
            .arg("--no-color")
            .arg("--tail")
            .arg(tail.to_string())
            .current_dir(&runtime_dir)
            .output();

        match output {
            Ok(output) if output.status.success() => Ok(PreviewRuntimeLogs {
                attempt_id,
                runtime_enabled,
                docker_project_name,
                compose_file_path: Some(compose_file_path),
                tail,
                logs: String::from_utf8_lossy(&output.stdout).to_string(),
                message: None,
            }),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                Ok(PreviewRuntimeLogs {
                    attempt_id,
                    runtime_enabled,
                    docker_project_name,
                    compose_file_path: Some(compose_file_path),
                    tail,
                    logs: stdout,
                    message: Some(format!(
                        "docker compose logs failed (status: {:?}): {}",
                        output.status.code(),
                        stderr
                    )),
                })
            }
            Err(error) => Ok(PreviewRuntimeLogs {
                attempt_id,
                runtime_enabled,
                docker_project_name,
                compose_file_path: Some(compose_file_path),
                tail,
                logs: String::new(),
                message: Some(format!("Failed to execute docker compose logs: {}", error)),
            }),
        }
    }

    /// Generate a sanitized tunnel name
    fn generate_tunnel_name(&self, task_name: &str, attempt_id: Uuid) -> String {
        // Sanitize task name (alphanumeric, dash, underscore only)
        let sanitized: String = task_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();

        // Truncate to 40 chars and add attempt ID prefix
        let truncated = if sanitized.len() > 40 {
            &sanitized[..40]
        } else {
            &sanitized
        };

        format!("acpms-{}-{}", &attempt_id.to_string()[..8], truncated)
    }

    async fn rollback_cloudflare_resources(
        &self,
        cloudflare: &CloudflareClient,
        zone_id: Option<&str>,
        dns_record_id: Option<&str>,
        tunnel_id: &str,
        reason: &str,
    ) {
        if let (Some(zone_id), Some(record_id)) = (zone_id, dns_record_id) {
            if let Err(error) = cloudflare.delete_dns_record(zone_id, record_id).await {
                warn!(
                    "Rollback warning ({}): failed to delete DNS record {} in zone {}: {}",
                    reason, record_id, zone_id, error
                );
            }
        }

        if let Err(error) = cloudflare.delete_tunnel(tunnel_id).await {
            warn!(
                "Rollback warning ({}): failed to delete tunnel {}: {}",
                reason, tunnel_id, error
            );
        }
    }

    async fn wait_for_runtime_ready(
        &self,
        attempt_id: Uuid,
        runtime_dir: &Path,
        compose_path: &Path,
        project_name: &str,
        require_cloudflared: bool,
    ) -> Result<()> {
        let timeout = TokioDuration::from_secs(preview_runtime_start_timeout_secs());
        let poll_interval = TokioDuration::from_secs(2);
        let deadline = Instant::now() + timeout;

        loop {
            let output = Command::new(preview_docker_command())
                .arg("compose")
                .arg("-p")
                .arg(project_name)
                .arg("-f")
                .arg(compose_path)
                .arg("ps")
                .arg("--services")
                .arg("--status")
                .arg("running")
                .current_dir(runtime_dir)
                .output();

            match output {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let running_services = parse_running_services_output(&stdout);
                    if runtime_services_ready(&running_services, require_cloudflared) {
                        return Ok(());
                    }
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    debug!(
                        "Runtime readiness check attempt {} returned non-success status {:?}: {}",
                        attempt_id,
                        output.status.code(),
                        stderr
                    );
                }
                Err(error) => {
                    debug!(
                        "Runtime readiness check attempt {} failed to execute docker compose ps: {}",
                        attempt_id, error
                    );
                }
            }

            if Instant::now() >= deadline {
                let required_services_label = if require_cloudflared {
                    "dev-server and cloudflared"
                } else {
                    "dev-server"
                };
                anyhow::bail!(
                    "Timed out after {}s waiting for {} to be running",
                    timeout.as_secs(),
                    required_services_label
                );
            }

            sleep(poll_interval).await;
        }
    }

    fn stop_runtime_containers_best_effort(
        &self,
        attempt_id: Uuid,
        runtime_dir: &Path,
        compose_path: &Path,
        project_name: &str,
    ) {
        let output = Command::new(preview_docker_command())
            .arg("compose")
            .arg("-p")
            .arg(project_name)
            .arg("-f")
            .arg(compose_path)
            .arg("down")
            .arg("--remove-orphans")
            .current_dir(runtime_dir)
            .output();

        match output {
            Ok(output) if output.status.success() => {
                debug!(
                    "Best-effort runtime teardown succeeded for attempt {}",
                    attempt_id
                );
            }
            Ok(output) => {
                warn!(
                    "Best-effort runtime teardown failed for attempt {} with status {:?}: {}",
                    attempt_id,
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(error) => {
                warn!(
                    "Best-effort runtime teardown command failed for attempt {}: {}",
                    attempt_id, error
                );
            }
        }
    }

    async fn mark_runtime_preparing(
        &self,
        attempt_id: Uuid,
        project_name: &str,
        compose_file_path: &Path,
        worktree_path: &Path,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE cloudflare_tunnels
            SET
                docker_project_name = $2,
                compose_file_path = $3,
                worktree_path = $4,
                status = $5,
                last_error = NULL,
                stopped_at = NULL
            WHERE attempt_id = $1
              AND deleted_at IS NULL
            "#,
        )
        .bind(attempt_id)
        .bind(project_name)
        .bind(compose_file_path.to_string_lossy().to_string())
        .bind(worktree_path.to_string_lossy().to_string())
        .bind(TunnelStatus::Creating)
        .execute(&self.db)
        .await
        .context("Failed to update preview runtime metadata")?;

        Ok(())
    }

    async fn mark_runtime_started(&self, attempt_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE cloudflare_tunnels
            SET
                status = $2,
                started_at = NOW(),
                stopped_at = NULL,
                last_error = NULL
            WHERE attempt_id = $1
              AND deleted_at IS NULL
            "#,
        )
        .bind(attempt_id)
        .bind(TunnelStatus::Active)
        .execute(&self.db)
        .await
        .context("Failed to mark preview runtime as started")?;

        Ok(())
    }

    async fn mark_runtime_stopped(&self, attempt_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE cloudflare_tunnels
            SET
                stopped_at = NOW(),
                last_error = NULL
            WHERE attempt_id = $1
              AND deleted_at IS NULL
            "#,
        )
        .bind(attempt_id)
        .execute(&self.db)
        .await
        .context("Failed to mark preview runtime as stopped")?;

        Ok(())
    }

    async fn record_runtime_error(&self, attempt_id: Uuid, message: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE cloudflare_tunnels
            SET
                status = $2,
                last_error = $3
            WHERE attempt_id = $1
              AND deleted_at IS NULL
            "#,
        )
        .bind(attempt_id)
        .bind(TunnelStatus::Failed)
        .bind(message)
        .execute(&self.db)
        .await
        .context("Failed to persist preview runtime error")?;

        Ok(())
    }

    async fn resolve_attempt_worktree_path(&self, attempt_id: Uuid) -> Result<PathBuf> {
        let execution_process_worktree = sqlx::query_scalar::<_, String>(
            r#"
            SELECT worktree_path
            FROM execution_processes
            WHERE attempt_id = $1
              AND worktree_path IS NOT NULL
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to query execution_processes for worktree path")?;

        if let Some(path) = execution_process_worktree {
            return Ok(PathBuf::from(path));
        }

        let tunnel_worktree_path = sqlx::query_scalar::<_, Option<String>>(
            r#"
            SELECT worktree_path
            FROM cloudflare_tunnels
            WHERE attempt_id = $1
              AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to query cloudflare_tunnels for stored worktree path")?
        .flatten()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty());

        if let Some(path) = tunnel_worktree_path {
            return Ok(PathBuf::from(path));
        }

        let attempt_metadata_worktree = sqlx::query_scalar::<_, Option<String>>(
            r#"
            SELECT metadata->>'worktree_path'
            FROM task_attempts
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to query task_attempts metadata for worktree path")?
        .flatten();

        attempt_metadata_worktree
            .map(PathBuf::from)
            .context("No worktree_path found in execution_processes or task_attempts.metadata")
    }
}

fn preview_docker_project_name(attempt_id: Uuid) -> String {
    format!("acpms-preview-{}", &attempt_id.to_string()[..8])
}

fn preview_docker_command() -> String {
    std::env::var("PREVIEW_DOCKER_COMMAND")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "docker".to_string())
}

fn is_docker_runtime_enabled() -> bool {
    std::env::var("PREVIEW_DOCKER_RUNTIME_ENABLED")
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn preview_dev_port() -> u16 {
    std::env::var("PREVIEW_DEV_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port > 0)
        .unwrap_or(3000)
}

fn preview_local_port_base() -> u16 {
    std::env::var("PREVIEW_LOCAL_PORT_BASE")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port > 0)
        .unwrap_or(42000)
}

fn preview_local_port_span() -> u16 {
    std::env::var("PREVIEW_LOCAL_PORT_SPAN")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|span| *span > 0)
        .unwrap_or(1000)
}

fn preview_local_public_port(attempt_id: Uuid) -> u16 {
    let base = preview_local_port_base() as u32;
    let span = preview_local_port_span() as u32;
    let hash = attempt_id.as_bytes().iter().fold(0u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(*byte as u32)
    });
    let offset = if span == 0 { 0 } else { hash % span };
    base.saturating_add(offset).min(u16::MAX as u32) as u16
}

fn allocate_preview_local_public_port(attempt_id: Uuid) -> Result<u16> {
    let preferred = preview_local_public_port(attempt_id);
    if is_loopback_port_available(preferred) {
        return Ok(preferred);
    }

    let base = preview_local_port_base() as u32;
    let span = preview_local_port_span().max(1) as u32;
    let preferred_offset = (preferred as u32).saturating_sub(base) % span;

    for step in 1..span {
        let candidate = base + ((preferred_offset + step) % span);
        if candidate > u16::MAX as u32 {
            continue;
        }
        let candidate = candidate as u16;
        if is_loopback_port_available(candidate) {
            return Ok(candidate);
        }
    }

    anyhow::bail!(
        "No available loopback preview port found in range {}-{}",
        preview_local_port_base(),
        preview_local_port_base().saturating_add(preview_local_port_span().saturating_sub(1))
    )
}

fn is_loopback_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn extract_local_preview_port(preview_url: &str) -> Option<u16> {
    let trimmed = preview_url.trim();
    let without_scheme = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))?;
    let authority = without_scheme.split('/').next()?.trim();
    if let Some(port) = authority.strip_prefix("localhost:") {
        return port.parse::<u16>().ok().filter(|port| *port > 0);
    }
    if let Some(port) = authority.strip_prefix("127.0.0.1:") {
        return port.parse::<u16>().ok().filter(|port| *port > 0);
    }
    None
}

fn local_preview_url(port: u16) -> String {
    format!("http://localhost:{}", port)
}

fn is_local_preview_tunnel_id(tunnel_id: &str) -> bool {
    tunnel_id.starts_with(LOCAL_PREVIEW_TUNNEL_PREFIX)
}

fn has_complete_cloudflare_config(settings: &SystemSettings) -> bool {
    settings
        .cloudflare_account_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        && settings
            .cloudflare_api_token_encrypted
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
        && settings
            .cloudflare_zone_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
        && settings
            .cloudflare_base_domain
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
}

fn preview_dev_image_override() -> Option<String> {
    std::env::var("PREVIEW_DEV_IMAGE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn resolve_preview_dev_image(worktree_path: &Path, dev_command: &str) -> String {
    if let Some(image) = preview_dev_image_override() {
        return image;
    }

    let normalized_command = dev_command.to_ascii_lowercase();

    if normalized_command.contains("python")
        || normalized_command.contains("uvicorn")
        || normalized_command.contains("flask")
        || normalized_command.contains("django")
    {
        return "python:3.12-alpine".to_string();
    }

    if normalized_command.contains(" go run ") || normalized_command.contains("go run ") {
        return "golang:1.22-alpine".to_string();
    }

    if normalized_command.contains("cargo run") {
        return "rust:1.77-alpine".to_string();
    }

    if has_python_preview_files(worktree_path) {
        return "python:3.12-alpine".to_string();
    }
    if worktree_path.join("go.mod").exists() {
        return "golang:1.22-alpine".to_string();
    }
    if worktree_path.join("Cargo.toml").exists() {
        return "rust:1.77-alpine".to_string();
    }

    "node:20-alpine".to_string()
}

fn preview_runtime_start_timeout_secs() -> u64 {
    std::env::var("PREVIEW_RUNTIME_START_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|timeout| *timeout > 0)
        .unwrap_or(90)
}

fn resolve_preview_dev_command(
    worktree_path: &Path,
    project_type: ProjectType,
    port: u16,
) -> Result<String> {
    resolve_preview_dev_command_with_lookup(worktree_path, project_type, port, |key| {
        std::env::var(key).ok()
    })
}

fn resolve_preview_dev_command_with_lookup<F>(
    worktree_path: &Path,
    project_type: ProjectType,
    port: u16,
    lookup_env: F,
) -> Result<String>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(command) = non_empty_env_value(&lookup_env, "PREVIEW_DEV_COMMAND") {
        return Ok(command);
    }

    if let Some(project_env_key) = project_type_command_env_key(project_type) {
        if let Some(command) = non_empty_env_value(&lookup_env, project_env_key) {
            return Ok(command);
        }
    }

    match load_package_scripts(worktree_path)? {
        Some(scripts) => {
            if let Some((script_name, script_value)) = select_preview_script(&scripts, project_type)
            {
                let package_manager = detect_package_manager(worktree_path);
                let run_command = package_manager_run_command(package_manager, &script_name);
                let additional_args = additional_preview_cli_args(&script_value, port);

                return Ok(format!(
                    "HOST=0.0.0.0 HOSTNAME=0.0.0.0 PORT={port} {run_command}{additional_args}"
                ));
            }

            if let Some(command) =
                resolve_non_node_preview_command(worktree_path, project_type, port)
            {
                return Ok(command);
            }

            let mut available_scripts = scripts.keys().cloned().collect::<Vec<_>>();
            available_scripts.sort();
            anyhow::bail!(
                "Unable to resolve preview command from package.json for project type '{}'. Tried scripts [{}], available scripts [{}].",
                project_type_name(project_type),
                project_type_script_candidates(project_type).join(", "),
                available_scripts.join(", ")
            );
        }
        None => {
            if let Some(command) =
                resolve_non_node_preview_command(worktree_path, project_type, port)
            {
                return Ok(command);
            }

            anyhow::bail!(
                "Unable to resolve preview command for project type '{}' because package.json is missing at {} and no supported non-Node entrypoint (Python/Go/Rust) was detected. Set PREVIEW_DEV_COMMAND or {}.",
                project_type_name(project_type),
                worktree_path.display(),
                project_type_command_env_key(project_type).unwrap_or("PREVIEW_DEV_COMMAND")
            );
        }
    }
}

fn non_empty_env_value<F>(lookup_env: &F, key: &str) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    lookup_env(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn project_type_command_env_key(project_type: ProjectType) -> Option<&'static str> {
    match project_type {
        ProjectType::Web => Some("PREVIEW_WEB_DEV_COMMAND"),
        ProjectType::Api => Some("PREVIEW_API_DEV_COMMAND"),
        ProjectType::Microservice => Some("PREVIEW_MICROSERVICE_DEV_COMMAND"),
        ProjectType::Extension => Some("PREVIEW_EXTENSION_DEV_COMMAND"),
        ProjectType::Mobile | ProjectType::Desktop => None,
    }
}

fn project_type_script_candidates(project_type: ProjectType) -> &'static [&'static str] {
    match project_type {
        ProjectType::Web | ProjectType::Extension => &["dev", "start", "preview", "serve"],
        ProjectType::Api | ProjectType::Microservice => &["dev", "start", "serve", "preview"],
        ProjectType::Mobile | ProjectType::Desktop => &[],
    }
}

fn project_type_name(project_type: ProjectType) -> &'static str {
    match project_type {
        ProjectType::Web => "web",
        ProjectType::Mobile => "mobile",
        ProjectType::Desktop => "desktop",
        ProjectType::Extension => "extension",
        ProjectType::Api => "api",
        ProjectType::Microservice => "microservice",
    }
}

fn load_package_scripts(worktree_path: &Path) -> Result<Option<BTreeMap<String, String>>> {
    let package_json_path = worktree_path.join("package.json");
    if !package_json_path.exists() {
        return Ok(None);
    }

    let package_json_content = fs::read_to_string(&package_json_path).with_context(|| {
        format!(
            "Failed to read package.json for preview command resolution: {}",
            package_json_path.display()
        )
    })?;

    let package_json_value: serde_json::Value = serde_json::from_str(&package_json_content)
        .with_context(|| {
            format!(
                "Failed to parse package.json for preview command resolution: {}",
                package_json_path.display()
            )
        })?;

    let scripts = package_json_value
        .get("scripts")
        .and_then(|value| value.as_object())
        .map(|scripts| {
            scripts
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .as_str()
                        .map(|script| (name.to_string(), script.to_string()))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    Ok(Some(scripts))
}

fn select_preview_script(
    scripts: &BTreeMap<String, String>,
    project_type: ProjectType,
) -> Option<(String, String)> {
    project_type_script_candidates(project_type)
        .iter()
        .find_map(|script_name| {
            scripts
                .get(*script_name)
                .map(|script| ((*script_name).to_string(), script.clone()))
        })
}

fn resolve_non_node_preview_command(
    worktree_path: &Path,
    project_type: ProjectType,
    port: u16,
) -> Option<String> {
    if !matches!(project_type, ProjectType::Api | ProjectType::Microservice) {
        return None;
    }

    let env_prefix = format!("HOST=0.0.0.0 HOSTNAME=0.0.0.0 PORT={port}");

    if worktree_path.join("manage.py").exists() {
        return Some(format!(
            "{env_prefix} python manage.py runserver 0.0.0.0:{port}"
        ));
    }

    if let Some(module) = detect_fastapi_app_module(worktree_path) {
        return Some(format!(
            "{env_prefix} python -m uvicorn {module}:app --host 0.0.0.0 --port {port}"
        ));
    }

    if worktree_path.join("app.py").exists() {
        return Some(format!("{env_prefix} python app.py"));
    }

    if worktree_path.join("main.py").exists() {
        return Some(format!("{env_prefix} python main.py"));
    }

    if worktree_path.join("go.mod").exists() {
        return Some(format!("{env_prefix} go run ."));
    }

    if worktree_path.join("Cargo.toml").exists() {
        return Some(format!("{env_prefix} cargo run"));
    }

    None
}

fn has_python_preview_files(worktree_path: &Path) -> bool {
    worktree_path.join("pyproject.toml").exists()
        || worktree_path.join("requirements.txt").exists()
        || worktree_path.join("manage.py").exists()
        || worktree_path.join("main.py").exists()
        || worktree_path.join("app.py").exists()
}

fn detect_fastapi_app_module(worktree_path: &Path) -> Option<String> {
    let candidates = [
        "main.py",
        "app.py",
        "src/main.py",
        "src/app.py",
        "app/main.py",
    ];

    for relative in candidates {
        let candidate_path = worktree_path.join(relative);
        if !candidate_path.exists() {
            continue;
        }
        let content = match fs::read_to_string(&candidate_path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        if content.contains("FastAPI(") {
            return Some(python_module_from_relative_path(relative));
        }
    }

    None
}

fn python_module_from_relative_path(relative_path: &str) -> String {
    relative_path
        .trim_end_matches(".py")
        .replace('/', ".")
        .replace('\\', ".")
}

fn detect_package_manager(worktree_path: &Path) -> PackageManager {
    if worktree_path.join("pnpm-lock.yaml").exists() {
        PackageManager::Pnpm
    } else if worktree_path.join("yarn.lock").exists() {
        PackageManager::Yarn
    } else if worktree_path.join("bun.lock").exists() || worktree_path.join("bun.lockb").exists() {
        PackageManager::Bun
    } else {
        PackageManager::Npm
    }
}

fn package_manager_run_command(package_manager: PackageManager, script_name: &str) -> String {
    match package_manager {
        PackageManager::Npm => format!("npm run {script_name}"),
        PackageManager::Pnpm => format!("pnpm run {script_name}"),
        PackageManager::Yarn => format!("yarn {script_name}"),
        PackageManager::Bun => format!("bun run {script_name}"),
    }
}

fn additional_preview_cli_args(script_value: &str, port: u16) -> String {
    let normalized = script_value.to_ascii_lowercase();
    let requires_cli_flags = normalized.contains("vite")
        || normalized.contains("next")
        || normalized.contains("nuxt")
        || normalized.contains("astro")
        || normalized.contains("svelte")
        || normalized.contains("webpack");

    if !requires_cli_flags {
        return String::new();
    }

    let has_host_arg = normalized.contains("--host") || normalized.contains("--hostname");
    let has_port_arg = normalized.contains("--port") || normalized.contains(" -p ");
    let mut args = Vec::new();

    if !has_host_arg {
        if normalized.contains("next") {
            args.push("--hostname 0.0.0.0".to_string());
        } else {
            args.push("--host 0.0.0.0".to_string());
        }
    }

    if !has_port_arg {
        args.push(format!("--port {port}"));
    }

    if args.is_empty() {
        String::new()
    } else {
        format!(" -- {}", args.join(" "))
    }
}

fn parse_running_services_output(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn runtime_services_ready(services: &[String], require_cloudflared: bool) -> bool {
    let has_dev_server = services.iter().any(|service| service == "dev-server");
    if !require_cloudflared {
        return has_dev_server;
    }

    let has_cloudflared = services.iter().any(|service| service == "cloudflared");
    has_dev_server && has_cloudflared
}

fn build_compose_content(
    worktree_path: &Path,
    dev_image: &str,
    dev_command: &str,
    port: u16,
    exposure_mode: PreviewExposureMode,
) -> Result<String> {
    let worktree = yaml_single_quote(&worktree_path.to_string_lossy());
    let command_json =
        serde_json::to_string(dev_command).context("Failed to JSON-encode dev command")?;
    let dev_server_base = format!(
        r#"services:
  dev-server:
    image: {dev_image}
    working_dir: /workspace
    command: ["sh", "-lc", {command_json}]
    volumes:
      - '{worktree}:/workspace'
    expose:
      - "{port}"
    restart: unless-stopped
    networks:
      - previewnet
"#
    );

    match exposure_mode {
        PreviewExposureMode::Cloudflare {
            credentials_path,
            tunnel_id,
        } => {
            let credentials = yaml_single_quote(&credentials_path.to_string_lossy());
            Ok(format!(
                r#"{dev_server_base}
  cloudflared:
    image: cloudflare/cloudflared:latest
    command: ["tunnel", "--no-autoupdate", "run", "--credentials-file", "/etc/cloudflared/credentials.json", "--url", "http://dev-server:{port}", "{tunnel_id}"]
    depends_on:
      - dev-server
    volumes:
      - '{credentials}:/etc/cloudflared/credentials.json:ro'
    restart: unless-stopped
    networks:
      - previewnet

networks:
  previewnet:
    driver: bridge
"#
            ))
        }
        PreviewExposureMode::Local { host_port } => Ok(format!(
            r#"{dev_server_base}
    ports:
      - "127.0.0.1:{host_port}:{port}"

networks:
  previewnet:
    driver: bridge
"#
        )),
    }
}

fn yaml_single_quote(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_temp_worktree(prefix: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("acpms-preview-{prefix}-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_package_json(worktree: &Path, scripts_json: &str) {
        let package_json = format!(r#"{{"name":"preview-test","scripts":{scripts_json}}}"#);
        fs::write(worktree.join("package.json"), package_json).unwrap();
    }

    #[tokio::test]
    async fn test_generate_tunnel_name() {
        let key = acpms_services::generate_encryption_key();
        let encryption = EncryptionService::new(&key).unwrap();
        let db = PgPool::connect_lazy("postgres://test").unwrap();
        let settings_service =
            SystemSettingsService::new_with_encryption(db.clone(), encryption.clone());

        let manager = PreviewManager {
            cloudflare: CloudflareClient::new("test".to_string(), "test".to_string()).unwrap(),
            encryption,
            settings_service,
            db,
            preview_ttl_days: 7,
        };

        let attempt_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

        // Test normal name
        let name = manager.generate_tunnel_name("Feature Auth Login", attempt_id);
        assert_eq!(name, "acpms-550e8400-feature-auth-login");

        // Test long name (should truncate)
        let long_name = "This is a very long task name that should be truncated to fit the limit";
        let name = manager.generate_tunnel_name(long_name, attempt_id);
        assert!(name.len() <= 55); // "acpms-" + 8-char attempt prefix + "-" + 40-char task name
        assert!(name.starts_with("acpms-550e8400-"));

        // Test special characters (should sanitize)
        let name = manager.generate_tunnel_name("Task #123 @user: fix bug!", attempt_id);
        assert_eq!(name, "acpms-550e8400-task--123--user--fix-bug-");
    }

    #[test]
    fn resolve_preview_command_prefers_global_env_override() {
        let worktree = create_temp_worktree("env-override");
        let command =
            resolve_preview_dev_command_with_lookup(&worktree, ProjectType::Web, 4321, |key| {
                if key == "PREVIEW_DEV_COMMAND" {
                    Some("pnpm run dev -- --host 0.0.0.0 --port 4321".to_string())
                } else {
                    None
                }
            })
            .unwrap();

        assert_eq!(command, "pnpm run dev -- --host 0.0.0.0 --port 4321");
        let _ = fs::remove_dir_all(worktree);
    }

    #[test]
    fn resolve_preview_command_detects_pnpm_and_adds_host_port_flags_for_vite() {
        let worktree = create_temp_worktree("pnpm-vite");
        write_package_json(&worktree, r#"{"dev":"vite"}"#);
        fs::write(worktree.join("pnpm-lock.yaml"), "").unwrap();

        let command =
            resolve_preview_dev_command_with_lookup(&worktree, ProjectType::Web, 4173, |_| None)
                .unwrap();

        assert!(command.contains("pnpm run dev"));
        assert!(command.contains("--host 0.0.0.0"));
        assert!(command.contains("--port 4173"));
        let _ = fs::remove_dir_all(worktree);
    }

    #[test]
    fn resolve_preview_command_uses_hostname_flag_for_next() {
        let worktree = create_temp_worktree("next");
        write_package_json(&worktree, r#"{"dev":"next dev"}"#);

        let command =
            resolve_preview_dev_command_with_lookup(&worktree, ProjectType::Web, 3001, |_| None)
                .unwrap();

        assert!(command.contains("npm run dev"));
        assert!(command.contains("--hostname 0.0.0.0"));
        assert!(command.contains("--port 3001"));
        let _ = fs::remove_dir_all(worktree);
    }

    #[test]
    fn resolve_preview_command_errors_when_package_json_missing_and_no_override() {
        let worktree = create_temp_worktree("missing-package");
        let error =
            resolve_preview_dev_command_with_lookup(&worktree, ProjectType::Web, 3000, |_| None)
                .unwrap_err();

        assert!(error.to_string().contains("package.json is missing"));
        let _ = fs::remove_dir_all(worktree);
    }

    #[test]
    fn resolve_preview_command_falls_back_to_django_manage_py_for_api() {
        let worktree = create_temp_worktree("django-fallback");
        fs::write(worktree.join("manage.py"), "print('django')").unwrap();

        let command =
            resolve_preview_dev_command_with_lookup(&worktree, ProjectType::Api, 8001, |_| None)
                .unwrap();

        assert_eq!(
            command,
            "HOST=0.0.0.0 HOSTNAME=0.0.0.0 PORT=8001 python manage.py runserver 0.0.0.0:8001"
        );
        let _ = fs::remove_dir_all(worktree);
    }

    #[test]
    fn resolve_preview_command_falls_back_to_fastapi_uvicorn_for_microservice() {
        let worktree = create_temp_worktree("fastapi-fallback");
        fs::create_dir_all(worktree.join("src")).unwrap();
        fs::write(
            worktree.join("src/app.py"),
            "from fastapi import FastAPI\napp = FastAPI()\n",
        )
        .unwrap();
        fs::write(worktree.join("requirements.txt"), "fastapi\nuvicorn\n").unwrap();

        let command = resolve_preview_dev_command_with_lookup(
            &worktree,
            ProjectType::Microservice,
            8100,
            |_| None,
        )
        .unwrap();

        assert_eq!(
            command,
            "HOST=0.0.0.0 HOSTNAME=0.0.0.0 PORT=8100 python -m uvicorn src.app:app --host 0.0.0.0 --port 8100"
        );
        let _ = fs::remove_dir_all(worktree);
    }

    #[test]
    fn resolve_preview_command_falls_back_to_go_run_for_api() {
        let worktree = create_temp_worktree("go-fallback");
        fs::write(worktree.join("go.mod"), "module example.com/demo\n").unwrap();

        let command =
            resolve_preview_dev_command_with_lookup(&worktree, ProjectType::Api, 8200, |_| None)
                .unwrap();

        assert_eq!(command, "HOST=0.0.0.0 HOSTNAME=0.0.0.0 PORT=8200 go run .");
        let _ = fs::remove_dir_all(worktree);
    }

    #[test]
    fn build_compose_content_includes_runtime_services_and_escaped_paths() {
        let worktree = create_temp_worktree("compose-spec");
        let quoted_worktree = worktree.join("repo'with-quote");
        fs::create_dir_all(&quoted_worktree).unwrap();
        let credentials_path = quoted_worktree.join(".acpms/preview/creds'.json");
        fs::create_dir_all(credentials_path.parent().unwrap()).unwrap();

        let compose = build_compose_content(
            &quoted_worktree,
            "node:20-alpine",
            "pnpm run dev -- --host 0.0.0.0 --port 3000",
            3000,
            PreviewExposureMode::Cloudflare {
                credentials_path: credentials_path.clone(),
                tunnel_id: "tunnel-123".to_string(),
            },
        )
        .unwrap();

        assert!(compose.contains("dev-server:"));
        assert!(compose.contains("image: node:20-alpine"));
        assert!(compose.contains("cloudflared:"));
        assert!(compose.contains("http://dev-server:3000"));
        assert!(compose.contains("repo''with-quote"));
        assert!(compose.contains("creds''.json"));
        let _ = fs::remove_dir_all(worktree);
    }

    #[test]
    fn build_compose_content_local_mode_exposes_host_port_without_cloudflared() {
        let worktree = create_temp_worktree("compose-local");
        let compose = build_compose_content(
            &worktree,
            "node:20-alpine",
            "npm run dev",
            3000,
            PreviewExposureMode::Local { host_port: 43123 },
        )
        .unwrap();

        assert!(compose.contains("dev-server:"));
        assert!(compose.contains("127.0.0.1:43123:3000"));
        assert!(!compose.contains("cloudflared:"));
        let _ = fs::remove_dir_all(worktree);
    }

    #[test]
    fn extract_local_preview_port_supports_localhost_and_loopback() {
        assert_eq!(
            extract_local_preview_port("http://localhost:43123"),
            Some(43123)
        );
        assert_eq!(
            extract_local_preview_port("http://127.0.0.1:43124/path"),
            Some(43124)
        );
        assert_eq!(
            extract_local_preview_port("https://example.com:43125"),
            None
        );
    }

    #[test]
    fn allocate_preview_local_public_port_skips_occupied_preferred_port() {
        let mut reserved = None;
        let mut attempt_id = Uuid::new_v4();
        let mut preferred = preview_local_public_port(attempt_id);

        for _ in 0..128 {
            if let Ok(listener) = TcpListener::bind(("127.0.0.1", preferred)) {
                reserved = Some(listener);
                break;
            }
            attempt_id = Uuid::new_v4();
            preferred = preview_local_public_port(attempt_id);
        }

        let _listener = reserved.expect("failed to reserve a preferred preview port for test");
        let allocated =
            allocate_preview_local_public_port(attempt_id).expect("should find fallback port");
        assert_ne!(allocated, preferred);
    }

    #[test]
    fn detect_package_manager_prefers_known_lockfiles() {
        let base = create_temp_worktree("pkg-manager");

        let pnpm_repo = base.join("pnpm");
        fs::create_dir_all(&pnpm_repo).unwrap();
        fs::write(pnpm_repo.join("pnpm-lock.yaml"), "").unwrap();
        assert_eq!(detect_package_manager(&pnpm_repo), PackageManager::Pnpm);

        let yarn_repo = base.join("yarn");
        fs::create_dir_all(&yarn_repo).unwrap();
        fs::write(yarn_repo.join("yarn.lock"), "").unwrap();
        assert_eq!(detect_package_manager(&yarn_repo), PackageManager::Yarn);

        let bun_repo = base.join("bun");
        fs::create_dir_all(&bun_repo).unwrap();
        fs::write(bun_repo.join("bun.lockb"), "").unwrap();
        assert_eq!(detect_package_manager(&bun_repo), PackageManager::Bun);

        let npm_repo = base.join("npm");
        fs::create_dir_all(&npm_repo).unwrap();
        assert_eq!(detect_package_manager(&npm_repo), PackageManager::Npm);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn additional_preview_cli_args_does_not_duplicate_host_or_port() {
        let already_configured =
            additional_preview_cli_args("next dev --hostname 0.0.0.0 --port 4010", 4010);
        assert!(already_configured.is_empty());

        let add_both = additional_preview_cli_args("vite", 4173);
        assert_eq!(add_both, " -- --host 0.0.0.0 --port 4173");
    }

    #[test]
    fn parse_running_services_output_filters_empty_lines_and_whitespace() {
        let parsed = parse_running_services_output("\n dev-server \n\ncloudflared\n  \n");
        assert_eq!(
            parsed,
            vec!["dev-server".to_string(), "cloudflared".to_string()]
        );
    }

    #[test]
    fn runtime_services_ready_requires_dev_server_and_cloudflared() {
        assert!(!runtime_services_ready(&[], true));
        assert!(!runtime_services_ready(&["dev-server".to_string()], true));
        assert!(!runtime_services_ready(&["cloudflared".to_string()], true));
        assert!(runtime_services_ready(
            &["dev-server".to_string(), "cloudflared".to_string()],
            true
        ));
        assert!(runtime_services_ready(
            &[
                "cloudflared".to_string(),
                "dev-server".to_string(),
                "postgres".to_string(),
            ],
            true
        ));

        assert!(!runtime_services_ready(&["cloudflared".to_string()], false));
        assert!(runtime_services_ready(&["dev-server".to_string()], false));
    }

    #[test]
    fn resolve_preview_dev_image_detects_runtime_from_command_and_files() {
        let worktree = create_temp_worktree("image-detect");
        fs::write(worktree.join("go.mod"), "module example.com/demo\n").unwrap();

        assert_eq!(
            resolve_preview_dev_image(&worktree, "HOST=0.0.0.0 PORT=3000 go run ."),
            "golang:1.22-alpine"
        );
        assert_eq!(
            resolve_preview_dev_image(&worktree, "HOST=0.0.0.0 PORT=3000 python app.py"),
            "python:3.12-alpine"
        );
        assert_eq!(
            resolve_preview_dev_image(&worktree, "HOST=0.0.0.0 PORT=3000 cargo run"),
            "rust:1.77-alpine"
        );

        let _ = fs::remove_dir_all(worktree);
    }
}
