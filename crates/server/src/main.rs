mod api;
mod error;
mod handlers;
mod middleware;
mod observability;
mod routes;
mod services;
mod ssh;
mod state;
mod types;

use acpms_executors::ExecutorOrchestrator;
use acpms_preview::PreviewManager;
use anyhow::Context;
use axum::middleware as axum_middleware;
use clap::Parser;
use observability::{init_logging, request_id, Metrics};
use state::AppState;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use uuid::Uuid;

use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use acpms_db::models::AttemptStatus;
use acpms_executors::AgentEvent;
use acpms_services::{StorageService, TaskAttemptService};

/// R7: Spawn task that uploads JSONL logs to S3 when attempt completes.
fn spawn_log_upload_on_complete(
    mut rx: broadcast::Receiver<AgentEvent>,
    pool: sqlx::PgPool,
    storage: Arc<StorageService>,
) {
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let (attempt_id, terminal) = match &event {
                AgentEvent::Status(s) => {
                    let t = matches!(
                        s.status,
                        AttemptStatus::Success | AttemptStatus::Failed | AttemptStatus::Cancelled
                    );
                    (s.attempt_id, t)
                }
                _ => continue,
            };
            if !terminal {
                continue;
            }
            // Upload JSONL logs to S3 (Vibe Kanban style - JSONL only, no agent_logs)
            match acpms_executors::read_attempt_log_file(attempt_id).await {
                Ok(bytes) if !bytes.is_empty() => {
                    match storage.upload_jsonl(attempt_id, &bytes).await {
                        Ok(key) => {
                            let attempt_service = TaskAttemptService::new(pool.clone());
                            if let Err(e) =
                                attempt_service.update_s3_log_key(attempt_id, &key).await
                            {
                                tracing::warn!(
                                    "Failed to update s3_log_key for attempt {}: {}",
                                    attempt_id,
                                    e
                                );
                            } else {
                                tracing::info!(
                                    "Uploaded logs for attempt {} to S3: {}",
                                    attempt_id,
                                    key
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to upload logs for attempt {} to S3: {}",
                                attempt_id,
                                e
                            );
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::debug!(
                        "Could not read log file for attempt {} (may not exist): {}",
                        attempt_id,
                        e
                    );
                }
            }
        }
    });
}

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::auth::register,
        routes::auth::login,
        routes::users::list_users,
        routes::users::get_user,
        routes::users::update_user,
        routes::users::delete_user,
        routes::users::get_avatar_upload_url,
        routes::projects::create_project,
        routes::projects::list_projects,
        routes::projects::get_project,
        routes::projects::update_project,
        routes::projects::delete_project,
        routes::projects::recheck_project_repository_access,
        routes::projects::link_existing_fork,
        routes::projects::create_project_fork,
        routes::projects::import_project_preflight,
        routes::projects::import_project_create_fork,
        routes::projects::import_project,
        // Tasks
        routes::tasks::create_task,
        routes::tasks::list_tasks,
        routes::tasks::get_task,
        routes::tasks::update_task,
        routes::tasks::delete_task,
        routes::tasks::update_task_status,
        routes::tasks::get_task_children,
        routes::tasks::assign_task,
        routes::tasks::update_task_metadata,
        // Sprints
        routes::sprints::list_project_sprints,
        routes::sprints::create_sprint,
        routes::sprints::generate_sprints,
        routes::sprints::get_sprint,
        routes::sprints::update_sprint,
        routes::sprints::delete_sprint,
        routes::sprints::get_active_sprint,
        routes::sprints::activate_sprint,
        routes::sprints::close_sprint,
        routes::sprints::get_sprint_overview,
        // Requirements
        routes::requirements::create_requirement,
        routes::requirements::list_project_requirements,
        routes::requirements::get_requirement,
        routes::requirements::update_requirement,
        routes::requirements::delete_requirement,
        routes::requirement_breakdowns::start_requirement_breakdown,
        routes::requirement_breakdowns::get_requirement_breakdown_session,
        routes::requirement_breakdowns::confirm_requirement_breakdown,
        routes::requirement_breakdowns::confirm_requirement_breakdown_manual,
        routes::requirement_breakdowns::cancel_requirement_breakdown,
        routes::requirement_breakdowns::start_requirement_task_sequence,
        // Dashboard
        routes::dashboard::get_dashboard,
        // Task Attempts
        routes::task_attempts::create_task_attempt,
        routes::task_attempts::get_task_attempts,
        routes::task_attempts::get_attempt,
        routes::task_attempts::get_attempt_logs,
        routes::task_attempts::patch_attempt_log,
        routes::execution_processes::list_execution_processes,
        routes::execution_processes::get_execution_process,
        routes::execution_processes::get_execution_process_raw_logs,
        routes::execution_processes::get_execution_process_normalized_logs,
        routes::execution_processes::follow_up_execution_process,
        routes::execution_processes::reset_execution_process,
        routes::task_attempts::send_attempt_input,
        routes::task_attempts::cancel_attempt,
        // GitLab
        routes::gitlab::list_merge_requests,
        routes::gitlab::get_merge_request_stats,
        routes::gitlab::link_project,
        routes::gitlab::get_status,
        routes::gitlab::get_task_merge_requests,
        routes::gitlab::handle_webhook,
        // Health
        routes::health::health_check,
        routes::health::readiness_check,
        routes::health::liveness_check,
    ),
    components(
        schemas(
            api::UserDto,
            api::AuthResponseDto,
            api::UserResponse,
            api::UserListResponse,
            api::AuthResponse,
            api::EmptyResponse,
            api::ResponseCode,
            api::ApiErrorDetail,
            routes::auth::RegisterRequest,
            routes::auth::LoginRequest,
            routes::users::UpdateUserRequest,
            routes::users::GetUploadUrlRequest,
            routes::users::UploadUrlResponse,
            api::ProjectDto,
            api::ProjectResponse,
            api::ProjectListResponse,
            api::ProjectStackSelectionDoc,
            api::CreateProjectRequestDoc,
            api::UpdateProjectRequestDoc,
            acpms_db::models::ProjectSettings,
            acpms_db::models::ProjectSettingsResponse,
            routes::projects::ImportProjectRequest,
            routes::projects::ImportProjectResponse,
            // Tasks
            api::TaskDto,
            api::TaskResponse,
            api::TaskListResponse,
            api::CreateTaskRequestDoc,
            api::UpdateTaskRequestDoc,
            routes::tasks::UpdateStatusRequest,
            routes::tasks::AssignTaskRequest,
            routes::tasks::UpdateMetadataRequest,
            // Sprints
            api::SprintDto,
            api::SprintResponse,
            api::SprintListResponse,
            api::CreateSprintRequestDoc,
            api::UpdateSprintRequestDoc,
            api::GenerateSprintsRequestDoc,
            api::CreateNextSprintRequestDoc,
            api::CloseSprintRequestDoc,
            api::CloseSprintResultResponse,
            api::SprintOverviewResponse,
            acpms_db::models::SprintCarryOverMode,
            acpms_db::models::CreateNextSprintRequest,
            acpms_db::models::CloseSprintRequest,
            acpms_db::models::CloseSprintResult,
            acpms_db::models::SprintOverview,
            // Requirements
            api::RequirementDto,
            api::RequirementResponse,
            api::RequirementListResponse,
            api::CreateRequirementRequestDoc,
            api::UpdateRequirementRequestDoc,
            // Dashboard
            api::DashboardResponse,
            api::DashboardDataDoc,
            api::DashboardStatsDoc,
            api::StatsMetricDoc,
            api::AgentStatsDoc,
            api::SystemLoadDoc,
            api::PrStatsDoc,
            api::DashboardProjectDoc,
            api::AgentAvatarDoc,
            api::DashboardAgentLogDoc,
            api::DashboardHumanTaskDoc,
            api::UserAvatarDoc,
            // Task Attempts
            api::TaskAttemptDto,
            api::TaskAttemptResponse,
            api::TaskAttemptListResponse,
            api::AgentLogDto,
            api::AgentLogListResponse,
            api::CreateTaskAttemptRequestDoc,
            api::SendInputRequestDoc,
            routes::task_attempts::CancelAttemptRequest,
            routes::task_attempts::ResumeAttemptRequest,
            routes::execution_processes::ExecutionProcessDto,
            // GitLab
            api::GitLabConfigurationDto,
            api::MergeRequestDto,
            api::MergeRequestOverviewDto,
            api::MergeRequestStatsDto,
            api::GitLabConfigurationResponse,
            api::MergeRequestListResponse,
            api::MergeRequestOverviewListResponse,
            api::MergeRequestStatsResponse,
            api::LinkGitLabProjectRequestDoc,
            // Health
            routes::health::HealthStatus,
            routes::health::ComponentHealth,
            routes::health::HealthResponse,
        )
    ),
    tags(
        (name = "Auth", description = "Authentication endpoints"),
        (name = "Users", description = "User management endpoints"),
        (name = "Projects", description = "Project management endpoints"),
        (name = "Tasks", description = "Task management endpoints"),
        (name = "Sprints", description = "Sprint management endpoints"),
        (name = "Requirements", description = "Requirement management endpoints"),
        (name = "Dashboard", description = "Dashboard endpoints"),
        (name = "Task Attempts", description = "Task attempt endpoints"),
        (name = "GitLab", description = "GitLab integration endpoints"),
        (name = "Health", description = "Health check endpoints"),
    )
)]
struct ApiDoc;

