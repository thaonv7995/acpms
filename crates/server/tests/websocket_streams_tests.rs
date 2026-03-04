//! WebSocket stream integration tests for execution-process and approval patch projections.

#[path = "helpers.rs"]
mod helpers;
use helpers::*;

use acpms_server::services::agent_auth::AuthFlowType;
use axum::{http::StatusCode, Router};
use chrono::{Duration as ChronoDuration, Utc};
use futures::StreamExt;
use serde_json::Value;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::{timeout, Duration};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};
use uuid::Uuid;

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

async fn spawn_router(router: Router) -> (SocketAddr, oneshot::Sender<()>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind tcp listener");
    let addr = listener.local_addr().expect("failed to get local addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    (addr, shutdown_tx, handle)
}

async fn connect_ws_with_bearer(url: &str, token: &str) -> WsStream {
    let mut request = url
        .into_client_request()
        .expect("failed to build websocket client request");
    request.headers_mut().insert(
        axum::http::header::AUTHORIZATION,
        axum::http::HeaderValue::from_str(&format!("Bearer {}", token))
            .expect("invalid auth header"),
    );

    let (ws, _) = connect_async(request)
        .await
        .expect("failed to connect websocket");
    ws
}

async fn recv_json_message(ws: &mut WsStream) -> Value {
    loop {
        let next = timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("timeout waiting websocket message")
            .expect("websocket closed unexpectedly")
            .expect("websocket read error");

        match next {
            Message::Text(text) => {
                return serde_json::from_str(&text).expect("invalid websocket json payload");
            }
            Message::Binary(bin) => {
                return serde_json::from_slice(&bin)
                    .expect("invalid websocket binary json payload");
            }
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Frame(_) => continue,
            Message::Close(frame) => panic!("websocket closed before payload: {:?}", frame),
        }
    }
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_agent_auth_session_ws_snapshot_and_upsert() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state.clone());

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let session = state
        .auth_session_store
        .create_session(
            user_id,
            "openai-codex".to_string(),
            AuthFlowType::DeviceFlow,
            300,
        )
        .await;

    let (addr, shutdown_tx, server_task) = spawn_router(router).await;
    let ws_url = format!(
        "ws://{}/api/v1/agent/auth/sessions/{}/ws",
        addr, session.session_id
    );
    let mut ws = connect_ws_with_bearer(&ws_url, &token).await;

    let snapshot = recv_json_message(&mut ws).await;
    assert_eq!(
        snapshot.get("type").and_then(Value::as_str),
        Some("snapshot")
    );
    let snapshot_session_id = snapshot
        .pointer("/session/session_id")
        .and_then(Value::as_str)
        .expect("snapshot missing session_id");
    assert_eq!(snapshot_session_id, session.session_id.to_string());
    let snapshot_seq = snapshot
        .get("sequence_id")
        .and_then(Value::as_u64)
        .expect("snapshot missing sequence");

    state
        .auth_session_store
        .update_owned_status(
            session.session_id,
            user_id,
            acpms_server::services::agent_auth::AuthSessionStatus::WaitingUserAction,
            None,
            Some("Waiting for user action".to_string()),
        )
        .await
        .expect("session should update");

    let upsert = recv_json_message(&mut ws).await;
    assert_eq!(upsert.get("type").and_then(Value::as_str), Some("upsert"));
    let upsert_seq = upsert
        .get("sequence_id")
        .and_then(Value::as_u64)
        .expect("upsert missing sequence");
    assert!(
        upsert_seq > snapshot_seq,
        "upsert sequence should advance after update"
    );
    assert_eq!(
        upsert.pointer("/session/status").and_then(Value::as_str),
        Some("waiting_user_action")
    );

    let _ = ws.close(None).await;
    let _ = shutdown_tx.send(());
    let _ = timeout(Duration::from_secs(2), server_task).await;
    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_agent_auth_session_ws_gap_detected_when_since_seq_ahead() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state.clone());

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let session = state
        .auth_session_store
        .create_session(
            user_id,
            "claude-code".to_string(),
            AuthFlowType::LoopbackProxy,
            300,
        )
        .await;

    let (addr, shutdown_tx, server_task) = spawn_router(router).await;
    let ws_url = format!(
        "ws://{}/api/v1/agent/auth/sessions/{}/ws?since_seq=999",
        addr, session.session_id
    );
    let mut ws = connect_ws_with_bearer(&ws_url, &token).await;

    let payload = recv_json_message(&mut ws).await;
    assert_eq!(
        payload.get("type").and_then(Value::as_str),
        Some("gap_detected")
    );
    assert_eq!(
        payload.get("requested_since_seq").and_then(Value::as_u64),
        Some(999)
    );

    let _ = ws.close(None).await;
    let _ = shutdown_tx.send(());
    let _ = timeout(Duration::from_secs(2), server_task).await;
    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_raw_logs_ws_emits_terminal_status_for_inactive_process() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state.clone());

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id =
        create_test_project(&pool, user_id, Some("WS Raw Logs Terminal Project")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("WS Raw Logs Terminal Task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let process_id = Uuid::new_v4();
    let process_created_at = Utc::now() - ChronoDuration::minutes(1);
    let completed_at = process_created_at + ChronoDuration::seconds(30);

    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name, created_at)
        VALUES ($1, $2, NULL, '/tmp/ws-raw', 'ws-raw-branch', $3)
        "#,
    )
    .bind(process_id)
    .bind(attempt_id)
    .bind(process_created_at)
    .execute(&pool)
    .await
    .expect("failed to seed execution process");

    sqlx::query(
        r#"
        INSERT INTO agent_logs (id, attempt_id, log_type, content, created_at)
        VALUES ($1, $2, 'process_stdout', 'process output line', $3)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(attempt_id)
    .bind(process_created_at + ChronoDuration::seconds(5))
    .execute(&pool)
    .await
    .expect("failed to seed process stdout log");

    sqlx::query(
        "UPDATE task_attempts SET status = 'success'::attempt_status, completed_at = $2 WHERE id = $1",
    )
    .bind(attempt_id)
    .bind(completed_at)
    .execute(&pool)
    .await
    .expect("failed to mark attempt as success");

    let (addr, shutdown_tx, server_task) = spawn_router(router).await;
    let ws_url = format!(
        "ws://{}/api/v1/execution-processes/{}/raw-logs/ws",
        addr, process_id
    );
    let mut ws = connect_ws_with_bearer(&ws_url, &token).await;

    let first_payload = recv_json_message(&mut ws).await;
    let second_payload = recv_json_message(&mut ws).await;

    let payloads = [first_payload, second_payload];
    let log_event = payloads
        .iter()
        .find(|payload| {
            payload.get("event").and_then(|event| event.get("type"))
                == Some(&Value::String("Log".to_string()))
        })
        .expect("expected initial log event payload");
    let status_event = payloads
        .iter()
        .find(|payload| {
            payload.get("event").and_then(|event| event.get("type"))
                == Some(&Value::String("Status".to_string()))
        })
        .expect("expected terminal status event payload");

    assert_eq!(
        log_event.pointer("/event/log_type").and_then(Value::as_str),
        Some("process_stdout")
    );
    assert_eq!(
        status_event
            .pointer("/event/status")
            .and_then(Value::as_str),
        Some("success")
    );

    let log_seq = log_event
        .get("sequence_id")
        .and_then(Value::as_u64)
        .expect("missing log sequence");
    let status_seq = status_event
        .get("sequence_id")
        .and_then(Value::as_u64)
        .expect("missing status sequence");
    assert!(
        status_seq > log_seq,
        "terminal status sequence must advance after log sequence"
    );

    let _ = ws.close(None).await;
    let _ = shutdown_tx.send(());
    let _ = timeout(Duration::from_secs(2), server_task).await;
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_approvals_patch_ws_projection_lifecycle_and_reconnect() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state.clone());

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("WS Approvals Patch Project")).await;
    let task_id =
        create_test_task(&pool, project_id, user_id, Some("WS Approvals Patch Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let process_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name)
        VALUES ($1, $2, NULL, '/tmp/ws-approval', 'ws-approval-branch')
        "#,
    )
    .bind(process_id)
    .bind(attempt_id)
    .execute(&pool)
    .await
    .expect("failed to seed execution process");

    let approval_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO tool_approvals (id, attempt_id, execution_process_id, tool_use_id, tool_name, tool_input, status)
        VALUES ($1, $2, $3, $4, $5, $6, 'pending'::approval_status)
        "#,
    )
    .bind(approval_id)
    .bind(attempt_id)
    .bind(process_id)
    .bind(format!("tool-use-{}", approval_id))
    .bind("Bash")
    .bind(serde_json::json!({"command":"echo hello"}))
    .execute(&pool)
    .await
    .expect("failed to seed pending approval");

    let (addr, shutdown_tx, server_task) = spawn_router(router.clone()).await;
    let ws_url = format!(
        "ws://{}/api/v1/approvals/stream/ws?execution_process_id={}&projection=patch",
        addr, process_id
    );
    let mut ws = connect_ws_with_bearer(&ws_url, &token).await;

    let snapshot = recv_json_message(&mut ws).await;
    assert_eq!(
        snapshot.get("type").and_then(Value::as_str),
        Some("snapshot")
    );
    let snapshot_seq = snapshot
        .get("sequence_id")
        .and_then(Value::as_u64)
        .expect("snapshot missing sequence_id");
    let snapshot_status = snapshot
        .pointer(&format!("/data/approvals/{}/status", approval_id))
        .and_then(Value::as_str)
        .expect("snapshot missing approval status");
    assert_eq!(snapshot_status, "pending");

    let respond_path = format!("/api/v1/approvals/{}/respond", approval_id);
    let (respond_status, respond_body) = make_request_with_string_headers(
        &router,
        "POST",
        &respond_path,
        Some(&serde_json::json!({"decision":"approve"}).to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    assert_eq!(
        respond_status,
        StatusCode::OK,
        "approval respond failed: {}",
        respond_body
    );

    let patch = recv_json_message(&mut ws).await;
    assert_eq!(patch.get("type").and_then(Value::as_str), Some("patch"));
    let patch_seq = patch
        .get("sequence_id")
        .and_then(Value::as_u64)
        .expect("patch missing sequence_id");
    assert!(patch_seq > snapshot_seq, "patch sequence must advance");

    let operations = patch
        .get("operations")
        .and_then(Value::as_array)
        .expect("patch missing operations");
    assert!(
        operations.iter().any(|operation| {
            operation.get("op").and_then(Value::as_str) == Some("replace")
                && operation.get("path").and_then(Value::as_str)
                    == Some(&format!("/approvals/{}", approval_id))
                && operation.pointer("/value/status").and_then(Value::as_str) == Some("approved")
        }),
        "expected replace operation with approved status"
    );

    let _ = ws.close(None).await;

    let reconnect_url = format!("{}&since_seq={}", ws_url, patch_seq);
    let mut reconnect_ws = connect_ws_with_bearer(&reconnect_url, &token).await;
    let reconnect_snapshot = recv_json_message(&mut reconnect_ws).await;
    assert_eq!(
        reconnect_snapshot.get("type").and_then(Value::as_str),
        Some("snapshot")
    );
    let reconnect_seq = reconnect_snapshot
        .get("sequence_id")
        .and_then(Value::as_u64)
        .expect("reconnect snapshot missing sequence_id");
    assert!(
        reconnect_seq > patch_seq,
        "reconnect snapshot sequence should continue from persisted cursor"
    );
    let reconnect_status = reconnect_snapshot
        .pointer(&format!("/data/approvals/{}/status", approval_id))
        .and_then(Value::as_str)
        .expect("reconnect snapshot missing approval status");
    assert_eq!(reconnect_status, "approved");

    let _ = reconnect_ws.close(None).await;
    let _ = shutdown_tx.send(());
    let _ = timeout(Duration::from_secs(2), server_task).await;
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_execution_processes_session_ws_reconnect_and_future_cursor_gap() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state.clone());

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("WS Session Stream Project")).await;
    let task_id =
        create_test_task(&pool, project_id, user_id, Some("WS Session Stream Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let now = Utc::now();
    let process_one_id = Uuid::new_v4();
    let process_two_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name, created_at)
        VALUES ($1, $2, NULL, '/tmp/ws-session-1', 'ws-session-1', $3)
        "#,
    )
    .bind(process_one_id)
    .bind(attempt_id)
    .bind(now - ChronoDuration::minutes(2))
    .execute(&pool)
    .await
    .expect("failed to seed first execution process");

    let (addr, shutdown_tx, server_task) = spawn_router(router).await;
    let ws_url = format!(
        "ws://{}/api/v1/execution-processes/stream/session/ws?session_id={}",
        addr, attempt_id
    );

    let mut ws = connect_ws_with_bearer(&ws_url, &token).await;
    let initial_snapshot = recv_json_message(&mut ws).await;
    assert_eq!(
        initial_snapshot
            .pointer("/message/type")
            .and_then(Value::as_str),
        Some("snapshot")
    );
    let first_sequence = initial_snapshot
        .get("sequence_id")
        .and_then(Value::as_u64)
        .expect("initial session snapshot missing sequence_id");
    let initial_processes = initial_snapshot
        .pointer("/message/processes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert_eq!(initial_processes.len(), 1);
    assert_eq!(
        initial_processes[0].get("id").and_then(Value::as_str),
        Some(process_one_id.to_string().as_str())
    );
    let _ = ws.close(None).await;

    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name, created_at)
        VALUES ($1, $2, NULL, '/tmp/ws-session-2', 'ws-session-2', $3)
        "#,
    )
    .bind(process_two_id)
    .bind(attempt_id)
    .bind(now - ChronoDuration::minutes(1))
    .execute(&pool)
    .await
    .expect("failed to seed second execution process");

    let reconnect_url = format!("{}&since_seq={}", ws_url, first_sequence);
    let mut reconnect_ws = connect_ws_with_bearer(&reconnect_url, &token).await;
    let reconnect_snapshot = recv_json_message(&mut reconnect_ws).await;
    assert_eq!(
        reconnect_snapshot
            .pointer("/message/type")
            .and_then(Value::as_str),
        Some("snapshot")
    );
    let reconnect_sequence = reconnect_snapshot
        .get("sequence_id")
        .and_then(Value::as_u64)
        .expect("reconnect session snapshot missing sequence_id");
    assert!(
        reconnect_sequence > first_sequence,
        "reconnect snapshot sequence should continue from persisted cursor"
    );

    let reconnect_processes = reconnect_snapshot
        .pointer("/message/processes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let reconnect_ids: Vec<&str> = reconnect_processes
        .iter()
        .filter_map(|process| process.get("id").and_then(Value::as_str))
        .collect();
    assert_eq!(reconnect_ids.len(), 2);
    assert_eq!(reconnect_ids[0], process_one_id.to_string());
    assert_eq!(reconnect_ids[1], process_two_id.to_string());
    let _ = reconnect_ws.close(None).await;

    let future_cursor = reconnect_sequence + 50;
    let future_cursor_url = format!("{}&since_seq={}", ws_url, future_cursor);
    let mut gap_ws = connect_ws_with_bearer(&future_cursor_url, &token).await;
    let gap_payload = recv_json_message(&mut gap_ws).await;
    assert_eq!(
        gap_payload.get("type").and_then(Value::as_str),
        Some("gap_detected")
    );
    assert_eq!(
        gap_payload
            .get("requested_since_seq")
            .and_then(Value::as_u64),
        Some(future_cursor)
    );
    assert_eq!(
        gap_payload
            .get("max_available_sequence_id")
            .and_then(Value::as_u64),
        Some(reconnect_sequence)
    );
    let _ = gap_ws.close(None).await;

    let _ = shutdown_tx.send(());
    let _ = timeout(Duration::from_secs(2), server_task).await;
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_normalized_logs_ws_future_cursor_returns_gap_detected() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state.clone());

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("WS Normalized Gap Project")).await;
    let task_id =
        create_test_task(&pool, project_id, user_id, Some("WS Normalized Gap Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let process_id = Uuid::new_v4();
    let process_created_at = Utc::now() - ChronoDuration::minutes(1);

    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name, created_at)
        VALUES ($1, $2, NULL, '/tmp/ws-normalized-gap', 'ws-normalized-gap', $3)
        "#,
    )
    .bind(process_id)
    .bind(attempt_id)
    .bind(process_created_at)
    .execute(&pool)
    .await
    .expect("failed to seed execution process");

    sqlx::query(
        r#"
        INSERT INTO agent_logs (id, attempt_id, log_type, content, created_at)
        VALUES ($1, $2, 'normalized', $3, $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(attempt_id)
    .bind(
        serde_json::json!({
            "entry_type": "assistant_message",
            "message": "normalized entry"
        })
        .to_string(),
    )
    .bind(process_created_at + ChronoDuration::seconds(5))
    .execute(&pool)
    .await
    .expect("failed to seed normalized log");

    let (addr, shutdown_tx, server_task) = spawn_router(router).await;
    let requested_since_seq = 99_u64;
    let ws_url = format!(
        "ws://{}/api/v1/execution-processes/{}/normalized-logs/ws?since_seq={}",
        addr, process_id, requested_since_seq
    );
    let mut ws = connect_ws_with_bearer(&ws_url, &token).await;
    let gap_payload = recv_json_message(&mut ws).await;

    assert_eq!(
        gap_payload.get("type").and_then(Value::as_str),
        Some("gap_detected")
    );
    assert_eq!(
        gap_payload
            .get("requested_since_seq")
            .and_then(Value::as_u64),
        Some(requested_since_seq)
    );
    assert_eq!(
        gap_payload
            .get("max_available_sequence_id")
            .and_then(Value::as_u64),
        Some(1)
    );

    let _ = ws.close(None).await;
    let _ = shutdown_tx.send(());
    let _ = timeout(Duration::from_secs(2), server_task).await;
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_raw_logs_ws_future_cursor_returns_gap_detected() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state.clone());

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("WS Raw Gap Project")).await;
    let task_id = create_test_task(&pool, project_id, user_id, Some("WS Raw Gap Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let process_id = Uuid::new_v4();
    let process_created_at = Utc::now() - ChronoDuration::minutes(1);

    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name, created_at)
        VALUES ($1, $2, NULL, '/tmp/ws-raw-gap', 'ws-raw-gap', $3)
        "#,
    )
    .bind(process_id)
    .bind(attempt_id)
    .bind(process_created_at)
    .execute(&pool)
    .await
    .expect("failed to seed execution process");

    sqlx::query(
        r#"
        INSERT INTO agent_logs (id, attempt_id, log_type, content, created_at)
        VALUES ($1, $2, 'process_stdout', 'raw output', $3)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(attempt_id)
    .bind(process_created_at + ChronoDuration::seconds(5))
    .execute(&pool)
    .await
    .expect("failed to seed raw log");

    let (addr, shutdown_tx, server_task) = spawn_router(router).await;
    let requested_since_seq = 99_u64;
    let ws_url = format!(
        "ws://{}/api/v1/execution-processes/{}/raw-logs/ws?since_seq={}",
        addr, process_id, requested_since_seq
    );
    let mut ws = connect_ws_with_bearer(&ws_url, &token).await;
    let gap_payload = recv_json_message(&mut ws).await;

    assert_eq!(
        gap_payload.get("type").and_then(Value::as_str),
        Some("gap_detected")
    );
    assert_eq!(
        gap_payload
            .get("requested_since_seq")
            .and_then(Value::as_u64),
        Some(requested_since_seq)
    );
    assert_eq!(
        gap_payload
            .get("max_available_sequence_id")
            .and_then(Value::as_u64),
        Some(1)
    );

    let _ = ws.close(None).await;
    let _ = shutdown_tx.send(());
    let _ = timeout(Duration::from_secs(2), server_task).await;
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
