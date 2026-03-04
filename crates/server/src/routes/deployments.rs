//! Deployment API Routes
//!
//! Provides endpoints for:
//! - Triggering builds for task attempts
//! - Listing build artifacts
//! - Getting preview URLs
//! - Triggering production deployments
//! - Listing project deployments
//! - GitLab merge webhook for auto-deploy

use acpms_db::PgPool;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::{stream, Stream};
use serde::Deserialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::convert::Infallible;
use std::path::Path as FsPath;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::{lookup_host, TcpStream};
use tokio::process::Command;
use tokio::time::timeout;
use uuid::Uuid;

use crate::api::ApiResponse;
use crate::error::ApiError;
use crate::middleware::{AuthUser, Permission, RbacChecker};
use crate::services::deployment_worker_pool::DeploymentJob;
use crate::ssh::{run_ssh_command, SshAuth, SshContext};
use crate::state::AppState;
use acpms_db::models::{
    ArtifactResponse, BuildStartedResponse, CreateDeploymentEnvironmentRequest,
    DeploymentCheckResult, DeploymentConnectionTestResponse, DeploymentEnvironment,
    DeploymentEnvironmentSecretInput, DeploymentRelease, DeploymentResponse, DeploymentRun,
    DeploymentRunStatus, DeploymentSecretType, DeploymentSourceType, DeploymentTargetType,
    DeploymentTimelineEvent, DeploymentTimelineEventType, DeploymentTimelineStep,
    DeploymentTriggerType, ListDeploymentReleasesQuery, ListDeploymentRunsQuery,
    ListDeploymentsQuery, RollbackDeploymentRunRequest, StartDeploymentRunRequest,
    TriggerBuildRequest, TriggerDeployRequest, UpdateDeploymentEnvironmentRequest,
};
use acpms_services::EncryptionService;
use chrono::{DateTime, Utc};

/// Create deployment routes
pub fn create_routes() -> Router<AppState> {
    Router::new()
        // Build endpoints
        .route("/attempts/:id/build", post(trigger_build))
        .route("/attempts/:id/artifacts", get(list_artifacts))
        // Deployment environment endpoints
        .route(
            "/projects/:id/deployment-environments/ssh-keyscan",
            post(ssh_keyscan),
        )
        .route(
            "/projects/:id/deployment-environments",
            get(list_deployment_environments).post(create_deployment_environment),
        )
        .route(
            "/projects/:id/deployment-environments/:env_id",
            get(get_deployment_environment)
                .put(update_deployment_environment)
                .patch(update_deployment_environment)
                .delete(delete_deployment_environment),
        )
        .route(
            "/projects/:id/deployment-environments/:env_id/test-connection",
            post(test_deployment_environment_connection),
        )
        .route(
            "/projects/:id/deployment-environments/:env_id/test-domain",
            post(test_deployment_environment_domain),
        )
        .route(
            "/projects/:id/deployment-environments/:env_id/releases",
            get(list_deployment_releases_for_environment),
        )
        // Deployment run endpoints
        .route(
            "/projects/:id/deployment-environments/:env_id/deploy",
            post(start_deployment_run),
        )
        .route("/projects/:id/deployment-runs", get(list_deployment_runs))
        .route("/deployment-runs/:run_id", get(get_deployment_run))
        .route(
            "/deployment-runs/:run_id/logs",
            get(list_deployment_run_logs),
        )
        .route(
            "/deployment-runs/:run_id/timeline",
            get(list_deployment_run_timeline),
        )
        .route(
            "/deployment-runs/:run_id/stream",
            get(stream_deployment_run),
        )
        .route(
            "/deployment-runs/:run_id/cancel",
            post(cancel_deployment_run),
        )
        .route("/deployment-runs/:run_id/retry", post(retry_deployment_run))
        .route(
            "/deployment-runs/:run_id/rollback",
            post(rollback_deployment_run),
        )
        .route(
            "/deployment-releases/:release_id",
            get(get_deployment_release),
        )
        // Production deployment endpoints
        .route("/projects/:id/deploy", post(trigger_deploy))
        .route("/projects/:id/deployments", get(list_deployments))
        .route("/deployments/:id", get(get_deployment))
        // Webhook endpoint
        .route("/webhooks/gitlab/merge", post(handle_merge_webhook))
}

async fn get_project_id_by_attempt(pool: &PgPool, attempt_id: Uuid) -> Result<Uuid, ApiError> {
    let project_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT t.project_id
        FROM task_attempts ta
        JOIN tasks t ON t.id = ta.task_id
        WHERE ta.id = $1
        "#,
    )
    .bind(attempt_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    project_id.ok_or_else(|| ApiError::NotFound("Attempt not found".into()))
}

async fn get_project_id_by_deployment(
    pool: &PgPool,
    deployment_id: Uuid,
) -> Result<Uuid, ApiError> {
    let project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM production_deployments WHERE id = $1")
            .bind(deployment_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

    project_id.ok_or_else(|| ApiError::NotFound("Deployment not found".into()))
}

async fn get_project_id_by_deployment_run(pool: &PgPool, run_id: Uuid) -> Result<Uuid, ApiError> {
    let project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM deployment_runs WHERE id = $1")
            .bind(run_id)
            .fetch_optional(pool)
            .await
            .map_err(map_sqlx_error)?;

    project_id.ok_or_else(|| ApiError::NotFound("Deployment run not found".to_string()))
}

async fn get_project_id_by_deployment_release(
    pool: &PgPool,
    release_id: Uuid,
) -> Result<Uuid, ApiError> {
    let project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM deployment_releases WHERE id = $1")
            .bind(release_id)
            .fetch_optional(pool)
            .await
            .map_err(map_sqlx_error)?;

    project_id.ok_or_else(|| ApiError::NotFound("Deployment release not found".to_string()))
}

fn map_sqlx_error(err: sqlx::Error) -> ApiError {
    match &err {
        sqlx::Error::Database(db_err) => {
            if db_err.code().as_deref() == Some("23505") {
                return ApiError::Conflict("Resource already exists".to_string());
            }
            ApiError::Database(err)
        }
        _ => ApiError::Database(err),
    }
}

fn slugify_name(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    let mut previous_dash = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}

fn is_valid_domain_name(domain: &str) -> bool {
    let domain = domain.trim();
    if domain.is_empty() || domain.len() > 253 || !domain.contains('.') {
        return false;
    }

    for label in domain.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
        if !label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        {
            return false;
        }
    }

    true
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn validate_healthcheck_values(timeout_secs: i32, expected_status: i32) -> Result<(), ApiError> {
    if !(1..=600).contains(&timeout_secs) {
        return Err(ApiError::Validation(
            "healthcheck_timeout_secs must be between 1 and 600".to_string(),
        ));
    }

    if !(100..=599).contains(&expected_status) {
        return Err(ApiError::Validation(
            "healthcheck_expected_status must be between 100 and 599".to_string(),
        ));
    }

    Ok(())
}

fn extract_ssh_host_port_username(
    target_config: &Value,
) -> Result<(String, u16, String), ApiError> {
    let Some(obj) = target_config.as_object() else {
        return Err(ApiError::Validation(
            "target_config must be a JSON object for ssh_remote target".to_string(),
        ));
    };

    let host = obj
        .get("host")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    if host.is_empty() {
        return Err(ApiError::Validation(
            "target_config.host is required for ssh_remote".to_string(),
        ));
    }

    let username = obj
        .get("username")
        .or_else(|| obj.get("user"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    if username.is_empty() {
        return Err(ApiError::Validation(
            "target_config.username is required for ssh_remote".to_string(),
        ));
    }

    let port = obj.get("port").and_then(|v| v.as_u64()).unwrap_or(22);
    if port == 0 || port > 65535 {
        return Err(ApiError::Validation(
            "target_config.port must be between 1 and 65535".to_string(),
        ));
    }

    Ok((host, port as u16, username))
}

fn validate_target_config(
    target_type: DeploymentTargetType,
    target_config: &Value,
) -> Result<(), ApiError> {
    match target_type {
        DeploymentTargetType::Local => {
            if !target_config.is_object() {
                return Err(ApiError::Validation(
                    "target_config must be a JSON object".to_string(),
                ));
            }
            Ok(())
        }
        DeploymentTargetType::SshRemote => {
            extract_ssh_host_port_username(target_config)?;
            Ok(())
        }
    }
}

fn validate_domain_config(domain_config: &Value) -> Result<Option<String>, ApiError> {
    if !domain_config.is_object() {
        return Err(ApiError::Validation(
            "domain_config must be a JSON object".to_string(),
        ));
    }

    let primary_domain = domain_config
        .get("primary_domain")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string);

    if let Some(ref domain) = primary_domain {
        if !is_valid_domain_name(domain) {
            return Err(ApiError::Validation(
                "domain_config.primary_domain is not a valid domain".to_string(),
            ));
        }
    }

    Ok(primary_domain)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DomainFailurePolicy {
    SoftFail,
    HardFail,
}

#[derive(Debug, Clone)]
struct DomainMappingConfig {
    primary_domain: String,
    alias_domains: Vec<String>,
    proxy_provider: String,
    ssl_mode: String,
    failure_policy: DomainFailurePolicy,
    proxy_config_path: Option<String>,
    reload_command: Option<String>,
    healthcheck_path: String,
}

fn parse_domain_mapping_config(
    domain_config: &Value,
) -> Result<Option<DomainMappingConfig>, ApiError> {
    let primary_domain = validate_domain_config(domain_config)?;
    let Some(primary_domain) = primary_domain else {
        return Ok(None);
    };

    let Some(config_obj) = domain_config.as_object() else {
        return Err(ApiError::Validation(
            "domain_config must be a JSON object".to_string(),
        ));
    };

    let alias_domains = match config_obj.get("alias_domains") {
        Some(Value::Array(values)) => {
            let mut domains = Vec::new();
            for value in values {
                let Some(domain) = value.as_str().map(str::trim).filter(|v| !v.is_empty()) else {
                    return Err(ApiError::Validation(
                        "domain_config.alias_domains must only contain non-empty strings"
                            .to_string(),
                    ));
                };
                if !is_valid_domain_name(domain) {
                    return Err(ApiError::Validation(format!(
                        "domain_config.alias_domains contains invalid domain: {}",
                        domain
                    )));
                }
                domains.push(domain.to_string());
            }
            domains
        }
        Some(_) => {
            return Err(ApiError::Validation(
                "domain_config.alias_domains must be an array of strings".to_string(),
            ));
        }
        None => Vec::new(),
    };

    let proxy_provider = config_obj
        .get("proxy_provider")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "nginx".to_string());
    if !["nginx", "caddy", "traefik"].contains(&proxy_provider.as_str()) {
        return Err(ApiError::Validation(
            "domain_config.proxy_provider must be one of: nginx, caddy, traefik".to_string(),
        ));
    }

    let ssl_mode = config_obj
        .get("ssl_mode")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "off".to_string());
    if !["off", "manual", "letsencrypt"].contains(&ssl_mode.as_str()) {
        return Err(ApiError::Validation(
            "domain_config.ssl_mode must be one of: off, manual, letsencrypt".to_string(),
        ));
    }

    let failure_policy = match config_obj
        .get("failure_policy")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("hard_fail") => DomainFailurePolicy::HardFail,
        _ => DomainFailurePolicy::SoftFail,
    };

    let proxy_config_path = normalize_optional_string(
        config_obj
            .get("proxy_config_path")
            .and_then(|v| v.as_str().map(ToString::to_string)),
    );
    let reload_command = normalize_optional_string(
        config_obj
            .get("reload_command")
            .and_then(|v| v.as_str().map(ToString::to_string)),
    );

    let healthcheck_path = config_obj
        .get("healthcheck_path")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| "/".to_string());
    let healthcheck_path = if healthcheck_path.starts_with('/') {
        healthcheck_path
    } else {
        format!("/{}", healthcheck_path)
    };

    Ok(Some(DomainMappingConfig {
        primary_domain,
        alias_domains,
        proxy_provider,
        ssl_mode,
        failure_policy,
        proxy_config_path,
        reload_command,
        healthcheck_path,
    }))
}

fn domain_failure_policy_label(policy: DomainFailurePolicy) -> &'static str {
    match policy {
        DomainFailurePolicy::SoftFail => "soft_fail",
        DomainFailurePolicy::HardFail => "hard_fail",
    }
}

fn domain_failure_event_type(policy: DomainFailurePolicy) -> DeploymentTimelineEventType {
    match policy {
        DomainFailurePolicy::SoftFail => DeploymentTimelineEventType::Warning,
        DomainFailurePolicy::HardFail => DeploymentTimelineEventType::Error,
    }
}

fn render_proxy_template(config: &DomainMappingConfig, deploy_path: &str) -> String {
    let domains = std::iter::once(config.primary_domain.as_str())
        .chain(config.alias_domains.iter().map(|v| v.as_str()))
        .collect::<Vec<_>>()
        .join(" ");

    match config.proxy_provider.as_str() {
        "nginx" => format!(
            "server {{\n    listen 80;\n    server_name {domains};\n    root {deploy_path};\n    location / {{\n        try_files $uri $uri/ /index.html;\n    }}\n}}\n"
        ),
        "caddy" => format!(
            "{domains} {{\n    root * {deploy_path}\n    encode zstd gzip\n    file_server\n}}\n"
        ),
        "traefik" => format!(
            "# static snippet for traefik file provider\nhttp:\n  routers:\n    acpms-router:\n      rule: \"Host(`{}`)\"\n      service: acpms-service\n  services:\n    acpms-service:\n      loadBalancer:\n        servers:\n          - url: \"http://127.0.0.1:8080\"\n",
            config.primary_domain
        ),
        _ => format!("# Unsupported proxy provider for domains: {}\n", domains),
    }
}

async fn run_local_shell_command(command: &str, timeout_duration: Duration) -> Result<(), String> {
    let output = timeout(
        timeout_duration,
        Command::new("sh").arg("-lc").arg(command).output(),
    )
    .await
    .map_err(|_| "Command timed out".to_string())?
    .map_err(|err| format!("Cannot execute command: {}", err))?;

    if !output.status.success() {
        let stderr = sanitize_command_output(&String::from_utf8_lossy(&output.stderr));
        let stdout = sanitize_command_output(&String::from_utf8_lossy(&output.stdout));
        let details = if stderr != "no output" {
            stderr
        } else {
            stdout
        };
        return Err(format!(
            "Command failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            details
        ));
    }

    Ok(())
}

fn domain_healthcheck_port(ssl_mode: &str) -> u16 {
    match ssl_mode {
        "manual" | "letsencrypt" => 443,
        _ => 80,
    }
}

fn build_remote_file_path(base_path: &str, file_name: &str) -> String {
    format!("{}/{}", base_path.trim_end_matches('/'), file_name)
}

async fn verify_domain_healthcheck_connectivity(
    config: &DomainMappingConfig,
) -> Result<(), String> {
    let port = domain_healthcheck_port(&config.ssl_mode);
    let mut addrs = timeout(
        Duration::from_secs(5),
        lookup_host((config.primary_domain.as_str(), port)),
    )
    .await
    .map_err(|_| "DNS lookup timed out".to_string())?
    .map_err(|err| format!("DNS lookup failed: {}", err))?;

    let addr = addrs
        .next()
        .ok_or_else(|| "DNS lookup returned no addresses".to_string())?;

    timeout(Duration::from_secs(5), TcpStream::connect(addr))
        .await
        .map_err(|_| "Domain TCP healthcheck timed out".to_string())?
        .map_err(|err| format!("Cannot connect to domain endpoint: {}", err))?;

    Ok(())
}