fn infer_download_target(artifact_type: &str) -> (&'static str, &'static str) {
    let value = artifact_type.to_ascii_lowercase();
    if value.contains("windows") {
        ("windows", "Windows")
    } else if value.contains("macos") || value.contains("darwin") || value.contains("osx") {
        ("macos", "macOS")
    } else if value.contains("ios") {
        ("ios", "iOS")
    } else if value.contains("android") {
        ("android", "Android")
    } else if value.contains("extension") {
        ("browser", "Browser")
    } else {
        ("generic", "Generic")
    }
}

fn download_rank(os: &str) -> i32 {
    match os {
        "windows" => 0,
        "macos" => 1,
        "ios" => 2,
        "android" => 3,
        "browser" => 4,
        _ => 5,
    }
}

fn finalize_app_downloads(
    project_type: acpms_db::models::ProjectType,
    mut app_downloads: Vec<serde_json::Value>,
) -> (Option<String>, Vec<serde_json::Value>) {
    if project_type == acpms_db::models::ProjectType::Desktop {
        let mut desktop_only: Vec<serde_json::Value> = app_downloads
            .iter()
            .filter(|entry| {
                entry
                    .get("os")
                    .and_then(|value| value.as_str())
                    .map(|os| os == "windows" || os == "macos")
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        if desktop_only.is_empty() {
            desktop_only = app_downloads;
        }
        app_downloads = desktop_only;
    }

    app_downloads.sort_by_key(|entry| {
        entry
            .get("os")
            .and_then(|value| value.as_str())
            .map(download_rank)
            .unwrap_or(99)
    });

    let primary_url = app_downloads
        .first()
        .and_then(|entry| entry.get("url"))
        .and_then(|value| value.as_str())
        .map(ToString::to_string);

    (primary_url, app_downloads)
}

async fn update_task_metadata_patch(
    db: &acpms_db::PgPool,
    task_id: Uuid,
    metadata_patch: serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE tasks
        SET metadata = COALESCE(metadata, '{}'::jsonb) || $2::jsonb,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(task_id)
    .bind(metadata_patch)
    .execute(db)
    .await?;
    Ok(())
}

fn resolve_auto_deploy(task_metadata: &serde_json::Value, project_auto_deploy: bool) -> bool {
    task_metadata
        .get("execution")
        .and_then(|value| value.get("auto_deploy"))
        .and_then(|value| value.as_bool())
        .or_else(|| {
            task_metadata
                .get("auto_deploy")
                .and_then(|value| value.as_bool())
        })
        .unwrap_or(project_auto_deploy)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskSuccessDeliveryMode {
    Preview,
    ArtifactDownloads,
}

fn task_success_delivery_mode(
    project_type: acpms_db::models::ProjectType,
) -> TaskSuccessDeliveryMode {
    match project_type {
        acpms_db::models::ProjectType::Web
        | acpms_db::models::ProjectType::Api
        | acpms_db::models::ProjectType::Microservice => TaskSuccessDeliveryMode::Preview,
        acpms_db::models::ProjectType::Desktop
        | acpms_db::models::ProjectType::Mobile
        | acpms_db::models::ProjectType::Extension => TaskSuccessDeliveryMode::ArtifactDownloads,
    }
}

fn preview_project_type_label(project_type: acpms_db::models::ProjectType) -> &'static str {
    match project_type {
        acpms_db::models::ProjectType::Web => "web",
        acpms_db::models::ProjectType::Api => "api",
        acpms_db::models::ProjectType::Microservice => "microservice",
        acpms_db::models::ProjectType::Extension => "extension",
        acpms_db::models::ProjectType::Desktop => "desktop",
        acpms_db::models::ProjectType::Mobile => "mobile",
    }
}

fn missing_preview_target_message(project_type: acpms_db::models::ProjectType) -> String {
    format!(
        "Agent did not output PREVIEW_TARGET for auto-deploy {} preview.",
        preview_project_type_label(project_type)
    )
}

fn preview_runtime_disabled_message(project_type: acpms_db::models::ProjectType) -> String {
    format!(
        "Preview runtime is disabled. Enable PREVIEW_DOCKER_RUNTIME_ENABLED to auto-publish {} preview after task completion.",
        preview_project_type_label(project_type)
    )
}

#[derive(Debug, Clone)]
struct ArchitectureNodeLite {
    id: String,
    node_type: String,
}

fn task_is_architecture_change(task_metadata: &serde_json::Value) -> bool {
    task_metadata
        .get("source")
        .and_then(|value| value.as_str())
        .map(|value| value.eq_ignore_ascii_case("architecture_change"))
        .unwrap_or(false)
}

fn extract_architecture_nodes(config: Option<&serde_json::Value>) -> Vec<ArchitectureNodeLite> {
    config
        .and_then(|value| value.get("nodes"))
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|node| {
            let id = node.get("id")?.as_str()?.trim();
            if id.is_empty() {
                return None;
            }
            let node_type = node
                .get("type")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("service")
                .to_ascii_lowercase();

            Some(ArchitectureNodeLite {
                id: id.to_string(),
                node_type,
            })
        })
        .collect()
}

fn architecture_requires_backend_code(task_metadata: &serde_json::Value) -> bool {
    if !task_is_architecture_change(task_metadata) {
        return false;
    }

    let old_nodes = extract_architecture_nodes(task_metadata.get("old_architecture"));
    let new_nodes = extract_architecture_nodes(task_metadata.get("new_architecture"));
    let old_ids: HashSet<&str> = old_nodes.iter().map(|node| node.id.as_str()).collect();

    new_nodes
        .into_iter()
        .filter(|node| !old_ids.contains(node.id.as_str()))
        .any(|node| {
            matches!(
                node.node_type.as_str(),
                "api" | "service" | "auth" | "gateway"
            )
        })
}

fn path_is_backend_or_service(path: &str) -> bool {
    let normalized = path.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    let segments: Vec<&str> = normalized.split('/').collect();
    segments.iter().any(|segment| {
        matches!(
            *segment,
            "backend"
                | "server"
                | "api"
                | "apis"
                | "service"
                | "services"
                | "gateway"
                | "gateways"
                | "auth"
        )
    }) || normalized.contains("auth-service")
        || normalized.contains("authentication-service")
        || normalized.contains("oauth")
}

async fn load_attempt_changed_files(
    db: &acpms_db::PgPool,
    storage_service: &Arc<acpms_services::StorageService>,
    attempt: &acpms_db::models::TaskAttempt,
) -> anyhow::Result<Vec<String>> {
    if let Some(key) = attempt.s3_diff_key.as_deref() {
        match storage_service
            .download_json::<acpms_executors::AttemptDiffSnapshot>(key)
            .await
        {
            Ok(snapshot) => {
                let paths = snapshot
                    .files
                    .into_iter()
                    .map(|file| file.path.trim().to_string())
                    .filter(|path| !path.is_empty())
                    .collect::<Vec<_>>();
                if !paths.is_empty() {
                    return Ok(paths);
                }
            }
            Err(error) => {
                tracing::warn!(
                    attempt_id = %attempt.id,
                    s3_diff_key = %key,
                    error = %error,
                    "Failed to load S3 diff snapshot while validating architecture change"
                );
            }
        }
    }

    let db_paths: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT file_path
        FROM file_diffs
        WHERE attempt_id = $1
        ORDER BY file_path
        "#,
    )
    .bind(attempt.id)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    Ok(db_paths
        .into_iter()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect())
}

