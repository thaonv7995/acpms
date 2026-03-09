use acpms_db::models::{
    project_repo_relative_path, AttemptStatus, ExecutionProcess as DbExecutionProcess,
    FileDiff as DbFileDiff, Project, ProjectSettings, RepositoryAccessMode, RepositoryContext,
    RepositoryVerificationStatus, SendInputRequest, Task, TaskStatus, TaskType,
    UpdateProjectRequest,
};
use acpms_executors::{
    build_skill_instruction_context, build_skill_metadata_patch, format_loaded_skills_log_line,
    AgentEvent, JobPriority, RetryInfo, SkillInstructionContext, SkillKnowledgeStatus,
    StatusManager, StatusMessage, SuggestedSkill,
};
use acpms_services::{
    NormalizedLogService, ProjectService, RepositoryAccessService, SubagentService,
    TaskAttemptService, TaskService,
};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

use crate::api::{AgentLogDto, ApiResponse, RetryInfoDto, RetryResponseDto, TaskAttemptDto};
use crate::error::{ApiError, ApiResult};
use crate::middleware::{AuthUser, Permission, RbacChecker};
use crate::routes::openclaw;
use crate::AppState;
use utoipa::{IntoParams, ToSchema};

fn task_require_review(task: &Task, settings: &ProjectSettings) -> bool {
    // Analysis-only tasks (e.g. requirement breakdown sessions) should not enter manual review.
    // They are non-execution support tasks and should complete automatically.
    if task_allows_analysis_only_attempt(task) {
        return false;
    }

    task.metadata
        .get("execution")
        .and_then(|v| v.get("require_review"))
        .and_then(|v| v.as_bool())
        .or_else(|| {
            task.metadata
                .get("require_review")
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(settings.require_review)
}

fn task_follow_up_creates_new_attempt(task: &Task) -> bool {
    task.status == TaskStatus::Done
}

fn task_run_build_and_tests(task: &Task) -> bool {
    task.metadata
        .get("execution")
        .and_then(|v| v.get("run_build_and_tests"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

fn task_allows_analysis_only_attempt(task: &Task) -> bool {
    let root_no_code_changes = task
        .metadata
        .get("no_code_changes")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let execution_no_code_changes = task
        .metadata
        .get("execution")
        .and_then(|v| v.get("no_code_changes"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let breakdown_mode_ai_support = task
        .metadata
        .get("breakdown_mode")
        .and_then(|v| v.as_str())
        .map(|v| v.eq_ignore_ascii_case("ai_support"))
        .unwrap_or(false);
    let breakdown_kind_analysis = task
        .metadata
        .get("breakdown_kind")
        .and_then(|v| v.as_str())
        .map(|v| v.eq_ignore_ascii_case("analysis_session"))
        .unwrap_or(false);

    root_no_code_changes
        || execution_no_code_changes
        || breakdown_mode_ai_support
        || breakdown_kind_analysis
}

fn repository_mode_blocks_coding_attempt(context: &RepositoryContext) -> bool {
    matches!(
        context.access_mode,
        RepositoryAccessMode::AnalysisOnly | RepositoryAccessMode::Unknown
    )
}

fn repository_access_mode_label(mode: RepositoryAccessMode) -> &'static str {
    match mode {
        RepositoryAccessMode::AnalysisOnly => "analysis_only",
        RepositoryAccessMode::DirectGitops => "direct_gitops",
        RepositoryAccessMode::BranchPushOnly => "branch_push_only",
        RepositoryAccessMode::ForkGitops => "fork_gitops",
        RepositoryAccessMode::Unknown => "unknown",
    }
}

fn repository_verification_status_label(status: RepositoryVerificationStatus) -> &'static str {
    match status {
        RepositoryVerificationStatus::Verified => "verified",
        RepositoryVerificationStatus::Unauthenticated => "unauthenticated",
        RepositoryVerificationStatus::Failed => "failed",
        RepositoryVerificationStatus::Unknown => "unknown",
    }
}

fn repository_provider_label(provider: acpms_db::models::RepositoryProvider) -> &'static str {
    match provider {
        acpms_db::models::RepositoryProvider::Github => "github",
        acpms_db::models::RepositoryProvider::Gitlab => "gitlab",
        acpms_db::models::RepositoryProvider::Unknown => "unknown",
    }
}

async fn maybe_autorecheck_legacy_repository_context(
    state: &AppState,
    project_service: &ProjectService,
    project: Project,
    source: &str,
) -> Project {
    let Some(repository_url) = project
        .repository_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
    else {
        return project;
    };

    if !project.repository_context.needs_backfill() {
        return project;
    }

    state
        .metrics
        .repository_backfill_total
        .with_label_values(&[source, "attempted"])
        .inc();

    let access_service = RepositoryAccessService::new((*state.settings_service).clone());
    let clone_error = access_service.check_cloneable(&repository_url).await.err();
    let can_clone = clone_error.is_none();
    let repository_context = {
        let mut context = access_service.preflight(&repository_url, can_clone).await;
        if let Some(error) = clone_error {
            context.access_mode = RepositoryAccessMode::Unknown;
            context.verification_status = RepositoryVerificationStatus::Failed;
            context.verification_error = Some(match context.verification_error {
                Some(existing) if !existing.trim().is_empty() => {
                    format!("{} Clone check failed: {}", existing, error)
                }
                _ => format!("Clone check failed: {}", error),
            });
            context.can_clone = false;
            context.can_push = false;
            context.can_open_change_request = false;
            context.can_merge = false;
            context.can_manage_webhooks = false;
        }
        context
    };

    state
        .metrics
        .repository_access_evaluations_total
        .with_label_values(&[
            source,
            repository_provider_label(repository_context.provider),
            repository_access_mode_label(repository_context.access_mode),
            repository_verification_status_label(repository_context.verification_status),
        ])
        .inc();

    match project_service
        .update_project(
            project.id,
            UpdateProjectRequest {
                name: None,
                description: None,
                repository_url: None,
                repository_context: Some(repository_context.clone()),
                metadata: None,
                require_review: None,
            },
        )
        .await
    {
        Ok(updated_project) => {
            state
                .metrics
                .repository_backfill_total
                .with_label_values(&[source, "success"])
                .inc();
            tracing::info!(
                project_id = %project.id,
                repository_url = %repository_url,
                access_mode = repository_access_mode_label(repository_context.access_mode),
                verification_status = repository_verification_status_label(repository_context.verification_status),
                "Auto-refreshed legacy repository context before attempt creation"
            );
            updated_project
        }
        Err(error) => {
            state
                .metrics
                .repository_backfill_total
                .with_label_values(&[source, "failure"])
                .inc();
            tracing::warn!(
                project_id = %project.id,
                repository_url = %repository_url,
                error = %error,
                "Failed to auto-refresh legacy repository context before attempt creation"
            );
            project
        }
    }
}

fn parse_repo_host_and_path(repo_url: &str) -> Option<(String, String)> {
    let trimmed = repo_url.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
    {
        let without_auth = rest.rsplit('@').next().unwrap_or(rest);
        let (host, path) = without_auth.split_once('/')?;
        let host = host.trim().to_ascii_lowercase();
        let path = path.trim().trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path).to_string();
        if host.is_empty() || path.is_empty() {
            return None;
        }
        return Some((host, path));
    }

    if let Some((left, right)) = trimmed.split_once(':') {
        if let Some(host) = left.split('@').nth(1) {
            let host = host.trim().to_ascii_lowercase();
            let path = right.trim().trim_matches('/');
            let path = path.strip_suffix(".git").unwrap_or(path).to_string();
            if host.is_empty() || path.is_empty() {
                return None;
            }
            return Some((host, path));
        }
    }

    None
}

fn parse_host_from_urlish(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let without_auth = without_scheme.rsplit('@').next().unwrap_or(without_scheme);
    let host = without_auth.split('/').next()?.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

async fn resolve_github_client_for_repo(
    settings_service: &acpms_services::SystemSettingsService,
    repository_url: &str,
) -> Result<(acpms_github::GitHubClient, String, String), String> {
    let pat = settings_service
        .get_pat_for_repo(repository_url)
        .await
        .map_err(|e| format!("Failed to resolve GitHub credentials: {}", e))?
        .unwrap_or_default();
    if pat.trim().is_empty() {
        return Err("GitHub PAT not available for this repository host.".to_string());
    }

    let settings = settings_service
        .get()
        .await
        .map_err(|e| format!("Failed to load system settings: {}", e))?;

    let client_base_url =
        if parse_host_from_urlish(&settings.gitlab_url) == parse_host_from_urlish(repository_url) {
            settings.gitlab_url
        } else {
            "https://github.com".to_string()
        };

    let (_, repo_path) = parse_repo_host_and_path(repository_url)
        .ok_or_else(|| "Could not parse target GitHub repository URL.".to_string())?;
    let (owner, repo) = repo_path.split_once('/').ok_or_else(|| {
        "Could not parse owner/repo from target GitHub repository URL.".to_string()
    })?;

    let client = acpms_github::GitHubClient::new(&client_base_url, &pat)
        .map_err(|e| format!("Failed to initialize GitHub client: {}", e))?;

    Ok((client, owner.to_string(), repo.to_string()))
}

#[derive(Debug, Clone)]
struct ArchitectureNodeRef {
    id: String,
    label: String,
    node_type: String,
}

fn task_is_architecture_change(task: &Task) -> bool {
    task.metadata
        .get("source")
        .and_then(|value| value.as_str())
        .map(|value| value.eq_ignore_ascii_case("architecture_change"))
        .unwrap_or(false)
}

fn extract_architecture_nodes(config: Option<&serde_json::Value>) -> Vec<ArchitectureNodeRef> {
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

            let label = node
                .get("label")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(id);

            let node_type = node
                .get("type")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("service");

            Some(ArchitectureNodeRef {
                id: id.to_string(),
                label: label.to_string(),
                node_type: node_type.to_ascii_lowercase(),
            })
        })
        .collect()
}

fn format_arch_node(node: &ArchitectureNodeRef) -> String {
    format!("{} [{}] ({})", node.label, node.id, node.node_type)
}

fn parse_diff_list(task: &Task, key: &str) -> Vec<String> {
    task.metadata
        .get("architecture_diff")
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::trim))
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn is_backend_or_infra_node_type(node_type: &str) -> bool {
    matches!(
        node_type,
        "api"
            | "service"
            | "auth"
            | "gateway"
            | "worker"
            | "database"
            | "cache"
            | "queue"
            | "storage"
    )
}

fn build_architecture_change_instruction_block(task: &Task) -> String {
    if !task_is_architecture_change(task) {
        return String::new();
    }

    let old_nodes = extract_architecture_nodes(task.metadata.get("old_architecture"));
    let new_nodes = extract_architecture_nodes(task.metadata.get("new_architecture"));
    let old_ids: HashSet<&str> = old_nodes.iter().map(|node| node.id.as_str()).collect();

    let added_nodes: Vec<ArchitectureNodeRef> = new_nodes
        .into_iter()
        .filter(|node| !old_ids.contains(node.id.as_str()))
        .collect();

    let backend_added_nodes: Vec<ArchitectureNodeRef> = added_nodes
        .iter()
        .filter(|node| is_backend_or_infra_node_type(&node.node_type))
        .cloned()
        .collect();

    let added_edges = parse_diff_list(task, "addedEdges");
    let removed_nodes = parse_diff_list(task, "removedNodes");

    let mut lines: Vec<String> = Vec::new();
    lines.push("## Architecture Alignment Rules (Required)".to_string());
    lines.push(
        "This task was generated from architecture edits. Prioritize architecture delta implementation over unrelated extension/UI tweaks.".to_string(),
    );

    if !added_nodes.is_empty() {
        lines.push(format!(
            "- Added components to implement: {}",
            added_nodes
                .iter()
                .map(format_arch_node)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if !backend_added_nodes.is_empty() {
        lines.push(format!(
            "- Added backend/infrastructure components: {}",
            backend_added_nodes
                .iter()
                .map(format_arch_node)
                .collect::<Vec<_>>()
                .join(", ")
        ));
        lines.push("- You MUST create/update concrete backend code paths for these components (new module/service/routes/config/tests as needed), not only extension UI files.".to_string());
        lines.push(
            "- If repository currently lacks backend runtime, scaffold a minimal runnable API/service surface that matches the new component boundary."
                .to_string(),
        );
    }

    if !added_edges.is_empty() {
        lines.push(format!(
            "- New integrations to wire in code: {}",
            added_edges.join(", ")
        ));
        lines.push(
            "- Update interfaces/contracts between connected components (API handlers, clients, service calls, auth flow, data access) to match these links."
                .to_string(),
        );
    }

    if !removed_nodes.is_empty() {
        lines.push(format!(
            "- Removed components to deprecate/decouple carefully: {}",
            removed_nodes.join(", ")
        ));
    }

    lines.push(
        "- Keep unrelated files unchanged. Only touch extension/frontend files when required to integrate with the new architecture."
            .to_string(),
    );
    lines.push(
        "- Final summary MUST include mapping: each added/changed architecture component -> created/updated files."
            .to_string(),
    );

    format!("\n\n{}\n", lines.join("\n"))
}

/// Prepend preferred-language instruction when set in system settings.
fn prepend_language_instruction(instruction: String, preferred_language: Option<&str>) -> String {
    let line = match preferred_language
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        Some("vi") => "Always respond in Vietnamese.\n\n",
        Some("en") => "Always respond in English.\n\n",
        _ => return instruction,
    };
    format!("{}{}", line, instruction)
}

fn build_attempt_instruction(
    task: &Task,
    skill_context: &SkillInstructionContext,
    require_review: bool,
) -> String {
    let run_build_and_tests = task_run_build_and_tests(task);
    let description = task.description.as_deref().unwrap_or_default();
    let architecture_rule_block = build_architecture_change_instruction_block(task);

    let verification_rule = if run_build_and_tests {
        "2. Run verification after changes (build/lint/tests where applicable)."
    } else {
        "2. Keep changes focused and lightweight. Skip expensive build/test runs unless absolutely necessary, then report clearly."
    };

    let finalize_rule = if require_review {
        "3. Do NOT commit or push changes. Prepare handoff notes for human review."
    } else {
        "3. Stage only changed files, commit with descriptive message, and push branch."
    };

    format!(
        r#"## Task
{}

## Description
{}

## Execution Rules
1. Implement only what this task requires.
{}
{}
4. End with a clear summary of what was validated and what remains risky.{}{}"#,
        task.title,
        description,
        verification_rule,
        finalize_rule,
        architecture_rule_block,
        skill_context.block
    )
}

fn parse_job_priority(priority: &str) -> JobPriority {
    match priority.trim().to_ascii_lowercase().as_str() {
        "high" => JobPriority::High,
        "low" => JobPriority::Low,
        _ => JobPriority::Normal,
    }
}

fn resolve_project_repo_path(project: &Project) -> PathBuf {
    let worktrees_base = std::env::var("WORKTREES_PATH").unwrap_or_else(|_| {
        std::env::var("HOME")
            .ok()
            .map(|h| format!("{}/Projects", h.trim_end_matches('/')))
            .unwrap_or_else(|| "./worktrees".to_string())
    });
    PathBuf::from(worktrees_base).join(project_repo_relative_path(
        project.id,
        &project.metadata,
        &project.name,
    ))
}

fn task_status_to_metadata_value(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Backlog => "backlog",
        TaskStatus::Todo => "todo",
        TaskStatus::InProgress => "in_progress",
        TaskStatus::InReview => "in_review",
        TaskStatus::Blocked => "blocked",
        TaskStatus::Done => "done",
        TaskStatus::Archived => "archived",
    }
}

fn task_status_from_metadata_value(value: &str) -> Option<TaskStatus> {
    match value {
        "backlog" => Some(TaskStatus::Backlog),
        "todo" => Some(TaskStatus::Todo),
        "in_progress" => Some(TaskStatus::InProgress),
        "in_review" => Some(TaskStatus::InReview),
        "blocked" => Some(TaskStatus::Blocked),
        "done" => Some(TaskStatus::Done),
        "archived" => Some(TaskStatus::Archived),
        _ => None,
    }
}

fn revert_task_status_from_previous_attempt_status(value: &str) -> Option<TaskStatus> {
    match value {
        "success" => Some(TaskStatus::Done),
        "failed" => Some(TaskStatus::InReview),
        "cancelled" => Some(TaskStatus::InReview),
        _ => None,
    }
}

async fn resolve_revert_task_status(
    pool: &sqlx::PgPool,
    task_id: Uuid,
    attempt_id: Uuid,
    attempt_metadata: &serde_json::Value,
) -> Result<TaskStatus, sqlx::Error> {
    if let Some(status) = attempt_metadata
        .get("previous_task_status")
        .and_then(|v| v.as_str())
        .and_then(task_status_from_metadata_value)
    {
        return Ok(status);
    }

    let previous_attempt_status = sqlx::query_scalar::<_, String>(
        r#"
        SELECT status::text
        FROM task_attempts
        WHERE task_id = $1 AND id != $2 AND status IN ('success', 'failed', 'cancelled')
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(task_id)
    .bind(attempt_id)
    .fetch_optional(pool)
    .await?;

    Ok(previous_attempt_status
        .as_deref()
        .and_then(revert_task_status_from_previous_attempt_status)
        .unwrap_or(TaskStatus::Todo))
}

async fn mark_submission_failed(
    attempt_service: &TaskAttemptService,
    task_service: &TaskService,
    attempt_id: Uuid,
    task_id: Uuid,
    revert_status: TaskStatus,
    reason: String,
) {
    if let Err(error) = attempt_service
        .update_status(attempt_id, AttemptStatus::Failed, Some(reason.clone()))
        .await
    {
        tracing::warn!(
            attempt_id = %attempt_id,
            error = %error,
            "Failed to mark attempt as failed after submission error"
        );
    }

    if let Err(error) = task_service
        .update_task_status(task_id, revert_status)
        .await
    {
        tracing::warn!(
            attempt_id = %attempt_id,
            task_id = %task_id,
            error = %error,
            "Failed to revert task status after submission error"
        );
    }
}

async fn create_execution_process_record(
    pool: &sqlx::PgPool,
    attempt_id: Uuid,
    worktree_path: Option<&std::path::Path>,
    branch_name: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    let worktree_path = worktree_path.map(|path| path.to_string_lossy().to_string());

    sqlx::query_scalar(
        r#"
        INSERT INTO execution_processes (attempt_id, process_id, worktree_path, branch_name)
        VALUES ($1, NULL, $2, $3)
        RETURNING id
        "#,
    )
    .bind(attempt_id)
    .bind(worktree_path)
    .bind(branch_name)
    .fetch_one(pool)
    .await
}

async fn fetch_execution_process_by_id(
    pool: &sqlx::PgPool,
    execution_process_id: Uuid,
) -> Result<Option<DbExecutionProcess>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, attempt_id, process_id, worktree_path, branch_name, created_at
        FROM execution_processes
        WHERE id = $1
        "#,
    )
    .bind(execution_process_id)
    .fetch_optional(pool)
    .await
}

async fn fetch_latest_execution_process_for_attempt(
    pool: &sqlx::PgPool,
    attempt_id: Uuid,
) -> Result<Option<DbExecutionProcess>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, attempt_id, process_id, worktree_path, branch_name, created_at
        FROM execution_processes
        WHERE attempt_id = $1
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(attempt_id)
    .fetch_optional(pool)
    .await
}

