mod helpers;

use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use acpms_db::models::{SystemRole, User};
use acpms_executors::{AgentEvent, ApprovalRequestMessage, StatusManager, StatusMessage};
use acpms_server::middleware::authenticate_openclaw_token;
use acpms_services::NewOpenClawGatewayEvent;
use axum::{
    body::{Body, Bytes},
    http::{header, Request, Response, StatusCode},
};
use chrono::Utc;
use futures::StreamExt;
use helpers::{
    cleanup_test_data, create_router, create_test_admin, create_test_app_state,
    create_test_attempt, create_test_project, create_test_router, create_test_task,
    create_test_user, make_request_with_string_headers, setup_test_db,
};
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

async fn test_database_ready() -> bool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/acpms_test".to_string());

    tokio::time::timeout(Duration::from_secs(2), PgPool::connect(&database_url))
        .await
        .ok()
        .and_then(Result::ok)
        .is_some()
}

fn configure_openclaw_env() {
    std::env::set_var("OPENCLAW_GATEWAY_ENABLED", "true");
    std::env::set_var("OPENCLAW_API_KEY", "oc_test_phase1_key");
    std::env::set_var("OPENCLAW_WEBHOOK_SECRET", "wh_sec_test_only");
    std::env::remove_var("OPENCLAW_WEBHOOK_URL");
    std::env::remove_var("OPENCLAW_ACTOR_USER_ID");
}

fn disable_openclaw_env() {
    std::env::set_var("OPENCLAW_GATEWAY_ENABLED", "false");
    std::env::remove_var("OPENCLAW_API_KEY");
    std::env::remove_var("OPENCLAW_WEBHOOK_URL");
    std::env::remove_var("OPENCLAW_WEBHOOK_SECRET");
    std::env::remove_var("OPENCLAW_ACTOR_USER_ID");
}

async fn read_next_sse_chunk(response: Response<Body>) -> String {
    let mut stream = response.into_body().into_data_stream();
    read_next_sse_chunk_from_stream(&mut stream).await
}

async fn read_next_sse_chunk_from_stream<S>(stream: &mut S) -> String
where
    S: futures::Stream<Item = Result<Bytes, axum::Error>> + Unpin,
{
    let chunk = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            match stream.next().await {
                Some(Ok(bytes)) if !bytes.is_empty() => break bytes,
                Some(Ok(_)) => continue,
                Some(Err(error)) => panic!("failed to read SSE chunk: {error}"),
                None => panic!("SSE stream ended before yielding data"),
            }
        }
    })
    .await
    .expect("timed out waiting for SSE chunk");

    String::from_utf8(chunk.to_vec()).expect("SSE chunk should be valid UTF-8")
}

async fn assert_no_sse_chunk_from_stream<S>(stream: &mut S, timeout: Duration)
where
    S: futures::Stream<Item = Result<Bytes, axum::Error>> + Unpin,
{
    let result = tokio::time::timeout(timeout, stream.next()).await;
    match result {
        Err(_) => {}
        Ok(None) => {}
        Ok(Some(Ok(bytes))) if bytes.is_empty() => {}
        Ok(Some(Ok(bytes))) => panic!(
            "expected no SSE chunk, but received: {}",
            String::from_utf8_lossy(&bytes)
        ),
        Ok(Some(Err(error))) => panic!("unexpected SSE stream error: {error}"),
    }
}

fn parse_sse_event_id(chunk: &str) -> i64 {
    chunk
        .lines()
        .find_map(|line| line.strip_prefix("id: "))
        .unwrap_or_else(|| panic!("missing SSE id in chunk: {chunk}"))
        .trim()
        .parse::<i64>()
        .unwrap_or_else(|error| panic!("invalid SSE id in chunk: {error}; chunk: {chunk}"))
}

#[tokio::test]
async fn openclaw_guide_requires_valid_api_key() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let router = create_test_router().await;
    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/openclaw/guide-for-openclaw",
        Some("{}"),
        vec![("content-type", "application/json".to_string())],
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
}