fn encryption_service_from_env() -> Result<EncryptionService, ApiError> {
    let key = std::env::var("ENCRYPTION_KEY")
        .map_err(|_| ApiError::Internal("ENCRYPTION_KEY is not configured".to_string()))?;
    EncryptionService::new(&key).map_err(|e| ApiError::Internal(e.to_string()))
}

async fn upsert_environment_secrets(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    environment_id: Uuid,
    secrets: &[DeploymentEnvironmentSecretInput],
) -> Result<(), ApiError> {
    if secrets.is_empty() {
        return Ok(());
    }

    let encryption = encryption_service_from_env()?;

    for secret in secrets {
        let secret_value = secret.value.trim();
        if secret_value.is_empty() {
            return Err(ApiError::Validation(
                "Secret value cannot be empty".to_string(),
            ));
        }

        let ciphertext = encryption
            .encrypt(secret_value)
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO deployment_environment_secrets (environment_id, secret_type, ciphertext)
            VALUES ($1, $2, $3)
            ON CONFLICT (environment_id, secret_type)
            DO UPDATE SET ciphertext = EXCLUDED.ciphertext, updated_at = NOW()
            "#,
        )
        .bind(environment_id)
        .bind(secret.secret_type)
        .bind(ciphertext)
        .execute(&mut **tx)
        .await
        .map_err(map_sqlx_error)?;
    }

    Ok(())
}

#[derive(Default)]
struct DecryptedSshSecrets {
    private_key: Option<String>,
    password: Option<String>,
    known_hosts: Option<String>,
}

// SshContext, SshAuth, SshCommandOutput, run_ssh_command from crate::ssh

fn shell_escape_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn sanitize_command_output(output: &str) -> String {
    let normalized = output.replace(['\n', '\r'], " ");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return "no output".to_string();
    }

    const MAX_LEN: usize = 240;
    if trimmed.len() > MAX_LEN {
        format!("{}...", &trimmed[..MAX_LEN])
    } else {
        trimmed.to_string()
    }
}

fn build_remote_deploy_script(deploy_path: &str, context_json: &str, marker_text: &str) -> String {
    let deploy_path_q = shell_escape_single_quoted(deploy_path);
    let context_q = shell_escape_single_quoted(context_json);
    let marker_q = shell_escape_single_quoted(marker_text);
    format!(
        "set -euo pipefail; \
         mkdir -p {deploy_path}; \
         printf '%s' {context} > {deploy_path}/.acpms-deploy-context.json; \
         printf '%s' {marker} > {deploy_path}/.acpms-last-deploy.txt",
        deploy_path = deploy_path_q,
        context = context_q,
        marker = marker_q
    )
}

fn build_remote_rollback_script(deploy_path: &str, marker_text: &str) -> String {
    let deploy_path_q = shell_escape_single_quoted(deploy_path);
    let marker_q = shell_escape_single_quoted(marker_text);
    format!(
        "set -euo pipefail; \
         mkdir -p {deploy_path}; \
         printf '%s' {marker} > {deploy_path}/.acpms-last-rollback.txt",
        deploy_path = deploy_path_q,
        marker = marker_q
    )
}

#[derive(sqlx::FromRow)]
struct DeploymentSecretRow {
    secret_type: DeploymentSecretType,
    ciphertext: String,
}

fn normalize_multiline_ssh_secret(secret: &str) -> Option<String> {
    let normalized = secret.replace("\r\n", "\n");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(format!("{}\n", trimmed))
}

#[cfg(test)]
mod deployments_secret_tests {
    use super::normalize_multiline_ssh_secret;

    #[test]
    fn normalize_multiline_ssh_secret_preserves_lines_and_appends_newline() {
        let key = "-----BEGIN OPENSSH PRIVATE KEY-----\nline-1\nline-2\n-----END OPENSSH PRIVATE KEY-----";
        let normalized = normalize_multiline_ssh_secret(key).expect("should normalize key");
        assert!(normalized.ends_with('\n'));
        assert!(normalized.contains("line-1\nline-2"));
    }

    #[test]
    fn normalize_multiline_ssh_secret_rejects_empty_values() {
        assert!(normalize_multiline_ssh_secret(" \n\t\r\n ").is_none());
    }
}

async fn load_decrypted_ssh_secrets(
    pool: &PgPool,
    environment_id: Uuid,
) -> Result<DecryptedSshSecrets, ApiError> {
    let rows = sqlx::query_as::<_, DeploymentSecretRow>(
        r#"
        SELECT secret_type, ciphertext
        FROM deployment_environment_secrets
        WHERE environment_id = $1
        "#,
    )
    .bind(environment_id)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)?;

    if rows.is_empty() {
        return Ok(DecryptedSshSecrets::default());
    }

    let encryption = encryption_service_from_env()?;
    let mut secrets = DecryptedSshSecrets::default();

    for row in rows {
        let decrypted = encryption.decrypt(&row.ciphertext).map_err(|e| {
            ApiError::Internal(format!("Failed to decrypt deployment secret: {}", e))
        })?;

        match row.secret_type {
            DeploymentSecretType::SshPrivateKey => {
                if let Some(value) = normalize_multiline_ssh_secret(&decrypted) {
                    secrets.private_key = Some(value);
                }
            }
            DeploymentSecretType::SshPassword => {
                let trimmed = decrypted.trim();
                if !trimmed.is_empty() {
                    secrets.password = Some(trimmed.to_string());
                }
            }
            DeploymentSecretType::KnownHosts => {
                if let Some(value) = normalize_multiline_ssh_secret(&decrypted) {
                    secrets.known_hosts = Some(value);
                }
            }
            _ => {}
        }
    }

    Ok(secrets)
}

async fn prepare_ssh_execution_context(
    pool: &PgPool,
    environment: &DeploymentEnvironment,
) -> Result<SshContext, ApiError> {
    let (host, port, username) = extract_ssh_host_port_username(&environment.target_config)?;
    let secrets = load_decrypted_ssh_secrets(pool, environment.id).await?;

    let known_hosts_content = secrets.known_hosts.ok_or_else(|| {
        ApiError::Validation(
            "Missing required secret: known_hosts (host verification policy requires known_hosts)"
                .to_string(),
        )
    })?;

    let private_key_content = secrets.private_key;
    let password = secrets.password;
    if private_key_content.is_none() && password.is_none() {
        return Err(ApiError::Validation(
            "Missing SSH credentials: provide ssh_private_key or ssh_password".to_string(),
        ));
    }

    // Use private key first when both exist; password as fallback if key fails
    let (auth, fallback_password) = if let Some(key_content) = private_key_content {
        let auth = SshAuth::PrivateKey {
            key_content: key_content,
        };
        let fallback = password;
        (auth, fallback)
    } else if let Some(pwd) = password {
        (SshAuth::Password { password: pwd }, None)
    } else {
        return Err(ApiError::Validation(
            "Missing SSH credentials: provide ssh_private_key or ssh_password".to_string(),
        ));
    };

    Ok(SshContext {
        host,
        port,
        username,
        known_hosts_content,
        auth,
        fallback_password,
    })
}

// run_ssh_command from crate::ssh

async fn get_project_deployment_environment(
    pool: &PgPool,
    project_id: Uuid,
    env_id: Uuid,
) -> Result<DeploymentEnvironment, ApiError> {
    sqlx::query_as::<_, DeploymentEnvironment>(
        "SELECT * FROM deployment_environments WHERE project_id = $1 AND id = $2",
    )
    .bind(project_id)
    .bind(env_id)
    .fetch_optional(pool)
    .await
    .map_err(map_sqlx_error)?
    .ok_or_else(|| ApiError::NotFound("Deployment environment not found".to_string()))
}

fn deployment_source_type_label(source_type: DeploymentSourceType) -> &'static str {
    match source_type {
        DeploymentSourceType::Branch => "branch",
        DeploymentSourceType::Commit => "commit",
        DeploymentSourceType::Artifact => "artifact",
        DeploymentSourceType::Release => "release",
    }
}

fn deployment_status_label(status: DeploymentRunStatus) -> &'static str {
    match status {
        DeploymentRunStatus::Queued => "queued",
        DeploymentRunStatus::Running => "running",
        DeploymentRunStatus::Success => "success",
        DeploymentRunStatus::Failed => "failed",
        DeploymentRunStatus::Cancelled => "cancelled",
        DeploymentRunStatus::RollingBack => "rolling_back",
        DeploymentRunStatus::RolledBack => "rolled_back",
    }
}

fn deployment_timeline_step_label(step: DeploymentTimelineStep) -> &'static str {
    match step {
        DeploymentTimelineStep::Precheck => "precheck",
        DeploymentTimelineStep::Connect => "connect",
        DeploymentTimelineStep::Prepare => "prepare",
        DeploymentTimelineStep::Deploy => "deploy",
        DeploymentTimelineStep::DomainConfig => "domain_config",
        DeploymentTimelineStep::Healthcheck => "healthcheck",
        DeploymentTimelineStep::Finalize => "finalize",
        DeploymentTimelineStep::Rollback => "rollback",
    }
}

fn rollback_result_label(status: DeploymentRunStatus) -> &'static str {
    match status {
        DeploymentRunStatus::RolledBack => "success",
        DeploymentRunStatus::Failed => "failed",
        DeploymentRunStatus::Cancelled => "cancelled",
        _ => "other",
    }
}

fn is_terminal_deployment_status(status: DeploymentRunStatus) -> bool {
    matches!(
        status,
        DeploymentRunStatus::Success
            | DeploymentRunStatus::Failed
            | DeploymentRunStatus::Cancelled
            | DeploymentRunStatus::RolledBack
    )
}

#[derive(sqlx::FromRow)]
struct DeploymentRunMetricSnapshot {
    status: DeploymentRunStatus,
    trigger_type: DeploymentTriggerType,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    environment_slug: String,
}

async fn deployment_run_metric_snapshot(
    pool: &PgPool,
    run_id: Uuid,
) -> Result<Option<DeploymentRunMetricSnapshot>, ApiError> {
    sqlx::query_as::<_, DeploymentRunMetricSnapshot>(
        r#"
        SELECT
            r.status,
            r.trigger_type,
            r.started_at,
            r.completed_at,
            e.slug AS environment_slug
        FROM deployment_runs r
        JOIN deployment_environments e ON e.id = r.environment_id
        WHERE r.id = $1
        "#,
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(map_sqlx_error)
}

async fn environment_slug_by_id(pool: &PgPool, environment_id: Uuid) -> Result<String, ApiError> {
    sqlx::query_scalar("SELECT slug FROM deployment_environments WHERE id = $1")
        .bind(environment_id)
        .fetch_one(pool)
        .await
        .map_err(map_sqlx_error)
}

fn observe_deployment_duration_if_available(
    state: &AppState,
    environment_slug: &str,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
) {
    let Some(started_at) = started_at else {
        return;
    };

    let end = completed_at.unwrap_or_else(Utc::now);
    let duration_secs = (end - started_at).num_milliseconds() as f64 / 1000.0;
    if duration_secs.is_finite() && duration_secs >= 0.0 {
        state
            .metrics
            .deployment_run_duration_seconds
            .with_label_values(&[environment_slug])
            .observe(duration_secs);
    }
}

async fn record_deployment_run_metrics(
    state: &AppState,
    run_id: Uuid,
    failure_step: Option<DeploymentTimelineStep>,
) -> Result<(), ApiError> {
    let Some(snapshot) = deployment_run_metric_snapshot(&state.db, run_id).await? else {
        return Ok(());
    };

    state
        .metrics
        .deployment_runs_total
        .with_label_values(&[
            deployment_status_label(snapshot.status),
            snapshot.environment_slug.as_str(),
        ])
        .inc();

    observe_deployment_duration_if_available(
        state,
        snapshot.environment_slug.as_str(),
        snapshot.started_at,
        snapshot.completed_at,
    );

    if snapshot.status == DeploymentRunStatus::Failed {
        let step_label = failure_step
            .map(deployment_timeline_step_label)
            .unwrap_or("unknown");
        state
            .metrics
            .deployment_failures_total
            .with_label_values(&[step_label, snapshot.environment_slug.as_str()])
            .inc();
    }

    if snapshot.trigger_type == DeploymentTriggerType::Rollback
        && is_terminal_deployment_status(snapshot.status)
    {
        state
            .metrics
            .rollback_runs_total
            .with_label_values(&[
                rollback_result_label(snapshot.status),
                snapshot.environment_slug.as_str(),
            ])
            .inc();
    }

    Ok(())
}

fn record_queued_deployment_metric(state: &AppState, environment_slug: &str) {
    state
        .metrics
        .deployment_runs_total
        .with_label_values(&["queued", environment_slug])
        .inc();
}

async fn append_deployment_timeline_event(
    pool: &PgPool,
    run_id: Uuid,
    step: DeploymentTimelineStep,
    event_type: DeploymentTimelineEventType,
    message: &str,
    payload: Value,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO deployment_timeline_events (run_id, step, event_type, message, payload)
        VALUES ($1, $2, $3, $4, $5::jsonb)
        "#,
    )
    .bind(run_id)
    .bind(step)
    .bind(event_type)
    .bind(message)
    .bind(payload)
    .execute(pool)
    .await
    .map_err(map_sqlx_error)?;

    Ok(())
}