async fn cleanup_execution_process_record(pool: &sqlx::PgPool, execution_process_id: Uuid) {
    if let Err(error) = sqlx::query("DELETE FROM execution_processes WHERE id = $1")
        .bind(execution_process_id)
        .execute(pool)
        .await
    {
        tracing::warn!(
            execution_process_id = %execution_process_id,
            error = %error,
            "Failed to cleanup execution process record after submission failure"
        );
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AttemptExecutionProcessDto {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub process_id: Option<i32>,
    pub worktree_path: Option<String>,
    pub branch_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<DbExecutionProcess> for AttemptExecutionProcessDto {
    fn from(value: DbExecutionProcess) -> Self {
        Self {
            id: value.id,
            attempt_id: value.attempt_id,
            process_id: value.process_id,
            worktree_path: value.worktree_path,
            branch_name: value.branch_name,
            created_at: value.created_at,
        }
    }
}

fn extract_resolved_skill_chain(metadata: &serde_json::Value) -> Option<Vec<String>> {
    let chain = metadata
        .get("resolved_skill_chain")
        .and_then(|value| value.as_array())?;
    let skills: Vec<String> = chain
        .iter()
        .filter_map(|value| value.as_str().map(str::trim))
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect();

    if skills.is_empty() {
        None
    } else {
        Some(skills)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredKnowledgeSuggestions {
    status: SkillKnowledgeStatus,
    detail: Option<String>,
    items: Vec<SuggestedSkill>,
}

fn extract_knowledge_suggestions(
    metadata: &serde_json::Value,
) -> Option<StoredKnowledgeSuggestions> {
    metadata
        .get("knowledge_suggestions")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

async fn persist_skill_instruction_context_metadata(
    pool: &sqlx::PgPool,
    attempt_id: Uuid,
    context: &SkillInstructionContext,
    source: &str,
) -> Result<(), sqlx::Error> {
    let patch = build_skill_metadata_patch(context, source);
    sqlx::query(
        r#"
        UPDATE task_attempts
        SET metadata = COALESCE(metadata, '{}'::jsonb) || $2::jsonb
        WHERE id = $1
        "#,
    )
    .bind(attempt_id)
    .bind(patch)
    .execute(pool)
    .await?;

    Ok(())
}

async fn append_skill_timeline_log(
    pool: &sqlx::PgPool,
    broadcast_tx: &tokio::sync::broadcast::Sender<AgentEvent>,
    attempt_id: Uuid,
    context: &SkillInstructionContext,
) -> anyhow::Result<()> {
    let message = format_loaded_skills_log_line(context);
    StatusManager::log(pool, broadcast_tx, attempt_id, "system", &message).await
}

fn skill_knowledge_status_label(status: &SkillKnowledgeStatus) -> &'static str {
    match status {
        SkillKnowledgeStatus::Disabled => "disabled",
        SkillKnowledgeStatus::Pending => "pending",
        SkillKnowledgeStatus::Ready => "ready",
        SkillKnowledgeStatus::Failed => "failed",
        SkillKnowledgeStatus::NoMatches => "no_matches",
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AttemptKnowledgeSuggestionDto {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub score: f32,
    pub source_path: String,
    pub origin: String,
}

impl From<SuggestedSkill> for AttemptKnowledgeSuggestionDto {
    fn from(value: SuggestedSkill) -> Self {
        Self {
            skill_id: value.skill_id,
            name: value.name,
            description: value.description,
            score: value.score,
            source_path: value.source_path,
            origin: value.origin,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AttemptKnowledgeSuggestionsDto {
    pub status: String,
    #[schema(nullable = true)]
    pub detail: Option<String>,
    pub items: Vec<AttemptKnowledgeSuggestionDto>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AttemptSkillsDto {
    pub attempt_id: Uuid,
    pub task_id: Uuid,
    #[schema(value_type = Vec<String>)]
    pub resolved_skill_chain: Vec<String>,
    pub source: String,
    pub knowledge_suggestions: AttemptKnowledgeSuggestionsDto,
}

#[utoipa::path(
    post,
    path = "/api/v1/tasks/{task_id}/attempts",
    tag = "Task Attempts",
    params(
        ("task_id" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 201, description = "Task attempt created", body = TaskAttemptResponse),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task not found")
    )
)]
pub async fn create_task_attempt(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<(StatusCode, Json<ApiResponse<TaskAttemptDto>>)> {
    let pool = state.db.clone();
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ExecuteTask,
        &pool,
    )
    .await?;

    // Resolve project and access mode before creating an attempt so read-only imports fail early.
    let project_service = ProjectService::new(pool.clone());
    let project = project_service
        .get_project(task.project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;
    let project = maybe_autorecheck_legacy_repository_context(
        &state,
        &project_service,
        project,
        "attempt_create",
    )
    .await;

    if task.task_type != TaskType::Init
        && repository_mode_blocks_coding_attempt(&project.repository_context)
        && !task_allows_analysis_only_attempt(&task)
    {
        state
            .metrics
            .repository_attempt_blocks_total
            .with_label_values(&[
                repository_access_mode_label(project.repository_context.access_mode),
                repository_verification_status_label(
                    project.repository_context.verification_status,
                ),
            ])
            .inc();
        let mode = repository_access_mode_label(project.repository_context.access_mode);
        let guidance = if project.repository_context.access_mode
            == RepositoryAccessMode::AnalysisOnly
        {
            "This imported repository is analysis-only. Link or create a writable fork before starting coding tasks."
        } else {
            "Repository access has not been verified yet. Re-check access or configure credentials before starting coding tasks."
        };

        return Err(ApiError::Conflict(format!(
            "Project repository is not writable for coding attempts (mode: {}). {}",
            mode, guidance
        )));
    }

    let previous_task_status = task.status;
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .create_attempt_with_metadata(
            task_id,
            serde_json::json!({
                "previous_task_status": task_status_to_metadata_value(previous_task_status),
            }),
        )
        .await
        .map_err(|e| {
            let message = e.to_string();
            if message.contains("already has an active attempt") {
                ApiError::BadRequest(message)
            } else {
                ApiError::Internal(message)
            }
        })?;

    // Update task status to InProgress
    if let Err(error) = task_service
        .update_task_status(task_id, TaskStatus::InProgress)
        .await
    {
        let message = format!("Failed to update task status: {}", error);
        let _ = attempt_service
            .update_status(attempt.id, AttemptStatus::Failed, Some(message.clone()))
            .await;
        return Err(ApiError::Internal(message));
    }
    openclaw::emit_task_status_changed(
        &state,
        task.project_id,
        task_id,
        previous_task_status,
        TaskStatus::InProgress,
        "routes.task_attempts.create_task_attempt",
    )
    .await;

    // Get project settings for timeout, retry config
    let settings = project_service
        .get_settings(task.project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let repo_path = resolve_project_repo_path(&project);

    let attempt_id = attempt.id;
    let skill_knowledge = state.orchestrator.skill_knowledge();
    let skill_context = build_skill_instruction_context(
        &task,
        &settings,
        project.project_type,
        Some(repo_path.as_path()),
        Some(&skill_knowledge),
    );
    if let Err(error) = persist_skill_instruction_context_metadata(
        &pool,
        attempt_id,
        &skill_context,
        "attempt_create",
    )
    .await
    {
        tracing::warn!(
            attempt_id = %attempt_id,
            error = %error,
            "Failed to persist skill instruction metadata during attempt creation"
        );
    }
    if task.task_type != TaskType::Init {
        if let Err(error) =
            append_skill_timeline_log(&pool, &state.broadcast_tx, attempt_id, &skill_context).await
        {
            tracing::warn!(
                attempt_id = %attempt_id,
                error = %error,
                "Failed to append skill timeline log during attempt creation"
            );
        }
    }

    let require_review = task_require_review(&task, &settings);
    let mut instruction = build_attempt_instruction(&task, &skill_context, require_review);
    let preferred_settings = state.settings_service.get().await.ok();
    let preferred_lang = preferred_settings
        .as_ref()
        .and_then(|s| s.preferred_agent_language.as_deref());
    instruction = prepend_language_instruction(instruction, preferred_lang);

    // Create a process record for this execution run (attempt may have multiple runs via resume).
    let execution_process_id =
        create_execution_process_record(&pool, attempt_id, Some(repo_path.as_path()), None)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to create execution process record: {}", e))
            })?;

    // Init tasks must use direct execution (not worker pool) because they have special routing
    // Worker pool only handles regular tasks (Feature/Bug/Refactor)
    if task.task_type == TaskType::Init {
        // Direct execution for Init tasks (routes to execute_init_task -> execute_from_scratch)
        // Pass the API-created attempt_id to avoid duplicate attempt creation
        let orchestrator = state.orchestrator.clone();
        tokio::spawn(async move {
            if let Err(e) = orchestrator
                .execute_task_with_attempt(task_id, Some(attempt_id))
                .await
            {
                tracing::error!("Init task execution failed for task {}: {:?}", task_id, e);
            }
        });
    } else if let Some(worker_pool) = &state.worker_pool {
        // Submit regular tasks to worker pool
        use acpms_executors::AgentJob;

        // Create job with project settings (timeout, retry config)
        let job = AgentJob::new(
            attempt_id,
            task.id,
            task.project_id,
            repo_path,
            instruction,
            require_review,
        )
        .with_timeout(settings.timeout_mins)
        .with_retry_config(settings.max_retries, settings.auto_retry)
        .with_project_max_concurrent(settings.max_concurrent)
        .with_priority(parse_job_priority(&settings.auto_execute_priority));

        if let Err(error) = worker_pool.submit(job).await {
            cleanup_execution_process_record(&pool, execution_process_id).await;
            let message = format!("Failed to submit job to worker pool: {}", error);
            mark_submission_failed(
                &attempt_service,
                &task_service,
                attempt_id,
                task.id,
                previous_task_status,
                message.clone(),
            )
            .await;
            return Err(ApiError::Internal(message));
        }
    } else {
        // Fallback to direct execution for regular tasks (without worker pool).
        // Keep the same attempt_id and review behavior as worker-pool execution.
        let orchestrator = state.orchestrator.clone();
        let task_id = task.id;
        let project_id = task.project_id;
        let instruction = instruction.clone();
        let repo_path = repo_path.clone();
        tokio::spawn(async move {
            let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
            if let Err(e) = orchestrator
                .execute_task_with_cancel_review(
                    attempt_id,
                    task_id,
                    repo_path,
                    instruction,
                    cancel_rx,
                    require_review,
                )
                .await
            {
                tracing::error!(
                    "Direct execution failed for attempt {} (task {}, project {}): {:?}",
                    attempt_id,
                    task_id,
                    project_id,
                    e
                );
            }
        });
    }

    let dto = TaskAttemptDto::from(attempt);
    let response = ApiResponse::created(dto, "Task attempt created and execution started");

    Ok((StatusCode::CREATED, Json(response)))
}

/// Create task attempt after editing task. Cleans up previous attempt's worktree and closes
/// open MR before creating new attempt. Used when user edits task (todo or in_review) with auto-start.
#[utoipa::path(
    post,
    path = "/api/v1/tasks/{task_id}/attempts/from-edit",
    tag = "Task Attempts",
    params(
        ("task_id" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 201, description = "Task attempt created after edit", body = TaskAttemptResponse),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task not found")
    )
)]
pub async fn create_task_attempt_from_edit(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<(StatusCode, Json<ApiResponse<TaskAttemptDto>>)> {
    let pool = state.db.clone();
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ExecuteTask,
        &pool,
    )
    .await?;

    // When task is in_review: cleanup previous attempt's worktree and close open MR
    if task.status == TaskStatus::InReview {
        let prev_attempt_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM task_attempts WHERE task_id = $1 ORDER BY created_at DESC LIMIT 1"#,
        )
        .bind(task_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch latest attempt: {}", e)))?;

        if let Some(prev_id) = prev_attempt_id {
            if let Err(e) = state.orchestrator.cleanup_worktree_public(prev_id).await {
                tracing::warn!(
                    prev_attempt_id = %prev_id,
                    error = %e,
                    "Cleanup worktree failed (non-fatal, continuing)"
                );
            }

            let mr_row: Option<(Option<i64>, Option<i64>, String, Option<i64>, Option<String>)> =
                sqlx::query_as(
                r#"
                SELECT gitlab_mr_iid, github_pr_number, status, target_project_id, target_repository_url
                FROM merge_requests
                WHERE attempt_id = $1 AND LOWER(status) NOT IN ('merged', 'closed')
                LIMIT 1
                "#,
                )
                .bind(prev_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to fetch MR/PR: {}", e)))?;

            if let Some((
                gitlab_iid,
                github_pr_number,
                _,
                target_project_id,
                target_repository_url,
            )) = mr_row
            {
                if let Some(mr_iid) = gitlab_iid {
                    if let Ok(Some(config)) = state.gitlab_service.get_config(task.project_id).await
                    {
                        if let Ok(client) = state.gitlab_service.get_client(task.project_id).await {
                            let gitlab_project_id =
                                target_project_id.unwrap_or(config.gitlab_project_id) as u64;
                            if let Err(e) = client
                                .close_merge_request(gitlab_project_id, mr_iid as u64)
                                .await
                            {
                                tracing::warn!(
                                    mr_iid = mr_iid,
                                    error = %e,
                                    "Close MR on GitLab failed (non-fatal)"
                                );
                            }
                        }
                    }
                    let _ = sqlx::query(
                        r#"UPDATE merge_requests SET status = 'closed', updated_at = NOW() WHERE attempt_id = $1 AND gitlab_mr_iid = $2"#,
                    )
                    .bind(prev_id)
                    .bind(mr_iid)
                    .execute(&pool)
                    .await;
                }
                if let (Some(pr_number), Some(target_repository_url)) =
                    (github_pr_number, target_repository_url.as_deref())
                {
                    match resolve_github_client_for_repo(
                        &state.settings_service,
                        target_repository_url,
                    )
                    .await
                    {
                        Ok((client, owner, repo)) => {
                            if let Err(e) = client
                                .close_pull_request(&owner, &repo, pr_number as u64)
                                .await
                            {
                                tracing::warn!(
                                    pr_number = pr_number,
                                    error = %e,
                                    "Close PR on GitHub failed (non-fatal)"
                                );
                            }
                        }
                        Err(error) => {
                            tracing::warn!(
                                pr_number = pr_number,
                                error = %error,
                                "Could not resolve GitHub client for closing PR"
                            );
                        }
                    }

                    let _ = sqlx::query(
                        r#"UPDATE merge_requests SET status = 'closed', updated_at = NOW() WHERE attempt_id = $1 AND github_pr_number = $2"#,
                    )
                    .bind(prev_id)
                    .bind(pr_number)
                    .execute(&pool)
                    .await;
                }
            }
        }
    }

    // Delegate to create_task_attempt logic (same flow)
    create_task_attempt(State(state), auth_user, Path(task_id)).await
}

#[utoipa::path(
    get,
    path = "/api/v1/tasks/{task_id}/attempts",
    tag = "Task Attempts",
    params(
        ("task_id" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "List task attempts", body = TaskAttemptListResponse),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_task_attempts(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<TaskAttemptDto>>>> {
    let pool = state.db.clone();
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    let attempt_service = TaskAttemptService::new(pool);
    let attempts = attempt_service
        .get_task_attempts(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dtos: Vec<TaskAttemptDto> = attempts.into_iter().map(TaskAttemptDto::from).collect();
    let response = ApiResponse::success(dtos, "Task attempts retrieved successfully");

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/attempts/{id}",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    responses(
        (status = 200, description = "Get attempt details", body = TaskAttemptResponse),
        (status = 404, description = "Attempt not found")
    )
)]
pub async fn get_attempt(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<TaskAttemptDto>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    let dto = TaskAttemptDto::from(attempt);
    let response = ApiResponse::success(dto, "Task attempt retrieved successfully");

    Ok(Json(response))
}

pub async fn get_attempt_skills(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<AttemptSkillsDto>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    let project_service = ProjectService::new(pool.clone());
    let project = project_service
        .get_project(task.project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;
    let settings = project_service
        .get_settings(task.project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let persisted_source = attempt
        .metadata
        .get("resolved_skill_chain_source")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    let existing_chain = extract_resolved_skill_chain(&attempt.metadata);
    let existing_knowledge = extract_knowledge_suggestions(&attempt.metadata);

    let (resolved_skill_chain, source, knowledge_suggestions) =
        if let (Some(skills), Some(knowledge)) = (existing_chain, existing_knowledge) {
            (
                skills,
                persisted_source.unwrap_or_else(|| "attempt_metadata".to_string()),
                AttemptKnowledgeSuggestionsDto {
                    status: skill_knowledge_status_label(&knowledge.status).to_string(),
                    detail: knowledge.detail,
                    items: knowledge.items.into_iter().map(Into::into).collect(),
                },
            )
        } else {
            let repo_path = project
                .repository_url
                .as_ref()
                .map(|_| resolve_project_repo_path(&project));
            let skill_knowledge = state.orchestrator.skill_knowledge();
            let context = build_skill_instruction_context(
                &task,
                &settings,
                project.project_type,
                repo_path.as_deref(),
                Some(&skill_knowledge),
            );
            let fallback_source = persisted_source
                .clone()
                .unwrap_or_else(|| "attempt_skills_read_fallback".to_string());
            if let Err(error) = persist_skill_instruction_context_metadata(
                &pool,
                attempt_id,
                &context,
                &fallback_source,
            )
            .await
            {
                tracing::warn!(
                    attempt_id = %attempt_id,
                    error = %error,
                    "Failed to persist fallback skill instruction metadata"
                );
            }
            (
                context.resolved_skill_chain,
                fallback_source,
                AttemptKnowledgeSuggestionsDto {
                    status: skill_knowledge_status_label(&context.knowledge_status).to_string(),
                    detail: context.knowledge_detail,
                    items: context
                        .suggested_skills
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                },
            )
        };

    let dto = AttemptSkillsDto {
        attempt_id,
        task_id: task.id,
        resolved_skill_chain,
        source,
        knowledge_suggestions,
    };
    let response = ApiResponse::success(dto, "Attempt skills retrieved successfully");
    Ok(Json(response))
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct AttemptLogsQuery {
    /// Maximum number of logs to return (1-200). If omitted, returns full attempt logs.
    pub limit: Option<i64>,
    /// Cursor timestamp (RFC3339). Returns logs older than this timestamp.
    pub before: Option<chrono::DateTime<chrono::Utc>>,
    /// UUID tie-breaker for stable pagination when multiple logs share the same timestamp.
    #[allow(dead_code)] // Reserved for cursor-based pagination
    pub before_id: Option<Uuid>,
}

#[utoipa::path(
    get,
    path = "/api/v1/attempts/{id}/logs",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID"),
        AttemptLogsQuery
    ),
    responses(
        (status = 200, description = "Get attempt logs", body = AgentLogListResponse),
        (status = 404, description = "Attempt not found")
    )
)]
pub async fn get_attempt_logs(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
    Query(query): Query<AttemptLogsQuery>,
) -> ApiResult<Json<ApiResponse<Vec<AgentLogDto>>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    // When no limit: return full logs (read up to 20MB). When limit specified: cap at 200 for pagination.
    let limit = match query.limit {
        None => 50_000, // Full logs: high cap for attempts without pagination
        Some(l) => (l as usize).min(200),
    };
    let max_bytes = match query.limit {
        None => 20_000_000, // 20MB for full read
        Some(_) => ((limit + 50) * 600).min(500_000),
    };

    // Prefer local JSONL when it has content; S3 is fallback. Use tail/head read to cap I/O.
    let bytes = if query.before.is_some() {
        // "before" cursor: need logs older than cursor, read from start (head)
        acpms_executors::read_attempt_log_file_head(attempt_id, max_bytes)
            .await
            .unwrap_or_default()
    } else {
        // No before: want most recent, read from end (tail)
        acpms_executors::read_attempt_log_file_tail(attempt_id, max_bytes)
            .await
            .unwrap_or_default()
    };

    let bytes = if bytes.is_empty() {
        // Local empty: fall back to S3 with same tail/head + pagination
        if let Some(ref s3_key) = attempt.s3_log_key {
            let result = if query.before.is_some() {
                state
                    .storage_service
                    .get_log_bytes_head(s3_key, max_bytes)
                    .await
            } else {
                state
                    .storage_service
                    .get_log_bytes_tail(s3_key, max_bytes)
                    .await
            };
            result.unwrap_or_else(|e| {
                tracing::warn!(
                    "Failed to load logs from S3 for attempt {}: {}",
                    attempt_id,
                    e
                );
                vec![]
            })
        } else {
            vec![]
        }
    } else {
        bytes
    };

    let logs: Vec<acpms_db::models::AgentLog> = if let Some(before) = query.before {
        // Filter created_at < before, take last `limit` (most recent of the "older" set)
        let parsed = acpms_executors::parse_jsonl_to_agent_logs(&bytes);
        let mut filtered: Vec<_> = parsed
            .into_iter()
            .filter(|l| l.created_at < before)
            .collect();
        filtered.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        let len = filtered.len();
        filtered
            .into_iter()
            .skip(len.saturating_sub(limit))
            .take(limit)
            .collect()
    } else {
        // Most recent `limit` entries
        acpms_executors::parse_jsonl_tail_to_agent_logs(&bytes, limit)
    };

    let dtos: Vec<AgentLogDto> = logs.into_iter().map(AgentLogDto::from).collect();
    let response = ApiResponse::success(dtos, "Attempt logs retrieved successfully");

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/attempts/{id}/processes",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    responses(
        (status = 200, description = "Get attempt execution processes", body = Vec<AttemptExecutionProcessDto>),
        (status = 404, description = "Attempt not found")
    )
)]
pub async fn get_attempt_execution_processes(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<AttemptExecutionProcessDto>>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Validate permission
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    let processes: Vec<DbExecutionProcess> = sqlx::query_as(
        r#"
        SELECT id, attempt_id, process_id, worktree_path, branch_name, created_at
        FROM execution_processes
        WHERE attempt_id = $1
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .bind(attempt_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch execution processes: {}", e)))?;

    let dtos = processes
        .into_iter()
        .map(AttemptExecutionProcessDto::from)
        .collect();
    let response = ApiResponse::success(dtos, "Execution processes retrieved successfully");
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/attempts/{id}/input",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    request_body = SendInputRequestDoc,
    responses(
        (status = 200, description = "Input sent successfully", body = EmptyResponse),
        (status = 404, description = "Attempt not found")
    )
)]
pub async fn send_attempt_input(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
    Json(payload): Json<SendInputRequest>,
) -> ApiResult<Json<ApiResponse<()>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Validate permission
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ExecuteTask,
        &pool,
    )
    .await?;

    // Send input via orchestrator
    let result: anyhow::Result<()> = state
        .orchestrator
        .send_input(attempt_id, &payload.input)
        .await;

    if let Err(e) = result {
        let message = e.to_string();
        if message.contains("Live input is not supported")
            || message.contains("No active session for attempt")
            || message.contains("Live input channel is closed")
        {
            return Err(ApiError::BadRequest(message));
        }
        return Err(ApiError::Internal(format!(
            "Failed to send input: {}",
            message
        )));
    }

    // Log user input so it appears in the timeline
    if let Err(e) = StatusManager::log(
        &pool,
        &state.broadcast_tx,
        attempt_id,
        "user",
        &payload.input,
    )
    .await
    {
        tracing::warn!("Failed to log user input: {}", e);
    }

    let response = ApiResponse::success((), "Input sent successfully");
    Ok(Json(response))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateLogRequest {
    pub content: String,
}

#[utoipa::path(
    patch,
    path = "/api/v1/attempts/{id}/logs/{log_id}",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID"),
        ("log_id" = Uuid, Path, description = "Log ID to update")
    ),
    request_body = UpdateLogRequest,
    responses(
        (status = 200, description = "Log updated successfully", body = ApiResponse<AgentLogDto>),
        (status = 404, description = "Attempt or log not found"),
        (status = 400, description = "Log is not editable (only user/stdin logs)")
    )
)]
pub async fn patch_attempt_log(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((attempt_id, log_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateLogRequest>,
) -> ApiResult<Json<ApiResponse<AgentLogDto>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ExecuteTask,
        &pool,
    )
    .await?;

    let updated = attempt_service
        .update_log_content(log_id, attempt_id, &payload.content)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("not editable") {
                ApiError::BadRequest(msg)
            } else {
                ApiError::Internal(msg)
            }
        })?;

    let dto = AgentLogDto::from(updated);
    let response = ApiResponse::success(dto, "Log updated successfully");
    Ok(Json(response))
}

/// Request to resume a completed attempt with a follow-up prompt
#[derive(Debug, Deserialize, ToSchema)]
pub struct ResumeAttemptRequest {
    /// Follow-up prompt to continue the conversation
    pub prompt: String,
    /// Optional source execution process for process-scoped follow-up.
    #[serde(default)]
    pub source_execution_process_id: Option<Uuid>,
}

#[utoipa::path(
    post,
    path = "/api/v1/attempts/{id}/resume",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    request_body = ResumeAttemptRequest,
    responses(
        (status = 200, description = "Attempt resumed successfully", body = TaskAttemptResponse),
        (status = 400, description = "Cannot resume attempt in current state"),
        (status = 404, description = "Attempt not found")
    )
)]
/// Deprecated compatibility endpoint.
/// Use `/api/v1/execution-processes/{process_id}/follow-up` for process-scoped follow-up.
pub async fn resume_attempt(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
    Json(payload): Json<ResumeAttemptRequest>,
) -> ApiResult<Json<ApiResponse<TaskAttemptDto>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task for permissions and context
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ExecuteTask,
        &pool,
    )
    .await?;

    // Only allow resuming completed/success attempts
    match attempt.status {
        AttemptStatus::Success => {}                           // OK to resume
        AttemptStatus::Failed | AttemptStatus::Cancelled => {} // Also allow resuming failed
        _ => {
            return Err(ApiError::BadRequest(format!(
                "Cannot resume attempt in '{:?}' state. Only completed, failed, or cancelled attempts can be resumed.",
                attempt.status
            )));
        }
    }

    let resume_source_process = if let Some(source_process_id) = payload.source_execution_process_id
    {
        let process = fetch_execution_process_by_id(&pool, source_process_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound("Execution process not found".to_string()))?;
        if process.attempt_id != attempt_id {
            return Err(ApiError::BadRequest(
                "Execution process does not belong to the requested attempt".to_string(),
            ));
        }
        process
    } else {
        fetch_latest_execution_process_for_attempt(&pool, attempt_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| {
                ApiError::BadRequest(
                    "Cannot resume attempt without execution process context".to_string(),
                )
            })?
    };

    let project_service = ProjectService::new(pool.clone());
    let project = project_service
        .get_project(task.project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;
    let project = maybe_autorecheck_legacy_repository_context(
        &state,
        &project_service,
        project,
        "attempt_follow_up",
    )
    .await;

    let settings = project_service
        .get_settings(task.project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let require_review = task_require_review(&task, &settings);

    if task.task_type != TaskType::Init
        && repository_mode_blocks_coding_attempt(&project.repository_context)
        && !task_allows_analysis_only_attempt(&task)
    {
        return Err(ApiError::Conflict(format!(
            "Project repository is not writable for coding follow-up attempts (mode: {}). Re-check repository access before continuing.",
            repository_access_mode_label(project.repository_context.access_mode)
        )));
    }

    // Build follow-up instruction combining task context + user prompt.
    // Wrap trivial messages (e.g. "Hi", "ok") to avoid full context re-run and token waste.
    let task_desc = task.description.as_deref().unwrap_or(&task.title);
    let wrapped_prompt = acpms_executors::follow_up_utils::wrap_trivial_follow_up(&payload.prompt);
    let mut instruction = format!(
        r#"## Previous Context
You previously worked on this task:
{}

## Follow-up Request
{}

Continue working on the same task. Build on your previous work."#,
        task_desc, wrapped_prompt
    );
    let preferred_settings = state.settings_service.get().await.ok();
    let preferred_lang = preferred_settings
        .as_ref()
        .and_then(|s| s.preferred_agent_language.as_deref());
    instruction = prepend_language_instruction(instruction, preferred_lang);

    let original_task_status = task.status;
    let should_create_new_attempt = task_follow_up_creates_new_attempt(&task);

    // Resolve base repository path for the project.
    // Do NOT use attempt.worktree_path here because completed attempts may have cleaned worktrees.
    let repo_path = resolve_project_repo_path(&project);

    if should_create_new_attempt {
        let new_attempt_metadata = serde_json::json!({
            "follow_up": true,
            "manual_follow_up": true,
            "previous_attempt_id": attempt_id.to_string(),
            "source_execution_process_id": resume_source_process.id.to_string(),
            "previous_task_status": task_status_to_metadata_value(original_task_status),
        });

        let new_attempt = attempt_service
            .create_attempt_with_status_and_metadata(
                task.id,
                AttemptStatus::Queued,
                new_attempt_metadata,
            )
            .await
            .map_err(|e| {
                let message = e.to_string();
                if message.contains("already has an active attempt") {
                    ApiError::BadRequest(message)
                } else {
                    ApiError::Internal(format!("Failed to create follow-up attempt: {}", message))
                }
            })?;

        sqlx::query(
            r#"
            UPDATE task_attempts
            SET metadata = metadata || jsonb_build_object(
                'follow_up_attempt_id', $2::text,
                'next_attempt_id', $2::text
            )
            WHERE id = $1
            "#,
        )
        .bind(attempt_id)
        .bind(new_attempt.id.to_string())
        .execute(&pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to link follow-up attempt: {}", e)))?;

        if let Err(e) = StatusManager::log(
            &pool,
            &state.broadcast_tx,
            new_attempt.id,
            "user",
            &payload.prompt,
        )
        .await
        {
            tracing::warn!("Failed to log user follow-up on new attempt: {}", e);
        }

        if let Err(e) = StatusManager::log(
            &pool,
            &state.broadcast_tx,
            attempt_id,
            "system",
            &format!(
                "This follow-up will continue in a new attempt: {}",
                new_attempt.id
            ),
        )
        .await
        {
            tracing::warn!("Failed to log follow-up attempt linkage: {}", e);
        }

        if let Err(error) = task_service
            .update_task_status(task.id, TaskStatus::InProgress)
            .await
        {
            let message = format!("Failed to update task status: {}", error);
            let _ = attempt_service
                .update_status(new_attempt.id, AttemptStatus::Failed, Some(message.clone()))
                .await;
            return Err(ApiError::Internal(message));
        }
        openclaw::emit_task_status_changed(
            &state,
            task.project_id,
            task.id,
            task.status,
            TaskStatus::InProgress,
            "routes.task_attempts.create_task_attempt_from_edit.new_attempt",
        )
        .await;

        let execution_process_id =
            create_execution_process_record(&pool, new_attempt.id, Some(repo_path.as_path()), None)
                .await
                .map_err(|e| {
                    ApiError::Internal(format!("Failed to create execution process record: {}", e))
                })?;

        if task.task_type == TaskType::Init {
            let orchestrator = state.orchestrator.clone();
            let new_attempt_id = new_attempt.id;
            let repo_path = repo_path.clone();
            tokio::spawn(async move {
                if let Err(e) = orchestrator
                    .execute_agent_for_attempt(new_attempt_id, &repo_path, &instruction)
                    .await
                {
                    tracing::error!(
                        "Follow-up execution failed for new attempt {}: {:?}",
                        new_attempt_id,
                        e
                    );
                }
            });
        } else if let Some(worker_pool) = &state.worker_pool {
            use acpms_executors::AgentJob;

            let job = AgentJob::new(
                new_attempt.id,
                task.id,
                task.project_id,
                repo_path.clone(),
                instruction.clone(),
                require_review,
            )
            .with_timeout(settings.timeout_mins)
            .with_retry_config(settings.max_retries, settings.auto_retry)
            .with_project_max_concurrent(settings.max_concurrent)
            .with_priority(JobPriority::High);

            if let Err(error) = worker_pool.submit(job).await {
                cleanup_execution_process_record(&pool, execution_process_id).await;
                let message = format!("Failed to submit follow-up job: {}", error);
                mark_submission_failed(
                    &attempt_service,
                    &task_service,
                    new_attempt.id,
                    task.id,
                    original_task_status,
                    message.clone(),
                )
                .await;
                return Err(ApiError::Internal(message));
            }
        } else {
            let orchestrator = state.orchestrator.clone();
            let new_attempt_id = new_attempt.id;
            let task_id = task.id;
            let project_id = task.project_id;
            let instruction = instruction.clone();
            let repo_path = repo_path.clone();
            tokio::spawn(async move {
                let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
                if let Err(e) = orchestrator
                    .execute_task_with_cancel_review(
                        new_attempt_id,
                        task_id,
                        repo_path,
                        instruction,
                        cancel_rx,
                        require_review,
                    )
                    .await
                {
                    tracing::error!(
                        "Direct follow-up execution failed for attempt {} (task {}, project {}): {:?}",
                        new_attempt_id,
                        task_id,
                        project_id,
                        e
                    );
                }
            });
        }

        let dto = TaskAttemptDto::from(new_attempt);
        let response = ApiResponse::success(dto, "Follow-up started in a new attempt");
        return Ok(Json(response));
    }

    // Log user follow-up message so it appears in the timeline
    if let Err(e) = StatusManager::log(
        &pool,
        &state.broadcast_tx,
        attempt_id,
        "user",
        &payload.prompt,
    )
    .await
    {
        tracing::warn!("Failed to log user follow-up: {}", e);
    }

    sqlx::query(
        r#"
        UPDATE task_attempts
        SET metadata = metadata || jsonb_build_object('previous_task_status', $2::text)
        WHERE id = $1
        "#,
    )
    .bind(attempt_id)
    .bind(task_status_to_metadata_value(original_task_status))
    .execute(&pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to persist previous task status: {}", e)))?;

    let original_attempt_status = attempt.status;
    let original_attempt_error = attempt.error_message.clone();

    // Reset attempt status to Running (clear completed_at)
    attempt_service
        .update_status(attempt_id, AttemptStatus::Running, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update attempt status: {}", e)))?;

    // Update task status to InProgress
    if let Err(error) = task_service
        .update_task_status(attempt.task_id, TaskStatus::InProgress)
        .await
    {
        let message = format!("Failed to update task status: {}", error);
        let _ = attempt_service
            .update_status(
                attempt_id,
                original_attempt_status,
                original_attempt_error.clone(),
            )
            .await;
        return Err(ApiError::Internal(message));
    }
    openclaw::emit_task_status_changed(
        &state,
        task.project_id,
        attempt.task_id,
        original_task_status,
        TaskStatus::InProgress,
        "routes.task_attempts.send_attempt_input.resume_attempt",
    )
    .await;

    // Broadcast running status so SSE/WebSocket consumers update immediately.
    let _ = state.broadcast_tx.send(AgentEvent::Status(StatusMessage {
        attempt_id,
        status: AttemptStatus::Running,
        timestamp: Utc::now(),
    }));

    let resume_repo_path = resume_source_process
        .worktree_path
        .as_deref()
        .map(std::path::PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(repo_path);
    let resume_branch_name = resume_source_process.branch_name.as_deref();

    // Create a process record for the follow-up run on this existing attempt.
    let execution_process_id = create_execution_process_record(
        &pool,
        attempt_id,
        Some(resume_repo_path.as_path()),
        resume_branch_name,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create execution process record: {}", e)))?;

    // Spawn agent execution in background
    if task.task_type == TaskType::Init {
        let orchestrator = state.orchestrator.clone();
        let aid = attempt_id;
        let repo_path = resume_repo_path.clone();
        tokio::spawn(async move {
            // For init tasks, use execute_from_scratch which reuses the worktree
            if let Err(e) = orchestrator
                .execute_agent_for_attempt(aid, &repo_path, &instruction)
                .await
            {
                tracing::error!("Resume agent execution failed for attempt {}: {:?}", aid, e);
            }
        });
    } else if let Some(worker_pool) = &state.worker_pool {
        use acpms_executors::AgentJob;

        let job = AgentJob::new(
            attempt_id,
            task.id,
            task.project_id,
            resume_repo_path,
            instruction,
            require_review,
        )
        .with_timeout(settings.timeout_mins)
        .with_retry_config(settings.max_retries, settings.auto_retry)
        .with_project_max_concurrent(settings.max_concurrent)
        .with_priority(JobPriority::High);

        if let Err(error) = worker_pool.submit(job).await {
            cleanup_execution_process_record(&pool, execution_process_id).await;
            let message = format!("Failed to submit resume job: {}", error);
            let _ = attempt_service
                .update_status(
                    attempt_id,
                    original_attempt_status,
                    original_attempt_error.clone(),
                )
                .await;
            let _ = task_service
                .update_task_status(task.id, original_task_status)
                .await;
            openclaw::emit_task_status_changed(
                &state,
                task.project_id,
                task.id,
                TaskStatus::InProgress,
                original_task_status,
                "routes.task_attempts.send_attempt_input.resume_revert",
            )
            .await;
            return Err(ApiError::Internal(message));
        }
    } else {
        let orchestrator = state.orchestrator.clone();
        let aid = attempt_id;
        let repo_path = resume_repo_path.clone();
        tokio::spawn(async move {
            if let Err(e) = orchestrator
                .execute_agent_for_attempt(aid, &repo_path, &instruction)
                .await
            {
                tracing::error!("Resume agent execution failed for attempt {}: {:?}", aid, e);
            }
        });
    }

    // Return updated attempt
    let updated = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::Internal("Failed to fetch updated attempt".to_string()))?;

    let dto = TaskAttemptDto::from(updated);
    let response = ApiResponse::success(dto, "Attempt resumed with follow-up");
    Ok(Json(response))
}

/// Request to cancel an attempt with optional reason
#[derive(Debug, Deserialize, ToSchema)]
pub struct CancelAttemptRequest {
    /// Reason for cancellation (optional)
    pub reason: Option<String>,
    /// Force kill after graceful timeout (default: false)
    #[serde(default)]
    pub force: bool,
}

#[utoipa::path(
    post,
    path = "/api/v1/attempts/{id}/cancel",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    request_body = CancelAttemptRequest,
    responses(
        (status = 200, description = "Attempt cancelled successfully", body = EmptyResponse),
        (status = 404, description = "Attempt not found"),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Cannot cancel attempt in current state")
    )
)]
pub async fn cancel_attempt(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
    Json(payload): Json<CancelAttemptRequest>,
) -> ApiResult<Json<ApiResponse<()>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Validate permission via ExecuteTask (all project roles except viewer)
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ExecuteTask,
        &pool,
    )
    .await?;

    // Check if attempt is in a cancellable state
    if !matches!(
        attempt.status,
        AttemptStatus::Queued | AttemptStatus::Running
    ) {
        return Err(ApiError::BadRequest(format!(
            "Cannot cancel attempt in {} state",
            match attempt.status {
                AttemptStatus::Success => "success",
                AttemptStatus::Failed => "failed",
                AttemptStatus::Cancelled => "cancelled",
                _ => "unknown",
            }
        )));
    }

    let reason = payload
        .reason
        .unwrap_or_else(|| "Cancelled by user".to_string());

    // Use worker pool to cancel if available (sends signal to regular tasks)
    if let Some(worker_pool) = &state.worker_pool {
        if let Err(e) = worker_pool.cancel(attempt_id).await {
            let message = e.to_string();
            if message.contains("No active job found") {
                tracing::warn!(
                    "Cancel requested for attempt {} but no active worker job found (e.g. Init task); will try terminate_session",
                    attempt_id
                );
            } else {
                return Err(ApiError::Internal(format!(
                    "Failed to cancel attempt: {}",
                    message
                )));
            }
        }
    }

    // Terminate active session (kills process). Required for Init tasks (no worker job)
    // and as fallback for regular tasks if cancel signal hasn't been processed yet.
    if let Err(e) = state.orchestrator.terminate_session(attempt_id).await {
        tracing::warn!(
            "terminate_session for attempt {} returned: {} (may already be stopped)",
            attempt_id,
            e
        );
    }

    // Update attempt status with cancellation reason
    sqlx::query(
        r#"
        UPDATE task_attempts
        SET status = 'cancelled',
            error_message = $2,
            completed_at = NOW(),
            metadata = metadata || jsonb_build_object('cancelled_by', $3::text, 'force_kill', $4)
        WHERE id = $1
        "#,
    )
    .bind(attempt_id)
    .bind(&reason)
    .bind(auth_user.id.to_string())
    .bind(payload.force)
    .execute(&pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update attempt status: {}", e)))?;

    // Revert task status to what it was before this attempt started.
    // Prefer the explicit snapshot stored on attempt creation, then fall back
    // to previous terminal attempts for backward compatibility.
    let revert_status = resolve_revert_task_status(&pool, task.id, attempt_id, &attempt.metadata)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to resolve revert task status: {}", e)))?;

    sqlx::query("UPDATE tasks SET status = $2, updated_at = NOW() WHERE id = $1")
        .bind(task.id)
        .bind(revert_status)
        .execute(&pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to revert task status: {}", e)))?;
    openclaw::emit_task_status_changed(
        &state,
        task.project_id,
        task.id,
        task.status,
        revert_status,
        "routes.task_attempts.cancel_attempt.revert_status",
    )
    .await;

    // Broadcast status update so timeline/kanban can react without polling.
    let _ = state.broadcast_tx.send(AgentEvent::Status(StatusMessage {
        attempt_id,
        status: AttemptStatus::Cancelled,
        timestamp: Utc::now(),
    }));

    // Emit a system log entry for timeline visibility.
    if let Err(e) = StatusManager::log(
        &pool,
        &state.broadcast_tx,
        attempt_id,
        "system",
        &format!("Attempt cancelled: {}", reason),
    )
    .await
    {
        tracing::warn!("Failed to emit cancellation log for {}: {}", attempt_id, e);
    }

    // Always cleanup worktree after terminal cancellation. For force-kill requests,
    // capture diffs first when the worktree still exists.
    let orchestrator = state.orchestrator.clone();
    let storage = state.storage_service.clone();
    let pool_bg = pool.clone();
    let aid = attempt_id;
    let task_id = attempt.task_id;
    let capture_diffs_before_cleanup = payload.force;
    let worktree_path = attempt
        .metadata
        .get("worktree_path")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from);

    tokio::spawn(async move {
        if capture_diffs_before_cleanup {
            if let Some(worktree_path) = worktree_path.as_ref().filter(|path| path.exists()) {
                if let Ok(snapshot) = orchestrator
                    .collect_diffs_for_s3(aid, task_id, worktree_path)
                    .await
                {
                    let s3_key = acpms_executors::AttemptDiffSnapshot::generate_s3_key(
                        aid,
                        snapshot.saved_at,
                    );
                    let snapshot_size = snapshot.calculate_total_size();
                    if storage.upload_json(&s3_key, &snapshot).await.is_ok() {
                        let _ = sqlx::query(
                            "UPDATE task_attempts SET s3_diff_key = $1, s3_diff_size = $2, s3_diff_saved_at = $3 WHERE id = $4",
                        )
                        .bind(&s3_key)
                        .bind(snapshot_size)
                        .bind(snapshot.saved_at)
                        .bind(aid)
                        .execute(&pool_bg)
                        .await;
                        tracing::info!(
                            "Saved {} file diffs to S3 before cancel: {}",
                            snapshot.total_files,
                            s3_key
                        );
                    }
                }
            }
        }

        if let Err(e) = orchestrator.cleanup_worktree_public(aid).await {
            tracing::warn!("Worktree cleanup failed after cancel: {}", e);
        }
    });

    let response = ApiResponse::success((), format!("Attempt cancelled: {}", reason));
    Ok(Json(response))
}

// ============================================================================
// Review Flow Endpoints
// ============================================================================

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct FileDiff {
    /// Change type: "added", "deleted", "modified", "renamed"
    pub change: String,
    /// Original file path (null for new files)
    pub old_path: Option<String>,
    /// New file path
    pub new_path: Option<String>,
    /// Original file content (null for new files)
    pub old_content: Option<String>,
    /// New file content (null for deleted files)
    pub new_content: Option<String>,
    /// Lines added
    pub additions: i32,
    /// Lines deleted
    pub deletions: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DiffResponse {
    /// List of file diffs
    pub files: Vec<FileDiff>,
    /// Total files changed
    pub total_files: i32,
    /// Total lines added
    pub total_additions: i32,
    /// Total lines deleted
    pub total_deletions: i32,
}

async fn load_persisted_diff_response(
    state: &AppState,
    attempt_service: &TaskAttemptService,
    attempt_id: Uuid,
    s3_diff_key: Option<&String>,
) -> Result<Option<(DiffResponse, String)>, ApiError> {
    // Tier 1: S3 snapshot (preferred)
    if let Some(s3_key) = s3_diff_key {
        match state
            .storage_service
            .download_json::<acpms_executors::AttemptDiffSnapshot>(s3_key)
            .await
        {
            Ok(snapshot) => {
                let file_diffs: Vec<FileDiff> = snapshot
                    .files
                    .into_iter()
                    .map(|f| FileDiff {
                        change: f.change,
                        old_path: f.old_path,
                        new_path: Some(f.path),
                        old_content: f.old_content,
                        new_content: f.new_content,
                        additions: f.additions,
                        deletions: f.deletions,
                    })
                    .collect();

                return Ok(Some((
                    DiffResponse {
                        files: file_diffs,
                        total_files: snapshot.total_files as i32,
                        total_additions: snapshot.total_additions,
                        total_deletions: snapshot.total_deletions,
                    },
                    "Diff retrieved from S3 storage".to_string(),
                )));
            }
            Err(e) => {
                tracing::warn!("Failed to load diffs from S3 {}: {}", s3_key, e);
            }
        }
    }

    // Tier 2: Legacy DB fallback
    let saved_diffs = attempt_service
        .get_saved_diffs(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    if !saved_diffs.is_empty() {
        let file_diffs: Vec<FileDiff> = saved_diffs
            .into_iter()
            .map(|d: DbFileDiff| FileDiff {
                change: d.change_type,
                old_path: d.old_path,
                new_path: Some(d.file_path),
                old_content: d.old_content,
                new_content: d.new_content,
                additions: d.additions,
                deletions: d.deletions,
            })
            .collect();

        let total_files = file_diffs.len() as i32;
        let total_additions = file_diffs.iter().map(|f| f.additions).sum();
        let total_deletions = file_diffs.iter().map(|f| f.deletions).sum();

        return Ok(Some((
            DiffResponse {
                files: file_diffs,
                total_files,
                total_additions,
                total_deletions,
            },
            "Diff retrieved from database (legacy)".to_string(),
        )));
    }

    Ok(None)
}

fn no_diff_message_for_attempt_status(status: &AttemptStatus) -> &'static str {
    match status {
        AttemptStatus::Queued | AttemptStatus::Running => {
            "Attempt in progress. No code changes yet."
        }
        AttemptStatus::Failed => "Attempt failed before making code changes.",
        AttemptStatus::Cancelled => "Attempt was cancelled before code changes were persisted.",
        _ => "No code changes found for this attempt.",
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BranchStatus {
    pub branch_name: String,
    pub target_branch_name: String,
    pub ahead_count: i32,
    pub behind_count: i32,
    pub has_conflicts: bool,
    pub is_attempt_active: bool,
    pub can_push: bool,
    pub can_merge: bool,
    pub pr_url: Option<String>,
    pub pr_status: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApproveRequest {
    #[allow(dead_code)]
    pub commit_message: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/attempts/{id}/diff",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    responses(
        (status = 200, description = "Git diff retrieved", body = ApiResponse<DiffResponse>),
        (status = 404, description = "Attempt not found or worktree not available")
    )
)]
pub async fn get_attempt_diff(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<DiffResponse>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    let attempt_active = matches!(
        attempt.status,
        AttemptStatus::Queued | AttemptStatus::Running
    );

    // Get worktree path from attempt metadata
    let worktree_path = attempt
        .metadata
        .get("worktree_path")
        .and_then(|v| v.as_str());

    // Check if worktree directory exists
    let worktree_exists = worktree_path
        .map(|p| std::path::Path::new(p).exists())
        .unwrap_or(false);

    // Check if worktree is still a valid git repo.
    // A stale directory may still exist after cleanup, but without .git context.
    let worktree_git_ready = if worktree_exists {
        if let Some(path) = worktree_path {
            match tokio::process::Command::new("git")
                .current_dir(path)
                .args(["rev-parse", "--is-inside-work-tree"])
                .output()
                .await
            {
                Ok(output) => {
                    output.status.success()
                        && String::from_utf8_lossy(&output.stdout).trim() == "true"
                }
                Err(_) => false,
            }
        } else {
            false
        }
    } else {
        false
    };

    // For completed attempts, prefer persisted diffs.
    // If persisted snapshot is empty while worktree is still available, fall back to live
    // computation to recover older attempts captured before diff-base fixes.
    if !attempt_active {
        if let Some((diff, message)) = load_persisted_diff_response(
            &state,
            &attempt_service,
            attempt_id,
            attempt.s3_diff_key.as_ref(),
        )
        .await?
        {
            if diff.total_files > 0 || !worktree_git_ready {
                let response = ApiResponse::success(diff, &message);
                return Ok(Json(response));
            }
        }
    }

    // If worktree is unavailable/invalid, load persisted diffs: S3 -> DB -> empty payload.
    if !worktree_exists || !worktree_git_ready {
        if !worktree_exists {
            tracing::debug!(
                "Attempt {} worktree missing; loading persisted diff snapshot",
                attempt_id
            );
        } else {
            tracing::warn!(
                "Attempt {} worktree exists but is not a valid git repo; falling back to persisted diffs",
                attempt_id
            );
        }

        if let Some((diff, message)) = load_persisted_diff_response(
            &state,
            &attempt_service,
            attempt_id,
            attempt.s3_diff_key.as_ref(),
        )
        .await?
        {
            let response = ApiResponse::success(diff, &message);
            return Ok(Json(response));
        }

        let response = ApiResponse::success(
            DiffResponse {
                files: Vec::new(),
                total_files: 0,
                total_additions: 0,
                total_deletions: 0,
            },
            no_diff_message_for_attempt_status(&attempt.status),
        );
        return Ok(Json(response));
    }

    let worktree_path = worktree_path.ok_or_else(|| {
        ApiError::Internal("Worktree path missing despite git-ready state".to_string())
    })?;

    // Resolve diff base from attempt metadata.
    // `diff_base_commit` is a fixed checkpoint captured before execution.
    let base_branch = attempt
        .metadata
        .get("base_branch")
        .and_then(|v| v.as_str())
        .unwrap_or("main");
    let diff_base_ref = attempt
        .metadata
        .get("diff_base_commit")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(base_branch);
    let diff_range = format!("{}..HEAD", diff_base_ref);

    let mut file_diffs: Vec<FileDiff> = Vec::new();
    let mut total_additions = 0i32;
    let mut total_deletions = 0i32;
    let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Get list of changed files (tracked)
    let name_status = tokio::process::Command::new("git")
        .current_dir(worktree_path)
        .args(["diff", "--name-status", "--find-renames", &diff_range])
        .output()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get changed files: {}", e)))?;

    if !name_status.status.success() {
        tracing::warn!(
            "git diff --name-status failed for attempt {} at {}: {}",
            attempt_id,
            worktree_path,
            String::from_utf8_lossy(&name_status.stderr)
        );

        if let Some((diff, message)) = load_persisted_diff_response(
            &state,
            &attempt_service,
            attempt_id,
            attempt.s3_diff_key.as_ref(),
        )
        .await?
        {
            let response = ApiResponse::success(diff, &message);
            return Ok(Json(response));
        }

        let response = ApiResponse::success(
            DiffResponse {
                files: Vec::new(),
                total_files: 0,
                total_additions: 0,
                total_deletions: 0,
            },
            no_diff_message_for_attempt_status(&attempt.status),
        );
        return Ok(Json(response));
    }

    let name_status_str = String::from_utf8_lossy(&name_status.stdout);

    for line in name_status_str.lines() {
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split('\t').collect();
        if parts.is_empty() {
            continue;
        }

        let status = parts[0];
        let (change_type, old_path, new_path) = match status.chars().next() {
            Some('M') => (
                "modified",
                parts.get(1).map(|s| s.to_string()),
                parts.get(1).map(|s| s.to_string()),
            ),
            Some('A') => ("added", None, parts.get(1).map(|s| s.to_string())),
            Some('D') => ("deleted", parts.get(1).map(|s| s.to_string()), None),
            Some('R') => (
                "renamed",
                parts.get(1).map(|s| s.to_string()),
                parts.get(2).map(|s| s.to_string()),
            ),
            _ => continue,
        };

        let Some(file_path) = new_path.as_ref().or(old_path.as_ref()) else {
            tracing::warn!(
                "Skipping malformed name-status line for attempt {}: {}",
                attempt_id,
                line
            );
            continue;
        };
        let path_key = file_path.to_string();

        // Get old content from base branch
        let old_content = if change_type != "added" {
            let old_ref = format!(
                "{}:{}",
                diff_base_ref,
                old_path.as_ref().unwrap_or(file_path)
            );
            let output = tokio::process::Command::new("git")
                .current_dir(worktree_path)
                .args(["show", &old_ref])
                .output()
                .await
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string());
            output
        } else {
            None
        };

        // Get new content from working directory
        let new_content = if change_type != "deleted" {
            let full_path = std::path::Path::new(worktree_path).join(file_path);
            tokio::fs::read_to_string(&full_path).await.ok()
        } else {
            None
        };

        // Count additions/deletions
        let (adds, dels) = count_line_changes(&old_content, &new_content);
        total_additions += adds;
        total_deletions += dels;

        file_diffs.push(FileDiff {
            change: change_type.to_string(),
            old_path,
            new_path,
            old_content,
            new_content,
            additions: adds,
            deletions: dels,
        });
        seen_paths.insert(path_key);
    }

    // Include tracked working-tree changes (unstaged + staged) so diff is
    // realtime during running attempts, even before commit.
    for args in [
        ["diff", "--name-status", "--find-renames"].as_slice(),
        ["diff", "--cached", "--name-status", "--find-renames"].as_slice(),
    ] {
        let output = tokio::process::Command::new("git")
            .current_dir(worktree_path)
            .args(args)
            .output()
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to get working tree changes: {}", e))
            })?;

        if !output.status.success() {
            tracing::warn!(
                "git {:?} failed for attempt {} at {}: {}",
                args,
                attempt_id,
                worktree_path,
                String::from_utf8_lossy(&output.stderr)
            );
            continue;
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.is_empty() {
                continue;
            }

            let status = parts[0];
            let (change_type, old_path, new_path) = match status.chars().next() {
                Some('M') => (
                    "modified",
                    parts.get(1).map(|s| s.to_string()),
                    parts.get(1).map(|s| s.to_string()),
                ),
                Some('A') => ("added", None, parts.get(1).map(|s| s.to_string())),
                Some('D') => ("deleted", parts.get(1).map(|s| s.to_string()), None),
                Some('R') => (
                    "renamed",
                    parts.get(1).map(|s| s.to_string()),
                    parts.get(2).map(|s| s.to_string()),
                ),
                _ => continue,
            };

            let Some(file_path) = new_path.as_ref().or(old_path.as_ref()) else {
                continue;
            };
            let path_key = file_path.to_string();

            if file_path.is_empty() || seen_paths.contains(file_path) {
                continue;
            }

            let old_content = if change_type != "added" {
                let old_ref = format!(
                    "{}:{}",
                    diff_base_ref,
                    old_path.as_ref().unwrap_or(file_path)
                );
                tokio::process::Command::new("git")
                    .current_dir(worktree_path)
                    .args(["show", &old_ref])
                    .output()
                    .await
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            } else {
                None
            };

            let new_content = if change_type != "deleted" {
                let full_path = std::path::Path::new(worktree_path).join(file_path);
                tokio::fs::read_to_string(&full_path).await.ok()
            } else {
                None
            };

            let (adds, dels) = count_line_changes(&old_content, &new_content);
            total_additions += adds;
            total_deletions += dels;

            file_diffs.push(FileDiff {
                change: change_type.to_string(),
                old_path,
                new_path,
                old_content,
                new_content,
                additions: adds,
                deletions: dels,
            });
            seen_paths.insert(path_key);
        }
    }

    // Handle untracked files (new files not yet staged)
    let untracked_output = tokio::process::Command::new("git")
        .current_dir(worktree_path)
        .args(["ls-files", "--others", "--exclude-standard"])
        .output()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get untracked files: {}", e)))?;

    let untracked_str = String::from_utf8_lossy(&untracked_output.stdout);

    for file in untracked_str.lines().filter(|l| !l.is_empty()) {
        if seen_paths.contains(file) {
            continue;
        }

        let full_path = std::path::Path::new(worktree_path).join(file);
        if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
            let line_count = content.lines().count() as i32;
            total_additions += line_count;

            file_diffs.push(FileDiff {
                change: "added".to_string(),
                old_path: None,
                new_path: Some(file.to_string()),
                old_content: None,
                new_content: Some(content),
                additions: line_count,
                deletions: 0,
            });
            seen_paths.insert(file.to_string());
        }
    }

    let total_files = file_diffs.len() as i32;

    // Completed attempt + live diff empty: fall back to persisted snapshot if available.
    if total_files == 0 && !attempt_active {
        if let Some((diff, message)) = load_persisted_diff_response(
            &state,
            &attempt_service,
            attempt_id,
            attempt.s3_diff_key.as_ref(),
        )
        .await?
        {
            let response = ApiResponse::success(diff, &message);
            return Ok(Json(response));
        }
    }

    let response = ApiResponse::success(
        DiffResponse {
            files: file_diffs,
            total_files,
            total_additions,
            total_deletions,
        },
        "Diff retrieved successfully",
    );

    Ok(Json(response))
}

