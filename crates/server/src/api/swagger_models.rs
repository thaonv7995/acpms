use super::{ApiErrorDetail, AuthResponseDto, ResponseCode, UserDto};
use acpms_db::models::{CloseSprintResult, ProjectType, SprintOverview};
use utoipa::ToSchema;
use validator::Validate;

macro_rules! define_response {
    ($name:ident, $data_type:ty) => {
        #[derive(ToSchema)]
        #[allow(dead_code)]
        pub struct $name {
            pub success: bool,
            pub code: ResponseCode,
            pub message: String,
            pub data: Option<$data_type>,
            #[schema(value_type = String)]
            pub metadata: Option<serde_json::Value>,
            pub error: Option<ApiErrorDetail>,
        }
    };
}

define_response!(UserResponse, UserDto);
define_response!(UserListResponse, Vec<UserDto>);
define_response!(AuthResponse, AuthResponseDto);
define_response!(EmptyResponse, ());

use super::ProjectDto;

define_response!(ProjectResponse, ProjectDto);
define_response!(ProjectListResponse, Vec<ProjectDto>);

#[derive(ToSchema, serde::Deserialize)]
pub struct ProjectStackSelectionDoc {
    /// Stack layer category (frontend, backend, database, auth, cache, queue)
    #[schema(value_type = String)]
    #[allow(dead_code)]
    pub layer: String,
    /// Selected stack value for the layer (e.g., nextjs, nestjs, postgresql)
    #[allow(dead_code)]
    pub stack: String,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
pub struct CreateProjectRequestDoc {
    #[validate(length(
        min = 1,
        max = 100,
        message = "Project name must be between 1 and 100 characters"
    ))]
    pub name: String,

    #[validate(length(max = 500, message = "Description must not exceed 500 characters"))]
    #[serde(default)]
    pub description: Option<String>,

    #[validate(url(message = "Invalid repository URL format"))]
    #[serde(default)]
    pub repository_url: Option<String>,

    #[schema(value_type = Object)]
    #[allow(dead_code)]
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,

    /// If true, agent changes require human review before commit/push.
    #[allow(dead_code)]
    #[serde(default)]
    pub require_review: Option<bool>,

    /// Flag to enable from-scratch initialization (create new GitLab repo)
    #[allow(dead_code)]
    #[serde(default)]
    pub create_from_scratch: Option<bool>,

    /// GitLab visibility for from-scratch projects: "private", "public", or "internal"
    #[validate(custom(function = "validate_visibility"))]
    #[serde(default)]
    pub visibility: Option<String>,

    /// Preferred tech stack/framework for initialization (e.g., "tauri", "nextjs").
    #[allow(dead_code)]
    #[serde(default)]
    pub tech_stack: Option<String>,

    /// Optional layered stack selections for richer scaffold guidance.
    #[schema(value_type = Vec<ProjectStackSelectionDoc>)]
    #[allow(dead_code)]
    #[serde(default)]
    pub stack_selections: Option<Vec<ProjectStackSelectionDoc>>,

    /// If true (default), create and run the init task automatically.
    #[allow(dead_code)]
    #[serde(default)]
    pub auto_create_init_task: Option<bool>,

    /// Project type classification (web, mobile, desktop, extension, api, microservice)
    #[schema(value_type = String)]
    #[allow(dead_code)]
    #[serde(default)]
    pub project_type: Option<ProjectType>,

    /// Optional template ID used for project bootstrapping.
    #[allow(dead_code)]
    #[serde(default)]
    pub template_id: Option<uuid::Uuid>,

    /// If true, enable preview deployments for this project.
    #[allow(dead_code)]
    #[serde(default)]
    pub preview_enabled: Option<bool>,

    /// Storage keys of reference files (from init-refs/upload-url) for agent to read.
    #[allow(dead_code)]
    #[serde(default)]
    pub reference_keys: Option<Vec<String>>,
}

fn validate_visibility(visibility: &str) -> Result<(), validator::ValidationError> {
    match visibility {
        "private" | "public" | "internal" => Ok(()),
        _ => Err(validator::ValidationError::new("invalid_visibility")),
    }
}

