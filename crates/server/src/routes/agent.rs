use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::middleware::AuthUser;
use crate::services::agent_auth::{AuthFlowType, AuthSessionRecord, AuthSessionStatus};
use crate::services::agent_auth_adapter::{
    parse_auth_required_action as parse_auth_required_action_by_provider, parse_loopback_port,
};
use crate::{api::ApiResponse, error::ApiError, state::AppState};
use sqlx::PgPool;

const AUTH_SESSION_TTL_SECONDS: i64 = 5 * 60;
const AUTH_SUBMIT_RATE_LIMIT_MAX_ATTEMPTS: u32 = 10;
const AUTH_SUBMIT_RATE_LIMIT_WINDOW_SECONDS: i64 = 60;
const AUTH_VERIFY_POLL_INTERVAL_SECONDS: u64 = 2;
const AUTH_VERIFY_POLL_MAX_ATTEMPTS: usize = 30;
const AUTH_EXIT_SUCCESS_VERIFY_RETRIES: usize = 4;
const AUTH_EXIT_SUCCESS_VERIFY_RETRY_DELAY_SECONDS: u64 = 1;
const AGENT_UI_AUTH_ENABLED_ENV: &str = "AGENT_UI_AUTH_ENABLED";
const ACPMS_AGENT_CLAUDE_BIN_ENV: &str = "ACPMS_AGENT_CLAUDE_BIN";
const ACPMS_AGENT_CODEX_BIN_ENV: &str = "ACPMS_AGENT_CODEX_BIN";
const ACPMS_AGENT_GEMINI_BIN_ENV: &str = "ACPMS_AGENT_GEMINI_BIN";
const ACPMS_AGENT_CURSOR_BIN_ENV: &str = "ACPMS_AGENT_CURSOR_BIN";
const ACPMS_AGENT_NPX_BIN_ENV: &str = "ACPMS_AGENT_NPX_BIN";
const ACPMS_GEMINI_HOME_ENV: &str = "ACPMS_GEMINI_HOME";