async fn append_deployment_audit_event(
    pool: &PgPool,
    user_id: Option<Uuid>,
    action: &str,
    run_id: Uuid,
    project_id: Uuid,
    environment_id: Uuid,
    metadata: Value,
) -> Result<(), ApiError> {
    let mut payload = match metadata {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    payload.insert("run_id".to_string(), serde_json::json!(run_id));
    payload.insert("project_id".to_string(), serde_json::json!(project_id));
    payload.insert(
        "environment_id".to_string(),
        serde_json::json!(environment_id),
    );

    sqlx::query(
        r#"
        INSERT INTO audit_logs (user_id, action, resource_type, resource_id, metadata)
        VALUES ($1, $2, 'deployment_runs', $3, $4::jsonb)
        "#,
    )
    .bind(user_id)
    .bind(action)
    .bind(run_id)
    .bind(Value::Object(payload))
    .execute(pool)
    .await
    .map_err(map_sqlx_error)?;

    Ok(())
}

async fn append_environment_audit_event(
    pool: &PgPool,
    user_id: Uuid,
    action: &str,
    environment: &DeploymentEnvironment,
    metadata: Value,
) -> Result<(), ApiError> {
    let mut payload = match metadata {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    payload.insert(
        "project_id".to_string(),
        serde_json::json!(environment.project_id),
    );
    payload.insert(
        "environment_id".to_string(),
        serde_json::json!(environment.id),
    );

    sqlx::query(
        r#"
        INSERT INTO audit_logs (user_id, action, resource_type, resource_id, metadata)
        VALUES ($1, $2, 'deployment_environments', $3, $4::jsonb)
        "#,
    )
    .bind(user_id)
    .bind(action)
    .bind(environment.id)
    .bind(Value::Object(payload))
    .execute(pool)
    .await
    .map_err(map_sqlx_error)?;

    Ok(())
}

async fn apply_local_domain_mapping(
    pool: &PgPool,
    run_id: Uuid,
    environment: &DeploymentEnvironment,
    deploy_path: &FsPath,
) -> Result<Option<String>, ApiError> {
    let config = match parse_domain_mapping_config(&environment.domain_config)? {
        Some(config) => config,
        None => {
            append_deployment_timeline_event(
                pool,
                run_id,
                DeploymentTimelineStep::DomainConfig,
                DeploymentTimelineEventType::System,
                "No primary domain configured; skipping domain mapping",
                serde_json::json!({}),
            )
            .await?;
            return Ok(None);
        }
    };

    append_deployment_timeline_event(
        pool,
        run_id,
        DeploymentTimelineStep::DomainConfig,
        DeploymentTimelineEventType::System,
        "Applying domain mapping configuration",
        serde_json::json!({
            "primary_domain": config.primary_domain,
            "alias_domains": config.alias_domains,
            "proxy_provider": config.proxy_provider,
            "ssl_mode": config.ssl_mode,
            "failure_policy": domain_failure_policy_label(config.failure_policy),
        }),
    )
    .await?;

    let template = render_proxy_template(&config, &environment.deploy_path);
    let staged_template_path =
        deploy_path.join(format!(".acpms-proxy-{}.conf", config.proxy_provider));
    tokio::fs::write(&staged_template_path, template)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    append_deployment_timeline_event(
        pool,
        run_id,
        DeploymentTimelineStep::DomainConfig,
        DeploymentTimelineEventType::System,
        "Rendered proxy template",
        serde_json::json!({
            "staged_template_path": staged_template_path.to_string_lossy(),
            "proxy_provider": config.proxy_provider,
        }),
    )
    .await?;

    let mut backup_path: Option<PathBuf> = None;
    if let Some(proxy_config_path) = &config.proxy_config_path {
        let proxy_path = PathBuf::from(proxy_config_path);
        if let Some(parent) = proxy_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
        }

        if tokio::fs::metadata(&proxy_path).await.is_ok() {
            let backup = PathBuf::from(format!("{}.acpms.bak.{}", proxy_config_path, run_id));
            tokio::fs::copy(&proxy_path, &backup)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            backup_path = Some(backup);
        }

        tokio::fs::copy(&staged_template_path, &proxy_path)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        append_deployment_timeline_event(
            pool,
            run_id,
            DeploymentTimelineStep::DomainConfig,
            DeploymentTimelineEventType::System,
            "Updated proxy config file",
            serde_json::json!({ "proxy_config_path": proxy_config_path }),
        )
        .await?;
    }

    if let Some(reload_command) = &config.reload_command {
        if let Err(err) = run_local_shell_command(reload_command, Duration::from_secs(45)).await {
            if let (Some(proxy_config_path), Some(backup_path)) =
                (config.proxy_config_path.as_deref(), backup_path.as_ref())
            {
                let _ = tokio::fs::copy(backup_path, proxy_config_path).await;
            }

            let message = format!("Domain reload command failed: {}", err);
            append_deployment_timeline_event(
                pool,
                run_id,
                DeploymentTimelineStep::DomainConfig,
                domain_failure_event_type(config.failure_policy),
                &message,
                serde_json::json!({
                    "reload_command": reload_command,
                    "failure_policy": domain_failure_policy_label(config.failure_policy),
                }),
            )
            .await?;

            if config.failure_policy == DomainFailurePolicy::HardFail {
                return Ok(Some(message));
            }
        } else {
            append_deployment_timeline_event(
                pool,
                run_id,
                DeploymentTimelineStep::DomainConfig,
                DeploymentTimelineEventType::System,
                "Proxy service reloaded successfully",
                serde_json::json!({ "reload_command": reload_command }),
            )
            .await?;
        }
    } else if config.proxy_config_path.is_some() {
        append_deployment_timeline_event(
            pool,
            run_id,
            DeploymentTimelineStep::DomainConfig,
            DeploymentTimelineEventType::Warning,
            "Proxy config updated without reload command; manual reload required",
            serde_json::json!({}),
        )
        .await?;
    }

    match verify_domain_healthcheck_connectivity(&config).await {
        Ok(()) => {
            append_deployment_timeline_event(
                pool,
                run_id,
                DeploymentTimelineStep::DomainConfig,
                DeploymentTimelineEventType::System,
                "Domain connectivity healthcheck passed",
                serde_json::json!({
                    "domain": config.primary_domain,
                    "healthcheck_path": config.healthcheck_path,
                    "port": domain_healthcheck_port(&config.ssl_mode),
                }),
            )
            .await?;
        }
        Err(err) => {
            let message = format!("Domain healthcheck failed: {}", err);
            append_deployment_timeline_event(
                pool,
                run_id,
                DeploymentTimelineStep::DomainConfig,
                domain_failure_event_type(config.failure_policy),
                &message,
                serde_json::json!({
                    "domain": config.primary_domain,
                    "healthcheck_path": config.healthcheck_path,
                    "port": domain_healthcheck_port(&config.ssl_mode),
                    "failure_policy": domain_failure_policy_label(config.failure_policy),
                }),
            )
            .await?;
            if config.failure_policy == DomainFailurePolicy::HardFail {
                return Ok(Some(message));
            }
        }
    }

    Ok(None)
}

async fn apply_ssh_domain_mapping(
    pool: &PgPool,
    run_id: Uuid,
    environment: &DeploymentEnvironment,
    ssh_context: &SshContext,
) -> Result<Option<String>, ApiError> {
    let config = match parse_domain_mapping_config(&environment.domain_config)? {
        Some(config) => config,
        None => {
            append_deployment_timeline_event(
                pool,
                run_id,
                DeploymentTimelineStep::DomainConfig,
                DeploymentTimelineEventType::System,
                "No primary domain configured; skipping domain mapping",
                serde_json::json!({}),
            )
            .await?;
            return Ok(None);
        }
    };

    append_deployment_timeline_event(
        pool,
        run_id,
        DeploymentTimelineStep::DomainConfig,
        DeploymentTimelineEventType::System,
        "Applying domain mapping on SSH target",
        serde_json::json!({
            "primary_domain": config.primary_domain,
            "alias_domains": config.alias_domains,
            "proxy_provider": config.proxy_provider,
            "ssl_mode": config.ssl_mode,
            "failure_policy": domain_failure_policy_label(config.failure_policy),
            "host": ssh_context.host,
        }),
    )
    .await?;

    let staged_remote_path = build_remote_file_path(
        &environment.deploy_path,
        &format!(".acpms-proxy-{}.conf", config.proxy_provider),
    );
    let template = render_proxy_template(&config, &environment.deploy_path);
    let write_staged_script = format!(
        "set -euo pipefail; mkdir -p {deploy_path}; printf '%s' {template} > {staged_path}",
        deploy_path = shell_escape_single_quoted(&environment.deploy_path),
        template = shell_escape_single_quoted(&template),
        staged_path = shell_escape_single_quoted(&staged_remote_path),
    );
    if let Err(err) =
        run_ssh_command(ssh_context, &write_staged_script, Duration::from_secs(45)).await
    {
        let message = format!("Failed to stage remote proxy template: {}", err);
        append_deployment_timeline_event(
            pool,
            run_id,
            DeploymentTimelineStep::DomainConfig,
            domain_failure_event_type(config.failure_policy),
            &message,
            serde_json::json!({
                "staged_path": staged_remote_path,
                "failure_policy": domain_failure_policy_label(config.failure_policy),
            }),
        )
        .await?;
        if config.failure_policy == DomainFailurePolicy::HardFail {
            return Ok(Some(message));
        }
    }

    if let Some(proxy_config_path) = &config.proxy_config_path {
        let backup_path = format!("{}.acpms.bak.{}", proxy_config_path, run_id);
        let apply_remote_script = if let Some(reload_command) = &config.reload_command {
            format!(
                "set -euo pipefail; \
                 target={target}; staged={staged}; backup={backup}; \
                 if [ -f \"$target\" ]; then cp \"$target\" \"$backup\"; fi; \
                 cp \"$staged\" \"$target\"; \
                 if ! sh -lc {reload}; then \
                     if [ -f \"$backup\" ]; then cp \"$backup\" \"$target\"; fi; \
                     exit 1; \
                 fi",
                target = shell_escape_single_quoted(proxy_config_path),
                staged = shell_escape_single_quoted(&staged_remote_path),
                backup = shell_escape_single_quoted(&backup_path),
                reload = shell_escape_single_quoted(reload_command),
            )
        } else {
            format!(
                "set -euo pipefail; cp {staged} {target}",
                staged = shell_escape_single_quoted(&staged_remote_path),
                target = shell_escape_single_quoted(proxy_config_path),
            )
        };

        if let Err(err) =
            run_ssh_command(ssh_context, &apply_remote_script, Duration::from_secs(60)).await
        {
            let message = format!("Failed to apply remote proxy config: {}", err);
            append_deployment_timeline_event(
                pool,
                run_id,
                DeploymentTimelineStep::DomainConfig,
                domain_failure_event_type(config.failure_policy),
                &message,
                serde_json::json!({
                    "proxy_config_path": proxy_config_path,
                    "failure_policy": domain_failure_policy_label(config.failure_policy),
                }),
            )
            .await?;
            if config.failure_policy == DomainFailurePolicy::HardFail {
                return Ok(Some(message));
            }
        } else {
            append_deployment_timeline_event(
                pool,
                run_id,
                DeploymentTimelineStep::DomainConfig,
                DeploymentTimelineEventType::System,
                "Remote proxy config updated successfully",
                serde_json::json!({ "proxy_config_path": proxy_config_path }),
            )
            .await?;
        }

        if config.reload_command.is_none() {
            append_deployment_timeline_event(
                pool,
                run_id,
                DeploymentTimelineStep::DomainConfig,
                DeploymentTimelineEventType::Warning,
                "Remote proxy config updated without reload command; manual reload required",
                serde_json::json!({}),
            )
            .await?;
        }
    } else if let Some(reload_command) = &config.reload_command {
        if let Err(err) = run_ssh_command(
            ssh_context,
            &format!("sh -lc {}", shell_escape_single_quoted(reload_command)),
            Duration::from_secs(45),
        )
        .await
        {
            let message = format!("Remote reload command failed: {}", err);
            append_deployment_timeline_event(
                pool,
                run_id,
                DeploymentTimelineStep::DomainConfig,
                domain_failure_event_type(config.failure_policy),
                &message,
                serde_json::json!({
                    "reload_command": reload_command,
                    "failure_policy": domain_failure_policy_label(config.failure_policy),
                }),
            )
            .await?;
            if config.failure_policy == DomainFailurePolicy::HardFail {
                return Ok(Some(message));
            }
        } else {
            append_deployment_timeline_event(
                pool,
                run_id,
                DeploymentTimelineStep::DomainConfig,
                DeploymentTimelineEventType::System,
                "Remote proxy service reloaded successfully",
                serde_json::json!({ "reload_command": reload_command }),
            )
            .await?;
        }
    }

    match verify_domain_healthcheck_connectivity(&config).await {
        Ok(()) => {
            append_deployment_timeline_event(
                pool,
                run_id,
                DeploymentTimelineStep::DomainConfig,
                DeploymentTimelineEventType::System,
                "Domain connectivity healthcheck passed",
                serde_json::json!({
                    "domain": config.primary_domain,
                    "healthcheck_path": config.healthcheck_path,
                    "port": domain_healthcheck_port(&config.ssl_mode),
                }),
            )
            .await?;
        }
        Err(err) => {
            let message = format!("Domain healthcheck failed: {}", err);
            append_deployment_timeline_event(
                pool,
                run_id,
                DeploymentTimelineStep::DomainConfig,
                domain_failure_event_type(config.failure_policy),
                &message,
                serde_json::json!({
                    "domain": config.primary_domain,
                    "healthcheck_path": config.healthcheck_path,
                    "port": domain_healthcheck_port(&config.ssl_mode),
                    "failure_policy": domain_failure_policy_label(config.failure_policy),
                }),
            )
            .await?;
            if config.failure_policy == DomainFailurePolicy::HardFail {
                return Ok(Some(message));
            }
        }
    }

    Ok(None)
}

async fn is_run_cancelled(pool: &PgPool, run_id: Uuid) -> Result<bool, ApiError> {
    let status: Option<DeploymentRunStatus> =
        sqlx::query_scalar("SELECT status FROM deployment_runs WHERE id = $1")
            .bind(run_id)
            .fetch_optional(pool)
            .await
            .map_err(map_sqlx_error)?;

    Ok(matches!(status, Some(DeploymentRunStatus::Cancelled)))
}

async fn mark_deployment_run_failed(
    pool: &PgPool,
    run_id: Uuid,
    error_message: &str,
) -> Result<bool, ApiError> {
    let result = sqlx::query(
        r#"
        UPDATE deployment_runs
        SET
            status = 'failed',
            error_message = $2,
            completed_at = COALESCE(completed_at, NOW()),
            updated_at = NOW()
        WHERE id = $1
          AND status IN ('queued', 'running', 'rolling_back')
        "#,
    )
    .bind(run_id)
    .bind(error_message)
    .execute(pool)
    .await
    .map_err(map_sqlx_error)?;

    Ok(result.rows_affected() > 0)
}

async fn mark_deployment_run_success(pool: &PgPool, run_id: Uuid) -> Result<bool, ApiError> {
    #[derive(sqlx::FromRow)]
    struct UpdatedRun {
        id: Uuid,
        project_id: Uuid,
        environment_id: Uuid,
        source_type: DeploymentSourceType,
        source_ref: Option<String>,
    }

    let mut tx = pool.begin().await.map_err(map_sqlx_error)?;

    let updated_run = sqlx::query_as::<_, UpdatedRun>(
        r#"
        UPDATE deployment_runs
        SET status = 'success', completed_at = COALESCE(completed_at, NOW()), updated_at = NOW()
        WHERE id = $1 AND status = 'running'
        RETURNING id, project_id, environment_id, source_type, source_ref
        "#,
    )
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_sqlx_error)?;

    let Some(updated_run) = updated_run else {
        tx.commit().await.map_err(map_sqlx_error)?;
        return Ok(false);
    };

    let env_slug: String =
        sqlx::query_scalar("SELECT slug FROM deployment_environments WHERE id = $1")
            .bind(updated_run.environment_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;

    sqlx::query(
        "UPDATE deployment_releases SET status = 'superseded', updated_at = NOW() WHERE environment_id = $1 AND status = 'active'",
    )
    .bind(updated_run.environment_id)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx_error)?;

    let version_label = format!("{}-{}", env_slug, Utc::now().format("%Y%m%d%H%M%S"));
    let source_ref = updated_run.source_ref.clone();
    let artifact_ref = match updated_run.source_type {
        DeploymentSourceType::Artifact => source_ref.clone(),
        _ => None,
    };
    let git_commit_sha = match updated_run.source_type {
        DeploymentSourceType::Commit => source_ref.clone(),
        _ => None,
    };
    let metadata = serde_json::json!({
        "run_id": updated_run.id,
        "source_type": deployment_source_type_label(updated_run.source_type),
        "source_ref": source_ref,
    });

    sqlx::query(
        r#"
        INSERT INTO deployment_releases (
            project_id, environment_id, run_id, version_label, artifact_ref, git_commit_sha, status, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, 'active', $7::jsonb)
        "#,
    )
    .bind(updated_run.project_id)
    .bind(updated_run.environment_id)
    .bind(updated_run.id)
    .bind(version_label)
    .bind(artifact_ref)
    .bind(git_commit_sha)
    .bind(metadata)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx_error)?;

    tx.commit().await.map_err(map_sqlx_error)?;
    Ok(true)
}

