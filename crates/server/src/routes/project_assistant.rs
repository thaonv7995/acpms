//! Project Assistant session routes.
//! POST/GET sessions, session isolation (user_id == auth_user.id).

use acpms_db::models::{
    CreateRequirementRequest, CreateTaskRequest, ProjectAssistantSession, RequirementPriority,
    TaskType,
};
use acpms_executors::{
    append_assistant_log, parse_jsonl_to_messages, read_assistant_log_file, AgentEvent,
    AssistantLogMessage, AssistantMessage, ProjectAssistantJob,
};
use acpms_services::{
    apply_preferred_language_to_follow_up_input, build_instruction,
    AssistantMessage as ServiceAssistantMessage, AttachmentContent, ProjectAssistantSessionService,
    ProjectService, RequirementService, TaskService, TaskSummary,
};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::ApiResponse;
use crate::error::{ApiError, ApiResult};
use crate::middleware::{AuthUser, Permission, RbacChecker};
use crate::AppState;

const SESSION_START_MESSAGE: &str =
    "The project assistant session has just started. Greet the user briefly and confirm you are ready to help with this project.";

async fn read_session_log_bytes(
    state: &AppState,
    session: &ProjectAssistantSession,
) -> Result<Vec<u8>, ApiError> {
    if session.status == "ended" {
        if let Some(s3_key) = session.s3_log_key.as_ref() {
            return state
                .storage_service
                .get_log_bytes(s3_key)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()));
        }
    }

    read_assistant_log_file(session.id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

async fn resolve_attachments(
    state: &AppState,
    project_id: Uuid,
    attachments: Option<&[AttachmentRef]>,
) -> Result<Option<Vec<AttachmentContent>>, ApiError> {
    let Some(atts) = attachments.filter(|atts| !atts.is_empty()) else {
        return Ok(None);
    };

    const MAX_FILE_SIZE: u64 = 1_048_576;
    const MAX_FILES: usize = 5;
    let prefix = format!("projects/{}/assistant-attachments/", project_id);
    let mut resolved = Vec::with_capacity(atts.len().min(MAX_FILES));

    for att in atts.iter().take(MAX_FILES) {
        if !att.key.starts_with(&prefix) {
            continue;
        }
        if let Ok(Some((size, ct))) = state.storage_service.head_object_metadata(&att.key).await {
            if size > MAX_FILE_SIZE {
                continue;
            }
            if let Some(ref content_type) = ct {
                let allowed =
                    content_type.starts_with("text/") || content_type == "application/json";
                if !allowed {
                    continue;
                }
            }
        } else {
            continue;
        }
        if let Ok(bytes) = state.storage_service.get_log_bytes(&att.key).await {
            if let Ok(content) = String::from_utf8(bytes) {
                resolved.push(AttachmentContent {
                    filename: att.filename.clone().unwrap_or_else(|| "file".to_string()),
                    content,
                });
            }
        }
    }

    if resolved.is_empty() {
        Ok(None)
    } else {
        Ok(Some(resolved))
    }
}

async fn build_project_instruction(
    state: &AppState,
    project_id: Uuid,
    session_id: Uuid,
    user_message: &str,
    attachments: Option<&[AttachmentContent]>,
) -> Result<String, ApiError> {
    // Run all independent data fetches concurrently.
    let project_fut = {
        let svc = ProjectService::new(state.db.clone());
        async move { svc.get_project(project_id).await }
    };
    let requirements_fut = {
        let svc = RequirementService::new(state.db.clone());
        async move { svc.get_project_requirements(project_id).await }
    };
    let tasks_fut = {
        let svc = TaskService::new(state.db.clone());
        async move { svc.get_project_tasks(project_id).await }
    };
    let history_fut = read_assistant_log_file(session_id);
    let settings_fut = state.settings_service.get();

    let (project_res, requirements_res, tasks_res, history_res, settings_res) = tokio::join!(
        project_fut,
        requirements_fut,
        tasks_fut,
        history_fut,
        settings_fut
    );

    let project = project_res
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;

    let requirements = requirements_res
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .into_iter()
        .take(20)
        .collect::<Vec<_>>();

    let tasks = tasks_res
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .into_iter()
        .take(30)
        .map(|t| TaskSummary {
            title: t.title,
            description: t.description,
            status: format!("{:?}", t.status),
        })
        .collect::<Vec<_>>();

    let bytes = history_res.map_err(|e| ApiError::Internal(e.to_string()))?;
    let history = parse_jsonl_to_messages(&bytes)
        .into_iter()
        .filter(|m| m.role != "stderr")
        .map(|m| ServiceAssistantMessage {
            role: m.role,
            content: m.content,
        })
        .collect::<Vec<_>>();

    let preferred_language = settings_res.ok().and_then(|s| s.preferred_agent_language);

    Ok(build_instruction(
        &project,
        &requirements,
        &tasks,
        &history,
        user_message,
        attachments,
        preferred_language.as_deref(),
    ))
}

async fn archive_and_end_session(
    state: &AppState,
    service: &ProjectAssistantSessionService,
    session: &ProjectAssistantSession,
) -> Result<ProjectAssistantSession, ApiError> {
    if session.status == "ended" {
        return Ok(session.clone());
    }

    let _ = state
        .orchestrator
        .terminate_assistant_session(session.id)
        .await;

    let bytes = read_assistant_log_file(session.id)
        .await
        .unwrap_or_default();
    let s3_key = state
        .storage_service
        .upload_assistant_log_jsonl(session.id, &bytes)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to upload logs: {}", e)))?;

    service
        .end_session(session.id, &s3_key)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    service
        .get_session(session.id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))
}