#[allow(dead_code)]
fn parse_diff_stats(stat_str: &str) -> (i32, i32, i32) {
    // Parse git diff --stat output like: "3 files changed, 10 insertions(+), 5 deletions(-)"
    let mut files = 0;
    let mut adds = 0;
    let mut dels = 0;

    for line in stat_str.lines() {
        if line.contains("files changed") || line.contains("file changed") {
            // Parse the summary line
            for part in line.split(',') {
                let part = part.trim();
                if part.contains("file") {
                    if let Some(num) = part.split_whitespace().next() {
                        files = num.parse().unwrap_or(0);
                    }
                } else if part.contains("insertion") {
                    if let Some(num) = part.split_whitespace().next() {
                        adds = num.parse().unwrap_or(0);
                    }
                } else if part.contains("deletion") {
                    if let Some(num) = part.split_whitespace().next() {
                        dels = num.parse().unwrap_or(0);
                    }
                }
            }
        }
    }

    (files, adds, dels)
}

/// Count line additions and deletions between old and new content
fn count_line_changes(old_content: &Option<String>, new_content: &Option<String>) -> (i32, i32) {
    match (old_content, new_content) {
        (None, Some(new)) => {
            // New file - all lines are additions
            (new.lines().count() as i32, 0)
        }
        (Some(old), None) => {
            // Deleted file - all lines are deletions
            (0, old.lines().count() as i32)
        }
        (Some(old), Some(new)) => {
            // Modified file - simple line count difference
            let old_lines: std::collections::HashSet<&str> = old.lines().collect();
            let new_lines: std::collections::HashSet<&str> = new.lines().collect();

            let additions = new_lines.difference(&old_lines).count() as i32;
            let deletions = old_lines.difference(&new_lines).count() as i32;

            (additions, deletions)
        }
        (None, None) => (0, 0),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/attempts/{id}/branch-status",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    responses(
        (status = 200, description = "Branch status retrieved", body = ApiResponse<BranchStatus>),
        (status = 404, description = "Attempt not found")
    )
)]
pub async fn get_branch_status(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<BranchStatus>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    // Init tasks work on setup branch (main); no MR creation/check. Skip MR logic.
    if task.task_type == TaskType::Init {
        let branch_name = attempt
            .metadata
            .get("branch_name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("main")
            .to_string();
        let status = BranchStatus {
            branch_name: branch_name.clone(),
            target_branch_name: "main".to_string(),
            ahead_count: 0,
            behind_count: 0,
            has_conflicts: false,
            is_attempt_active: matches!(
                attempt.status,
                AttemptStatus::Queued | AttemptStatus::Running
            ),
            can_push: false,
            can_merge: false,
            pr_url: None,
            pr_status: None,
        };
        return Ok(Json(ApiResponse::success(
            status,
            "Branch status retrieved successfully",
        )));
    }

    // Branch and target metadata defaults.
    let expected_branch = format!("feat/attempt-{}", attempt_id);
    let mut branch_name = attempt
        .metadata
        .get("branch_name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(expected_branch.as_str())
        .to_string();
    let mut target_branch_name = attempt
        .metadata
        .get("base_branch")
        .and_then(|v| v.as_str())
        .unwrap_or("main")
        .to_string();
    if target_branch_name.starts_with("origin/") || target_branch_name.starts_with("upstream/") {
        target_branch_name = target_branch_name
            .strip_prefix("origin/")
            .or_else(|| target_branch_name.strip_prefix("upstream/"))
            .unwrap_or(&target_branch_name)
            .to_string();
    }

    // Hide review/merge actions if task is no longer in review or change request already closed.
    // Sync MR/PR status from the provider when opening task log.
    let mr_row: Option<(String, Option<i64>, Option<i64>, String, Option<i64>, Option<String>)> =
        sqlx::query_as(
        r#"
        SELECT status, gitlab_mr_iid, github_pr_number, web_url, target_project_id, target_repository_url
        FROM merge_requests
        WHERE attempt_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        )
        .bind(attempt_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check merge request status: {}", e)))?;

    let mut mr_already_merged = mr_row
        .as_ref()
        .map(|(s, _, _, _, _, _)| s.eq_ignore_ascii_case("merged"))
        .unwrap_or(false);
    let mut change_request_closed = mr_row
        .as_ref()
        .map(|(s, _, _, _, _, _)| s.eq_ignore_ascii_case("closed"))
        .unwrap_or(false);

    let (pr_url_from_mr, pr_status_from_mr) = mr_row
        .as_ref()
        .map(|(s, _, _, url, _, _)| (url.clone(), s.clone()))
        .unwrap_or((String::new(), String::new()));

    // Sync from GitLab/GitHub if the underlying change request is still open.
    let mut gitlab_can_merge: Option<bool> = None;
    let mut github_can_merge: Option<bool> = None;
    if let Some((
        db_status,
        gitlab_iid,
        github_pr_number,
        _,
        target_project_id,
        target_repository_url,
    )) = &mr_row
    {
        if let Some(mr_iid) = gitlab_iid {
            if !db_status.eq_ignore_ascii_case("merged")
                && !db_status.eq_ignore_ascii_case("closed")
            {
                if let Ok(Some(config)) = state.gitlab_service.get_config(task.project_id).await {
                    if let Ok(client) = state.gitlab_service.get_client(task.project_id).await {
                        let gitlab_project_id =
                            target_project_id.unwrap_or(config.gitlab_project_id) as u64;
                        if let Ok(mr) = client
                            .get_merge_request(gitlab_project_id, *mr_iid as u64)
                            .await
                        {
                            if mr.state.eq_ignore_ascii_case("merged") {
                                let _ = sqlx::query(
                                    r#"
                                    UPDATE merge_requests
                                    SET status = 'merged', updated_at = NOW()
                                    WHERE attempt_id = $1 AND gitlab_mr_iid = $2
                                    "#,
                                )
                                .bind(attempt_id)
                                .bind(mr_iid)
                                .execute(&pool)
                                .await;
                                mr_already_merged = true;
                                change_request_closed = true;
                            } else if mr.state.eq_ignore_ascii_case("closed") {
                                let _ = sqlx::query(
                                    r#"
                                    UPDATE merge_requests
                                    SET status = 'closed', updated_at = NOW()
                                    WHERE attempt_id = $1 AND gitlab_mr_iid = $2
                                    "#,
                                )
                                .bind(attempt_id)
                                .bind(mr_iid)
                                .execute(&pool)
                                .await;
                                change_request_closed = true;
                            } else {
                                if let Some(ref ms) = mr.merge_status {
                                    gitlab_can_merge =
                                        Some(ms.eq_ignore_ascii_case("can_be_merged"));
                                } else if let Some(has_conf) = mr.has_conflicts {
                                    gitlab_can_merge = Some(!has_conf);
                                }
                            }
                        }
                    }
                }
            }
        }
        if let (Some(pr_number), Some(target_repository_url)) =
            (github_pr_number, target_repository_url.as_deref())
        {
            if !db_status.eq_ignore_ascii_case("merged")
                && !db_status.eq_ignore_ascii_case("closed")
            {
                if let Ok((client, owner, repo)) =
                    resolve_github_client_for_repo(&state.settings_service, target_repository_url)
                        .await
                {
                    if let Ok(pr) = client
                        .get_pull_request(&owner, &repo, *pr_number as u64)
                        .await
                    {
                        if pr.state.eq_ignore_ascii_case("closed") && pr.merged.unwrap_or(false) {
                            let _ = sqlx::query(
                                r#"
                                UPDATE merge_requests
                                SET status = 'merged', updated_at = NOW()
                                WHERE attempt_id = $1 AND github_pr_number = $2
                                "#,
                            )
                            .bind(attempt_id)
                            .bind(pr_number)
                            .execute(&pool)
                            .await;
                            mr_already_merged = true;
                            change_request_closed = true;
                        } else if pr.state.eq_ignore_ascii_case("closed") {
                            let _ = sqlx::query(
                                r#"
                                UPDATE merge_requests
                                SET status = 'closed', updated_at = NOW()
                                WHERE attempt_id = $1 AND github_pr_number = $2
                                "#,
                            )
                            .bind(attempt_id)
                            .bind(pr_number)
                            .execute(&pool)
                            .await;
                            change_request_closed = true;
                        } else if let Some(mergeable) = pr.mergeable {
                            github_can_merge = Some(mergeable);
                        } else if let Some(mergeable_state) = pr.mergeable_state.as_deref() {
                            github_can_merge = Some(matches!(
                                mergeable_state,
                                "clean" | "unstable" | "has_hooks"
                            ));
                        }
                    }
                }
            }
        }
    }

    // When MR is merged (from DB or GitLab sync), ensure task is Done and cleanup worktree
    if mr_already_merged && task.status != TaskStatus::Done {
        let _ = sqlx::query("UPDATE tasks SET status = 'done', updated_at = NOW() WHERE id = $1")
            .bind(task.id)
            .execute(&pool)
            .await;
        openclaw::emit_task_status_changed(
            &state,
            task.project_id,
            task.id,
            task.status,
            TaskStatus::Done,
            "routes.task_attempts.get_attempt_diff.mr_merged_sync",
        )
        .await;

        // Cleanup worktree (merged on GitLab, no longer needed)
        if let Err(e) = state.orchestrator.cleanup_worktree_public(attempt_id).await {
            tracing::warn!("Worktree cleanup after MR merge: {}", e);
        }
    }

    // Merge/push actions valid when: attempt succeeded, MR not yet merged.
    // Allow when task is InReview (normal) OR Done (MR opened but merge was skipped/failed earlier).
    let can_action = matches!(attempt.status, AttemptStatus::Success)
        && !mr_already_merged
        && !change_request_closed
        && (task.status == TaskStatus::InReview || task.status == TaskStatus::Done);

    let mut ahead_count = 0;
    let mut behind_count = 0;
    let mut has_conflicts = false;

    if let Some(worktree_path) = attempt
        .metadata
        .get("worktree_path")
        .and_then(|v| v.as_str())
        .filter(|p| std::path::Path::new(p).exists())
    {
        // Prefer live git branch from worktree when available.
        if let Ok(output) = tokio::process::Command::new("git")
            .current_dir(worktree_path)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .await
        {
            if output.status.success() {
                let current = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !current.is_empty() {
                    branch_name = current;
                }
            }
        }

        // Resolve target reference in local/remote namespace.
        let mut target_ref = target_branch_name.clone();
        let target_exists = tokio::process::Command::new("git")
            .current_dir(worktree_path)
            .args(["rev-parse", "--verify", &target_ref])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !target_exists {
            for remote_name in ["origin", "upstream"] {
                let remote_ref = format!("{}/{}", remote_name, target_branch_name);
                let remote_exists = tokio::process::Command::new("git")
                    .current_dir(worktree_path)
                    .args(["rev-parse", "--verify", &remote_ref])
                    .output()
                    .await
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                if remote_exists {
                    target_ref = remote_ref;
                    break;
                }
            }
        }

        // ahead/behind counts: `<target>...HEAD` => left=behind, right=ahead.
        let rev_spec = format!("{}...HEAD", target_ref);
        if let Ok(output) = tokio::process::Command::new("git")
            .current_dir(worktree_path)
            .args(["rev-list", "--left-right", "--count", &rev_spec])
            .output()
            .await
        {
            if output.status.success() {
                let counts = String::from_utf8_lossy(&output.stdout);
                let mut parts = counts.split_whitespace();
                behind_count = parts
                    .next()
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(0);
                ahead_count = parts
                    .next()
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(0);
            }
        }

        // Detect likely conflicts: prefer provider mergeability when available.
        if let Some(can_merge) = gitlab_can_merge.or(github_can_merge) {
            has_conflicts = !can_merge;
        } else {
            // Fallback: local git merge-tree when GitLab data unavailable
            if let Ok(base_output) = tokio::process::Command::new("git")
                .current_dir(worktree_path)
                .args(["merge-base", "HEAD", &target_ref])
                .output()
                .await
            {
                if base_output.status.success() {
                    let merge_base = String::from_utf8_lossy(&base_output.stdout)
                        .trim()
                        .to_string();
                    if !merge_base.is_empty() {
                        if let Ok(tree_output) = tokio::process::Command::new("git")
                            .current_dir(worktree_path)
                            .args(["merge-tree", &merge_base, "HEAD", &target_ref])
                            .output()
                            .await
                        {
                            if tree_output.status.success() {
                                has_conflicts = String::from_utf8_lossy(&tree_output.stdout)
                                    .contains("<<<<<<<");
                            }
                        }
                    }
                }
            }
        }
    }

    // When worktree is missing, still use provider mergeability if available.
    if !has_conflicts && (gitlab_can_merge == Some(false) || github_can_merge == Some(false)) {
        has_conflicts = true;
    }

    let effective_pr_status = if mr_already_merged {
        Some("merged".to_string())
    } else if change_request_closed {
        Some("closed".to_string())
    } else if pr_status_from_mr.is_empty() {
        None
    } else {
        Some(pr_status_from_mr.clone())
    };

    let pr_url = attempt
        .metadata
        .get("pr_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            if pr_url_from_mr.is_empty() {
                None
            } else {
                Some(pr_url_from_mr.clone())
            }
        });
    let pr_status = attempt
        .metadata
        .get("pr_status")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or(effective_pr_status);

    let status = BranchStatus {
        branch_name,
        target_branch_name,
        ahead_count,
        behind_count,
        has_conflicts,
        is_attempt_active: matches!(
            attempt.status,
            AttemptStatus::Queued | AttemptStatus::Running
        ),
        can_push: can_action,
        can_merge: can_action && !has_conflicts,
        pr_url,
        pr_status,
    };

    Ok(Json(ApiResponse::success(
        status,
        "Branch status retrieved successfully",
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/attempts/{id}/approve",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    request_body = ApproveRequest,
    responses(
        (status = 200, description = "Changes approved, committed and pushed", body = EmptyResponse),
        (status = 404, description = "Attempt not found"),
        (status = 400, description = "Task not in review state")
    )
)]
pub async fn approve_attempt(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
    Json(payload): Json<ApproveRequest>,
) -> ApiResult<Json<ApiResponse<()>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions and status
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ExecuteTask,
        &pool,
    )
    .await?;

    // Init tasks work on setup branch (main); no MR. Approve is a no-op.
    if task.task_type == TaskType::Init {
        return Ok(Json(ApiResponse::success(
            (),
            "Init task already completed on setup branch",
        )));
    }

    // Accept both in_review and done to make approve idempotent.
    // done can happen in no-review flow or if task was already finalized.
    let already_done = task.status == TaskStatus::Done;
    if task.status != TaskStatus::InReview && task.status != TaskStatus::Done {
        return Err(ApiError::BadRequest(format!(
            "Task is not in review/done state (current: {:?})",
            task.status
        )));
    }

    // In review flow, agent keeps changes uncommitted/unpushed.
    // Approve must finalize git operations before MR/merge.
    if task.status == TaskStatus::InReview {
        let worktree_path_str = attempt
            .metadata
            .get("worktree_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ApiError::BadRequest("Worktree path not found".to_string()))?;
        let worktree_path = std::path::PathBuf::from(worktree_path_str);
        if !worktree_path.exists() {
            return Err(ApiError::BadRequest(
                "Worktree no longer exists for this attempt".to_string(),
            ));
        }

        let commit_message = payload
            .commit_message
            .clone()
            .unwrap_or_else(|| format!("chore: approve attempt {}", attempt_id));

        state
            .orchestrator
            .worktree_manager()
            .commit_worktree(&worktree_path, &commit_message)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to commit approved changes: {}", e)))?;

        state
            .orchestrator
            .worktree_manager()
            .push_worktree(&worktree_path)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to push approved changes: {}", e)))?;
    }

    // Avoid duplicate MR creation on repeated approvals.
    let has_existing_mr: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM merge_requests
            WHERE attempt_id = $1
        )
        "#,
    )
    .bind(attempt_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to check existing merge request: {}", e)))?;

    // Create MR via GitOps if needed (only while task is still in review).
    // For in-review attempts, branch has just been committed/pushed above.
    if task.status == TaskStatus::InReview {
        if !has_existing_mr {
            state
                .orchestrator
                .handle_gitops_public(attempt_id)
                .await
                .map_err(|e| ApiError::Internal(format!("GitOps MR creation failed: {}", e)))?;
        } else {
            tracing::info!(
                "Skipping GitOps MR creation for attempt {}: merge request already exists for attempt",
                attempt_id,
            );
        }
    } else {
        tracing::info!(
            "Skipping GitOps MR creation for attempt {}: task already finalized",
            attempt_id
        );
    }

    // Merge the current attempt MR into target branch. Uses orchestrator's merge logic
    // (same as agent flow) which can resolve gitlab_project_id when project not yet linked.
    // If merge fails with 404 (MR deleted on GitLab), recreate MR and retry.
    // If merge fails with conflict/405/406: do NOT mark Done, do NOT cleanup worktree.
    let merged = match state
        .orchestrator
        .handle_gitops_merge_public(attempt_id)
        .await
    {
        Ok(m) => m,
        Err(e) => {
            let err_str = e.to_string();
            let is_404 = err_str.contains("404");
            let no_mr_found = err_str.contains("No merge request found");
            if is_404 || no_mr_found {
                tracing::info!("Merge failed (404 or no MR found). Creating MR and retrying...");
                let _ = sqlx::query("DELETE FROM merge_requests WHERE attempt_id = $1")
                    .bind(attempt_id)
                    .execute(&pool)
                    .await;
                match state.orchestrator.handle_gitops_public(attempt_id).await {
                    Ok(()) => {
                        match state
                            .orchestrator
                            .handle_gitops_merge_public(attempt_id)
                            .await
                        {
                            Ok(m) => m,
                            Err(retry_e) => {
                                tracing::warn!("Merge retry failed: {}", retry_e);
                                return Err(ApiError::Conflict(format!(
                                    "Merge failed: {}. Use 'Request Changes' with instruction to have agent resolve conflicts.",
                                    retry_e
                                )));
                            }
                        }
                    }
                    Err(create_e) => {
                        tracing::warn!("Failed to recreate MR: {}", create_e);
                        return Err(ApiError::Internal(format!(
                            "Failed to recreate merge request: {}",
                            create_e
                        )));
                    }
                }
            } else {
                tracing::warn!("Merge MR failed: {}. Task stays InReview.", e);
                return Err(ApiError::Conflict(format!(
                    "Merge failed: {}. Use 'Request Changes' with instruction 'Pull main, resolve conflicts, push again.' to have the agent fix.",
                    e
                )));
            }
        }
    };

    // Update task status to Done if not already done (only when merge succeeded).
    if !already_done {
        sqlx::query("UPDATE tasks SET status = 'done', updated_at = NOW() WHERE id = $1")
            .bind(task.id)
            .execute(&pool)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update task status: {}", e)))?;
        openclaw::emit_task_status_changed(
            &state,
            task.project_id,
            task.id,
            task.status,
            TaskStatus::Done,
            "routes.task_attempts.approve_attempt.merge_success",
        )
        .await;
    }

    // Run diff capture + S3 upload + worktree cleanup in background to avoid blocking response.
    let worktree_path_opt = attempt
        .metadata
        .get("worktree_path")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from)
        .filter(|p| p.exists());
    if let Some(worktree_path) = worktree_path_opt {
        let orchestrator = state.orchestrator.clone();
        let storage = state.storage_service.clone();
        let pool_bg = pool.clone();
        let aid = attempt_id;
        let task_id = attempt.task_id;
        tokio::spawn(async move {
            if let Ok(snapshot) = orchestrator
                .collect_diffs_for_s3(aid, task_id, &worktree_path)
                .await
            {
                let s3_key =
                    acpms_executors::AttemptDiffSnapshot::generate_s3_key(aid, snapshot.saved_at);
                let snapshot_size = snapshot.calculate_total_size();
                if storage.upload_json(&s3_key, &snapshot).await.is_ok() {
                    let _ = sqlx::query(
                        "UPDATE task_attempts SET s3_diff_key = $1, s3_diff_size = $2, s3_diff_saved_at = $3 WHERE id = $4",
                    )
                    .bind(&s3_key)
                    .bind(snapshot_size)
                    .bind(snapshot.saved_at)
                    .bind(aid)
                    .execute(&pool_bg)
                    .await;
                    tracing::info!(
                        "Saved {} file diffs to S3: {}",
                        snapshot.total_files,
                        s3_key
                    );
                }
            }
            if let Err(e) = orchestrator.cleanup_worktree_public(aid).await {
                tracing::warn!("Worktree cleanup failed after approval: {}", e);
            }
        });
    }

    let response = if merged {
        ApiResponse::success((), "Changes approved and merged successfully")
    } else if already_done {
        ApiResponse::success((), "Task already completed; approval accepted")
    } else {
        ApiResponse::success((), "Changes approved and pushed successfully")
    };
    Ok(Json(response))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RejectRequest {
    pub reason: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/v1/attempts/{id}/reject",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    request_body = RejectRequest,
    responses(
        (status = 200, description = "Changes rejected and worktree cleaned up", body = EmptyResponse),
        (status = 404, description = "Attempt not found"),
        (status = 400, description = "Task not in review state")
    )
)]
pub async fn reject_attempt(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
    Json(payload): Json<RejectRequest>,
) -> ApiResult<Json<ApiResponse<()>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions and status
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ExecuteTask,
        &pool,
    )
    .await?;

    // Verify task is in review state
    if task.status != TaskStatus::InReview {
        return Err(ApiError::BadRequest(format!(
            "Task is not in review state (current: {:?})",
            task.status
        )));
    }

    let reason = payload
        .reason
        .unwrap_or_else(|| "Rejected by reviewer".to_string());

    // Update attempt status to Failed with rejection reason
    sqlx::query(
        "UPDATE task_attempts SET status = 'failed', error_message = $2, completed_at = NOW() WHERE id = $1"
    )
    .bind(attempt_id)
    .bind(&reason)
    .execute(&pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update attempt status: {}", e)))?;

    // Revert task status to Todo so it can be retried
    sqlx::query("UPDATE tasks SET status = 'todo', updated_at = NOW() WHERE id = $1")
        .bind(task.id)
        .execute(&pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update task status: {}", e)))?;
    openclaw::emit_task_status_changed(
        &state,
        task.project_id,
        task.id,
        task.status,
        TaskStatus::Todo,
        "routes.task_attempts.reject_attempt.revert_to_todo",
    )
    .await;

    // Run diff capture + S3 upload + cleanup in background (avoids blocking response).
    if let Some(worktree_path_str) = attempt
        .metadata
        .get("worktree_path")
        .and_then(|v| v.as_str())
    {
        let worktree_path = std::path::PathBuf::from(worktree_path_str);
        if worktree_path.exists() {
            let orchestrator = state.orchestrator.clone();
            let storage = state.storage_service.clone();
            let pool_bg = pool.clone();
            let aid = attempt_id;
            let task_id = attempt.task_id;
            tokio::spawn(async move {
                if let Ok(snapshot) = orchestrator
                    .collect_diffs_for_s3(aid, task_id, &worktree_path)
                    .await
                {
                    let s3_key = acpms_executors::AttemptDiffSnapshot::generate_s3_key(
                        aid,
                        snapshot.saved_at,
                    );
                    let snapshot_size = snapshot.calculate_total_size();
                    if storage.upload_json(&s3_key, &snapshot).await.is_ok() {
                        let _ = sqlx::query(
                            "UPDATE task_attempts SET s3_diff_key = $1, s3_diff_size = $2, s3_diff_saved_at = $3 WHERE id = $4",
                        )
                        .bind(&s3_key)
                        .bind(snapshot_size)
                        .bind(snapshot.saved_at)
                        .bind(aid)
                        .execute(&pool_bg)
                        .await;
                        tracing::info!(
                            "Saved {} file diffs to S3 before rejection: {}",
                            snapshot.total_files,
                            s3_key
                        );
                    }
                }
                if let Err(e) = orchestrator.cleanup_worktree_public(aid).await {
                    tracing::warn!("Worktree cleanup failed after rejection: {}", e);
                }
            });
        }
    }

    let response = ApiResponse::success((), format!("Changes rejected: {}", reason));
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/attempts/{id}/rebase",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    responses(
        (status = 200, description = "Branch rebased successfully", body = EmptyResponse),
        (status = 400, description = "Task not in review state or rebase conflict"),
        (status = 404, description = "Attempt not found")
    )
)]
pub async fn rebase_attempt(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<()>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ExecuteTask,
        &pool,
    )
    .await?;

    if task.status != TaskStatus::InReview {
        return Err(ApiError::BadRequest(format!(
            "Task is not in review state (current: {:?})",
            task.status
        )));
    }

    let worktree_path = attempt
        .metadata
        .get("worktree_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("Worktree path not found".to_string()))?;

    if !std::path::Path::new(worktree_path).exists() {
        return Err(ApiError::BadRequest(
            "Worktree no longer exists for this attempt".to_string(),
        ));
    }

    let target_branch = attempt
        .metadata
        .get("base_branch")
        .and_then(|v| v.as_str())
        .unwrap_or("main")
        .to_string();

    // Best effort: fetch latest target branch. Ignore failure for local-only repos.
    let _ = tokio::process::Command::new("git")
        .current_dir(worktree_path)
        .args(["fetch", "origin", &target_branch])
        .output()
        .await;

    let target_ref = {
        let local_exists = tokio::process::Command::new("git")
            .current_dir(worktree_path)
            .args(["rev-parse", "--verify", &target_branch])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);
        if local_exists {
            target_branch.clone()
        } else {
            let remote_ref = format!("origin/{}", target_branch);
            let remote_exists = tokio::process::Command::new("git")
                .current_dir(worktree_path)
                .args(["rev-parse", "--verify", &remote_ref])
                .output()
                .await
                .map(|o| o.status.success())
                .unwrap_or(false);
            if remote_exists {
                remote_ref
            } else {
                target_branch.clone()
            }
        }
    };

    let rebase_output = tokio::process::Command::new("git")
        .current_dir(worktree_path)
        .args(["rebase", &target_ref])
        .output()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to execute rebase: {}", e)))?;

    if !rebase_output.status.success() {
        let _ = tokio::process::Command::new("git")
            .current_dir(worktree_path)
            .args(["rebase", "--abort"])
            .output()
            .await;

        let stderr = String::from_utf8_lossy(&rebase_output.stderr)
            .trim()
            .to_string();
        let stdout = String::from_utf8_lossy(&rebase_output.stdout)
            .trim()
            .to_string();
        let reason = if !stderr.is_empty() { stderr } else { stdout };
        return Err(ApiError::BadRequest(format!(
            "Rebase failed. Resolve conflicts and retry. {}",
            reason
        )));
    }

    if let Err(e) = StatusManager::log(
        &pool,
        &state.broadcast_tx,
        attempt_id,
        "system",
        &format!("Branch rebased onto {}", target_ref),
    )
    .await
    {
        tracing::warn!("Failed to emit rebase log for {}: {}", attempt_id, e);
    }

    Ok(Json(ApiResponse::success(
        (),
        format!("Rebased onto {}", target_ref),
    )))
}