async fn mark_deployment_run_rolled_back(
    pool: &PgPool,
    run_id: Uuid,
    target_release_id: Uuid,
) -> Result<bool, ApiError> {
    #[derive(sqlx::FromRow)]
    struct UpdatedRun {
        id: Uuid,
        environment_id: Uuid,
    }

    let mut tx = pool.begin().await.map_err(map_sqlx_error)?;

    let updated_run = sqlx::query_as::<_, UpdatedRun>(
        r#"
        UPDATE deployment_runs
        SET status = 'rolled_back', completed_at = COALESCE(completed_at, NOW()), updated_at = NOW()
        WHERE id = $1 AND status = 'rolling_back'
        RETURNING id, environment_id
        "#,
    )
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_sqlx_error)?;

    let Some(updated_run) = updated_run else {
        tx.commit().await.map_err(map_sqlx_error)?;
        return Ok(false);
    };

    sqlx::query(
        r#"
        UPDATE deployment_releases
        SET status = 'rolled_back', updated_at = NOW()
        WHERE environment_id = $1
          AND status = 'active'
          AND id <> $2
        "#,
    )
    .bind(updated_run.environment_id)
    .bind(target_release_id)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx_error)?;

    sqlx::query(
        r#"
        UPDATE deployment_releases
        SET
            status = 'active',
            metadata = COALESCE(metadata, '{}'::jsonb) || jsonb_build_object('last_rollback_run_id', $2::text),
            updated_at = NOW()
        WHERE id = $1 AND environment_id = $3
        "#,
    )
    .bind(target_release_id)
    .bind(updated_run.id)
    .bind(updated_run.environment_id)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx_error)?;

    tx.commit().await.map_err(map_sqlx_error)?;
    Ok(true)
}

pub(crate) async fn process_deployment_run_background(state: AppState, run_id: Uuid) {
    // Mimic queue pickup behavior: keep a short queued window for cancel action.
    tokio::time::sleep(Duration::from_millis(200)).await;

    if let Err(err) = process_deployment_run(state.clone(), run_id).await {
        tracing::error!(
            "Deployment run {} failed in background processor: {}",
            run_id,
            err
        );
        let _ = append_deployment_timeline_event(
            &state.db,
            run_id,
            DeploymentTimelineStep::Finalize,
            DeploymentTimelineEventType::Error,
            "Deployment processor encountered an internal error",
            serde_json::json!({ "error": err.to_string() }),
        )
        .await;
        if let Ok(true) = mark_deployment_run_failed(&state.db, run_id, &err.to_string()).await {
            let _ = record_deployment_run_metrics(
                &state,
                run_id,
                Some(DeploymentTimelineStep::Finalize),
            )
            .await;
        }
    }
}

/// Create and enqueue a deployment run when a Deploy task completes successfully.
/// Used by the attempt success hook for task_type == Deploy.
#[allow(dead_code)]
pub async fn create_and_enqueue_deployment_run_for_deploy_task(
    state: &AppState,
    project_id: Uuid,
    attempt_id: Uuid,
    triggered_by: Option<Uuid>,
) -> Result<Option<Uuid>, ApiError> {
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
    .fetch_optional(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    let Some(environment) = env else {
        return Ok(None);
    };

    let has_active_run: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM deployment_runs
            WHERE environment_id = $1
              AND status IN ('queued', 'running', 'rolling_back')
        )
        "#,
    )
    .bind(environment.id)
    .fetch_one(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    if has_active_run {
        tracing::info!(
            project_id = %project_id,
            environment_id = %environment.id,
            "Skipping deploy task trigger: active run already exists"
        );
        return Ok(None);
    }

    let run = sqlx::query_as::<_, DeploymentRun>(
        r#"
        INSERT INTO deployment_runs (
            project_id, environment_id, status, trigger_type, triggered_by,
            source_type, source_ref, attempt_id, metadata
        )
        VALUES ($1, $2, 'queued', 'auto', $3, 'artifact', NULL, $4, '{}'::jsonb)
        RETURNING *
        "#,
    )
    .bind(project_id)
    .bind(environment.id)
    .bind(triggered_by)
    .bind(attempt_id)
    .fetch_one(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    record_queued_deployment_metric(state, environment.slug.as_str());

    append_deployment_audit_event(
        &state.db,
        triggered_by,
        "deployment_runs.start",
        run.id,
        run.project_id,
        run.environment_id,
        serde_json::json!({
            "trigger_type": "auto",
            "source_type": "artifact",
            "attempt_id": attempt_id,
        }),
    )
    .await?;

    enqueue_deployment_job(state, run.id, run.project_id, run.environment_id).await?;

    Ok(Some(run.id))
}

async fn enqueue_deployment_job(
    state: &AppState,
    run_id: Uuid,
    project_id: Uuid,
    environment_id: Uuid,
) -> Result<(), ApiError> {
    if let Some(worker_pool) = &state.deployment_worker_pool {
        if let Err(err) = worker_pool
            .submit(DeploymentJob::new(run_id, project_id, environment_id))
            .await
        {
            let message = format!("Failed to queue deployment run: {}", err);
            let _ = append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Finalize,
                DeploymentTimelineEventType::Error,
                "Deployment queue rejected run",
                serde_json::json!({ "error": message }),
            )
            .await;
            if let Ok(true) = mark_deployment_run_failed(&state.db, run_id, &message).await {
                let _ = record_deployment_run_metrics(
                    state,
                    run_id,
                    Some(DeploymentTimelineStep::Finalize),
                )
                .await;
            }
            return Err(ApiError::Internal(message));
        }

        return Ok(());
    }

    let state_for_job = state.clone();
    tokio::spawn(async move {
        process_deployment_run_background(state_for_job, run_id).await;
    });
    Ok(())
}

async fn process_deployment_run(state: AppState, run_id: Uuid) -> Result<(), ApiError> {
    let run = sqlx::query_as::<_, DeploymentRun>(
        r#"
        UPDATE deployment_runs
        SET
            status = CASE
                WHEN trigger_type = 'rollback' THEN 'rolling_back'::deployment_run_status
                ELSE 'running'::deployment_run_status
            END,
            started_at = COALESCE(started_at, NOW()),
            updated_at = NOW()
        WHERE id = $1 AND status = 'queued'
        RETURNING *
        "#,
    )
    .bind(run_id)
    .fetch_optional(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    let Some(run) = run else {
        // Run could already be cancelled or picked by another worker.
        return Ok(());
    };

    append_deployment_timeline_event(
        &state.db,
        run_id,
        DeploymentTimelineStep::Precheck,
        DeploymentTimelineEventType::System,
        "Deployment run started",
        serde_json::json!({ "run_id": run_id }),
    )
    .await?;

    let environment =
        get_project_deployment_environment(&state.db, run.project_id, run.environment_id).await?;
    if !environment.is_enabled {
        append_deployment_timeline_event(
            &state.db,
            run_id,
            DeploymentTimelineStep::Precheck,
            DeploymentTimelineEventType::Error,
            "Environment is disabled",
            serde_json::json!({ "environment_id": environment.id }),
        )
        .await?;
        if mark_deployment_run_failed(&state.db, run_id, "Environment is disabled").await? {
            record_deployment_run_metrics(&state, run_id, Some(DeploymentTimelineStep::Precheck))
                .await?;
        }
        return Ok(());
    }

    if is_run_cancelled(&state.db, run_id).await? {
        append_deployment_timeline_event(
            &state.db,
            run_id,
            DeploymentTimelineStep::Finalize,
            DeploymentTimelineEventType::Warning,
            "Deployment run was cancelled before processing steps",
            serde_json::json!({}),
        )
        .await?;
        return Ok(());
    }

    let rollback_target_release_id = if run.trigger_type == DeploymentTriggerType::Rollback {
        let release_id_from_source = run
            .source_ref
            .as_ref()
            .and_then(|value| Uuid::parse_str(value).ok());
        let release_id_from_metadata = run
            .metadata
            .get("target_release_id")
            .and_then(|value| value.as_str())
            .and_then(|value| Uuid::parse_str(value).ok());

        let target_release_id = release_id_from_source.or(release_id_from_metadata);
        let Some(target_release_id) = target_release_id else {
            append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Rollback,
                DeploymentTimelineEventType::Error,
                "Missing target release for rollback",
                serde_json::json!({}),
            )
            .await?;
            if mark_deployment_run_failed(
                &state.db,
                run_id,
                "Rollback run is missing target_release_id",
            )
            .await?
            {
                record_deployment_run_metrics(
                    &state,
                    run_id,
                    Some(DeploymentTimelineStep::Rollback),
                )
                .await?;
            }
            return Ok(());
        };

        let release_exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM deployment_releases
                WHERE id = $1
                  AND project_id = $2
                  AND environment_id = $3
                  AND status <> 'failed'
            )
            "#,
        )
        .bind(target_release_id)
        .bind(run.project_id)
        .bind(run.environment_id)
        .fetch_one(&state.db)
        .await
        .map_err(map_sqlx_error)?;

        if !release_exists {
            append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Rollback,
                DeploymentTimelineEventType::Error,
                "Rollback target release not found",
                serde_json::json!({ "target_release_id": target_release_id }),
            )
            .await?;
            if mark_deployment_run_failed(&state.db, run_id, "Rollback target release not found")
                .await?
            {
                record_deployment_run_metrics(
                    &state,
                    run_id,
                    Some(DeploymentTimelineStep::Rollback),
                )
                .await?;
            }
            return Ok(());
        }

        Some(target_release_id)
    } else {
        None
    };

    match environment.target_type {
        DeploymentTargetType::Local => {
            let deploy_path = FsPath::new(&environment.deploy_path);
            append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Connect,
                DeploymentTimelineEventType::System,
                "Validating local deploy path",
                serde_json::json!({ "deploy_path": environment.deploy_path }),
            )
            .await?;

            if let Err(err) = tokio::fs::create_dir_all(deploy_path).await {
                append_deployment_timeline_event(
                    &state.db,
                    run_id,
                    DeploymentTimelineStep::Connect,
                    DeploymentTimelineEventType::Error,
                    "Failed to access local deploy path",
                    serde_json::json!({ "error": err.to_string() }),
                )
                .await?;
                if mark_deployment_run_failed(
                    &state.db,
                    run_id,
                    &format!("Cannot access local deploy path: {}", err),
                )
                .await?
                {
                    record_deployment_run_metrics(
                        &state,
                        run_id,
                        Some(DeploymentTimelineStep::Connect),
                    )
                    .await?;
                }
                return Ok(());
            }

            if is_run_cancelled(&state.db, run_id).await? {
                append_deployment_timeline_event(
                    &state.db,
                    run_id,
                    DeploymentTimelineStep::Finalize,
                    DeploymentTimelineEventType::Warning,
                    "Deployment run cancelled",
                    serde_json::json!({ "step": "connect" }),
                )
                .await?;
                return Ok(());
            }

            if let Some(target_release_id) = rollback_target_release_id {
                append_deployment_timeline_event(
                    &state.db,
                    run_id,
                    DeploymentTimelineStep::Rollback,
                    DeploymentTimelineEventType::Command,
                    "Applying local rollback",
                    serde_json::json!({
                        "target_release_id": target_release_id,
                        "strategy": "local_release_reactivation",
                    }),
                )
                .await?;

                let rollback_marker_path = deploy_path.join(".acpms-last-rollback.txt");
                let rollback_marker_content = format!(
                    "run_id={}\nenvironment_id={}\ntarget_release_id={}\ntime={}\n",
                    run.id,
                    run.environment_id,
                    target_release_id,
                    Utc::now().to_rfc3339()
                );
                tokio::fs::write(&rollback_marker_path, rollback_marker_content)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;

                if is_run_cancelled(&state.db, run_id).await? {
                    append_deployment_timeline_event(
                        &state.db,
                        run_id,
                        DeploymentTimelineStep::Finalize,
                        DeploymentTimelineEventType::Warning,
                        "Rollback run cancelled",
                        serde_json::json!({ "step": "rollback" }),
                    )
                    .await?;
                    return Ok(());
                }

                if mark_deployment_run_rolled_back(&state.db, run_id, target_release_id).await? {
                    record_deployment_run_metrics(&state, run_id, None).await?;
                }
                append_deployment_timeline_event(
                    &state.db,
                    run_id,
                    DeploymentTimelineStep::Finalize,
                    DeploymentTimelineEventType::System,
                    "Rollback run completed successfully",
                    serde_json::json!({ "target_release_id": target_release_id }),
                )
                .await?;
                return Ok(());
            }

            append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Prepare,
                DeploymentTimelineEventType::System,
                "Preparing deployment context",
                serde_json::json!({}),
            )
            .await?;

            let context_path = deploy_path.join(".acpms-deploy-context.json");
            let context_content = serde_json::json!({
                "run_id": run_id,
                "project_id": run.project_id,
                "environment_id": run.environment_id,
                "source_type": deployment_source_type_label(run.source_type),
                "source_ref": run.source_ref,
                "generated_at": Utc::now(),
            });
            tokio::fs::write(&context_path, context_content.to_string())
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            if is_run_cancelled(&state.db, run_id).await? {
                append_deployment_timeline_event(
                    &state.db,
                    run_id,
                    DeploymentTimelineStep::Finalize,
                    DeploymentTimelineEventType::Warning,
                    "Deployment run cancelled",
                    serde_json::json!({ "step": "prepare" }),
                )
                .await?;
                return Ok(());
            }

            append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Deploy,
                DeploymentTimelineEventType::Command,
                "Applying local deployment",
                serde_json::json!({ "strategy": "local_file_marker" }),
            )
            .await?;

            let marker_path = deploy_path.join(".acpms-last-deploy.txt");
            let marker_content = format!(
                "run_id={}\nproject_id={}\nenvironment_id={}\nsource_type={}\nsource_ref={}\ntime={}\n",
                run.id,
                run.project_id,
                run.environment_id,
                deployment_source_type_label(run.source_type),
                run.source_ref.clone().unwrap_or_default(),
                Utc::now().to_rfc3339()
            );
            tokio::fs::write(&marker_path, marker_content)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            if is_run_cancelled(&state.db, run_id).await? {
                append_deployment_timeline_event(
                    &state.db,
                    run_id,
                    DeploymentTimelineStep::Finalize,
                    DeploymentTimelineEventType::Warning,
                    "Deployment run cancelled",
                    serde_json::json!({ "step": "domain_config" }),
                )
                .await?;
                return Ok(());
            }

            if let Some(domain_error) =
                apply_local_domain_mapping(&state.db, run_id, &environment, deploy_path).await?
            {
                if mark_deployment_run_failed(&state.db, run_id, &domain_error).await? {
                    record_deployment_run_metrics(
                        &state,
                        run_id,
                        Some(DeploymentTimelineStep::DomainConfig),
                    )
                    .await?;
                }
                return Ok(());
            }

            append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Healthcheck,
                DeploymentTimelineEventType::System,
                "Running deployment healthcheck",
                serde_json::json!({ "healthcheck_url": environment.healthcheck_url }),
            )
            .await?;

            if let Some(url) = environment.healthcheck_url.clone() {
                if url::Url::parse(&url).is_err() {
                    append_deployment_timeline_event(
                        &state.db,
                        run_id,
                        DeploymentTimelineStep::Healthcheck,
                        DeploymentTimelineEventType::Error,
                        "Invalid healthcheck URL",
                        serde_json::json!({ "healthcheck_url": url }),
                    )
                    .await?;
                    if mark_deployment_run_failed(&state.db, run_id, "Invalid healthcheck URL")
                        .await?
                    {
                        record_deployment_run_metrics(
                            &state,
                            run_id,
                            Some(DeploymentTimelineStep::Healthcheck),
                        )
                        .await?;
                    }
                    return Ok(());
                }
            }

            if is_run_cancelled(&state.db, run_id).await? {
                append_deployment_timeline_event(
                    &state.db,
                    run_id,
                    DeploymentTimelineStep::Finalize,
                    DeploymentTimelineEventType::Warning,
                    "Deployment run cancelled",
                    serde_json::json!({ "step": "healthcheck" }),
                )
                .await?;
                return Ok(());
            }

            if mark_deployment_run_success(&state.db, run_id).await? {
                record_deployment_run_metrics(&state, run_id, None).await?;
            }
            append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Finalize,
                DeploymentTimelineEventType::System,
                "Deployment run completed successfully",
                serde_json::json!({}),
            )
            .await?;
        }
        DeploymentTargetType::SshRemote => {
            append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Connect,
                DeploymentTimelineEventType::System,
                "Preparing SSH execution context",
                serde_json::json!({}),
            )
            .await?;

            let ssh_context = match prepare_ssh_execution_context(&state.db, &environment).await {
                Ok(context) => context,
                Err(err) => {
                    append_deployment_timeline_event(
                        &state.db,
                        run_id,
                        DeploymentTimelineStep::Connect,
                        DeploymentTimelineEventType::Error,
                        "Invalid SSH deployment configuration",
                        serde_json::json!({ "error": err.to_string() }),
                    )
                    .await?;
                    if mark_deployment_run_failed(&state.db, run_id, &err.to_string()).await? {
                        record_deployment_run_metrics(
                            &state,
                            run_id,
                            Some(DeploymentTimelineStep::Connect),
                        )
                        .await?;
                    }
                    return Ok(());
                }
            };

            let auth_label = match &ssh_context.auth {
                SshAuth::PrivateKey { .. } => "private_key",
                SshAuth::Password { .. } => "password",
            };
            append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Connect,
                DeploymentTimelineEventType::System,
                "SSH context prepared",
                serde_json::json!({
                    "host": &ssh_context.host,
                    "port": ssh_context.port,
                    "username": &ssh_context.username,
                    "auth": auth_label,
                    "host_verification": "known_hosts_strict",
                }),
            )
            .await?;

            match run_ssh_command(
                &ssh_context,
                "printf '%s' 'ACPMS_SSH_READY'",
                Duration::from_secs(15),
            )
            .await
            {
                Ok(output) => {
                    if !output.stderr.trim().is_empty() {
                        append_deployment_timeline_event(
                            &state.db,
                            run_id,
                            DeploymentTimelineStep::Connect,
                            DeploymentTimelineEventType::Warning,
                            "SSH handshake completed with diagnostics",
                            serde_json::json!({
                                "stderr": sanitize_command_output(&output.stderr)
                            }),
                        )
                        .await?;
                    } else {
                        append_deployment_timeline_event(
                            &state.db,
                            run_id,
                            DeploymentTimelineStep::Connect,
                            DeploymentTimelineEventType::System,
                            "SSH handshake and host verification succeeded",
                            serde_json::json!({}),
                        )
                        .await?;
                    }
                }
                Err(err) => {
                    append_deployment_timeline_event(
                        &state.db,
                        run_id,
                        DeploymentTimelineStep::Connect,
                        DeploymentTimelineEventType::Error,
                        "SSH handshake failed",
                        serde_json::json!({ "error": err }),
                    )
                    .await?;
                    if mark_deployment_run_failed(
                        &state.db,
                        run_id,
                        "SSH handshake failed during deployment",
                    )
                    .await?
                    {
                        record_deployment_run_metrics(
                            &state,
                            run_id,
                            Some(DeploymentTimelineStep::Connect),
                        )
                        .await?;
                    }
                    return Ok(());
                }
            }

            if is_run_cancelled(&state.db, run_id).await? {
                append_deployment_timeline_event(
                    &state.db,
                    run_id,
                    DeploymentTimelineStep::Finalize,
                    DeploymentTimelineEventType::Warning,
                    "Deployment run cancelled",
                    serde_json::json!({ "step": "connect" }),
                )
                .await?;
                return Ok(());
            }

            if let Some(target_release_id) = rollback_target_release_id {
                append_deployment_timeline_event(
                    &state.db,
                    run_id,
                    DeploymentTimelineStep::Rollback,
                    DeploymentTimelineEventType::Command,
                    "Applying rollback on SSH target",
                    serde_json::json!({
                        "target_release_id": target_release_id,
                        "strategy": "remote_release_reactivation",
                    }),
                )
                .await?;

                let rollback_marker_content = format!(
                    "run_id={}\nenvironment_id={}\ntarget_release_id={}\ntime={}\n",
                    run.id,
                    run.environment_id,
                    target_release_id,
                    Utc::now().to_rfc3339()
                );
                let rollback_script = build_remote_rollback_script(
                    &environment.deploy_path,
                    &rollback_marker_content,
                );

                if let Err(err) =
                    run_ssh_command(&ssh_context, &rollback_script, Duration::from_secs(60)).await
                {
                    append_deployment_timeline_event(
                        &state.db,
                        run_id,
                        DeploymentTimelineStep::Rollback,
                        DeploymentTimelineEventType::Error,
                        "Remote rollback command failed",
                        serde_json::json!({ "error": err }),
                    )
                    .await?;
                    if mark_deployment_run_failed(
                        &state.db,
                        run_id,
                        "Remote rollback command failed",
                    )
                    .await?
                    {
                        record_deployment_run_metrics(
                            &state,
                            run_id,
                            Some(DeploymentTimelineStep::Rollback),
                        )
                        .await?;
                    }
                    return Ok(());
                }

                if is_run_cancelled(&state.db, run_id).await? {
                    append_deployment_timeline_event(
                        &state.db,
                        run_id,
                        DeploymentTimelineStep::Finalize,
                        DeploymentTimelineEventType::Warning,
                        "Rollback run cancelled",
                        serde_json::json!({ "step": "rollback" }),
                    )
                    .await?;
                    return Ok(());
                }

                if mark_deployment_run_rolled_back(&state.db, run_id, target_release_id).await? {
                    record_deployment_run_metrics(&state, run_id, None).await?;
                }
                append_deployment_timeline_event(
                    &state.db,
                    run_id,
                    DeploymentTimelineStep::Finalize,
                    DeploymentTimelineEventType::System,
                    "Rollback run completed successfully",
                    serde_json::json!({ "target_release_id": target_release_id }),
                )
                .await?;
                return Ok(());
            }

            append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Prepare,
                DeploymentTimelineEventType::System,
                "Preparing remote deployment context",
                serde_json::json!({}),
            )
            .await?;

            let context_content = serde_json::json!({
                "run_id": run_id,
                "project_id": run.project_id,
                "environment_id": run.environment_id,
                "source_type": deployment_source_type_label(run.source_type),
                "source_ref": run.source_ref,
                "generated_at": Utc::now(),
            })
            .to_string();

            let marker_content = format!(
                "run_id={}\nproject_id={}\nenvironment_id={}\nsource_type={}\nsource_ref={}\ntime={}\n",
                run.id,
                run.project_id,
                run.environment_id,
                deployment_source_type_label(run.source_type),
                run.source_ref.clone().unwrap_or_default(),
                Utc::now().to_rfc3339()
            );

            let deploy_script = build_remote_deploy_script(
                &environment.deploy_path,
                &context_content,
                &marker_content,
            );
            append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Deploy,
                DeploymentTimelineEventType::Command,
                "Applying remote deployment",
                serde_json::json!({
                    "target_type": "ssh_remote",
                    "deploy_path": &environment.deploy_path,
                }),
            )
            .await?;

            if let Err(err) =
                run_ssh_command(&ssh_context, &deploy_script, Duration::from_secs(90)).await
            {
                append_deployment_timeline_event(
                    &state.db,
                    run_id,
                    DeploymentTimelineStep::Deploy,
                    DeploymentTimelineEventType::Error,
                    "Remote deployment command failed",
                    serde_json::json!({ "error": err }),
                )
                .await?;
                if mark_deployment_run_failed(&state.db, run_id, "Remote deployment command failed")
                    .await?
                {
                    record_deployment_run_metrics(
                        &state,
                        run_id,
                        Some(DeploymentTimelineStep::Deploy),
                    )
                    .await?;
                }
                return Ok(());
            }

            if is_run_cancelled(&state.db, run_id).await? {
                append_deployment_timeline_event(
                    &state.db,
                    run_id,
                    DeploymentTimelineStep::Finalize,
                    DeploymentTimelineEventType::Warning,
                    "Deployment run cancelled",
                    serde_json::json!({ "step": "domain_config" }),
                )
                .await?;
                return Ok(());
            }

            if let Some(domain_error) =
                apply_ssh_domain_mapping(&state.db, run_id, &environment, &ssh_context).await?
            {
                if mark_deployment_run_failed(&state.db, run_id, &domain_error).await? {
                    record_deployment_run_metrics(
                        &state,
                        run_id,
                        Some(DeploymentTimelineStep::DomainConfig),
                    )
                    .await?;
                }
                return Ok(());
            }

            append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Healthcheck,
                DeploymentTimelineEventType::System,
                "Running deployment healthcheck",
                serde_json::json!({ "healthcheck_url": environment.healthcheck_url }),
            )
            .await?;

            if let Some(url) = environment.healthcheck_url.clone() {
                if url::Url::parse(&url).is_err() {
                    append_deployment_timeline_event(
                        &state.db,
                        run_id,
                        DeploymentTimelineStep::Healthcheck,
                        DeploymentTimelineEventType::Error,
                        "Invalid healthcheck URL",
                        serde_json::json!({ "healthcheck_url": url }),
                    )
                    .await?;
                    if mark_deployment_run_failed(&state.db, run_id, "Invalid healthcheck URL")
                        .await?
                    {
                        record_deployment_run_metrics(
                            &state,
                            run_id,
                            Some(DeploymentTimelineStep::Healthcheck),
                        )
                        .await?;
                    }
                    return Ok(());
                }
            }

            if is_run_cancelled(&state.db, run_id).await? {
                append_deployment_timeline_event(
                    &state.db,
                    run_id,
                    DeploymentTimelineStep::Finalize,
                    DeploymentTimelineEventType::Warning,
                    "Deployment run cancelled",
                    serde_json::json!({ "step": "healthcheck" }),
                )
                .await?;
                return Ok(());
            }

            if mark_deployment_run_success(&state.db, run_id).await? {
                record_deployment_run_metrics(&state, run_id, None).await?;
            }
            append_deployment_timeline_event(
                &state.db,
                run_id,
                DeploymentTimelineStep::Finalize,
                DeploymentTimelineEventType::System,
                "Deployment run completed successfully",
                serde_json::json!({}),
            )
            .await?;
        }
    }

    Ok(())
}