#[derive(ToSchema, serde::Deserialize, Validate)]
pub struct UpdateProjectRequestDoc {
    #[validate(length(
        min = 1,
        max = 100,
        message = "Project name must be between 1 and 100 characters"
    ))]
    #[serde(default)]
    pub name: Option<String>,

    #[validate(length(max = 500, message = "Description must not exceed 500 characters"))]
    #[serde(default)]
    pub description: Option<String>,

    #[validate(url(message = "Invalid repository URL format"))]
    #[serde(default)]
    pub repository_url: Option<String>,

    #[schema(value_type = Object)]
    #[allow(dead_code)]
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,

    /// Require human review before committing agent changes
    #[allow(dead_code)]
    #[serde(default)]
    pub require_review: Option<bool>,
}

// Tasks
use super::{
    ProjectDocumentDto, TaskAttemptDto, TaskContextAttachmentDto, TaskContextDto, TaskDto,
};
define_response!(TaskResponse, TaskDto);
define_response!(TaskListResponse, Vec<TaskDto>);
define_response!(TaskAttemptResponse, TaskAttemptDto);
define_response!(TaskAttemptListResponse, Vec<TaskAttemptDto>);
define_response!(TaskContextResponse, TaskContextDto);
define_response!(TaskContextListResponse, Vec<TaskContextDto>);
define_response!(TaskContextAttachmentResponse, TaskContextAttachmentDto);
define_response!(ProjectDocumentResponse, ProjectDocumentDto);
define_response!(ProjectDocumentListResponse, Vec<ProjectDocumentDto>);

#[derive(ToSchema, serde::Deserialize, Validate)]
pub struct CreateTaskRequestDoc {
    #[allow(dead_code)]
    pub project_id: uuid::Uuid,
    #[allow(dead_code)]
    pub requirement_id: Option<uuid::Uuid>,
    #[allow(dead_code)]
    pub sprint_id: Option<uuid::Uuid>,

    #[validate(length(
        min = 1,
        max = 200,
        message = "Task title must be between 1 and 200 characters"
    ))]
    pub title: String,

    #[validate(length(max = 2000, message = "Description must not exceed 2000 characters"))]
    pub description: Option<String>,

    #[schema(value_type = String)]
    #[allow(dead_code)]
    pub task_type: crate::api::TaskType,
    #[allow(dead_code)]
    pub assigned_to: Option<uuid::Uuid>,
    #[schema(value_type = Object)]
    #[allow(dead_code)]
    pub metadata: Option<serde_json::Value>,
    #[allow(dead_code)]
    pub parent_task_id: Option<uuid::Uuid>,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
pub struct UpdateTaskRequestDoc {
    #[validate(length(
        min = 1,
        max = 200,
        message = "Task title must be between 1 and 200 characters"
    ))]
    pub title: Option<String>,

    #[validate(length(max = 2000, message = "Description must not exceed 2000 characters"))]
    pub description: Option<String>,

    #[schema(value_type = String)]
    #[allow(dead_code)]
    pub task_type: Option<crate::api::TaskType>,
    #[schema(value_type = String)]
    #[allow(dead_code)]
    pub status: Option<crate::api::TaskStatus>,
    #[allow(dead_code)]
    pub assigned_to: Option<uuid::Uuid>,
    #[allow(dead_code)]
    pub sprint_id: Option<uuid::Uuid>,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
#[allow(dead_code)]
pub struct CreateTaskContextRequestDoc {
    #[validate(length(max = 255, message = "Title must not exceed 255 characters"))]
    pub title: Option<String>,

    #[validate(length(min = 1, max = 64, message = "Content type is required"))]
    pub content_type: String,

    #[validate(length(
        max = 20000,
        message = "Context content must not exceed 20000 characters"
    ))]
    pub raw_content: String,

    #[validate(length(min = 1, max = 32, message = "Source is required"))]
    pub source: String,

    pub sort_order: i32,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
#[allow(dead_code)]
pub struct OpenClawCreateTaskContextRequestDoc {
    #[validate(length(max = 255, message = "Title must not exceed 255 characters"))]
    pub title: Option<String>,

    #[validate(length(min = 1, max = 64, message = "Content type is required"))]
    pub content_type: String,

