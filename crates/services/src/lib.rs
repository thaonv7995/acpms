pub mod auth;
pub mod dashboard;
pub mod events;
pub mod gitlab;
pub mod normalized_logs;
pub mod project;
pub mod project_document;
pub mod project_assistant_instruction;
pub mod project_assistant_session;
pub mod project_assistant_tools;
pub mod project_summary;
pub mod repository_access;
pub mod requirement;
pub mod sprint;
pub mod subagent;
pub mod task;
pub mod task_attempt; // Restored
pub mod task_context;
pub mod user;
pub mod workspace_repos;

// Re-export ProjectTypeDetector from acpms-db
pub use acpms_db::ProjectTypeDetector;

#[path = "project-template-service.rs"]
pub mod project_template_service;

pub mod prompts;

#[path = "review-service.rs"]
pub mod review_service;

#[path = "encryption-service.rs"]
pub mod encryption_service;

#[path = "encryption-key-rotation.rs"]
pub mod encryption_key_rotation;

#[path = "gitlab-oauth-types.rs"]
pub mod gitlab_oauth_types;

#[path = "webhook-event-handlers.rs"]
pub mod webhook_event_handlers;

#[path = "token-refresh-service.rs"]
pub mod token_refresh_service;

#[path = "token-blacklist-service.rs"]
pub mod token_blacklist_service;

#[path = "webhook-manager.rs"]
pub mod webhook_manager;

#[path = "webhook-manager-admin.rs"]
pub mod webhook_manager_admin;

#[path = "gitlab-oauth-service.rs"]
pub mod gitlab_oauth_service;

#[path = "gitlab-sync-service.rs"]
pub mod gitlab_sync_service;

#[path = "openclaw-gateway-events.rs"]
pub mod openclaw_gateway_events;

#[path = "system-settings-service.rs"]
pub mod system_settings_service;

#[cfg(test)]
#[path = "encryption-service-tests.rs"]
mod encryption_service_tests;

#[cfg(test)]
#[path = "security-tests.rs"]
mod security_tests;

pub use auth::*;
pub use dashboard::DashboardService;
pub use encryption_key_rotation::KeyRotationService;
pub use encryption_service::{generate_encryption_key, EncryptionService};
pub use events::{
    LogMsg, PatchBuilder, PatchOperation, PatchResponse, PatchStore, SequencedPatch, StreamMessage,
    StreamService,
};
pub use gitlab::GitLabService;
pub use gitlab_oauth_service::GitLabOAuthService;
pub use gitlab_oauth_types::*;
pub use gitlab_sync_service::{GitLabSyncService, SyncResult};
pub use normalized_logs::NormalizedLogService;
pub use openclaw_gateway_events::{
    FailedOpenClawWebhookDelivery, NewOpenClawGatewayEvent, OpenClawGatewayEvent,
    OpenClawGatewayEventService, OpenClawGatewayMetricsObserver, OpenClawWebhookDeliveryStats,
};
pub use project::ProjectService;
pub use project_document::{
    ProjectDocumentService, ProjectDocumentServiceError, UpdateProjectDocumentInput,
    UpsertProjectDocumentInput,
};
pub use project_assistant_instruction::{
    apply_preferred_language_to_follow_up_input, build_instruction, build_start_instruction,
    normalize_preferred_agent_language, AssistantMessage, AttachmentContent, TaskSummary,
};
pub use project_assistant_session::ProjectAssistantSessionService;
pub use project_assistant_tools::{parse_tool_call_line, ToolCall};
pub use project_summary::{
    derive_project_execution_status, derive_project_lifecycle_status, derive_project_progress,
    load_project_summaries, summarize_project, ProjectComputedSummary,
};
pub use repository_access::RepositoryAccessService;
pub use requirement::RequirementService;
pub use sprint::SprintService;
pub use system_settings_service::{
    cloudflare_token_looks_masked_or_corrupted, CloudflareConfigOverrides,
    ResolvedCloudflareConfig, SystemSettingsService,
};
pub use task::{TaskService, TaskWithLatestAttempt};
pub use task_attempt::*;
pub use task_context::{
    CreateTaskContextAttachmentInput, CreateTaskContextInput, TaskContextService,
    TaskContextWithAttachments, UpdateTaskContextInput,
};
pub use token_blacklist_service::TokenBlacklistService;
pub use token_refresh_service::RefreshTokenService;
pub use user::{is_hidden_user_email, UserService, OPENCLAW_SERVICE_USER_EMAIL};
pub use webhook_event_handlers::WebhookEventHandlers;
pub use webhook_manager::WebhookManager;
pub use webhook_manager_admin::{FailedWebhookEvent, WebhookAdminService, WebhookStats};
pub use workspace_repos::WorkspaceRepoService;

#[path = "storage_service.rs"]
pub mod storage_service;
pub use storage_service::StorageService;

// ProjectTypeDetector re-exported above from acpms_db
pub use project_template_service::ProjectTemplateService;
pub use review_service::ReviewService;

#[path = "build-service.rs"]
pub mod build_service;

#[path = "production-deploy-service.rs"]
pub mod production_deploy_service;

pub use build_service::{BuildError, BuildService};
pub use production_deploy_service::{DeployError, DeployResult, ProductionDeployService};
pub use subagent::{SubagentRelationship, SubagentService, SubagentTreeNode, SubagentTreeStats};