#[derive(Deserialize)]
struct SshKeyscanRequest {
    host: String,
}

#[derive(serde::Serialize)]
struct SshKeyscanResponse {
    known_hosts: String,
}

async fn ssh_keyscan(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<SshKeyscanRequest>,
) -> Result<Json<ApiResponse<SshKeyscanResponse>>, ApiError> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ViewDeployments,
        &state.db,
    )
    .await?;

    let host = payload.host.trim();
    if host.is_empty() {
        return Err(ApiError::Validation("host is required".to_string()));
    }

    let output = timeout(
        Duration::from_secs(10),
        Command::new("ssh-keyscan").arg("-H").arg(host).output(),
    )
    .await
    .map_err(|_| ApiError::Internal("ssh-keyscan timed out".to_string()))?
    .map_err(|e| ApiError::Internal(format!("ssh-keyscan failed: {}", e)))?;

    let known_hosts = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if known_hosts.is_empty() {
        return Err(ApiError::Validation(format!(
            "ssh-keyscan returned no keys for host '{}'. Check host is reachable from server.",
            host
        )));
    }

    Ok(Json(ApiResponse::success(
        SshKeyscanResponse { known_hosts },
        "Known hosts retrieved",
    )))
}

async fn list_deployment_environments(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<DeploymentEnvironment>>>, ApiError> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ViewDeployments,
        &state.db,
    )
    .await?;

    let environments = sqlx::query_as::<_, DeploymentEnvironment>(
        r#"
        SELECT *
        FROM deployment_environments
        WHERE project_id = $1
        ORDER BY is_default DESC, name ASC
        "#,
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    Ok(Json(ApiResponse::success(
        environments,
        "Deployment environments retrieved successfully",
    )))
}

async fn create_deployment_environment(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<CreateDeploymentEnvironmentRequest>,
) -> Result<(StatusCode, Json<ApiResponse<DeploymentEnvironment>>), ApiError> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::Validation("name is required".to_string()));
    }

    let slug = slugify_name(&name);
    if slug.is_empty() {
        return Err(ApiError::Validation(
            "name must contain at least one alphanumeric character".to_string(),
        ));
    }

    let deploy_path = payload.deploy_path.trim().to_string();
    if deploy_path.is_empty() {
        return Err(ApiError::Validation("deploy_path is required".to_string()));
    }

    let target_type = payload.target_type;
    let runtime_type = payload
        .runtime_type
        .unwrap_or(acpms_db::models::DeploymentRuntimeType::RawScript);
    let artifact_strategy = payload
        .artifact_strategy
        .unwrap_or(acpms_db::models::DeploymentArtifactStrategy::BuildArtifact);
    let branch_policy = payload
        .branch_policy
        .unwrap_or_else(|| serde_json::json!({}));
    let target_config = payload
        .target_config
        .unwrap_or_else(|| serde_json::json!({}));
    let domain_config = payload
        .domain_config
        .unwrap_or_else(|| serde_json::json!({}));
    let healthcheck_url = normalize_optional_string(payload.healthcheck_url);
    let healthcheck_timeout_secs = payload.healthcheck_timeout_secs.unwrap_or(60);
    let healthcheck_expected_status = payload.healthcheck_expected_status.unwrap_or(200);
    let is_enabled = payload.is_enabled.unwrap_or(true);
    let is_default = payload.is_default.unwrap_or(false);

    validate_healthcheck_values(healthcheck_timeout_secs, healthcheck_expected_status)?;
    validate_target_config(target_type, &target_config)?;
    validate_domain_config(&domain_config)?;

    let mut tx = state.db.begin().await.map_err(map_sqlx_error)?;

    if is_default {
        sqlx::query("UPDATE deployment_environments SET is_default = false WHERE project_id = $1")
            .bind(project_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
    }

    let environment = sqlx::query_as::<_, DeploymentEnvironment>(
        r#"
        INSERT INTO deployment_environments (
            project_id, name, slug, description, target_type, is_enabled, is_default,
            runtime_type, deploy_path, artifact_strategy, branch_policy, healthcheck_url,
            healthcheck_timeout_secs, healthcheck_expected_status, target_config, domain_config,
            created_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::jsonb, $12, $13, $14, $15::jsonb, $16::jsonb, $17)
        RETURNING *
        "#,
    )
    .bind(project_id)
    .bind(name)
    .bind(slug)
    .bind(payload.description)
    .bind(target_type)
    .bind(is_enabled)
    .bind(is_default)
    .bind(runtime_type)
    .bind(deploy_path)
    .bind(artifact_strategy)
    .bind(branch_policy)
    .bind(healthcheck_url)
    .bind(healthcheck_timeout_secs)
    .bind(healthcheck_expected_status)
    .bind(target_config)
    .bind(domain_config)
    .bind(auth_user.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_sqlx_error)?;

    if let Some(secrets) = payload.secrets.as_ref() {
        upsert_environment_secrets(&mut tx, environment.id, secrets).await?;
    }

    tx.commit().await.map_err(map_sqlx_error)?;

    append_environment_audit_event(
        &state.db,
        auth_user.id,
        "deployment_environments.create",
        &environment,
        serde_json::json!({
            "target_type": format!("{:?}", environment.target_type).to_ascii_lowercase(),
            "runtime_type": format!("{:?}", environment.runtime_type).to_ascii_lowercase(),
            "is_default": environment.is_default,
            "is_enabled": environment.is_enabled,
            "has_primary_domain": validate_domain_config(&environment.domain_config)?.is_some(),
            "secrets_updated": payload.secrets.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
        }),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::created(
            environment,
            "Deployment environment created successfully",
        )),
    ))
}