#[tokio::test]
async fn openclaw_guide_returns_bootstrap_payload() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    std::env::set_var("ACPMS_PUBLIC_URL", "https://acpms.example.com");
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let router = create_test_router().await;
    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/openclaw/guide-for-openclaw",
        Some(
            r#"{
              "reporting": {
                "primary_user": {
                  "display_name": "Alice",
                  "timezone": "Asia/Ho_Chi_Minh",
                  "preferred_language": "vi"
                },
                "channels": [
                  { "type": "telegram", "target": "@alice_ops" }
                ]
              }
            }"#,
        ),
        vec![
            ("content-type", "application/json".to_string()),
            ("authorization", "Bearer oc_test_phase1_key".to_string()),
        ],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    let json: Value = serde_json::from_str(&body).expect("valid json");
    assert_eq!(json["success"], true, "{body}");
    assert_eq!(
        json["data"]["acpms_profile"]["base_endpoint_url"],
        "https://acpms.example.com/api/openclaw/v1"
    );
    assert_eq!(
        json["data"]["auth_rules"]["rest_auth_header"],
        "Authorization: Bearer <OPENCLAW_API_KEY>"
    );
    assert_eq!(json["data"]["handoff_contract"]["contract_version"], "v1");
    assert_eq!(
        json["data"]["handoff_contract"]["connection_bundle_fields"][0],
        "Base Endpoint URL"
    );
    assert_eq!(
        json["data"]["handoff_contract"]["required_route_prefixes"][0],
        "/api/openclaw/v1/*"
    );
    assert_eq!(
        json["data"]["handoff_contract"]["required_route_prefixes"][1],
        "/api/openclaw/ws/*"
    );
    assert!(json["data"]["instruction_prompt"]
        .as_str()
        .expect("instruction prompt")
        .contains("Alice"));
}

#[tokio::test]
async fn openclaw_guide_accepts_get_method() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    std::env::set_var("ACPMS_PUBLIC_URL", "https://acpms.example.com");
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let router = create_test_router().await;
    let (status, body) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/openclaw/guide-for-openclaw",
        None,
        vec![("authorization", "Bearer oc_test_phase1_key".to_string())],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    let json: Value = serde_json::from_str(&body).expect("valid json");
    assert_eq!(json["success"], true, "{body}");
    assert_eq!(
        json["data"]["acpms_profile"]["guide_url"],
        "https://acpms.example.com/api/openclaw/guide-for-openclaw"
    );
}

#[tokio::test]
async fn openclaw_can_access_mirrored_projects_and_openapi() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    std::env::set_var("ACPMS_PUBLIC_URL", "https://acpms.example.com");
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let router = create_test_router().await;
    let auth_header = ("authorization", "Bearer oc_test_phase1_key".to_string());

    let (projects_status, projects_body) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/openclaw/v1/projects",
        None,
        vec![auth_header.clone()],
    )
    .await;
    assert_eq!(projects_status, StatusCode::OK, "{projects_body}");

    let (openapi_status, openapi_body) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/openclaw/openapi.json",
        None,
        vec![auth_header],
    )
    .await;
    assert_eq!(openapi_status, StatusCode::OK, "{openapi_body}");
    let json: Value = serde_json::from_str(&openapi_body).expect("valid json");
    assert!(json["paths"].get("/api/openclaw/v1/projects").is_some());
    assert!(json["paths"]
        .get("/api/openclaw/guide-for-openclaw")
        .is_some());
    assert!(json["paths"]
        .get("/api/openclaw/v1/events/stream")
        .is_some());
    assert!(json["paths"]
        .get("/api/openclaw/ws/attempts/{id}/stream")
        .is_some());
    assert!(json["paths"]
        .get("/api/openclaw/ws/execution-processes/stream/attempt")
        .is_some());
    assert!(json["paths"]
        .get("/api/openclaw/ws/approvals/stream")
        .is_some());
    assert!(json["paths"]
        .get("/api/openclaw/ws/agent/auth/sessions/{id}")
        .is_some());
    assert!(json["paths"].get("/api/v1/projects").is_none());
}

