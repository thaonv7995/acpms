use crate::api::AgentActivityStatusDto;
use crate::middleware::{authenticate_bearer_token, Permission, RbacChecker};
use crate::routes::agent::AgentAuthSessionDoc;
use crate::services::agent_auth::AuthSessionStatus;
use crate::{error::ApiError, AppState};
use acpms_db::models::AttemptStatus;
use acpms_executors::{AgentEvent, LogMessage, StatusMessage};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, Query, State, WebSocketUpgrade,
    },
    http::{header, HeaderMap},
    response::IntoResponse,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use chrono::{DateTime, Utc};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use tokio::time::{self, Duration, MissedTickBehavior};
use uuid::Uuid;

const WS_BEARER_PROTOCOL: &str = "acpms-bearer";
const DASHBOARD_STATUS_LIMIT: i64 = 10;

fn extract_token_from_ws_protocol(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::SEC_WEBSOCKET_PROTOCOL)?.to_str().ok()?;
    let protocols: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();

    protocols.windows(2).find_map(|window| {
        if window[0].eq_ignore_ascii_case(WS_BEARER_PROTOCOL) {
            Some(window[1].to_string())
        } else {
            None
        }
    })
}

fn ws_upgrade_with_protocol(ws: WebSocketUpgrade, headers: &HeaderMap) -> WebSocketUpgrade {
    let requested_auth_protocol = headers
        .get(header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|v| v.to_str().ok())
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .any(|p| p.eq_ignore_ascii_case(WS_BEARER_PROTOCOL))
        })
        .unwrap_or(false);

    if requested_auth_protocol {
        ws.protocols([WS_BEARER_PROTOCOL])
    } else {
        ws
    }
}

/// Client-to-server messages for bidirectional WebSocket communication
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
    UserInput { content: String },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AgentActivityStatusWsMessage {
    Snapshot {
        statuses: Vec<AgentActivityStatusDto>,
    },
    Upsert {
        status: AgentActivityStatusDto,
    },
    Remove {
        attempt_id: Uuid,
    },
}