async fn get_deployment_environment(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path((project_id, env_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<DeploymentEnvironment>>, ApiError> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ViewDeployments,
        &state.db,
    )
    .await?;

    let environment = get_project_deployment_environment(&state.db, project_id, env_id).await?;
    Ok(Json(ApiResponse::success(
        environment,
        "Deployment environment retrieved successfully",
    )))
}

async fn update_deployment_environment(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path((project_id, env_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateDeploymentEnvironmentRequest>,
) -> Result<Json<ApiResponse<DeploymentEnvironment>>, ApiError> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    let existing = get_project_deployment_environment(&state.db, project_id, env_id).await?;

    let name = payload
        .name
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or(existing.name.clone());
    let slug = slugify_name(&name);
    if slug.is_empty() {
        return Err(ApiError::Validation(
            "name must contain at least one alphanumeric character".to_string(),
        ));
    }

    let deploy_path = payload
        .deploy_path
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or(existing.deploy_path.clone());
    if deploy_path.is_empty() {
        return Err(ApiError::Validation("deploy_path is required".to_string()));
    }

    let target_type = payload.target_type.unwrap_or(existing.target_type);
    let runtime_type = payload.runtime_type.unwrap_or(existing.runtime_type);
    let artifact_strategy = payload
        .artifact_strategy
        .unwrap_or(existing.artifact_strategy);
    let branch_policy = payload
        .branch_policy
        .unwrap_or(existing.branch_policy.clone());
    let target_config = payload
        .target_config
        .unwrap_or(existing.target_config.clone());
    let domain_config = payload
        .domain_config
        .unwrap_or(existing.domain_config.clone());
    let healthcheck_url = match payload.healthcheck_url {
        Some(value) => normalize_optional_string(Some(value)),
        None => existing.healthcheck_url.clone(),
    };
    let healthcheck_timeout_secs = payload
        .healthcheck_timeout_secs
        .unwrap_or(existing.healthcheck_timeout_secs);
    let healthcheck_expected_status = payload
        .healthcheck_expected_status
        .unwrap_or(existing.healthcheck_expected_status);
    let is_enabled = payload.is_enabled.unwrap_or(existing.is_enabled);
    let is_default = payload.is_default.unwrap_or(existing.is_default);
    let description = match payload.description {
        Some(value) => normalize_optional_string(Some(value)),
        None => existing.description.clone(),
    };

    validate_healthcheck_values(healthcheck_timeout_secs, healthcheck_expected_status)?;
    validate_target_config(target_type, &target_config)?;
    validate_domain_config(&domain_config)?;

    let mut tx = state.db.begin().await.map_err(map_sqlx_error)?;

    if is_default {
        sqlx::query(
            "UPDATE deployment_environments SET is_default = false WHERE project_id = $1 AND id <> $2",
        )
        .bind(project_id)
        .bind(env_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;
    }

    let updated = sqlx::query_as::<_, DeploymentEnvironment>(
        r#"
        UPDATE deployment_environments
        SET
            name = $3,
            slug = $4,
            description = $5,
            target_type = $6,
            is_enabled = $7,
            is_default = $8,
            runtime_type = $9,
            deploy_path = $10,
            artifact_strategy = $11,
            branch_policy = $12::jsonb,
            healthcheck_url = $13,
            healthcheck_timeout_secs = $14,
            healthcheck_expected_status = $15,
            target_config = $16::jsonb,
            domain_config = $17::jsonb,
            updated_at = NOW()
        WHERE project_id = $1 AND id = $2
        RETURNING *
        "#,
    )
    .bind(project_id)
    .bind(env_id)
    .bind(name)
    .bind(slug)
    .bind(description)
    .bind(target_type)
    .bind(is_enabled)
    .bind(is_default)
    .bind(runtime_type)
    .bind(deploy_path)
    .bind(artifact_strategy)
    .bind(branch_policy)
    .bind(healthcheck_url)
    .bind(healthcheck_timeout_secs)
    .bind(healthcheck_expected_status)
    .bind(target_config)
    .bind(domain_config)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_sqlx_error)?;

    if let Some(secrets) = payload.secrets.as_ref() {
        upsert_environment_secrets(&mut tx, env_id, secrets).await?;
    }

    tx.commit().await.map_err(map_sqlx_error)?;

    append_environment_audit_event(
        &state.db,
        auth_user.id,
        "deployment_environments.update",
        &updated,
        serde_json::json!({
            "previous_target_type": format!("{:?}", existing.target_type).to_ascii_lowercase(),
            "target_type": format!("{:?}", updated.target_type).to_ascii_lowercase(),
            "previous_is_enabled": existing.is_enabled,
            "is_enabled": updated.is_enabled,
            "previous_is_default": existing.is_default,
            "is_default": updated.is_default,
            "has_primary_domain": validate_domain_config(&updated.domain_config)?.is_some(),
            "secrets_updated": payload.secrets.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
        }),
    )
    .await?;

    Ok(Json(ApiResponse::success(
        updated,
        "Deployment environment updated successfully",
    )))
}

async fn delete_deployment_environment(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path((project_id, env_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    let environment = get_project_deployment_environment(&state.db, project_id, env_id).await?;

    let deleted_id = sqlx::query_scalar::<_, Uuid>(
        "DELETE FROM deployment_environments WHERE project_id = $1 AND id = $2 RETURNING id",
    )
    .bind(project_id)
    .bind(env_id)
    .fetch_optional(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    if deleted_id.is_none() {
        return Err(ApiError::NotFound(
            "Deployment environment not found".to_string(),
        ));
    }

    append_environment_audit_event(
        &state.db,
        auth_user.id,
        "deployment_environments.delete",
        &environment,
        serde_json::json!({
            "target_type": format!("{:?}", environment.target_type).to_ascii_lowercase(),
            "was_default": environment.is_default,
            "was_enabled": environment.is_enabled,
        }),
    )
    .await?;

    Ok(Json(ApiResponse::success(
        (),
        "Deployment environment deleted successfully",
    )))
}

async fn test_deployment_environment_connection(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path((project_id, env_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<DeploymentConnectionTestResponse>>, ApiError> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    let environment = get_project_deployment_environment(&state.db, project_id, env_id).await?;
    let mut checks: Vec<DeploymentCheckResult> = Vec::new();

    match environment.target_type {
        DeploymentTargetType::Local => {
            let deploy_path = FsPath::new(&environment.deploy_path);

            if deploy_path.exists() {
                if deploy_path.is_dir() {
                    checks.push(DeploymentCheckResult {
                        step: "path_exists".to_string(),
                        status: "pass".to_string(),
                        message: format!("Deploy path exists: {}", environment.deploy_path),
                    });
                } else {
                    checks.push(DeploymentCheckResult {
                        step: "path_exists".to_string(),
                        status: "fail".to_string(),
                        message: "Deploy path exists but is not a directory".to_string(),
                    });
                }
            } else {
                match tokio::fs::create_dir_all(deploy_path).await {
                    Ok(_) => checks.push(DeploymentCheckResult {
                        step: "path_exists".to_string(),
                        status: "pass".to_string(),
                        message: "Deploy path did not exist and was created successfully"
                            .to_string(),
                    }),
                    Err(err) => checks.push(DeploymentCheckResult {
                        step: "path_exists".to_string(),
                        status: "fail".to_string(),
                        message: format!("Cannot create deploy path: {}", err),
                    }),
                }
            }

            let writable_path =
                deploy_path.join(format!(".acpms-conn-check-{}.tmp", Uuid::new_v4()));
            match tokio::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&writable_path)
                .await
            {
                Ok(mut file) => {
                    let _ = file.write_all(b"acpms").await;
                    let _ = tokio::fs::remove_file(&writable_path).await;
                    checks.push(DeploymentCheckResult {
                        step: "write_access".to_string(),
                        status: "pass".to_string(),
                        message: "Write access verified for deploy path".to_string(),
                    });
                }
                Err(err) => checks.push(DeploymentCheckResult {
                    step: "write_access".to_string(),
                    status: "fail".to_string(),
                    message: format!("No write access to deploy path: {}", err),
                }),
            }
        }
        DeploymentTargetType::SshRemote => {
            let (host, port, username) =
                match extract_ssh_host_port_username(&environment.target_config) {
                    Ok(v) => v,
                    Err(err) => {
                        checks.push(DeploymentCheckResult {
                            step: "ssh_config".to_string(),
                            status: "fail".to_string(),
                            message: err.to_string(),
                        });
                        let response = DeploymentConnectionTestResponse {
                            success: false,
                            checks,
                        };
                        return Ok(Json(ApiResponse::success(
                            response,
                            "Connection test failed",
                        )));
                    }
                };

            checks.push(DeploymentCheckResult {
                step: "ssh_config".to_string(),
                status: "pass".to_string(),
                message: format!("SSH target parsed: {}@{}:{}", username, host, port),
            });

            let tcp_check = timeout(
                Duration::from_secs(5),
                TcpStream::connect((host.as_str(), port)),
            )
            .await;

            let mut tcp_ok = false;
            match tcp_check {
                Ok(Ok(_)) => {
                    tcp_ok = true;
                    checks.push(DeploymentCheckResult {
                        step: "tcp_connect".to_string(),
                        status: "pass".to_string(),
                        message: "TCP connectivity to SSH host verified".to_string(),
                    });
                }
                Ok(Err(err)) => checks.push(DeploymentCheckResult {
                    step: "tcp_connect".to_string(),
                    status: "fail".to_string(),
                    message: format!("Cannot connect to SSH host: {}", err),
                }),
                Err(_) => checks.push(DeploymentCheckResult {
                    step: "tcp_connect".to_string(),
                    status: "fail".to_string(),
                    message: "TCP connectivity check timed out".to_string(),
                }),
            }

            if tcp_ok {
                match prepare_ssh_execution_context(&state.db, &environment).await {
                    Ok(ssh_context) => {
                        let auth_label = match &ssh_context.auth {
                            SshAuth::PrivateKey { .. } => "private_key",
                            SshAuth::Password { .. } => "password",
                        };
                        checks.push(DeploymentCheckResult {
                            step: "ssh_credentials".to_string(),
                            status: "pass".to_string(),
                            message: format!("SSH authentication configured: {}", auth_label),
                        });
                        checks.push(DeploymentCheckResult {
                            step: "host_verification".to_string(),
                            status: "pass".to_string(),
                            message:
                                "Host verification is enforced via known_hosts + StrictHostKeyChecking"
                                    .to_string(),
                        });

                        match run_ssh_command(
                            &ssh_context,
                            "printf '%s' 'ACPMS_SSH_OK'",
                            Duration::from_secs(12),
                        )
                        .await
                        {
                            Ok(result) => {
                                if result.stdout.trim().contains("ACPMS_SSH_OK") {
                                    checks.push(DeploymentCheckResult {
                                        step: "ssh_handshake".to_string(),
                                        status: "pass".to_string(),
                                        message:
                                            "SSH authentication and command execution succeeded"
                                                .to_string(),
                                    });
                                } else {
                                    checks.push(DeploymentCheckResult {
                                        step: "ssh_handshake".to_string(),
                                        status: "fail".to_string(),
                                        message: format!(
                                            "SSH command returned unexpected output: {}",
                                            sanitize_command_output(&result.stdout)
                                        ),
                                    });
                                }
                            }
                            Err(err) => checks.push(DeploymentCheckResult {
                                step: "ssh_handshake".to_string(),
                                status: "fail".to_string(),
                                message: err,
                            }),
                        }
                    }
                    Err(err) => checks.push(DeploymentCheckResult {
                        step: "ssh_credentials".to_string(),
                        status: "fail".to_string(),
                        message: err.to_string(),
                    }),
                }
            }
        }
    }

    let success = checks.iter().all(|check| check.status == "pass");
    let response = DeploymentConnectionTestResponse { success, checks };

    append_environment_audit_event(
        &state.db,
        auth_user.id,
        "deployment_environments.test_connection",
        &environment,
        serde_json::json!({
            "success": response.success,
            "checks_total": response.checks.len(),
            "checks_failed": response.checks.iter().filter(|c| c.status != "pass").count(),
        }),
    )
    .await?;

    let message = if success {
        "Connection test passed"
    } else {
        "Connection test failed"
    };
    Ok(Json(ApiResponse::success(response, message)))
}

async fn test_deployment_environment_domain(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path((project_id, env_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<DeploymentConnectionTestResponse>>, ApiError> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    let environment = get_project_deployment_environment(&state.db, project_id, env_id).await?;
    let mut checks: Vec<DeploymentCheckResult> = Vec::new();

    let primary_domain = match validate_domain_config(&environment.domain_config) {
        Ok(value) => value,
        Err(err) => {
            checks.push(DeploymentCheckResult {
                step: "domain_config".to_string(),
                status: "fail".to_string(),
                message: err.to_string(),
            });
            let response = DeploymentConnectionTestResponse {
                success: false,
                checks,
            };
            return Ok(Json(ApiResponse::success(response, "Domain test failed")));
        }
    };

    let Some(domain) = primary_domain else {
        checks.push(DeploymentCheckResult {
            step: "domain_config".to_string(),
            status: "fail".to_string(),
            message: "domain_config.primary_domain is required".to_string(),
        });
        let response = DeploymentConnectionTestResponse {
            success: false,
            checks,
        };
        return Ok(Json(ApiResponse::success(response, "Domain test failed")));
    };

    checks.push(DeploymentCheckResult {
        step: "domain_syntax".to_string(),
        status: "pass".to_string(),
        message: format!("Domain syntax is valid: {}", domain),
    });

    match timeout(Duration::from_secs(5), lookup_host((domain.as_str(), 80))).await {
        Ok(Ok(addrs)) => {
            let mut resolved_addrs = addrs;
            if resolved_addrs.next().is_some() {
                checks.push(DeploymentCheckResult {
                    step: "dns_lookup".to_string(),
                    status: "pass".to_string(),
                    message: "DNS lookup resolved successfully".to_string(),
                });
            } else {
                checks.push(DeploymentCheckResult {
                    step: "dns_lookup".to_string(),
                    status: "fail".to_string(),
                    message: "DNS lookup returned no addresses".to_string(),
                });
            }
        }
        Ok(Err(err)) => checks.push(DeploymentCheckResult {
            step: "dns_lookup".to_string(),
            status: "fail".to_string(),
            message: format!("DNS lookup failed: {}", err),
        }),
        Err(_) => checks.push(DeploymentCheckResult {
            step: "dns_lookup".to_string(),
            status: "fail".to_string(),
            message: "DNS lookup timed out".to_string(),
        }),
    }

    let success = checks.iter().all(|check| check.status == "pass");
    let response = DeploymentConnectionTestResponse { success, checks };

    append_environment_audit_event(
        &state.db,
        auth_user.id,
        "deployment_environments.test_domain",
        &environment,
        serde_json::json!({
            "success": response.success,
            "checks_total": response.checks.len(),
            "checks_failed": response.checks.iter().filter(|c| c.status != "pass").count(),
        }),
    )
    .await?;

    let message = if success {
        "Domain test passed"
    } else {
        "Domain test failed"
    };
    Ok(Json(ApiResponse::success(response, message)))
}

async fn list_deployment_releases_for_environment(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path((project_id, env_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ListDeploymentReleasesQuery>,
) -> Result<Json<ApiResponse<Vec<DeploymentRelease>>>, ApiError> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ViewDeployments,
        &state.db,
    )
    .await?;

    // Validate environment belongs to the project.
    get_project_deployment_environment(&state.db, project_id, env_id).await?;

    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let releases = sqlx::query_as::<_, DeploymentRelease>(
        r#"
        SELECT *
        FROM deployment_releases
        WHERE project_id = $1
          AND environment_id = $2
          AND ($3::deployment_release_status IS NULL OR status = $3)
        ORDER BY deployed_at DESC
        LIMIT $4
        "#,
    )
    .bind(project_id)
    .bind(env_id)
    .bind(query.status)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    Ok(Json(ApiResponse::success(
        releases,
        "Deployment releases retrieved successfully",
    )))
}

async fn get_deployment_release(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(release_id): Path<Uuid>,
) -> Result<Json<ApiResponse<DeploymentRelease>>, ApiError> {
    let project_id = get_project_id_by_deployment_release(&state.db, release_id).await?;
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ViewDeployments,
        &state.db,
    )
    .await?;

    let release =
        sqlx::query_as::<_, DeploymentRelease>("SELECT * FROM deployment_releases WHERE id = $1")
            .bind(release_id)
            .fetch_optional(&state.db)
            .await
            .map_err(map_sqlx_error)?
            .ok_or_else(|| ApiError::NotFound("Deployment release not found".to_string()))?;

    Ok(Json(ApiResponse::success(
        release,
        "Deployment release retrieved successfully",
    )))
}

async fn start_deployment_run(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path((project_id, env_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<Option<StartDeploymentRunRequest>>,
) -> Result<(StatusCode, Json<ApiResponse<DeploymentRun>>), ApiError> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ExecuteTask, &state.db)
        .await?;

    let environment = get_project_deployment_environment(&state.db, project_id, env_id).await?;
    if !environment.is_enabled {
        return Err(ApiError::Conflict(
            "Deployment environment is disabled".to_string(),
        ));
    }

    let has_active_run: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM deployment_runs
            WHERE environment_id = $1
              AND status IN ('queued', 'running', 'rolling_back')
        )
        "#,
    )
    .bind(env_id)
    .fetch_one(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    if has_active_run {
        return Err(ApiError::Conflict(
            "Another deployment run is already active for this environment".to_string(),
        ));
    }

    let source_type = payload
        .as_ref()
        .and_then(|p| p.source_type)
        .unwrap_or(DeploymentSourceType::Branch);
    let source_ref = payload
        .as_ref()
        .and_then(|p| p.source_ref.as_ref())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    let attempt_id = payload.as_ref().and_then(|p| p.attempt_id);
    let metadata = payload
        .and_then(|p| p.metadata)
        .unwrap_or_else(|| serde_json::json!({}));

    let run = sqlx::query_as::<_, DeploymentRun>(
        r#"
        INSERT INTO deployment_runs (
            project_id, environment_id, status, trigger_type, triggered_by,
            source_type, source_ref, attempt_id, metadata
        )
        VALUES ($1, $2, 'queued', 'manual', $3, $4, $5, $6, $7::jsonb)
        RETURNING *
        "#,
    )
    .bind(project_id)
    .bind(env_id)
    .bind(auth_user.id)
    .bind(source_type)
    .bind(source_ref)
    .bind(attempt_id)
    .bind(metadata)
    .fetch_one(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    record_queued_deployment_metric(&state, environment.slug.as_str());

    append_deployment_audit_event(
        &state.db,
        Some(auth_user.id),
        "deployment_runs.start",
        run.id,
        run.project_id,
        run.environment_id,
        serde_json::json!({
            "trigger_type": "manual",
            "source_type": deployment_source_type_label(run.source_type),
            "source_ref": run.source_ref,
            "attempt_id": run.attempt_id,
        }),
    )
    .await?;

    enqueue_deployment_job(&state, run.id, run.project_id, run.environment_id).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::created(
            run,
            "Deployment run queued successfully",
        )),
    ))
}

async fn list_deployment_runs(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListDeploymentRunsQuery>,
) -> Result<Json<ApiResponse<Vec<DeploymentRun>>>, ApiError> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ViewDeployments,
        &state.db,
    )
    .await?;

    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let runs = sqlx::query_as::<_, DeploymentRun>(
        r#"
        SELECT *
        FROM deployment_runs
        WHERE project_id = $1
          AND ($2::uuid IS NULL OR environment_id = $2)
          AND ($3::deployment_run_status IS NULL OR status = $3)
        ORDER BY created_at DESC
        LIMIT $4
        "#,
    )
    .bind(project_id)
    .bind(query.environment_id)
    .bind(query.status)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    Ok(Json(ApiResponse::success(
        runs,
        "Deployment runs retrieved successfully",
    )))
}

async fn get_deployment_run(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ApiResponse<DeploymentRun>>, ApiError> {
    let project_id = get_project_id_by_deployment_run(&state.db, run_id).await?;
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ViewDeployments,
        &state.db,
    )
    .await?;

    let run = sqlx::query_as::<_, DeploymentRun>("SELECT * FROM deployment_runs WHERE id = $1")
        .bind(run_id)
        .fetch_optional(&state.db)
        .await
        .map_err(map_sqlx_error)?
        .ok_or_else(|| ApiError::NotFound("Deployment run not found".to_string()))?;

    Ok(Json(ApiResponse::success(
        run,
        "Deployment run retrieved successfully",
    )))
}

async fn list_deployment_run_logs(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<DeploymentTimelineEvent>>>, ApiError> {
    let project_id = get_project_id_by_deployment_run(&state.db, run_id).await?;
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ViewDeployments,
        &state.db,
    )
    .await?;

    let events = sqlx::query_as::<_, DeploymentTimelineEvent>(
        r#"
        SELECT *
        FROM deployment_timeline_events
        WHERE run_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(run_id)
    .fetch_all(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    Ok(Json(ApiResponse::success(
        events,
        "Deployment run logs retrieved successfully",
    )))
}

async fn list_deployment_run_timeline(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<DeploymentTimelineEvent>>>, ApiError> {
    let project_id = get_project_id_by_deployment_run(&state.db, run_id).await?;
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ViewDeployments,
        &state.db,
    )
    .await?;

    let events = sqlx::query_as::<_, DeploymentTimelineEvent>(
        r#"
        SELECT *
        FROM deployment_timeline_events
        WHERE run_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(run_id)
    .fetch_all(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    Ok(Json(ApiResponse::success(
        events,
        "Deployment run timeline retrieved successfully",
    )))
}

#[derive(Debug, Deserialize)]
struct DeploymentRunStreamQuery {
    after_id: Option<Uuid>,
}

#[derive(Clone)]
struct DeploymentRunStreamState {
    state: AppState,
    run_id: Uuid,
    last_seen_id: Option<Uuid>,
    buffered_events: VecDeque<DeploymentTimelineEvent>,
    completed: bool,
}

async fn stream_deployment_run(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
    Query(query): Query<DeploymentRunStreamQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let project_id = get_project_id_by_deployment_run(&state.db, run_id).await?;
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ViewDeployments,
        &state.db,
    )
    .await?;

    let stream_state = DeploymentRunStreamState {
        state,
        run_id,
        last_seen_id: query.after_id,
        buffered_events: VecDeque::new(),
        completed: false,
    };

    let sse_stream = stream::unfold(stream_state, |mut stream_state| async move {
        if stream_state.completed {
            return None;
        }

        loop {
            if let Some(event_row) = stream_state.buffered_events.pop_front() {
                stream_state.last_seen_id = Some(event_row.id);
                let data = serde_json::to_string(&event_row).unwrap_or_else(|_| "{}".to_string());
                let event = Event::default()
                    .id(event_row.id.to_string())
                    .event("timeline")
                    .data(data);
                return Some((Ok(event), stream_state));
            }

            let events = if let Some(last_seen_id) = stream_state.last_seen_id {
                sqlx::query_as::<_, DeploymentTimelineEvent>(
                    r#"
                    SELECT *
                    FROM deployment_timeline_events
                    WHERE run_id = $1
                      AND (
                        created_at > (
                            SELECT created_at
                            FROM deployment_timeline_events
                            WHERE id = $2
                        )
                        OR (
                            created_at = (
                                SELECT created_at
                                FROM deployment_timeline_events
                                WHERE id = $2
                            )
                            AND id::text > $2::text
                        )
                      )
                    ORDER BY created_at ASC, id ASC
                    LIMIT 100
                    "#,
                )
                .bind(stream_state.run_id)
                .bind(last_seen_id)
                .fetch_all(&stream_state.state.db)
                .await
            } else {
                sqlx::query_as::<_, DeploymentTimelineEvent>(
                    r#"
                    SELECT *
                    FROM deployment_timeline_events
                    WHERE run_id = $1
                    ORDER BY created_at ASC, id ASC
                    LIMIT 100
                    "#,
                )
                .bind(stream_state.run_id)
                .fetch_all(&stream_state.state.db)
                .await
            };

            match events {
                Ok(rows) if !rows.is_empty() => {
                    stream_state.buffered_events = VecDeque::from(rows);
                    continue;
                }
                Ok(_) => {
                    let status = sqlx::query_scalar::<_, DeploymentRunStatus>(
                        "SELECT status FROM deployment_runs WHERE id = $1",
                    )
                    .bind(stream_state.run_id)
                    .fetch_optional(&stream_state.state.db)
                    .await;

                    match status {
                        Ok(Some(run_status)) if is_terminal_deployment_status(run_status) => {
                            stream_state.completed = true;
                            let payload = serde_json::json!({
                                "run_id": stream_state.run_id,
                                "status": deployment_status_label(run_status),
                            })
                            .to_string();
                            let event = Event::default().event("completed").data(payload);
                            return Some((Ok(event), stream_state));
                        }
                        Ok(Some(_)) => {
                            tokio::time::sleep(Duration::from_millis(750)).await;
                            continue;
                        }
                        Ok(None) => {
                            stream_state.completed = true;
                            let event = Event::default()
                                .event("error")
                                .data("{\"message\":\"Deployment run not found\"}");
                            return Some((Ok(event), stream_state));
                        }
                        Err(err) => {
                            stream_state.completed = true;
                            let event = Event::default().event("error").data(
                                serde_json::json!({
                                    "message": "Failed to load deployment run status",
                                    "error": err.to_string(),
                                })
                                .to_string(),
                            );
                            return Some((Ok(event), stream_state));
                        }
                    }
                }
                Err(err) => {
                    stream_state.completed = true;
                    let event = Event::default().event("error").data(
                        serde_json::json!({
                            "message": "Failed to stream deployment timeline",
                            "error": err.to_string(),
                        })
                        .to_string(),
                    );
                    return Some((Ok(event), stream_state));
                }
            }
        }
    });

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::default()))
}

async fn cancel_deployment_run(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ApiResponse<DeploymentRun>>, ApiError> {
    let project_id = get_project_id_by_deployment_run(&state.db, run_id).await?;
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ExecuteTask, &state.db)
        .await?;

    let run = sqlx::query_as::<_, DeploymentRun>(
        r#"
        UPDATE deployment_runs
        SET
            status = 'cancelled',
            completed_at = COALESCE(completed_at, NOW()),
            error_message = COALESCE(error_message, 'Cancelled by user'),
            updated_at = NOW()
        WHERE id = $1
          AND status IN ('queued', 'running', 'rolling_back', 'success', 'failed')
        RETURNING *
        "#,
    )
    .bind(run_id)
    .fetch_optional(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    if let Some(run) = run {
        record_deployment_run_metrics(&state, run.id, None).await?;

        append_deployment_audit_event(
            &state.db,
            Some(auth_user.id),
            "deployment_runs.cancel",
            run.id,
            run.project_id,
            run.environment_id,
            serde_json::json!({
                "status_after": deployment_status_label(run.status),
                "reason": "cancelled_by_user",
            }),
        )
        .await?;

        let _ = append_deployment_timeline_event(
            &state.db,
            run.id,
            DeploymentTimelineStep::Finalize,
            DeploymentTimelineEventType::Warning,
            "Deployment run cancelled by user",
            serde_json::json!({}),
        )
        .await;
        return Ok(Json(ApiResponse::success(
            run,
            "Deployment run cancelled successfully",
        )));
    }

    let current_status: Option<DeploymentRunStatus> =
        sqlx::query_scalar("SELECT status FROM deployment_runs WHERE id = $1")
            .bind(run_id)
            .fetch_optional(&state.db)
            .await
            .map_err(map_sqlx_error)?;

    match current_status {
        None => Err(ApiError::NotFound("Deployment run not found".to_string())),
        Some(status) => Err(ApiError::Conflict(format!(
            "Cannot cancel deployment run in {:?} state",
            status
        ))),
    }
}

async fn retry_deployment_run(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<(StatusCode, Json<ApiResponse<DeploymentRun>>), ApiError> {
    let project_id = get_project_id_by_deployment_run(&state.db, run_id).await?;
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ExecuteTask, &state.db)
        .await?;

    let original_run =
        sqlx::query_as::<_, DeploymentRun>("SELECT * FROM deployment_runs WHERE id = $1")
            .bind(run_id)
            .fetch_optional(&state.db)
            .await
            .map_err(map_sqlx_error)?
            .ok_or_else(|| ApiError::NotFound("Deployment run not found".to_string()))?;

    if !matches!(
        original_run.status,
        DeploymentRunStatus::Failed | DeploymentRunStatus::Cancelled
    ) {
        return Err(ApiError::Conflict(format!(
            "Cannot retry deployment run in {:?} state",
            original_run.status
        )));
    }

    let has_active_run: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM deployment_runs
            WHERE environment_id = $1
              AND status IN ('queued', 'running', 'rolling_back')
        )
        "#,
    )
    .bind(original_run.environment_id)
    .fetch_one(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    if has_active_run {
        return Err(ApiError::Conflict(
            "Another deployment run is already active for this environment".to_string(),
        ));
    }

    let mut metadata = original_run.metadata.clone();
    match metadata.as_object_mut() {
        Some(obj) => {
            obj.insert(
                "retry_of_run_id".to_string(),
                serde_json::json!(original_run.id),
            );
        }
        None => {
            metadata = serde_json::json!({ "retry_of_run_id": original_run.id });
        }
    }

    let retried_run = sqlx::query_as::<_, DeploymentRun>(
        r#"
        INSERT INTO deployment_runs (
            project_id, environment_id, status, trigger_type, triggered_by,
            source_type, source_ref, attempt_id, metadata
        )
        VALUES ($1, $2, 'queued', $3, $4, $5, $6, $7, $8::jsonb)
        RETURNING *
        "#,
    )
    .bind(original_run.project_id)
    .bind(original_run.environment_id)
    .bind(DeploymentTriggerType::Retry)
    .bind(auth_user.id)
    .bind(original_run.source_type)
    .bind(original_run.source_ref)
    .bind(original_run.attempt_id)
    .bind(metadata)
    .fetch_one(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    let environment_slug = environment_slug_by_id(&state.db, retried_run.environment_id).await?;
    record_queued_deployment_metric(&state, environment_slug.as_str());

    append_deployment_audit_event(
        &state.db,
        Some(auth_user.id),
        "deployment_runs.retry",
        retried_run.id,
        retried_run.project_id,
        retried_run.environment_id,
        serde_json::json!({
            "retry_of_run_id": original_run.id,
            "source_type": deployment_source_type_label(retried_run.source_type),
            "source_ref": retried_run.source_ref,
        }),
    )
    .await?;

    enqueue_deployment_job(
        &state,
        retried_run.id,
        retried_run.project_id,
        retried_run.environment_id,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::created(
            retried_run,
            "Deployment run retried successfully",
        )),
    ))
}

async fn rollback_deployment_run(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
    payload: Option<Json<RollbackDeploymentRunRequest>>,
) -> Result<(StatusCode, Json<ApiResponse<DeploymentRun>>), ApiError> {
    #[derive(sqlx::FromRow)]
    struct ReleaseTarget {
        id: Uuid,
        version_label: String,
    }

    let project_id = get_project_id_by_deployment_run(&state.db, run_id).await?;
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ExecuteTask, &state.db)
        .await?;

    let source_run =
        sqlx::query_as::<_, DeploymentRun>("SELECT * FROM deployment_runs WHERE id = $1")
            .bind(run_id)
            .fetch_optional(&state.db)
            .await
            .map_err(map_sqlx_error)?
            .ok_or_else(|| ApiError::NotFound("Deployment run not found".to_string()))?;

    if source_run.status != DeploymentRunStatus::Success {
        return Err(ApiError::Conflict(format!(
            "Rollback is only allowed from success runs, current status: {:?}",
            source_run.status
        )));
    }

    let has_active_run: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM deployment_runs
            WHERE environment_id = $1
              AND status IN ('queued', 'running', 'rolling_back')
        )
        "#,
    )
    .bind(source_run.environment_id)
    .fetch_one(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    if has_active_run {
        return Err(ApiError::Conflict(
            "Another deployment run is already active for this environment".to_string(),
        ));
    }

    let payload = payload.map(|p| p.0);
    let target_release =
        if let Some(target_release_id) = payload.as_ref().and_then(|body| body.target_release_id) {
            sqlx::query_as::<_, ReleaseTarget>(
                r#"
            SELECT id, version_label
            FROM deployment_releases
            WHERE id = $1
              AND project_id = $2
              AND environment_id = $3
              AND status <> 'failed'
            "#,
            )
            .bind(target_release_id)
            .bind(source_run.project_id)
            .bind(source_run.environment_id)
            .fetch_optional(&state.db)
            .await
            .map_err(map_sqlx_error)?
        } else {
            sqlx::query_as::<_, ReleaseTarget>(
                r#"
            SELECT id, version_label
            FROM deployment_releases
            WHERE project_id = $1
              AND environment_id = $2
              AND run_id = $3
              AND status <> 'failed'
            ORDER BY deployed_at DESC
            LIMIT 1
            "#,
            )
            .bind(source_run.project_id)
            .bind(source_run.environment_id)
            .bind(source_run.id)
            .fetch_optional(&state.db)
            .await
            .map_err(map_sqlx_error)?
        }
        .ok_or_else(|| {
            ApiError::NotFound("No eligible release found for rollback target".to_string())
        })?;

    let mut metadata = payload
        .and_then(|body| body.metadata)
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(metadata_obj) = metadata.as_object_mut() {
        metadata_obj.insert(
            "rollback_of_run_id".to_string(),
            serde_json::json!(source_run.id),
        );
        metadata_obj.insert(
            "target_release_id".to_string(),
            serde_json::json!(target_release.id),
        );
        metadata_obj.insert(
            "target_release_version".to_string(),
            serde_json::json!(target_release.version_label),
        );
    } else {
        metadata = serde_json::json!({
            "rollback_of_run_id": source_run.id,
            "target_release_id": target_release.id,
            "target_release_version": target_release.version_label,
        });
    }

    let rollback_run = sqlx::query_as::<_, DeploymentRun>(
        r#"
        INSERT INTO deployment_runs (
            project_id, environment_id, status, trigger_type, triggered_by,
            source_type, source_ref, attempt_id, metadata
        )
        VALUES ($1, $2, 'queued', $3, $4, $5, $6, NULL, $7::jsonb)
        RETURNING *
        "#,
    )
    .bind(source_run.project_id)
    .bind(source_run.environment_id)
    .bind(DeploymentTriggerType::Rollback)
    .bind(auth_user.id)
    .bind(DeploymentSourceType::Release)
    .bind(target_release.id.to_string())
    .bind(metadata)
    .fetch_one(&state.db)
    .await
    .map_err(map_sqlx_error)?;

    let environment_slug = environment_slug_by_id(&state.db, rollback_run.environment_id).await?;
    record_queued_deployment_metric(&state, environment_slug.as_str());

    append_deployment_timeline_event(
        &state.db,
        rollback_run.id,
        DeploymentTimelineStep::Rollback,
        DeploymentTimelineEventType::System,
        "Rollback run queued",
        serde_json::json!({
            "rollback_of_run_id": source_run.id,
            "target_release_id": target_release.id,
            "target_release_version": target_release.version_label,
        }),
    )
    .await?;

    append_deployment_audit_event(
        &state.db,
        Some(auth_user.id),
        "deployment_runs.rollback",
        rollback_run.id,
        rollback_run.project_id,
        rollback_run.environment_id,
        serde_json::json!({
            "rollback_of_run_id": source_run.id,
            "target_release_id": target_release.id,
            "target_release_version": target_release.version_label,
        }),
    )
    .await?;

    enqueue_deployment_job(
        &state,
        rollback_run.id,
        rollback_run.project_id,
        rollback_run.environment_id,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::created(
            rollback_run,
            "Rollback run queued successfully",
        )),
    ))
}

