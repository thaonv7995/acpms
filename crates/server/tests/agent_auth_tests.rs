//! Agent Auth API Tests
#[path = "helpers.rs"]
mod helpers;
use acpms_server::services::agent_auth::AuthFlowType;
use helpers::*;
use serde_json::Value;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Stdio, time::Duration};
use tokio::process::Command;
use uuid::Uuid;

struct EnvVarGuard {
    key: &'static str,
    original: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: String) -> Self {
        let original = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(original) = &self.original {
            std::env::set_var(self.key, original);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn create_fake_codex_auth_cli() -> (PathBuf, PathBuf) {
    create_fake_auth_cli(
        "codex",
        "acpms-fake-codex-auth",
        r#"#!/bin/sh
set -eu

if [ "$#" -ge 2 ] && [ "$1" = "login" ] && [ "$2" = "--device-auth" ]; then
  printf '%s\n' "Open this URL to continue device auth..."
  exit 0
fi

if [ "$#" -ge 2 ] && [ "$1" = "login" ] && [ "$2" = "status" ]; then
  printf '%s\n' "Not logged in. Please login first."
  exit 1
fi

printf '%s\n' "unexpected args: $*" >&2
exit 2
"#,
    )
}

fn create_fake_claude_auth_cli() -> (PathBuf, PathBuf) {
    create_fake_auth_cli(
        "claude",
        "acpms-fake-claude-auth",
        r#"#!/bin/sh
set -eu

if [ "$#" -ge 1 ] && [ "$1" = "--version" ]; then
  printf '%s\n' "2.1.0"
  exit 0
fi

if [ "$#" -ge 1 ] && [ "$1" = "setup-token" ]; then
  printf '%s\n' "Setup token flow started"
  exit 0
fi

if [ "$#" -ge 1 ] && [ "$1" = "auth" ] && [ "${2:-}" = "status" ]; then
  printf '%s\n' "Not authenticated"
  exit 1
fi

if [ "$#" -ge 1 ] && [ "$1" = "auth" ]; then
  printf '%s\n' "Open browser for Claude login"
  exit 0
fi

printf '%s\n' "unexpected args: $*" >&2
exit 2
"#,
    )
}

fn create_fake_cursor_auth_cli() -> (PathBuf, PathBuf) {
    create_fake_auth_cli(
        "agent",
        "acpms-fake-cursor-auth",
        r#"#!/bin/sh
set -eu

if [ "$#" -ge 1 ] && [ "$1" = "login" ]; then
  printf '%s\n' "Open browser for Cursor login"
  exit 0
fi

if [ "$#" -ge 1 ] && [ "$1" = "status" ]; then
  printf '%s\n' "not authenticated"
  exit 1
fi

printf '%s\n' "unexpected args: $*" >&2
exit 2
"#,
    )
}

fn create_fake_gemini_auth_cli() -> (PathBuf, PathBuf) {
    create_fake_auth_cli(
        "gemini",
        "acpms-fake-gemini-auth",
        r#"#!/bin/sh
set -eu

if [ "$#" -ge 2 ] && [ "$1" = "-p" ] && [ "$2" = "ping" ]; then
  printf '%s\n' "authentication required"
  exit 1
fi

printf '%s\n' "Gemini sign in started"
exit 0
"#,
    )
}

fn create_fake_auth_cli(bin_name: &str, dir_prefix: &str, script: &str) -> (PathBuf, PathBuf) {
    let base_dir = std::env::temp_dir().join(format!("{}-{}", dir_prefix, Uuid::new_v4()));
    fs::create_dir_all(&base_dir).expect("failed to create fake codex auth directory");

    let script_path = base_dir.join(bin_name);
    fs::write(&script_path, script).expect("failed to write fake codex auth script");
    let mut permissions = fs::metadata(&script_path)
        .expect("failed to stat fake codex auth script")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).expect("failed to chmod fake codex auth script");

    (base_dir, script_path)
}

async fn initiate_auth_session(
    router: &axum::Router,
    token: &str,
    provider: &str,
) -> (Uuid, String) {
    let request = format!(r#"{{"provider":"{}"}}"#, provider);
    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        router,
        "POST",
        "/api/v1/agent/auth/initiate",
        Some(&request),
        vec![
            auth_header_bearer(token),
            ("content-type", "application/json".to_string()),
        ],
    )
    .await;
    assert_eq!(status, 200, "Expected 200, got {}: {}", status, body);

    let response: Value = serde_json::from_str(&body).expect("Failed to parse response");
    let session_id = response["data"]["session_id"]
        .as_str()
        .and_then(|value| Uuid::parse_str(value).ok())
        .expect("missing session id in initiate response");
    (session_id, body)
}

async fn wait_for_failed_session(router: &axum::Router, token: &str, session_id: Uuid) -> Value {
    let mut failed_payload: Option<Value> = None;
    for _ in 0..25 {
        let (session_status, session_body): (axum::http::StatusCode, String) =
            make_request_with_string_headers(
                router,
                "GET",
                &format!("/api/v1/agent/auth/sessions/{}", session_id),
                None,
                vec![auth_header_bearer(token)],
            )
            .await;
        assert_eq!(
            session_status, 200,
            "Expected session fetch 200, got {}: {}",
            session_status, session_body
        );

        let session_response: Value =
            serde_json::from_str(&session_body).expect("Failed to parse session response");
        if session_response["data"]["status"].as_str() == Some("failed") {
            failed_payload = Some(session_response);
            break;
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    failed_payload
        .unwrap_or_else(|| panic!("session did not reach failed state in expected timeframe"))
}

fn assert_provider_not_authenticated_failure(payload: &Value) {
    assert_eq!(
        payload["data"]["status"].as_str(),
        Some("failed"),
        "unexpected payload: {}",
        payload
    );
    let last_error = payload["data"]["last_error"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    assert!(
        last_error.contains("provider is not authenticated yet"),
        "expected provider-auth failure message, got: {}",
        payload
    );
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_provider_statuses_unauthorized() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool).await;
    let router = create_router(state);

    let (status, _body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/agent/providers/status",
        None,
        vec![],
    )
    .await;

    assert_eq!(status, 401);
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_initiate_agent_auth_invalid_provider() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/agent/auth/initiate",
        Some(r#"{"provider":"unknown-provider"}"#),
        vec![
            auth_header_bearer(&token),
            ("content-type", "application/json".to_string()),
        ],
    )
    .await;

    assert_eq!(status, 400, "Expected 400, got {}: {}", status, body);
    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    assert_eq!(response["success"].as_bool(), Some(false));

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_initiate_auth_marks_failed_when_process_exits_success_but_provider_unauthed() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let (fake_dir, fake_codex_path) = create_fake_codex_auth_cli();
    let _codex_bin_guard = EnvVarGuard::set(
        "ACPMS_AGENT_CODEX_BIN",
        fake_codex_path.display().to_string(),
    );

    let (session_id, _body) = initiate_auth_session(&router, &token, "openai-codex").await;
    let failed_payload = wait_for_failed_session(&router, &token, session_id).await;
    assert_provider_not_authenticated_failure(&failed_payload);

    cleanup_test_data(&pool, user_id, None).await;
    let _ = fs::remove_dir_all(fake_dir);
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_initiate_claude_auth_marks_failed_when_process_exits_success_but_provider_unauthed() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let (fake_dir, fake_claude_path) = create_fake_claude_auth_cli();
    let _claude_bin_guard = EnvVarGuard::set(
        "ACPMS_AGENT_CLAUDE_BIN",
        fake_claude_path.display().to_string(),
    );

    let (session_id, _body) = initiate_auth_session(&router, &token, "claude-code").await;
    let failed_payload = wait_for_failed_session(&router, &token, session_id).await;
    assert_provider_not_authenticated_failure(&failed_payload);

    cleanup_test_data(&pool, user_id, None).await;
    let _ = fs::remove_dir_all(fake_dir);
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_initiate_cursor_auth_marks_failed_when_process_exits_success_but_provider_unauthed() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let (fake_dir, fake_cursor_path) = create_fake_cursor_auth_cli();
    let _cursor_bin_guard = EnvVarGuard::set(
        "ACPMS_AGENT_CURSOR_BIN",
        fake_cursor_path.display().to_string(),
    );

    let (session_id, _body) = initiate_auth_session(&router, &token, "cursor-cli").await;
    let failed_payload = wait_for_failed_session(&router, &token, session_id).await;
    assert_provider_not_authenticated_failure(&failed_payload);

    cleanup_test_data(&pool, user_id, None).await;
    let _ = fs::remove_dir_all(fake_dir);
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_initiate_gemini_auth_marks_failed_when_process_exits_success_but_provider_unauthed() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let (fake_dir, fake_gemini_path) = create_fake_gemini_auth_cli();
    let _gemini_bin_guard = EnvVarGuard::set(
        "ACPMS_AGENT_GEMINI_BIN",
        fake_gemini_path.display().to_string(),
    );
    let _gemini_home_guard = EnvVarGuard::set(
        "ACPMS_GEMINI_HOME",
        std::env::temp_dir()
            .join(format!("acpms-gemini-home-{}", Uuid::new_v4()))
            .display()
            .to_string(),
    );

    let (session_id, _body) = initiate_auth_session(&router, &token, "gemini-cli").await;
    let failed_payload = wait_for_failed_session(&router, &token, session_id).await;
    assert_provider_not_authenticated_failure(&failed_payload);

    cleanup_test_data(&pool, user_id, None).await;
    let _ = fs::remove_dir_all(fake_dir);
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_submit_code_wrong_owner_returns_not_found() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;

    let (owner_id, _) = create_test_user(&pool, None, None, None).await;
    let (other_user_id, _) = create_test_user(&pool, None, None, None).await;
    let session = state
        .auth_session_store
        .create_session(
            owner_id,
            "openai-codex".to_string(),
            AuthFlowType::DeviceFlow,
            300,
        )
        .await;

    let router = create_router(state);
    let other_user_token = generate_test_token(other_user_id);

    let body = format!(
        r#"{{"session_id":"{}","code":"ABCD-1234"}}"#,
        session.session_id
    );
    let (status, response_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            "/api/v1/agent/auth/submit-code",
            Some(&body),
            vec![
                auth_header_bearer(&other_user_token),
                ("content-type", "application/json".to_string()),
            ],
        )
        .await;

    assert_eq!(
        status, 404,
        "Expected 404, got {}: {}",
        status, response_body
    );

    cleanup_test_data(&pool, owner_id, None).await;
    cleanup_test_data(&pool, other_user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_submit_code_expired_session_rejected() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let session = state
        .auth_session_store
        .create_session(
            user_id,
            "openai-codex".to_string(),
            AuthFlowType::DeviceFlow,
            1,
        )
        .await;

    let router = create_router(state);
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let body = format!(
        r#"{{"session_id":"{}","code":"ABCD-1234"}}"#,
        session.session_id
    );
    let (status, response_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            "/api/v1/agent/auth/submit-code",
            Some(&body),
            vec![
                auth_header_bearer(&token),
                ("content-type", "application/json".to_string()),
            ],
        )
        .await;

    assert_eq!(
        status, 400,
        "Expected 400, got {}: {}",
        status, response_body
    );
    assert!(
        response_body.to_lowercase().contains("expired"),
        "Expected expired error, got: {}",
        response_body
    );

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_submit_code_rejects_non_localhost_callback_url() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
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

    let router = create_router(state);
    let body = format!(
        r#"{{"session_id":"{}","code":"https://example.com/callback?code=abc"}}"#,
        session.session_id
    );
    let (status, response_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            "/api/v1/agent/auth/submit-code",
            Some(&body),
            vec![
                auth_header_bearer(&token),
                ("content-type", "application/json".to_string()),
            ],
        )
        .await;

    assert_eq!(
        status, 400,
        "Expected 400, got {}: {}",
        status, response_body
    );
    assert!(
        response_body
            .to_lowercase()
            .contains("only localhost callback urls are allowed"),
        "Expected localhost-only guard error, got: {}",
        response_body
    );

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_submit_code_rejects_malformed_localhost_callback_url() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state.clone());
    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let session = state
        .auth_session_store
        .create_session(
            user_id,
            "gemini-cli".to_string(),
            AuthFlowType::LoopbackProxy,
            300,
        )
        .await;

    let body = format!(
        r#"{{"session_id":"{}","code":"http://localhost:notaport/callback?code=abc"}}"#,
        session.session_id
    );
    let (status, response_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            "/api/v1/agent/auth/submit-code",
            Some(&body),
            vec![
                auth_header_bearer(&token),
                ("content-type", "application/json".to_string()),
            ],
        )
        .await;

    assert_eq!(
        status, 400,
        "Expected 400, got {}: {}",
        status, response_body
    );
    assert!(
        response_body
            .to_lowercase()
            .contains("invalid callback url format"),
        "Expected malformed callback URL error, got: {}",
        response_body
    );

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_submit_code_callback_error_message_redacts_query_params() {
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

    let body = format!(
        r#"{{"session_id":"{}","code":"http://127.0.0.1:65530/callback?code=supersecret&state=verysecret"}}"#,
        session.session_id
    );
    let (status, response_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            "/api/v1/agent/auth/submit-code",
            Some(&body),
            vec![
                auth_header_bearer(&token),
                ("content-type", "application/json".to_string()),
            ],
        )
        .await;

    assert_eq!(
        status, 400,
        "Expected 400, got {}: {}",
        status, response_body
    );
    let response_lower = response_body.to_lowercase();
    assert!(
        response_lower.contains("failed to call callback url"),
        "Expected callback error, got: {}",
        response_body
    );
    assert!(
        !response_lower.contains("supersecret"),
        "Expected redacted callback error payload, got: {}",
        response_body
    );
    assert!(
        !response_lower.contains("state=verysecret"),
        "Expected redacted callback error payload, got: {}",
        response_body
    );

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_auth_session_not_found() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let missing_session = uuid::Uuid::new_v4();

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/agent/auth/sessions/{}", missing_session),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 404, "Expected 404, got {}: {}", status, body);
    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    assert_eq!(response["success"].as_bool(), Some(false));

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_submit_code_valid_session_with_stdin_writer() {
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

    let _ = state
        .auth_session_store
        .update_owned_status(
            session.session_id,
            user_id,
            acpms_server::services::agent_auth::AuthSessionStatus::WaitingUserAction,
            None,
            None,
        )
        .await;

    let mut child = Command::new("sh")
        .arg("-c")
        .arg("cat >/dev/null")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn stdin sink process");
    let stdin = child.stdin.take().expect("missing child stdin");
    let _ = state
        .auth_session_store
        .set_stdin_writer(session.session_id, Some(stdin))
        .await;

    let body = format!(
        r#"{{"session_id":"{}","code":"ABCD-1234"}}"#,
        session.session_id
    );
    let (status, response_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            "/api/v1/agent/auth/submit-code",
            Some(&body),
            vec![
                auth_header_bearer(&token),
                ("content-type", "application/json".to_string()),
            ],
        )
        .await;

    assert_eq!(
        status, 200,
        "Expected 200, got {}: {}",
        status, response_body
    );
    let response: serde_json::Value =
        serde_json::from_str(&response_body).expect("Failed to parse response");
    assert_eq!(response["success"].as_bool(), Some(true));
    assert_eq!(
        response["data"]["status"].as_str(),
        Some("verifying"),
        "unexpected payload: {}",
        response_body
    );

    let _ = child.kill().await;
    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_cancel_auth_is_idempotent() {
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

    let body = format!(r#"{{"session_id":"{}"}}"#, session.session_id);

    let (first_status, first_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            "/api/v1/agent/auth/cancel",
            Some(&body),
            vec![
                auth_header_bearer(&token),
                ("content-type", "application/json".to_string()),
            ],
        )
        .await;
    assert_eq!(
        first_status, 200,
        "Expected first cancel 200, got {}: {}",
        first_status, first_body
    );

    let (second_status, second_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            "/api/v1/agent/auth/cancel",
            Some(&body),
            vec![
                auth_header_bearer(&token),
                ("content-type", "application/json".to_string()),
            ],
        )
        .await;
    assert_eq!(
        second_status, 200,
        "Expected second cancel 200, got {}: {}",
        second_status, second_body
    );

    let second_json: serde_json::Value =
        serde_json::from_str(&second_body).expect("parse second cancel response");
    assert_eq!(
        second_json["data"]["status"].as_str(),
        Some("cancelled"),
        "unexpected payload: {}",
        second_body
    );

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_cancel_auth_kills_running_child_process() {
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

    let mut child = Command::new("sh")
        .arg("-c")
        .arg("sleep 60")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn long running process");
    let pid = child.id().expect("child pid");
    let stdin = child.stdin.take().expect("child stdin");
    let _ = state
        .auth_session_store
        .set_stdin_writer(session.session_id, Some(stdin))
        .await;
    let _ = state
        .auth_session_store
        .set_process_info(session.session_id, Some(pid), None)
        .await;

    let body = format!(r#"{{"session_id":"{}"}}"#, session.session_id);
    let (status, response_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            "/api/v1/agent/auth/cancel",
            Some(&body),
            vec![
                auth_header_bearer(&token),
                ("content-type", "application/json".to_string()),
            ],
        )
        .await;

    assert_eq!(
        status, 200,
        "Expected 200, got {}: {}",
        status, response_body
    );

    let wait_result = tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await;
    assert!(
        wait_result.is_ok(),
        "Expected child process to terminate after cancel"
    );

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_expired_session_kills_child_and_marks_timeout() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state.clone());

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let session = state
        .auth_session_store
        .create_session(user_id, "gemini-cli".to_string(), AuthFlowType::OobCode, 1)
        .await;

    let mut child = Command::new("sh")
        .arg("-c")
        .arg("sleep 60")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn long running process");
    let pid = child.id().expect("child pid");
    let stdin = child.stdin.take().expect("child stdin");
    let _ = state
        .auth_session_store
        .set_stdin_writer(session.session_id, Some(stdin))
        .await;
    let _ = state
        .auth_session_store
        .set_process_info(session.session_id, Some(pid), None)
        .await;

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let (status, response_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/agent/auth/sessions/{}", session.session_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;

    assert_eq!(
        status, 200,
        "Expected 200, got {}: {}",
        status, response_body
    );
    let response_json: serde_json::Value =
        serde_json::from_str(&response_body).expect("parse session response");
    assert_eq!(
        response_json["data"]["status"].as_str(),
        Some("timed_out"),
        "unexpected payload: {}",
        response_body
    );

    let wait_result = tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await;
    assert!(
        wait_result.is_ok(),
        "Expected child process to terminate after session timeout"
    );

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_cancel_auth_writes_audit_log_record() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state.clone());

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let session = state
        .auth_session_store
        .create_session(
            user_id,
            "gemini-cli".to_string(),
            AuthFlowType::OobCode,
            300,
        )
        .await;

    let body = format!(r#"{{"session_id":"{}"}}"#, session.session_id);
    let (status, response_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            "/api/v1/agent/auth/cancel",
            Some(&body),
            vec![
                auth_header_bearer(&token),
                ("content-type", "application/json".to_string()),
            ],
        )
        .await;

    assert_eq!(
        status, 200,
        "Expected 200, got {}: {}",
        status, response_body
    );

    let row: Option<(String, String, Value)> = sqlx::query_as(
        r#"
        SELECT action, resource_type, metadata
        FROM audit_logs
        WHERE user_id = $1
          AND resource_id = $2
          AND action = 'agent_auth_cancelled'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(session.session_id)
    .fetch_optional(&pool)
    .await
    .expect("query audit log");

    let (action, resource_type, metadata) = row.expect("expected audit row");
    assert_eq!(action, "agent_auth_cancelled");
    assert_eq!(resource_type, "agent_auth_sessions");
    assert_eq!(
        metadata.get("provider").and_then(Value::as_str),
        Some("gemini-cli")
    );
    assert_eq!(
        metadata.get("status").and_then(Value::as_str),
        Some("cancelled")
    );

    cleanup_test_data(&pool, user_id, None).await;
}