#[derive(Debug, Deserialize)]
pub struct ExecutionProcessesWsQuery {
    pub attempt_id: Uuid,
    pub since_seq: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ExecutionProcessesSessionWsQuery {
    pub session_id: Uuid,
    pub since_seq: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ExecutionProcessLogsWsQuery {
    pub since_seq: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AttemptStreamWsQuery {
    pub since: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AgentAuthSessionWsQuery {
    pub since_seq: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq, Eq)]
pub struct ExecutionProcessWsDto {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub process_id: Option<i32>,
    pub worktree_path: Option<String>,
    pub branch_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ExecutionProcessesWsMessage {
    Snapshot {
        processes: Vec<ExecutionProcessWsDto>,
    },
    Upsert {
        process: ExecutionProcessWsDto,
    },
    Remove {
        process_id: Uuid,
    },
}

#[derive(Debug, Serialize)]
struct SequencedExecutionProcessesWsMessage {
    sequence_id: u64,
    message: ExecutionProcessesWsMessage,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SequencedCollectionGapWsMessage {
    GapDetected {
        requested_since_seq: u64,
        max_available_sequence_id: u64,
    },
}

#[derive(Debug, Deserialize)]
pub struct ApprovalsWsQuery {
    pub attempt_id: Option<Uuid>,
    pub execution_process_id: Option<Uuid>,
    pub projection: Option<String>,
    pub since_seq: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq, Eq)]
pub struct ApprovalWsDto {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub execution_process_id: Option<Uuid>,
    pub tool_use_id: String,
    pub tool_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub responded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ApprovalsWsMessage {
    Snapshot { approvals: Vec<ApprovalWsDto> },
    Upsert { approval: ApprovalWsDto },
    Remove { approval_id: Uuid },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApprovalsProjection {
    Legacy,
    Patch,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct JsonPatchOperation {
    op: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ApprovalsPatchWsMessage {
    Snapshot {
        sequence_id: u64,
        data: serde_json::Value,
    },
    Patch {
        sequence_id: u64,
        operations: Vec<JsonPatchOperation>,
    },
    GapDetected {
        requested_since_seq: u64,
        max_available_sequence_id: u64,
    },
}

#[derive(Debug, Clone, Copy)]
enum ExecutionProcessLogMode {
    Raw,
    Normalized,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ExecutionProcessLogWsMessage {
    Event {
        sequence_id: u64,
        event: AgentEvent,
    },
    GapDetected {
        requested_since_seq: u64,
        max_available_sequence_id: u64,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AgentAuthSessionWsMessage {
    Snapshot {
        sequence_id: u64,
        session: AgentAuthSessionDoc,
    },
    Upsert {
        sequence_id: u64,
        session: AgentAuthSessionDoc,
    },
    GapDetected {
        requested_since_seq: u64,
        max_available_sequence_id: u64,
    },
}

async fn send_sequenced_agent_event(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    sequence_id: u64,
    event: AgentEvent,
) -> Result<(), ApiError> {
    let payload =
        serde_json::to_string(&ExecutionProcessLogWsMessage::Event { sequence_id, event })
            .map_err(|e| ApiError::Internal(format!("Serialize error: {}", e)))?;
    sender
        .send(Message::Text(payload))
        .await
        .map_err(|_| ApiError::Internal("WebSocket send failed".to_string()))
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct ExecutionProcessAccessRow {
    attempt_id: Uuid,
    project_id: Uuid,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct ExecutionProcessStreamContext {
    process_id: Uuid,
    attempt_id: Uuid,
    project_id: Uuid,
    lower_bound: DateTime<Utc>,
    upper_bound: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct SequencedLogEvent {
    sequence_id: u64,
    event: AgentEvent,
}

fn matches_log_mode(log_type: &str, mode: ExecutionProcessLogMode) -> bool {
    match mode {
        ExecutionProcessLogMode::Raw => {
            matches!(
                log_type,
                "process_stdout" | "process_stderr" | "stdout" | "stderr"
            )
        }
        ExecutionProcessLogMode::Normalized => {
            matches!(log_type, "normalized" | "user" | "stdin")
        }
    }
}

fn is_terminal_attempt_status(status: &AttemptStatus) -> bool {
    matches!(
        status,
        AttemptStatus::Success | AttemptStatus::Failed | AttemptStatus::Cancelled
    )
}

fn timestamp_in_process_window(
    ts: DateTime<Utc>,
    lower_bound: DateTime<Utc>,
    upper_bound: Option<DateTime<Utc>>,
) -> bool {
    ts >= lower_bound && upper_bound.map(|upper| ts < upper).unwrap_or(true)
}

fn log_event_in_process_window(log: &LogMessage, ctx: &ExecutionProcessStreamContext) -> bool {
    if log.attempt_id != ctx.attempt_id {
        return false;
    }
    let ts = log.created_at.unwrap_or(log.timestamp);
    timestamp_in_process_window(ts, ctx.lower_bound, ctx.upper_bound)
}

fn status_event_in_process_window(
    status: &StatusMessage,
    ctx: &ExecutionProcessStreamContext,
) -> bool {
    if status.attempt_id != ctx.attempt_id {
        return false;
    }
    timestamp_in_process_window(status.timestamp, ctx.lower_bound, ctx.upper_bound)
}

async fn resolve_execution_process_stream_context(
    state: &AppState,
    process_id: Uuid,
) -> Result<Option<ExecutionProcessStreamContext>, ApiError> {
    let process_row: Option<ExecutionProcessAccessRow> = sqlx::query_as(
        r#"
        SELECT ep.attempt_id, t.project_id, ep.created_at
        FROM execution_processes ep
        JOIN task_attempts ta ON ta.id = ep.attempt_id
        JOIN tasks t ON t.id = ta.task_id
        WHERE ep.id = $1
        "#,
    )
    .bind(process_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let Some(process_row) = process_row else {
        return Ok(None);
    };

    let next_process_created_at: Option<DateTime<Utc>> = sqlx::query_scalar(
        r#"
        SELECT created_at
        FROM execution_processes
        WHERE attempt_id = $1
          AND created_at > $2
        ORDER BY created_at ASC
        LIMIT 1
        "#,
    )
    .bind(process_row.attempt_id)
    .bind(process_row.created_at)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    Ok(Some(ExecutionProcessStreamContext {
        process_id,
        attempt_id: process_row.attempt_id,
        project_id: process_row.project_id,
        lower_bound: process_row.created_at,
        upper_bound: next_process_created_at,
    }))
}

async fn load_initial_execution_process_log_events(
    state: &AppState,
    ctx: &ExecutionProcessStreamContext,
    mode: ExecutionProcessLogMode,
) -> Result<Vec<SequencedLogEvent>, ApiError> {
    let bytes = load_log_bytes_for_attempt(state, ctx.attempt_id).await?;
    let mut logs = acpms_executors::parse_jsonl_to_agent_logs(&bytes);
    logs.retain(|log| {
        log.created_at >= ctx.lower_bound
            && (ctx.upper_bound.is_none() || log.created_at < ctx.upper_bound.unwrap())
    });
    logs.retain(|log| matches_log_mode(&log.log_type, mode));
    logs.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut events: Vec<SequencedLogEvent> = Vec::new();
    for (i, log) in logs.into_iter().enumerate() {
        let sequence_id = u64::try_from(i + 1).map_err(|e| {
            ApiError::Internal(format!(
                "Invalid sequence id for execution process log: {}",
                e
            ))
        })?;
        events.push(SequencedLogEvent {
            sequence_id,
            event: AgentEvent::Log(LogMessage {
                attempt_id: log.attempt_id,
                log_type: log.log_type,
                content: log.content,
                timestamp: log.created_at,
                id: Some(log.id),
                created_at: Some(log.created_at),
                tool_name: None,
            }),
        });
    }

    Ok(events)
}

async fn load_log_bytes_for_attempt(
    state: &AppState,
    attempt_id: Uuid,
) -> Result<Vec<u8>, ApiError> {
    let s3_key = sqlx::query_scalar::<_, Option<String>>(
        "SELECT s3_log_key FROM task_attempts WHERE id = $1",
    )
    .bind(attempt_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .flatten();

    if let Some(key) = s3_key {
        state
            .storage_service
            .get_log_bytes(&key)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))
    } else {
        acpms_executors::read_attempt_log_file(attempt_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))
    }
}

async fn resolve_sequence_id_for_live_log_event(
    state: &AppState,
    ctx: &ExecutionProcessStreamContext,
    mode: ExecutionProcessLogMode,
    log: &LogMessage,
) -> Result<Option<u64>, ApiError> {
    if !matches_log_mode(&log.log_type, mode) || !log_event_in_process_window(log, ctx) {
        return Ok(None);
    }

    let log_id = match log.id {
        Some(id) => id,
        None => return Ok(None),
    };
    let log_created_at = log.created_at.unwrap_or(log.timestamp);

    let bytes = load_log_bytes_for_attempt(state, ctx.attempt_id).await?;
    let mut logs = acpms_executors::parse_jsonl_to_agent_logs(&bytes);
    logs.retain(|l| {
        l.created_at >= ctx.lower_bound
            && (ctx.upper_bound.is_none() || l.created_at < ctx.upper_bound.unwrap())
            && matches_log_mode(&l.log_type, mode)
    });
    logs.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });

    let count = logs
        .iter()
        .take_while(|l| {
            l.created_at < log_created_at || (l.created_at == log_created_at && l.id <= log_id)
        })
        .count();

    if count == 0 {
        return Ok(None);
    }

    u64::try_from(count)
        .map(Some)
        .map_err(|e| ApiError::Internal(format!("Invalid live log sequence id: {}", e)))
}

async fn resolve_terminal_status_sequence_id(
    state: &AppState,
    ctx: &ExecutionProcessStreamContext,
    mode: ExecutionProcessLogMode,
) -> Result<u64, ApiError> {
    let bytes = load_log_bytes_for_attempt(state, ctx.attempt_id).await?;
    let mut logs = acpms_executors::parse_jsonl_to_agent_logs(&bytes);
    logs.retain(|l| {
        l.created_at >= ctx.lower_bound
            && (ctx.upper_bound.is_none() || l.created_at < ctx.upper_bound.unwrap())
            && matches_log_mode(&l.log_type, mode)
    });
    let count = logs.len();
    let next = count + 1;
    u64::try_from(next).map_err(|e| {
        ApiError::Internal(format!(
            "Invalid terminal status sequence id {}: {}",
            next, e
        ))
    })
}

async fn load_execution_process_terminal_status_event(
    state: &AppState,
    ctx: &ExecutionProcessStreamContext,
) -> Result<Option<AgentEvent>, ApiError> {
    let status_row: Option<(AttemptStatus, Option<DateTime<Utc>>)> =
        sqlx::query_as("SELECT status, completed_at FROM task_attempts WHERE id = $1")
            .bind(ctx.attempt_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let Some((status, completed_at)) = status_row else {
        return Ok(None);
    };

    if !is_terminal_attempt_status(&status) {
        return Ok(None);
    }

    let timestamp = completed_at.unwrap_or_else(Utc::now);
    if !timestamp_in_process_window(timestamp, ctx.lower_bound, ctx.upper_bound) {
        return Ok(None);
    }

    Ok(Some(AgentEvent::Status(StatusMessage {
        attempt_id: ctx.attempt_id,
        status,
        timestamp,
    })))
}

async fn fetch_attempt_execution_processes(
    state: &AppState,
    attempt_id: Uuid,
) -> Result<Vec<ExecutionProcessWsDto>, ApiError> {
    let processes: Vec<ExecutionProcessWsDto> = sqlx::query_as(
        r#"
        SELECT id, attempt_id, process_id, worktree_path, branch_name, created_at
        FROM execution_processes
        WHERE attempt_id = $1
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .bind(attempt_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    Ok(processes)
}

#[derive(Debug, Clone, Copy)]
enum ApprovalScope {
    Attempt(Uuid),
    ExecutionProcess(Uuid),
}

fn execution_processes_stream_sequence_key(attempt_id: Uuid) -> String {
    format!("/collections/execution-processes/{}", attempt_id)
}

fn approvals_stream_sequence_key(scope: ApprovalScope) -> String {
    match scope {
        ApprovalScope::Attempt(attempt_id) => {
            format!("/collections/approvals/attempt/{}", attempt_id)
        }
        ApprovalScope::ExecutionProcess(process_id) => {
            format!("/collections/approvals/process/{}", process_id)
        }
    }
}

async fn resolve_project_id_for_approval_scope(
    state: &AppState,
    scope: ApprovalScope,
) -> Result<Option<Uuid>, ApiError> {
    let row = match scope {
        ApprovalScope::Attempt(attempt_id) => sqlx::query_scalar(
            r#"
                SELECT t.project_id
                FROM task_attempts ta
                JOIN tasks t ON t.id = ta.task_id
                WHERE ta.id = $1
                "#,
        )
        .bind(attempt_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?,
        ApprovalScope::ExecutionProcess(process_id) => sqlx::query_scalar(
            r#"
                SELECT t.project_id
                FROM execution_processes ep
                JOIN task_attempts ta ON ta.id = ep.attempt_id
                JOIN tasks t ON t.id = ta.task_id
                WHERE ep.id = $1
                "#,
        )
        .bind(process_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?,
    };

    Ok(row)
}

async fn fetch_approvals_for_scope(
    state: &AppState,
    scope: ApprovalScope,
) -> Result<Vec<ApprovalWsDto>, ApiError> {
    let rows: Vec<ApprovalWsDto> = match scope {
        ApprovalScope::Attempt(attempt_id) => sqlx::query_as(
            r#"
                SELECT
                    id,
                    attempt_id,
                    execution_process_id,
                    tool_use_id,
                    tool_name,
                    status::text as status,
                    created_at,
                    responded_at
                FROM tool_approvals
                WHERE attempt_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
        )
        .bind(attempt_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?,
        ApprovalScope::ExecutionProcess(process_id) => sqlx::query_as(
            r#"
                SELECT
                    id,
                    attempt_id,
                    execution_process_id,
                    tool_use_id,
                    tool_name,
                    status::text as status,
                    created_at,
                    responded_at
                FROM tool_approvals
                WHERE execution_process_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
        )
        .bind(process_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?,
    };

    Ok(rows)
}

fn approval_scope_attempt_filter(scope: ApprovalScope, event: &AgentEvent) -> bool {
    let attempt_id = match event {
        AgentEvent::Log(log) => log.attempt_id,
        AgentEvent::Status(status) => status.attempt_id,
        AgentEvent::ApprovalRequest(approval) => approval.attempt_id,
        AgentEvent::UserMessage(user_msg) => user_msg.attempt_id,
        AgentEvent::AssistantLog(_) => return false,
    };

    match scope {
        ApprovalScope::Attempt(target_attempt_id) => attempt_id == target_attempt_id,
        ApprovalScope::ExecutionProcess(_) => true,
    }
}

async fn sync_approvals_snapshot(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
    scope: ApprovalScope,
    known: &mut std::collections::HashMap<Uuid, ApprovalWsDto>,
) -> Result<(), ApiError> {
    let current_rows = fetch_approvals_for_scope(state, scope).await?;
    let mut current: std::collections::HashMap<Uuid, ApprovalWsDto> =
        std::collections::HashMap::new();
    for row in current_rows {
        current.insert(row.id, row);
    }

    for id in sorted_map_ids(&current) {
        let Some(approval) = current.get(&id) else {
            continue;
        };
        if known.get(&id) != Some(approval) {
            let payload = serde_json::to_string(&ApprovalsWsMessage::Upsert {
                approval: approval.clone(),
            })
            .map_err(|e| ApiError::Internal(format!("Serialize error: {}", e)))?;
            if sender.send(Message::Text(payload)).await.is_err() {
                return Err(ApiError::Internal("WebSocket send failed".to_string()));
            }
        }
    }

    for removed_id in sorted_removed_ids(known, &current) {
        let payload = serde_json::to_string(&ApprovalsWsMessage::Remove {
            approval_id: removed_id,
        })
        .map_err(|e| ApiError::Internal(format!("Serialize error: {}", e)))?;
        if sender.send(Message::Text(payload)).await.is_err() {
            return Err(ApiError::Internal("WebSocket send failed".to_string()));
        }
    }

    *known = current;
    Ok(())
}

async fn sync_approvals_patch_snapshot(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
    scope: ApprovalScope,
    known: &mut std::collections::HashMap<Uuid, ApprovalWsDto>,
    sequence_key: &str,
) -> Result<(), ApiError> {
    let current_rows = fetch_approvals_for_scope(state, scope).await?;
    let mut current: std::collections::HashMap<Uuid, ApprovalWsDto> =
        std::collections::HashMap::new();
    for row in current_rows {
        current.insert(row.id, row);
    }

    let operations = build_approvals_patch_operations(known, &current)?;

    if !operations.is_empty() {
        let sequence_id = state
            .patch_store
            .reserve_collection_sequence(sequence_key)
            .await;
        let payload = serde_json::to_string(&ApprovalsPatchWsMessage::Patch {
            sequence_id,
            operations,
        })
        .map_err(|e| ApiError::Internal(format!("Serialize error: {}", e)))?;
        if sender.send(Message::Text(payload)).await.is_err() {
            return Err(ApiError::Internal("WebSocket send failed".to_string()));
        }
    }

    *known = current;
    Ok(())
}

fn agent_event_attempt_id(event: &AgentEvent) -> Option<Uuid> {
    match event {
        AgentEvent::Log(log) => Some(log.attempt_id),
        AgentEvent::Status(status) => Some(status.attempt_id),
        AgentEvent::ApprovalRequest(approval) => Some(approval.attempt_id),
        AgentEvent::UserMessage(user_msg) => Some(user_msg.attempt_id),
        AgentEvent::AssistantLog(_) => None,
    }
}

fn sorted_map_ids<T>(map: &std::collections::HashMap<Uuid, T>) -> Vec<Uuid> {
    let mut ids: Vec<Uuid> = map.keys().copied().collect();
    ids.sort();
    ids
}

enum InitialApprovalsProjectionPayload {
    Snapshot { payload: String },
}

fn build_initial_approvals_projection_payload(
    projection: ApprovalsProjection,
    known: &std::collections::HashMap<Uuid, ApprovalWsDto>,
    initial_approvals: &[ApprovalWsDto],
    snapshot_sequence_id: Option<u64>,
) -> Result<InitialApprovalsProjectionPayload, ApiError> {
    match projection {
        ApprovalsProjection::Legacy => {
            let payload = serde_json::to_string(&ApprovalsWsMessage::Snapshot {
                approvals: initial_approvals.to_vec(),
            })
            .map_err(|e| ApiError::Internal(format!("Serialize error: {}", e)))?;
            Ok(InitialApprovalsProjectionPayload::Snapshot { payload })
        }
        ApprovalsProjection::Patch => {
            let sequence_id = snapshot_sequence_id.ok_or_else(|| {
                ApiError::Internal(
                    "Missing snapshot sequence_id for approvals patch projection".to_string(),
                )
            })?;

            let data = build_approvals_snapshot_data(known)?;
            let payload =
                serde_json::to_string(&ApprovalsPatchWsMessage::Snapshot { sequence_id, data })
                    .map_err(|e| ApiError::Internal(format!("Serialize error: {}", e)))?;
            Ok(InitialApprovalsProjectionPayload::Snapshot { payload })
        }
    }
}

fn build_approvals_snapshot_data(
    approvals: &std::collections::HashMap<Uuid, ApprovalWsDto>,
) -> Result<serde_json::Value, ApiError> {
    let mut approvals_map = serde_json::Map::new();
    for id in sorted_map_ids(approvals) {
        let Some(approval) = approvals.get(&id) else {
            continue;
        };
        let value = serde_json::to_value(approval)
            .map_err(|e| ApiError::Internal(format!("Serialize error: {}", e)))?;
        approvals_map.insert(id.to_string(), value);
    }

    Ok(serde_json::json!({ "approvals": approvals_map }))
}

fn build_approvals_patch_operations(
    known: &std::collections::HashMap<Uuid, ApprovalWsDto>,
    current: &std::collections::HashMap<Uuid, ApprovalWsDto>,
) -> Result<Vec<JsonPatchOperation>, ApiError> {
    let mut operations: Vec<JsonPatchOperation> = Vec::new();

    for id in sorted_map_ids(current) {
        let Some(approval) = current.get(&id) else {
            continue;
        };
        if known.get(&id) != Some(approval) {
            let value = serde_json::to_value(approval)
                .map_err(|e| ApiError::Internal(format!("Serialize error: {}", e)))?;
            operations.push(JsonPatchOperation {
                op: if known.contains_key(&id) {
                    "replace".to_string()
                } else {
                    "add".to_string()
                },
                path: format!("/approvals/{}", id),
                value: Some(value),
            });
        }
    }

    for removed_id in sorted_removed_ids(known, current) {
        operations.push(JsonPatchOperation {
            op: "remove".to_string(),
            path: format!("/approvals/{}", removed_id),
            value: None,
        });
    }

    Ok(operations)
}

fn sorted_removed_ids<T>(
    known: &std::collections::HashMap<Uuid, T>,
    current: &std::collections::HashMap<Uuid, T>,
) -> Vec<Uuid> {
    let mut removed_ids: Vec<Uuid> = known
        .keys()
        .filter(|id| !current.contains_key(id))
        .copied()
        .collect();
    removed_ids.sort();
    removed_ids
}

async fn sync_execution_processes_snapshot(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
    attempt_id: Uuid,
    known: &mut std::collections::HashMap<Uuid, ExecutionProcessWsDto>,
    sequence_key: &str,
) -> Result<(), ApiError> {
    let processes = fetch_attempt_execution_processes(state, attempt_id).await?;
    let mut current: std::collections::HashMap<Uuid, ExecutionProcessWsDto> =
        std::collections::HashMap::new();
    for process in processes {
        current.insert(process.id, process);
    }

    let mut upsert_ids: Vec<Uuid> = current.keys().copied().collect();
    upsert_ids.sort();
    for id in upsert_ids {
        let Some(process) = current.get(&id) else {
            continue;
        };
        let changed = known.get(&id) != Some(process);
        if changed {
            let sequence_id = state
                .patch_store
                .reserve_collection_sequence(sequence_key)
                .await;
            let payload = serde_json::to_string(&SequencedExecutionProcessesWsMessage {
                sequence_id,
                message: ExecutionProcessesWsMessage::Upsert {
                    process: process.clone(),
                },
            })
            .map_err(|e| ApiError::Internal(format!("Serialize error: {}", e)))?;
            if sender.send(Message::Text(payload)).await.is_err() {
                return Err(ApiError::Internal("WebSocket send failed".to_string()));
            }
        }
    }

    for removed_id in sorted_removed_ids(known, &current) {
        let sequence_id = state
            .patch_store
            .reserve_collection_sequence(sequence_key)
            .await;
        let payload = serde_json::to_string(&SequencedExecutionProcessesWsMessage {
            sequence_id,
            message: ExecutionProcessesWsMessage::Remove {
                process_id: removed_id,
            },
        })
        .map_err(|e| ApiError::Internal(format!("Serialize error: {}", e)))?;
        if sender.send(Message::Text(payload)).await.is_err() {
            return Err(ApiError::Internal("WebSocket send failed".to_string()));
        }
    }

    *known = current;
    Ok(())
}

/// WebSocket handler with JWT authentication.
///
/// ## Security
/// - Requires valid JWT token in `Sec-WebSocket-Protocol` (`acpms-bearer`, `<token>`)
///   or in Authorization header (non-browser clients)
/// - Verifies user has permission to view task attempt
/// - Closes connection on authentication failure
///
/// ## Protocol
/// - Streams AgentEvent messages as JSON
/// - Filters events by attempt_id
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(attempt_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<impl IntoResponse, ApiError> {
    // Browser WebSocket clients should send token via subprotocol:
    // new WebSocket(url, ["acpms-bearer", token])
    let token = extract_token_from_ws_protocol(&headers)
        .or_else(|| auth_header.map(|h| h.token().to_string()))
        .ok_or(ApiError::Unauthorized)?;

    let auth_user = authenticate_bearer_token(&token, &state).await?;
    let user_id = auth_user.id;

    #[derive(sqlx::FromRow)]
    struct AttemptRow {
        task_id: Uuid,
    }

    #[derive(sqlx::FromRow)]
    struct TaskRow {
        project_id: Uuid,
    }

    // Fetch task attempt to verify it exists and get task_id
    let attempt = sqlx::query_as::<_, AttemptRow>(
        r#"
        SELECT task_id FROM task_attempts WHERE id = $1
        "#,
    )
    .bind(attempt_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
    .ok_or_else(|| ApiError::NotFound(format!("Task attempt {} not found", attempt_id)))?;

    // Fetch task to get project_id
    let task = sqlx::query_as::<_, TaskRow>(
        r#"
        SELECT project_id FROM tasks WHERE id = $1
        "#,
    )
    .bind(attempt.task_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
    .ok_or_else(|| ApiError::NotFound(format!("Task {} not found", attempt.task_id)))?;

    // Check permission using RBAC (includes system admin bypass).
    RbacChecker::check_permission(user_id, task.project_id, Permission::ViewTask, &state.db)
        .await?;

    // User is authenticated and authorized - upgrade to WebSocket
    let ws = ws_upgrade_with_protocol(ws, &headers);
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, attempt_id, state)))
}

/// WebSocket endpoint for attempt stream (same format as SSE /attempts/:id/stream).
/// WS primary, SSE fallback - Vibe Kanban parity.
pub async fn attempt_stream_ws_handler(
    ws: WebSocketUpgrade,
    Path(attempt_id): Path<Uuid>,
    Query(query): Query<AttemptStreamWsQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<impl IntoResponse, ApiError> {
    let token = extract_token_from_ws_protocol(&headers)
        .or_else(|| auth_header.map(|h| h.token().to_string()))
        .ok_or(ApiError::Unauthorized)?;

    let auth_user = authenticate_bearer_token(&token, &state).await?;
    let user_id = auth_user.id;

    let project_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT t.project_id
        FROM task_attempts ta
        JOIN tasks t ON ta.task_id = t.id
        WHERE ta.id = $1
        "#,
    )
    .bind(attempt_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let project_id =
        project_id.ok_or_else(|| ApiError::NotFound("Task attempt not found".into()))?;

    RbacChecker::check_permission(user_id, project_id, Permission::ViewTask, &state.db).await?;

    let since = query.since;
    let ws = ws_upgrade_with_protocol(ws, &headers);
    Ok(ws.on_upgrade(move |socket| handle_attempt_stream_socket(socket, state, attempt_id, since)))
}

async fn handle_attempt_stream_socket(
    socket: WebSocket,
    state: AppState,
    attempt_id: Uuid,
    since: Option<u64>,
) {
    let stream = state
        .stream_service
        .stream_task_attempt_with_catchup(attempt_id, since)
        .await;

    let (mut send, mut _recv) = socket.split();

    let mut stream = stream;
    while let Some(msg_result) = futures::stream::StreamExt::next(&mut stream).await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(_) => continue,
        };
        let json = serde_json::to_string(&msg).unwrap_or_default();
        if send.send(Message::Text(json)).await.is_err() {
            break;
        }
    }
}

pub async fn execution_processes_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<ExecutionProcessesWsQuery>,
    headers: HeaderMap,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<impl IntoResponse, ApiError> {
    let token = extract_token_from_ws_protocol(&headers)
        .or_else(|| auth_header.map(|h| h.token().to_string()))
        .ok_or(ApiError::Unauthorized)?;
    let auth_user = authenticate_bearer_token(&token, &state).await?;

    #[derive(sqlx::FromRow)]
    struct AttemptProjectRow {
        project_id: Uuid,
    }

    let attempt_project: AttemptProjectRow = sqlx::query_as(
        r#"
        SELECT t.project_id
        FROM task_attempts ta
        JOIN tasks t ON t.id = ta.task_id
        WHERE ta.id = $1
        "#,
    )
    .bind(query.attempt_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
    .ok_or_else(|| ApiError::NotFound(format!("Task attempt {} not found", query.attempt_id)))?;

    RbacChecker::check_permission(
        auth_user.id,
        attempt_project.project_id,
        Permission::ViewProject,
        &state.db,
    )
    .await?;

    let ws = ws_upgrade_with_protocol(ws, &headers);
    Ok(ws.on_upgrade(move |socket| {
        handle_execution_processes_socket(socket, state, query.attempt_id, query.since_seq)
    }))
}

pub async fn execution_processes_session_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<ExecutionProcessesSessionWsQuery>,
    headers: HeaderMap,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<impl IntoResponse, ApiError> {
    let token = extract_token_from_ws_protocol(&headers)
        .or_else(|| auth_header.map(|h| h.token().to_string()))
        .ok_or(ApiError::Unauthorized)?;
    let auth_user = authenticate_bearer_token(&token, &state).await?;

    #[derive(sqlx::FromRow)]
    struct AttemptProjectRow {
        project_id: Uuid,
    }

    let attempt_project: AttemptProjectRow = sqlx::query_as(
        r#"
        SELECT t.project_id
        FROM task_attempts ta
        JOIN tasks t ON t.id = ta.task_id
        WHERE ta.id = $1
        "#,
    )
    .bind(query.session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
    .ok_or_else(|| ApiError::NotFound(format!("Task attempt {} not found", query.session_id)))?;

    RbacChecker::check_permission(
        auth_user.id,
        attempt_project.project_id,
        Permission::ViewProject,
        &state.db,
    )
    .await?;

    let ws = ws_upgrade_with_protocol(ws, &headers);
    Ok(ws.on_upgrade(move |socket| {
        handle_execution_processes_socket(socket, state, query.session_id, query.since_seq)
    }))
}

pub async fn execution_process_raw_logs_ws_handler(
    ws: WebSocketUpgrade,
    Path(process_id): Path<Uuid>,
    State(state): State<AppState>,
    Query(query): Query<ExecutionProcessLogsWsQuery>,
    headers: HeaderMap,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<impl IntoResponse, ApiError> {
    execution_process_logs_ws_handler(
        ws,
        process_id,
        state,
        headers,
        auth_header,
        ExecutionProcessLogMode::Raw,
        query.since_seq,
    )
    .await
}

pub async fn execution_process_normalized_logs_ws_handler(
    ws: WebSocketUpgrade,
    Path(process_id): Path<Uuid>,
    State(state): State<AppState>,
    Query(query): Query<ExecutionProcessLogsWsQuery>,
    headers: HeaderMap,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<impl IntoResponse, ApiError> {
    execution_process_logs_ws_handler(
        ws,
        process_id,
        state,
        headers,
        auth_header,
        ExecutionProcessLogMode::Normalized,
        query.since_seq,
    )
    .await
}

async fn execution_process_logs_ws_handler(
    ws: WebSocketUpgrade,
    process_id: Uuid,
    state: AppState,
    headers: HeaderMap,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
    mode: ExecutionProcessLogMode,
    since_seq: Option<u64>,
) -> Result<impl IntoResponse, ApiError> {
    let token = extract_token_from_ws_protocol(&headers)
        .or_else(|| auth_header.map(|h| h.token().to_string()))
        .ok_or(ApiError::Unauthorized)?;
    let auth_user = authenticate_bearer_token(&token, &state).await?;

    let stream_ctx = resolve_execution_process_stream_context(&state, process_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Execution process {} not found", process_id)))?;

    RbacChecker::check_permission(
        auth_user.id,
        stream_ctx.project_id,
        Permission::ViewProject,
        &state.db,
    )
    .await?;

    let ws = ws_upgrade_with_protocol(ws, &headers);
    Ok(ws.on_upgrade(move |socket| {
        handle_execution_process_socket(socket, stream_ctx, state, mode, since_seq)
    }))
}

pub async fn approvals_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<ApprovalsWsQuery>,
    headers: HeaderMap,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<impl IntoResponse, ApiError> {
    let projection = match query.projection.as_deref() {
        Some("patch") => ApprovalsProjection::Patch,
        _ => ApprovalsProjection::Legacy,
    };

    let scope = if let Some(process_id) = query.execution_process_id {
        ApprovalScope::ExecutionProcess(process_id)
    } else if let Some(attempt_id) = query.attempt_id {
        ApprovalScope::Attempt(attempt_id)
    } else {
        return Err(ApiError::BadRequest(
            "Either attempt_id or execution_process_id is required".to_string(),
        ));
    };

    let token = extract_token_from_ws_protocol(&headers)
        .or_else(|| auth_header.map(|h| h.token().to_string()))
        .ok_or(ApiError::Unauthorized)?;
    let auth_user = authenticate_bearer_token(&token, &state).await?;

    let project_id = resolve_project_id_for_approval_scope(&state, scope)
        .await?
        .ok_or_else(|| ApiError::NotFound("Approval scope not found".to_string()))?;

    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewTask, &state.db)
        .await?;

    let attempt_filter = match scope {
        ApprovalScope::Attempt(attempt_id) => Some(attempt_id),
        ApprovalScope::ExecutionProcess(process_id) => {
            sqlx::query_scalar("SELECT attempt_id FROM execution_processes WHERE id = $1")
                .bind(process_id)
                .fetch_optional(&state.db)
                .await
                .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        }
    };

    let ws = ws_upgrade_with_protocol(ws, &headers);
    Ok(ws.on_upgrade(move |socket| {
        handle_approvals_socket(
            socket,
            state,
            scope,
            attempt_filter,
            projection,
            query.since_seq,
        )
    }))
}

pub async fn agent_activity_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<impl IntoResponse, ApiError> {
    let token = extract_token_from_ws_protocol(&headers)
        .or_else(|| auth_header.map(|h| h.token().to_string()))
        .ok_or(ApiError::Unauthorized)?;

    let auth_user = authenticate_bearer_token(&token, &state).await?;
    let user_id = auth_user.id;
    let is_admin = RbacChecker::is_system_admin(user_id, &state.db).await?;

    let ws = ws_upgrade_with_protocol(ws, &headers);
    Ok(ws.on_upgrade(move |socket| {
        handle_agent_activity_status_socket(socket, state, user_id, is_admin)
    }))
}

pub async fn agent_auth_session_ws_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<Uuid>,
    State(state): State<AppState>,
    Query(query): Query<AgentAuthSessionWsQuery>,
    headers: HeaderMap,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<impl IntoResponse, ApiError> {
    crate::routes::agent::ensure_agent_ui_auth_enabled()?;

    let token = extract_token_from_ws_protocol(&headers)
        .or_else(|| auth_header.map(|h| h.token().to_string()))
        .ok_or(ApiError::Unauthorized)?;
    let auth_user = authenticate_bearer_token(&token, &state).await?;

    let _session = state
        .auth_session_store
        .get_owned(session_id, auth_user.id)
        .await
        .ok_or_else(|| ApiError::NotFound("Auth session not found".to_string()))?;

    let ws = ws_upgrade_with_protocol(ws, &headers);
    Ok(ws.on_upgrade(move |socket| {
        handle_agent_auth_session_socket(socket, state, auth_user.id, session_id, query.since_seq)
    }))
}

async fn handle_socket(socket: WebSocket, attempt_id: Uuid, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to broadcast channel (server → client)
    let mut rx = state.broadcast_tx.subscribe();

    // Handle incoming messages (client → server)
    let state_recv = state.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                    match client_msg {
                        ClientMessage::UserInput { content } => {
                            if let Err(e) = state_recv
                                .orchestrator
                                .send_input(attempt_id, &content)
                                .await
                            {
                                tracing::error!("Failed to send user input: {}", e);
                            }
                        }
                    }
                }
            }
        }
    });

    // Broadcast outgoing messages (server → client)
    while let Ok(msg) = rx.recv().await {
        // Filter by attempt_id
        let should_send = match &msg {
            AgentEvent::Log(log) => log.attempt_id == attempt_id,
            AgentEvent::Status(status) => status.attempt_id == attempt_id,
            AgentEvent::ApprovalRequest(approval) => approval.attempt_id == attempt_id,
            AgentEvent::UserMessage(user_msg) => user_msg.attempt_id == attempt_id,
            AgentEvent::AssistantLog(_) => false,
        };

        if should_send {
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    }

    // Cleanup: abort the receiver task when sender closes
    recv_task.abort();
}

/// WebSocket for Project Assistant session logs (real-time stream).
/// GET /api/v1/projects/:project_id/assistant/sessions/:session_id/logs/ws
pub async fn assistant_logs_ws_handler(
    ws: WebSocketUpgrade,
    Path((project_id, session_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    headers: HeaderMap,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<impl IntoResponse, ApiError> {
    let token = extract_token_from_ws_protocol(&headers)
        .or_else(|| auth_header.map(|h| h.token().to_string()))
        .ok_or(ApiError::Unauthorized)?;

    let auth_user = authenticate_bearer_token(&token, &state).await?;

    RbacChecker::check_permission(auth_user.id, project_id, Permission::ViewProject, &state.db)
        .await?;

    let session = acpms_services::ProjectAssistantSessionService::new(state.db.clone())
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

    let ws = ws_upgrade_with_protocol(ws, &headers);
    Ok(ws.on_upgrade(move |socket| handle_assistant_logs_socket(socket, session_id, state)))
}

async fn handle_assistant_logs_socket(socket: WebSocket, session_id: Uuid, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    let mut rx = state.broadcast_tx.subscribe();

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    while let Ok(msg) = rx.recv().await {
        let should_send = match &msg {
            AgentEvent::AssistantLog(log) => log.session_id == session_id,
            _ => false,
        };
        if should_send {
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    }

    recv_task.abort();
}

fn is_terminal_auth_session_status(status: &AuthSessionStatus) -> bool {
    matches!(
        status,
        AuthSessionStatus::Succeeded
            | AuthSessionStatus::Failed
            | AuthSessionStatus::Cancelled
            | AuthSessionStatus::TimedOut
    )
}

async fn handle_agent_auth_session_socket(
    socket: WebSocket,
    state: AppState,
    user_id: Uuid,
    session_id: Uuid,
    since_seq: Option<u64>,
) {
    let (mut sender, mut receiver) = socket.split();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    let mut tick = time::interval(Duration::from_millis(500));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut last_sent_seq: Option<u64> = None;
    let mut sent_initial = false;

    loop {
        tokio::select! {
            _ = tick.tick() => {
                let Some(current) = state.auth_session_store.get_owned(session_id, user_id).await else {
                    break;
                };

                let current_seq = current.last_seq;
                if !sent_initial {
                    if let Some(requested_since_seq) = since_seq {
                        if requested_since_seq > current_seq {
                            let payload = match serde_json::to_string(&AgentAuthSessionWsMessage::GapDetected {
                                requested_since_seq,
                                max_available_sequence_id: current_seq,
                            }) {
                                Ok(payload) => payload,
                                Err(_) => break,
                            };
                            let _ = sender.send(Message::Text(payload)).await;
                            break;
                        }
                    }

                    let payload = match serde_json::to_string(&AgentAuthSessionWsMessage::Snapshot {
                        sequence_id: current_seq,
                        session: AgentAuthSessionDoc::from(current.clone()),
                    }) {
                        Ok(payload) => payload,
                        Err(_) => break,
                    };
                    if sender.send(Message::Text(payload)).await.is_err() {
                        break;
                    }
                    sent_initial = true;
                    last_sent_seq = Some(current_seq);

                    if is_terminal_auth_session_status(&current.status) {
                        break;
                    }
                    continue;
                }

                if current_seq > last_sent_seq.unwrap_or(0) {
                    let payload = match serde_json::to_string(&AgentAuthSessionWsMessage::Upsert {
                        sequence_id: current_seq,
                        session: AgentAuthSessionDoc::from(current.clone()),
                    }) {
                        Ok(payload) => payload,
                        Err(_) => break,
                    };
                    if sender.send(Message::Text(payload)).await.is_err() {
                        break;
                    }
                    last_sent_seq = Some(current_seq);
                }

                if is_terminal_auth_session_status(&current.status) {
                    break;
                }
            }
            _ = &mut recv_task => {
                break;
            }
        }
    }

    recv_task.abort();
}

async fn handle_execution_processes_socket(
    socket: WebSocket,
    state: AppState,
    attempt_id: Uuid,
    since_seq: Option<u64>,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.broadcast_tx.subscribe();
    let mut known: std::collections::HashMap<Uuid, ExecutionProcessWsDto> =
        std::collections::HashMap::new();
    let sequence_key = execution_processes_stream_sequence_key(attempt_id);
    let snapshot_sequence_id = match state
        .patch_store
        .reserve_collection_snapshot_sequence(&sequence_key, since_seq)
        .await
    {
        Ok(sequence_id) => sequence_id,
        Err((requested_since_seq, max_available_sequence_id)) => {
            let payload =
                match serde_json::to_string(&SequencedCollectionGapWsMessage::GapDetected {
                    requested_since_seq,
                    max_available_sequence_id,
                }) {
                    Ok(payload) => payload,
                    Err(error) => {
                        tracing::warn!(
                            attempt_id = %attempt_id,
                            error = %error,
                            "Failed to serialize execution process gap_detected message"
                        );
                        return;
                    }
                };
            let _ = sender.send(Message::Text(payload)).await;
            return;
        }
    };

    let initial_processes = match fetch_attempt_execution_processes(&state, attempt_id).await {
        Ok(processes) => processes,
        Err(error) => {
            tracing::warn!(
                attempt_id = %attempt_id,
                error = %error,
                "Failed to fetch initial execution process snapshot"
            );
            vec![]
        }
    };

    for process in &initial_processes {
        known.insert(process.id, process.clone());
    }

    let snapshot_payload = match serde_json::to_string(&SequencedExecutionProcessesWsMessage {
        sequence_id: snapshot_sequence_id,
        message: ExecutionProcessesWsMessage::Snapshot {
            processes: initial_processes,
        },
    }) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!(
                attempt_id = %attempt_id,
                error = %error,
                "Failed to serialize execution process snapshot"
            );
            return;
        }
    };

    if sender.send(Message::Text(snapshot_payload)).await.is_err() {
        return;
    }

    // Drain client messages to detect closure.
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    let mut sync_interval = time::interval(Duration::from_secs(2));
    sync_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = sync_interval.tick() => {
                if let Err(error) =
                    sync_execution_processes_snapshot(
                        &mut sender,
                        &state,
                        attempt_id,
                        &mut known,
                        &sequence_key,
                    ).await
                {
                    tracing::warn!(
                        attempt_id = %attempt_id,
                        error = %error,
                        "Execution process periodic sync failed"
                    );
                    break;
                }
            }
            recv_result = rx.recv() => {
                let Ok(msg) = recv_result else {
                    break;
                };

                if agent_event_attempt_id(&msg) != Some(attempt_id) {
                    continue;
                }

                if let Err(error) =
                    sync_execution_processes_snapshot(
                        &mut sender,
                        &state,
                        attempt_id,
                        &mut known,
                        &sequence_key,
                    ).await
                {
                    tracing::warn!(
                        attempt_id = %attempt_id,
                        error = %error,
                        "Execution process event-triggered sync failed"
                    );
                    break;
                }
            }
            _ = &mut recv_task => {
                break;
            }
        }
    }

    recv_task.abort();
}

async fn handle_approvals_socket(
    socket: WebSocket,
    state: AppState,
    scope: ApprovalScope,
    attempt_filter: Option<Uuid>,
    projection: ApprovalsProjection,
    since_seq: Option<u64>,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.broadcast_tx.subscribe();
    let mut known: std::collections::HashMap<Uuid, ApprovalWsDto> =
        std::collections::HashMap::new();
    let sequence_key = approvals_stream_sequence_key(scope);

    let initial_approvals = match fetch_approvals_for_scope(&state, scope).await {
        Ok(rows) => rows,
        Err(error) => {
            tracing::warn!(error = %error, "Failed to fetch initial approvals snapshot");
            vec![]
        }
    };
    for approval in &initial_approvals {
        known.insert(approval.id, approval.clone());
    }

    let snapshot_sequence_id = if projection == ApprovalsProjection::Patch {
        match state
            .patch_store
            .reserve_collection_snapshot_sequence(&sequence_key, since_seq)
            .await
        {
            Ok(sequence_id) => Some(sequence_id),
            Err((requested_since_seq, max_available_sequence_id)) => {
                let payload = match serde_json::to_string(&ApprovalsPatchWsMessage::GapDetected {
                    requested_since_seq,
                    max_available_sequence_id,
                }) {
                    Ok(payload) => payload,
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            "Failed to serialize approvals gap_detected message"
                        );
                        return;
                    }
                };
                let _ = sender.send(Message::Text(payload)).await;
                return;
            }
        }
    } else {
        None
    };

    let initial_payload = match build_initial_approvals_projection_payload(
        projection,
        &known,
        &initial_approvals,
        snapshot_sequence_id,
    ) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!(error = %error, "Failed to build approvals initial projection payload");
            return;
        }
    };

    let InitialApprovalsProjectionPayload::Snapshot { payload } = initial_payload;
    if sender.send(Message::Text(payload)).await.is_err() {
        return;
    }

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    let mut sync_interval = time::interval(Duration::from_secs(1));
    sync_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = sync_interval.tick() => {
                let sync_result = if projection == ApprovalsProjection::Patch {
                    sync_approvals_patch_snapshot(
                        &mut sender,
                        &state,
                        scope,
                        &mut known,
                        &sequence_key,
                    ).await
                } else {
                    sync_approvals_snapshot(&mut sender, &state, scope, &mut known).await
                };
                if let Err(error) = sync_result {
                    tracing::warn!(error = %error, "Approvals periodic sync failed");
                    break;
                }
            }
            recv_result = rx.recv() => {
                let Ok(msg) = recv_result else {
                    break;
                };

                let should_sync = if let Some(target_attempt_id) = attempt_filter {
                    agent_event_attempt_id(&msg) == Some(target_attempt_id)
                } else {
                    approval_scope_attempt_filter(scope, &msg)
                };

                if !should_sync {
                    continue;
                }

                let sync_result = if projection == ApprovalsProjection::Patch {
                    sync_approvals_patch_snapshot(
                        &mut sender,
                        &state,
                        scope,
                        &mut known,
                        &sequence_key,
                    ).await
                } else {
                    sync_approvals_snapshot(&mut sender, &state, scope, &mut known).await
                };
                if let Err(error) = sync_result {
                    tracing::warn!(error = %error, "Approvals event-triggered sync failed");
                    break;
                }
            }
            _ = &mut recv_task => {
                break;
            }
        }
    }

    recv_task.abort();
}

async fn handle_execution_process_socket(
    socket: WebSocket,
    stream_ctx: ExecutionProcessStreamContext,
    state: AppState,
    mode: ExecutionProcessLogMode,
    since_seq: Option<u64>,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.broadcast_tx.subscribe();
    let mut latest_sent_sequence_id: u64 = 0;

    // Drain client messages to detect socket close.
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    let initial_events =
        match load_initial_execution_process_log_events(&state, &stream_ctx, mode).await {
            Ok(events) => events,
            Err(error) => {
                tracing::warn!(
                    process_id = %stream_ctx.process_id,
                    error = %error,
                    "Failed to load initial execution process logs"
                );
                recv_task.abort();
                return;
            }
        };

    for event in initial_events {
        let sequence_id = event.sequence_id;
        if sequence_id > latest_sent_sequence_id {
            latest_sent_sequence_id = sequence_id;
        }
        if since_seq.map(|since| sequence_id <= since).unwrap_or(false) {
            continue;
        }
        if let Err(error) = send_sequenced_agent_event(&mut sender, sequence_id, event.event).await
        {
            tracing::warn!(
                process_id = %stream_ctx.process_id,
                error = %error,
                "Failed to send initial sequenced execution process event"
            );
            recv_task.abort();
            return;
        }
    }

    let terminal_event =
        match load_execution_process_terminal_status_event(&state, &stream_ctx).await {
            Ok(event) => event,
            Err(error) => {
                tracing::warn!(
                    process_id = %stream_ctx.process_id,
                    error = %error,
                    "Failed to load terminal execution process status snapshot"
                );
                None
            }
        };

    if let Some(event) = terminal_event {
        let sequence_id = match resolve_terminal_status_sequence_id(&state, &stream_ctx, mode).await
        {
            Ok(sequence_id) => sequence_id,
            Err(error) => {
                tracing::warn!(
                    process_id = %stream_ctx.process_id,
                    error = %error,
                    "Failed to resolve terminal status sequence id"
                );
                recv_task.abort();
                return;
            }
        };
        if sequence_id > latest_sent_sequence_id {
            latest_sent_sequence_id = sequence_id;
        }
        if !since_seq.map(|since| sequence_id <= since).unwrap_or(false) {
            if let Err(error) = send_sequenced_agent_event(&mut sender, sequence_id, event).await {
                tracing::warn!(
                    process_id = %stream_ctx.process_id,
                    error = %error,
                    "Failed to send terminal sequenced execution process event"
                );
                recv_task.abort();
                return;
            }
        }
    }

    let max_available_sequence_id = latest_sent_sequence_id;
    if let Some(since) = since_seq {
        if since > max_available_sequence_id {
            let payload = match serde_json::to_string(&ExecutionProcessLogWsMessage::GapDetected {
                requested_since_seq: since,
                max_available_sequence_id,
            }) {
                Ok(payload) => payload,
                Err(error) => {
                    tracing::warn!(
                        process_id = %stream_ctx.process_id,
                        error = %error,
                        "Failed to serialize gap_detected message"
                    );
                    recv_task.abort();
                    return;
                }
            };

            let _ = sender.send(Message::Text(payload)).await;
            recv_task.abort();
            return;
        }
    }

    loop {
        tokio::select! {
            recv_result = rx.recv() => {
                let Ok(msg) = recv_result else {
                    break;
                };

                let should_send = match &msg {
                    AgentEvent::Log(log) => {
                        matches_log_mode(&log.log_type, mode) && log_event_in_process_window(log, &stream_ctx)
                    }
                    AgentEvent::Status(status) => {
                        is_terminal_attempt_status(&status.status)
                            && status_event_in_process_window(status, &stream_ctx)
                    }
                    AgentEvent::ApprovalRequest(_) | AgentEvent::UserMessage(_) | AgentEvent::AssistantLog(_) => false,
                };

                if should_send {
                    let sequence_id = match &msg {
                        AgentEvent::Log(log) => match resolve_sequence_id_for_live_log_event(
                            &state,
                            &stream_ctx,
                            mode,
                            log,
                        )
                        .await
                        {
                            Ok(Some(sequence_id)) => sequence_id,
                            Ok(None) => continue,
                            Err(error) => {
                                tracing::warn!(
                                    process_id = %stream_ctx.process_id,
                                    error = %error,
                                    "Failed to resolve live log sequence id"
                                );
                                continue;
                            }
                        },
                        AgentEvent::Status(_) => match resolve_terminal_status_sequence_id(
                            &state,
                            &stream_ctx,
                            mode,
                        )
                        .await
                        {
                            Ok(sequence_id) => sequence_id,
                            Err(error) => {
                                tracing::warn!(
                                    process_id = %stream_ctx.process_id,
                                    error = %error,
                                    "Failed to resolve live status sequence id"
                                );
                                continue;
                            }
                        },
                        AgentEvent::ApprovalRequest(_) | AgentEvent::UserMessage(_) | AgentEvent::AssistantLog(_) => continue,
                    };

                    if sequence_id <= latest_sent_sequence_id {
                        continue;
                    }
                    latest_sent_sequence_id = sequence_id;
                    if let Err(error) = send_sequenced_agent_event(&mut sender, sequence_id, msg).await {
                        tracing::warn!(
                            process_id = %stream_ctx.process_id,
                            error = %error,
                            "Failed to send live sequenced execution process event"
                        );
                        break;
                    }
                }
            }
            _ = &mut recv_task => {
                break;
            }
        }
    }

    recv_task.abort();
}

async fn fetch_dashboard_statuses(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
) -> Result<Vec<AgentActivityStatusDto>, ApiError> {
    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            Uuid,
            String,
            String,
            AttemptStatus,
            Option<chrono::DateTime<chrono::Utc>>,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        r#"
        SELECT
            ta.id,
            ta.task_id,
            t.title as task_title,
            p.name as project_name,
            ta.status,
            ta.started_at,
            ta.created_at
        FROM task_attempts ta
        JOIN tasks t ON t.id = ta.task_id
        JOIN projects p ON p.id = t.project_id
        WHERE (
                $1
                OR EXISTS (
                    SELECT 1
                    FROM project_members pm
                    WHERE pm.project_id = p.id
                      AND pm.user_id = $2
                )
            )
          AND (
                ta.status = 'running'
                OR (ta.status = 'queued' AND ta.created_at > NOW() - INTERVAL '1 hour')
                OR (ta.status NOT IN ('queued', 'running') AND ta.created_at > NOW() - INTERVAL '1 hour')
              )
        ORDER BY
            CASE ta.status
                WHEN 'running' THEN 1
                WHEN 'queued' THEN 2
                ELSE 3
            END,
            ta.created_at DESC
        LIMIT $3
        "#,
    )
    .bind(is_admin)
    .bind(user_id)
    .bind(DASHBOARD_STATUS_LIMIT)
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let statuses = rows
        .into_iter()
        .enumerate()
        .map(
            |(i, (id, _task_id, task_title, project_name, status, started_at, created_at))| {
                AgentActivityStatusDto {
                    id,
                    name: format!("Agent-{}", i + 1),
                    task_title,
                    project_name,
                    status,
                    started_at,
                    created_at,
                }
            },
        )
        .collect();

    Ok(statuses)
}

async fn fetch_attempt_status_for_user(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    attempt_id: Uuid,
) -> Result<Option<AgentActivityStatusDto>, ApiError> {
    let row = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            AttemptStatus,
            Option<chrono::DateTime<chrono::Utc>>,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        r#"
        SELECT
            ta.id,
            t.title as task_title,
            p.name as project_name,
            ta.status,
            ta.started_at,
            ta.created_at
        FROM task_attempts ta
        JOIN tasks t ON t.id = ta.task_id
        JOIN projects p ON p.id = t.project_id
        WHERE ta.id = $1
          AND (
                $2
                OR EXISTS (
                    SELECT 1
                    FROM project_members pm
                    WHERE pm.project_id = p.id
                      AND pm.user_id = $3
                )
              )
          AND (
                ta.status = 'running'
                OR (ta.status = 'queued' AND ta.created_at > NOW() - INTERVAL '1 hour')
                OR (ta.status NOT IN ('queued', 'running') AND ta.created_at > NOW() - INTERVAL '1 hour')
              )
        LIMIT 1
        "#,
    )
    .bind(attempt_id)
    .bind(is_admin)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(row.map(
        |(id, task_title, project_name, status, started_at, created_at)| AgentActivityStatusDto {
            id,
            name: "Agent".to_string(),
            task_title,
            project_name,
            status,
            started_at,
            created_at,
        },
    ))
}

async fn handle_agent_activity_status_socket(
    socket: WebSocket,
    state: AppState,
    user_id: Uuid,
    is_admin: bool,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.broadcast_tx.subscribe();

    // Drain client messages to detect close and keep connection healthy.
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    let initial_snapshot = match fetch_dashboard_statuses(&state, user_id, is_admin).await {
        Ok(statuses) => AgentActivityStatusWsMessage::Snapshot { statuses },
        Err(error) => {
            tracing::warn!(
                user_id = %user_id,
                error = %error,
                "Failed to load dashboard status snapshot for websocket"
            );
            AgentActivityStatusWsMessage::Snapshot { statuses: vec![] }
        }
    };

    if let Ok(json) = serde_json::to_string(&initial_snapshot) {
        if sender.send(Message::Text(json)).await.is_err() {
            recv_task.abort();
            return;
        }
    }

    let mut snapshot_interval = time::interval(Duration::from_secs(10));
    snapshot_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = snapshot_interval.tick() => {
                match fetch_dashboard_statuses(&state, user_id, is_admin).await {
                    Ok(statuses) => {
                        let outbound = AgentActivityStatusWsMessage::Snapshot { statuses };
                        if let Ok(json) = serde_json::to_string(&outbound) {
                            if sender.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(error) => {
                        tracing::warn!(
                            user_id = %user_id,
                            error = %error,
                            "Failed to refresh dashboard status snapshot for websocket"
                        );
                    }
                }
            }
            recv_result = rx.recv() => {
                let Ok(msg) = recv_result else {
                    break;
                };
                let AgentEvent::Status(status_msg) = msg else {
                    continue;
                };

                let outbound = match fetch_attempt_status_for_user(
                    &state,
                    user_id,
                    is_admin,
                    status_msg.attempt_id,
                )
                .await
                {
                    Ok(Some(status)) => AgentActivityStatusWsMessage::Upsert { status },
                    Ok(None) => AgentActivityStatusWsMessage::Remove {
                        attempt_id: status_msg.attempt_id,
                    },
                    Err(error) => {
                        tracing::warn!(
                            user_id = %user_id,
                            attempt_id = %status_msg.attempt_id,
                            error = %error,
                            "Failed to resolve dashboard status update for websocket"
                        );
                        continue;
                    }
                };

                if let Ok(json) = serde_json::to_string(&outbound) {
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
            _ = &mut recv_task => {
                break;
            }
        }
    }

    recv_task.abort();
}

/// WebSocket handler for project-level agent logs.
/// Streams all agent events from running attempts in a specific project.
///
/// ## Security
/// - Requires valid JWT token in `Sec-WebSocket-Protocol` (`acpms-bearer`, `<token>`)
///   or in Authorization header (non-browser clients)
/// - Verifies user is a member of the project
///
/// ## Protocol
/// - Streams AgentEvent messages (Log and Status) for all active agents in the project
/// - Includes task metadata in events for multi-agent display
pub async fn project_ws_handler(
    ws: WebSocketUpgrade,
    Path(project_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<impl IntoResponse, ApiError> {
    let token = extract_token_from_ws_protocol(&headers)
        .or_else(|| auth_header.map(|h| h.token().to_string()))
        .ok_or(ApiError::Unauthorized)?;

    let auth_user = authenticate_bearer_token(&token, &state).await?;
    let user_id = auth_user.id;

    // Check if project exists
    let project_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1)")
            .bind(project_id)
            .fetch_one(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    if !project_exists {
        return Err(ApiError::NotFound(format!(
            "Project {} not found",
            project_id
        )));
    }

    // Check permission using RBAC (includes system admin bypass).
    RbacChecker::check_permission(user_id, project_id, Permission::ViewProject, &state.db).await?;

    // User is authenticated and authorized - upgrade to WebSocket
    let ws = ws_upgrade_with_protocol(ws, &headers);
    Ok(ws.on_upgrade(move |socket| handle_project_socket(socket, project_id, state)))
}

/// Extended agent event with task context for multi-agent display
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectAgentEvent {
    #[serde(flatten)]
    pub event: AgentEvent,
    pub task_id: Uuid,
    pub task_title: String,
}

async fn handle_project_socket(socket: WebSocket, project_id: Uuid, state: AppState) {
    let (mut sender, _receiver) = socket.split();

    // Subscribe to broadcast channel
    let mut rx = state.broadcast_tx.subscribe();

    // Cache for attempt_id -> (task_id, task_title) mapping
    // This avoids repeated DB queries for the same attempt
    let mut attempt_cache: std::collections::HashMap<Uuid, Option<(Uuid, String)>> =
        std::collections::HashMap::new();

    while let Ok(msg) = rx.recv().await {
        let Some(attempt_id) = agent_event_attempt_id(&msg) else {
            continue; // Skip AssistantLog and other non-attempt events
        };

        // Check cache first, then query DB if not cached
        let task_info = if let Some(cached) = attempt_cache.get(&attempt_id) {
            cached.clone()
        } else {
            // Query to get task info for this attempt
            let result: Option<(Uuid, String)> = sqlx::query_as(
                r#"
                SELECT t.id, t.title
                FROM task_attempts ta
                JOIN tasks t ON ta.task_id = t.id
                WHERE ta.id = $1 AND t.project_id = $2
                "#,
            )
            .bind(attempt_id)
            .bind(project_id)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

            attempt_cache.insert(attempt_id, result.clone());
            result
        };

        // Only send if the attempt belongs to this project
        if let Some((task_id, task_title)) = task_info {
            let project_event = ProjectAgentEvent {
                event: msg,
                task_id,
                task_title,
            };

            if let Ok(json) = serde_json::to_string(&project_event) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    }
}

/// Get list of currently active agents in a project
/// Returns attempt IDs and task info for agents with status 'running'
pub async fn get_project_active_agents(
    Path(project_id): Path<Uuid>,
    State(state): State<AppState>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<impl IntoResponse, ApiError> {
    let auth_user = authenticate_bearer_token(auth.token(), &state).await?;
    let user_id = auth_user.id;

    // Check permission using RBAC (includes system admin bypass).
    RbacChecker::check_permission(user_id, project_id, Permission::ViewProject, &state.db).await?;

    #[derive(sqlx::FromRow, serde::Serialize)]
    struct ActiveAgent {
        attempt_id: Uuid,
        task_id: Uuid,
        task_title: String,
        task_type: String,
        started_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let active_agents: Vec<ActiveAgent> = sqlx::query_as(
        r#"
        SELECT
            ta.id as attempt_id,
            t.id as task_id,
            t.title as task_title,
            t.task_type::text as task_type,
            ta.started_at
        FROM task_attempts ta
        JOIN tasks t ON ta.task_id = t.id
        WHERE t.project_id = $1 AND ta.status = 'running'
        ORDER BY ta.started_at DESC
        "#,
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    Ok(axum::Json(serde_json::json!({
        "success": true,
        "data": active_agents
    })))
}

#[cfg(test)]
mod tests {
    use super::{
        approval_scope_attempt_filter, approvals_stream_sequence_key,
        build_approvals_patch_operations, build_approvals_snapshot_data,
        build_initial_approvals_projection_payload, execution_processes_stream_sequence_key,
        extract_token_from_ws_protocol, is_terminal_attempt_status, log_event_in_process_window,
        matches_log_mode, sorted_map_ids, sorted_removed_ids, status_event_in_process_window,
        timestamp_in_process_window, AgentAuthSessionWsMessage, ApprovalScope, ApprovalWsDto,
        ApprovalsProjection, ExecutionProcessLogMode, ExecutionProcessStreamContext,
        InitialApprovalsProjectionPayload,
    };
    use crate::routes::agent::AgentAuthSessionDoc;
    use crate::services::agent_auth::{AuthFlowType, AuthSessionStatus};
    use acpms_db::models::AttemptStatus;
    use acpms_executors::{AgentEvent, LogMessage, StatusMessage};
    use axum::http::{header, HeaderMap, HeaderValue};
    use chrono::{DateTime, Duration, Utc};
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn extract_token_from_ws_protocol_parses_bearer_protocol() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("acpms-bearer, abc.def.ghi"),
        );

        assert_eq!(
            extract_token_from_ws_protocol(&headers).as_deref(),
            Some("abc.def.ghi")
        );
    }

    #[test]
    fn extract_token_from_ws_protocol_returns_none_without_marker() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("graphql-ws"),
        );

        assert_eq!(extract_token_from_ws_protocol(&headers), None);
    }

    #[test]
    fn execution_processes_stream_sequence_key_is_stable() {
        let attempt_id =
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("valid attempt id");
        assert_eq!(
            execution_processes_stream_sequence_key(attempt_id),
            format!("/collections/execution-processes/{}", attempt_id)
        );
    }

    #[test]
    fn approvals_stream_sequence_key_is_scope_specific() {
        let attempt_id =
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("valid attempt id");
        let process_id =
            Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("valid process id");

        assert_eq!(
            approvals_stream_sequence_key(ApprovalScope::Attempt(attempt_id)),
            format!("/collections/approvals/attempt/{}", attempt_id)
        );
        assert_eq!(
            approvals_stream_sequence_key(ApprovalScope::ExecutionProcess(process_id)),
            format!("/collections/approvals/process/{}", process_id)
        );
    }

    #[test]
    fn sorted_removed_ids_returns_sorted_missing_keys() {
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").expect("valid uuid a");
        let id_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").expect("valid uuid b");
        let id_c = Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").expect("valid uuid c");

        let mut known = HashMap::new();
        known.insert(id_c, 1u8);
        known.insert(id_a, 2u8);
        known.insert(id_b, 3u8);

        let mut current = HashMap::new();
        current.insert(id_b, 9u8);

        let removed = sorted_removed_ids(&known, &current);
        assert_eq!(removed, vec![id_a, id_c]);
    }

    #[test]
    fn sorted_map_ids_returns_sorted_keys() {
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").expect("valid uuid a");
        let id_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").expect("valid uuid b");
        let id_c = Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").expect("valid uuid c");

        let mut map = HashMap::new();
        map.insert(id_c, 3u8);
        map.insert(id_a, 1u8);
        map.insert(id_b, 2u8);

        assert_eq!(sorted_map_ids(&map), vec![id_a, id_b, id_c]);
    }

    #[test]
    fn approval_scope_attempt_filter_matches_attempt_scoped_events() {
        let target_attempt_id =
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("valid attempt id");
        let other_attempt_id =
            Uuid::parse_str("99999999-9999-9999-9999-999999999999").expect("valid attempt id");
        let now = DateTime::parse_from_rfc3339("2026-02-26T10:00:00Z")
            .expect("valid datetime")
            .with_timezone(&Utc);

        let target_log = AgentEvent::Log(LogMessage {
            attempt_id: target_attempt_id,
            log_type: "stdout".to_string(),
            content: "ok".to_string(),
            timestamp: now,
            id: None,
            created_at: Some(now),
            tool_name: None,
        });
        let other_status = AgentEvent::Status(StatusMessage {
            attempt_id: other_attempt_id,
            status: AttemptStatus::Running,
            timestamp: now,
        });

        assert!(approval_scope_attempt_filter(
            ApprovalScope::Attempt(target_attempt_id),
            &target_log
        ));
        assert!(!approval_scope_attempt_filter(
            ApprovalScope::Attempt(target_attempt_id),
            &other_status
        ));
    }

    #[test]
    fn approval_scope_attempt_filter_allows_all_attempts_for_process_scope() {
        let process_scope = ApprovalScope::ExecutionProcess(Uuid::new_v4());
        let now = DateTime::parse_from_rfc3339("2026-02-26T10:00:00Z")
            .expect("valid datetime")
            .with_timezone(&Utc);

        let log_event = AgentEvent::Log(LogMessage {
            attempt_id: Uuid::new_v4(),
            log_type: "stdout".to_string(),
            content: "ok".to_string(),
            timestamp: now,
            id: None,
            created_at: Some(now),
            tool_name: None,
        });
        let status_event = AgentEvent::Status(StatusMessage {
            attempt_id: Uuid::new_v4(),
            status: AttemptStatus::Success,
            timestamp: now,
        });

        assert!(approval_scope_attempt_filter(process_scope, &log_event));
        assert!(approval_scope_attempt_filter(process_scope, &status_event));
    }

    fn build_approval(id: Uuid, status: &str, created_at: &str) -> ApprovalWsDto {
        ApprovalWsDto {
            id,
            attempt_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111")
                .expect("valid attempt id"),
            execution_process_id: Some(
                Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("valid process id"),
            ),
            tool_use_id: format!("tool-{}", id),
            tool_name: "Bash".to_string(),
            status: status.to_string(),
            created_at: DateTime::parse_from_rfc3339(created_at)
                .expect("valid created_at")
                .with_timezone(&Utc),
            responded_at: None,
        }
    }

    #[test]
    fn build_approvals_patch_operations_are_deterministic() {
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").expect("valid uuid a");
        let id_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").expect("valid uuid b");
        let id_c = Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").expect("valid uuid c");

        let mut known = HashMap::new();
        known.insert(
            id_b,
            build_approval(id_b, "pending", "2026-02-26T10:01:00.000Z"),
        );
        known.insert(
            id_c,
            build_approval(id_c, "pending", "2026-02-26T10:02:00.000Z"),
        );

        let mut current = HashMap::new();
        current.insert(
            id_a,
            build_approval(id_a, "pending", "2026-02-26T10:00:00.000Z"),
        );
        current.insert(
            id_b,
            build_approval(id_b, "approved", "2026-02-26T10:01:00.000Z"),
        );

        let operations =
            build_approvals_patch_operations(&known, &current).expect("operations should build");
        assert_eq!(operations.len(), 3);

        assert_eq!(operations[0].op, "add");
        assert_eq!(operations[0].path, format!("/approvals/{}", id_a));
        assert!(operations[0].value.is_some());

        assert_eq!(operations[1].op, "replace");
        assert_eq!(operations[1].path, format!("/approvals/{}", id_b));
        assert!(operations[1].value.is_some());

        assert_eq!(operations[2].op, "remove");
        assert_eq!(operations[2].path, format!("/approvals/{}", id_c));
        assert!(operations[2].value.is_none());
    }

    #[test]
    fn build_approvals_patch_operations_empty_when_no_changes() {
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").expect("valid uuid a");
        let mut known = HashMap::new();
        known.insert(
            id_a,
            build_approval(id_a, "pending", "2026-02-26T10:00:00.000Z"),
        );
        let current = known.clone();

        let operations =
            build_approvals_patch_operations(&known, &current).expect("operations should build");
        assert!(operations.is_empty());
    }

    #[test]
    fn build_approvals_snapshot_data_is_sorted_by_id() {
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").expect("valid uuid a");
        let id_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").expect("valid uuid b");
        let id_c = Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").expect("valid uuid c");

        let mut approvals = HashMap::new();
        approvals.insert(
            id_c,
            build_approval(id_c, "pending", "2026-02-26T10:02:00.000Z"),
        );
        approvals.insert(
            id_a,
            build_approval(id_a, "pending", "2026-02-26T10:00:00.000Z"),
        );
        approvals.insert(
            id_b,
            build_approval(id_b, "pending", "2026-02-26T10:01:00.000Z"),
        );

        let snapshot = build_approvals_snapshot_data(&approvals).expect("snapshot should build");
        let approvals_obj = snapshot
            .get("approvals")
            .and_then(|value| value.as_object())
            .expect("approvals object");
        let keys: Vec<String> = approvals_obj.keys().cloned().collect();

        assert_eq!(
            keys,
            vec![id_a.to_string(), id_b.to_string(), id_c.to_string()]
        );
    }

    #[test]
    fn build_initial_approvals_projection_payload_patch_uses_provided_snapshot_sequence() {
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").expect("valid uuid a");
        let mut known = HashMap::new();
        let approval = build_approval(id_a, "pending", "2026-02-26T10:00:00.000Z");
        known.insert(id_a, approval.clone());
        let initial_approvals = vec![approval];

        let payload = build_initial_approvals_projection_payload(
            ApprovalsProjection::Patch,
            &known,
            &initial_approvals,
            Some(5),
        )
        .expect("patch payload should build");

        let InitialApprovalsProjectionPayload::Snapshot { payload } = payload;

        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("valid json");
        assert_eq!(
            parsed.get("type").and_then(|v| v.as_str()),
            Some("snapshot")
        );
        assert_eq!(parsed.get("sequence_id").and_then(|v| v.as_u64()), Some(5));
    }

    #[test]
    fn build_initial_approvals_projection_payload_patch_requires_snapshot_sequence() {
        let known: HashMap<Uuid, ApprovalWsDto> = HashMap::new();
        let initial_approvals: Vec<ApprovalWsDto> = Vec::new();

        let error = match build_initial_approvals_projection_payload(
            ApprovalsProjection::Patch,
            &known,
            &initial_approvals,
            None,
        ) {
            Ok(_) => panic!("missing sequence should fail"),
            Err(error) => error,
        };

        let crate::error::ApiError::Internal(message) = error else {
            panic!("expected internal error");
        };
        assert_eq!(
            message,
            "Missing snapshot sequence_id for approvals patch projection"
        );
    }

    #[test]
    fn build_initial_approvals_projection_payload_legacy_uses_snapshot_without_sequence() {
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").expect("valid uuid a");
        let mut known = HashMap::new();
        let approval = build_approval(id_a, "pending", "2026-02-26T10:00:00.000Z");
        known.insert(id_a, approval.clone());
        let initial_approvals = vec![approval];

        let payload = build_initial_approvals_projection_payload(
            ApprovalsProjection::Legacy,
            &known,
            &initial_approvals,
            None,
        )
        .expect("legacy payload should build");

        let InitialApprovalsProjectionPayload::Snapshot { payload } = payload;

        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("valid json");
        assert_eq!(
            parsed.get("type").and_then(|v| v.as_str()),
            Some("snapshot")
        );
        assert!(parsed.get("sequence_id").is_none());
        assert_eq!(
            parsed
                .get("approvals")
                .and_then(|v| v.as_array())
                .map(|v| v.len()),
            Some(1)
        );
    }

    #[test]
    fn timestamp_in_process_window_includes_lower_and_excludes_upper() {
        let lower = DateTime::parse_from_rfc3339("2026-02-26T10:00:00Z")
            .expect("valid lower")
            .with_timezone(&Utc);
        let upper = lower + Duration::seconds(30);

        assert!(timestamp_in_process_window(lower, lower, Some(upper)));
        assert!(timestamp_in_process_window(
            lower + Duration::seconds(1),
            lower,
            Some(upper)
        ));
        assert!(!timestamp_in_process_window(upper, lower, Some(upper)));
    }

    fn sample_auth_session_doc() -> AgentAuthSessionDoc {
        AgentAuthSessionDoc {
            session_id: Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")
                .expect("valid session id"),
            provider: "openai-codex".to_string(),
            flow_type: AuthFlowType::DeviceFlow,
            status: AuthSessionStatus::WaitingUserAction,
            created_at: DateTime::parse_from_rfc3339("2026-02-27T10:00:00Z")
                .expect("valid created_at")
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339("2026-02-27T10:00:01Z")
                .expect("valid updated_at")
                .with_timezone(&Utc),
            expires_at: DateTime::parse_from_rfc3339("2026-02-27T10:05:00Z")
                .expect("valid expires_at")
                .with_timezone(&Utc),
            process_pid: Some(1234),
            allowed_loopback_port: None,
            last_seq: 3,
            last_error: None,
            result: Some("waiting for code".to_string()),
            action_url: Some("https://github.com/login/device".to_string()),
            action_code: Some("ABCD-1234".to_string()),
            action_hint: Some("Open URL and enter code".to_string()),
        }
    }

    #[test]
    fn agent_auth_snapshot_message_contract_is_stable() {
        let payload = serde_json::to_value(AgentAuthSessionWsMessage::Snapshot {
            sequence_id: 7,
            session: sample_auth_session_doc(),
        })
        .expect("serialize auth snapshot");

        assert_eq!(
            payload.get("type").and_then(|v| v.as_str()),
            Some("snapshot")
        );
        assert_eq!(payload.get("sequence_id").and_then(|v| v.as_u64()), Some(7));
        let session = payload
            .get("session")
            .and_then(|v| v.as_object())
            .expect("session payload");
        assert_eq!(
            session.get("provider").and_then(|v| v.as_str()),
            Some("openai-codex")
        );
        assert_eq!(
            session.get("flow_type").and_then(|v| v.as_str()),
            Some("device_flow")
        );
        assert_eq!(
            session.get("status").and_then(|v| v.as_str()),
            Some("waiting_user_action")
        );
        assert_eq!(
            session.get("action_code").and_then(|v| v.as_str()),
            Some("ABCD-1234")
        );
    }

    #[test]
    fn agent_auth_gap_message_contract_is_stable() {
        let payload = serde_json::to_value(AgentAuthSessionWsMessage::GapDetected {
            requested_since_seq: 15,
            max_available_sequence_id: 4,
        })
        .expect("serialize auth gap payload");

        assert_eq!(
            payload.get("type").and_then(|v| v.as_str()),
            Some("gap_detected")
        );
        assert_eq!(
            payload.get("requested_since_seq").and_then(|v| v.as_u64()),
            Some(15)
        );
        assert_eq!(
            payload
                .get("max_available_sequence_id")
                .and_then(|v| v.as_u64()),
            Some(4)
        );
    }

    #[test]
    fn log_event_in_process_window_prefers_created_at_and_filters_attempt() {
        let attempt_id =
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("valid attempt id");
        let other_attempt_id =
            Uuid::parse_str("99999999-9999-9999-9999-999999999999").expect("valid attempt id");
        let lower = DateTime::parse_from_rfc3339("2026-02-26T10:00:00Z")
            .expect("valid lower")
            .with_timezone(&Utc);
        let upper = lower + Duration::minutes(1);
        let ctx = ExecutionProcessStreamContext {
            process_id: Uuid::new_v4(),
            attempt_id,
            project_id: Uuid::new_v4(),
            lower_bound: lower,
            upper_bound: Some(upper),
        };

        let in_window = LogMessage {
            attempt_id,
            log_type: "stdout".to_string(),
            content: "line".to_string(),
            timestamp: lower + Duration::seconds(100), // ignored because created_at is present.
            id: Some(Uuid::new_v4()),
            created_at: Some(lower + Duration::seconds(5)),
            tool_name: None,
        };
        assert!(log_event_in_process_window(&in_window, &ctx));

        let wrong_attempt = LogMessage {
            attempt_id: other_attempt_id,
            ..in_window.clone()
        };
        assert!(!log_event_in_process_window(&wrong_attempt, &ctx));

        let upper_bound_event = LogMessage {
            attempt_id,
            created_at: Some(upper),
            ..in_window
        };
        assert!(!log_event_in_process_window(&upper_bound_event, &ctx));
    }

    #[test]
    fn status_event_in_process_window_applies_boundary_and_attempt_filters() {
        let attempt_id =
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("valid attempt id");
        let lower = DateTime::parse_from_rfc3339("2026-02-26T10:00:00Z")
            .expect("valid lower")
            .with_timezone(&Utc);
        let upper = lower + Duration::minutes(1);
        let ctx = ExecutionProcessStreamContext {
            process_id: Uuid::new_v4(),
            attempt_id,
            project_id: Uuid::new_v4(),
            lower_bound: lower,
            upper_bound: Some(upper),
        };

        let in_window = StatusMessage {
            attempt_id,
            status: AttemptStatus::Success,
            timestamp: lower + Duration::seconds(10),
        };
        assert!(status_event_in_process_window(&in_window, &ctx));

        let at_upper_bound = StatusMessage {
            attempt_id,
            status: AttemptStatus::Success,
            timestamp: upper,
        };
        assert!(!status_event_in_process_window(&at_upper_bound, &ctx));

        let wrong_attempt = StatusMessage {
            attempt_id: Uuid::new_v4(),
            status: AttemptStatus::Success,
            timestamp: lower + Duration::seconds(10),
        };
        assert!(!status_event_in_process_window(&wrong_attempt, &ctx));
    }

    #[test]
    fn matches_log_mode_respects_raw_and_normalized_types() {
        assert!(matches_log_mode("stdout", ExecutionProcessLogMode::Raw));
        assert!(matches_log_mode(
            "process_stdout",
            ExecutionProcessLogMode::Raw
        ));
        assert!(!matches_log_mode(
            "normalized",
            ExecutionProcessLogMode::Raw
        ));

        assert!(matches_log_mode(
            "normalized",
            ExecutionProcessLogMode::Normalized
        ));
        assert!(matches_log_mode(
            "user",
            ExecutionProcessLogMode::Normalized
        ));
        assert!(matches_log_mode(
            "stdin",
            ExecutionProcessLogMode::Normalized
        ));
        assert!(!matches_log_mode(
            "stderr",
            ExecutionProcessLogMode::Normalized
        ));
    }

    #[test]
    fn is_terminal_attempt_status_accepts_only_finished_states() {
        assert!(is_terminal_attempt_status(&AttemptStatus::Success));
        assert!(is_terminal_attempt_status(&AttemptStatus::Failed));
        assert!(is_terminal_attempt_status(&AttemptStatus::Cancelled));
        assert!(!is_terminal_attempt_status(&AttemptStatus::Running));
        assert!(!is_terminal_attempt_status(&AttemptStatus::Queued));
    }
}
