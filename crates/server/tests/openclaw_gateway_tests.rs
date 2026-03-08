mod helpers;

use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use axum::http::StatusCode;
use helpers::{create_test_router, make_request_with_string_headers};
use serde_json::Value;
use sqlx::PgPool;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

async fn test_database_ready() -> bool {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5432/acpms_test".to_string()
    });

    tokio::time::timeout(Duration::from_secs(2), PgPool::connect(&database_url))
        .await
        .ok()
        .and_then(Result::ok)
        .is_some()
}

fn configure_openclaw_env() {
    std::env::set_var("OPENCLAW_GATEWAY_ENABLED", "true");
    std::env::set_var("OPENCLAW_API_KEY", "oc_test_phase1_key");
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
    assert!(json["paths"].get("/api/v1/projects").is_none());
}