async fn enforce_architecture_change_output(
    db: &acpms_db::PgPool,
    storage_service: &Arc<acpms_services::StorageService>,
    task: &acpms_db::models::Task,
    attempt: &acpms_db::models::TaskAttempt,
) -> anyhow::Result<()> {
    if !architecture_requires_backend_code(&task.metadata) {
        return Ok(());
    }

    let changed_files = load_attempt_changed_files(db, storage_service, attempt).await?;
    let touched_backend_code = changed_files
        .iter()
        .any(|path| path_is_backend_or_service(path));

    if touched_backend_code {
        return Ok(());
    }

    let changed_preview = if changed_files.is_empty() {
        "none".to_string()
    } else {
        let max_preview = 10usize;
        let listed = changed_files
            .iter()
            .take(max_preview)
            .cloned()
            .collect::<Vec<_>>();
        let extra = changed_files.len().saturating_sub(listed.len());
        if extra > 0 {
            format!("{} (+{} more)", listed.join(", "), extra)
        } else {
            listed.join(", ")
        }
    };

    let message = format!(
        "This task needs backend changes. Please add backend API/service modules for the changed files: {}.",
        changed_preview
    );
    let _ = append_attempt_system_log(db, attempt.id, &message).await;
    anyhow::bail!(message);
}

#[allow(dead_code)]
fn upsert_production_deploy_success(
    metadata_patch: &mut serde_json::Map<String, serde_json::Value>,
    result: &acpms_services::DeployResult,
) {
    metadata_patch.insert(
        "production_deployment_status".to_string(),
        serde_json::Value::String("active".to_string()),
    );
    metadata_patch.insert(
        "production_deployment_url".to_string(),
        serde_json::Value::String(result.url.clone()),
    );
    metadata_patch.insert(
        "production_deployment_type".to_string(),
        serde_json::Value::String(result.deployment_type.clone()),
    );
    metadata_patch.insert(
        "production_deployment_id".to_string(),
        serde_json::Value::String(result.deployment_id.to_string()),
    );
}

#[allow(dead_code)]
fn upsert_production_deploy_failure(
    metadata_patch: &mut serde_json::Map<String, serde_json::Value>,
    status: &str,
    error: String,
) {
    metadata_patch.insert(
        "production_deployment_status".to_string(),
        serde_json::Value::String(status.to_string()),
    );
    metadata_patch.insert(
        "production_deployment_error".to_string(),
        serde_json::Value::String(error),
    );
}

fn is_cloudflare_configured(
    cloudflare_account_id: Option<&str>,
    cloudflare_api_token_encrypted: Option<&str>,
) -> bool {
    cloudflare_account_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        && cloudflare_api_token_encrypted
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
}

#[allow(dead_code)]
fn missing_cloudflare_config_fields(
    cloudflare_account_id: Option<&str>,
    cloudflare_api_token_encrypted: Option<&str>,
) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if cloudflare_account_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        missing.push("cloudflare_account_id");
    }
    if cloudflare_api_token_encrypted
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        missing.push("cloudflare_api_token");
    }
    missing
}

async fn append_attempt_system_log(
    _db: &acpms_db::PgPool,
    attempt_id: Uuid,
    content: &str,
) -> anyhow::Result<()> {
    let id = uuid::Uuid::new_v4();
    let created_at = chrono::Utc::now();
    acpms_executors::append_log_to_jsonl(attempt_id, "system", content, id, created_at).await?;
    Ok(())
}

struct AttemptSuccessDeploymentHook {
    db: acpms_db::PgPool,
    preview_manager: Arc<PreviewManager>,
    build_service: Arc<acpms_services::BuildService>,
    deploy_service: Arc<acpms_services::ProductionDeployService>,
    storage_service: Arc<acpms_services::StorageService>,
}

#[async_trait::async_trait]
impl acpms_executors::AttemptSuccessHook for AttemptSuccessDeploymentHook {
    async fn before_mark_success(&self, attempt_id: Uuid) -> anyhow::Result<()> {
        handle_attempt_success_deployment(
            &self.db,
            &self.preview_manager,
            &self.build_service,
            &self.deploy_service,
            &self.storage_service,
            attempt_id,
        )
        .await
    }
}

