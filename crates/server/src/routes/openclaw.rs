use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json, Response,
    },
    routing::{get, post},
    Router,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    convert::Infallible,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio_stream::wrappers::BroadcastStream;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    api::{
        openapi_spec::{build_filtered_openclaw_openapi_json, OpenClawOpenApiQuery},
        ApiErrorDetail, ApiResponse, ResponseCode, TaskContextDto,
    },
    error::{ApiError, ApiResult},
    middleware::{AuthUser, Permission, RbacChecker, ValidatedJson},
    AppState,
};
use acpms_db::models::{Task, TaskStatus};
use acpms_services::{CreateTaskContextInput, TaskContextService, TaskService};

const OPENCLAW_HANDOFF_CONTRACT_VERSION: &str = "v1";
const OPENCLAW_CONNECTION_BUNDLE_FIELDS: &[&str] = &[
    "Base Endpoint URL",
    "OpenAPI (Swagger)",
    "Guide Endpoint",
    "Global Event SSE",
    "WebSocket Base",
    "API Key (Bearer)",
    "Webhook Secret",
];
const OPENCLAW_REQUIRED_FIRST_ACTIONS: &[&str] = &[
    "Store the API Key as the Bearer credential for ACPMS.",
    "Call the Guide Endpoint first and treat its response as the authoritative runtime guide.",
    "Load the OpenAPI document.",
    "Open and maintain the Global Event SSE connection.",
    "Use only ACPMS OpenClaw routes.",
    "Follow the ACPMS operating rules returned by the Guide Endpoint.",
];
const OPENCLAW_REQUIRED_ROUTE_PREFIXES: &[&str] = &["/api/openclaw/v1/*", "/api/openclaw/ws/*"];
const OPENCLAW_EVENT_REPLAY_PAGE_SIZE: i64 = 1000;
const OPENCLAW_REPORTING_REQUIREMENTS: &[&str] = &[
    "report important status, analyses, plans, started attempts, completed attempts, failed attempts, blocked work, and approval requests",
    "do not expose secrets, API keys, or webhook secrets in user-facing output",
    "what ACPMS currently says",
    "what you recommend",
    "what you already changed",
];

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct OpenClawGuideRequest {
    #[serde(default)]
    pub reporting: Option<OpenClawReportingRequest>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OpenClawReportingRequest {
    #[serde(default)]
    pub primary_user: Option<OpenClawPrimaryUserRequest>,
    #[serde(default)]
    pub channels: Vec<OpenClawReportingChannel>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OpenClawPrimaryUserRequest {
    pub display_name: Option<String>,
    pub timezone: Option<String>,
    pub preferred_language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpenClawReportingChannel {
    #[serde(rename = "type")]
    pub channel_type: String,
    pub target: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpenClawGuideResponse {
    pub instruction_prompt: String,
    pub core_missions: Vec<String>,
    pub acpms_profile: OpenClawAcpmsProfile,
    pub handoff_contract: OpenClawHandoffContract,
    pub operating_model: OpenClawOperatingModel,
    pub operating_rules: OpenClawOperatingRules,
    pub auth_rules: OpenClawAuthRules,
    pub reporting_policy: OpenClawReportingPolicy,
    pub connection_status: OpenClawConnectionStatus,
    pub setup_steps: Vec<String>,
    pub next_calls: Vec<OpenClawNextCall>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpenClawAcpmsProfile {
    pub product_name: String,
    pub role: String,
    pub base_endpoint_url: String,
    pub openapi_url: String,
    pub guide_url: String,
    pub events_stream_url: String,
    pub websocket_base_url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpenClawHandoffContract {
    pub contract_version: String,
    pub connection_bundle_fields: Vec<String>,
    pub required_first_actions: Vec<String>,
    pub required_route_prefixes: Vec<String>,
    pub reporting_requirements: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpenClawOperatingModel {
    pub role: String,
    pub primary_human_relationship: String,
    pub human_reporting_required: bool,
    pub preferred_reporting_channels: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpenClawOperatingRules {
    pub rulebook_version: String,
    pub default_autonomy_mode: String,
    pub must_load_acpms_context_before_mutation: bool,
    pub must_report_material_changes: bool,
    pub must_confirm_before_destructive_actions: bool,
    pub high_priority_report_events: Vec<String>,
    pub recommended_reporting_template: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpenClawAuthRules {
    pub rest_auth_header: String,
    pub event_stream_resume: String,
    pub webhook_signature_header: String,
    pub webhook_secret_usage: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpenClawReportingPolicy {
    pub report_to_primary_user: bool,
    pub notify_on: Vec<String>,
    pub channels: Vec<OpenClawReportingChannel>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpenClawConnectionStatus {
    pub primary_transport: String,
    pub webhook_registered: bool,
    pub missing_steps: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpenClawNextCall {
    pub method: String,
    pub path: String,
    pub purpose: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OpenClawEventStreamParams {
    pub after: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpenClawEventCursorExpiredData {
    error_type: &'static str,
    requested_after: i64,
    oldest_available_event_id: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpenClawGuideApiResponseDoc {
    pub success: bool,
    pub code: ResponseCode,
    pub message: String,
    pub data: Option<OpenClawGuideResponse>,
    pub metadata: Option<Value>,
    pub error: Option<ApiErrorDetail>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpenClawCursorExpiredApiResponseDoc {
    pub success: bool,
    pub code: ResponseCode,
    pub message: String,
    pub data: Option<OpenClawEventCursorExpiredData>,
    pub metadata: Option<Value>,
    pub error: Option<ApiErrorDetail>,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct OpenClawCreateTaskContextRequest {
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

struct OpenClawEventStreamDisconnectGuard {
    metrics: crate::observability::Metrics,
    after_cursor: Option<i64>,
    replay_count: usize,
    user_agent: Option<String>,
    forwarded_for: Option<String>,
}

impl Drop for OpenClawEventStreamDisconnectGuard {
    fn drop(&mut self) {
        self.metrics.openclaw_event_stream_active_connections.dec();
        tracing::info!(
            after_cursor = self.after_cursor,
            replay_count = self.replay_count,
            user_agent = self.user_agent.as_deref().unwrap_or("-"),
            forwarded_for = self.forwarded_for.as_deref().unwrap_or("-"),
            "OpenClaw event stream disconnected"
        );
    }
}

async fn fetch_task_for_openclaw_permission(
    state: &AppState,
    auth_user: &AuthUser,
    task_id: Uuid,
    permission: Permission,
) -> ApiResult<Task> {
    let task_service = TaskService::new(state.db.clone());
    let task = task_service
        .get_task(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    RbacChecker::check_permission(auth_user.id, task.project_id, permission, &state.db).await?;
    Ok(task)
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn infer_public_base_url(headers: &HeaderMap) -> String {
    if let Ok(value) = std::env::var("ACPMS_PUBLIC_URL") {
        let trimmed = value.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let host = header_value(headers, "x-forwarded-host")
        .or_else(|| header_value(headers, "host"))
        .unwrap_or_else(|| "localhost:3000".to_string());
    let proto = header_value(headers, "x-forwarded-proto").unwrap_or_else(|| {
        if host.starts_with("localhost") || host.starts_with("127.0.0.1") {
            "http".to_string()
        } else {
            "https".to_string()
        }
    });

    format!("{}://{}", proto, host)
}

fn to_websocket_base_url(base_url: &str) -> String {
    let websocket_origin = if let Some(rest) = base_url.strip_prefix("https://") {
        format!("wss://{}", rest)
    } else if let Some(rest) = base_url.strip_prefix("http://") {
        format!("ws://{}", rest)
    } else {
        format!("wss://{}", base_url)
    };

    format!("{}/api/openclaw/ws", websocket_origin.trim_end_matches('/'))
}

fn build_instruction_prompt(
    profile: &OpenClawAcpmsProfile,
    channels: &[OpenClawReportingChannel],
    display_name: Option<&str>,
    timezone: Option<&str>,
    preferred_language: Option<&str>,
) -> String {
    let channels_text = if channels.is_empty() {
        "Use the reporting channel already configured in OpenClaw for the primary user.".to_string()
    } else {
        let rendered = channels
            .iter()
            .map(|channel| format!("{} -> {}", channel.channel_type, channel.target))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "Use the configured reporting channels to keep the primary user informed: {}.",
            rendered
        )
    };

    let language_hint = preferred_language
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("Preferred human reporting language: {}.", value))
        .unwrap_or_default();
    let user_hint = display_name
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("Primary human user: {}.", value))
        .unwrap_or_default();
    let timezone_hint = timezone
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("Primary human timezone: {}.", value))
        .unwrap_or_default();

    format!(
        "You are OpenClaw connected to ACPMS (Agentic Coding Project Management System) as a trusted Super Admin integration.\n\nYour role:\n- Operate ACPMS through its OpenClaw gateway as an automation and control plane.\n- Behave as an operations assistant for the primary human user.\n- Load ACPMS context before proposing or executing work.\n- Analyze user requirements by combining them with ACPMS context such as projects, tasks, requirements, sprint state, attempt history, and architecture metadata.\n- Turn approved solutions into ACPMS actions such as creating requirements, creating tasks, and starting task attempts.\n- Report meaningful status, risk, and completion updates back to the primary user.\n\nACPMS connection rules:\n- Base API: {base_endpoint_url}\n- OpenAPI spec: {openapi_url}\n- Bootstrap guide: {guide_url}\n- Global event stream: {events_stream_url}\n- WebSocket base: {websocket_base_url}\n- Always authenticate with: Authorization: Bearer <OPENCLAW_API_KEY>\n- Use only /api/openclaw/v1/* and /api/openclaw/ws/* for ACPMS integration traffic.\n- Treat ACPMS as the source of truth.\n\nBootstrap workflow:\n1. Call the bootstrap guide and treat it as the authoritative runtime guide.\n2. Load the OpenAPI contract from the OpenClaw gateway.\n3. Open and maintain the global event SSE connection.\n4. Use ACPMS context before mutation.\n5. Report material actions, failures, blockers, and approvals to the primary user.\n\nOperating rules:\n- Default mode is analyze_then_confirm.\n- Read and analyze freely, but confirm before destructive or high-impact actions unless autonomous mode was explicitly enabled.\n- Distinguish clearly between ACPMS facts, your recommendation, and any ACPMS changes you already made.\n- Never expose secrets, bearer tokens, or webhook secrets in user-facing messages.\n\nReporting rules:\n- Report what the user asked.\n- Report what ACPMS context you checked.\n- Report what conclusion you reached.\n- Report what ACPMS action you took, if any.\n- Report current status and next step.\n- Report immediately on attempt start, completion, failure, needs input, approval requirement, or deployment risk.\n{channels_text}\n{user_hint}\n{timezone_hint}\n{language_hint}",
        base_endpoint_url = profile.base_endpoint_url,
        openapi_url = profile.openapi_url,
        guide_url = profile.guide_url,
        events_stream_url = profile.events_stream_url,
        websocket_base_url = profile.websocket_base_url,
        channels_text = channels_text,
        user_hint = user_hint,
        timezone_hint = timezone_hint,
        language_hint = language_hint,
    )
}

fn build_handoff_contract() -> OpenClawHandoffContract {
    OpenClawHandoffContract {
        contract_version: OPENCLAW_HANDOFF_CONTRACT_VERSION.to_string(),
        connection_bundle_fields: OPENCLAW_CONNECTION_BUNDLE_FIELDS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        required_first_actions: OPENCLAW_REQUIRED_FIRST_ACTIONS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        required_route_prefixes: OPENCLAW_REQUIRED_ROUTE_PREFIXES
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        reporting_requirements: OPENCLAW_REPORTING_REQUIREMENTS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    }
}

fn build_openclaw_guide_response(
    state: &AppState,
    headers: &HeaderMap,
    payload: OpenClawGuideRequest,
) -> ApiResult<Json<ApiResponse<OpenClawGuideResponse>>> {
    let base_url = infer_public_base_url(headers);
    let profile = OpenClawAcpmsProfile {
        product_name: "ACPMS".to_string(),
        role: "super_admin_integration".to_string(),
        base_endpoint_url: format!("{}/api/openclaw/v1", base_url),
        openapi_url: format!("{}/api/openclaw/openapi.json", base_url),
        guide_url: format!("{}/api/openclaw/guide-for-openclaw", base_url),
        events_stream_url: format!("{}/api/openclaw/v1/events/stream", base_url),
        websocket_base_url: to_websocket_base_url(&base_url),
    };

    let channels = payload
        .reporting
        .as_ref()
        .map(|reporting| reporting.channels.clone())
        .unwrap_or_default();
    let preferred_language = payload
        .reporting
        .as_ref()
        .and_then(|reporting| reporting.primary_user.as_ref())
        .and_then(|user| user.preferred_language.as_deref());
    let display_name = payload
        .reporting
        .as_ref()
        .and_then(|reporting| reporting.primary_user.as_ref())
        .and_then(|user| user.display_name.as_deref());
    let timezone = payload
        .reporting
        .as_ref()
        .and_then(|reporting| reporting.primary_user.as_ref())
        .and_then(|user| user.timezone.as_deref());

    let instruction_prompt = build_instruction_prompt(
        &profile,
        &channels,
        display_name,
        timezone,
        preferred_language,
    );
    let webhook_configured = state.openclaw_gateway.webhook_url.is_some()
        && state.openclaw_gateway.webhook_secret.is_some();

    let response = OpenClawGuideResponse {
        instruction_prompt,
        core_missions: vec![
            "Bootstrap ACPMS correctly by calling the guide first, loading the OpenAPI contract, storing the Bearer credential, and maintaining the global event stream connection.".to_string(),
            "Build and maintain situational awareness by reading ACPMS projects, requirements, sprints, tasks, attempts, execution processes, approvals, repository context, and recent events before proposing changes.".to_string(),
            "Translate human goals into explicit ACPMS execution plans with scope, dependencies, risks, acceptance criteria, and the smallest safe next actions.".to_string(),
            "Create or update ACPMS artifacts when appropriate, including requirements, tasks, attempts, and supporting metadata, so ACPMS stays aligned with the real execution plan.".to_string(),
            "Start, observe, and steer execution through ACPMS by tracking attempt state, blockers, approvals, failures, deployment risk, and completion signals.".to_string(),
            "Report to the primary human user in a structured way: what ACPMS says, what you concluded, what you changed, what is blocked, and what decision or next step is needed.".to_string(),
            "Protect ACPMS integrity by using only OpenClaw routes, treating ACPMS as the source of truth, and confirming before destructive or high-impact actions unless autonomous mode was explicitly enabled.".to_string(),
        ],
        acpms_profile: profile,
        handoff_contract: build_handoff_contract(),
        operating_model: OpenClawOperatingModel {
            role: "operations_assistant".to_string(),
            primary_human_relationship: "reporting_assistant".to_string(),
            human_reporting_required: true,
            preferred_reporting_channels: channels
                .iter()
                .map(|channel| channel.channel_type.clone())
                .collect(),
        },
        operating_rules: OpenClawOperatingRules {
            rulebook_version: "v1".to_string(),
            default_autonomy_mode: "analyze_then_confirm".to_string(),
            must_load_acpms_context_before_mutation: true,
            must_report_material_changes: true,
            must_confirm_before_destructive_actions: true,
            high_priority_report_events: vec![
                "attempt_started".to_string(),
                "attempt_completed".to_string(),
                "attempt_failed".to_string(),
                "attempt_needs_input".to_string(),
                "approval_required".to_string(),
                "deployment_risk".to_string(),
                "system_health_issue".to_string(),
            ],
            recommended_reporting_template: vec![
                "what the user asked".to_string(),
                "what ACPMS context was checked".to_string(),
                "what was concluded".to_string(),
                "what ACPMS action was taken, if any".to_string(),
                "current status".to_string(),
                "next step or approval needed".to_string(),
            ],
        },
        auth_rules: OpenClawAuthRules {
            rest_auth_header: "Authorization: Bearer <OPENCLAW_API_KEY>".to_string(),
            event_stream_resume: "Reconnect with Last-Event-ID or ?after=<event_id> when supported"
                .to_string(),
            webhook_signature_header: "X-Agentic-Signature".to_string(),
            webhook_secret_usage: "Use OPENCLAW_WEBHOOK_SECRET to verify HMAC-SHA256 signatures from ACPMS only when optional webhook delivery is enabled".to_string(),
        },
        reporting_policy: OpenClawReportingPolicy {
            report_to_primary_user: true,
            notify_on: vec![
                "attempt_started".to_string(),
                "attempt_completed".to_string(),
                "attempt_failed".to_string(),
                "approval_needed".to_string(),
                "deployment_risk".to_string(),
                "system_health_issue".to_string(),
            ],
            channels,
        },
        connection_status: OpenClawConnectionStatus {
            primary_transport: "sse_events_stream".to_string(),
            webhook_registered: webhook_configured,
            missing_steps: vec![
                "Load the OpenAPI contract".to_string(),
                "Open the global ACPMS event stream and keep it connected".to_string(),
            ],
        },
        setup_steps: vec![
            "Call the bootstrap guide and load its runtime policy".to_string(),
            "Load the OpenAPI contract".to_string(),
            "Open the global ACPMS event stream and keep it connected".to_string(),
            "Use ACPMS context when analyzing user requirements".to_string(),
            "Use mirrored /api/openclaw/v1 routes for ACPMS operations".to_string(),
            "Store the webhook secret only if optional ACPMS webhooks are enabled".to_string(),
        ],
        next_calls: vec![
            OpenClawNextCall {
                method: "GET".to_string(),
                path: "/api/openclaw/openapi.json".to_string(),
                purpose: "Load ACPMS tool surface".to_string(),
            },
            OpenClawNextCall {
                method: "GET".to_string(),
                path: "/api/openclaw/v1/events/stream".to_string(),
                purpose: "Subscribe to ACPMS lifecycle events".to_string(),
            },
            OpenClawNextCall {
                method: "GET".to_string(),
                path: "/api/openclaw/v1/projects".to_string(),
                purpose: "Validate project access and enumerate workspaces".to_string(),
            },
        ],
    };

    Ok(Json(ApiResponse::success(
        response,
        "OpenClaw bootstrap guide generated successfully",
    )))
}

pub async fn guide_for_openclaw(
    State(state): State<AppState>,
    headers: HeaderMap,
    _auth_user: AuthUser,
    payload: Option<Json<OpenClawGuideRequest>>,
) -> ApiResult<Json<ApiResponse<OpenClawGuideResponse>>> {
    build_openclaw_guide_response(
        &state,
        &headers,
        payload.map(|value| value.0).unwrap_or_default(),
    )
}

pub async fn guide_for_openclaw_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    _auth_user: AuthUser,
) -> ApiResult<Json<ApiResponse<OpenClawGuideResponse>>> {
    build_openclaw_guide_response(&state, &headers, OpenClawGuideRequest::default())
}

pub async fn openapi_json(
    _auth_user: AuthUser,
    Query(query): Query<OpenClawOpenApiQuery>,
) -> Json<Value> {
    Json(build_filtered_openclaw_openapi_json(&query))
}

pub async fn create_task_context_from_openclaw(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(task_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<OpenClawCreateTaskContextRequest>,
) -> ApiResult<Json<ApiResponse<TaskContextDto>>> {
    fetch_task_for_openclaw_permission(&state, &auth_user, task_id, Permission::ModifyTask).await?;

    let service = TaskContextService::new(state.db.clone());
    if service
        .count_task_contexts(task_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        >= 10
    {
        return Err(ApiError::BadRequest(
            "A task cannot have more than 10 context blocks".to_string(),
        ));
    }

    let context = service
        .create_task_context(
            task_id,
            auth_user.id,
            CreateTaskContextInput {
                title: req.title,
                content_type: req.content_type,
                raw_content: req.raw_content,
                source: "openclaw".to_string(),
                sort_order: req.sort_order,
            },
        )
        .await
        .map_err(|e| match e.to_string().as_str() {
            "Unsupported task context content_type" => {
                ApiError::BadRequest("Unsupported task context content_type".to_string())
            }
            "Unsupported task context source" => {
                ApiError::BadRequest("Unsupported task context source".to_string())
            }
            _ => ApiError::Internal(e.to_string()),
        })?;

    Ok(Json(ApiResponse::success(
        TaskContextDto::from_parts(context, Vec::new()),
        "Task context created successfully",
    )))
}

fn parse_resume_cursor(
    headers: &HeaderMap,
    params: &OpenClawEventStreamParams,
) -> Result<Option<i64>, crate::error::ApiError> {
    let header_cursor = header_value(headers, "last-event-id");
    if header_cursor.is_some() && params.after.is_some() {
        return Err(crate::error::ApiError::BadRequest(
            "Provide either Last-Event-ID or ?after=, not both".to_string(),
        ));
    }

    let raw = header_cursor.or_else(|| params.after.clone());
    raw.map(|value| {
        value
            .parse::<i64>()
            .map_err(|_| crate::error::ApiError::BadRequest("Invalid event cursor".to_string()))
    })
    .transpose()
}

fn to_sse_event(event: acpms_services::OpenClawGatewayEvent) -> Result<Event, Infallible> {
    let data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
    Ok(Event::default()
        .id(event.sequence_id.to_string())
        .event(event.event_type)
        .data(data))
}

fn event_cursor_expired_response(
    requested_after: i64,
    oldest_available_event_id: Option<i64>,
) -> Response {
    let response = ApiResponse {
        success: false,
        code: ResponseCode::StateConflict,
        message: "Event cursor expired".to_string(),
        data: Some(OpenClawEventCursorExpiredData {
            error_type: "EventCursorExpired",
            requested_after,
            oldest_available_event_id,
        }),
        metadata: None,
        error: Some(ApiErrorDetail {
            details: Some(
                "Reconnect without Last-Event-ID or resume from the oldest available event cursor"
                    .to_string(),
            ),
            trace_id: None,
        }),
    };

    (StatusCode::CONFLICT, Json(response)).into_response()
}

pub fn task_status_label(status: TaskStatus) -> &'static str {
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

pub async fn emit_task_status_changed(
    state: &AppState,
    project_id: Uuid,
    task_id: Uuid,
    previous_status: TaskStatus,
    new_status: TaskStatus,
    source: &str,
) {
    if previous_status == new_status {
        return;
    }

    if let Err(error) = state
        .openclaw_event_service
        .record_task_status_changed(
            project_id,
            task_id,
            task_status_label(previous_status),
            task_status_label(new_status),
            source,
        )
        .await
    {
        tracing::warn!(
            task_id = %task_id,
            previous_status = task_status_label(previous_status),
            new_status = task_status_label(new_status),
            error = %error,
            "Failed to emit OpenClaw task.status_changed event"
        );
    }
}

pub async fn events_stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    _auth_user: AuthUser,
    Query(params): Query<OpenClawEventStreamParams>,
) -> Result<Response, crate::error::ApiError> {
    let after_cursor = parse_resume_cursor(&headers, &params)?;
    let user_agent = header_value(&headers, "user-agent");
    let forwarded_for = header_value(&headers, "x-forwarded-for");

    if let Some(after) = after_cursor {
        if let Some(oldest) = state
            .openclaw_event_service
            .oldest_sequence_id()
            .await
            .map_err(|error| crate::error::ApiError::Internal(error.to_string()))?
        {
            if after < oldest.saturating_sub(1) {
                state
                    .metrics
                    .openclaw_event_stream_cursor_expired_total
                    .with_label_values(&["stale_cursor"])
                    .inc();
                tracing::warn!(
                    after_cursor = after,
                    oldest_available_event_id = oldest,
                    user_agent = user_agent.as_deref().unwrap_or("-"),
                    forwarded_for = forwarded_for.as_deref().unwrap_or("-"),
                    "OpenClaw event stream cursor expired"
                );
                return Ok(event_cursor_expired_response(after, Some(oldest)));
            }
        }
    }

    let live_rx = state.openclaw_event_service.subscribe_live();
    if let Some(after) = after_cursor {
        tracing::info!(
            after_cursor = after,
            user_agent = user_agent.as_deref().unwrap_or("-"),
            forwarded_for = forwarded_for.as_deref().unwrap_or("-"),
            "OpenClaw event stream replay started"
        );
    }
    let replay_events = if let Some(after) = after_cursor {
        let mut replay_events = Vec::new();
        let mut replay_cursor = after;
        loop {
            let page = state
                .openclaw_event_service
                .list_events_after(replay_cursor, OPENCLAW_EVENT_REPLAY_PAGE_SIZE)
                .await
                .map_err(|error| crate::error::ApiError::Internal(error.to_string()))?;

            if page.is_empty() {
                break;
            }

            let page_len = page.len();
            replay_cursor = page
                .last()
                .map(|event| event.sequence_id)
                .unwrap_or(replay_cursor);
            replay_events.extend(page);

            if page_len < OPENCLAW_EVENT_REPLAY_PAGE_SIZE as usize {
                break;
            }
        }
        replay_events
    } else {
        Vec::new()
    };
    if let Some(after) = after_cursor {
        tracing::info!(
            after_cursor = after,
            replay_count = replay_events.len(),
            user_agent = user_agent.as_deref().unwrap_or("-"),
            forwarded_for = forwarded_for.as_deref().unwrap_or("-"),
            "OpenClaw event stream replay completed"
        );
    }
    let stream_mode = if after_cursor.is_some() {
        "resume"
    } else {
        "live"
    };
    state
        .metrics
        .openclaw_event_stream_connections_total
        .with_label_values(&[stream_mode])
        .inc();
    state.metrics.openclaw_event_stream_active_connections.inc();
    if !replay_events.is_empty() {
        state
            .metrics
            .openclaw_event_stream_replay_events_total
            .with_label_values(&[stream_mode])
            .inc_by(replay_events.len() as u64);
    }
    tracing::info!(
        after_cursor = after_cursor,
        replay_count = replay_events.len(),
        user_agent = user_agent.as_deref().unwrap_or("-"),
        forwarded_for = forwarded_for.as_deref().unwrap_or("-"),
        "OpenClaw event stream opened"
    );
    let last_sent_id = Arc::new(AtomicI64::new(
        replay_events
            .last()
            .map(|event| event.sequence_id)
            .unwrap_or(after_cursor.unwrap_or(0)),
    ));
    let disconnect_guard = Arc::new(OpenClawEventStreamDisconnectGuard {
        metrics: state.metrics.clone(),
        after_cursor,
        replay_count: replay_events.len(),
        user_agent,
        forwarded_for,
    });

    let replay_stream = futures::stream::iter(replay_events.into_iter().map(to_sse_event));
    let live_stream = BroadcastStream::new(live_rx).filter_map(move |message| {
        let last_sent_id = last_sent_id.clone();
        let disconnect_guard = disconnect_guard.clone();
        async move {
            let _disconnect_guard = disconnect_guard;
            match message {
                Ok(event) if event.sequence_id > last_sent_id.load(Ordering::Relaxed) => {
                    last_sent_id.store(event.sequence_id, Ordering::Relaxed);
                    Some(to_sse_event(event))
                }
                Ok(_) => None,
                Err(_) => None,
            }
        }
    });

    Ok(Sse::new(replay_stream.chain(live_stream))
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response())
}

pub fn create_router(state: AppState) -> Router {
    let v1_routes = super::build_business_api_routes()
        .route("/events/stream", get(events_stream))
        .route(
            "/tasks/:task_id/context",
            post(create_task_context_from_openclaw),
        );

    Router::new()
        .route(
            "/guide-for-openclaw",
            get(guide_for_openclaw_get).post(guide_for_openclaw),
        )
        .route("/openapi.json", get(openapi_json))
        .nest("/v1", v1_routes)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::require_openclaw_auth,
        ))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::{
        build_handoff_contract, OPENCLAW_CONNECTION_BUNDLE_FIELDS,
        OPENCLAW_HANDOFF_CONTRACT_VERSION, OPENCLAW_REPORTING_REQUIREMENTS,
        OPENCLAW_REQUIRED_FIRST_ACTIONS, OPENCLAW_REQUIRED_ROUTE_PREFIXES,
    };

    #[test]
    fn handoff_contract_uses_canonical_values() {
        let handoff = build_handoff_contract();

        assert_eq!(handoff.contract_version, OPENCLAW_HANDOFF_CONTRACT_VERSION);
        assert_eq!(
            handoff.connection_bundle_fields.len(),
            OPENCLAW_CONNECTION_BUNDLE_FIELDS.len()
        );
        assert_eq!(
            handoff.required_first_actions.len(),
            OPENCLAW_REQUIRED_FIRST_ACTIONS.len()
        );
        assert_eq!(
            handoff.required_route_prefixes.len(),
            OPENCLAW_REQUIRED_ROUTE_PREFIXES.len()
        );
        assert_eq!(
            handoff.reporting_requirements.len(),
            OPENCLAW_REPORTING_REQUIREMENTS.len()
        );
    }

    #[test]
    fn install_script_mentions_canonical_handoff_contract() {
        let install_script = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../install.sh"));

        for value in OPENCLAW_CONNECTION_BUNDLE_FIELDS {
            assert!(
                install_script.contains(value),
                "install.sh is missing handoff connection field: {value}"
            );
        }

        for value in OPENCLAW_REQUIRED_FIRST_ACTIONS {
            assert!(
                install_script.contains(value),
                "install.sh is missing required first action: {value}"
            );
        }

        for value in OPENCLAW_REQUIRED_ROUTE_PREFIXES {
            assert!(
                install_script.contains(value),
                "install.sh is missing route prefix: {value}"
            );
        }

        for value in OPENCLAW_REPORTING_REQUIREMENTS {
            assert!(
                install_script.contains(value),
                "install.sh is missing reporting requirement: {value}"
            );
        }
    }
}
