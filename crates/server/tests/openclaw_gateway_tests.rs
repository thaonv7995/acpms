mod helpers;

use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use acpms_services::NewOpenClawGatewayEvent;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use helpers::{
    create_router, create_test_app_state, create_test_router, make_request_with_string_headers,
    setup_test_db,
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
    assert!(json["data"]["instruction_prompt"]
        .as_str()
        .expect("instruction prompt")
        .contains("Alice"));
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
    assert!(json["paths"].get("/api/v1/projects").is_none());
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
        json["data"]["oldest_available_event_id"],
        first_event.sequence_id,
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
}
