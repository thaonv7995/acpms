use crate::observability::Metrics;
use crate::services::agent_auth::AuthSessionStore;
use crate::services::deployment_worker_pool::DeploymentWorkerPool;
use crate::services::project_assistant_worker_pool::ProjectAssistantWorkerPool;
use acpms_executors::{AgentEvent, ExecutorOrchestrator, WorkerPool};
use acpms_preview::PreviewManager;
use acpms_services::{
    BuildService, GitLabOAuthService, GitLabService, GitLabSyncService,
    OpenClawGatewayEventService, PatchStore, ProductionDeployService, SprintService, StreamService,
    SystemSettingsService, UserService, WebhookAdminService, WebhookManager,
};
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct OpenClawGatewayConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub webhook_url: Option<String>,
    pub webhook_secret: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub event_retention_hours: i64,
}

impl OpenClawGatewayConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("OPENCLAW_GATEWAY_ENABLED")
            .ok()
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false);

        let actor_user_id = std::env::var("OPENCLAW_ACTOR_USER_ID")
            .ok()
            .and_then(|value| match Uuid::parse_str(value.trim()) {
                Ok(id) => Some(id),
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        "Ignoring invalid OPENCLAW_ACTOR_USER_ID; falling back to the default OpenClaw service principal id"
                    );
                    None
                }
            });

        let event_retention_hours = std::env::var("OPENCLAW_EVENT_RETENTION_HOURS")
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(168);

        Self {
            enabled,
            api_key: std::env::var("OPENCLAW_API_KEY")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            webhook_url: std::env::var("OPENCLAW_WEBHOOK_URL")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            webhook_secret: std::env::var("OPENCLAW_WEBHOOK_SECRET")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            actor_user_id,
            event_retention_hours,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    /// Path where agent worktrees are stored (from system_settings or env). Updated live when admin changes in Settings.
    pub worktrees_path: Arc<RwLock<PathBuf>>,
    pub db: PgPool,
    pub metrics: Metrics,
    pub orchestrator: Arc<ExecutorOrchestrator>,
    pub worker_pool: Option<Arc<WorkerPool>>,
    pub deployment_worker_pool: Option<Arc<DeploymentWorkerPool>>,
    pub project_assistant_worker_pool: Option<Arc<ProjectAssistantWorkerPool>>,
    pub broadcast_tx: broadcast::Sender<AgentEvent>,
    pub gitlab_service: Arc<GitLabService>,
    pub gitlab_sync_service: Arc<GitLabSyncService>,
    pub user_service: UserService,
    pub sprint_service: SprintService,
    pub webhook_manager: Arc<WebhookManager>,
    pub gitlab_oauth_service: Arc<GitLabOAuthService>,
    pub webhook_admin_service: Arc<WebhookAdminService>,
    pub settings_service: Arc<SystemSettingsService>,
    pub preview_manager: Arc<PreviewManager>,
    pub storage_service: Arc<acpms_services::StorageService>,
    pub build_service: Arc<BuildService>,
    pub deploy_service: Arc<ProductionDeployService>,
    // Phase 3: JSON Patch streaming
    pub patch_store: Arc<PatchStore>,
    pub stream_service: Arc<StreamService>,
    pub auth_session_store: Arc<AuthSessionStore>,
    pub openclaw_gateway: Arc<OpenClawGatewayConfig>,
    pub openclaw_event_service: Arc<OpenClawGatewayEventService>,
}

impl axum::extract::FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}

impl axum::extract::FromRef<AppState> for Arc<ExecutorOrchestrator> {
    fn from_ref(state: &AppState) -> Self {
        state.orchestrator.clone()
    }
}

impl axum::extract::FromRef<AppState> for Arc<GitLabService> {
    fn from_ref(state: &AppState) -> Self {
        state.gitlab_service.clone()
    }
}

impl axum::extract::FromRef<AppState> for Arc<PreviewManager> {
    fn from_ref(state: &AppState) -> Self {
        state.preview_manager.clone()
    }
}

impl axum::extract::FromRef<AppState> for Arc<acpms_services::StorageService> {
    fn from_ref(state: &AppState) -> Self {
        state.storage_service.clone()
    }
}
