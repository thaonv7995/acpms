use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::path::PathBuf;
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

// Enums

/// System-wide role for global permissions
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS, ToSchema)]
#[sqlx(type_name = "system_role")]
#[ts(export)]
pub enum SystemRole {
    #[serde(rename = "admin")]
    #[sqlx(rename = "admin")]
    Admin,

    #[serde(rename = "product_owner")]
    #[sqlx(rename = "product_owner")]
    ProductOwner,

    #[serde(rename = "business_analyst")]
    #[sqlx(rename = "business_analyst")]
    BusinessAnalyst,

    #[serde(rename = "developer")]
    #[sqlx(rename = "developer")]
    Developer,

    #[serde(rename = "quality_assurance")]
    #[sqlx(rename = "quality_assurance")]
    QualityAssurance,

    #[serde(rename = "viewer")]
    #[sqlx(rename = "viewer")]
    Viewer,
}

/// Project-specific role for per-project permissions
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS, ToSchema)]
#[sqlx(type_name = "project_role")]
#[ts(export)]
pub enum ProjectRole {
    #[serde(rename = "owner")]
    #[sqlx(rename = "owner")]
    Owner,

    #[serde(rename = "admin")]
    #[sqlx(rename = "admin")]
    Admin,

    #[serde(rename = "product_owner")]
    #[sqlx(rename = "product_owner")]
    ProductOwner,

    #[serde(rename = "developer")]
    #[sqlx(rename = "developer")]
    Developer,

    #[serde(rename = "business_analyst")]
    #[sqlx(rename = "business_analyst")]
    BusinessAnalyst,

    #[serde(rename = "quality_assurance")]
    #[sqlx(rename = "quality_assurance")]
    QualityAssurance,

    #[serde(rename = "viewer")]
    #[sqlx(rename = "viewer")]
    Viewer,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS, ToSchema)]
#[sqlx(type_name = "task_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TaskType {
    Feature,
    Bug,
    Refactor,
    Docs,
    Test,
    Init,
    Hotfix,
    Chore,
    Spike,
    SmallTask,
    Deploy,
}

impl TaskType {
    /// Check if this is an init task
    pub fn is_init(&self) -> bool {
        matches!(self, TaskType::Init)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS, ToSchema)]
#[sqlx(type_name = "task_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TaskStatus {
    Backlog,
    Todo,
    InProgress,
    InReview, // Agent completed, waiting for human to review diff and approve
    Blocked,
    Done,
    Archived,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS, ToSchema)]
#[sqlx(type_name = "attempt_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum AttemptStatus {
    Queued,
    Running,
    Success,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS, ToSchema)]
#[sqlx(type_name = "requirement_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RequirementStatus {
    Todo,
    InProgress,
    Done,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS, ToSchema)]
#[sqlx(type_name = "requirement_priority", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum RequirementPriority {
    #[serde(alias = "Low")]
    Low,
    #[serde(alias = "Medium")]
    Medium,
    #[serde(alias = "High")]
    High,
    #[serde(alias = "Critical")]
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS, ToSchema)]
#[sqlx(type_name = "sprint_status")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum SprintStatus {
    #[serde(rename = "planned")]
    #[sqlx(rename = "planning")]
    Planned,
    #[sqlx(rename = "active")]
    Active,
    #[serde(rename = "closed")]
    #[sqlx(rename = "completed")]
    Closed,
    #[sqlx(rename = "archived")]
    Archived,
}

/// Project type enum for categorizing projects
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS, ToSchema, Default,
)]
#[sqlx(type_name = "project_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ProjectType {
    /// Web applications (Next.js, Vite, SvelteKit)
    #[default]
    Web,
    /// Mobile applications (React Native, Flutter, Expo)
    Mobile,
    /// Desktop applications (Electron, Tauri)
    Desktop,
    /// Browser extensions (Chrome, Firefox)
    Extension,
    /// REST/GraphQL APIs (FastAPI, Express, NestJS)
    Api,
    /// Containerized microservices (Go, Rust, gRPC)
    Microservice,
}

impl ProjectType {
    /// Get the default preview_enabled setting for this project type
    pub fn default_preview_enabled(&self) -> bool {
        match self {
            ProjectType::Web => true,
            ProjectType::Mobile => false,
            ProjectType::Desktop => false,
            ProjectType::Extension => true,
            ProjectType::Api => true,
            ProjectType::Microservice => true,
        }
    }

    /// Get the default build command for this project type
    pub fn default_build_command(&self) -> &'static str {
        match self {
            ProjectType::Web => "npm run build",
            ProjectType::Mobile => "npx expo build",
            ProjectType::Desktop => "npm run package",
            ProjectType::Extension => "npm run build:ext",
            ProjectType::Api => "cargo build --release",
            ProjectType::Microservice => "docker build -t app .",
        }
    }

    /// Get display name for the project type
    pub fn display_name(&self) -> &'static str {
        match self {
            ProjectType::Web => "Web Application",
            ProjectType::Mobile => "Mobile Application",
            ProjectType::Desktop => "Desktop Application",
            ProjectType::Extension => "Browser Extension",
            ProjectType::Api => "API Service",
            ProjectType::Microservice => "Microservice",
        }
    }
}

/// Repository provider inferred from the imported repository URL.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RepositoryProvider {
    Github,
    Gitlab,
    #[default]
    Unknown,
}

/// Effective repository mode after capability verification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RepositoryAccessMode {
    AnalysisOnly,
    DirectGitops,
    BranchPushOnly,
    ForkGitops,
    #[default]
    Unknown,
}

/// Verification status for repository capability assessment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RepositoryVerificationStatus {
    Verified,
    Unauthenticated,
    Failed,
    #[default]
    Unknown,
}

/// Provider-specific repository access and topology metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, ToSchema, Default)]
#[ts(export)]
pub struct RepositoryContext {
    #[serde(default)]
    pub provider: RepositoryProvider,
    #[serde(default)]
    pub access_mode: RepositoryAccessMode,
    #[serde(default)]
    pub verification_status: RepositoryVerificationStatus,
    #[serde(default)]
    pub verification_error: Option<String>,
    #[serde(default)]
    pub can_clone: bool,
    #[serde(default)]
    pub can_push: bool,
    #[serde(default)]
    pub can_open_change_request: bool,
    #[serde(default)]
    pub can_merge: bool,
    #[serde(default)]
    pub can_manage_webhooks: bool,
    #[serde(default)]
    pub can_fork: bool,
    #[serde(default)]
    pub upstream_repository_url: Option<String>,
    #[serde(default)]
    pub writable_repository_url: Option<String>,
    #[serde(default)]
    pub effective_clone_url: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub upstream_project_id: Option<i64>,
    #[serde(default)]
    pub writable_project_id: Option<i64>,
    #[serde(default)]
    pub verified_at: Option<DateTime<Utc>>,
}