/// Agent provider status response (legacy endpoint response)
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentStatusResponse {
    /// Provider name (e.g., "claude-code")
    pub provider: String,
    /// Whether the agent is connected/authenticated
    pub connected: bool,
    /// Status message
    pub message: String,
    /// Session info (if connected)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_info: Option<SessionInfo>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SessionInfo {
    /// Path to session directory
    pub session_dir: String,
    /// Number of projects found
    pub project_count: usize,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthState {
    Authenticated,
    Unauthenticated,
    Expired,
    Unknown,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAvailabilityReason {
    Ok,
    CliMissing,
    NotAuthenticated,
    AuthExpired,
    AuthCheckFailed,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProviderStatusDoc {
    pub provider: String,
    pub installed: bool,
    pub auth_state: ProviderAuthState,
    pub available: bool,
    pub reason: ProviderAvailabilityReason,
    pub message: String,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentProvidersStatusResponse {
    pub default_provider: String,
    pub providers: Vec<ProviderStatusDoc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct InitiateAgentAuthRequest {
    pub provider: String,
    /// If true and provider is gemini-cli: remove ~/.gemini/oauth_creds.json so the next auth run prompts for login (Gemini has no logout command).
    #[serde(default)]
    pub force_reauth: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitAgentAuthCodeRequest {
    pub session_id: Uuid,
    pub code: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CancelAgentAuthRequest {
    pub session_id: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentAuthSessionDoc {
    pub session_id: Uuid,
    pub provider: String,
    pub flow_type: AuthFlowType,
    pub status: AuthSessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub process_pid: Option<u32>,
    pub allowed_loopback_port: Option<u16>,
    pub last_seq: u64,
    pub last_error: Option<String>,
    pub result: Option<String>,
    pub action_url: Option<String>,
    pub action_code: Option<String>,
    pub action_hint: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SubmitAgentAuthCodeResponse {
    pub session_id: Uuid,
    pub status: AuthSessionStatus,
    pub accepted: bool,
    pub message: String,
}

impl From<AuthSessionRecord> for AgentAuthSessionDoc {
    fn from(value: AuthSessionRecord) -> Self {
        Self {
            session_id: value.session_id,
            provider: value.provider,
            flow_type: value.flow_type,
            status: value.status,
            created_at: value.created_at,
            updated_at: value.updated_at,
            expires_at: value.expires_at,
            process_pid: value.process_pid,
            allowed_loopback_port: value.allowed_loopback_port,
            last_seq: value.last_seq,
            last_error: value.last_error,
            result: value.result,
            action_url: value.action_url,
            action_code: value.action_code,
            action_hint: value.action_hint,
        }
    }
}

fn is_terminal_session_status(status: AuthSessionStatus) -> bool {
    matches!(
        status,
        AuthSessionStatus::Succeeded
            | AuthSessionStatus::Failed
            | AuthSessionStatus::Cancelled
            | AuthSessionStatus::TimedOut
    )
}

/// Get legacy selected provider status.
///
/// This endpoint is kept for backward compatibility. For the new UI use
/// `GET /api/v1/agent/providers/status`.
#[utoipa::path(
    get,
    path = "/api/v1/agent/status",
    tag = "Agent",
    responses(
        (status = 200, description = "Agent status retrieved", body = ApiResponse<AgentStatusResponse>),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_agent_status(
    _auth_user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<AgentStatusResponse>>, ApiError> {
    let settings = state
        .settings_service
        .get_response()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let provider = settings.agent_cli_provider.clone();
    let provider_status = check_provider_status(&provider).await;

    let status = AgentStatusResponse {
        provider: provider.clone(),
        connected: provider_status.available,
        message: provider_status.message,
        session_info: if provider == "claude-code" && provider_status.available {
            get_claude_session_info()
        } else {
            None
        },
    };

    Ok(Json(ApiResponse::success(status, "Agent status retrieved")))
}

/// Get status for all supported providers.
#[utoipa::path(
    get,
    path = "/api/v1/agent/providers/status",
    tag = "Agent",
    responses(
        (status = 200, description = "Agent providers status retrieved", body = ApiResponse<AgentProvidersStatusResponse>),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_provider_statuses(
    _auth_user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<AgentProvidersStatusResponse>>, ApiError> {
    let settings = state
        .settings_service
        .get_response()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let providers_to_check = ["claude-code", "openai-codex", "gemini-cli", "cursor-cli"];
    let checks = providers_to_check
        .iter()
        .map(|provider| check_provider_status(provider));
    let providers = join_all(checks).await;

    let response = AgentProvidersStatusResponse {
        default_provider: settings.agent_cli_provider,
        providers,
    };

    Ok(Json(ApiResponse::success(
        response,
        "Agent providers status retrieved",
    )))
}

/// Initiate an auth session for provider flow.
#[utoipa::path(
    post,
    path = "/api/v1/agent/auth/initiate",
    tag = "Agent",
    request_body = InitiateAgentAuthRequest,
    responses(
        (status = 200, description = "Auth session initiated", body = ApiResponse<AgentAuthSessionDoc>),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn initiate_agent_auth(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<InitiateAgentAuthRequest>,
) -> Result<Json<ApiResponse<AgentAuthSessionDoc>>, ApiError> {
    ensure_agent_ui_auth_enabled()?;
    let provider = normalize_agent_cli_provider(req.provider.trim());
    if !is_supported_provider(&provider) {
        return Err(ApiError::BadRequest(format!(
            "Unsupported provider '{}'. Expected one of: claude-code, openai-codex, gemini-cli, cursor-cli",
            provider
        )));
    }

    if provider == "gemini-cli" {
        if req.force_reauth == Some(true) {
            clear_gemini_oauth_creds().await;
        }
        let provider_status = check_provider_status(&provider).await;
        let skip_already_available = req.force_reauth == Some(true);
        if provider_status.available && !skip_already_available {
            let flow_type = default_flow_type_for_provider(&provider);
            let session = state
                .auth_session_store
                .create_session(
                    auth_user.id,
                    provider.clone(),
                    flow_type,
                    AUTH_SESSION_TTL_SECONDS,
                )
                .await;
            // Do not set action_url: bare Google OAuth URL would 400 (missing response_type, client_id, etc.)
            let _ = state
                .auth_session_store
                .update_action(
                    session.session_id,
                    None,
                    None,
                    Some("Gemini CLI is already available. To switch account: run `gemini`, type /auth to change account, then refresh. Or remove ~/.gemini/ (or ~/.gemini/oauth_creds.json) and run `gemini` to sign in again (no logout command).".to_string()),
                    None,
                )
                .await;
            let session = state
                .auth_session_store
                .update_status(
                    session.session_id,
                    AuthSessionStatus::Succeeded,
                    None,
                    Some(
                        "Gemini CLI is already available; no additional sign-in required"
                            .to_string(),
                    ),
                )
                .await
                .unwrap_or(session);

            let _ = append_agent_auth_audit_event(
                &state.db,
                auth_user.id,
                "agent_auth_already_available",
                session.session_id,
                provider.as_str(),
                "succeeded",
                Some(json!({
                    "provider_status_message": provider_status.message,
                })),
            )
            .await;

            return Ok(Json(ApiResponse::success(
                AgentAuthSessionDoc::from(session),
                format!("Auth session completed for provider '{}'", provider),
            )));
        }
    }

    if provider == "cursor-cli" {
        if req.force_reauth == Some(true) {
            cursor_logout().await;
        }
        let provider_status = check_provider_status(&provider).await;
        let skip_already_available = req.force_reauth == Some(true);
        if provider_status.available && !skip_already_available {
            let flow_type = default_flow_type_for_provider(&provider);
            let session = state
                .auth_session_store
                .create_session(
                    auth_user.id,
                    provider.clone(),
                    flow_type,
                    AUTH_SESSION_TTL_SECONDS,
                )
                .await;
            let _ = state
                .auth_session_store
                .update_action(
                    session.session_id,
                    None,
                    None,
                    Some("Cursor CLI is already available. To switch account: run `agent logout` then `agent login` in terminal, then refresh provider status.".to_string()),
                    None,
                )
                .await;
            let session = state
                .auth_session_store
                .update_status(
                    session.session_id,
                    AuthSessionStatus::Succeeded,
                    None,
                    Some(
                        "Cursor CLI is already available; no additional sign-in required"
                            .to_string(),
                    ),
                )
                .await
                .unwrap_or(session);

            let _ = append_agent_auth_audit_event(
                &state.db,
                auth_user.id,
                "agent_auth_already_available",
                session.session_id,
                provider.as_str(),
                "succeeded",
                Some(json!({
                    "provider_status_message": provider_status.message,
                })),
            )
            .await;

            return Ok(Json(ApiResponse::success(
                AgentAuthSessionDoc::from(session),
                format!("Auth session completed for provider '{}'", provider),
            )));
        }
    }

    let flow_type = default_flow_type_for_provider(&provider);
    let session = state
        .auth_session_store
        .create_session(
            auth_user.id,
            provider.clone(),
            flow_type,
            AUTH_SESSION_TTL_SECONDS,
        )
        .await;
    state
        .metrics
        .auth_session_started_total
        .with_label_values(&[provider.as_str()])
        .inc();

    let session = match launch_auth_process(state.clone(), session.clone()).await {
        Ok(session) => session,
        Err(err) => {
            let _ = state
                .auth_session_store
                .update_status(
                    session.session_id,
                    AuthSessionStatus::Failed,
                    Some(err.clone()),
                    None,
                )
                .await;
            state
                .metrics
                .auth_session_failed_total
                .with_label_values(&[provider.as_str()])
                .inc();
            let _ = append_agent_auth_audit_event(
                &state.db,
                auth_user.id,
                "agent_auth_initiate_failed",
                session.session_id,
                provider.as_str(),
                "failed",
                Some(json!({ "reason": err })),
            )
            .await;
            return Err(ApiError::BadRequest(err));
        }
    };

    let _ = append_agent_auth_audit_event(
        &state.db,
        auth_user.id,
        "agent_auth_initiated",
        session.session_id,
        provider.as_str(),
        "waiting_user_action",
        None,
    )
    .await;

    Ok(Json(ApiResponse::success(
        AgentAuthSessionDoc::from(session),
        format!("Auth session initiated for provider '{}'", provider),
    )))
}

/// Submit OOB code or callback URL to an existing auth session.
#[utoipa::path(
    post,
    path = "/api/v1/agent/auth/submit-code",
    tag = "Agent",
    request_body = SubmitAgentAuthCodeRequest,
    responses(
        (status = 200, description = "Auth code accepted", body = ApiResponse<SubmitAgentAuthCodeResponse>),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn submit_agent_auth_code(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<SubmitAgentAuthCodeRequest>,
) -> Result<Json<ApiResponse<SubmitAgentAuthCodeResponse>>, ApiError> {
    ensure_agent_ui_auth_enabled()?;
    let input = req.code.trim().to_string();
    if input.is_empty() {
        return Err(ApiError::BadRequest(
            "Auth code/callback payload cannot be empty".to_string(),
        ));
    }

    let current = state
        .auth_session_store
        .get_owned(req.session_id, auth_user.id)
        .await
        .ok_or_else(|| ApiError::NotFound("Auth session not found".to_string()))?;

    if current.expires_at < Utc::now() {
        let _ = state
            .auth_session_store
            .update_owned_status(
                req.session_id,
                auth_user.id,
                AuthSessionStatus::TimedOut,
                Some("Auth session expired before code submission".to_string()),
                None,
            )
            .await;
        return Err(ApiError::BadRequest("Auth session has expired".to_string()));
    }

    if is_terminal_session_status(current.status.clone()) {
        return Err(ApiError::BadRequest(
            "Auth session is already completed".to_string(),
        ));
    }

    state
        .auth_session_store
        .check_and_record_submit_attempt(
            req.session_id,
            AUTH_SUBMIT_RATE_LIMIT_MAX_ATTEMPTS,
            AUTH_SUBMIT_RATE_LIMIT_WINDOW_SECONDS,
        )
        .await
        .map_err(ApiError::BadRequest)?;

    if looks_like_loopback_callback(&input) {
        trigger_loopback_callback(&current, &input).await?;
    } else if looks_like_http_url(&input) {
        return Err(ApiError::BadRequest(
            "Only localhost callback URLs are allowed for auth submit".to_string(),
        ));
    } else {
        state
            .auth_session_store
            .write_to_stdin(req.session_id, &format!("{}\n", input))
            .await
            .map_err(ApiError::BadRequest)?;
    }

    let updated = state
        .auth_session_store
        .update_owned_status(
            req.session_id,
            auth_user.id,
            AuthSessionStatus::Verifying,
            None,
            Some("Auth input accepted".to_string()),
        )
        .await
        .ok_or_else(|| ApiError::NotFound("Auth session not found".to_string()))?;

    let _ = append_agent_auth_audit_event(
        &state.db,
        auth_user.id,
        "agent_auth_submit_code",
        updated.session_id,
        updated.provider.as_str(),
        "verifying",
        None,
    )
    .await;

    let verify_store = state.auth_session_store.clone();
    let verify_metrics = state.metrics.clone();
    let verify_db = state.db.clone();
    tokio::spawn(async move {
        poll_verifying_session_status(verify_store, verify_metrics, verify_db, updated.session_id)
            .await;
    });

    Ok(Json(ApiResponse::success(
        SubmitAgentAuthCodeResponse {
            session_id: updated.session_id,
            status: updated.status,
            accepted: true,
            message: "Auth input accepted for verification".to_string(),
        },
        "Auth code submitted",
    )))
}

async fn poll_verifying_session_status(
    store: std::sync::Arc<crate::services::agent_auth::AuthSessionStore>,
    metrics: crate::observability::Metrics,
    db: PgPool,
    session_id: Uuid,
) {
    for _ in 0..AUTH_VERIFY_POLL_MAX_ATTEMPTS {
        tokio::time::sleep(Duration::from_secs(AUTH_VERIFY_POLL_INTERVAL_SECONDS)).await;

        let Some(current) = store.get(session_id).await else {
            return;
        };
        if is_terminal_session_status(current.status.clone()) {
            return;
        }
        if current.status != AuthSessionStatus::Verifying {
            continue;
        }

        let provider_status = check_provider_status(&current.provider).await;
        if !provider_status.available {
            continue;
        }

        if let Some(pid) = current.process_pid {
            let _ = Command::new("kill")
                .arg(pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }

        let _ = store.set_stdin_writer(session_id, None).await;
        let _ = store.set_process_info(session_id, None, None).await;

        let Some(updated) = store
            .update_status(
                session_id,
                AuthSessionStatus::Succeeded,
                None,
                Some("Authentication verified by provider status".to_string()),
            )
            .await
        else {
            return;
        };

        metrics
            .auth_session_success_total
            .with_label_values(&[updated.provider.as_str()])
            .inc();
        let _ = append_agent_auth_audit_event(
            &db,
            updated.user_id,
            "agent_auth_succeeded",
            session_id,
            updated.provider.as_str(),
            "succeeded",
            Some(json!({
                "verification_source": "provider_status_poll",
                "provider_status_message": provider_status.message,
            })),
        )
        .await;
        return;
    }

    let Some(current) = store.get(session_id).await else {
        return;
    };
    if is_terminal_session_status(current.status.clone())
        || current.status != AuthSessionStatus::Verifying
    {
        return;
    }

    let Some(updated) = store
        .update_status(
            session_id,
            AuthSessionStatus::Failed,
            Some(
                "Authentication verification timed out. Please refresh provider status and try re-auth."
                    .to_string(),
            ),
            None,
        )
        .await
    else {
        return;
    };

    metrics
        .auth_session_failed_total
        .with_label_values(&[updated.provider.as_str()])
        .inc();
    let _ = append_agent_auth_audit_event(
        &db,
        updated.user_id,
        "agent_auth_failed",
        session_id,
        updated.provider.as_str(),
        "failed",
        Some(json!({
            "reason": "verification_poll_timeout",
            "max_attempts": AUTH_VERIFY_POLL_MAX_ATTEMPTS,
            "interval_seconds": AUTH_VERIFY_POLL_INTERVAL_SECONDS,
        })),
    )
    .await;
}

/// Cancel an existing auth session.
#[utoipa::path(
    post,
    path = "/api/v1/agent/auth/cancel",
    tag = "Agent",
    request_body = CancelAgentAuthRequest,
    responses(
        (status = 200, description = "Auth session cancelled", body = ApiResponse<AgentAuthSessionDoc>),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn cancel_agent_auth(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<CancelAgentAuthRequest>,
) -> Result<Json<ApiResponse<AgentAuthSessionDoc>>, ApiError> {
    ensure_agent_ui_auth_enabled()?;
    let current = state
        .auth_session_store
        .get_owned(req.session_id, auth_user.id)
        .await
        .ok_or_else(|| ApiError::NotFound("Auth session not found".to_string()))?;

    if is_terminal_session_status(current.status.clone()) {
        return Ok(Json(ApiResponse::success(
            AgentAuthSessionDoc::from(current),
            "Auth session already completed",
        )));
    }

    if let Some(pid) = current.process_pid {
        let _ = Command::new("kill")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }
    let _ = state
        .auth_session_store
        .set_stdin_writer(req.session_id, None)
        .await;
    let _ = state
        .auth_session_store
        .set_process_info(req.session_id, None, None)
        .await;

    let updated = state
        .auth_session_store
        .update_owned_status(
            req.session_id,
            auth_user.id,
            AuthSessionStatus::Cancelled,
            None,
            Some("Cancelled by user".to_string()),
        )
        .await
        .ok_or_else(|| ApiError::NotFound("Auth session not found".to_string()))?;

    let _ = append_agent_auth_audit_event(
        &state.db,
        auth_user.id,
        "agent_auth_cancelled",
        updated.session_id,
        updated.provider.as_str(),
        "cancelled",
        None,
    )
    .await;

    Ok(Json(ApiResponse::success(
        AgentAuthSessionDoc::from(updated),
        "Auth session cancelled",
    )))
}

/// Get a specific auth session by id.
#[utoipa::path(
    get,
    path = "/api/v1/agent/auth/sessions/{id}",
    tag = "Agent",
    params(
        ("id" = Uuid, Path, description = "Auth session ID")
    ),
    responses(
        (status = 200, description = "Auth session retrieved", body = ApiResponse<AgentAuthSessionDoc>),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_agent_auth_session(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ApiResponse<AgentAuthSessionDoc>>, ApiError> {
    ensure_agent_ui_auth_enabled()?;
    let mut session = state
        .auth_session_store
        .get_owned(session_id, auth_user.id)
        .await
        .ok_or_else(|| ApiError::NotFound("Auth session not found".to_string()))?;

    if session.expires_at < Utc::now() && !is_terminal_session_status(session.status.clone()) {
        if let Some(pid) = session.process_pid {
            let _ = Command::new("kill")
                .arg(pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }
        let _ = state
            .auth_session_store
            .set_stdin_writer(session_id, None)
            .await;
        let _ = state
            .auth_session_store
            .set_process_info(session_id, None, None)
            .await;
        if let Some(updated) = state
            .auth_session_store
            .update_owned_status(
                session_id,
                auth_user.id,
                AuthSessionStatus::TimedOut,
                Some("Auth session expired".to_string()),
                None,
            )
            .await
        {
            state
                .metrics
                .auth_session_timeout_total
                .with_label_values(&[updated.provider.as_str()])
                .inc();
            let _ = append_agent_auth_audit_event(
                &state.db,
                auth_user.id,
                "agent_auth_timed_out",
                updated.session_id,
                updated.provider.as_str(),
                "timed_out",
                None,
            )
            .await;
            session = updated;
        }
    }

    Ok(Json(ApiResponse::success(
        AgentAuthSessionDoc::from(session),
        "Auth session retrieved",
    )))
}

/// Spawns the provider's auth CLI and streams stdout/stderr to update action_url / action_code.
///
/// Why Gemini and Cursor feel slower than Codex/Claude:
/// - **Gemini:** Runs as `script -q /dev/null gemini` (pty). Two processes; pty output is
///   line-buffered so we only get lines after the child flushes. We send Enter after 2s to
///   advance to the auth screen; until then no URL/code is printed.
/// - **Cursor:** `agent login` is typically a Node/Electron binary; cold start (load runtime,
///   then print auth URL) can take several seconds. No server-side delay.
/// - **Codex/Claude:** Single process, device-flow or OAuth URL printed soon after start.
async fn launch_auth_process(
    state: AppState,
    session: AuthSessionRecord,
) -> Result<AuthSessionRecord, String> {
    let (cmd, args) = auth_command_for_provider(&session.provider)
        .await
        .ok_or_else(|| {
            format!(
                "No auth command configured for provider '{}'",
                session.provider
            )
        })?;

    let mut command = Command::new(&cmd);
    command
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if session.provider == "gemini-cli" {
        // Force manual URL/code flow so UI can always show actionable auth link/code.
        command.env("NO_BROWSER", "true");
        // Keep Gemini auth state under an explicit home when provided.
        if let Some(gemini_home) = read_non_empty_env(ACPMS_GEMINI_HOME_ENV) {
            command.env("HOME", gemini_home);
        }
        // Some headless service environments have no TERM; Gemini CLI may exit early.
        if std::env::var_os("TERM").is_none() {
            command.env("TERM", "xterm-256color");
        }
    }

    let mut child = command.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            return format!(
                "Provider CLI command '{}' is not installed or not available in PATH",
                cmd
            );
        }
        format!(
            "Failed to start auth process for '{}': {}",
            session.provider, e
        )
    })?;

    let process_pid = child.id();
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let _ = state
        .auth_session_store
        .set_process_info(session.session_id, process_pid, None)
        .await;
    let _ = state
        .auth_session_store
        .set_stdin_writer(session.session_id, stdin)
        .await;
    if session.provider == "gemini-cli" {
        // Do not set a bare Google OAuth URL (missing response_type etc. causes 400). Let CLI stdout set action_url/action_code via adapter.
        let _ = state
            .auth_session_store
            .update_action(
                session.session_id,
                None,
                None,
                Some("Gemini sign-in is starting; URL or device code may appear below in a few seconds. If nothing appears after ~30s, run `gemini` in a terminal to sign in, then refresh provider status.".to_string()),
                None,
            )
            .await;
        // Gemini runs inside `script` (pty); it shows a method-selection screen first. We send
        // Enter after a short delay so the process has time to start and display the screen;
        // then Enter selects default (Google login) and the CLI can print the URL/code. 2s is
        // a trade-off: too short and Enter may be lost, too long and the user waits unnecessarily.
        let store_stdin = state.auth_session_store.clone();
        let sid = session.session_id;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let _ = store_stdin.write_to_stdin(sid, "\n").await;
        });
    }

    if session.provider == "cursor-cli" {
        let _ = state
            .auth_session_store
            .update_action(
                session.session_id,
                None,
                None,
                Some("Cursor sign-in is starting; auth URL may appear below in a few seconds and will open in a new tab. If nothing appears after ~30s, run `agent login` in a terminal, then refresh provider status.".to_string()),
                None,
            )
            .await;
    }

    let updated = state
        .auth_session_store
        .update_status(
            session.session_id,
            AuthSessionStatus::WaitingUserAction,
            None,
            Some("Auth process started".to_string()),
        )
        .await
        .ok_or_else(|| "Auth session not found after spawn".to_string())?;

    let watchdog_store = state.auth_session_store.clone();
    let watchdog_session_id = session.session_id;
    let watchdog_metrics = state.metrics.clone();
    let watchdog_db = state.db.clone();
    let watchdog_sleep = (session.expires_at - Utc::now())
        .to_std()
        .unwrap_or_else(|_| std::time::Duration::from_secs(0));
    tokio::spawn(async move {
        tokio::time::sleep(watchdog_sleep).await;
        if let Some(current) = watchdog_store.get(watchdog_session_id).await {
            if is_terminal_session_status(current.status.clone()) {
                return;
            }
            if let Some(pid) = current.process_pid {
                let _ = Command::new("kill")
                    .arg(pid.to_string())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await;
            }
            let _ = watchdog_store
                .set_stdin_writer(watchdog_session_id, None)
                .await;
            let _ = watchdog_store
                .set_process_info(watchdog_session_id, None, None)
                .await;
            let _ = watchdog_store
                .update_status(
                    watchdog_session_id,
                    AuthSessionStatus::TimedOut,
                    Some("Auth session timed out".to_string()),
                    None,
                )
                .await;
            watchdog_metrics
                .auth_session_timeout_total
                .with_label_values(&[current.provider.as_str()])
                .inc();
            let _ = append_agent_auth_audit_event(
                &watchdog_db,
                current.user_id,
                "agent_auth_timed_out",
                watchdog_session_id,
                current.provider.as_str(),
                "timed_out",
                None,
            )
            .await;
        }
    });

    let store = state.auth_session_store.clone();
    let session_id = session.session_id;
    let provider = session.provider.clone();
    let session_user_id = session.user_id;
    let metrics = state.metrics.clone();
    let db = state.db.clone();
    tokio::spawn(async move {
        let stdout_handle = stdout.map(|stdout| {
            let store = store.clone();
            let provider = provider.clone();
            tokio::spawn(process_auth_stream_output(
                store, session_id, provider, stdout,
            ))
        });
        let stderr_handle = stderr.map(|stderr| {
            let store = store.clone();
            let provider = provider.clone();
            tokio::spawn(process_auth_stream_output(
                store, session_id, provider, stderr,
            ))
        });

        let wait_result = child.wait().await;

        if let Some(handle) = stdout_handle {
            let _ = handle.await;
        }
        if let Some(handle) = stderr_handle {
            let _ = handle.await;
        }

        // Process info no longer active after wait
        let _ = store.set_process_info(session_id, None, None).await;
        let _ = store.set_stdin_writer(session_id, None).await;

        // If a terminal state was already set elsewhere (cancel/timeout/succeeded/failed),
        // keep it unchanged.
        if let Some(current) = store.get(session_id).await {
            if is_terminal_session_status(current.status.clone()) {
                return;
            }
        }

        match wait_result {
            Ok(status) if status.success() => {
                let provider_status = verify_provider_status_with_retry(
                    &provider,
                    AUTH_EXIT_SUCCESS_VERIFY_RETRIES,
                    Duration::from_secs(AUTH_EXIT_SUCCESS_VERIFY_RETRY_DELAY_SECONDS),
                )
                .await;

                if provider_status.available {
                    let _ = store
                        .update_status(
                            session_id,
                            AuthSessionStatus::Succeeded,
                            None,
                            Some("Auth process completed successfully".to_string()),
                        )
                        .await;
                    metrics
                        .auth_session_success_total
                        .with_label_values(&[provider.as_str()])
                        .inc();
                    let _ = append_agent_auth_audit_event(
                        &db,
                        session_user_id,
                        "agent_auth_succeeded",
                        session_id,
                        provider.as_str(),
                        "succeeded",
                        Some(json!({
                            "verification_source": "provider_status_after_process_exit",
                            "provider_status_message": provider_status.message,
                        })),
                    )
                    .await;
                } else {
                    let _ = store
                        .update_status(
                            session_id,
                            AuthSessionStatus::Failed,
                            Some(format!(
                                "Auth process exited successfully but provider is not authenticated yet: {}",
                                provider_status.message
                            )),
                            None,
                        )
                        .await;
                    metrics
                        .auth_session_failed_total
                        .with_label_values(&[provider.as_str()])
                        .inc();
                    let _ = append_agent_auth_audit_event(
                        &db,
                        session_user_id,
                        "agent_auth_failed",
                        session_id,
                        provider.as_str(),
                        "failed",
                        Some(json!({
                            "reason": "provider_not_authenticated_after_process_exit",
                            "provider_status_message": provider_status.message,
                        })),
                    )
                    .await;
                }
            }
            Ok(status) => {
                let provider_status = verify_provider_status_with_retry(
                    &provider,
                    AUTH_EXIT_SUCCESS_VERIFY_RETRIES,
                    Duration::from_secs(AUTH_EXIT_SUCCESS_VERIFY_RETRY_DELAY_SECONDS),
                )
                .await;
                let error_message = if provider_status.message.trim().is_empty() {
                    format!("Auth process exited with status {:?}", status.code())
                } else {
                    format!(
                        "Auth process exited with status {:?}. {}",
                        status.code(),
                        provider_status.message
                    )
                };
                let _ = store
                    .update_status(
                        session_id,
                        AuthSessionStatus::Failed,
                        Some(error_message),
                        None,
                    )
                    .await;
                metrics
                    .auth_session_failed_total
                    .with_label_values(&[provider.as_str()])
                    .inc();
                let _ = append_agent_auth_audit_event(
                    &db,
                    session_user_id,
                    "agent_auth_failed",
                    session_id,
                    provider.as_str(),
                    "failed",
                    Some(json!({ "exit_code": status.code() })),
                )
                .await;
            }
            Err(err) => {
                let _ = store
                    .update_status(
                        session_id,
                        AuthSessionStatus::Failed,
                        Some(format!("Failed waiting for auth process: {}", err)),
                        None,
                    )
                    .await;
                metrics
                    .auth_session_failed_total
                    .with_label_values(&[provider.as_str()])
                    .inc();
                let _ = append_agent_auth_audit_event(
                    &db,
                    session_user_id,
                    "agent_auth_failed",
                    session_id,
                    provider.as_str(),
                    "failed",
                    Some(json!({ "wait_error": err.to_string() })),
                )
                .await;
            }
        }
    });

    Ok(updated)
}

async fn auth_command_for_provider(provider: &str) -> Option<(String, Vec<String>)> {
    match provider {
        "openai-codex" => {
            if let Some(cmd) = resolve_provider_cli_command("openai-codex") {
                return Some((cmd, owned_args(CODEX_AUTH_DEVICE_ARGS)));
            }
            let npx_cmd = resolve_npx_command()?;
            Some((npx_cmd, owned_args(CODEX_NPX_AUTH_DEVICE_ARGS)))
        }
        "claude-code" => {
            if let Some(cmd) = resolve_provider_cli_command("claude-code") {
                let args = resolve_claude_auth_args(false, None).await;
                return Some((cmd, owned_args(args)));
            }
            let npx_cmd = resolve_npx_command()?;
            let args = resolve_claude_auth_args(true, Some(npx_cmd.as_str())).await;
            Some((npx_cmd, owned_args(args)))
        }
        // Gemini CLI requires an interactive TTY for login.
        // Run via `script` and auto-pick `gemini auth` when CLI supports it.
        "gemini-cli" => {
            let script_cmd = resolve_command_in_path("script")?;
            if let Some(gemini_cmd) = resolve_provider_cli_command("gemini-cli") {
                let args = resolve_gemini_auth_script_args(Some(gemini_cmd.as_str()), None).await;
                return Some((script_cmd, args));
            }
            let npx_cmd = resolve_npx_command()?;
            let args = resolve_gemini_auth_script_args(None, Some(npx_cmd.as_str())).await;
            Some((script_cmd, args))
        }
        "cursor-cli" => resolve_provider_cli_command("cursor-cli")
            .map(|cmd| (cmd, owned_args(CURSOR_AUTH_LOGIN_ARGS))),
        _ => None,
    }
}

#[cfg(test)]
fn select_auth_command_for_provider(
    provider: &str,
    codex_available: bool,
    claude_available: bool,
    gemini_available: bool,
    claude_args: &'static [&'static str],
) -> Option<(&'static str, &'static [&'static str])> {
    match provider {
        "openai-codex" => {
            if codex_available {
                Some(("codex", CODEX_AUTH_DEVICE_ARGS))
            } else {
                Some(("npx", CODEX_NPX_AUTH_DEVICE_ARGS))
            }
        }
        "claude-code" => {
            if claude_available {
                Some(("claude", claude_args))
            } else {
                Some(("npx", claude_args))
            }
        }
        // Gemini CLI v0.1.x has no `auth` subcommand and requires an interactive TTY.
        // Run via `script` to allocate a pseudo-terminal for login flow.
        // Why slower than Codex/Claude: (1) script + gemini = 2 processes, (2) pty output is
        // often line-buffered so we only see output after newline, (3) we delay sending Enter
        // so the CLI can print the auth screen first.
        "gemini-cli" => {
            if gemini_available {
                Some(("script", GEMINI_SCRIPT_ARGS))
            } else {
                Some(("script", GEMINI_NPX_SCRIPT_ARGS))
            }
        }
        // Why slower than Codex/Claude: `agent` is often a Node/Electron wrapper; cold start
        // (load runtime, then print auth URL) can take several seconds.
        "cursor-cli" => Some(("agent", CURSOR_AUTH_LOGIN_ARGS)),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CliSemver {
    major: u64,
    minor: u64,
    patch: u64,
}

const CODEX_AUTH_DEVICE_ARGS: &[&str] = &["login", "--device-auth"];
const CODEX_NPX_AUTH_DEVICE_ARGS: &[&str] = &["-y", "@openai/codex", "login", "--device-auth"];
const CLAUDE_AUTH_LOGIN_ARGS: &[&str] = &["auth", "login"];
const CLAUDE_SETUP_TOKEN_ARGS: &[&str] = &["setup-token"];
const CLAUDE_NPX_AUTH_LOGIN_ARGS: &[&str] = &["-y", "@anthropic-ai/claude-code", "auth", "login"];
const CLAUDE_NPX_SETUP_TOKEN_ARGS: &[&str] = &["-y", "@anthropic-ai/claude-code", "setup-token"];
#[cfg(test)]
const GEMINI_SCRIPT_ARGS: &[&str] = &["-q", "/dev/null", "gemini"];
#[cfg(test)]
const GEMINI_NPX_SCRIPT_ARGS: &[&str] = &["-q", "/dev/null", "npx", "-y", "@google/gemini-cli"];
const CURSOR_AUTH_LOGIN_ARGS: &[&str] = &["login"];
const CLAUDE_OAUTH_URL_PREFIX: &str = "https://claude.ai/oauth/authorize";

fn owned_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

async fn resolve_claude_auth_args(use_npx: bool, npx_cmd: Option<&str>) -> &'static [&'static str] {
    let (version_cmd, version_args, timeout_secs) = if use_npx {
        (
            npx_cmd.unwrap_or("npx"),
            &["-y", "@anthropic-ai/claude-code", "--version"][..],
            20_u64,
        )
    } else {
        ("claude", &["--version"][..], 3_u64)
    };

    if let Ok(outcome) = run_command_probe(version_cmd, version_args, timeout_secs).await {
        let combined = format!("{}\n{}", outcome.stdout, outcome.stderr);
        if let Some(version) = parse_cli_semver(&combined) {
            if version.major >= 2 {
                if use_npx {
                    return CLAUDE_NPX_SETUP_TOKEN_ARGS;
                }
                return CLAUDE_SETUP_TOKEN_ARGS;
            }
        }
    }

    if use_npx {
        CLAUDE_NPX_AUTH_LOGIN_ARGS
    } else {
        CLAUDE_AUTH_LOGIN_ARGS
    }
}

fn parse_cli_semver(text: &str) -> Option<CliSemver> {
    let mut numbers: Vec<u64> = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
            continue;
        }

        if ch == '.' && !current.is_empty() {
            numbers.push(current.parse().ok()?);
            current.clear();
            continue;
        }

        if !current.is_empty() {
            numbers.push(current.parse().ok()?);
            current.clear();
            if numbers.len() >= 3 {
                break;
            }
        } else {
            numbers.clear();
        }
    }

    if !current.is_empty() && numbers.len() < 3 {
        numbers.push(current.parse().ok()?);
    }

    if numbers.len() < 3 {
        return None;
    }

    Some(CliSemver {
        major: numbers[0],
        minor: numbers[1],
        patch: numbers[2],
    })
}

fn is_agent_ui_auth_enabled() -> bool {
    match std::env::var(AGENT_UI_AUTH_ENABLED_ENV) {
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        ),
        Err(_) => true,
    }
}

/// Run `agent logout` so the next auth run will prompt for login (switch account).
/// No-op if `agent` is not available or logout fails (e.g. not logged in).
async fn cursor_logout() {
    let _ = run_command_probe("agent", &["logout"], 15).await;
    tracing::info!("Cursor CLI logout attempted for re-auth");
}

/// Remove Gemini CLI OAuth credentials so the next auth run will prompt for login.
/// Uses HOME or ACPMS_GEMINI_HOME. No-op if path cannot be determined or file is missing.
async fn clear_gemini_oauth_creds() {
    let home = std::env::var(ACPMS_GEMINI_HOME_ENV)
        .ok()
        .or_else(|| std::env::var("HOME").ok());
    let Some(home) = home else {
        tracing::warn!(
            "Cannot clear Gemini creds: HOME and {} not set",
            ACPMS_GEMINI_HOME_ENV
        );
        return;
    };
    let path = std::path::Path::new(&home)
        .join(".gemini")
        .join("oauth_creds.json");
    if path.exists() {
        if let Err(e) = tokio::fs::remove_file(&path).await {
            tracing::warn!(path = %path.display(), error = %e, "Failed to remove Gemini oauth_creds.json");
        } else {
            tracing::info!(path = %path.display(), "Cleared Gemini OAuth credentials for re-auth");
        }
    }
}

pub(crate) fn ensure_agent_ui_auth_enabled() -> Result<(), ApiError> {
    if is_agent_ui_auth_enabled() {
        return Ok(());
    }
    Err(ApiError::Forbidden(
        "Agent UI auth feature is disabled by server configuration".to_string(),
    ))
}

async fn process_auth_stream_output<R>(
    store: std::sync::Arc<crate::services::agent_auth::AuthSessionStore>,
    session_id: Uuid,
    provider: String,
    reader: R,
) where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    let mut pending_claude_oauth_url: Option<String> = None;
    let mut pending_cursor_oauth_url: Option<String> = None;

    while let Ok(Some(line)) = lines.next_line().await {
        let normalized_line = strip_ansi_sequences(&line).replace('\r', "");
        let trimmed_line = normalized_line.trim();

        if provider == "claude-code" {
            if let Some(start_url) = extract_claude_oauth_url_start(trimmed_line) {
                pending_claude_oauth_url = Some(start_url);
                continue;
            }

            if let Some(url) = pending_claude_oauth_url.as_mut() {
                if is_auth_url_continuation_fragment(trimmed_line) {
                    url.push_str(trimmed_line);
                    continue;
                }

                let finalized_url = pending_claude_oauth_url.take().unwrap_or_default();
                if !finalized_url.is_empty() {
                    let _ = store
                        .update_action(
                            session_id,
                            Some(finalized_url.clone()),
                            None,
                            Some("Complete auth in browser. If redirected to localhost and it fails, paste that localhost URL below.".to_string()),
                            parse_loopback_port(&finalized_url),
                        )
                        .await;
                }
            }
        }

        if provider == "cursor-cli" {
            if let Some(start_url) = extract_cursor_oauth_url_start(trimmed_line) {
                pending_cursor_oauth_url = Some(start_url);
                continue;
            }

            if let Some(url) = pending_cursor_oauth_url.as_mut() {
                if is_auth_url_continuation_fragment(trimmed_line) {
                    url.push_str(trimmed_line);
                    continue;
                }

                let finalized_url = pending_cursor_oauth_url.take().unwrap_or_default();
                if !finalized_url.is_empty() {
                    let _ = store
                        .update_action(
                            session_id,
                            Some(finalized_url),
                            None,
                            Some(
                                "Open this URL in browser to complete Cursor login. No need to paste callback."
                                    .to_string(),
                            ),
                            None,
                        )
                        .await;
                }
            }
        }

        if let Some(parsed) = parse_auth_required_action_by_provider(&provider, &normalized_line) {
            let _ = store
                .update_action(
                    session_id,
                    parsed.action_url,
                    parsed.action_code,
                    Some(parsed.action_hint),
                    parsed.allowed_loopback_port,
                )
                .await;
        }
    }

    if let Some(finalized_url) = pending_claude_oauth_url.take() {
        let _ = store
            .update_action(
                session_id,
                Some(finalized_url.clone()),
                None,
                Some("Complete auth in browser. If redirected to localhost and it fails, paste that localhost URL below.".to_string()),
                parse_loopback_port(&finalized_url),
            )
            .await;
    }

    if let Some(finalized_url) = pending_cursor_oauth_url.take() {
        let _ = store
            .update_action(
                session_id,
                Some(finalized_url),
                None,
                Some(
                    "Open this URL in browser to complete Cursor login. No need to paste callback."
                        .to_string(),
                ),
                None,
            )
            .await;
    }
}

fn strip_ansi_sequences(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            continue;
        }
        output.push(ch);
    }
    output
}

fn extract_claude_oauth_url_start(line: &str) -> Option<String> {
    let start = line.find(CLAUDE_OAUTH_URL_PREFIX)?;
    let mut url = line[start..].to_string();
    url.retain(|c| !c.is_whitespace());
    if url.starts_with(CLAUDE_OAUTH_URL_PREFIX) {
        Some(url)
    } else {
        None
    }
}

const CURSOR_OAUTH_URL_PREFIXES: [&str; 2] = [
    "https://cursor.com/loginDeepControl",
    "https://www.cursor.com/loginDeepControl",
];

fn extract_cursor_oauth_url_start(line: &str) -> Option<String> {
    for prefix in CURSOR_OAUTH_URL_PREFIXES {
        if let Some(start) = line.find(prefix) {
            let mut url = line[start..].to_string();
            url.retain(|c| !c.is_whitespace());
            if url.starts_with(prefix) {
                return Some(url);
            }
        }
    }
    None
}

fn is_auth_url_continuation_fragment(fragment: &str) -> bool {
    if fragment.is_empty() || fragment.contains(' ') {
        return false;
    }

    fragment.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                '-' | '.'
                    | '_'
                    | '~'
                    | ':'
                    | '/'
                    | '?'
                    | '#'
                    | '['
                    | ']'
                    | '@'
                    | '!'
                    | '$'
                    | '&'
                    | '\''
                    | '('
                    | ')'
                    | '*'
                    | '+'
                    | ','
                    | ';'
                    | '='
                    | '%'
            )
    })
}

#[derive(Debug)]
struct CommandProbeOutcome {
    success: bool,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

fn read_non_empty_env(var: &str) -> Option<String> {
    let value = std::env::var(var).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn resolve_provider_cli_command(provider: &str) -> Option<String> {
    let (default_cmd, override_env) = match provider {
        "claude-code" => ("claude", ACPMS_AGENT_CLAUDE_BIN_ENV),
        "openai-codex" => ("codex", ACPMS_AGENT_CODEX_BIN_ENV),
        "gemini-cli" => ("gemini", ACPMS_AGENT_GEMINI_BIN_ENV),
        "cursor-cli" => ("agent", ACPMS_AGENT_CURSOR_BIN_ENV),
        _ => return None,
    };
    resolve_command_with_override(default_cmd, override_env)
}

fn resolve_npx_command() -> Option<String> {
    if let Some(override_cmd) = read_non_empty_env(ACPMS_AGENT_NPX_BIN_ENV) {
        return resolve_command_in_path(&override_cmd);
    }
    resolve_command_in_path("npx")
}

fn resolve_command_with_override(default_cmd: &str, override_env: &str) -> Option<String> {
    if let Some(override_cmd) = read_non_empty_env(override_env) {
        return resolve_command_in_path(&override_cmd);
    }
    resolve_command_in_path(default_cmd)
}

fn resolve_command_in_path(command: &str) -> Option<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.contains(std::path::MAIN_SEPARATOR) {
        let path = std::path::Path::new(trimmed);
        if is_executable_file(path) {
            return Some(path.to_string_lossy().to_string());
        }
        return None;
    }

    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(trimmed);
        if is_executable_file(&candidate) {
            return Some(candidate.to_string_lossy().to_string());
        }
    }

    None
}

fn is_executable_file(path: &std::path::Path) -> bool {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

async fn run_command_probe(
    cmd: &str,
    args: &[&str],
    timeout_secs: u64,
) -> Result<CommandProbeOutcome, String> {
    let mut command = Command::new(cmd);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let timed = timeout(Duration::from_secs(timeout_secs), command.output()).await;
    match timed {
        Err(_) => Ok(CommandProbeOutcome {
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: true,
        }),
        Ok(Err(err)) => Err(err.to_string()),
        Ok(Ok(output)) => Ok(CommandProbeOutcome {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            timed_out: false,
        }),
    }
}

async fn run_command_probe_owned(
    cmd: &str,
    args: &[String],
    timeout_secs: u64,
) -> Result<CommandProbeOutcome, String> {
    let arg_refs: Vec<&str> = args.iter().map(|arg| arg.as_str()).collect();
    run_command_probe(cmd, &arg_refs, timeout_secs).await
}

async fn resolve_gemini_auth_script_args(
    gemini_cmd: Option<&str>,
    npx_cmd: Option<&str>,
) -> Vec<String> {
    let mut args = vec!["-q".to_string(), "/dev/null".to_string()];
    if let Some(cmd) = gemini_cmd {
        args.push(cmd.to_string());
        if gemini_supports_auth_subcommand(Some(cmd), None).await {
            args.push("auth".to_string());
        }
        return args;
    }

    if let Some(npx) = npx_cmd {
        args.push(npx.to_string());
        args.push("-y".to_string());
        args.push("@google/gemini-cli".to_string());
        if gemini_supports_auth_subcommand(None, Some(npx)).await {
            args.push("auth".to_string());
        }
    }

    args
}

async fn gemini_supports_auth_subcommand(gemini_cmd: Option<&str>, npx_cmd: Option<&str>) -> bool {
    let probe = if let Some(cmd) = gemini_cmd {
        run_command_probe(cmd, &["--help"], 8).await
    } else if let Some(npx) = npx_cmd {
        run_command_probe(npx, &["-y", "@google/gemini-cli", "--help"], 25).await
    } else {
        return false;
    };

    let Ok(outcome) = probe else {
        // Probe failed: prefer modern flow (`gemini auth`) because latest CLI uses it.
        return true;
    };
    if outcome.timed_out {
        return true;
    }

    let combined = format!("{}\n{}", outcome.stdout, outcome.stderr);
    if help_mentions_auth_subcommand(&combined) {
        return true;
    }

    // Help completed successfully and no `auth` token was exposed => legacy behavior.
    if outcome.success {
        return false;
    }

    // Inconclusive command output; prefer modern auth flow.
    true
}

fn help_mentions_auth_subcommand(output: &str) -> bool {
    output.split_whitespace().any(|token| {
        token
            .trim_matches(|c: char| !c.is_ascii_alphanumeric())
            .eq_ignore_ascii_case("auth")
    })
}

fn looks_like_loopback_callback(input: &str) -> bool {
    if parse_loopback_port(input).is_some() {
        return true;
    }

    let lower = input.trim().to_ascii_lowercase();
    lower.starts_with("http://localhost")
        || lower.starts_with("https://localhost")
        || lower.starts_with("http://127.0.0.1")
        || lower.starts_with("https://127.0.0.1")
}

fn looks_like_http_url(input: &str) -> bool {
    match url::Url::parse(input) {
        Ok(parsed) => matches!(parsed.scheme(), "http" | "https"),
        Err(_) => {
            let lower = input.trim().to_ascii_lowercase();
            lower.starts_with("http://") || lower.starts_with("https://")
        }
    }
}

async fn trigger_loopback_callback(
    session: &AuthSessionRecord,
    callback_url: &str,
) -> Result<(), ApiError> {
    let parsed = validate_loopback_callback_url(callback_url, session.allowed_loopback_port)?;
    let redacted_target = redact_callback_target(&parsed);

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| ApiError::Internal(format!("Failed to build HTTP client: {}", e)))?;

    client.get(parsed).send().await.map_err(|e| {
        let reason = if e.is_timeout() {
            "request timed out"
        } else if e.is_connect() {
            "connection failed"
        } else {
            "request failed"
        };
        ApiError::BadRequest(format!(
            "Failed to call callback URL {} ({})",
            redacted_target, reason
        ))
    })?;

    Ok(())
}

fn validate_loopback_callback_url(
    callback_url: &str,
    expected_port: Option<u16>,
) -> Result<url::Url, ApiError> {
    let parsed = url::Url::parse(callback_url)
        .map_err(|_| ApiError::BadRequest("Invalid callback URL format".to_string()))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| ApiError::BadRequest("Callback URL missing host".to_string()))?;
    if host != "127.0.0.1" && host != "localhost" {
        return Err(ApiError::BadRequest(
            "Callback URL must use localhost or 127.0.0.1".to_string(),
        ));
    }

    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| ApiError::BadRequest("Callback URL missing port".to_string()))?;
    if let Some(expected_port) = expected_port {
        if port != expected_port {
            return Err(ApiError::BadRequest(format!(
                "Callback port mismatch: expected {}, got {}",
                expected_port, port
            )));
        }
    }

    Ok(parsed)
}

fn redact_callback_target(parsed: &url::Url) -> String {
    let host = parsed.host_str().unwrap_or("localhost");
    let port = parsed
        .port_or_known_default()
        .map(|p| format!(":{}", p))
        .unwrap_or_default();
    let path = if parsed.path().is_empty() {
        "/"
    } else {
        parsed.path()
    };
    format!("{}://{}{}{}", parsed.scheme(), host, port, path)
}

fn first_non_empty_line(text: &str) -> Option<String> {
    text.lines()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn summarize_probe_output(stdout: &str, stderr: &str, fallback: &str) -> String {
    first_non_empty_line(stdout)
        .or_else(|| first_non_empty_line(stderr))
        .unwrap_or_else(|| fallback.to_string())
}

async fn append_agent_auth_audit_event(
    pool: &PgPool,
    user_id: Uuid,
    action: &str,
    session_id: Uuid,
    provider: &str,
    status: &str,
    extra_metadata: Option<serde_json::Value>,
) -> Result<(), ApiError> {
    let mut metadata = serde_json::Map::new();
    metadata.insert("session_id".to_string(), json!(session_id));
    metadata.insert("provider".to_string(), json!(provider));
    metadata.insert("status".to_string(), json!(status));
    if let Some(extra) = extra_metadata {
        metadata.insert("details".to_string(), extra);
    }

    sqlx::query(
        r#"
        INSERT INTO audit_logs (user_id, action, resource_type, resource_id, metadata)
        VALUES ($1, $2, 'agent_auth_sessions', $3, $4::jsonb)
        "#,
    )
    .bind(user_id)
    .bind(action)
    .bind(session_id)
    .bind(serde_json::Value::Object(metadata))
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to write auth audit event: {}", e)))?;

    Ok(())
}

fn build_provider_status(
    provider: &str,
    installed: bool,
    auth_state: ProviderAuthState,
    reason: ProviderAvailabilityReason,
    message: String,
) -> ProviderStatusDoc {
    let available = installed && auth_state == ProviderAuthState::Authenticated;
    ProviderStatusDoc {
        provider: provider.to_string(),
        installed,
        auth_state,
        available,
        reason,
        message,
        checked_at: Utc::now(),
    }
}

async fn check_codex_provider_status() -> ProviderStatusDoc {
    let codex_cmd = resolve_provider_cli_command("openai-codex");
    let npx_cmd = resolve_npx_command();
    if codex_cmd.is_none() && npx_cmd.is_none() {
        return build_provider_status(
            "openai-codex",
            false,
            ProviderAuthState::Unknown,
            ProviderAvailabilityReason::CliMissing,
            "Codex CLI not found. Install: npm i -g @openai/codex (or ensure server PATH includes npm global bin)".to_string(),
        );
    }

    let (probe_cmd, probe_args, timeout_secs) = if let Some(cmd) = codex_cmd {
        (cmd, vec!["login".to_string(), "status".to_string()], 8_u64)
    } else {
        (
            npx_cmd.unwrap_or_default(),
            vec![
                "-y".to_string(),
                "@openai/codex".to_string(),
                "login".to_string(),
                "status".to_string(),
            ],
            20_u64,
        )
    };

    let probe = run_command_probe_owned(&probe_cmd, &probe_args, timeout_secs).await;
    let probe = match probe {
        Ok(result) => result,
        Err(err) => {
            return build_provider_status(
                "openai-codex",
                true,
                ProviderAuthState::Unknown,
                ProviderAvailabilityReason::AuthCheckFailed,
                format!("Failed to check Codex auth status: {}", err),
            )
        }
    };

    if probe.timed_out {
        return build_provider_status(
            "openai-codex",
            true,
            ProviderAuthState::Unknown,
            ProviderAvailabilityReason::AuthCheckFailed,
            "Timed out while checking Codex auth status".to_string(),
        );
    }

    if probe.success {
        return build_provider_status(
            "openai-codex",
            true,
            ProviderAuthState::Authenticated,
            ProviderAvailabilityReason::Ok,
            "Codex is available".to_string(),
        );
    }

    let output = format!(
        "{} {}",
        probe.stdout.to_lowercase(),
        probe.stderr.to_lowercase()
    );
    if output.contains("expired") {
        return build_provider_status(
            "openai-codex",
            true,
            ProviderAuthState::Expired,
            ProviderAvailabilityReason::AuthExpired,
            summarize_probe_output(&probe.stdout, &probe.stderr, "Codex credentials expired"),
        );
    }
    if output.contains("not logged")
        || output.contains("log in")
        || output.contains("login")
        || output.contains("authenticate")
    {
        return build_provider_status(
            "openai-codex",
            true,
            ProviderAuthState::Unauthenticated,
            ProviderAvailabilityReason::NotAuthenticated,
            summarize_probe_output(&probe.stdout, &probe.stderr, "Codex is not authenticated"),
        );
    }

    build_provider_status(
        "openai-codex",
        true,
        ProviderAuthState::Unknown,
        ProviderAvailabilityReason::AuthCheckFailed,
        format!(
            "Unable to determine Codex auth state (exit code: {:?})",
            probe.exit_code
        ),
    )
}

async fn check_claude_provider_status() -> ProviderStatusDoc {
    let claude_cmd = resolve_provider_cli_command("claude-code");
    let npx_cmd = resolve_npx_command();
    if claude_cmd.is_none() && npx_cmd.is_none() {
        return build_provider_status(
            "claude-code",
            false,
            ProviderAuthState::Unknown,
            ProviderAvailabilityReason::CliMissing,
            "Claude CLI not found. Install: npm install -g @anthropic-ai/claude-code (or ensure server PATH includes npm global bin)".to_string(),
        );
    }

    let (probe_cmd, probe_args, timeout_secs) = if let Some(cmd) = claude_cmd {
        (cmd, vec!["auth".to_string(), "status".to_string()], 8_u64)
    } else {
        (
            npx_cmd.unwrap_or_default(),
            vec![
                "-y".to_string(),
                "@anthropic-ai/claude-code".to_string(),
                "auth".to_string(),
                "status".to_string(),
            ],
            20_u64,
        )
    };

    let probe = run_command_probe_owned(&probe_cmd, &probe_args, timeout_secs).await;
    let probe = match probe {
        Ok(result) => result,
        Err(err) => {
            return build_provider_status(
                "claude-code",
                true,
                ProviderAuthState::Unknown,
                ProviderAvailabilityReason::AuthCheckFailed,
                format!("Failed to check Claude auth status: {}", err),
            )
        }
    };

    if probe.timed_out {
        return build_provider_status(
            "claude-code",
            true,
            ProviderAuthState::Unknown,
            ProviderAvailabilityReason::AuthCheckFailed,
            "Timed out while checking Claude auth status".to_string(),
        );
    }

    if probe.success {
        return build_provider_status(
            "claude-code",
            true,
            ProviderAuthState::Authenticated,
            ProviderAvailabilityReason::Ok,
            "Claude Code CLI is connected and ready".to_string(),
        );
    }

    let output = format!(
        "{} {}",
        probe.stdout.to_lowercase(),
        probe.stderr.to_lowercase()
    );
    if output.contains("token has expired") || output.contains("oauth token has expired") {
        return build_provider_status(
            "claude-code",
            true,
            ProviderAuthState::Expired,
            ProviderAvailabilityReason::AuthExpired,
            summarize_probe_output(&probe.stdout, &probe.stderr, "Claude token has expired"),
        );
    }
    if output.contains("failed to authenticate")
        || output.contains("not authenticated")
        || output.contains("login")
    {
        return build_provider_status(
            "claude-code",
            true,
            ProviderAuthState::Unauthenticated,
            ProviderAvailabilityReason::NotAuthenticated,
            summarize_probe_output(&probe.stdout, &probe.stderr, "Claude is not authenticated"),
        );
    }

    build_provider_status(
        "claude-code",
        true,
        ProviderAuthState::Unknown,
        ProviderAvailabilityReason::AuthCheckFailed,
        format!(
            "Unable to determine Claude auth state (exit code: {:?})",
            probe.exit_code
        ),
    )
}

async fn check_gemini_provider_status() -> ProviderStatusDoc {
    let gemini_cmd = resolve_provider_cli_command("gemini-cli");
    let npx_cmd = resolve_npx_command();
    if gemini_cmd.is_none() && npx_cmd.is_none() {
        return build_provider_status(
            "gemini-cli",
            false,
            ProviderAuthState::Unknown,
            ProviderAvailabilityReason::CliMissing,
            "Gemini CLI not found. Install: npm i -g @google/gemini-cli (or ensure server PATH includes npm global bin; macOS: brew install gemini-cli)".to_string(),
        );
    }

    let (probe_cmd, probe_args, timeout_secs) = if let Some(cmd) = gemini_cmd {
        (cmd, vec!["-p".to_string(), "ping".to_string()], 20_u64)
    } else {
        (
            npx_cmd.unwrap_or_default(),
            vec![
                "-y".to_string(),
                "@google/gemini-cli".to_string(),
                "-p".to_string(),
                "ping".to_string(),
            ],
            25_u64,
        )
    };

    let probe = run_command_probe_owned(&probe_cmd, &probe_args, timeout_secs).await;
    let probe = match probe {
        Ok(result) => result,
        Err(err) => {
            if gemini_api_key_configured() || has_gemini_local_credentials() {
                return build_provider_status(
                    "gemini-cli",
                    true,
                    ProviderAuthState::Authenticated,
                    ProviderAvailabilityReason::Ok,
                    format!(
                        "Gemini credentials detected on server (live probe failed to run: {})",
                        err
                    ),
                );
            }
            return build_provider_status(
                "gemini-cli",
                true,
                ProviderAuthState::Unknown,
                ProviderAvailabilityReason::AuthCheckFailed,
                format!("Failed to check Gemini auth state: {}", err),
            );
        }
    };

    if probe.timed_out {
        if gemini_api_key_configured() || has_gemini_local_credentials() {
            return build_provider_status(
                "gemini-cli",
                true,
                ProviderAuthState::Authenticated,
                ProviderAvailabilityReason::Ok,
                "Gemini credentials detected on server (live probe timed out)".to_string(),
            );
        }
        return build_provider_status(
            "gemini-cli",
            true,
            ProviderAuthState::Unknown,
            ProviderAvailabilityReason::AuthCheckFailed,
            "Timed out while checking Gemini auth state".to_string(),
        );
    }

    if probe.success {
        return build_provider_status(
            "gemini-cli",
            true,
            ProviderAuthState::Authenticated,
            ProviderAvailabilityReason::Ok,
            "Gemini CLI is available".to_string(),
        );
    }

    let output = format!(
        "{} {}",
        probe.stdout.to_lowercase(),
        probe.stderr.to_lowercase()
    );
    if output.contains("auth")
        || output.contains("authenticate")
        || output.contains("login")
        || output.contains("credential")
        || output.contains("api key")
        || output.contains("unauthorized")
        || output.contains("401")
    {
        return build_provider_status(
            "gemini-cli",
            true,
            ProviderAuthState::Unauthenticated,
            ProviderAvailabilityReason::NotAuthenticated,
            summarize_probe_output(&probe.stdout, &probe.stderr, "Gemini is not authenticated"),
        );
    }

    build_provider_status(
        "gemini-cli",
        true,
        ProviderAuthState::Unknown,
        ProviderAvailabilityReason::AuthCheckFailed,
        format!(
            "Unable to determine Gemini auth state (exit code: {:?})",
            probe.exit_code
        ),
    )
}

async fn check_cursor_provider_status() -> ProviderStatusDoc {
    let Some(cursor_cmd) = resolve_provider_cli_command("cursor-cli") else {
        return build_provider_status(
            "cursor-cli",
            false,
            ProviderAuthState::Unknown,
            ProviderAvailabilityReason::CliMissing,
            "Cursor CLI not found. Install: curl https://cursor.com/install -fsS | bash"
                .to_string(),
        );
    };

    // Use `agent status` (official auth check per Cursor docs); `agent -p "echo ok" --force`
    // can fail or return non-zero after login (e.g. prompt run behavior).
    let probe = match run_command_probe_owned(&cursor_cmd, &["status".to_string()], 15).await {
        Ok(result) => result,
        Err(err) => {
            return build_provider_status(
                "cursor-cli",
                true,
                ProviderAuthState::Unknown,
                ProviderAvailabilityReason::AuthCheckFailed,
                format!("Failed to check Cursor auth status: {}", err),
            )
        }
    };

    if probe.timed_out {
        return build_provider_status(
            "cursor-cli",
            true,
            ProviderAuthState::Unknown,
            ProviderAvailabilityReason::AuthCheckFailed,
            "Timed out while checking Cursor auth status".to_string(),
        );
    }

    if probe.success {
        return build_provider_status(
            "cursor-cli",
            true,
            ProviderAuthState::Authenticated,
            ProviderAvailabilityReason::Ok,
            "Cursor CLI is available".to_string(),
        );
    }

    let output = format!(
        "{} {}",
        probe.stdout.to_lowercase(),
        probe.stderr.to_lowercase()
    );
    if output.contains("login")
        || output.contains("authenticate")
        || output.contains("not logged")
        || output.contains("unauthorized")
        || output.contains("not authenticated")
    {
        return build_provider_status(
            "cursor-cli",
            true,
            ProviderAuthState::Unauthenticated,
            ProviderAvailabilityReason::NotAuthenticated,
            summarize_probe_output(
                &probe.stdout,
                &probe.stderr,
                "Cursor CLI is not authenticated",
            ),
        );
    }

    build_provider_status(
        "cursor-cli",
        true,
        ProviderAuthState::Unknown,
        ProviderAvailabilityReason::AuthCheckFailed,
        format!(
            "Unable to determine Cursor auth state (exit code: {:?})",
            probe.exit_code
        ),
    )
}

fn gemini_api_key_configured() -> bool {
    std::env::var("GEMINI_API_KEY")
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn has_gemini_local_credentials() -> bool {
    let home = std::env::var(ACPMS_GEMINI_HOME_ENV)
        .ok()
        .or_else(|| std::env::var("HOME").ok());
    let Some(home) = home else {
        return false;
    };
    let creds = std::path::Path::new(&home)
        .join(".gemini")
        .join("oauth_creds.json");
    if !creds.exists() {
        return false;
    }

    std::fs::metadata(&creds)
        .map(|meta| meta.is_file() && meta.len() > 0)
        .unwrap_or(false)
}

/// Canonical provider values: claude-code | openai-codex | gemini-cli | cursor-cli
pub(crate) fn normalize_agent_cli_provider(s: &str) -> String {
    match s.trim().to_lowercase().as_str() {
        "claude-code" => "claude-code".to_string(),
        "openai-codex" | "codex" => "openai-codex".to_string(),
        "gemini-cli" | "gemini" => "gemini-cli".to_string(),
        "cursor-cli" | "cursor" => "cursor-cli".to_string(),
        other => other.to_string(),
    }
}

async fn verify_provider_status_with_retry(
    provider: &str,
    retries: usize,
    retry_delay: Duration,
) -> ProviderStatusDoc {
    let attempts = retries.max(1);
    for attempt in 0..attempts {
        let status = check_provider_status(provider).await;
        if status.available || attempt + 1 == attempts {
            return status;
        }
        tokio::time::sleep(retry_delay).await;
    }

    check_provider_status(provider).await
}

pub(crate) async fn check_provider_status(provider: &str) -> ProviderStatusDoc {
    let normalized = normalize_agent_cli_provider(provider);
    match normalized.as_str() {
        "claude-code" => check_claude_provider_status().await,
        "openai-codex" => check_codex_provider_status().await,
        "gemini-cli" => check_gemini_provider_status().await,
        "cursor-cli" => check_cursor_provider_status().await,
        other => build_provider_status(
            other,
            false,
            ProviderAuthState::Unknown,
            ProviderAvailabilityReason::AuthCheckFailed,
            format!("Unknown provider '{}'", other),
        ),
    }
}

fn is_supported_provider(provider: &str) -> bool {
    matches!(
        normalize_agent_cli_provider(provider).as_str(),
        "claude-code" | "openai-codex" | "gemini-cli" | "cursor-cli"
    )
}

fn default_flow_type_for_provider(provider: &str) -> AuthFlowType {
    match provider {
        "openai-codex" => AuthFlowType::DeviceFlow,
        "claude-code" => AuthFlowType::LoopbackProxy,
        "gemini-cli" => AuthFlowType::Unknown,
        "cursor-cli" => AuthFlowType::DeviceFlow,
        _ => AuthFlowType::Unknown,
    }
}

fn get_claude_session_info() -> Option<SessionInfo> {
    let session_dir = std::env::var("CLAUDE_SESSION_DIR").unwrap_or_else(|_| {
        std::env::var("HOME")
            .map(|home| format!("{}/.claude", home))
            .unwrap_or_else(|_| "/root/.claude".to_string())
    });
    let session_path = std::path::Path::new(&session_dir);
    if !session_path.exists() {
        return None;
    }

    let projects_dir = session_path.join("projects");
    let project_count = if projects_dir.exists() {
        std::fs::read_dir(&projects_dir)
            .map(|entries| entries.filter_map(|entry| entry.ok()).count())
            .unwrap_or(0)
    } else {
        0
    };

    Some(SessionInfo {
        session_dir,
        project_count,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        extract_claude_oauth_url_start, extract_cursor_oauth_url_start,
        is_auth_url_continuation_fragment, looks_like_http_url, looks_like_loopback_callback,
        normalize_agent_cli_provider, parse_auth_required_action_by_provider, parse_cli_semver,
        parse_loopback_port, redact_callback_target, select_auth_command_for_provider,
        strip_ansi_sequences, validate_loopback_callback_url, CLAUDE_AUTH_LOGIN_ARGS,
        CLAUDE_NPX_AUTH_LOGIN_ARGS, CODEX_AUTH_DEVICE_ARGS, CODEX_NPX_AUTH_DEVICE_ARGS,
        GEMINI_NPX_SCRIPT_ARGS, GEMINI_SCRIPT_ARGS,
    };

    #[test]
    fn parse_codex_action_from_output_line() {
        let line =
            "First, copy your one-time code: ABCD-1234 then open https://github.com/login/device";
        let parsed =
            parse_auth_required_action_by_provider("openai-codex", line).expect("expected action");
        assert_eq!(
            parsed.action_url.as_deref(),
            Some("https://github.com/login/device")
        );
        assert_eq!(parsed.action_code.as_deref(), Some("ABCD-1234"));
        assert!(parsed.allowed_loopback_port.is_none());
    }

    #[test]
    fn parse_loopback_port_for_localhost() {
        let port = parse_loopback_port("http://127.0.0.1:55789/?code=abc");
        assert_eq!(port, Some(55789));
    }

    #[test]
    fn ignore_non_auth_lines() {
        let parsed = parse_auth_required_action_by_provider("claude-code", "waiting for auth...");
        assert!(parsed.is_none());
    }

    #[test]
    fn callback_validator_accepts_localhost() {
        let parsed =
            validate_loopback_callback_url("http://127.0.0.1:55432/?code=abc", Some(55432))
                .expect("localhost callback should be valid");
        assert_eq!(parsed.host_str(), Some("127.0.0.1"));
        assert_eq!(parsed.port_or_known_default(), Some(55432));
    }

    #[test]
    fn callback_validator_rejects_non_localhost_hosts() {
        let err = validate_loopback_callback_url("http://169.254.169.254:80/?code=abc", None)
            .expect_err("metadata host should be rejected");
        assert!(
            err.to_string()
                .contains("Callback URL must use localhost or 127.0.0.1"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn callback_validator_rejects_port_mismatch() {
        let err = validate_loopback_callback_url("http://localhost:4444/?code=abc", Some(5555))
            .expect_err("port mismatch should fail");
        assert!(
            err.to_string().contains("Callback port mismatch"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn detects_http_urls_for_submit_guard() {
        assert!(looks_like_http_url(
            "https://accounts.google.com/o/oauth2/auth"
        ));
        assert!(looks_like_http_url("http://example.com/callback"));
        assert!(looks_like_http_url("http://localhost:notaport/callback"));
        assert!(!looks_like_http_url("ABCD-1234"));
    }

    #[test]
    fn detects_localhost_callback_even_when_url_is_malformed() {
        assert!(looks_like_loopback_callback(
            "http://localhost:notaport/callback?code=abc"
        ));
        assert!(looks_like_loopback_callback(
            "https://127.0.0.1:notaport/callback"
        ));
    }

    #[test]
    fn redact_callback_target_strips_query_and_fragment() {
        let parsed =
            url::Url::parse("http://127.0.0.1:5000/callback?code=secret#frag").expect("url");
        let redacted = redact_callback_target(&parsed);
        assert_eq!(redacted, "http://127.0.0.1:5000/callback");
        assert!(!redacted.contains("secret"));
    }

    #[test]
    fn parse_cli_semver_from_version_string() {
        let parsed = parse_cli_semver("2.1.34 (Claude Code)").expect("semver");
        assert_eq!(parsed.major, 2);
        assert_eq!(parsed.minor, 1);
        assert_eq!(parsed.patch, 34);
    }

    #[test]
    fn strip_ansi_sequences_removes_escape_codes() {
        let raw = "\u{1b}[2J\u{1b}[3JBrowser didn't open?\u{1b}[0m";
        let clean = strip_ansi_sequences(raw);
        assert_eq!(clean, "Browser didn't open?");
    }

    #[test]
    fn extract_claude_oauth_url_start_detects_prefix() {
        let line = "https://claude.ai/oauth/authorize?code=true&client_id=abc";
        let extracted = extract_claude_oauth_url_start(line).expect("url");
        assert_eq!(extracted, line);
    }

    #[test]
    fn extract_cursor_oauth_url_start_detects_prefix() {
        let line =
            "Open this URL: https://cursor.com/loginDeepControl?challenge=abc&uuid=xyz&mode=login";
        let extracted = extract_cursor_oauth_url_start(line).expect("url");
        assert_eq!(
            extracted,
            "https://cursor.com/loginDeepControl?challenge=abc&uuid=xyz&mode=login"
        );
    }

    #[test]
    fn extract_cursor_oauth_url_start_handles_www_prefix() {
        let line = "https://www.cursor.com/loginDeepControl?challenge=abc";
        let extracted = extract_cursor_oauth_url_start(line).expect("url");
        assert_eq!(
            extracted,
            "https://www.cursor.com/loginDeepControl?challenge=abc"
        );
    }

    #[test]
    fn auth_url_continuation_fragment_validation() {
        assert!(is_auth_url_continuation_fragment(
            "44d1962f5e&response_type=code"
        ));
        assert!(is_auth_url_continuation_fragment(
            "&uuid=c581c686-cdd2-4b68-ad67-4812a282c4ea&mode=login&redirectTarget=cli"
        ));
        assert!(!is_auth_url_continuation_fragment("Paste code here >"));
    }

    #[test]
    fn auth_command_prefers_installed_codex_binary() {
        let (cmd, args) = select_auth_command_for_provider(
            "openai-codex",
            true,
            false,
            false,
            CLAUDE_AUTH_LOGIN_ARGS,
        )
        .expect("command");
        assert_eq!(cmd, "codex");
        assert_eq!(args, CODEX_AUTH_DEVICE_ARGS);
    }

    #[test]
    fn auth_command_falls_back_to_npx_for_codex() {
        let (cmd, args) = select_auth_command_for_provider(
            "openai-codex",
            false,
            false,
            false,
            CLAUDE_AUTH_LOGIN_ARGS,
        )
        .expect("command");
        assert_eq!(cmd, "npx");
        assert_eq!(args, CODEX_NPX_AUTH_DEVICE_ARGS);
    }

    #[test]
    fn auth_command_falls_back_to_npx_for_claude() {
        let (cmd, args) = select_auth_command_for_provider(
            "claude-code",
            false,
            false,
            false,
            CLAUDE_NPX_AUTH_LOGIN_ARGS,
        )
        .expect("command");
        assert_eq!(cmd, "npx");
        assert_eq!(args, CLAUDE_NPX_AUTH_LOGIN_ARGS);
    }

    #[test]
    fn auth_command_falls_back_to_npx_for_gemini() {
        let (cmd, args) = select_auth_command_for_provider(
            "gemini-cli",
            false,
            false,
            false,
            CLAUDE_AUTH_LOGIN_ARGS,
        )
        .expect("command");
        assert_eq!(cmd, "script");
        assert_eq!(args, GEMINI_NPX_SCRIPT_ARGS);
    }

    #[test]
    fn auth_command_uses_gemini_binary_when_available() {
        let (cmd, args) = select_auth_command_for_provider(
            "gemini-cli",
            false,
            false,
            true,
            CLAUDE_AUTH_LOGIN_ARGS,
        )
        .expect("command");
        assert_eq!(cmd, "script");
        assert_eq!(args, GEMINI_SCRIPT_ARGS);
    }

    #[test]
    fn normalize_provider_aliases_to_canonical_values() {
        assert_eq!(normalize_agent_cli_provider("codex"), "openai-codex");
        assert_eq!(normalize_agent_cli_provider("gemini"), "gemini-cli");
        assert_eq!(normalize_agent_cli_provider("cursor"), "cursor-cli");
        assert_eq!(normalize_agent_cli_provider("claude-code"), "claude-code");
    }

    #[test]
    fn help_parser_detects_auth_subcommand_token() {
        let help = r#"
            Commands:
              auth        Sign in to Gemini
              update      Check for updates
        "#;
        assert!(super::help_mentions_auth_subcommand(help));
    }

    #[test]
    fn help_parser_ignores_authenticate_word_without_auth_token() {
        let help = "Use this command to authenticate with Gemini CLI.";
        assert!(!super::help_mentions_auth_subcommand(help));
    }
}