/// Process ProjectAssistantJob (called by worker pool).
pub async fn process_project_assistant_job(state: AppState, job: ProjectAssistantJob) {
    match state
        .orchestrator
        .spawn_project_assistant_session(
            job.session_id,
            job.project_id,
            job.repo_path.clone(),
            job.instruction,
        )
        .await
    {
        Ok(()) => {
            // Agent sẽ trả lời greeting qua stdout (build_start_instruction), orchestrator stream vào assistant log.
        }
        Err(e) => {
            tracing::error!(
                session_id = %job.session_id,
                error = %e,
                "Project Assistant spawn failed"
            );
            let err_msg = format!("Error: {}", e);
            if let Ok(id) = append_assistant_log(job.session_id, "system", &err_msg, None).await {
                let created_at = chrono::Utc::now();
                let _ = state
                    .broadcast_tx
                    .send(AgentEvent::AssistantLog(AssistantLogMessage {
                        session_id: job.session_id,
                        id,
                        role: "system".to_string(),
                        content: err_msg,
                        metadata: None,
                        created_at,
                    }));
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionPayload {
    /// Default true: end active if any, create new. false: get_or_create.
    #[serde(default = "default_force_new")]
    pub force_new: bool,
}

fn default_force_new() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct AssistantSessionDto {
    pub id: Uuid,
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub status: String,
    pub s3_log_key: Option<String>,
    pub created_at: String,
    pub ended_at: Option<String>,
}

impl From<acpms_db::models::ProjectAssistantSession> for AssistantSessionDto {
    fn from(s: acpms_db::models::ProjectAssistantSession) -> Self {
        Self {
            id: s.id,
            project_id: s.project_id,
            user_id: s.user_id,
            status: s.status,
            s3_log_key: s.s3_log_key,
            created_at: s.created_at.to_rfc3339(),
            ended_at: s.ended_at.map(|d| d.to_rfc3339()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AssistantMessageDto {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
}

impl From<AssistantMessage> for AssistantMessageDto {
    fn from(m: AssistantMessage) -> Self {
        Self {
            id: m.id,
            session_id: m.session_id,
            role: m.role,
            content: m.content,
            metadata: m.metadata,
            created_at: m.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SessionWithMessagesDto {
    pub session: AssistantSessionDto,
    pub messages: Vec<AssistantMessageDto>,
}

/// POST /api/v1/projects/:project_id/assistant/sessions
pub async fn create_session(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<CreateSessionPayload>,
) -> ApiResult<(StatusCode, Json<ApiResponse<AssistantSessionDto>>)> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let service = ProjectAssistantSessionService::new(state.db.clone());
    let session = if payload.force_new {
        if let Some(active_session) = service
            .find_active_session(project_id, auth_user.id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
        {
            // Terminate the running agent and end the DB row synchronously so the
            // UNIQUE partial index (one active session per user/project) is satisfied
            // before the INSERT below.
            let _ = state
                .orchestrator
                .terminate_assistant_session(active_session.id)
                .await;
            service
                .end_session(
                    active_session.id,
                    active_session.s3_log_key.as_deref().unwrap_or(""),
                )
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            // Archive logs to S3 in the background (non-critical).
            let state_clone = state.clone();
            let old_session_id = active_session.id;
            tokio::spawn(async move {
                let bytes = read_assistant_log_file(old_session_id)
                    .await
                    .unwrap_or_default();
                match state_clone
                    .storage_service
                    .upload_assistant_log_jsonl(old_session_id, &bytes)
                    .await
                {
                    Ok(s3_key) => {
                        let svc = ProjectAssistantSessionService::new(state_clone.db.clone());
                        if let Err(e) = svc.update_s3_log_key(old_session_id, &s3_key).await {
                            tracing::error!(session_id = %old_session_id, "Failed to update s3_log_key after archive: {:?}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!(session_id = %old_session_id, "Failed to upload assistant logs to S3: {:?}", e);
                    }
                }
            });
        }
        service
            .create_session(project_id, auth_user.id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
    } else {
        service
            .get_or_create_session(project_id, auth_user.id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
    };

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::created(
            AssistantSessionDto::from(session),
            "Session created",
        )),
    ))
}

/// GET /api/v1/projects/:project_id/assistant/sessions
pub async fn list_sessions(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<AssistantSessionDto>>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    const KEEP_RECENT_SESSIONS: i64 = 3;
    let service = ProjectAssistantSessionService::new(state.db.clone());
    let sessions = service
        .list_sessions(project_id, auth_user.id, KEEP_RECENT_SESSIONS)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let dtos: Vec<AssistantSessionDto> = sessions
        .into_iter()
        .map(AssistantSessionDto::from)
        .collect();
    Ok(Json(ApiResponse::success(dtos, "Sessions retrieved")))
}

/// GET /api/v1/projects/:project_id/assistant/sessions/:session_id
pub async fn get_session(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, session_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<SessionWithMessagesDto>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let service = ProjectAssistantSessionService::new(state.db.clone());
    let session = service
        .get_session(session_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let session = session.ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;

    // Session isolation: only owner can access
    if session.user_id != auth_user.id {
        return Err(ApiError::Forbidden(
            "Session belongs to another user".to_string(),
        ));
    }
    if session.project_id != project_id {
        return Err(ApiError::Forbidden(
            "Session does not belong to this project".to_string(),
        ));
    }

    let bytes = read_session_log_bytes(&state, &session).await?;
    let messages = parse_jsonl_to_messages(&bytes);

    let dto = SessionWithMessagesDto {
        session: AssistantSessionDto::from(session),
        messages: messages
            .into_iter()
            .map(AssistantMessageDto::from)
            .collect(),
    };

    Ok(Json(ApiResponse::success(dto, "Session retrieved")))
}

/// POST /api/v1/projects/:project_id/assistant/sessions/:session_id/start
/// Spawn agent CLI với instruction khởi động. Agent trả lời greeting qua stdout, orchestrator stream vào assistant log.
pub async fn start_session(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, session_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<(StatusCode, Json<ApiResponse<()>>)> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let service = ProjectAssistantSessionService::new(state.db.clone());
    let session = service
        .get_session(session_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;

    if session.user_id != auth_user.id {
        return Err(ApiError::Forbidden(
            "Session belongs to another user".to_string(),
        ));
    }
    if session.project_id != project_id {
        return Err(ApiError::Forbidden(
            "Session does not belong to this project".to_string(),
        ));
    }
    if session.status != "active" {
        return Err(ApiError::BadRequest("Session is not active".to_string()));
    }

    if state
        .orchestrator
        .is_assistant_session_active(session_id)
        .await
    {
        return Ok((
            StatusCode::OK,
            Json(ApiResponse::success((), "Agent already running")),
        ));
    }

    let instruction =
        build_project_instruction(&state, project_id, session_id, SESSION_START_MESSAGE, None)
            .await?;

    let worktrees_path = state.worktrees_path.read().await.clone();
    let repo_path = worktrees_path.join(format!("assistant-{}-{}", project_id, auth_user.id));

    let job = ProjectAssistantJob {
        session_id,
        project_id,
        user_id: auth_user.id,
        repo_path,
        instruction,
    };

    if let Some(pool) = &state.project_assistant_worker_pool {
        pool.submit(job)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    } else {
        return Err(ApiError::Internal(
            "Project Assistant worker pool not available".to_string(),
        ));
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(ApiResponse::success((), "Agent starting")),
    ))
}

/// GET /api/v1/projects/:project_id/assistant/sessions/:session_id/status
/// Trả về { active: bool } - agent CLI có đang chạy không.
pub async fn get_session_status(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, session_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<serde_json::Value>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let service = ProjectAssistantSessionService::new(state.db.clone());
    let session = service
        .get_session(session_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;

    if session.user_id != auth_user.id {
        return Err(ApiError::Forbidden(
            "Session belongs to another user".to_string(),
        ));
    }
    if session.project_id != project_id {
        return Err(ApiError::Forbidden(
            "Session does not belong to this project".to_string(),
        ));
    }

    let active = state
        .orchestrator
        .is_assistant_session_active(session_id)
        .await;

    Ok(Json(ApiResponse::success(
        serde_json::json!({ "active": active }),
        "Session status",
    )))
}

#[derive(Debug, Deserialize)]
pub struct PostMessagePayload {
    pub content: String,
    #[serde(default)]
    pub attachments: Option<Vec<AttachmentRef>>,
}

#[derive(Debug, Deserialize)]
pub struct AttachmentRef {
    pub key: String,
    pub filename: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PostInputPayload {
    pub content: String,
}

/// POST /api/v1/projects/:project_id/assistant/sessions/:session_id/messages
pub async fn post_message(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, session_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<PostMessagePayload>,
) -> ApiResult<(StatusCode, Json<ApiResponse<()>>)> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let service = ProjectAssistantSessionService::new(state.db.clone());
    let session = service
        .get_session(session_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;

    if session.user_id != auth_user.id {
        return Err(ApiError::Forbidden(
            "Session belongs to another user".to_string(),
        ));
    }
    if session.project_id != project_id {
        return Err(ApiError::Forbidden(
            "Session does not belong to this project".to_string(),
        ));
    }
    if session.status != "active" {
        return Err(ApiError::BadRequest("Session is not active".to_string()));
    }

    // If CLI already running, return 409 - use POST input for follow-up
    if state
        .orchestrator
        .is_assistant_session_active(session_id)
        .await
    {
        return Err(ApiError::Conflict(
            "Use POST /input for follow-up messages".to_string(),
        ));
    }

    let attachments =
        resolve_attachments(&state, project_id, payload.attachments.as_deref()).await?;
    let instruction = build_project_instruction(
        &state,
        project_id,
        session_id,
        &payload.content,
        attachments.as_deref(),
    )
    .await?;

    append_assistant_log(session_id, "user", &payload.content, None)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let worktrees_path = state.worktrees_path.read().await.clone();
    let repo_path = worktrees_path.join(format!("assistant-{}-{}", project_id, auth_user.id));

    let job = ProjectAssistantJob {
        session_id,
        project_id,
        user_id: auth_user.id,
        repo_path,
        instruction,
    };

    if let Some(pool) = &state.project_assistant_worker_pool {
        pool.submit(job)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    } else {
        return Err(ApiError::Internal(
            "Project Assistant worker pool not available".to_string(),
        ));
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(ApiResponse::success((), "Message submitted")),
    ))
}

/// POST /api/v1/projects/:project_id/assistant/sessions/:session_id/input
pub async fn post_input(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, session_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<PostInputPayload>,
) -> ApiResult<Json<ApiResponse<()>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let service = ProjectAssistantSessionService::new(state.db.clone());
    let session = service
        .get_session(session_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;

    if session.user_id != auth_user.id {
        return Err(ApiError::Forbidden(
            "Session belongs to another user".to_string(),
        ));
    }
    if session.project_id != project_id {
        return Err(ApiError::Forbidden(
            "Session does not belong to this project".to_string(),
        ));
    }

    let preferred_language = state
        .settings_service
        .get()
        .await
        .ok()
        .and_then(|s| s.preferred_agent_language);
    let provider_input = apply_preferred_language_to_follow_up_input(
        &payload.content,
        preferred_language.as_deref(),
    );

    state
        .orchestrator
        .send_input_to_assistant_session(session_id, &provider_input)
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;

    // Append user message to JSONL
    append_assistant_log(session_id, "user", &payload.content, None)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(ApiResponse::success((), "Input sent")))
}

#[derive(Debug, Deserialize)]
pub struct AssistantAttachmentUploadUrlRequest {
    pub filename: String,
    pub content_type: String,
}

#[derive(Debug, Serialize)]
pub struct AssistantAttachmentUploadUrlResponse {
    pub upload_url: String,
    pub key: String,
}

fn sanitize_assistant_filename(filename: &str) -> String {
    let mut out = String::with_capacity(filename.len());
    for ch in filename.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "attachment.bin".to_string()
    } else {
        trimmed.to_string()
    }
}

/// POST /api/v1/projects/:project_id/assistant/attachments/upload-url
pub async fn get_assistant_attachment_upload_url(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(req): Json<AssistantAttachmentUploadUrlRequest>,
) -> ApiResult<Json<ApiResponse<AssistantAttachmentUploadUrlResponse>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let safe_name = sanitize_assistant_filename(&req.filename);
    let key = format!(
        "projects/{}/assistant-attachments/{}-{}",
        project_id,
        Uuid::new_v4(),
        safe_name
    );

    let upload_url = state
        .storage_service
        .get_presigned_upload_url(
            &key,
            &req.content_type,
            std::time::Duration::from_secs(3600),
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(ApiResponse::success(
        AssistantAttachmentUploadUrlResponse { upload_url, key },
        "Upload URL created",
    )))
}

#[derive(Debug, Deserialize)]
pub struct ConfirmToolPayload {
    pub tool_call_id: String,
    pub confirmed: bool,
}

/// POST /api/v1/projects/:project_id/assistant/sessions/:session_id/confirm-tool
pub async fn confirm_tool(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, session_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<ConfirmToolPayload>,
) -> ApiResult<Json<ApiResponse<serde_json::Value>>> {
    let service = ProjectAssistantSessionService::new(state.db.clone());
    let session = service
        .get_session(session_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;

    if session.user_id != auth_user.id {
        return Err(ApiError::Forbidden(
            "Session belongs to another user".to_string(),
        ));
    }
    if session.project_id != project_id {
        return Err(ApiError::Forbidden(
            "Session does not belong to this project".to_string(),
        ));
    }

    let bytes = read_session_log_bytes(&state, &session).await?;
    let messages = parse_jsonl_to_messages(&bytes);

    if let Some(existing) = messages.iter().rev().find_map(|message| {
        message.metadata.as_ref().and_then(|metadata| {
            let confirmation = metadata.get("tool_confirmation")?;
            let matches = confirmation
                .get("tool_call_id")
                .and_then(|value| value.as_str())
                .map(|tool_call_id| tool_call_id == payload.tool_call_id)
                .unwrap_or(false);
            matches.then(|| confirmation.clone())
        })
    }) {
        return Ok(Json(ApiResponse::success(
            existing,
            "Tool call already processed",
        )));
    }

    if session.status != "active" {
        return Err(ApiError::BadRequest("Session is not active".to_string()));
    }

    let tool_call = messages
        .iter()
        .rev()
        .filter_map(|m| {
            m.metadata.as_ref().and_then(|meta| {
                meta.get("tool_calls")
                    .and_then(|tc| tc.as_array())
                    .and_then(|arr| {
                        arr.iter().find(|t| {
                            t.get("id")
                                .and_then(|v| v.as_str())
                                .map(|s| s == payload.tool_call_id)
                                .unwrap_or(false)
                        })
                    })
            })
        })
        .next()
        .ok_or_else(|| ApiError::NotFound("Tool call not found".to_string()))?;

    let name = tool_call
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("Invalid tool call: missing name".to_string()))?;
    let args = tool_call
        .get("args")
        .and_then(|v| v.as_object())
        .ok_or_else(|| ApiError::BadRequest("Invalid tool call: missing args".to_string()))?;

    if payload.confirmed {
        if name == "create_requirement" {
            RbacChecker::check_permission(
                auth_user.id,
                project_id,
                Permission::CreateRequirement,
                &state.db,
            )
            .await?;

            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::BadRequest("Missing title".to_string()))?
                .to_string();
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let priority = args
                .get("priority")
                .and_then(|v| v.as_str())
                .map(|s| match s.to_lowercase().as_str() {
                    "low" => RequirementPriority::Low,
                    "high" => RequirementPriority::High,
                    "critical" => RequirementPriority::Critical,
                    _ => RequirementPriority::Medium,
                })
                .unwrap_or(RequirementPriority::Medium);

            let req = CreateRequirementRequest {
                project_id,
                sprint_id: None,
                title,
                content,
                priority: Some(priority),
                due_date: None,
                metadata: None,
            };

            let req_service = RequirementService::new(state.db.clone());
            let requirement = req_service
                .create_requirement(auth_user.id, req)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            let result = serde_json::json!({
                "tool_call_id": payload.tool_call_id,
                "confirmed": true,
                "entity_type": "requirement",
                "entity_id": requirement.id,
            });

            let log_content = format!(
                "User confirmed create_requirement: {} (id: {})",
                requirement.title, requirement.id
            );
            append_assistant_log(
                session_id,
                "system",
                &log_content,
                Some(&serde_json::json!({ "tool_confirmation": result })),
            )
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

            return Ok(Json(ApiResponse::success(result, "Requirement created")));
        } else if name == "create_task" {
            RbacChecker::check_permission(
                auth_user.id,
                project_id,
                Permission::CreateTask,
                &state.db,
            )
            .await?;

            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::BadRequest("Missing title".to_string()))?
                .to_string();
            let description = args
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from);
            let task_type = args
                .get("task_type")
                .and_then(|v| v.as_str())
                .map(|s| {
                    let s = s.to_lowercase();
                    match s.as_str() {
                        "bug" => TaskType::Bug,
                        "refactor" => TaskType::Refactor,
                        "docs" => TaskType::Docs,
                        "test" => TaskType::Test,
                        "chore" => TaskType::Chore,
                        "hotfix" => TaskType::Hotfix,
                        "spike" => TaskType::Spike,
                        "small_task" => TaskType::SmallTask,
                        _ => TaskType::Feature,
                    }
                })
                .unwrap_or(TaskType::Feature);
            let requirement_id = args
                .get("requirement_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let sprint_id = args
                .get("sprint_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());

            let req = CreateTaskRequest {
                project_id,
                requirement_id,
                sprint_id,
                title,
                description,
                task_type,
                assigned_to: None,
                metadata: None,
                parent_task_id: None,
            };

            let task_service = TaskService::new(state.db.clone());
            let task = task_service
                .create_task(auth_user.id, req)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            let result = serde_json::json!({
                "tool_call_id": payload.tool_call_id,
                "confirmed": true,
                "entity_type": "task",
                "entity_id": task.id,
            });

            let log_content = format!(
                "User confirmed create_task: {} (id: {})",
                task.title, task.id
            );
            append_assistant_log(
                session_id,
                "system",
                &log_content,
                Some(&serde_json::json!({ "tool_confirmation": result })),
            )
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

            return Ok(Json(ApiResponse::success(result, "Task created")));
        } else {
            return Err(ApiError::BadRequest(format!("Unknown tool: {}", name)));
        }
    } else {
        let result = serde_json::json!({
            "tool_call_id": payload.tool_call_id,
            "confirmed": false,
        });
        let log_content = format!("User rejected tool call {}: {}", payload.tool_call_id, name);
        append_assistant_log(
            session_id,
            "system",
            &log_content,
            Some(&serde_json::json!({ "tool_confirmation": result })),
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        Ok(Json(ApiResponse::success(result, "Tool call rejected")))
    }
}

/// POST /api/v1/projects/:project_id/assistant/sessions/:session_id/end
pub async fn end_session(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((project_id, session_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ApiResponse<AssistantSessionDto>>> {
    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let service = ProjectAssistantSessionService::new(state.db.clone());
    let session = service
        .get_session(session_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;

    if session.user_id != auth_user.id {
        return Err(ApiError::Forbidden(
            "Session belongs to another user".to_string(),
        ));
    }
    if session.project_id != project_id {
        return Err(ApiError::Forbidden(
            "Session does not belong to this project".to_string(),
        ));
    }
    if session.status == "ended" {
        return Ok(Json(ApiResponse::success(
            AssistantSessionDto::from(session),
            "Session already ended",
        )));
    }

    let updated = archive_and_end_session(&state, &service, &session).await?;

    Ok(Json(ApiResponse::success(
        AssistantSessionDto::from(updated),
        "Session ended",
    )))
}