/// Trigger a build for a task attempt
///
/// POST /api/v1/attempts/{id}/build
/// Returns 202 Accepted immediately; build runs in background. Poll GET /attempts/:id/artifacts for results.
#[utoipa::path(
    post,
    path = "/api/v1/attempts/{id}/build",
    tag = "Deployments",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    request_body = TriggerBuildRequest,
    responses(
        (status = 202, description = "Build started successfully (runs in background)"),
        (status = 404, description = "Attempt not found"),
        (status = 500, description = "Internal server error")
    )
)]
async fn trigger_build(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(attempt_id): Path<Uuid>,
    Json(req): Json<Option<TriggerBuildRequest>>,
) -> Result<(StatusCode, Json<ApiResponse<BuildStartedResponse>>), ApiError> {
    let project_id = get_project_id_by_attempt(&state.db, attempt_id).await?;
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ExecuteTask, &state.db)
        .await?;

    // Get attempt and task info
    let attempt = sqlx::query_as::<_, acpms_db::models::TaskAttempt>(
        "SELECT * FROM task_attempts WHERE id = $1",
    )
    .bind(attempt_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("Attempt not found".into()))?;

    // Get task to find project
    let task = sqlx::query_as::<_, acpms_db::models::Task>("SELECT * FROM tasks WHERE id = $1")
        .bind(attempt.task_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Get project
    let project =
        sqlx::query_as::<_, acpms_db::models::Project>("SELECT * FROM projects WHERE id = $1")
            .bind(task.project_id)
            .fetch_one(&state.db)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Build override config if provided
    let build_override = req.and_then(|r| {
        if r.build_command.is_some() || r.output_dir.is_some() {
            Some(acpms_db::models::BuildConfig {
                command: r
                    .build_command
                    .unwrap_or_else(|| project.project_type.default_build_command().to_string()),
                output_dir: r.output_dir.unwrap_or_else(|| "dist".to_string()),
            })
        } else {
            None
        }
    });

    // Run build in background to avoid blocking HTTP request
    let build_service = state.build_service.clone();
    let project_clone = project.clone();
    tokio::spawn(async move {
        if let Err(e) = build_service
            .run_build(&project_clone, attempt_id, build_override)
            .await
        {
            tracing::error!("Background build failed for attempt {}: {}", attempt_id, e);
        }
    });

    let data = BuildStartedResponse {
        attempt_id,
        status: "building".to_string(),
    };
    let response = ApiResponse::success(data, "Build started successfully");
    Ok((StatusCode::ACCEPTED, Json(response)))
}