    #[validate(length(
        max = 20000,
        message = "Context content must not exceed 20000 characters"
    ))]
    pub raw_content: String,

    pub sort_order: i32,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
#[allow(dead_code)]
pub struct UpdateTaskContextRequestDoc {
    #[validate(length(max = 255, message = "Title must not exceed 255 characters"))]
    pub title: Option<Option<String>>,

    #[validate(length(min = 1, max = 64, message = "Content type is required"))]
    pub content_type: Option<String>,

    #[validate(length(
        max = 20000,
        message = "Context content must not exceed 20000 characters"
    ))]
    pub raw_content: Option<String>,

    pub sort_order: Option<i32>,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
#[allow(dead_code)]
pub struct CreateTaskContextAttachmentRequestDoc {
    #[validate(length(
        min = 1,
        max = 512,
        message = "Storage key must be between 1 and 512 characters"
    ))]
    pub storage_key: String,

    #[validate(length(
        min = 1,
        max = 255,
        message = "Filename must be between 1 and 255 characters"
    ))]
    pub filename: String,

    #[validate(length(min = 1, max = 255, message = "Content type is required"))]
    pub content_type: String,

    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
#[allow(dead_code)]
pub struct CreateProjectDocumentRequestDoc {
    #[validate(length(
        min = 1,
        max = 255,
        message = "Title must be between 1 and 255 characters"
    ))]
    pub title: String,

    #[validate(length(
        min = 1,
        max = 255,
        message = "Filename must be between 1 and 255 characters"
    ))]
    pub filename: String,

    #[validate(length(min = 1, max = 32, message = "Document kind is required"))]
    pub document_kind: String,

    #[validate(length(min = 1, max = 255, message = "Content type is required"))]
    pub content_type: String,

    #[validate(length(
        min = 1,
        max = 512,
        message = "Storage key must be between 1 and 512 characters"
    ))]
    pub storage_key: Option<String>,

    pub checksum: Option<String>,
    pub size_bytes: Option<i64>,
    pub content_text: Option<String>,

    #[validate(length(min = 1, max = 32, message = "Source is required"))]
    pub source: String,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
#[allow(dead_code)]
pub struct OpenClawCreateProjectDocumentRequestDoc {
    #[validate(length(
        min = 1,
        max = 255,
        message = "Title must be between 1 and 255 characters"
    ))]
    pub title: String,

    #[validate(length(
        min = 1,
        max = 255,
        message = "Filename must be between 1 and 255 characters"
    ))]
    pub filename: String,

    #[validate(length(min = 1, max = 32, message = "Document kind is required"))]
    pub document_kind: String,

    #[validate(length(min = 1, max = 255, message = "Content type is required"))]
    pub content_type: String,

    #[validate(length(min = 1, message = "Document content must not be empty"))]
    pub content_text: String,

    #[validate(length(min = 1, max = 32, message = "Source is required"))]
    pub source: String,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
#[allow(dead_code)]
pub struct UpdateProjectDocumentRequestDoc {
    #[validate(length(
        min = 1,
        max = 255,
        message = "Title must be between 1 and 255 characters"
    ))]
    pub title: Option<String>,

    #[validate(length(min = 1, max = 32, message = "Document kind is required"))]
    pub document_kind: Option<String>,

    #[validate(length(min = 1, max = 255, message = "Content type is required"))]
    pub content_type: Option<String>,

    #[validate(length(
        min = 1,
        max = 512,
        message = "Storage key must be between 1 and 512 characters"
    ))]
    pub storage_key: Option<String>,

    pub checksum: Option<Option<String>>,
    pub size_bytes: Option<i64>,
}

// Task Attempts - Agent Logs
use super::AgentLogDto;
define_response!(AgentLogResponse, AgentLogDto);
define_response!(AgentLogListResponse, Vec<AgentLogDto>);