async fn handle_attempt_success_deployment(
    db: &acpms_db::PgPool,
    preview_manager: &Arc<PreviewManager>,
    build_service: &Arc<acpms_services::BuildService>,
    _deploy_service: &Arc<acpms_services::ProductionDeployService>,
    storage_service: &Arc<acpms_services::StorageService>,
    attempt_id: Uuid,
) -> anyhow::Result<()> {
    let attempt = sqlx::query_as::<_, acpms_db::models::TaskAttempt>(
        "SELECT * FROM task_attempts WHERE id = $1",
    )
    .bind(attempt_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| anyhow::anyhow!("Attempt {} not found", attempt_id))?;

    let task = sqlx::query_as::<_, acpms_db::models::Task>("SELECT * FROM tasks WHERE id = $1")
        .bind(attempt.task_id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Task {} not found", attempt.task_id))?;

    let project =
        sqlx::query_as::<_, acpms_db::models::Project>("SELECT * FROM projects WHERE id = $1")
            .bind(task.project_id)
            .fetch_optional(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Project {} not found", task.project_id))?;

    // Deploy task: agent deploys directly via SSH (no hook trigger)
    if matches!(task.task_type, acpms_db::models::TaskType::Deploy) {
        return Ok(());
    }

    enforce_architecture_change_output(db, storage_service, &task, &attempt).await?;

    let auto_deploy_enabled = resolve_auto_deploy(&task.metadata, project.settings.auto_deploy);
    let preview_wanted = auto_deploy_enabled || project.settings.preview_enabled;
    let delivery_mode = task_success_delivery_mode(project.project_type);
    let requires_cloudflare_for_preview =
        preview_wanted && matches!(delivery_mode, TaskSuccessDeliveryMode::Preview);

    if requires_cloudflare_for_preview {
        let cloudflare_settings: Option<(Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT cloudflare_account_id, cloudflare_api_token_encrypted
            FROM system_settings
            LIMIT 1
            "#,
        )
        .fetch_optional(db)
        .await?;

        let (account_id, api_token) = cloudflare_settings
            .as_ref()
            .map(|(account_id, api_token)| (account_id.as_deref(), api_token.as_deref()))
            .unwrap_or((None, None));
        let cloudflare_ready = is_cloudflare_configured(account_id, api_token);

        if !cloudflare_ready {
            // Skip deploy only when Cloudflare is not configured. When configured, we must proceed to deploy.
            let message = "Cloudflare is not configured. Configure in System Settings (/settings) to enable preview. Task completed successfully.".to_string();
            let mut metadata_patch = serde_json::Map::new();
            metadata_patch.insert(
                "deployment_status".to_string(),
                serde_json::Value::String("skipped_cloudflare_not_configured".to_string()),
            );
            metadata_patch.insert(
                "deployment_error".to_string(),
                serde_json::Value::String(message.clone()),
            );
            metadata_patch.insert(
                "production_deployment_status".to_string(),
                serde_json::Value::String("skipped_cloudflare_not_configured".to_string()),
            );
            metadata_patch.insert(
                "production_deployment_error".to_string(),
                serde_json::Value::String(message.clone()),
            );
            metadata_patch.insert(
                "deploy_precheck".to_string(),
                serde_json::Value::String("skipped_cloudflare_not_configured".to_string()),
            );
            metadata_patch.insert(
                "deploy_precheck_reason".to_string(),
                serde_json::Value::String(message.clone()),
            );

            update_task_metadata_patch(db, task.id, serde_json::Value::Object(metadata_patch))
                .await?;
            let _ = append_attempt_system_log(db, attempt_id, &message).await;
            return Ok(());
        }
    }

    let _agent_reported_deploy = attempt.metadata.get("preview_target").is_some()
        || attempt.metadata.get("preview_url").is_some()
        || attempt.metadata.get("preview_url_agent").is_some()
        || attempt.metadata.get("deployment_report").is_some();

    let mut metadata_patch = serde_json::Map::new();

    match delivery_mode {
        TaskSuccessDeliveryMode::Preview => {
            let preview_target = attempt
                .metadata
                .get("preview_target")
                .and_then(|value| value.as_str());
            let preview_url = attempt
                .metadata
                .get("preview_url")
                .and_then(|value| value.as_str())
                .or_else(|| {
                    attempt
                        .metadata
                        .get("preview_url_agent")
                        .and_then(|value| value.as_str())
                });

            if let Some(agent_url) = preview_url {
                metadata_patch.insert(
                    "preview_url".to_string(),
                    serde_json::Value::String(agent_url.to_string()),
                );
                if let Some(target) = preview_target {
                    metadata_patch.insert(
                        "preview_target".to_string(),
                        serde_json::Value::String(target.to_string()),
                    );
                }
                metadata_patch.insert(
                    "deployment_kind".to_string(),
                    serde_json::Value::String("agent_preview_url".to_string()),
                );
                metadata_patch.insert(
                    "deployment_status".to_string(),
                    serde_json::Value::String("active".to_string()),
                );
            } else if preview_wanted && preview_target.is_none() {
                metadata_patch.insert(
                    "deployment_status".to_string(),
                    serde_json::Value::String("missing_preview_target".to_string()),
                );
                metadata_patch.insert(
                    "deployment_error".to_string(),
                    serde_json::Value::String(missing_preview_target_message(project.project_type)),
                );
            } else if preview_wanted {
                if !preview_manager.runtime_enabled() {
                    metadata_patch.insert(
                        "deployment_status".to_string(),
                        serde_json::Value::String("skipped_preview_runtime_disabled".to_string()),
                    );
                    metadata_patch.insert(
                        "deployment_error".to_string(),
                        serde_json::Value::String(preview_runtime_disabled_message(
                            project.project_type,
                        )),
                    );
                } else if let Some(preview) = preview_manager
                    .create_preview_if_enabled(
                        &project,
                        attempt_id,
                        &task.title,
                        None,
                        preview_target,
                    )
                    .await?
                {
                    metadata_patch.insert(
                        "preview_url".to_string(),
                        serde_json::Value::String(preview.preview_url.clone()),
                    );
                    if let Some(target) = preview_target {
                        metadata_patch.insert(
                            "preview_target".to_string(),
                            serde_json::Value::String(target.to_string()),
                        );
                    }
                    metadata_patch.insert(
                        "deployment_kind".to_string(),
                        serde_json::Value::String("preview_tunnel".to_string()),
                    );

                    if let Err(e) = preview_manager
                        .start_preview_runtime(attempt_id, project.project_type)
                        .await
                    {
                        tracing::warn!(
                            "Preview tunnel created but runtime start failed for attempt {}: {}",
                            attempt_id,
                            e
                        );
                        metadata_patch.insert(
                            "deployment_status".to_string(),
                            serde_json::Value::String("preview_runtime_failed".to_string()),
                        );
                        metadata_patch.insert(
                            "deployment_error".to_string(),
                            serde_json::Value::String(format!(
                                "Preview URL created but runtime failed to start: {}. You may need to manually start preview.",
                                e
                            )),
                        );
                    } else {
                        metadata_patch.insert(
                            "deployment_status".to_string(),
                            serde_json::Value::String("active".to_string()),
                        );
                    }
                }
            }

            // auto_deploy = preview only; production deploy is via merge webhook (production_deploy_on_merge)
        }
        TaskSuccessDeliveryMode::ArtifactDownloads => {
            if !preview_wanted {
                metadata_patch.insert("app_download_url".to_string(), serde_json::Value::Null);
                metadata_patch.insert(
                    "app_downloads".to_string(),
                    serde_json::Value::Array(Vec::new()),
                );
            } else if let Err(e) = build_service.run_build(&project, attempt_id, None).await {
                tracing::error!(
                    "Build pipeline failed for attempt {} ({:?}): {}",
                    attempt_id,
                    project.project_type,
                    e
                );
            } else {
                let artifacts = build_service.get_attempt_artifacts(attempt_id).await?;
                let _primary_artifact = artifacts.first();
                if !artifacts.is_empty() {
                    let mut app_downloads: Vec<serde_json::Value> = Vec::new();

                    for artifact in &artifacts {
                        let (target, label) = infer_download_target(&artifact.artifact_type);
                        let public_url = storage_service.get_public_url(&artifact.artifact_key);
                        let presigned_url = build_service
                            .get_artifact_download_url(&artifact.artifact_key)
                            .await
                            .ok();

                        app_downloads.push(serde_json::json!({
                            "artifact_id": artifact.id,
                            "artifact_type": artifact.artifact_type,
                            "os": target,
                            "label": label,
                            "url": public_url,
                            "presigned_url": presigned_url,
                            "size_bytes": artifact.size_bytes,
                            "created_at": artifact.created_at,
                        }));
                    }

                    let (primary_url, app_downloads) =
                        finalize_app_downloads(project.project_type, app_downloads);

                    if let Some(primary_url) = primary_url {
                        metadata_patch.insert(
                            "app_download_url".to_string(),
                            serde_json::Value::String(primary_url),
                        );
                    }

                    metadata_patch.insert(
                        "app_downloads".to_string(),
                        serde_json::Value::Array(app_downloads),
                    );
                    metadata_patch.insert(
                        "deployment_kind".to_string(),
                        serde_json::Value::String("artifact_downloads".to_string()),
                    );
                }
            }
        }
    }

    if !metadata_patch.is_empty() {
        update_task_metadata_patch(db, task.id, serde_json::Value::Object(metadata_patch)).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use acpms_deployment::CloudflareClient;
    use acpms_preview::PreviewManager;
    use acpms_services::{
        hash_password, BuildService, EncryptionService, ProductionDeployService, StorageService,
        SystemSettingsService,
    };
    use sqlx::PgPool;
    use std::sync::Arc;
    use tokio::fs;
    use tokio::sync::RwLock;
    use uuid::Uuid;

    struct DeploymentHookTestEnv {
        db: PgPool,
        worktrees_path: Arc<RwLock<std::path::PathBuf>>,
        preview_manager: Arc<PreviewManager>,
        build_service: Arc<BuildService>,
        deploy_service: Arc<ProductionDeployService>,
        storage_service: Arc<StorageService>,
    }

    fn ensure_test_env_defaults() {
        if std::env::var("ENCRYPTION_KEY").is_err() {
            std::env::set_var(
                "ENCRYPTION_KEY",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            );
        }
        if std::env::var("S3_ENDPOINT").is_err() {
            std::env::set_var("S3_ENDPOINT", "http://localhost:9000");
        }
        if std::env::var("S3_PUBLIC_ENDPOINT").is_err() {
            std::env::set_var("S3_PUBLIC_ENDPOINT", "http://localhost:9000");
        }
        if std::env::var("S3_ACCESS_KEY").is_err() {
            std::env::set_var("S3_ACCESS_KEY", "admin");
        }
        if std::env::var("S3_SECRET_KEY").is_err() {
            std::env::set_var("S3_SECRET_KEY", "adminpassword123");
        }
        if std::env::var("S3_REGION").is_err() {
            std::env::set_var("S3_REGION", "us-east-1");
        }
        if std::env::var("S3_BUCKET_NAME").is_err() {
            std::env::set_var("S3_BUCKET_NAME", "acpms-media");
        }
    }

    async fn setup_test_db() -> PgPool {
        ensure_test_env_defaults();
        dotenvy::dotenv().ok();
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:5432/acpms_test".to_string()
        });
        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to test database");
        let _ = sqlx::migrate!("../db/migrations").run(&pool).await;
        pool
    }

    async fn create_deployment_hook_test_env(pool: PgPool) -> DeploymentHookTestEnv {
        ensure_test_env_defaults();

        let worktrees_path = Arc::new(RwLock::new(
            std::env::temp_dir().join(format!("acpms-main-tests-worktrees-{}", Uuid::new_v4())),
        ));
        let worktrees_dir = worktrees_path.read().await.clone();
        fs::create_dir_all(&worktrees_dir)
            .await
            .expect("failed to create test worktrees dir");

        let encryption_service = Arc::new(
            EncryptionService::new(
                &std::env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY must exist in tests"),
            )
            .expect("failed to init EncryptionService"),
        );
        let settings_service = Arc::new(
            SystemSettingsService::new(pool.clone()).expect("failed to init settings service"),
        );
        let cloudflare_client = CloudflareClient::new(
            std::env::var("CLOUDFLARE_API_TOKEN").unwrap_or_default(),
            std::env::var("CLOUDFLARE_ACCOUNT_ID").unwrap_or_default(),
        )
        .expect("failed to init cloudflare client");
        let preview_manager = Arc::new(PreviewManager::new(
            cloudflare_client,
            (*encryption_service).clone(),
            (*settings_service).clone(),
            pool.clone(),
            Some(7),
        ));
        let storage_service = Arc::new(
            StorageService::new()
                .await
                .expect("failed to init storage service"),
        );
        let build_service = Arc::new(BuildService::new(
            (*storage_service).clone(),
            pool.clone(),
            worktrees_path.clone(),
        ));
        let deploy_service = Arc::new(ProductionDeployService::new(
            pool.clone(),
            (*settings_service).clone(),
            (*encryption_service).clone(),
        ));

        DeploymentHookTestEnv {
            db: pool,
            worktrees_path,
            preview_manager,
            build_service,
            deploy_service,
            storage_service,
        }
    }

    async fn create_test_user(pool: &PgPool) -> Uuid {
        let user_id = Uuid::new_v4();
        let email = format!("test-{}@example.com", user_id);
        let password_hash = hash_password("testpassword123").expect("failed to hash test password");
        sqlx::query(
            r#"
            INSERT INTO users (id, email, name, password_hash, global_roles)
            VALUES ($1, $2, $3, $4, ARRAY['viewer']::system_role[])
            "#,
        )
        .bind(user_id)
        .bind(email)
        .bind("Test User")
        .bind(password_hash)
        .execute(pool)
        .await
        .expect("failed to create test user");
        user_id
    }

    async fn create_test_project(pool: &PgPool, created_by: Uuid, name: &str) -> Uuid {
        let project_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO projects (id, name, description, created_by, metadata, architecture_config, require_review, project_type, settings)
            VALUES ($1, $2, $3, $4, '{}'::jsonb, '{}'::jsonb, true, 'web', '{}'::jsonb)
            "#,
        )
        .bind(project_id)
        .bind(name)
        .bind("Test Description")
        .bind(created_by)
        .execute(pool)
        .await
        .expect("failed to create test project");

        sqlx::query(
            r#"
            INSERT INTO project_members (project_id, user_id, roles)
            VALUES ($1, $2, ARRAY['owner']::project_role[])
            "#,
        )
        .bind(project_id)
        .bind(created_by)
        .execute(pool)
        .await
        .expect("failed to create project membership");

        project_id
    }

    async fn create_test_task(
        pool: &PgPool,
        project_id: Uuid,
        created_by: Uuid,
        title: &str,
    ) -> Uuid {
        let task_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO tasks (id, project_id, title, description, task_type, status, created_by, metadata)
            VALUES ($1, $2, $3, $4, 'feature', 'todo', $5, '{}'::jsonb)
            "#,
        )
        .bind(task_id)
        .bind(project_id)
        .bind(title)
        .bind("Test Task Description")
        .bind(created_by)
        .execute(pool)
        .await
        .expect("failed to create test task");
        task_id
    }

    async fn create_test_attempt(pool: &PgPool, task_id: Uuid, status: &str) -> Uuid {
        let attempt_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO task_attempts (id, task_id, status, metadata)
            VALUES ($1, $2, $3::attempt_status, '{}'::jsonb)
            "#,
        )
        .bind(attempt_id)
        .bind(task_id)
        .bind(status)
        .execute(pool)
        .await
        .expect("failed to create test attempt");
        attempt_id
    }

    async fn cleanup_test_data(pool: &PgPool, user_id: Uuid, project_id: Uuid) {
        let _ = sqlx::query("DELETE FROM tasks WHERE project_id = $1")
            .bind(project_id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM project_members WHERE project_id = $1")
            .bind(project_id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM projects WHERE id = $1")
            .bind(project_id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
    }

    async fn configure_binary_preview_project(
        pool: &PgPool,
        project_id: Uuid,
        project_type: acpms_db::models::ProjectType,
        build_command: &str,
        build_output_dir: &str,
    ) {
        sqlx::query(
            r#"
            UPDATE projects
            SET
                project_type = $2,
                metadata = COALESCE(metadata, '{}'::jsonb) || jsonb_build_object(
                    'build_command', $3,
                    'build_output_dir', $4
                ),
                settings = COALESCE(settings, '{}'::jsonb) || '{"auto_deploy": false, "preview_enabled": true}'::jsonb
            WHERE id = $1
            "#,
        )
        .bind(project_id)
        .bind(project_type)
        .bind(build_command)
        .bind(build_output_dir)
        .execute(pool)
        .await
        .expect("failed to configure binary preview project");
    }

    async fn create_attempt_worktree(
        worktrees_path: &std::sync::Arc<tokio::sync::RwLock<std::path::PathBuf>>,
        attempt_id: Uuid,
    ) -> std::path::PathBuf {
        let base = worktrees_path.read().await.clone();
        let path = base.join(format!("attempt-{}", attempt_id));
        fs::create_dir_all(&path)
            .await
            .expect("failed to create attempt worktree");
        path
    }

    async fn load_task_metadata(pool: &PgPool, task_id: Uuid) -> serde_json::Value {
        sqlx::query_scalar::<_, serde_json::Value>("SELECT metadata FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_one(pool)
            .await
            .expect("failed to load task metadata")
    }

    #[test]
    fn resolve_auto_deploy_prefers_execution_override() {
        let metadata = serde_json::json!({
            "execution": {
                "auto_deploy": true
            },
            "auto_deploy": false
        });

        assert!(resolve_auto_deploy(&metadata, false));
    }

    #[test]
    fn resolve_auto_deploy_falls_back_to_legacy_field() {
        let metadata = serde_json::json!({
            "auto_deploy": true
        });

        assert!(resolve_auto_deploy(&metadata, false));
    }

    #[test]
    fn resolve_auto_deploy_uses_project_default_when_not_set() {
        let metadata = serde_json::json!({});

        assert!(resolve_auto_deploy(&metadata, true));
        assert!(!resolve_auto_deploy(&metadata, false));
    }

    #[test]
    fn task_success_delivery_mode_routes_preview_capable_project_types_to_preview() {
        assert_eq!(
            task_success_delivery_mode(acpms_db::models::ProjectType::Web),
            TaskSuccessDeliveryMode::Preview
        );
        assert_eq!(
            task_success_delivery_mode(acpms_db::models::ProjectType::Api),
            TaskSuccessDeliveryMode::Preview
        );
        assert_eq!(
            task_success_delivery_mode(acpms_db::models::ProjectType::Microservice),
            TaskSuccessDeliveryMode::Preview
        );
    }

    #[test]
    fn task_success_delivery_mode_routes_binary_project_types_to_artifact_downloads() {
        assert_eq!(
            task_success_delivery_mode(acpms_db::models::ProjectType::Desktop),
            TaskSuccessDeliveryMode::ArtifactDownloads
        );
        assert_eq!(
            task_success_delivery_mode(acpms_db::models::ProjectType::Mobile),
            TaskSuccessDeliveryMode::ArtifactDownloads
        );
        assert_eq!(
            task_success_delivery_mode(acpms_db::models::ProjectType::Extension),
            TaskSuccessDeliveryMode::ArtifactDownloads
        );
    }

    #[test]
    fn missing_preview_target_message_uses_project_type_specific_label() {
        assert_eq!(
            missing_preview_target_message(acpms_db::models::ProjectType::Extension),
            "Agent did not output PREVIEW_TARGET for auto-deploy extension preview."
        );
        assert_eq!(
            missing_preview_target_message(acpms_db::models::ProjectType::Api),
            "Agent did not output PREVIEW_TARGET for auto-deploy api preview."
        );
    }

    #[test]
    fn preview_runtime_disabled_message_is_project_type_specific() {
        assert_eq!(
            preview_runtime_disabled_message(acpms_db::models::ProjectType::Microservice),
            "Preview runtime is disabled. Enable PREVIEW_DOCKER_RUNTIME_ENABLED to auto-publish microservice preview after task completion."
        );
    }

    #[test]
    fn cloudflare_configured_requires_account_and_token() {
        assert!(is_cloudflare_configured(Some("account"), Some("token")));
        assert!(!is_cloudflare_configured(Some("account"), None));
        assert!(!is_cloudflare_configured(None, Some("token")));
        assert!(!is_cloudflare_configured(Some(" "), Some("token")));
        assert!(!is_cloudflare_configured(Some("account"), Some(" ")));
    }

    #[test]
    fn missing_cloudflare_fields_reports_exact_missing_keys() {
        assert_eq!(
            missing_cloudflare_config_fields(None, None),
            vec!["cloudflare_account_id", "cloudflare_api_token"]
        );
        assert_eq!(
            missing_cloudflare_config_fields(Some("account"), None),
            vec!["cloudflare_api_token"]
        );
        assert_eq!(
            missing_cloudflare_config_fields(None, Some("token")),
            vec!["cloudflare_account_id"]
        );
        assert!(missing_cloudflare_config_fields(Some("account"), Some("token")).is_empty());
    }

    #[test]
    fn finalize_app_downloads_prefers_native_desktop_installers() {
        let app_downloads = vec![
            serde_json::json!({ "os": "generic", "url": "https://example.test/bundle.zip" }),
            serde_json::json!({ "os": "macos", "url": "https://example.test/app.dmg" }),
            serde_json::json!({ "os": "windows", "url": "https://example.test/app.exe" }),
        ];

        let (primary_url, filtered) =
            finalize_app_downloads(acpms_db::models::ProjectType::Desktop, app_downloads);

        assert_eq!(primary_url.as_deref(), Some("https://example.test/app.exe"));
        assert_eq!(filtered.len(), 2);
        assert_eq!(
            filtered[0].get("os").and_then(|value| value.as_str()),
            Some("windows")
        );
        assert_eq!(
            filtered[1].get("os").and_then(|value| value.as_str()),
            Some("macos")
        );
    }

    #[test]
    fn finalize_app_downloads_keeps_desktop_bundle_when_no_native_installer_exists() {
        let app_downloads = vec![
            serde_json::json!({ "os": "generic", "url": "https://example.test/bundle.tar.gz" }),
            serde_json::json!({ "os": "browser", "url": "https://example.test/web.zip" }),
        ];

        let (primary_url, filtered) =
            finalize_app_downloads(acpms_db::models::ProjectType::Desktop, app_downloads);

        assert_eq!(primary_url.as_deref(), Some("https://example.test/web.zip"));
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn finalize_app_downloads_preserves_extension_entries_and_browser_priority() {
        let app_downloads = vec![
            serde_json::json!({ "os": "generic", "url": "https://example.test/source.zip" }),
            serde_json::json!({ "os": "browser", "url": "https://example.test/extension.zip" }),
        ];

        let (primary_url, filtered) =
            finalize_app_downloads(acpms_db::models::ProjectType::Extension, app_downloads);

        assert_eq!(
            primary_url.as_deref(),
            Some("https://example.test/extension.zip")
        );
        assert_eq!(filtered.len(), 2);
        assert_eq!(
            filtered[0].get("os").and_then(|value| value.as_str()),
            Some("browser")
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL test db + MinIO; run manually for artifact delivery integration"]
    async fn handle_attempt_success_deployment_builds_desktop_artifacts_and_updates_task_metadata()
    {
        let pool = setup_test_db().await;
        let test_env = create_deployment_hook_test_env(pool.clone()).await;

        let user_id = create_test_user(&pool).await;
        let project_id = create_test_project(&pool, user_id, "Desktop Artifact Project").await;
        let task_id = create_test_task(&pool, project_id, user_id, "Build Desktop Artifact").await;
        let attempt_id = create_test_attempt(&pool, task_id, "success").await;

        configure_binary_preview_project(
            &pool,
            project_id,
            acpms_db::models::ProjectType::Desktop,
            "mkdir -p out && printf windows > out/app-win.exe && printf macos > out/app-mac.dmg",
            "out",
        )
        .await;

        let worktree_path = create_attempt_worktree(&test_env.worktrees_path, attempt_id).await;

        handle_attempt_success_deployment(
            &test_env.db,
            &test_env.preview_manager,
            &test_env.build_service,
            &test_env.deploy_service,
            &test_env.storage_service,
            attempt_id,
        )
        .await
        .expect("desktop artifact delivery should succeed");

        let metadata = load_task_metadata(&pool, task_id).await;
        assert_eq!(
            metadata
                .get("deployment_kind")
                .and_then(|value| value.as_str()),
            Some("artifact_downloads")
        );

        let app_download_url = metadata
            .get("app_download_url")
            .and_then(|value| value.as_str())
            .expect("missing primary app_download_url");
        assert!(
            app_download_url.contains("/builds/"),
            "expected public artifact URL, got: {}",
            app_download_url
        );

        let app_downloads = metadata
            .get("app_downloads")
            .and_then(|value| value.as_array())
            .expect("missing app_downloads array");
        assert_eq!(app_downloads.len(), 2);
        assert_eq!(
            app_downloads[0].get("os").and_then(|value| value.as_str()),
            Some("windows")
        );
        assert_eq!(
            app_downloads[1].get("os").and_then(|value| value.as_str()),
            Some("macos")
        );
        assert!(app_downloads.iter().all(|entry| {
            entry
                .get("presigned_url")
                .and_then(|value| value.as_str())
                .map(|url| url.contains("/builds/"))
                .unwrap_or(false)
        }));

        let artifact_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM build_artifacts WHERE attempt_id = $1",
        )
        .bind(attempt_id)
        .fetch_one(&pool)
        .await
        .expect("failed to count build artifacts");
        assert_eq!(artifact_count, 2);

        let _ = fs::remove_dir_all(worktree_path).await;
        cleanup_test_data(&pool, user_id, project_id).await;
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL test db + MinIO; run manually for artifact delivery integration"]
    async fn handle_attempt_success_deployment_builds_extension_artifact_and_updates_task_metadata()
    {
        let pool = setup_test_db().await;
        let test_env = create_deployment_hook_test_env(pool.clone()).await;

        let user_id = create_test_user(&pool).await;
        let project_id = create_test_project(&pool, user_id, "Extension Artifact Project").await;
        let task_id =
            create_test_task(&pool, project_id, user_id, "Build Extension Artifact").await;
        let attempt_id = create_test_attempt(&pool, task_id, "success").await;

        configure_binary_preview_project(
            &pool,
            project_id,
            acpms_db::models::ProjectType::Extension,
            "mkdir -p ext && printf extension-bundle > ext/qa-extension.zip",
            "ext",
        )
        .await;

        let worktree_path = create_attempt_worktree(&test_env.worktrees_path, attempt_id).await;

        handle_attempt_success_deployment(
            &test_env.db,
            &test_env.preview_manager,
            &test_env.build_service,
            &test_env.deploy_service,
            &test_env.storage_service,
            attempt_id,
        )
        .await
        .expect("extension artifact delivery should succeed");

        let metadata = load_task_metadata(&pool, task_id).await;
        assert_eq!(
            metadata
                .get("deployment_kind")
                .and_then(|value| value.as_str()),
            Some("artifact_downloads")
        );
        assert!(
            metadata.get("preview_url").is_none(),
            "extension delivery must not create a preview URL"
        );

        let app_downloads = metadata
            .get("app_downloads")
            .and_then(|value| value.as_array())
            .expect("missing app_downloads array");
        assert_eq!(app_downloads.len(), 1);
        assert_eq!(
            app_downloads[0].get("os").and_then(|value| value.as_str()),
            Some("browser")
        );
        assert_eq!(
            app_downloads[0]
                .get("label")
                .and_then(|value| value.as_str()),
            Some("Browser")
        );
        assert!(app_downloads[0]
            .get("url")
            .and_then(|value| value.as_str())
            .map(|url| url.contains("/builds/"))
            .unwrap_or(false));

        let artifact_type = sqlx::query_scalar::<_, String>(
            "SELECT artifact_type FROM build_artifacts WHERE attempt_id = $1 LIMIT 1",
        )
        .bind(attempt_id)
        .fetch_one(&pool)
        .await
        .expect("failed to load extension artifact type");
        assert_eq!(artifact_type, "extension_zip");

        let _ = fs::remove_dir_all(worktree_path).await;
        cleanup_test_data(&pool, user_id, project_id).await;
    }

    #[test]
    fn architecture_requires_backend_code_detects_added_api_node() {
        let metadata = serde_json::json!({
            "source": "architecture_change",
            "old_architecture": {
                "nodes": [
                    { "id": "browser-ext", "type": "frontend" }
                ]
            },
            "new_architecture": {
                "nodes": [
                    { "id": "browser-ext", "type": "frontend" },
                    { "id": "authen-service", "type": "api" }
                ]
            }
        });

        assert!(architecture_requires_backend_code(&metadata));
    }

    #[test]
    fn architecture_requires_backend_code_ignores_non_architecture_tasks() {
        let metadata = serde_json::json!({
            "source": "manual_task",
            "old_architecture": { "nodes": [] },
            "new_architecture": {
                "nodes": [
                    { "id": "authen-service", "type": "api" }
                ]
            }
        });

        assert!(!architecture_requires_backend_code(&metadata));
    }

    #[test]
    fn path_is_backend_or_service_matches_service_paths() {
        assert!(path_is_backend_or_service("services/auth/index.ts"));
        assert!(path_is_backend_or_service("backend/src/main.rs"));
        assert!(path_is_backend_or_service("api/routes/auth.ts"));
        assert!(!path_is_backend_or_service("manifest.json"));
        assert!(!path_is_backend_or_service("src/popup/index.ts"));
    }
}

#[derive(clap::Parser)]
#[command(name = "acpms-server")]
struct Cli {
    /// Run database migrations and exit
    #[arg(long)]
    migrate: bool,

    /// Create super admin user (requires ADMIN_PASSWORD env)
    #[arg(long, value_name = "EMAIL")]
    create_admin: Option<String>,

    /// Remove legacy seeded admin account (admin@acpms.local/admin123) if at least one other admin exists
    #[arg(long)]
    remove_seeded_admin: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize structured logging
    init_logging();

    // Load environment variables
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    // --migrate: run migrations and exit
    if cli.migrate {
        let database_url = std::env::var("DATABASE_URL")
            .context("DATABASE_URL environment variable must be set")?;
        let pool = acpms_db::PgPoolOptions::new()
            .min_connections(1)
            .max_connections(2)
            .connect(&database_url)
            .await?;
        sqlx::migrate!("../db/migrations").run(&pool).await?;
        tracing::info!("Database migrations completed");
        return Ok(());
    }

    // --create-admin: ensure one admin (create only if none; else update password if email matches)
    if let Some(admin_email) = cli.create_admin {
        let pass = std::env::var("ADMIN_PASSWORD")
            .context("ADMIN_PASSWORD environment variable must be set for --create-admin")?;
        if pass.len() < 12 {
            anyhow::bail!("ADMIN_PASSWORD must be at least 12 characters");
        }
        let database_url = std::env::var("DATABASE_URL")
            .context("DATABASE_URL environment variable must be set")?;
        let pool = acpms_db::PgPoolOptions::new()
            .min_connections(1)
            .max_connections(2)
            .connect(&database_url)
            .await?;
        let password_hash = acpms_services::hash_password(&pass)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;
        let user_service = acpms_services::UserService::new(pool.clone());
        let has_admin = user_service.has_any_admin().await?;
        let existing = user_service.get_user_by_email(&admin_email).await?;

        if let Some(user) = existing {
            // User với email này đã tồn tại: luôn cập nhật password để đăng nhập được với mật khẩu vừa nhập
            user_service
                .change_password(user.id, password_hash.clone())
                .await
                .map_err(|e| anyhow::anyhow!("Failed to update password: {}", e))?;
            let is_admin = user
                .global_roles
                .contains(&acpms_db::models::SystemRole::Admin);
            if !is_admin {
                let mut roles = user.global_roles.clone();
                roles.push(acpms_db::models::SystemRole::Admin);
                user_service
                    .update_user(user.id, None, None, None, Some(roles))
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to add admin role: {}", e))?;
                tracing::info!("Updated password and added admin role for: {}", admin_email);
            } else {
                tracing::info!("Updated password for admin: {}", admin_email);
            }
        } else if !has_admin {
            user_service
                .create_user(
                    &admin_email,
                    &admin_email,
                    &password_hash,
                    &[acpms_db::models::SystemRole::Admin],
                )
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create admin: {}", e))?;
            tracing::info!("Created admin user: {}", admin_email);
        } else {
            // Đã có admin khác và email này chưa có trong hệ thống → tạo user mới làm admin
            user_service
                .create_user(
                    &admin_email,
                    &admin_email,
                    &password_hash,
                    &[acpms_db::models::SystemRole::Admin],
                )
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create admin: {}", e))?;
            tracing::info!("Created additional admin user: {}", admin_email);
        }
        return Ok(());
    }

    // --remove-seeded-admin: remove legacy default seeded admin when safe
    if cli.remove_seeded_admin {
        const SEEDED_EMAIL: &str = "admin@acpms.local";
        const SEEDED_NAME: &str = "Admin User";
        const SEEDED_HASH: &str = "$2y$12$ovlS6fjllYtHTCmNjNANPegmUp96x.67NXlc.cPoWTcEurDB4rbJK";

        let database_url = std::env::var("DATABASE_URL")
            .context("DATABASE_URL environment variable must be set")?;
        let pool = acpms_db::PgPoolOptions::new()
            .min_connections(1)
            .max_connections(2)
            .connect(&database_url)
            .await?;

        let non_seed_admin_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM users
            WHERE email <> $1
              AND 'admin'::system_role = ANY(global_roles)
            "#,
        )
        .bind(SEEDED_EMAIL)
        .fetch_one(&pool)
        .await?;

        if non_seed_admin_count <= 0 {
            tracing::warn!(
                "Skip removing seeded admin: no other admin account exists yet (email != {})",
                SEEDED_EMAIL
            );
            return Ok(());
        }

        let result = sqlx::query(
            r#"
            DELETE FROM users
            WHERE email = $1
              AND name = $2
              AND password_hash = $3
            "#,
        )
        .bind(SEEDED_EMAIL)
        .bind(SEEDED_NAME)
        .bind(SEEDED_HASH)
        .execute(&pool)
        .await?;

        if result.rows_affected() > 0 {
            tracing::info!(
                "Removed legacy seeded admin account: {} (rows={})",
                SEEDED_EMAIL,
                result.rows_affected()
            );
        } else {
            tracing::info!("No legacy seeded admin account found to remove");
        }

        return Ok(());
    }

    // Validate JWT_SECRET exists
    std::env::var("JWT_SECRET").context("JWT_SECRET environment variable must be set")?;

    // Database connection with optimized pool settings
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL environment variable must be set")?;

    let pool = acpms_db::PgPoolOptions::new()
        .min_connections(5)
        .max_connections(20)
        .acquire_timeout(std::time::Duration::from_secs(3))
        .idle_timeout(std::time::Duration::from_secs(600)) // 10 minutes
        .max_lifetime(std::time::Duration::from_secs(1800)) // 30 minutes
        .connect(&database_url)
        .await?;

    // Run migrations
    sqlx::migrate!("../db/migrations").run(&pool).await?;

    tracing::info!("Database migrations completed");

    // Initialize Executor Components
    let (broadcast_tx, _) = broadcast::channel(100); // Buffer size 100
    let settings_service = Arc::new(SystemSettingsService::new(pool.clone())?);
    let worktrees_path_str = settings_service
        .get_worktrees_path()
        .await
        .context("Failed to get worktrees path")?;
    tracing::info!("Worktrees path: {}", worktrees_path_str);
    let worktrees_path = Arc::new(tokio::sync::RwLock::new(PathBuf::from(&worktrees_path_str)));
    use crate::services::deployment_worker_pool::{DeploymentJob, DeploymentWorkerPool};
    use crate::services::project_assistant_worker_pool::{
        ProjectAssistantJobHandler, ProjectAssistantWorkerPool,
    };
    use acpms_executors::{ProjectAssistantJob, WorkerPool, WorkerPoolConfig};
    use acpms_services::{
        BuildService, EncryptionService, GitLabOAuthService, GitLabService, GitLabSyncService,
        ProductionDeployService, SprintService, StorageService, SystemSettingsService, UserService,
        WebhookAdminService, WebhookManager,
    };

    // Initialize encryption service
    let encryption_key = std::env::var("ENCRYPTION_KEY").context("ENCRYPTION_KEY must be set")?;
    let encryption_service = Arc::new(EncryptionService::new(&encryption_key)?);

    // Initialize Services
    let gitlab_service = Arc::new(GitLabService::new(pool.clone())?);
    let gitlab_sync_service = Arc::new(GitLabSyncService::new(
        pool.clone(),
        (*gitlab_service).clone(),
    ));
    let user_service = UserService::new(pool.clone());
    let sprint_service = SprintService::new(pool.clone());
    let webhook_manager = Arc::new(WebhookManager::new(pool.clone()));
    let gitlab_oauth_service = Arc::new(GitLabOAuthService::from_env(pool.clone())?);
    let webhook_admin_service = Arc::new(WebhookAdminService::new(pool.clone()));

    // Initialize StorageService with graceful degradation
    let storage_service = match StorageService::new().await {
        Ok(service) => {
            tracing::info!("StorageService initialized successfully");
            Arc::new(service)
        }
        Err(e) => {
            tracing::warn!(
                "Failed to initialize StorageService (non-fatal for testing): {}",
                e
            );
            tracing::warn!("Avatar uploads will not work until S3 is configured");
            // Create a dummy service that will fail gracefully on operations
            // For now, we'll skip StorageService in AppState
            return Err(e.context("StorageService initialization failed"));
        }
    };

    // Initialize Cloudflare client for preview manager
    let cloudflare_client = acpms_deployment::CloudflareClient::new(
        std::env::var("CLOUDFLARE_API_TOKEN").unwrap_or_default(),
        std::env::var("CLOUDFLARE_ACCOUNT_ID").unwrap_or_default(),
    )?;

    let preview_manager = Arc::new(PreviewManager::new(
        cloudflare_client,
        (*encryption_service).clone(),
        (*settings_service).clone(),
        pool.clone(),
        Some(7), // TTL 7 days
    ));

    // Initialize Build and Deploy Services
    let build_service = Arc::new(BuildService::new(
        (*storage_service).clone(),
        pool.clone(),
        worktrees_path.clone(),
    ));

    let deploy_service = Arc::new(ProductionDeployService::new(
        pool.clone(),
        (*settings_service).clone(),
        (*encryption_service).clone(),
    ));

    let attempt_success_hook = Arc::new(AttemptSuccessDeploymentHook {
        db: pool.clone(),
        preview_manager: preview_manager.clone(),
        build_service: build_service.clone(),
        deploy_service: deploy_service.clone(),
        storage_service: storage_service.clone(),
    });

    acpms_executors::init_agent_log_buffer(pool.clone());

    // R7: Upload JSONL logs to S3 when attempt completes (success/failed/cancelled)
    spawn_log_upload_on_complete(
        broadcast_tx.subscribe(),
        pool.clone(),
        storage_service.clone(),
    );

    let deploy_context_preparer = Arc::new(
        acpms_server::deploy_context_preparer::ServerDeployContextPreparer::new(
            pool.clone(),
            encryption_service.clone(),
        ),
    );

    let skill_roots = acpms_executors::discover_global_skill_roots();
    let skill_knowledge = if skill_roots.iter().any(|root| root.path.is_dir()) {
        let handle = acpms_executors::SkillKnowledgeHandle::pending();
        let build_handle = handle.clone();
        tokio::spawn(async move {
            match tokio::task::spawn_blocking(move || {
                acpms_executors::KnowledgeIndex::build(skill_roots)
            })
            .await
            {
                Ok(Ok(index)) => {
                    let count = build_handle.set_ready_index(index);
                    tracing::info!(skills = count, "Knowledge index built");
                }
                Ok(Err(error)) => {
                    tracing::warn!(
                        "Failed to build knowledge index in background (non-fatal): {}",
                        error
                    );
                    build_handle.set_failed(error.to_string());
                }
                Err(error) => {
                    tracing::warn!("Knowledge index build task panicked (non-fatal): {}", error);
                    build_handle.set_failed(error.to_string());
                }
            }
        });
        handle
    } else {
        tracing::info!("No global skill roots found, skill knowledge disabled");
        acpms_executors::SkillKnowledgeHandle::disabled()
    };

    let orchestrator = Arc::new(
        ExecutorOrchestrator::new(
            pool.clone(),
            worktrees_path.clone(),
            broadcast_tx.clone(),
            storage_service.clone() as Arc<dyn acpms_executors::DiffStorageUploader>,
        )?
        .with_attempt_success_hook(attempt_success_hook)
        .with_deploy_context_preparer(deploy_context_preparer)
        .with_skill_knowledge(skill_knowledge),
    );

    // Initialize Worker Pool
    let worker_count = std::env::var("WORKER_POOL_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let worker_config = WorkerPoolConfig::default().with_worker_count(worker_count);

    let worker_pool = Arc::new(WorkerPool::new(orchestrator.clone(), worker_config));
    worker_pool.start();

    tracing::info!("Worker pool initialized with {} workers", worker_count);

    // Initialize deployment worker pool
    let deployment_worker_count = std::env::var("DEPLOYMENT_WORKER_POOL_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);

    // Initialize metrics
    let metrics = Metrics::new()?;
    tracing::info!("Prometheus metrics initialized");

    // Phase 3: Initialize JSON Patch streaming infrastructure
    let patch_store = Arc::new(acpms_services::PatchStore::new(100)); // Keep last 100 patches
    let stream_service = Arc::new(
        acpms_services::StreamService::new(patch_store.clone(), broadcast_tx.clone(), pool.clone())
            .with_storage(storage_service.clone()),
    );
    tracing::info!("JSON Patch streaming infrastructure initialized");

    // Create AppState
    let mut state = AppState {
        worktrees_path: worktrees_path,
        db: pool.clone(),
        metrics: metrics.clone(),
        orchestrator,
        worker_pool: Some(worker_pool.clone()),
        deployment_worker_pool: None,
        project_assistant_worker_pool: None,
        broadcast_tx: broadcast_tx.clone(),
        gitlab_service,
        gitlab_sync_service,
        user_service,
        sprint_service,
        webhook_manager,
        gitlab_oauth_service,
        webhook_admin_service,
        settings_service,
        preview_manager: preview_manager.clone(),
        storage_service: storage_service.clone(),
        build_service: build_service.clone(),
        deploy_service,
        patch_store,
        stream_service,
        auth_session_store: Arc::new(crate::services::agent_auth::AuthSessionStore::new()),
    };

    let deployment_handler_state = state.clone();
    let deployment_handler = Arc::new(move |job: DeploymentJob| {
        let handler_state = deployment_handler_state.clone();
        Box::pin(async move {
            routes::deployments::process_deployment_run_background(handler_state, job.run_id).await;
        }) as futures::future::BoxFuture<'static, ()>
    });
    let deployment_worker_pool = Arc::new(DeploymentWorkerPool::new(
        deployment_worker_count,
        deployment_handler,
    ));
    deployment_worker_pool.start();
    state.deployment_worker_pool = Some(deployment_worker_pool.clone());
    tracing::info!(
        "Deployment worker pool initialized with {} workers",
        deployment_worker_count
    );

    // Initialize Project Assistant worker pool
    let project_assistant_handler_state = state.clone();
    let project_assistant_handler: ProjectAssistantJobHandler =
        Arc::new(move |job: ProjectAssistantJob| {
            let handler_state = project_assistant_handler_state.clone();
            Box::pin(async move {
                routes::project_assistant::process_project_assistant_job(handler_state, job).await;
            }) as futures::future::BoxFuture<'static, ()>
        });
    let project_assistant_worker_pool = Arc::new(ProjectAssistantWorkerPool::new(
        2,
        project_assistant_handler,
    ));
    project_assistant_worker_pool.start();
    state.project_assistant_worker_pool = Some(project_assistant_worker_pool.clone());
    tracing::info!("Project Assistant worker pool initialized");

    // Build application with middleware layers
    let metrics_route_metrics = metrics.clone();
    let app = routes::create_router(state)
        .route(
            "/metrics",
            axum::routing::get(move || {
                let metrics = metrics_route_metrics.clone();
                async move {
                    metrics
                        .encode()
                        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                }
            }),
        )
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        // Middleware layers (innermost to outermost)
        .layer(axum_middleware::from_fn_with_state(
            metrics.clone(),
            middleware::metrics_middleware,
        ))
        .layer(axum_middleware::from_fn(request_id::request_id_middleware))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    // Spawn Cleanup Job
    let preview_manager_cleanup = preview_manager.clone();
    tokio::spawn(async move {
        tracing::info!("Starting Preview Cleanup Job");
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // Every hour

        loop {
            interval.tick().await;
            if let Err(e) = preview_manager_cleanup.cleanup_expired_previews().await {
                tracing::error!("Failed to cleanup expired previews: {}", e);
            }
        }
    });

    // Run server with graceful shutdown (bind 0.0.0.0 so LAN can connect)
    let port: u16 = std::env::var("ACPMS_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Server listening on {}", addr);
    tracing::info!(
        "Swagger UI available at http://localhost:{}/swagger-ui/",
        port
    );
    tracing::info!("Health check available at http://localhost:{}/health", port);
    tracing::info!("Metrics available at http://localhost:{}/metrics", port);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Graceful shutdown handler
    let shutdown_signal = async move {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!("Failed to install CTRL+C signal handler: {}", err);
            return;
        }
        tracing::info!("Shutdown signal received, starting graceful shutdown...");

        deployment_worker_pool.stop().await;
        tracing::info!("Deployment worker pool stopped");

        // Stop worker pool
        worker_pool.stop().await;
        tracing::info!("Worker pool stopped");

        // Close database connections
        pool.close().await;
        tracing::info!("Database connections closed");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    tracing::info!("Server shutdown complete");
    Ok(())
}
