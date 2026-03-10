use acpms_db::models::{
    AgentLog, GitLabConfiguration, MergeRequestDb, Project, ProjectDocument, ProjectSettings,
    ProjectType, RepositoryContext, Requirement, ReviewComment, Sprint, Task, TaskAttempt,
    TaskContext, TaskContextAttachment, TaskWithAttemptStatus, User,
};
use acpms_services::{ProjectComputedSummary, TaskContextWithAttachments, TaskWithLatestAttempt};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

// Re-export enums and types for OpenAPI documentation and usage
pub use acpms_db::models::{
    AttemptStatus, RequirementPriority, RequirementStatus, SprintStatus, SystemRole, TaskStatus,
    TaskType,
};

// User DTO
#[derive(Debug, Serialize, ToSchema)]
pub struct UserDto {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub gitlab_username: Option<String>,
    #[schema(value_type = Vec<String>)]
    pub global_roles: Vec<SystemRole>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for UserDto {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            name: user.name,
            avatar_url: user.avatar_url,
            gitlab_username: user.gitlab_username,
            global_roles: user.global_roles,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

// Auth Response DTO
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponseDto {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64, // seconds until access token expires
    pub user: UserDto,
}

// Project DTO
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProjectSummaryDto {
    #[schema(value_type = String)]
    pub lifecycle_status: String,
    #[schema(value_type = String)]
    pub execution_status: String,
    pub progress: i64,
    pub total_tasks: i64,
    pub completed_tasks: i64,
    pub active_tasks: i64,
    pub review_tasks: i64,
    pub blocked_tasks: i64,
}

impl From<ProjectComputedSummary> for ProjectSummaryDto {
    fn from(summary: ProjectComputedSummary) -> Self {
        Self {
            lifecycle_status: summary.lifecycle_status,
            execution_status: summary.execution_status,
            progress: summary.progress,
            total_tasks: summary.total_tasks,
            completed_tasks: summary.completed_tasks,
            active_tasks: summary.active_tasks,
            review_tasks: summary.review_tasks,
            blocked_tasks: summary.blocked_tasks,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectDto {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub repository_url: Option<String>,
    pub repository_context: RepositoryContext,
    #[schema(value_type = Object)]
    pub metadata: serde_json::Value,
    #[schema(value_type = Object)]
    pub architecture_config: serde_json::Value,
    /// Legacy field for backward compatibility - use settings.require_review instead
    pub require_review: bool,
    /// Project-level settings controlling agent execution, deployment, review, and notifications
    pub settings: ProjectSettings,
    /// Project type classification (web, mobile, desktop, extension, api, microservice)
    #[schema(value_type = String)]
    pub project_type: ProjectType,
    pub summary: Option<ProjectSummaryDto>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Project> for ProjectDto {
    fn from(project: Project) -> Self {
        Self {
            id: project.id,
            name: project.name,
            description: project.description,
            repository_url: project.repository_url,
            repository_context: project.repository_context,
            metadata: project.metadata,
            architecture_config: project.architecture_config,
            require_review: project.require_review,
            settings: project.settings,
            project_type: project.project_type,
            summary: None,
            created_by: project.created_by,
            created_at: project.created_at,
            updated_at: project.updated_at,
        }
    }
}

impl ProjectDto {
    pub fn with_summary(mut self, summary: ProjectSummaryDto) -> Self {
        self.summary = Some(summary);
        self
    }
}

// Task DTO
#[derive(Debug, Serialize, ToSchema)]
pub struct TaskDto {
    pub id: Uuid,
    pub project_id: Uuid,
    pub requirement_id: Option<Uuid>,
    pub sprint_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    #[schema(value_type = String)]
    pub task_type: TaskType,
    #[schema(value_type = String)]
    pub status: TaskStatus,
    pub assigned_to: Option<Uuid>,
    pub parent_task_id: Option<Uuid>,
    pub gitlab_issue_id: Option<i32>,
    #[schema(value_type = Object)]
    pub metadata: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Latest attempt ID for this task (for kanban views)
    pub latest_attempt_id: Option<Uuid>,
    /// Has at least one attempt with status='running' (for kanban spinner)
    pub has_in_progress_attempt: Option<bool>,
    /// Latest attempt has status='failed' (for kanban X icon)
    pub last_attempt_failed: Option<bool>,
    /// Executor from latest attempt metadata (for kanban badge)
    pub executor: Option<String>,
}

impl From<Task> for TaskDto {
    fn from(task: Task) -> Self {
        Self {
            id: task.id,
            project_id: task.project_id,
            requirement_id: task.requirement_id,
            sprint_id: task.sprint_id,
            title: task.title,
            description: task.description,
            task_type: task.task_type,
            status: task.status,
            assigned_to: task.assigned_to,
            parent_task_id: task.parent_task_id,
            gitlab_issue_id: task.gitlab_issue_id,
            metadata: task.metadata,
            created_by: task.created_by,
            created_at: task.created_at,
            updated_at: task.updated_at,
            latest_attempt_id: None,
            has_in_progress_attempt: None,
            last_attempt_failed: None,
            executor: None,
        }
    }
}

impl From<TaskWithLatestAttempt> for TaskDto {
    fn from(task: TaskWithLatestAttempt) -> Self {
        Self {
            id: task.id,
            project_id: task.project_id,
            requirement_id: task.requirement_id,
            sprint_id: task.sprint_id,
            title: task.title,
            description: task.description,
            task_type: task.task_type,
            status: task.status,
            assigned_to: task.assigned_to,
            parent_task_id: task.parent_task_id,
            gitlab_issue_id: task.gitlab_issue_id,
            metadata: task.metadata,
            created_by: task.created_by,
            created_at: task.created_at,
            updated_at: task.updated_at,
            latest_attempt_id: task.latest_attempt_id,
            has_in_progress_attempt: None,
            last_attempt_failed: None,
            executor: None,
        }
    }
}

impl From<TaskWithAttemptStatus> for TaskDto {
    fn from(task: TaskWithAttemptStatus) -> Self {
        Self {
            id: task.id,
            project_id: task.project_id,
            requirement_id: task.requirement_id,
            sprint_id: task.sprint_id,
            title: task.title,
            description: task.description,
            task_type: task.task_type,
            status: task.status,
            assigned_to: task.assigned_to,
            parent_task_id: task.parent_task_id,
            gitlab_issue_id: task.gitlab_issue_id,
            metadata: task.metadata,
            created_by: task.created_by,
            created_at: task.created_at,
            updated_at: task.updated_at,
            latest_attempt_id: None, // TaskWithAttemptStatus doesn't have this field
            has_in_progress_attempt: Some(task.has_in_progress_attempt),
            last_attempt_failed: Some(task.last_attempt_failed),
            executor: task.executor,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskContextAttachmentDto {
    pub id: Uuid,
    pub task_context_id: Uuid,
    pub storage_key: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<TaskContextAttachment> for TaskContextAttachmentDto {
    fn from(attachment: TaskContextAttachment) -> Self {
        Self {
            id: attachment.id,
            task_context_id: attachment.task_context_id,
            storage_key: attachment.storage_key,
            filename: attachment.filename,
            content_type: attachment.content_type,
            size_bytes: attachment.size_bytes,
            checksum: attachment.checksum,
            created_at: attachment.created_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskContextDto {
    pub id: Uuid,
    pub task_id: Uuid,
    pub title: Option<String>,
    pub content_type: String,
    pub raw_content: String,
    pub source: String,
    pub sort_order: i32,
    pub attachments: Vec<TaskContextAttachmentDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TaskContextDto {
    pub fn from_parts(context: TaskContext, attachments: Vec<TaskContextAttachmentDto>) -> Self {
        Self {
            id: context.id,
            task_id: context.task_id,
            title: context.title,
            content_type: context.content_type,
            raw_content: context.raw_content,
            source: context.source,
            sort_order: context.sort_order,
            attachments,
            created_at: context.created_at,
            updated_at: context.updated_at,
        }
    }
}

impl From<TaskContextWithAttachments> for TaskContextDto {
    fn from(value: TaskContextWithAttachments) -> Self {
        Self::from_parts(
            value.context,
            value
                .attachments
                .into_iter()
                .map(TaskContextAttachmentDto::from)
                .collect(),
        )
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectDocumentDto {
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ProjectDocument> for ProjectDocumentDto {
    fn from(document: ProjectDocument) -> Self {
        Self {
            id: document.id,
            project_id: document.project_id,
            title: document.title,
            filename: document.filename,
            document_kind: document.document_kind,
            content_type: document.content_type,
            storage_key: document.storage_key,
            checksum: document.checksum,
            size_bytes: document.size_bytes,
            source: document.source,
            version: document.version,
            ingestion_status: document.ingestion_status,
            index_error: document.index_error,
            indexed_at: document.indexed_at,
            created_at: document.created_at,
            updated_at: document.updated_at,
        }
    }
}

// Sprint DTO
#[derive(Debug, Serialize, ToSchema)]
pub struct SprintDto {
    pub id: Uuid,
    pub project_id: Uuid,
    pub sequence: i32,
    pub name: String,
    pub description: Option<String>,
    pub goal: Option<String>,
    #[schema(value_type = String)]
    pub status: SprintStatus,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub closed_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Sprint> for SprintDto {
    fn from(sprint: Sprint) -> Self {
        Self {
            id: sprint.id,
            project_id: sprint.project_id,
            sequence: sprint.sequence,
            name: sprint.name,
            description: sprint.description,
            goal: sprint.goal,
            status: sprint.status,
            start_date: sprint.start_date,
            end_date: sprint.end_date,
            closed_at: sprint.closed_at,
            closed_by: sprint.closed_by,
            created_at: sprint.created_at,
            updated_at: sprint.updated_at,
        }
    }
}

// Requirement DTO
#[derive(Debug, Serialize, ToSchema)]
pub struct RequirementDto {
    pub id: Uuid,
    pub project_id: Uuid,
    pub sprint_id: Option<Uuid>,
    pub title: String,
    pub content: String,
    #[schema(value_type = String)]
    pub status: RequirementStatus,
    #[schema(value_type = String)]
    pub priority: RequirementPriority,
    #[schema(value_type = Option<String>)]
    pub due_date: Option<chrono::NaiveDate>,
    #[schema(value_type = Object)]
    pub metadata: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Requirement> for RequirementDto {
    fn from(req: Requirement) -> Self {
        Self {
            id: req.id,
            project_id: req.project_id,
            sprint_id: req.sprint_id,
            title: req.title,
            content: req.content,
            status: req.status,
            priority: req.priority,
            due_date: req.due_date,
            metadata: req.metadata,
            created_by: req.created_by,
            created_at: req.created_at,
            updated_at: req.updated_at,
        }
    }
}

// Task Attempt DTO
#[derive(Debug, Serialize, ToSchema)]
pub struct TaskAttemptDto {
    pub id: Uuid,
    pub task_id: Uuid,
    #[schema(value_type = String)]
    pub status: AttemptStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    #[schema(value_type = Object)]
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl From<TaskAttempt> for TaskAttemptDto {
    fn from(attempt: TaskAttempt) -> Self {
        Self {
            id: attempt.id,
            task_id: attempt.task_id,
            status: attempt.status,
            started_at: attempt.started_at,
            completed_at: attempt.completed_at,
            error_message: attempt.error_message,
            metadata: attempt.metadata,
            created_at: attempt.created_at,
        }
    }
}

// Agent Log DTO - matches frontend AttemptLog type
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentLogDto {
    pub id: Uuid,
    pub attempt_id: Uuid,
    #[serde(rename = "type")]
    pub log_type: String,
    #[serde(rename = "message")]
    pub content: String,
    #[serde(rename = "timestamp")]
    pub created_at: DateTime<Utc>,
    pub level: String,
}

impl From<AgentLog> for AgentLogDto {
    fn from(log: AgentLog) -> Self {
        // Map log_type to level
        let level = match log.log_type.as_str() {
            "error" | "stderr" => "error",
            "system" => "info",
            "stdout" => "info",
            _ => "info",
        }
        .to_string();

        Self {
            id: log.id,
            attempt_id: log.attempt_id,
            log_type: log.log_type,
            content: log.content,
            created_at: log.created_at,
            level,
        }
    }
}

// GitLab Integration DTOs
#[derive(Debug, Serialize, ToSchema)]
pub struct GitLabConfigurationDto {
    pub id: Uuid,
    pub project_id: Uuid,
    pub gitlab_project_id: i64,
    pub base_url: String,
    // Sensitive data like encrypted PAT or webhook secret should explicitly NOT be exposing in DTO unless necessary
    // Excluding pat_encrypted and webhook_secret
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<GitLabConfiguration> for GitLabConfigurationDto {
    fn from(config: GitLabConfiguration) -> Self {
        Self {
            id: config.id,
            project_id: config.project_id,
            gitlab_project_id: config.gitlab_project_id,
            base_url: config.base_url,
            created_at: config.created_at,
            updated_at: config.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MergeRequestDto {
    pub id: Uuid,
    pub task_id: Uuid,
    pub attempt_id: Option<Uuid>,
    /// MR/PR number: GitLab IID or GitHub PR number
    pub mr_number: i64,
    pub web_url: String,
    pub status: String,
    pub provider: String,
    pub source_repository_url: Option<String>,
    pub target_repository_url: Option<String>,
    pub source_branch: Option<String>,
    pub target_branch: Option<String>,
    pub source_project_id: Option<i64>,
    pub target_project_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<MergeRequestDb> for MergeRequestDto {
    fn from(mr: MergeRequestDb) -> Self {
        let mr_number = mr.gitlab_mr_iid.or(mr.github_pr_number).unwrap_or(0);
        Self {
            id: mr.id,
            task_id: mr.task_id,
            attempt_id: mr.attempt_id,
            mr_number,
            web_url: mr.web_url,
            status: mr.status,
            provider: mr.provider,
            source_repository_url: mr.source_repository_url,
            target_repository_url: mr.target_repository_url,
            source_branch: mr.source_branch,
            target_branch: mr.target_branch,
            source_project_id: mr.source_project_id,
            target_project_id: mr.target_project_id,
            created_at: mr.created_at,
            updated_at: mr.updated_at,
        }
    }
}

/// Merge request overview DTO for dashboard/list page.
#[derive(Debug, Serialize, ToSchema)]
pub struct MergeRequestOverviewDto {
    pub id: Uuid,
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub title: String,
    pub description: Option<String>,
    pub mr_number: i64,
    pub status: String,
    pub web_url: String,
    pub author_name: String,
    pub author_avatar: Option<String>,
    pub author_is_agent: bool,
    pub source_branch: String,
    pub target_branch: String,
    pub changed_files: i32,
    pub additions: i32,
    pub deletions: i32,
    pub latest_attempt_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Merge request dashboard statistics.
#[derive(Debug, Serialize, ToSchema)]
pub struct MergeRequestStatsDto {
    pub open: i64,
    pub pending_review: i64,
    pub merged: i64,
    pub ai_generated: i64,
}

// Agent Activity DTOs (for global agent logs dashboard)

/// Agent status for the activity dashboard
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentActivityStatusDto {
    pub id: Uuid,
    pub name: String,
    pub task_title: String,
    pub project_name: String,
    #[schema(value_type = String)]
    pub status: AttemptStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Extended agent log with task/project context
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentActivityLogDto {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub task_id: Uuid,
    pub task_title: String,
    pub project_name: String,
    pub log_type: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

// Retry Information DTOs

/// Retry information for a task attempt
#[derive(Debug, Serialize, ToSchema)]
pub struct RetryInfoDto {
    /// Current retry count (0 for first attempt)
    pub retry_count: i32,
    /// Maximum retries allowed (from project settings)
    pub max_retries: i32,
    /// Remaining retry attempts
    pub remaining_retries: i32,
    /// Whether this attempt can be manually retried
    pub can_retry: bool,
    /// Whether auto-retry is enabled for this project
    pub auto_retry_enabled: bool,
    /// Previous attempt ID (if this is a retry attempt)
    pub previous_attempt_id: Option<Uuid>,
    /// Error from previous attempt (if this is a retry)
    pub previous_error: Option<String>,
    /// Next scheduled retry attempt ID (if auto-retry scheduled)
    pub next_retry_attempt_id: Option<Uuid>,
    /// Backoff duration in seconds until next retry (if retry possible)
    pub next_backoff_seconds: Option<u64>,
}

/// Response for retry operation
#[derive(Debug, Serialize, ToSchema)]
pub struct RetryResponseDto {
    /// The newly created retry attempt
    pub attempt: TaskAttemptDto,
    /// Retry information
    pub retry_info: RetryInfoDto,
}

// Review Comment DTOs

/// Review comment DTO for API responses
#[derive(Debug, Serialize, ToSchema)]
pub struct ReviewCommentDto {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub user_avatar: Option<String>,
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
    pub resolved_by_name: Option<String>,
    /// Timestamp when comment was resolved
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ReviewComment> for ReviewCommentDto {
    fn from(comment: ReviewComment) -> Self {
        Self {
            id: comment.id,
            attempt_id: comment.attempt_id,
            user_id: comment.user_id,
            user_name: "Unknown User".to_string(),
            user_avatar: None,
            file_path: comment.file_path,
            line_number: comment.line_number,
            content: comment.content,
            resolved: comment.resolved,
            resolved_by: comment.resolved_by,
            resolved_by_name: None,
            resolved_at: comment.resolved_at,
            created_at: comment.created_at,
            updated_at: comment.updated_at,
        }
    }
}

/// Request changes response DTO
#[derive(Debug, Serialize, ToSchema)]
pub struct RequestChangesResponseDto {
    /// The original attempt that was reviewed
    pub original_attempt_id: Uuid,
    /// The new attempt created with feedback
    pub new_attempt_id: Uuid,
    /// Feedback that was included
    pub feedback: String,
    /// Number of review comments included
    pub comments_included: i32,
}