#[tokio::test]
async fn openclaw_can_list_tasks_without_project_id_as_system_admin() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let project_id = create_test_project(&pool, user_id, Some("OpenClaw Task Visibility")).await;
    let task_id = create_test_task(&pool, project_id, user_id, Some("OpenClaw Visible Task")).await;

    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (status, body) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/openclaw/v1/tasks",
        None,
        vec![("authorization", "Bearer oc_test_phase1_key".to_string())],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    let json: Value = serde_json::from_str(&body).expect("valid json");
    let tasks = json["data"].as_array().expect("tasks array");
    assert!(
        tasks
            .iter()
            .any(|task| task["id"].as_str() == Some(&task_id.to_string())),
        "expected task {task_id} to be visible in gateway response: {body}"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
async fn openclaw_event_stream_requires_valid_api_key() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let router = create_test_router().await;
    let (status, body) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/openclaw/v1/events/stream",
        None,
        vec![],
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
}

#[tokio::test]
async fn openclaw_gateway_returns_forbidden_when_disabled() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    disable_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let router = create_test_router().await;
    let auth_header = ("authorization", "Bearer oc_test_phase1_key".to_string());

    let (guide_status, guide_body) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/openclaw/guide-for-openclaw",
        Some("{}"),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header.clone(),
        ],
    )
    .await;
    assert_eq!(guide_status, StatusCode::FORBIDDEN, "{guide_body}");

    let (stream_status, stream_body) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/openclaw/v1/events/stream",
        None,
        vec![auth_header],
    )
    .await;
    assert_eq!(stream_status, StatusCode::FORBIDDEN, "{stream_body}");
}

#[tokio::test]
async fn openclaw_auth_uses_dedicated_service_principal() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    sqlx::query("DELETE FROM users WHERE email = 'openclaw-gateway@acpms.local'")
        .execute(&pool)
        .await
        .expect("clear openclaw service user");
    let (admin_user_id, _) = create_test_admin(&pool).await;

    let state = create_test_app_state(pool.clone()).await;
    let auth_user = authenticate_openclaw_token(&state, "oc_test_phase1_key")
        .await
        .expect("authenticate openclaw token");

    let service_user = sqlx::query_as::<_, User>(
        r#"
        SELECT
            id,
            email,
            name,
            avatar_url,
            gitlab_id,
            gitlab_username,
            password_hash,
            global_roles,
            created_at,
            updated_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(auth_user.id)
    .fetch_one(&pool)
    .await
    .expect("load OpenClaw service principal");

    assert_eq!(service_user.email, "openclaw-gateway@acpms.local");
    assert_eq!(service_user.name, "OpenClaw Gateway");
    assert_eq!(service_user.password_hash, None);
    assert!(service_user.global_roles.contains(&SystemRole::Admin));
    assert_ne!(service_user.id, admin_user_id);
}

#[tokio::test]
async fn openclaw_event_stream_returns_machine_readable_cursor_expired() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    sqlx::query("DELETE FROM openclaw_gateway_events")
        .execute(&pool)
        .await
        .expect("clear openclaw events");

    let state = create_test_app_state(pool.clone()).await;
    let first_event = state
        .openclaw_event_service
        .record_event(NewOpenClawGatewayEvent {
            event_type: "attempt.completed".to_string(),
            project_id: None,
            task_id: None,
            attempt_id: None,
            source: "test".to_string(),
            payload: serde_json::json!({ "status": "success" }),
        })
        .await
        .expect("seed openclaw event");
    let router = create_router(state);

    let (status, body) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/openclaw/v1/events/stream?after=-1",
        None,
        vec![("authorization", "Bearer oc_test_phase1_key".to_string())],
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let json: Value = serde_json::from_str(&body).expect("valid json");
    assert_eq!(json["success"], false, "{body}");
    assert_eq!(json["code"], "4092", "{body}");
    assert_eq!(json["data"]["error_type"], "EventCursorExpired", "{body}");
    assert_eq!(
        json["data"]["oldest_available_event_id"], first_event.sequence_id,
        "{body}"
    );
}