impl RepositoryContext {
    pub fn is_read_only(&self) -> bool {
        matches!(
            self.access_mode,
            RepositoryAccessMode::AnalysisOnly | RepositoryAccessMode::Unknown
        )
    }

    pub fn supports_gitops(&self) -> bool {
        matches!(
            self.access_mode,
            RepositoryAccessMode::DirectGitops | RepositoryAccessMode::ForkGitops
        ) && self.can_push
            && self.can_open_change_request
    }

    pub fn needs_backfill(&self) -> bool {
        self.provider == RepositoryProvider::Unknown
            && self.access_mode == RepositoryAccessMode::Unknown
            && self.verification_status == RepositoryVerificationStatus::Unknown
            && self.verified_at.is_none()
    }
}

// ===== Project Settings =====

/// Default value functions for serde
fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_max_retries() -> i32 {
    3
}

fn default_timeout_mins() -> i32 {
    30
}

fn default_preview_ttl_days() -> i32 {
    7
}

fn default_deploy_branch() -> String {
    "main".to_string()
}

fn default_empty_string_vec() -> Vec<String> {
    Vec::new()
}

fn default_auto_execute_priority() -> String {
    "normal".to_string()
}

fn default_retry_backoff() -> String {
    "exponential".to_string()
}

fn default_max_concurrent() -> i32 {
    3
}

/// Project-level settings controlling agent execution, deployment, review, and notifications.
/// Stored as JSONB in the database for flexibility with schema validation at application layer.
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ProjectSettings {
    /// If true, agent changes require human review before commit/push
    #[serde(default = "default_true")]
    pub require_review: bool,

    /// If true, deploy preview (Cloudflare tunnel) when task completes. Not related to production.
    #[serde(default = "default_false")]
    pub auto_deploy: bool,

    /// If true, create preview environments for task attempts (legacy alias for auto_deploy)
    #[serde(default = "default_true")]
    pub preview_enabled: bool,

    /// If true, deploy to production when MR is merged into deploy_branch
    #[serde(default = "default_false")]
    pub production_deploy_on_merge: bool,

    /// If true, use GitOps workflow (create MRs) vs direct push
    #[serde(default = "default_true")]
    pub gitops_enabled: bool,

    /// Maximum number of retry attempts for failed tasks
    #[serde(default = "default_max_retries")]
    pub max_retries: i32,

    /// Agent execution timeout in minutes
    #[serde(default = "default_timeout_mins")]
    pub timeout_mins: i32,

    /// Preview environment lifetime in days
    #[serde(default = "default_preview_ttl_days")]
    pub preview_ttl_days: i32,

    /// If true, automatically merge approved MRs
    #[serde(default = "default_false")]
    pub auto_merge: bool,

    /// Branch that triggers production deployment
    #[serde(default = "default_deploy_branch")]
    pub deploy_branch: String,

    /// Target branch for merge requests (default: deploy_branch or "main")
    #[serde(default)]
    pub mr_target_branch: Option<String>,

    /// If true, send notification on task success
    #[serde(default = "default_false")]
    pub notify_on_success: bool,

    /// If true, send notification on task failure
    #[serde(default = "default_true")]
    pub notify_on_failure: bool,

    /// If true, send notification when review is needed
    #[serde(default = "default_true")]
    pub notify_on_review: bool,

    /// Notification channels (Slack, email, etc.)
    #[serde(default = "default_empty_string_vec")]
    pub notify_channels: Vec<String>,

    /// If true, automatically start task execution on creation
    #[serde(default = "default_false")]
    pub auto_execute: bool,

    /// Task types to auto-execute: ["bug", "hotfix", "feature", "refactor", "docs", "test", "chore", "spike", "small_task"]
    #[serde(default = "default_empty_string_vec")]
    pub auto_execute_types: Vec<String>,

    /// If true, automatically retry failed tasks
    #[serde(default = "default_false")]
    pub auto_retry: bool,

    /// Queue priority for auto-execute: "low", "normal", "high"
    #[serde(default = "default_auto_execute_priority")]
    pub auto_execute_priority: String,

    /// Retry backoff strategy: "fixed", "exponential"
    #[serde(default = "default_retry_backoff")]
    pub retry_backoff: String,

    /// Maximum concurrent tasks per project
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: i32,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            require_review: true,
            auto_deploy: false,
            preview_enabled: true,
            production_deploy_on_merge: false,
            gitops_enabled: true,
            max_retries: 3,
            timeout_mins: 30,
            preview_ttl_days: 7,
            auto_merge: false,
            deploy_branch: "main".to_string(),
            mr_target_branch: None,
            notify_on_success: false,
            notify_on_failure: true,
            notify_on_review: true,
            notify_channels: Vec::new(),
            auto_execute: false,
            auto_execute_types: Vec::new(),
            auto_retry: false,
            auto_execute_priority: "normal".to_string(),
            retry_backoff: "exponential".to_string(),
            max_concurrent: 3,
        }
    }
}