#[derive(ToSchema, serde::Deserialize, Validate)]
pub struct CreateTaskAttemptRequestDoc {
    #[allow(dead_code)]
    pub task_id: uuid::Uuid,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
pub struct SendInputRequestDoc {
    #[validate(length(min = 1, message = "Input cannot be empty"))]
    pub input: String,
}

// Sprints
use super::SprintDto;
define_response!(SprintResponse, SprintDto);
define_response!(SprintListResponse, Vec<SprintDto>);
define_response!(CloseSprintResultResponse, CloseSprintResult);
define_response!(SprintOverviewResponse, SprintOverview);

#[derive(ToSchema, serde::Deserialize, Validate)]
pub struct CreateSprintRequestDoc {
    #[allow(dead_code)]
    #[serde(default)]
    pub sequence: Option<i32>,

    #[validate(length(
        min = 1,
        max = 100,
        message = "Sprint name must be between 1 and 100 characters"
    ))]
    pub name: String,

    #[validate(length(max = 500, message = "Description must not exceed 500 characters"))]
    pub description: Option<String>,
    #[validate(length(max = 500, message = "Goal must not exceed 500 characters"))]
    pub goal: Option<String>,

    #[allow(dead_code)]
    #[serde(default)]
    pub start_date: Option<chrono::DateTime<chrono::Utc>>,
    #[allow(dead_code)]
    #[serde(default)]
    pub end_date: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
pub struct UpdateSprintRequestDoc {
    #[validate(length(
        min = 1,
        max = 100,
        message = "Sprint name must be between 1 and 100 characters"
    ))]
    pub name: Option<String>,

    #[validate(length(max = 500, message = "Description must not exceed 500 characters"))]
    pub description: Option<String>,
    #[validate(length(max = 500, message = "Goal must not exceed 500 characters"))]
    pub goal: Option<String>,

    #[schema(value_type = String)]
    #[allow(dead_code)]
    pub status: Option<crate::api::SprintStatus>,
    #[allow(dead_code)]
    pub start_date: Option<chrono::DateTime<chrono::Utc>>,
    #[allow(dead_code)]
    pub end_date: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
pub struct GenerateSprintsRequestDoc {
    #[allow(dead_code)]
    pub start_date: chrono::DateTime<chrono::Utc>,

    #[validate(range(min = 1, max = 8, message = "Duration must be between 1 and 8 weeks"))]
    pub duration_weeks: i32,

    #[validate(range(min = 1, max = 10, message = "Count must be between 1 and 10"))]
    pub count: i32,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
#[allow(dead_code)]
pub struct CreateNextSprintRequestDoc {
    #[validate(length(
        min = 1,
        max = 100,
        message = "Sprint name must be between 1 and 100 characters"
    ))]
    pub name: Option<String>,

    #[validate(length(max = 500, message = "Goal must not exceed 500 characters"))]
    pub goal: Option<String>,

    #[allow(dead_code)]
    pub start_date: Option<chrono::DateTime<chrono::Utc>>,
    #[allow(dead_code)]
    pub end_date: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
#[allow(dead_code)]
pub struct CloseSprintRequestDoc {
    #[schema(value_type = String)]
    pub carry_over_mode: acpms_db::models::SprintCarryOverMode,
    #[allow(dead_code)]
    pub next_sprint_id: Option<uuid::Uuid>,
    pub create_next_sprint: Option<CreateNextSprintRequestDoc>,
    #[validate(length(max = 1000, message = "Reason must not exceed 1000 characters"))]
    pub reason: Option<String>,
}

// Requirements
use super::RequirementDto;
define_response!(RequirementResponse, RequirementDto);
define_response!(RequirementListResponse, Vec<RequirementDto>);

#[derive(ToSchema, serde::Deserialize, Validate)]
pub struct CreateRequirementRequestDoc {
    #[allow(dead_code)]
    pub project_id: uuid::Uuid,
    #[allow(dead_code)]
    pub sprint_id: Option<uuid::Uuid>,

    #[validate(length(
        min = 1,
        max = 200,
        message = "Requirement title must be between 1 and 200 characters"
    ))]
    pub title: String,

    #[validate(length(min = 1, message = "Content is required"))]
    pub content: String,

    #[schema(value_type = String)]
    #[allow(dead_code)]
    pub priority: Option<crate::api::RequirementPriority>,

    #[schema(value_type = Option<String>, example = "2026-03-15")]
    #[allow(dead_code)]
    pub due_date: Option<chrono::NaiveDate>,

    #[schema(value_type = String)]
    #[allow(dead_code)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(ToSchema, serde::Deserialize, Validate)]