#[tokio::test]
async fn openclaw_event_stream_returns_sse_content_type() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    let state = create_test_app_state(pool).await;
    let router = create_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/openclaw/v1/events/stream")
                .header(header::AUTHORIZATION, "Bearer oc_test_phase1_key")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("execute request");

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        content_type.starts_with("text/event-stream"),
        "unexpected content-type: {content_type}"
    );
}

#[tokio::test]
async fn openclaw_event_service_publishes_live_events_to_subscribers() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    sqlx::query("DELETE FROM openclaw_gateway_events")
        .execute(&pool)
        .await
        .expect("clear openclaw events");

    let state = create_test_app_state(pool).await;
    let mut live_rx = state.openclaw_event_service.subscribe_live();

    let event = state
        .openclaw_event_service
        .record_event(NewOpenClawGatewayEvent {
            event_type: "attempt.started".to_string(),
            project_id: None,
            task_id: None,
            attempt_id: None,
            source: "test.live".to_string(),
            payload: serde_json::json!({ "status": "running" }),
        })
        .await
        .expect("record live event");

    let received = tokio::time::timeout(Duration::from_secs(1), live_rx.recv())
        .await
        .expect("timed out waiting for live event")
        .expect("live event should be delivered");

    assert_eq!(received.sequence_id, event.sequence_id);
    assert_eq!(received.event_type, "attempt.started");
}

#[tokio::test]
async fn openclaw_event_stream_delivers_live_event_without_cursor() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    sqlx::query("DELETE FROM openclaw_gateway_events")
        .execute(&pool)
        .await
        .expect("clear openclaw events");

    let state = create_test_app_state(pool).await;
    let openclaw_event_service = state.openclaw_event_service.clone();
    let router = create_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/openclaw/v1/events/stream")
                .header(header::AUTHORIZATION, "Bearer oc_test_phase1_key")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("execute request");
    assert_eq!(response.status(), StatusCode::OK);

    let expected_event = openclaw_event_service
        .record_event(NewOpenClawGatewayEvent {
            event_type: "attempt.completed".to_string(),
            project_id: None,
            task_id: None,
            attempt_id: None,
            source: "test.sse.live".to_string(),
            payload: serde_json::json!({ "status": "success" }),
        })
        .await
        .expect("record live stream event");
    let chunk = read_next_sse_chunk(response).await;

    assert!(chunk.contains("event: attempt.completed"), "{chunk}");
    assert!(
        chunk.contains(&format!("id: {}", expected_event.sequence_id)),
        "{chunk}"
    );
    assert!(chunk.contains("\"status\":\"success\""), "{chunk}");
}

#[tokio::test]
async fn openclaw_event_service_lists_replay_events_in_sequence_order() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    sqlx::query("DELETE FROM openclaw_gateway_events")
        .execute(&pool)
        .await
        .expect("clear openclaw events");

    let state = create_test_app_state(pool).await;
    let first_event = state
        .openclaw_event_service
        .record_event(NewOpenClawGatewayEvent {
            event_type: "attempt.started".to_string(),
            project_id: None,
            task_id: None,
            attempt_id: None,
            source: "test.replay".to_string(),
            payload: serde_json::json!({ "step": 1 }),
        })
        .await
        .expect("record first event");
    let second_event = state
        .openclaw_event_service
        .record_event(NewOpenClawGatewayEvent {
            event_type: "attempt.needs_input".to_string(),
            project_id: None,
            task_id: None,
            attempt_id: None,
            source: "test.replay".to_string(),
            payload: serde_json::json!({ "step": 2 }),
        })
        .await
        .expect("record second event");
    let third_event = state
        .openclaw_event_service
        .record_event(NewOpenClawGatewayEvent {
            event_type: "attempt.completed".to_string(),
            project_id: None,
            task_id: None,
            attempt_id: None,
            source: "test.replay".to_string(),
            payload: serde_json::json!({ "step": 3 }),
        })
        .await
        .expect("record third event");

    let replay_events = state
        .openclaw_event_service
        .list_events_after(first_event.sequence_id, 10)
        .await
        .expect("load replay events");

    let replay_ids = replay_events
        .iter()
        .map(|event| event.sequence_id)
        .collect::<Vec<_>>();
    assert_eq!(
        replay_ids,
        vec![second_event.sequence_id, third_event.sequence_id]
    );
    assert_eq!(replay_events[0].event_type, "attempt.needs_input");
    assert_eq!(replay_events[1].event_type, "attempt.completed");
}