/// List build artifacts for an attempt
///
/// GET /api/v1/attempts/{id}/artifacts
#[utoipa::path(
    get,
    path = "/api/v1/attempts/{id}/artifacts",
    tag = "Deployments",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    responses(
        (status = 200, description = "Artifacts retrieved"),
        (status = 404, description = "Attempt not found"),
        (status = 500, description = "Internal server error")
    )
)]
async fn list_artifacts(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(attempt_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<ArtifactResponse>>>, ApiError> {
    let project_id = get_project_id_by_attempt(&state.db, attempt_id).await?;
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewTask, &state.db)
        .await?;

    let artifacts = state
        .build_service
        .get_attempt_artifacts(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Convert to response format with download URLs
    let mut responses = Vec::new();
    for artifact in artifacts {
        let download_url = state
            .build_service
            .get_artifact_download_url(&artifact.artifact_key)
            .await
            .ok();

        responses.push(ArtifactResponse {
            id: artifact.id,
            artifact_key: artifact.artifact_key,
            artifact_type: artifact.artifact_type,
            size_bytes: artifact.size_bytes,
            file_count: artifact.file_count,
            download_url,
            created_at: artifact.created_at,
        });
    }

    let response = ApiResponse::success(responses, "Artifacts retrieved successfully");
    Ok(Json(response))
}

/// Trigger production deployment for a project
///
/// POST /api/v1/projects/{id}/deploy
#[utoipa::path(
    post,
    path = "/api/v1/projects/{id}/deploy",
    tag = "Deployments",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    request_body = TriggerDeployRequest,
    responses(
        (status = 200, description = "Deployment triggered"),
        (status = 404, description = "Project or artifact not found"),
        (status = 500, description = "Internal server error")
    )
)]
async fn trigger_deploy(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(req): Json<Option<TriggerDeployRequest>>,
) -> Result<Json<ApiResponse<DeploymentResponse>>, ApiError> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    // Get project
    let project =
        sqlx::query_as::<_, acpms_db::models::Project>("SELECT * FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound("Project not found".into()))?;

    // Get artifact (use provided ID or latest)
    let artifact = if let Some(ref r) = req {
        if let Some(artifact_id) = r.artifact_id {
            sqlx::query_as::<_, acpms_db::models::BuildArtifact>(
                "SELECT * FROM build_artifacts WHERE id = $1",
            )
            .bind(artifact_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
        } else {
            state
                .build_service
                .get_latest_artifact(project_id)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?
        }
    } else {
        state
            .build_service
            .get_latest_artifact(project_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
    };

    let artifact = artifact.ok_or_else(|| ApiError::NotFound("No build artifact found".into()))?;

    // Deploy
    let result = state
        .deploy_service
        .deploy(&project, &artifact, Some(auth_user.id))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = ApiResponse::success(
        DeploymentResponse {
            deployment_id: result.deployment_id,
            url: result.url,
            status: "active".to_string(),
            deployment_type: result.deployment_type,
        },
        "Deployment triggered successfully",
    );
    Ok(Json(response))
}

/// List deployments for a project
///
/// GET /api/v1/projects/{id}/deployments
#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}/deployments",
    tag = "Deployments",
    params(
        ("id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Deployments retrieved"),
        (status = 500, description = "Internal server error")
    )
)]
async fn list_deployments(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListDeploymentsQuery>,
) -> Result<Json<ApiResponse<Vec<DeploymentResponse>>>, ApiError> {
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ViewDeployments,
        &state.db,
    )
    .await?;

    let deployments = state
        .deploy_service
        .get_project_deployments(project_id, query.limit)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let responses: Vec<DeploymentResponse> = deployments
        .into_iter()
        .map(|d| DeploymentResponse {
            deployment_id: d.id,
            url: d.url,
            status: d.status,
            deployment_type: d.deployment_type,
        })
        .collect();

    let response = ApiResponse::success(responses, "Deployments retrieved successfully");
    Ok(Json(response))
}

/// Get a specific deployment
///
/// GET /api/v1/deployments/{id}
async fn get_deployment(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(deployment_id): Path<Uuid>,
) -> Result<Json<ApiResponse<DeploymentResponse>>, ApiError> {
    let project_id = get_project_id_by_deployment(&state.db, deployment_id).await?;
    RbacChecker::check_permission(
        auth_user.id,
        project_id,
        Permission::ViewDeployments,
        &state.db,
    )
    .await?;

    let deployment = state
        .deploy_service
        .get_deployment(deployment_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Deployment not found".into()))?;

    let response = ApiResponse::success(
        DeploymentResponse {
            deployment_id: deployment.id,
            url: deployment.url,
            status: deployment.status,
            deployment_type: deployment.deployment_type,
        },
        "Deployment retrieved",
    );
    Ok(Json(response))
}

/// GitLab merge request payload (relevant fields)
#[derive(Debug, Deserialize)]
struct GitLabMergeEvent {
    #[allow(dead_code)]
    object_kind: String,
    object_attributes: MergeRequestAttributes,
    #[allow(dead_code)]
    project: GitLabProject,
}

#[derive(Debug, Deserialize)]
struct MergeRequestAttributes {
    #[allow(dead_code)]
    id: i64,
    #[allow(dead_code)]
    iid: i64,
    state: String,
    action: Option<String>,
    target_branch: String,
    #[allow(dead_code)]
    source_branch: String,
}

#[derive(Debug, Deserialize)]
struct GitLabProject {
    #[allow(dead_code)]
    id: i64,
    #[allow(dead_code)]
    name: String,
}

/// Handle GitLab merge webhook for auto-deploy
///
/// POST /api/v1/webhooks/gitlab/merge
#[utoipa::path(
    post,
    path = "/api/v1/webhooks/gitlab/merge",
    tag = "Deployments",
    responses(
        (status = 200, description = "Webhook processed"),
        (status = 400, description = "Invalid webhook payload"),
        (status = 401, description = "Unauthorized")
    )
)]
async fn handle_merge_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    // Validate webhook token
    let token = headers
        .get("X-Gitlab-Token")
        .and_then(|h| h.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    // Find project by webhook secret
    let gitlab_config = sqlx::query_as::<_, acpms_db::models::GitLabConfiguration>(
        "SELECT * FROM gitlab_configurations WHERE webhook_secret = $1",
    )
    .bind(token)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or(ApiError::Unauthorized)?;

    // Parse event
    let event_type = payload
        .get("object_kind")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Only process merge request events
    if event_type != "merge_request" {
        return Ok(Json(ApiResponse::success(
            (),
            "Event ignored (not merge_request)",
        )));
    }

    // Parse merge event
    let merge_event: GitLabMergeEvent = serde_json::from_value(payload.clone())
        .map_err(|e| ApiError::BadRequest(format!("Invalid merge event payload: {}", e)))?;

    // Check if this is a merge action to deploy branch
    let is_merge = merge_event
        .object_attributes
        .action
        .as_ref()
        .map(|a| a == "merge")
        .unwrap_or(false);

    if !is_merge || merge_event.object_attributes.state != "merged" {
        return Ok(Json(ApiResponse::success((), "Event ignored (not merged)")));
    }

    // Get project
    let project =
        sqlx::query_as::<_, acpms_db::models::Project>("SELECT * FROM projects WHERE id = $1")
            .bind(gitlab_config.project_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound("Project not found".into()))?;

    // Check if production_deploy_on_merge is enabled and target branch matches deploy_branch
    if !project.settings.production_deploy_on_merge {
        return Ok(Json(ApiResponse::success(
            (),
            "Auto-deploy disabled for project",
        )));
    }

    if merge_event.object_attributes.target_branch != project.settings.deploy_branch {
        return Ok(Json(ApiResponse::success(
            (),
            format!(
                "Target branch {} does not match deploy branch {}",
                merge_event.object_attributes.target_branch, project.settings.deploy_branch
            ),
        )));
    }

    tracing::info!(
        "Auto-deploy triggered for project {} on merge to {}",
        project.name,
        project.settings.deploy_branch
    );

    // Get latest artifact
    let artifact = state
        .build_service
        .get_latest_artifact(project.id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    if let Some(artifact) = artifact {
        // Trigger deployment
        let result = state
            .deploy_service
            .deploy(&project, &artifact, None)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        tracing::info!("Auto-deploy completed: {} -> {}", project.name, result.url);

        return Ok(Json(ApiResponse::success(
            (),
            format!("Deployed to {}", result.url),
        )));
    }

    Ok(Json(ApiResponse::success(
        (),
        "No artifact found for deployment",
    )))
}
