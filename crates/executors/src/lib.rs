pub mod agent_log_buffer;
pub mod assistant_log_buffer;
pub mod claude;
pub mod codex;
pub mod cursor;
mod diff_snapshot;
pub mod document_tasks;
pub mod follow_up_utils;
pub mod gemini;
pub mod input_queue;
pub mod job_queue;
pub mod knowledge_index;
pub mod normalization;
pub mod normalization_contract;
pub mod orchestrator;
pub mod process;
pub mod project_vault;
pub mod retry_handler;
pub mod router_config;
pub mod session;
pub mod skill_runtime;
pub mod task_skills;
pub mod worker_pool;
pub mod worktree;

// SDK Control Mode modules
pub mod agent_client;
pub mod approval;
pub mod log_writer;
pub mod msg_store;
pub mod protocol;
pub mod stdout_dup;

#[path = "sdk-normalized-types.rs"]
pub mod sdk_normalized_types;

#[path = "git-credential-helper.rs"]
pub mod git_credential_helper;

pub mod webhook_job;

#[path = "worker-pool-config.rs"]
pub mod worker_pool_config;

#[path = "worker-pool-executor.rs"]
pub mod worker_pool_executor;

#[path = "orchestrator-gitops.rs"]
pub mod orchestrator_gitops;

#[path = "orchestrator-status.rs"]
pub mod orchestrator_status;

pub use agent_log_buffer::{
    append_log_to_jsonl, buffer_agent_log, flush_agent_log_buffer, get_attempt_log_file_path,
    init_agent_log_buffer, parse_jsonl_tail_to_agent_logs, parse_jsonl_to_agent_logs,
    read_attempt_log_file, read_attempt_log_file_head, read_attempt_log_file_tail,
};
pub use assistant_log_buffer::{
    append_assistant_log, get_assistant_log_file_path, parse_jsonl_to_messages,
    parse_tool_call_metadata, read_assistant_log_file, AgentTextBuffer, AssistantMessage,
};
pub use claude::*;
pub use codex::*;
pub use cursor::*;
pub use diff_snapshot::{AttemptDiffSnapshot, DiffStorageUploader, FileDiffData};
pub use document_tasks::publish_docs_task_to_vault;
pub use gemini::*;
pub use git_credential_helper::GitCredentialHelper;
pub use input_queue::{InputMessage, InputQueue, InputQueueError};
pub use job_queue::*;
pub use knowledge_index::{
    discover_global_skill_roots, IndexedKnowledgeBackend, KnowledgeIndex, KnowledgeRoot,
    SkillKnowledgeBackend, SkillKnowledgeHandle, SkillKnowledgeSnapshot, SkillKnowledgeStatus,
    SkillMatch,
};
pub use normalization::{
    ActionOperation, ActionType, AggregatedAction, ExecutionStatus, FileChange, FileChangeType,
    LogNormalizer, NormalizedEntry, NormalizedEntryType, SubagentSpawn, TodoItem, TodoStatus,
    ToolStatus,
};
pub use normalization_contract::validate_sdk_normalized_entry;
pub use orchestrator::*;
pub use orchestrator_gitops::*;
pub use orchestrator_status::*;
pub use process::*;
pub use project_vault::{
    format_project_vault_search_follow_up, format_project_vault_search_summary,
    search_project_vault, RuntimeProjectVaultSearchMatch, RuntimeProjectVaultSearchResult,
};
pub use retry_handler::{RetryHandler, RetryInfo, RetryScheduleResult};
pub use router_config::{
    default_filters, serialize_filters, AgentSettings, FilterAction, MessageFilter,
};
pub use skill_runtime::{
    PlannedSkill, RuntimeSkillLoadResult, RuntimeSkillSearchMatch, RuntimeSkillSearchResult,
    SkillPlan, SkillPlanDecision, SkillRuntime, SkillSelectionTrace, SkippedSkill,
};
pub use task_skills::{
    build_skill_instruction_block, build_skill_instruction_block_with_rag,
    build_skill_instruction_context, build_skill_metadata_patch, build_skill_plan,
    format_loaded_skills_log_line, get_runtime_skill_attachment, render_skill_instruction_context,
    resolve_skill_chain, RuntimeLoadedSkill, SkillInstructionContext, SuggestedSkill,
};
pub use worker_pool::*;
pub use worker_pool_config::*;
pub use worker_pool_executor::*;
pub use worktree::*;

// Re-export for testing and cross-module use
pub use orchestrator::{normalize_stderr_for_display, sanitize_log, should_skip_log_line};

use acpms_db::models::AttemptStatus;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessage {
    pub attempt_id: Uuid,
    pub log_type: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    /// Unique identifier for this log entry
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    /// When the log was created (for ordering)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    /// Tool name if this is a tool-related log
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusMessage {
    pub attempt_id: Uuid,
    pub status: AttemptStatus,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequestMessage {
    pub attempt_id: Uuid,
    pub tool_use_id: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessageEvent {
    pub attempt_id: Uuid,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantLogMessage {
    pub session_id: Uuid,
    pub id: Uuid,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentEvent {
    Log(LogMessage),
    Status(StatusMessage),
    ApprovalRequest(ApprovalRequestMessage),
    UserMessage(UserMessageEvent),
    #[serde(rename = "assistant_log")]
    AssistantLog(AssistantLogMessage),
}

/// Hook executed right before an attempt is marked as `Success`.
///
/// Server can inject deployment/metadata/report finalization here so success
/// is only persisted after post-run pipeline has completed.
#[async_trait]
pub trait AttemptSuccessHook: Send + Sync {
    async fn before_mark_success(&self, attempt_id: Uuid) -> Result<()>;
}

/// Prepares deploy context (SSH key, config) in worktree so the agent can deploy directly.
/// Used for Deploy task type: agent SSHs to server and runs deploy, no API call.
#[async_trait]
pub trait DeployContextPreparer: Send + Sync {
    async fn prepare(&self, attempt_id: Uuid, worktree_path: &std::path::Path) -> Result<()>;
}