#[tokio::test]
async fn openclaw_event_stream_replays_events_after_last_event_id() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    sqlx::query("DELETE FROM openclaw_gateway_events")
        .execute(&pool)
        .await
        .expect("clear openclaw events");

    let state = create_test_app_state(pool).await;
    let first_event = state
        .openclaw_event_service
        .record_event(NewOpenClawGatewayEvent {
            event_type: "attempt.started".to_string(),
            project_id: None,
            task_id: None,
            attempt_id: None,
            source: "test.sse.replay".to_string(),
            payload: serde_json::json!({ "status": "running" }),
        })
        .await
        .expect("record first replay event");
    let second_event = state
        .openclaw_event_service
        .record_event(NewOpenClawGatewayEvent {
            event_type: "attempt.completed".to_string(),
            project_id: None,
            task_id: None,
            attempt_id: None,
            source: "test.sse.replay".to_string(),
            payload: serde_json::json!({ "status": "success" }),
        })
        .await
        .expect("record second replay event");
    let router = create_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/openclaw/v1/events/stream")
                .header(header::AUTHORIZATION, "Bearer oc_test_phase1_key")
                .header("Last-Event-ID", first_event.sequence_id.to_string())
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("execute request");
    assert_eq!(response.status(), StatusCode::OK);

    let chunk = read_next_sse_chunk(response).await;

    assert!(chunk.contains("event: attempt.completed"), "{chunk}");
    assert!(
        chunk.contains(&format!("id: {}", second_event.sequence_id)),
        "{chunk}"
    );
    assert!(
        !chunk.contains(&format!("id: {}", first_event.sequence_id)),
        "{chunk}"
    );
}

#[tokio::test]
async fn openclaw_event_stream_replays_backlogs_larger_than_one_page() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    sqlx::query("DELETE FROM openclaw_gateway_events")
        .execute(&pool)
        .await
        .expect("clear openclaw events");

    let state = create_test_app_state(pool).await;
    let first_event = state
        .openclaw_event_service
        .record_event(NewOpenClawGatewayEvent {
            event_type: "attempt.started".to_string(),
            project_id: None,
            task_id: None,
            attempt_id: None,
            source: "test.sse.replay.large".to_string(),
            payload: serde_json::json!({ "index": 0 }),
        })
        .await
        .expect("record first replay event");

    let mut final_event_id = first_event.sequence_id;
    for index in 1..=1005 {
        final_event_id = state
            .openclaw_event_service
            .record_event(NewOpenClawGatewayEvent {
                event_type: "attempt.progress".to_string(),
                project_id: None,
                task_id: None,
                attempt_id: None,
                source: "test.sse.replay.large".to_string(),
                payload: serde_json::json!({ "index": index }),
            })
            .await
            .expect("record replay backlog event")
            .sequence_id;
    }
    let router = create_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/openclaw/v1/events/stream")
                .header(header::AUTHORIZATION, "Bearer oc_test_phase1_key")
                .header("Last-Event-ID", first_event.sequence_id.to_string())
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("execute request");
    assert_eq!(response.status(), StatusCode::OK);

    let mut stream = response.into_body().into_data_stream();
    let mut replayed_count = 0usize;
    let mut last_seen_id = first_event.sequence_id;
    while replayed_count < 1005 {
        let chunk = read_next_sse_chunk_from_stream(&mut stream).await;
        last_seen_id = parse_sse_event_id(&chunk);
        replayed_count += 1;
    }

    assert_eq!(replayed_count, 1005);
    assert_eq!(last_seen_id, final_event_id);
}