impl ProjectSettings {
    /// Create a new ProjectSettings with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge partial settings update into existing settings
    pub fn merge(&mut self, update: &serde_json::Value) {
        if let Some(v) = update.get("require_review").and_then(|v| v.as_bool()) {
            self.require_review = v;
        }
        if let Some(v) = update.get("auto_deploy").and_then(|v| v.as_bool()) {
            self.auto_deploy = v;
        }
        if let Some(v) = update.get("preview_enabled").and_then(|v| v.as_bool()) {
            self.preview_enabled = v;
        }
        if let Some(v) = update
            .get("production_deploy_on_merge")
            .and_then(|v| v.as_bool())
        {
            self.production_deploy_on_merge = v;
        }
        if let Some(v) = update.get("gitops_enabled").and_then(|v| v.as_bool()) {
            self.gitops_enabled = v;
        }
        if let Some(v) = update.get("max_retries").and_then(|v| v.as_i64()) {
            self.max_retries = v as i32;
        }
        if let Some(v) = update.get("timeout_mins").and_then(|v| v.as_i64()) {
            self.timeout_mins = v as i32;
        }
        if let Some(v) = update.get("preview_ttl_days").and_then(|v| v.as_i64()) {
            self.preview_ttl_days = v as i32;
        }
        if let Some(v) = update.get("auto_merge").and_then(|v| v.as_bool()) {
            self.auto_merge = v;
        }
        if let Some(v) = update.get("deploy_branch").and_then(|v| v.as_str()) {
            self.deploy_branch = v.to_string();
        }
        if let Some(v) = update.get("mr_target_branch").and_then(|v| v.as_str()) {
            self.mr_target_branch = Some(v.to_string());
        }
        if let Some(v) = update.get("notify_on_success").and_then(|v| v.as_bool()) {
            self.notify_on_success = v;
        }
        if let Some(v) = update.get("notify_on_failure").and_then(|v| v.as_bool()) {
            self.notify_on_failure = v;
        }
        if let Some(v) = update.get("notify_on_review").and_then(|v| v.as_bool()) {
            self.notify_on_review = v;
        }
        if let Some(v) = update.get("notify_channels").and_then(|v| v.as_array()) {
            self.notify_channels = v
                .iter()
                .filter_map(|s| s.as_str().map(|s| s.to_string()))
                .collect();
        }
        if let Some(v) = update.get("auto_execute").and_then(|v| v.as_bool()) {
            self.auto_execute = v;
        }
        if let Some(v) = update.get("auto_execute_types").and_then(|v| v.as_array()) {
            self.auto_execute_types = v
                .iter()
                .filter_map(|s| s.as_str().map(|s| s.to_string()))
                .collect();
        }
        if let Some(v) = update.get("auto_retry").and_then(|v| v.as_bool()) {
            self.auto_retry = v;
        }
        if let Some(v) = update.get("auto_execute_priority").and_then(|v| v.as_str()) {
            self.auto_execute_priority = v.to_string();
        }
        if let Some(v) = update.get("retry_backoff").and_then(|v| v.as_str()) {
            self.retry_backoff = v.to_string();
        }
        if let Some(v) = update.get("max_concurrent").and_then(|v| v.as_i64()) {
            self.max_concurrent = v as i32;
        }
    }
}

/// Request to update project settings (partial update supported)
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct UpdateProjectSettingsRequest {
    pub require_review: Option<bool>,
    pub auto_deploy: Option<bool>,
    pub preview_enabled: Option<bool>,
    pub gitops_enabled: Option<bool>,
    pub max_retries: Option<i32>,
    pub timeout_mins: Option<i32>,
    pub preview_ttl_days: Option<i32>,
    pub auto_merge: Option<bool>,
    pub deploy_branch: Option<String>,
    pub mr_target_branch: Option<String>,
    pub notify_on_success: Option<bool>,
    pub notify_on_failure: Option<bool>,
    pub notify_on_review: Option<bool>,
    pub notify_channels: Option<Vec<String>>,
    pub auto_execute: Option<bool>,
    pub auto_execute_types: Option<Vec<String>>,
    pub auto_retry: Option<bool>,
    pub auto_execute_priority: Option<String>,
    pub retry_backoff: Option<String>,
    pub max_concurrent: Option<i32>,
}

/// Response for project settings with defaults included for UI reset functionality
#[derive(Debug, Clone, Serialize, TS, ToSchema)]
#[ts(export)]
pub struct ProjectSettingsResponse {
    pub settings: ProjectSettings,
    pub defaults: ProjectSettings,
}

// User model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub gitlab_id: Option<i32>,
    pub gitlab_username: Option<String>,
    #[serde(skip)]
    #[ts(skip)]
    pub password_hash: Option<String>,
    pub global_roles: Vec<SystemRole>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct CreateUserRequest {
    pub email: String,
    pub name: String,
    pub password: String,
}

// Project model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub repository_url: Option<String>,
    /// Provider-specific repository access and topology metadata
    #[serde(default)]
    #[sqlx(json)]
    pub repository_context: RepositoryContext,
    pub metadata: serde_json::Value,
    /// Legacy field for backward compatibility - use settings.require_review instead
    pub require_review: bool,
    pub architecture_config: serde_json::Value,
    /// Project-level settings (JSONB) - new unified settings object
    #[sqlx(json)]
    pub settings: ProjectSettings,
    /// Agent settings (router, filters, etc.) - Phase 2
    pub agent_settings: serde_json::Value,
    /// Project type classification (web, mobile, desktop, extension, api, microservice)
    pub project_type: ProjectType,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Slugify project name for workspace/repository path usage.
pub fn slugify_project_name(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "project".to_string();
    }

    let mut out = String::with_capacity(trimmed.len());
    let mut prev_dash = false;

    for ch in trimmed.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }

    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "project".to_string()
    } else if out.len() > 64 {
        out.chars().take(64).collect()
    } else {
        out
    }
}

/// Resolve effective project slug from metadata fallback to project name.
pub fn resolve_project_slug(metadata: &serde_json::Value, project_name: &str) -> String {
    metadata
        .get("slug")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(slugify_project_name)
        .unwrap_or_else(|| slugify_project_name(project_name))
}