pub struct UpdateRequirementRequestDoc {
    #[validate(length(
        min = 1,
        max = 200,
        message = "Requirement title must be between 1 and 200 characters"
    ))]
    pub title: Option<String>,

    #[validate(length(min = 1, message = "Content cannot be empty"))]
    pub content: Option<String>,

    #[allow(dead_code)]
    pub sprint_id: Option<uuid::Uuid>,

    #[schema(value_type = String)]
    #[allow(dead_code)]
    pub status: Option<crate::api::RequirementStatus>,

    #[schema(value_type = String)]
    #[allow(dead_code)]
    pub priority: Option<crate::api::RequirementPriority>,

    #[schema(value_type = Option<String>, example = "2026-03-15")]
    #[allow(dead_code)]
    pub due_date: Option<chrono::NaiveDate>,

    #[schema(value_type = String)]
    #[allow(dead_code)]
    pub metadata: Option<serde_json::Value>,
}

// GitLab
use super::{
    GitLabConfigurationDto, MergeRequestDto, MergeRequestOverviewDto, MergeRequestStatsDto,
};
define_response!(GitLabConfigurationResponse, GitLabConfigurationDto);
define_response!(MergeRequestListResponse, Vec<MergeRequestDto>);
define_response!(
    MergeRequestOverviewListResponse,
    Vec<MergeRequestOverviewDto>
);
define_response!(MergeRequestStatsResponse, MergeRequestStatsDto);

#[derive(ToSchema, serde::Deserialize)]
#[allow(dead_code)] // Fields used by ToSchema for OpenAPI docs
pub struct LinkGitLabProjectRequestDoc {
    /// GitLab project ID (numeric). Optional if repository_url is provided.
    pub gitlab_project_id: Option<i64>,

    /// Repository URL (e.g. https://gitlab.com/group/repo). Resolved to project_id via GitLab API.
    pub repository_url: Option<String>,
}

// Dashboard
#[derive(ToSchema, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardDataDoc {
    pub stats: DashboardStatsDoc,
    pub projects: Vec<DashboardProjectDoc>,
    pub agent_logs: Vec<DashboardAgentLogDoc>,
    pub human_tasks: Vec<DashboardHumanTaskDoc>,
}

#[derive(ToSchema, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStatsDoc {
    pub active_projects: StatsMetricDoc,
    pub agents_online: AgentStatsDoc,
    pub system_load: SystemLoadDoc,
    pub pending_prs: PrStatsDoc,
}

#[derive(ToSchema, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsMetricDoc {
    pub count: i64,
    pub trend: String,
}

#[derive(ToSchema, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatsDoc {
    pub online: i64,
    pub total: i64,
}

#[derive(ToSchema, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemLoadDoc {
    pub percentage: i64,
    pub status: String,
}

#[derive(ToSchema, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrStatsDoc {
    pub count: i64,
    pub requires_review: bool,
}

#[derive(ToSchema, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardProjectDoc {
    pub id: uuid::Uuid,
    pub name: String,
    pub subtitle: String,
    pub status: String,
    pub progress: i64,
    pub agents: Vec<AgentAvatarDoc>,
}

#[derive(ToSchema, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAvatarDoc {
    pub id: String,
    pub initial: String,
    pub color: String,
}

#[derive(ToSchema, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardAgentLogDoc {
    pub id: uuid::Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub agent_name: String,
    pub agent_color: String,
    pub message: String,
    pub highlight: Option<String>,
}

#[derive(ToSchema, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardHumanTaskDoc {
    pub id: uuid::Uuid,
    #[serde(rename = "projectId")]
    pub project_id: uuid::Uuid,
    #[serde(rename = "projectName")]
    pub project_name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub title: String,
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub assignee: Option<UserAvatarDoc>,
}

#[derive(ToSchema, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserAvatarDoc {
    pub id: uuid::Uuid,
    pub avatar: Option<String>,
}

define_response!(DashboardResponse, DashboardDataDoc);