#[tokio::test]
async fn openclaw_event_stream_emits_attempt_start_then_completion_in_order() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    sqlx::query("DELETE FROM openclaw_gateway_events")
        .execute(&pool)
        .await
        .expect("clear openclaw events");

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let project_id = create_test_project(&pool, user_id, Some("OpenClaw Stream Project")).await;
    let task_id = create_test_task(&pool, project_id, user_id, Some("OpenClaw Stream Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("queued")).await;

    let state = create_test_app_state(pool.clone()).await;
    let broadcast_tx = state.broadcast_tx.clone();
    let router = create_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/openclaw/v1/events/stream")
                .header(header::AUTHORIZATION, "Bearer oc_test_phase1_key")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("execute request");
    assert_eq!(response.status(), StatusCode::OK);

    broadcast_tx
        .send(AgentEvent::Status(StatusMessage {
            attempt_id,
            status: acpms_db::models::AttemptStatus::Running,
            timestamp: Utc::now(),
        }))
        .expect("broadcast running status");
    broadcast_tx
        .send(AgentEvent::Status(StatusMessage {
            attempt_id,
            status: acpms_db::models::AttemptStatus::Success,
            timestamp: Utc::now(),
        }))
        .expect("broadcast success status");

    let mut stream = response.into_body().into_data_stream();
    let first_chunk = read_next_sse_chunk_from_stream(&mut stream).await;
    let second_chunk = read_next_sse_chunk_from_stream(&mut stream).await;

    assert!(
        first_chunk.contains("event: attempt.started"),
        "{first_chunk}"
    );
    assert!(
        second_chunk.contains("event: attempt.completed"),
        "{second_chunk}"
    );

    let persisted_events = sqlx::query_scalar::<_, String>(
        r#"
        SELECT event_type
        FROM openclaw_gateway_events
        WHERE attempt_id = $1
        ORDER BY sequence_id ASC
        "#,
    )
    .bind(attempt_id)
    .fetch_all(&pool)
    .await
    .expect("load persisted openclaw events");

    assert_eq!(
        persisted_events,
        vec![
            "attempt.started".to_string(),
            "attempt.completed".to_string()
        ]
    );
}

#[tokio::test]
async fn openclaw_needs_input_event_can_be_resolved_via_attempt_input_api() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    sqlx::query("DELETE FROM openclaw_gateway_events")
        .execute(&pool)
        .await
        .expect("clear openclaw events");

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let project_id = create_test_project(&pool, user_id, Some("OpenClaw HITL Project")).await;
    let task_id = create_test_task(&pool, project_id, user_id, Some("OpenClaw HITL Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let state = create_test_app_state(pool.clone()).await;
    let broadcast_tx = state.broadcast_tx.clone();
    let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    state
        .orchestrator
        .attach_input_sender_for_attempt(attempt_id, input_tx)
        .await;
    let router = create_router(state);

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/openclaw/v1/events/stream")
                .header(header::AUTHORIZATION, "Bearer oc_test_phase1_key")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("execute request");
    assert_eq!(response.status(), StatusCode::OK);

    broadcast_tx
        .send(AgentEvent::ApprovalRequest(ApprovalRequestMessage {
            attempt_id,
            tool_use_id: "toolu_123".to_string(),
            tool_name: "ask_user".to_string(),
            tool_input: serde_json::json!({ "question": "Need approval" }),
            timestamp: Utc::now(),
        }))
        .expect("broadcast approval request");

    let chunk = read_next_sse_chunk(response).await;
    assert!(chunk.contains("event: attempt.needs_input"), "{chunk}");
    assert!(chunk.contains("\"tool_name\":\"ask_user\""), "{chunk}");

    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/openclaw/v1/attempts/{attempt_id}/input"),
        Some(r#"{ "input": "Approved. Continue." }"#),
        vec![
            ("content-type", "application/json".to_string()),
            ("authorization", "Bearer oc_test_phase1_key".to_string()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let forwarded_input = tokio::time::timeout(Duration::from_secs(1), input_rx.recv())
        .await
        .expect("timed out waiting for forwarded attempt input")
        .expect("input channel should receive forwarded message");
    assert_eq!(forwarded_input, "Approved. Continue.");
}

#[tokio::test]
async fn openclaw_attempt_log_stream_remains_independent_from_global_event_stream() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    sqlx::query("DELETE FROM openclaw_gateway_events")
        .execute(&pool)
        .await
        .expect("clear openclaw events");

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let project_id =
        create_test_project(&pool, user_id, Some("OpenClaw Independence Project")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("OpenClaw Independence Task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let state = create_test_app_state(pool.clone()).await;
    let openclaw_event_service = state.openclaw_event_service.clone();
    let broadcast_tx = state.broadcast_tx.clone();
    let router = create_router(state);

    let global_response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/openclaw/v1/events/stream")
                .header(header::AUTHORIZATION, "Bearer oc_test_phase1_key")
                .body(Body::empty())
                .expect("build global request"),
        )
        .await
        .expect("execute global request");
    assert_eq!(global_response.status(), StatusCode::OK);

    let attempt_response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/openclaw/v1/attempts/{attempt_id}/stream?since=1"
                ))
                .header(header::AUTHORIZATION, "Bearer oc_test_phase1_key")
                .body(Body::empty())
                .expect("build attempt request"),
        )
        .await
        .expect("execute attempt request");
    assert_eq!(attempt_response.status(), StatusCode::OK);

    let mut global_stream = global_response.into_body().into_data_stream();
    let mut attempt_stream = attempt_response.into_body().into_data_stream();

    let global_event = openclaw_event_service
        .record_event(NewOpenClawGatewayEvent {
            event_type: "attempt.completed".to_string(),
            project_id: Some(project_id),
            task_id: Some(task_id),
            attempt_id: Some(attempt_id),
            source: "test.independence.global".to_string(),
            payload: serde_json::json!({ "status": "success" }),
        })
        .await
        .expect("record global event");

    let global_chunk = read_next_sse_chunk_from_stream(&mut global_stream).await;
    assert!(
        global_chunk.contains("event: attempt.completed"),
        "{global_chunk}"
    );
    assert!(
        global_chunk.contains(&format!("id: {}", global_event.sequence_id)),
        "{global_chunk}"
    );
    assert_no_sse_chunk_from_stream(&mut attempt_stream, Duration::from_millis(200)).await;

    StatusManager::log(
        &pool,
        &broadcast_tx,
        attempt_id,
        "system",
        "attempt log line from test",
    )
    .await
    .expect("write attempt log");

    let attempt_chunk = read_next_sse_chunk_from_stream(&mut attempt_stream).await;
    assert!(
        attempt_chunk.contains("\"path\":\"/attempts/"),
        "{attempt_chunk}"
    );
    assert!(
        attempt_chunk.contains("attempt log line from test"),
        "{attempt_chunk}"
    );
    assert_no_sse_chunk_from_stream(&mut global_stream, Duration::from_millis(200)).await;
}

#[tokio::test]
async fn openclaw_event_cleanup_removes_expired_rows_and_updates_retained_metric() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    sqlx::query("DELETE FROM openclaw_gateway_events")
        .execute(&pool)
        .await
        .expect("clear openclaw events");

    let state = create_test_app_state(pool.clone()).await;
    sqlx::query(
        r#"
        INSERT INTO openclaw_gateway_events (
            event_type,
            occurred_at,
            project_id,
            task_id,
            attempt_id,
            source,
            payload
        )
        VALUES ($1, NOW() - INTERVAL '10 days', NULL, NULL, NULL, $2, $3)
        "#,
    )
    .bind("attempt.failed")
    .bind("test.cleanup.expired")
    .bind(serde_json::json!({ "status": "failed" }))
    .execute(&pool)
    .await
    .expect("insert expired event");
    state
        .openclaw_event_service
        .record_event(NewOpenClawGatewayEvent {
            event_type: "attempt.completed".to_string(),
            project_id: None,
            task_id: None,
            attempt_id: None,
            source: "test.cleanup.live".to_string(),
            payload: serde_json::json!({ "status": "success" }),
        })
        .await
        .expect("record retained event");
    state
        .openclaw_event_service
        .sync_retained_event_row_count_metric()
        .await
        .expect("sync retained row metric");

    let deleted = state
        .openclaw_event_service
        .cleanup_expired_events()
        .await
        .expect("cleanup expired events");
    let retained_rows = state
        .openclaw_event_service
        .retained_event_row_count()
        .await
        .expect("count retained rows");
    let encoded_metrics = state.metrics.encode().expect("encode metrics");

    assert_eq!(deleted, 1);
    assert_eq!(retained_rows, 1);
    assert!(encoded_metrics.contains("acpms_openclaw_retained_event_rows 1"));
}

#[tokio::test]
async fn openclaw_webhook_deliveries_are_persisted_and_retryable() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    std::env::set_var("OPENCLAW_WEBHOOK_URL", "http://127.0.0.1:9/openclaw");
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    sqlx::query("DELETE FROM openclaw_webhook_deliveries")
        .execute(&pool)
        .await
        .expect("clear openclaw webhook deliveries");
    sqlx::query("DELETE FROM openclaw_gateway_events")
        .execute(&pool)
        .await
        .expect("clear openclaw events");

    let state = create_test_app_state(pool.clone()).await;
    let event = state
        .openclaw_event_service
        .record_event(NewOpenClawGatewayEvent {
            event_type: "attempt.completed".to_string(),
            project_id: None,
            task_id: None,
            attempt_id: None,
            source: "test.webhook.queue".to_string(),
            payload: serde_json::json!({ "status": "success" }),
        })
        .await
        .expect("record event with webhook delivery");

    let delivery_id = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let maybe_id = sqlx::query_scalar::<_, uuid::Uuid>(
                "SELECT id FROM openclaw_webhook_deliveries WHERE event_sequence_id = $1",
            )
            .bind(event.sequence_id)
            .fetch_optional(&pool)
            .await
            .expect("load queued webhook delivery");

            if let Some(delivery_id) = maybe_id {
                break delivery_id;
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("timed out waiting for queued webhook delivery");

    sqlx::query(
        r#"
        UPDATE openclaw_webhook_deliveries
        SET
            status = 'failed',
            attempt_count = max_attempts,
            last_error = 'simulated failure'
        WHERE id = $1
        "#,
    )
    .bind(delivery_id)
    .execute(&pool)
    .await
    .expect("mark queued webhook delivery as failed");

    let failed_deliveries = state
        .openclaw_event_service
        .get_failed_webhook_deliveries(10)
        .await
        .expect("load failed openclaw webhook deliveries");
    assert_eq!(failed_deliveries.len(), 1);
    assert_eq!(failed_deliveries[0].id, delivery_id);
    assert_eq!(failed_deliveries[0].event_sequence_id, event.sequence_id);

    state
        .openclaw_event_service
        .retry_failed_webhook_delivery(delivery_id)
        .await
        .expect("retry failed openclaw webhook delivery");

    let stats = state
        .openclaw_event_service
        .webhook_delivery_stats()
        .await
        .expect("load openclaw webhook delivery stats");
    assert_eq!(stats.failed, 0);
    assert_eq!(stats.pending, 1);
}

#[tokio::test]
async fn openclaw_ws_routes_require_openclaw_auth() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    configure_openclaw_env();
    if !test_database_ready().await {
        eprintln!("skipping openclaw gateway test because DATABASE_URL is not reachable");
        return;
    }

    let router = create_test_router().await;
    let (status, body) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/openclaw/ws/agent-activity/status",
        None,
        vec![],
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");

    let (approvals_status, approvals_body) = make_request_with_string_headers(
        &router,
        "GET",
        &format!(
            "/api/openclaw/ws/approvals/stream?attempt_id={}",
            uuid::Uuid::new_v4()
        ),
        None,
        vec![],
    )
    .await;

    assert_eq!(
        approvals_status,
        StatusCode::UNAUTHORIZED,
        "{approvals_body}"
    );
}