/// Resolve repository directory relative to worktrees base.
///
/// Layout:
/// - v3 (default): `<slug>` or `<slug>-2`, `<slug>-3` on collision (stored in metadata.repo_relative_path)
/// - v2 (legacy): `<project_id>/<slug>` — kept for backward compat
/// - v1 (legacy): `<slug>`
pub fn project_repo_relative_path(
    project_id: Uuid,
    metadata: &serde_json::Value,
    project_name: &str,
) -> PathBuf {
    if let Some(rel) = metadata.get("repo_relative_path").and_then(|v| v.as_str()) {
        if !rel.is_empty() {
            return PathBuf::from(rel);
        }
    }

    let slug = resolve_project_slug(metadata, project_name);
    let repo_path_version = metadata
        .get("repo_path_version")
        .and_then(|v| v.as_i64())
        .unwrap_or(3);

    if repo_path_version == 2 {
        PathBuf::from(project_id.to_string()).join(slug)
    } else {
        PathBuf::from(slug)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStackLayer {
    Frontend,
    Backend,
    Database,
    Auth,
    Cache,
    Queue,
}

impl ProjectStackLayer {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Frontend => "Frontend",
            Self::Backend => "Backend",
            Self::Database => "Database",
            Self::Auth => "Authentication",
            Self::Cache => "Cache",
            Self::Queue => "Queue / Messaging",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectStackSelection {
    pub layer: ProjectStackLayer,
    pub stack: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub description: Option<String>,
    pub repository_url: Option<String>,
    pub repository_context: Option<RepositoryContext>,
    pub metadata: Option<serde_json::Value>,
    /// If true, agent changes require human review before commit/push
    #[serde(default = "default_require_review_option")]
    pub require_review: Option<bool>,

    // From-scratch initialization fields
    pub create_from_scratch: Option<bool>,
    pub visibility: Option<String>, // "private", "public", "internal"
    /// Preferred tech stack/framework for initialization (e.g., "tauri", "nextjs").
    pub tech_stack: Option<String>,
    /// Optional layered stack selections (e.g., frontend/backend/database) for richer init prompts.
    pub stack_selections: Option<Vec<ProjectStackSelection>>,
    /// If true (default), create and run the init task automatically.
    #[serde(default = "default_auto_create_init_task_option")]
    pub auto_create_init_task: Option<bool>,

    /// Project type classification (defaults to Web if not specified)
    #[serde(default)]
    pub project_type: Option<ProjectType>,

    /// Template ID to create project from (optional)
    pub template_id: Option<Uuid>,

    /// If true, enable preview deployments for this project.
    pub preview_enabled: Option<bool>,

    /// Storage keys of reference files uploaded via init-refs/upload-url (for from-scratch).
    pub reference_keys: Option<Vec<String>>,
}

fn default_require_review_option() -> Option<bool> {
    Some(true)
}

fn default_auto_create_init_task_option() -> Option<bool> {
    Some(true)
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub repository_url: Option<String>,
    pub repository_context: Option<RepositoryContext>,
    pub metadata: Option<serde_json::Value>,
    pub require_review: Option<bool>,
}

// Project member model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct ProjectMember {
    pub id: Uuid,
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub roles: Vec<ProjectRole>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Requirement model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct Requirement {
    pub id: Uuid,
    pub project_id: Uuid,
    pub sprint_id: Option<Uuid>,
    pub title: String,
    pub content: String,
    pub status: RequirementStatus,
    pub priority: RequirementPriority,
    pub due_date: Option<NaiveDate>,
    pub metadata: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct CreateRequirementRequest {
    pub project_id: Uuid,
    pub sprint_id: Option<Uuid>,
    pub title: String,
    pub content: String,
    pub priority: Option<RequirementPriority>,
    pub due_date: Option<NaiveDate>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct UpdateRequirementRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub sprint_id: Option<Uuid>,
    pub status: Option<RequirementStatus>,
    pub priority: Option<RequirementPriority>,
    pub due_date: Option<NaiveDate>,
    pub metadata: Option<serde_json::Value>,
}

// Task model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid,
    pub requirement_id: Option<Uuid>,
    pub sprint_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub assigned_to: Option<Uuid>,
    pub parent_task_id: Option<Uuid>,
    pub gitlab_issue_id: Option<i32>,
    pub metadata: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct CreateTaskRequest {
    pub project_id: Uuid,
    pub requirement_id: Option<Uuid>,
    pub sprint_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub task_type: TaskType,
    pub assigned_to: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
    pub parent_task_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub task_type: Option<TaskType>,
    pub status: Option<TaskStatus>,
    pub assigned_to: Option<Uuid>,
    pub sprint_id: Option<Uuid>,
}

// Task context model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct TaskContext {
    pub id: Uuid,
    pub task_id: Uuid,
    pub title: Option<String>,
    pub content_type: String,
    pub raw_content: String,
    pub source: String,
    pub sort_order: i32,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct TaskContextAttachment {
    pub id: Uuid,
    pub task_context_id: Uuid,
    pub storage_key: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct ProjectDocument {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub filename: String,
    pub document_kind: String,
    pub content_type: String,
    pub storage_key: String,
    pub checksum: Option<String>,
    pub size_bytes: i64,
    pub source: String,
    pub version: i32,
    pub ingestion_status: String,
    pub index_error: Option<String>,
    pub indexed_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct ProjectDocumentChunk {
    pub id: Uuid,
    pub document_id: Uuid,
    pub project_id: Uuid,
    pub chunk_index: i32,
    pub content: String,
    pub content_hash: String,
    pub token_count: Option<i32>,
    pub embedding: Vec<f32>,
    pub created_at: DateTime<Utc>,
}

// Task with computed attempt status fields for kanban view
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct TaskWithAttemptStatus {
    // Base task fields
    pub id: Uuid,
    pub project_id: Uuid,
    pub requirement_id: Option<Uuid>,
    pub sprint_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub assigned_to: Option<Uuid>,
    pub parent_task_id: Option<Uuid>,
    pub gitlab_issue_id: Option<i32>,
    pub metadata: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // Computed attempt status fields
    pub has_in_progress_attempt: bool,
    pub last_attempt_failed: bool,
    pub executor: Option<String>,
}

// Task attempt model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct TaskAttempt {
    pub id: Uuid,
    pub task_id: Uuid,
    pub status: AttemptStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    // Diff summary fields (populated when diff is saved to DB)
    pub diff_total_files: Option<i32>,
    pub diff_total_additions: Option<i32>,
    pub diff_total_deletions: Option<i32>,
    pub diff_saved_at: Option<DateTime<Utc>>,
    // S3-based diff storage (MinIO)
    pub s3_diff_key: Option<String>,
    pub s3_diff_size: Option<i64>,
    pub s3_diff_saved_at: Option<DateTime<Utc>>,
    // S3-based log storage (JSONL, R7)
    pub s3_log_key: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct CreateTaskAttemptRequest {
    pub task_id: Uuid,
}

/// Stored file diff for an attempt
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct FileDiff {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub file_path: String,
    pub old_path: Option<String>,
    pub change_type: String,
    pub additions: i32,
    pub deletions: i32,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Normalized log entry extracted from agent output
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct NormalizedLogEntry {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub raw_log_id: Option<Uuid>,
    pub entry_type: String,
    pub entry_data: serde_json::Value,
    pub line_number: i32,
    pub created_at: DateTime<Utc>,
}

/// Workspace repository for multi-repo support (Phase 4)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct WorkspaceRepo {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub project_id: Uuid,
    pub repo_name: String,
    pub repo_url: String,
    pub worktree_path: String,
    pub relative_path: String,
    pub target_branch: String,
    pub base_branch: String,
    pub is_primary: bool,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkspaceRepo {
    pub repo_name: String,
    pub repo_url: String,
    pub worktree_path: String,
    pub relative_path: String,
    pub target_branch: String,
    pub base_branch: String,
    pub is_primary: bool,
}

// Execution process model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct ExecutionProcess {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub process_id: Option<i32>,
    pub worktree_path: Option<String>,
    pub branch_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

// Agent log model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct AgentLog {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub log_type: String, // stdout, stderr, system
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct SendInputRequest {
    pub input: String,
}

// Project Assistant session model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct ProjectAssistantSession {
    pub id: Uuid,
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub status: String,
    pub s3_log_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

// GitLab Integration models
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct GitLabConfiguration {
    pub id: Uuid,
    pub project_id: Uuid,
    pub gitlab_project_id: i64,
    pub base_url: String,
    pub pat_encrypted: String,
    pub webhook_secret: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct MergeRequestDb {
    pub id: Uuid,
    pub task_id: Uuid,
    pub attempt_id: Option<Uuid>,
    /// GitLab MR IID. NULL for GitHub PRs.
    pub gitlab_mr_iid: Option<i64>,
    pub web_url: String,
    pub status: String,
    pub source_repository_url: Option<String>,
    pub target_repository_url: Option<String>,
    pub source_branch: Option<String>,
    pub target_branch: Option<String>,
    pub source_project_id: Option<i64>,
    pub target_project_id: Option<i64>,
    pub source_namespace: Option<String>,
    pub target_namespace: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Provider: "gitlab" | "github". Default "gitlab".
    #[serde(default = "default_merge_request_provider")]
    pub provider: String,
    /// GitHub PR number. NULL for GitLab MRs.
    pub github_pr_number: Option<i64>,
}

fn default_merge_request_provider() -> String {
    "gitlab".to_string()
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct LinkGitLabProjectRequest {
    /// GitLab project ID (numeric). Optional if repository_url is provided.
    pub gitlab_project_id: Option<i64>,
    /// Repository URL (e.g. https://gitlab.com/group/repo). Resolved to project_id via GitLab API.
    pub repository_url: Option<String>,
    // Note: base_url and PAT come from system_settings (global config)
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct GitLabWebhook {
    pub id: Uuid,
    pub project_id: Uuid,
    pub gitlab_id: i64,
    pub url: String,
    pub events: Vec<String>,
    pub secret_token: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// System Settings model (singleton - only one row)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SystemSettings {
    pub id: Uuid,
    // Source Control (GitLab hoặc GitHub - system chỉ setup 1)
    pub gitlab_url: String,
    #[serde(skip_serializing)] // Never expose encrypted PAT in API responses
    pub gitlab_pat_encrypted: Option<String>,
    pub gitlab_auto_sync: bool,
    // Agent CLI Provider (Claude / Codex / Gemini)
    pub agent_cli_provider: String,
    // Cloudflare Configuration
    pub cloudflare_account_id: Option<String>,
    #[serde(skip_serializing)] // Never expose encrypted token in API responses
    pub cloudflare_api_token_encrypted: Option<String>,
    pub cloudflare_zone_id: Option<String>,
    pub cloudflare_base_domain: Option<String>,
    // Notifications
    pub notifications_email_enabled: bool,
    pub notifications_slack_enabled: bool,
    pub notifications_slack_webhook_url: Option<String>,
    /// Path where agent worktrees are stored. Overrides WORKTREES_PATH env when set.
    pub worktrees_path: Option<String>,
    /// Preferred language for agent conversation (en, vi). Injected into instructions.
    pub preferred_agent_language: Option<String>,
    // Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct UpdateSystemSettingsRequest {
    pub gitlab_url: Option<String>,
    pub gitlab_pat: Option<String>, // Plain text - will be encrypted before storage (dùng cho GitLab hoặc GitHub)
    pub gitlab_auto_sync: Option<bool>,
    pub agent_cli_provider: Option<String>,
    pub cloudflare_account_id: Option<String>,
    pub cloudflare_api_token: Option<String>, // Plain text - will be encrypted before storage
    pub cloudflare_zone_id: Option<String>,
    pub cloudflare_base_domain: Option<String>,
    pub notifications_email_enabled: Option<bool>,
    pub notifications_slack_enabled: Option<bool>,
    pub notifications_slack_webhook_url: Option<String>,
    pub worktrees_path: Option<String>,
    pub preferred_agent_language: Option<String>,
}

// Response DTO for settings (safe to expose)
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SystemSettingsResponse {
    pub gitlab_url: String,
    pub gitlab_pat_configured: bool, // Indicator that PAT is set (not the actual value)
    pub gitlab_auto_sync: bool,
    pub agent_cli_provider: String,
    pub cloudflare_account_id: Option<String>,
    pub cloudflare_api_token_configured: bool,
    pub cloudflare_zone_id: Option<String>,
    pub cloudflare_base_domain: Option<String>,
    pub notifications_email_enabled: bool,
    pub notifications_slack_enabled: bool,
    pub notifications_slack_webhook_url: Option<String>,
    /// Path where agent worktrees (cloned source code) are stored. From env WORKTREES_PATH.
    pub worktrees_path: String,
    /// Preferred language for agent conversation: en or vi.
    pub preferred_agent_language: String,
    pub updated_at: DateTime<Utc>,
}

impl From<SystemSettings> for SystemSettingsResponse {
    fn from(s: SystemSettings) -> Self {
        let default_worktrees = std::env::var("HOME")
            .ok()
            .map(|h| format!("{}/Projects", h.trim_end_matches('/')))
            .unwrap_or_else(|| "./worktrees".to_string());
        let worktrees_path = s
            .worktrees_path
            .filter(|p| !p.is_empty())
            .or_else(|| std::env::var("WORKTREES_PATH").ok())
            .unwrap_or(default_worktrees);
        Self {
            gitlab_url: s.gitlab_url,
            gitlab_pat_configured: s.gitlab_pat_encrypted.is_some(),
            gitlab_auto_sync: s.gitlab_auto_sync,
            agent_cli_provider: s.agent_cli_provider,
            cloudflare_account_id: s.cloudflare_account_id,
            cloudflare_api_token_configured: s.cloudflare_api_token_encrypted.is_some(),
            cloudflare_zone_id: s.cloudflare_zone_id,
            cloudflare_base_domain: s.cloudflare_base_domain,
            notifications_email_enabled: s.notifications_email_enabled,
            notifications_slack_enabled: s.notifications_slack_enabled,
            notifications_slack_webhook_url: s.notifications_slack_webhook_url,
            worktrees_path,
            preferred_agent_language: s
                .preferred_agent_language
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "en".to_string()),
            updated_at: s.updated_at,
        }
    }
}

// Task Metadata Types

/// Source of project initialization
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum InitSource {
    GitlabImport,
    FromScratch,
}

/// Metadata for init tasks
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InitTaskMetadata {
    pub source: InitSource,
    pub repository_url: Option<String>, // For gitlab_import
    /// User-selected project type for gitlab_import. When Some, user choice is respected; when None, auto-detect from repo.
    #[serde(default)]
    pub project_type: Option<ProjectType>,
    pub visibility: Option<String>, // For from_scratch
    /// Storage keys of reference files for agent to read (from_scratch only).
    #[serde(default)]
    pub reference_keys: Option<Vec<String>>,
    /// Preferred tech stack for from_scratch (e.g., "tauri", "nextjs").
    #[serde(default)]
    pub preferred_stack: Option<String>,
    /// Layered stack selections for from_scratch (frontend/backend/database, etc.).
    #[serde(default)]
    pub stack_selections: Option<Vec<ProjectStackSelection>>,
}

impl InitTaskMetadata {
    /// Create metadata for GitLab import flow
    pub fn gitlab_import(
        repository_url: String,
        project_type: Option<ProjectType>,
    ) -> serde_json::Value {
        let mut init = serde_json::json!({
            "source": "gitlab_import",
            "repository_url": repository_url
        });
        if let Some(pt) = project_type {
            init["project_type"] = serde_json::to_value(pt).unwrap_or_default();
        }
        serde_json::json!({ "init": init })
    }

    /// Create metadata for from-scratch flow
    pub fn from_scratch(
        visibility: String,
        reference_keys: Option<Vec<String>>,
        preferred_stack: Option<String>,
        stack_selections: Option<Vec<ProjectStackSelection>>,
    ) -> serde_json::Value {
        let mut init = serde_json::json!({
            "source": "from_scratch",
            "visibility": visibility
        });
        if let Some(ref keys) = reference_keys {
            if !keys.is_empty() {
                init["reference_keys"] = serde_json::to_value(keys).unwrap_or_default();
            }
        }
        if let Some(ref stack) = preferred_stack {
            if !stack.trim().is_empty() {
                init["preferred_stack"] = serde_json::json!(stack.trim());
            }
        }
        if let Some(ref selections) = stack_selections {
            if !selections.is_empty() {
                init["stack_selections"] =
                    serde_json::to_value(selections).unwrap_or(serde_json::Value::Array(vec![]));
            }
        }
        serde_json::json!({ "init": init })
    }

    /// Parse metadata from task record
    pub fn parse(metadata: &serde_json::Value) -> anyhow::Result<Self> {
        let init_meta = metadata
            .get("init")
            .ok_or_else(|| anyhow::anyhow!("Missing 'init' metadata"))?;

        serde_json::from_value(init_meta.clone())
            .map_err(|e| anyhow::anyhow!("Failed to parse init metadata: {}", e))
    }
}

// Sprint model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct Sprint {
    pub id: Uuid,
    pub project_id: Uuid,
    pub sequence: i32,
    pub name: String,
    pub description: Option<String>,
    pub goal: Option<String>,
    pub status: SprintStatus,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub closed_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct CreateSprintRequest {
    pub project_id: Uuid,
    pub sequence: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub goal: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct GenerateSprintsRequest {
    pub project_id: Uuid,
    pub start_date: DateTime<Utc>,
    pub duration_weeks: i32,
    pub count: i32,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct UpdateSprintRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub goal: Option<String>,
    pub status: Option<SprintStatus>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS, ToSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SprintCarryOverMode {
    MoveToNext,
    MoveToBacklog,
    KeepInClosed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct CreateNextSprintRequest {
    pub name: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub goal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct CloseSprintRequest {
    pub carry_over_mode: SprintCarryOverMode,
    pub next_sprint_id: Option<Uuid>,
    pub create_next_sprint: Option<CreateNextSprintRequest>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CloseSprintResult {
    pub closed_sprint_id: Uuid,
    pub moved_task_count: i64,
    pub moved_to_sprint_id: Option<Uuid>,
    pub carry_over_mode: SprintCarryOverMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SprintOverview {
    pub sprint_id: Uuid,
    pub project_id: Uuid,
    pub total_tasks: i64,
    pub done_tasks: i64,
    pub canceled_tasks: i64,
    pub remaining_tasks: i64,
    pub completion_rate: i32,
    pub moved_in_count: i64,
    pub moved_out_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SprintTaskMovement {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub from_sprint_id: Option<Uuid>,
    pub to_sprint_id: Option<Uuid>,
    pub moved_by: Option<Uuid>,
    pub moved_at: DateTime<Utc>,
    pub reason: Option<String>,
}

// Cloudflare Tunnel models
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[ts(export)]
pub enum TunnelStatus {
    Creating,
    Active,
    Failed,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct CloudflareTunnel {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub tunnel_id: String,
    pub tunnel_name: String,
    #[serde(skip)]
    #[ts(skip)]
    pub credentials_encrypted: String,
    pub preview_url: String,
    pub status: TunnelStatus,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub dns_record_id: Option<String>,
    pub docker_project_name: Option<String>,
    pub compose_file_path: Option<String>,
    pub worktree_path: Option<String>,
    pub last_error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PreviewInfo {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub preview_url: String,
    pub status: TunnelStatus,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

// ===== Project Templates =====

/// Project template for quick scaffolding with predefined tech stacks and settings
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct ProjectTemplate {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub project_type: ProjectType,
    pub repository_url: String,
    /// Array of technologies used in this template, e.g., ["React", "TypeScript", "Vite"]
    pub tech_stack: serde_json::Value,
    /// Default project settings to apply when creating from this template
    pub default_settings: serde_json::Value,
    /// Whether this is an officially maintained template
    pub is_official: bool,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct CreateProjectTemplateRequest {
    pub name: String,
    pub description: Option<String>,
    pub project_type: ProjectType,
    pub repository_url: String,
    pub tech_stack: Option<Vec<String>>,
    pub default_settings: Option<serde_json::Value>,
    pub is_official: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct UpdateProjectTemplateRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub project_type: Option<ProjectType>,
    pub repository_url: Option<String>,
    pub tech_stack: Option<Vec<String>>,
    pub default_settings: Option<serde_json::Value>,
    pub is_official: Option<bool>,
}

/// Request to create a project from a template
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct CreateProjectFromTemplateRequest {
    pub template_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    /// Override visibility (defaults to private)
    pub visibility: Option<String>,
}

/// Query parameters for listing templates
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct ListTemplatesQuery {
    /// Filter by project type
    pub project_type: Option<ProjectType>,
    /// Filter by official templates only
    pub official_only: Option<bool>,
}

// ===== Review Comments =====

/// Review comment on a task attempt
/// Supports line-level, file-level, and general comments for code review workflow
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct ReviewComment {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub user_id: Uuid,
    /// Relative path to file in repository. NULL for general comments
    pub file_path: Option<String>,
    /// Line number in file. NULL for file-level or general comments
    pub line_number: Option<i32>,
    /// Comment text content
    pub content: String,
    /// Whether this comment has been addressed/resolved
    pub resolved: bool,
    /// User who marked the comment as resolved
    pub resolved_by: Option<Uuid>,
    /// Timestamp when comment was resolved
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to add a review comment
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct AddReviewCommentRequest {
    /// Relative path to file (optional - NULL for general comment)
    pub file_path: Option<String>,
    /// Line number in file (optional - NULL for file-level comment)
    pub line_number: Option<i32>,
    /// Comment text content
    pub content: String,
}

/// Request to request changes on an attempt
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct RequestChangesRequest {
    /// Feedback/summary for the agent explaining what needs to change
    pub feedback: String,
    /// Whether to include existing review comments as context (default: true)
    #[serde(default = "default_true")]
    pub include_comments: bool,
}

/// Response containing the new attempt created from request-changes
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct RequestChangesResponse {
    /// The original attempt that was reviewed
    pub original_attempt_id: Uuid,
    /// The new attempt created with feedback
    pub new_attempt_id: Uuid,
    /// Feedback that was included
    pub feedback: String,
    /// Number of review comments included
    pub comments_included: i32,
}

// ===== Build Artifacts & Deployments =====

/// Build artifact stored in MinIO after a successful build
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct BuildArtifact {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub project_id: Uuid,
    /// Storage key in MinIO: builds/{project_id}/{attempt_id}/{artifact_name}
    pub artifact_key: String,
    /// Type of artifact: dist, binary, apk, ipa, image, etc.
    pub artifact_type: String,
    pub size_bytes: Option<i64>,
    pub file_count: Option<i32>,
    pub build_command: Option<String>,
    pub build_duration_secs: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// Preview deployment using Cloudflare tunnels
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct PreviewDeployment {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub project_id: Uuid,
    pub artifact_id: Option<Uuid>,
    pub url: String,
    pub tunnel_id: Option<String>,
    pub dns_record_id: Option<String>,
    /// Deployment status: active, expired, destroyed
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub destroyed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Production deployment to Cloudflare Pages, Workers, or external services
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct ProductionDeployment {
    pub id: Uuid,
    pub project_id: Uuid,
    pub artifact_id: Option<Uuid>,
    /// Target platform: pages, workers, container, manual
    pub deployment_type: String,
    pub url: String,
    pub deployment_id: Option<String>,
    /// Deployment status: active, failed, superseded
    pub status: String,
    pub triggered_by: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS)]
#[sqlx(type_name = "deployment_target_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeploymentTargetType {
    Local,
    SshRemote,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS)]
#[sqlx(type_name = "deployment_runtime_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeploymentRuntimeType {
    Compose,
    Systemd,
    RawScript,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS)]
#[sqlx(type_name = "deployment_artifact_strategy", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeploymentArtifactStrategy {
    GitPull,
    UploadBundle,
    BuildArtifact,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS)]
#[sqlx(type_name = "deployment_secret_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeploymentSecretType {
    SshPrivateKey,
    SshPassword,
    ApiToken,
    KnownHosts,
    EnvFile,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS)]
#[sqlx(type_name = "deployment_run_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeploymentRunStatus {
    Queued,
    Running,
    Success,
    Failed,
    Cancelled,
    RollingBack,
    RolledBack,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS)]
#[sqlx(type_name = "deployment_trigger_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeploymentTriggerType {
    Manual,
    Auto,
    Rollback,
    Retry,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS)]
#[sqlx(type_name = "deployment_source_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeploymentSourceType {
    Branch,
    Commit,
    Artifact,
    Release,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS)]
#[sqlx(type_name = "deployment_release_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeploymentReleaseStatus {
    Active,
    Superseded,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS)]
#[sqlx(type_name = "deployment_timeline_step", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeploymentTimelineStep {
    Precheck,
    Connect,
    Prepare,
    Deploy,
    DomainConfig,
    Healthcheck,
    Finalize,
    Rollback,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, TS)]
#[sqlx(
    type_name = "deployment_timeline_event_type",
    rename_all = "snake_case"
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeploymentTimelineEventType {
    System,
    Agent,
    Command,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct DeploymentEnvironment {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub target_type: DeploymentTargetType,
    pub is_enabled: bool,
    pub is_default: bool,
    pub runtime_type: DeploymentRuntimeType,
    pub deploy_path: String,
    pub artifact_strategy: DeploymentArtifactStrategy,
    pub branch_policy: serde_json::Value,
    pub healthcheck_url: Option<String>,
    pub healthcheck_timeout_secs: i32,
    pub healthcheck_expected_status: i32,
    pub target_config: serde_json::Value,
    pub domain_config: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct DeploymentEnvironmentSecret {
    pub id: Uuid,
    pub environment_id: Uuid,
    pub secret_type: DeploymentSecretType,
    pub ciphertext: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct DeploymentRun {
    pub id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub status: DeploymentRunStatus,
    pub trigger_type: DeploymentTriggerType,
    pub triggered_by: Option<Uuid>,
    pub source_type: DeploymentSourceType,
    pub source_ref: Option<String>,
    pub attempt_id: Option<Uuid>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct DeploymentRelease {
    pub id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub run_id: Uuid,
    pub version_label: String,
    pub artifact_ref: Option<String>,
    pub git_commit_sha: Option<String>,
    pub status: DeploymentReleaseStatus,
    pub deployed_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct DeploymentTimelineEvent {
    pub id: Uuid,
    pub run_id: Uuid,
    pub step: DeploymentTimelineStep,
    pub event_type: DeploymentTimelineEventType,
    pub message: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct DeploymentEnvironmentSecretInput {
    pub secret_type: DeploymentSecretType,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct CreateDeploymentEnvironmentRequest {
    pub name: String,
    pub description: Option<String>,
    pub target_type: DeploymentTargetType,
    pub is_enabled: Option<bool>,
    pub is_default: Option<bool>,
    pub runtime_type: Option<DeploymentRuntimeType>,
    pub deploy_path: String,
    pub artifact_strategy: Option<DeploymentArtifactStrategy>,
    pub branch_policy: Option<serde_json::Value>,
    pub healthcheck_url: Option<String>,
    pub healthcheck_timeout_secs: Option<i32>,
    pub healthcheck_expected_status: Option<i32>,
    pub target_config: Option<serde_json::Value>,
    pub domain_config: Option<serde_json::Value>,
    pub secrets: Option<Vec<DeploymentEnvironmentSecretInput>>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct UpdateDeploymentEnvironmentRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub target_type: Option<DeploymentTargetType>,
    pub is_enabled: Option<bool>,
    pub is_default: Option<bool>,
    pub runtime_type: Option<DeploymentRuntimeType>,
    pub deploy_path: Option<String>,
    pub artifact_strategy: Option<DeploymentArtifactStrategy>,
    pub branch_policy: Option<serde_json::Value>,
    pub healthcheck_url: Option<String>,
    pub healthcheck_timeout_secs: Option<i32>,
    pub healthcheck_expected_status: Option<i32>,
    pub target_config: Option<serde_json::Value>,
    pub domain_config: Option<serde_json::Value>,
    pub secrets: Option<Vec<DeploymentEnvironmentSecretInput>>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct DeploymentCheckResult {
    pub step: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct DeploymentConnectionTestResponse {
    pub success: bool,
    pub checks: Vec<DeploymentCheckResult>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct StartDeploymentRunRequest {
    pub source_type: Option<DeploymentSourceType>,
    pub source_ref: Option<String>,
    pub attempt_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct ListDeploymentRunsQuery {
    pub environment_id: Option<Uuid>,
    pub status: Option<DeploymentRunStatus>,
    pub limit: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct ListDeploymentReleasesQuery {
    pub status: Option<DeploymentReleaseStatus>,
    pub limit: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct RollbackDeploymentRunRequest {
    pub target_release_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
}

/// Build configuration for a project type
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BuildConfig {
    pub command: String,
    pub output_dir: String,
}

/// Result of a build operation
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BuildResult {
    pub artifact_key: String,
    pub size_bytes: u64,
    pub files_count: usize,
    pub build_duration_secs: i32,
}

/// Response when a build is started (non-blocking)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BuildStartedResponse {
    pub attempt_id: Uuid,
    pub status: String,
}

/// Request to trigger a build for an attempt
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct TriggerBuildRequest {
    /// Override build command (optional)
    pub build_command: Option<String>,
    /// Override output directory (optional)
    pub output_dir: Option<String>,
}

/// Request to trigger a production deployment
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct TriggerDeployRequest {
    /// The artifact ID to deploy (if not provided, will use latest build)
    pub artifact_id: Option<Uuid>,
    /// Target deployment type: pages, workers, container
    pub deployment_type: Option<String>,
}

/// Response for deployment operations
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct DeploymentResponse {
    pub deployment_id: Uuid,
    pub url: String,
    pub status: String,
    pub deployment_type: String,
}

/// List deployments query parameters
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct ListDeploymentsQuery {
    /// Filter by deployment type
    pub deployment_type: Option<String>,
    /// Filter by status
    pub status: Option<String>,
    /// Limit results
    pub limit: Option<i32>,
}

/// Preview deployment response with extended info
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct PreviewDeploymentResponse {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub url: String,
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Artifact list response
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ArtifactResponse {
    pub id: Uuid,
    pub artifact_key: String,
    pub artifact_type: String,
    pub size_bytes: Option<i64>,
    pub file_count: Option<i32>,
    pub download_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ===== Subagent Tracking =====

/// Subagent relationship tracking when agents spawn subagents via Task tool
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export)]
pub struct SubagentRelationship {
    pub id: Uuid,
    pub parent_attempt_id: Uuid,
    pub child_attempt_id: Uuid,
    pub spawned_at: DateTime<Utc>,
    pub spawn_tool_use_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::RequirementPriority;

    #[test]
    fn requirement_priority_deserializes_lowercase_values() {
        let parsed: RequirementPriority =
            serde_json::from_str("\"high\"").expect("priority should parse");
        assert_eq!(parsed, RequirementPriority::High);
    }

    #[test]
    fn requirement_priority_deserializes_legacy_pascal_case_values() {
        let parsed: RequirementPriority =
            serde_json::from_str("\"High\"").expect("priority should parse");
        assert_eq!(parsed, RequirementPriority::High);
    }

    #[test]
    fn requirement_priority_serializes_to_lowercase() {
        let encoded = serde_json::to_string(&RequirementPriority::Critical)
            .expect("priority should serialize");
        assert_eq!(encoded, "\"critical\"");
    }
}