// ============================================================================
// Retry Endpoints
// ============================================================================

#[utoipa::path(
    get,
    path = "/api/v1/attempts/{id}/retry-info",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    responses(
        (status = 200, description = "Retry information retrieved", body = ApiResponse<RetryInfoDto>),
        (status = 404, description = "Attempt not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_retry_info(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<RetryInfoDto>>> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ViewProject,
        &pool,
    )
    .await?;

    // Get project settings for retry config
    let project_service = ProjectService::new(pool.clone());
    let settings = project_service
        .get_settings(task.project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Build retry info from attempt and settings
    let retry_info = RetryInfo::from_attempt(&attempt, &settings);

    let dto = RetryInfoDto {
        retry_count: retry_info.retry_count,
        max_retries: retry_info.max_retries,
        remaining_retries: retry_info.remaining_retries,
        can_retry: retry_info.can_retry,
        auto_retry_enabled: retry_info.auto_retry_enabled,
        previous_attempt_id: retry_info.previous_attempt_id,
        previous_error: retry_info.previous_error,
        next_retry_attempt_id: retry_info.next_retry_attempt_id,
        next_backoff_seconds: retry_info.next_backoff_seconds,
    };

    let response = ApiResponse::success(dto, "Retry info retrieved successfully");
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/attempts/{id}/retry",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID to retry")
    ),
    responses(
        (status = 201, description = "Retry attempt created", body = ApiResponse<RetryResponseDto>),
        (status = 404, description = "Attempt not found"),
        (status = 400, description = "Cannot retry - max retries exceeded or attempt not failed"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn retry_attempt(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
) -> ApiResult<(StatusCode, Json<ApiResponse<RetryResponseDto>>)> {
    let pool = state.db.clone();
    let attempt_service = TaskAttemptService::new(pool.clone());
    let attempt = attempt_service
        .get_attempt(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task attempt not found".to_string()))?;

    // Get task to check permissions
    let task_service = TaskService::new(pool.clone());
    let task = task_service
        .get_task(attempt.task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    // Check permission using RBAC
    RbacChecker::check_permission(
        auth_user.id,
        task.project_id,
        Permission::ExecuteTask,
        &pool,
    )
    .await?;

    // Verify attempt is in a retriable state (failed or cancelled)
    if !matches!(
        attempt.status,
        AttemptStatus::Failed | AttemptStatus::Cancelled
    ) {
        return Err(ApiError::BadRequest(format!(
            "Cannot retry attempt in {} state. Only failed or cancelled attempts can be retried.",
            match attempt.status {
                AttemptStatus::Queued => "queued",
                AttemptStatus::Running => "running",
                AttemptStatus::Success => "success",
                _ => "unknown",
            }
        )));
    }

    // Get project settings for retry config
    let project_service = ProjectService::new(pool.clone());
    let settings = project_service
        .get_settings(task.project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Check retry count
    let retry_info = RetryInfo::from_attempt(&attempt, &settings);
    if retry_info.retry_count >= settings.max_retries {
        return Err(ApiError::BadRequest(format!(
            "Maximum retries ({}) exceeded for this task",
            settings.max_retries
        )));
    }

    // Create new retry attempt with metadata linking to previous attempt
    let retry_count = retry_info.retry_count + 1;
    let previous_task_status = task.status;
    let retry_metadata = serde_json::json!({
        "retry_count": retry_count,
        "previous_attempt_id": attempt_id.to_string(),
        "previous_error": attempt.error_message,
        "manual_retry": true,
        "retried_by": auth_user.id.to_string(),
        "previous_task_status": task_status_to_metadata_value(previous_task_status),
    });

    let new_attempt = attempt_service
        .create_attempt_with_status_and_metadata(task.id, AttemptStatus::Queued, retry_metadata)
        .await
        .map_err(|e| {
            let message = e.to_string();
            if message.contains("already has an active attempt") {
                ApiError::BadRequest(message)
            } else {
                ApiError::Internal(format!("Failed to create retry attempt: {}", message))
            }
        })?;

    // Link previous attempt to new retry
    sqlx::query(
        r#"
        UPDATE task_attempts
        SET metadata = metadata || jsonb_build_object('retry_attempt_id', $2::text)
        WHERE id = $1
        "#,
    )
    .bind(attempt_id)
    .bind(new_attempt.id.to_string())
    .execute(&pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to link retry attempt: {}", e)))?;

    // Update task status to InProgress
    if let Err(error) = task_service
        .update_task_status(task.id, TaskStatus::InProgress)
        .await
    {
        let message = format!("Failed to update task status: {}", error);
        let _ = attempt_service
            .update_status(new_attempt.id, AttemptStatus::Failed, Some(message.clone()))
            .await;
        return Err(ApiError::Internal(message));
    }
    openclaw::emit_task_status_changed(
        &state,
        task.project_id,
        task.id,
        task.status,
        TaskStatus::InProgress,
        "routes.task_attempts.retry_attempt",
    )
    .await;

    // Get project for job creation
    let project = project_service
        .get_project(task.project_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;

    let repo_path = resolve_project_repo_path(&project);

    let skill_knowledge = state.orchestrator.skill_knowledge();
    let skill_context = build_skill_instruction_context(
        &task,
        &settings,
        project.project_type,
        Some(repo_path.as_path()),
        Some(&skill_knowledge),
    );
    if let Err(error) = persist_skill_instruction_context_metadata(
        &pool,
        new_attempt.id,
        &skill_context,
        "attempt_retry_create",
    )
    .await
    {
        tracing::warn!(
            attempt_id = %new_attempt.id,
            error = %error,
            "Failed to persist skill instruction metadata during retry creation"
        );
    }
    if let Err(error) =
        append_skill_timeline_log(&pool, &state.broadcast_tx, new_attempt.id, &skill_context).await
    {
        tracing::warn!(
            attempt_id = %new_attempt.id,
            error = %error,
            "Failed to append skill timeline log during retry creation"
        );
    }

    let require_review = task_require_review(&task, &settings);
    let mut instruction = build_attempt_instruction(&task, &skill_context, require_review);
    let preferred_settings = state.settings_service.get().await.ok();
    let preferred_lang = preferred_settings
        .as_ref()
        .and_then(|s| s.preferred_agent_language.as_deref());
    instruction = prepend_language_instruction(instruction, preferred_lang);

    // Create a process record for this retry attempt run.
    let execution_process_id =
        create_execution_process_record(&pool, new_attempt.id, Some(repo_path.as_path()), None)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to create execution process record: {}", e))
            })?;

    // Submit retry job to worker pool
    if let Some(worker_pool) = &state.worker_pool {
        use acpms_executors::AgentJob;

        let job = AgentJob::new(
            new_attempt.id,
            task.id,
            task.project_id,
            repo_path,
            instruction,
            require_review,
        )
        .with_timeout(settings.timeout_mins)
        .with_retry_config(settings.max_retries, settings.auto_retry)
        .with_project_max_concurrent(settings.max_concurrent)
        .with_retry_count(retry_count)
        .with_priority(JobPriority::High); // Retries get higher priority

        if let Err(error) = worker_pool.submit(job).await {
            cleanup_execution_process_record(&pool, execution_process_id).await;
            let message = format!("Failed to submit retry job: {}", error);
            mark_submission_failed(
                &attempt_service,
                &task_service,
                new_attempt.id,
                task.id,
                previous_task_status,
                message.clone(),
            )
            .await;
            return Err(ApiError::Internal(message));
        }
    } else {
        // Fallback to direct execution (without worker pool), preserving retry attempt_id.
        let orchestrator = state.orchestrator.clone();
        let attempt_id = new_attempt.id;
        let task_id = task.id;
        let project_id = task.project_id;
        let instruction = instruction.clone();
        let repo_path = repo_path.clone();
        tokio::spawn(async move {
            let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
            if let Err(e) = orchestrator
                .execute_task_with_cancel_review(
                    attempt_id,
                    task_id,
                    repo_path,
                    instruction,
                    cancel_rx,
                    require_review,
                )
                .await
            {
                tracing::error!(
                    "Direct retry execution failed for attempt {} (task {}, project {}): {:?}",
                    attempt_id,
                    task_id,
                    project_id,
                    e
                );
            }
        });
    }

    // Build response
    let retry_info_dto = RetryInfoDto {
        retry_count,
        max_retries: settings.max_retries,
        remaining_retries: (settings.max_retries - retry_count).max(0),
        can_retry: retry_count < settings.max_retries,
        auto_retry_enabled: settings.auto_retry,
        previous_attempt_id: Some(attempt_id),
        previous_error: attempt.error_message,
        next_retry_attempt_id: None,
        next_backoff_seconds: None,
    };

    let response_dto = RetryResponseDto {
        attempt: TaskAttemptDto::from(new_attempt),
        retry_info: retry_info_dto,
    };

    let response =
        ApiResponse::created(response_dto, "Retry attempt created and execution started");
    Ok((StatusCode::CREATED, Json(response)))
}

// ============================================================================
// Structured Logs and Subagent Tree Endpoints
// ============================================================================

/// Helper function to get project_id for an attempt
async fn get_project_id_for_attempt(
    pool: &sqlx::PgPool,
    attempt_id: Uuid,
) -> Result<Uuid, ApiError> {
    let record: (Uuid,) = sqlx::query_as(
        r#"
        SELECT t.project_id
        FROM task_attempts ta
        JOIN tasks t ON ta.task_id = t.id
        WHERE ta.id = $1
        "#,
    )
    .bind(attempt_id)
    .fetch_one(pool)
    .await
    .map_err(|_| ApiError::NotFound("Attempt not found".into()))?;

    Ok(record.0)
}

/// Helper function to check project permission
async fn check_project_permission(
    pool: &sqlx::PgPool,
    user: &AuthUser,
    project_id: Uuid,
) -> Result<(), ApiError> {
    RbacChecker::check_permission(user.id, project_id, Permission::ViewProject, pool).await
}

#[derive(Deserialize, IntoParams)]
pub struct StructuredLogsParams {
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: usize,

    /// Number of entries per page (max 500)
    #[serde(default = "default_page_size")]
    pub page_size: usize,

    /// Include subagent logs in results
    #[serde(default)]
    pub include_subagents: bool,

    /// Filter by entry types (comma-separated: action,file_change,todo_item,tool_status)
    pub entry_types: Option<String>,

    /// Filter by tool names (comma-separated: Read,Edit,Bash)
    pub tool_names: Option<String>,
}

fn default_page() -> usize {
    1
}
fn default_page_size() -> usize {
    100
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct FileDiffSummary {
    pub id: Uuid,
    pub file_path: String,
    pub additions: i32,
    pub deletions: i32,
    pub change_type: String,
}

#[derive(Serialize, ToSchema)]
pub struct StructuredLogsResponse {
    /// List of normalized log entries
    pub entries: Vec<serde_json::Value>,
    /// Total number of entries (before pagination)
    pub total: usize,
    /// Current page number
    pub page: usize,
    /// Number of entries per page
    pub page_size: usize,
    /// File diffs associated with this attempt
    pub file_diffs: Vec<FileDiffSummary>,
}

#[derive(Serialize, ToSchema)]
pub struct DiffSummaryResponse {
    pub files: Vec<FileDiffSummary>,
}

/// GET /api/v1/attempts/{id}/diff-summary
/// Lightweight endpoint returning only file diff metadata (no log processing).
/// Use this instead of structured-logs when only file_diffs are needed (e.g. timeline).
#[utoipa::path(
    get,
    path = "/api/v1/attempts/{id}/diff-summary",
    tag = "Task Attempts",
    params(("id" = Uuid, Path, description = "Attempt ID")),
    responses(
        (status = 200, description = "Diff summary retrieved", body = ApiResponse<DiffSummaryResponse>),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Attempt not found")
    )
)]
pub async fn get_attempt_diff_summary(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<DiffSummaryResponse>>> {
    let project_id = get_project_id_for_attempt(&state.db, attempt_id).await?;
    check_project_permission(&state.db, &auth_user, project_id).await?;

    let mut file_diffs = sqlx::query_as(
        r#"
        SELECT
            id,
            file_path,
            additions,
            deletions,
            change_type
        FROM file_diffs
        WHERE attempt_id = $1
        ORDER BY file_path
        "#,
    )
    .bind(attempt_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if file_diffs.is_empty() {
        let s3_diff_key = sqlx::query_scalar::<_, Option<String>>(
            "SELECT s3_diff_key FROM task_attempts WHERE id = $1",
        )
        .bind(attempt_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch attempt diff key: {}", e)))?;

        if let Some(s3_key) = s3_diff_key {
            match state
                .storage_service
                .download_json::<acpms_executors::AttemptDiffSnapshot>(&s3_key)
                .await
            {
                Ok(snapshot) => {
                    file_diffs = snapshot
                        .files
                        .into_iter()
                        .map(|file| FileDiffSummary {
                            id: Uuid::new_v4(),
                            file_path: file.path,
                            additions: file.additions,
                            deletions: file.deletions,
                            change_type: file.change,
                        })
                        .collect();
                    file_diffs.sort_by(|a, b| a.file_path.cmp(&b.file_path));
                }
                Err(err) => {
                    tracing::warn!(
                        "Failed to load S3 diff snapshot for diff-summary (attempt {}): {}",
                        attempt_id,
                        err
                    );
                }
            }
        }
    }

    Ok(Json(ApiResponse::success(
        DiffSummaryResponse { files: file_diffs },
        "Diff summary retrieved successfully",
    )))
}

/// GET /api/v1/attempts/{id}/structured-logs
/// Returns aggregated, normalized log entries (not raw logs)
#[utoipa::path(
    get,
    path = "/api/v1/attempts/{id}/structured-logs",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID"),
        StructuredLogsParams
    ),
    responses(
        (status = 200, description = "Structured logs retrieved", body = ApiResponse<StructuredLogsResponse>),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Attempt not found")
    )
)]
pub async fn get_structured_logs(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
    Query(params): Query<StructuredLogsParams>,
) -> ApiResult<Json<ApiResponse<StructuredLogsResponse>>> {
    // Get project_id for permission check
    let project_id = get_project_id_for_attempt(&state.db, attempt_id).await?;

    // Check permission
    check_project_permission(&state.db, &auth_user, project_id).await?;

    let normalized_service = NormalizedLogService::new(state.db.clone());

    // Resolve attempt IDs (single or subagent tree)
    let attempt_ids: Vec<Uuid> = if params.include_subagents {
        let subagent_service = SubagentService::new(state.db.clone());
        subagent_service
            .get_all_attempt_ids(attempt_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to get subagent tree: {}", e)))?
    } else {
        vec![attempt_id]
    };

    // Parse filters
    let entry_types: Option<Vec<String>> = params.entry_types.as_ref().map(|s| {
        s.split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect()
    });
    let tool_names: Option<Vec<String>> = params.tool_names.as_ref().map(|s| {
        s.split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect()
    });

    let page_size = params.page_size.clamp(1, 500);
    let page = params.page.max(1);

    // Server-side pagination: O(page_size) instead of O(total_entries)
    let (db_entries, total) = normalized_service
        .get_entries_paginated(
            &attempt_ids,
            entry_types.as_deref(),
            tool_names.as_deref(),
            page,
            page_size,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get structured logs: {}", e)))?;

    let paginated: Vec<serde_json::Value> = db_entries.into_iter().map(|e| e.entry_data).collect();

    // Get file diffs for this attempt
    let mut file_diffs = sqlx::query_as(
        r#"
        SELECT
            id,
            file_path,
            additions,
            deletions,
            change_type
        FROM file_diffs
        WHERE attempt_id = $1
        ORDER BY file_path
        "#,
    )
    .bind(attempt_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Fallback for modern attempts where diffs are persisted in S3 snapshot
    // and `file_diffs` table might be empty.
    if file_diffs.is_empty() {
        let s3_diff_key = sqlx::query_scalar::<_, Option<String>>(
            "SELECT s3_diff_key FROM task_attempts WHERE id = $1",
        )
        .bind(attempt_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch attempt diff key: {}", e)))?;

        if let Some(s3_key) = s3_diff_key {
            match state
                .storage_service
                .download_json::<acpms_executors::AttemptDiffSnapshot>(&s3_key)
                .await
            {
                Ok(snapshot) => {
                    file_diffs = snapshot
                        .files
                        .into_iter()
                        .map(|file| FileDiffSummary {
                            // Synthetic ID for frontend correlation in timeline view.
                            // `GET /attempts/:id/diff` still resolves by path/content.
                            id: Uuid::new_v4(),
                            file_path: file.path,
                            additions: file.additions,
                            deletions: file.deletions,
                            change_type: file.change,
                        })
                        .collect();
                    file_diffs.sort_by(|a, b| a.file_path.cmp(&b.file_path));
                }
                Err(err) => {
                    tracing::warn!(
                        "Failed to load S3 diff snapshot for structured logs (attempt {}): {}",
                        attempt_id,
                        err
                    );
                }
            }
        }
    }

    Ok(Json(ApiResponse::success(
        StructuredLogsResponse {
            entries: paginated,
            total: total as usize,
            page,
            page_size,
            file_diffs,
        },
        "Structured logs retrieved successfully",
    )))
}

#[derive(Serialize, ToSchema)]
pub struct SubagentTreeNode {
    pub attempt_id: Uuid,
    pub status: AttemptStatus,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub depth: i32,
    #[serde(default)]
    pub children: Vec<SubagentTreeNode>,
}

#[derive(Serialize, ToSchema)]
pub struct SubagentTreeResponse {
    /// List of subagent tree nodes
    pub nodes: Vec<SubagentTreeNode>,
    /// Total count of subagents
    pub total_count: usize,
}

/// GET /api/v1/attempts/{id}/subagent-tree
/// Returns hierarchical tree of subagents
#[utoipa::path(
    get,
    path = "/api/v1/attempts/{id}/subagent-tree",
    tag = "Task Attempts",
    params(
        ("id" = Uuid, Path, description = "Attempt ID")
    ),
    responses(
        (status = 200, description = "Subagent tree retrieved", body = ApiResponse<SubagentTreeResponse>),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Attempt not found")
    )
)]
pub async fn get_subagent_tree(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(attempt_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<SubagentTreeResponse>>> {
    // Get project_id for permission check
    let project_id = get_project_id_for_attempt(&state.db, attempt_id).await?;

    // Check permission
    check_project_permission(&state.db, &auth_user, project_id).await?;

    let subagent_service = SubagentService::new(state.db.clone());
    let service_nodes = subagent_service
        .get_subagent_tree(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get subagent tree: {}", e)))?;

    // Convert service nodes to API response nodes
    let nodes: Vec<SubagentTreeNode> = service_nodes
        .into_iter()
        .map(|node| SubagentTreeNode {
            attempt_id: node.attempt_id,
            status: node.status,
            started_at: node.started_at,
            completed_at: node.completed_at,
            depth: node.depth,
            children: node
                .children
                .into_iter()
                .map(|child| SubagentTreeNode {
                    attempt_id: child.attempt_id,
                    status: child.status,
                    started_at: child.started_at,
                    completed_at: child.completed_at,
                    depth: child.depth,
                    children: Vec::new(), // Flatten for now
                })
                .collect(),
        })
        .collect();

    let total_count = nodes.len();

    Ok(Json(ApiResponse::success(
        SubagentTreeResponse { nodes, total_count },
        "Subagent tree retrieved successfully",
    )))
}
