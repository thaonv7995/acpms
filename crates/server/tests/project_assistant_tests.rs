//! Project Assistant API integration tests
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::LazyLock,
    time::{Duration, Instant},
};

use acpms_executors::{append_assistant_log, get_assistant_log_file_path};
use acpms_server::state::AppState;
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::sync::Mutex;
use uuid::Uuid;

static PROJECT_ASSISTANT_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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

fn create_fake_cli(
    bin_name: &str,
    bootstrap_json: &str,
    response_json_template: &str,
) -> (PathBuf, PathBuf, PathBuf) {
    let base_dir =
        std::env::temp_dir().join(format!("acpms-fake-cli-{}-{}", bin_name, Uuid::new_v4()));
    fs::create_dir_all(&base_dir).expect("failed to create fake codex directory");

    let bin_dir = base_dir.join("bin");
    fs::create_dir_all(&bin_dir).expect("failed to create fake codex bin directory");

    let script_path = bin_dir.join(bin_name);
    let stdin_log_path = base_dir.join("stdin.log");
    let stdin_log_path_str = stdin_log_path.display().to_string();
    let script = format!(
        r#"#!/bin/sh
set -eu

printf '%s\n' '{bootstrap_json}'

while IFS= read -r line; do
  printf '%s\n' "$line" >> "{stdin_log_path_str}"
  escaped=$(printf '%s' "$line" | sed 's/\\/\\\\/g; s/"/\\"/g')
  printf '{response_json_template}\n' "$escaped"
done
"#
    );
    fs::write(&script_path, script).expect("failed to write fake codex script");
    let mut permissions = fs::metadata(&script_path)
        .expect("failed to stat fake codex script")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).expect("failed to chmod fake codex script");

    (base_dir, script_path, stdin_log_path)
}

fn create_fake_codex_cli() -> (PathBuf, PathBuf, PathBuf) {
    create_fake_cli(
        "codex",
        r#"{"type":"item.completed","item":{"id":"boot","type":"agent_message","text":"assistant booted codex"}}"#,
        r#"{"type":"item.completed","item":{"id":"reply","type":"agent_message","text":"%s"}}"#,
    )
}

fn create_fake_gemini_cli() -> (PathBuf, PathBuf, PathBuf) {
    create_fake_cli(
        "gemini",
        r#"{"type":"message","timestamp":"2026-03-01T15:41:37.071Z","role":"assistant","content":"assistant booted gemini","delta":true}"#,
        r#"{"type":"message","timestamp":"2026-03-01T15:41:38.071Z","role":"assistant","content":"%s","delta":true}"#,
    )
}

fn create_fake_cursor_cli() -> (PathBuf, PathBuf, PathBuf) {
    create_fake_cli(
        "agent",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"assistant booted cursor"}]},"session_id":"fake-cursor"}"#,
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"%s"}]},"session_id":"fake-cursor"}"#,
    )
}

fn create_test_log_dir() -> PathBuf {
    let log_dir =
        std::env::temp_dir().join(format!("acpms-assistant-test-logs-{}", Uuid::new_v4()));
    fs::create_dir_all(&log_dir).expect("failed to create test log dir");
    log_dir
}

async fn configure_project_assistant_provider(pool: &PgPool, provider: &str) {
    sqlx::query("UPDATE system_settings SET agent_cli_provider = $1")
        .bind(provider)
        .execute(pool)
        .await
        .expect("failed to configure project assistant provider");
}

fn parse_api_data(body: &str) -> Value {
    let parsed: Value = serde_json::from_str(body).expect("failed to parse API response");
    parsed
        .get("data")
        .cloned()
        .expect("response missing data field")
}

async fn create_assistant_session(
    router: &axum::Router,
    project_id: Uuid,
    token: &str,
    force_new: bool,
) -> Value {
    let (status, body) = make_request_with_string_headers(
        router,
        "POST",
        &format!("/api/v1/projects/{}/assistant/sessions", project_id),
        Some(&json!({ "force_new": force_new }).to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(token),
        ],
    )
    .await;

    assert_eq!(status, 201, "create session failed: {}", body);
    parse_api_data(&body)
}

async fn wait_for_active_session(state: &AppState, session_id: Uuid, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if state
            .orchestrator
            .is_assistant_session_active(session_id)
            .await
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "assistant session {} did not become active in time",
            session_id
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn wait_for_log_contains(session_id: Uuid, needle: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        let bytes = tokio::fs::read(get_assistant_log_file_path(session_id))
            .await
            .unwrap_or_default();
        let contents = String::from_utf8_lossy(&bytes);
        if contents.contains(needle) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "assistant log for {} did not contain {:?} in time",
            session_id,
            needle
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn requirement_count(pool: &PgPool, project_id: Uuid, title: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM requirements WHERE project_id = $1 AND title = $2",
    )
    .bind(project_id)
    .bind(title)
    .fetch_one(pool)
    .await
    .expect("failed to count requirements")
}

async fn run_project_assistant_start_then_input_keeps_session_alive(
    provider: &str,
    fake_cli_factory: fn() -> (PathBuf, PathBuf, PathBuf),
    expected_boot_message: &str,
) {
    let pool = setup_test_db().await;
    configure_project_assistant_provider(&pool, provider).await;

    let (_fake_cli_dir, fake_cli_path, fake_stdin_log) = fake_cli_factory();
    let _provider_bin_guard = match provider {
        "openai-codex" => {
            EnvVarGuard::set("ACPMS_AGENT_CODEX_BIN", fake_cli_path.display().to_string())
        }
        "gemini-cli" => EnvVarGuard::set(
            "ACPMS_AGENT_GEMINI_BIN",
            fake_cli_path.display().to_string(),
        ),
        "cursor-cli" => EnvVarGuard::set(
            "ACPMS_AGENT_CURSOR_BIN",
            fake_cli_path.display().to_string(),
        ),
        other => panic!("unsupported provider for fake CLI test: {other}"),
    };
    let _log_dir_guard =
        EnvVarGuard::set("ACPMS_LOG_DIR", create_test_log_dir().display().to_string());

    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state.clone());

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let session = create_assistant_session(&router, project_id, &token, false).await;
    let session_id: Uuid = session["id"]
        .as_str()
        .expect("missing session id")
        .parse()
        .expect("invalid session id");

    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/assistant/sessions/{}/start",
            project_id, session_id
        ),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;
    assert_eq!(status, 202, "start session failed: {}", body);

    wait_for_active_session(&state, session_id, Duration::from_secs(5)).await;
    wait_for_log_contains(session_id, expected_boot_message, Duration::from_secs(5)).await;

    let follow_up = format!(
        "Please confirm provider {} still receives follow-up input.",
        provider
    );
    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/assistant/sessions/{}/input",
            project_id, session_id
        ),
        Some(&json!({ "content": follow_up }).to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    assert_eq!(status, 200, "post input failed: {}", body);

    wait_for_log_contains(session_id, &follow_up, Duration::from_secs(5)).await;
    let stdin_log =
        fs::read_to_string(&fake_stdin_log).expect("failed to read fake provider stdin log");
    assert!(
        stdin_log.contains(&follow_up),
        "expected fake provider process to receive follow-up input, got: {}",
        stdin_log
    );

    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/assistant/sessions/{}/end",
            project_id, session_id
        ),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;
    assert_eq!(status, 200, "end session failed: {}", body);

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database and storage"]
async fn test_project_assistant_start_then_input_keeps_session_alive() {
    let _suite_lock = PROJECT_ASSISTANT_TEST_LOCK.lock().await;
    run_project_assistant_start_then_input_keeps_session_alive(
        "openai-codex",
        create_fake_codex_cli,
        "assistant booted codex",
    )
    .await;
}

#[tokio::test]
#[ignore = "requires test database and storage"]
async fn test_project_assistant_start_then_input_keeps_session_alive_gemini() {
    let _suite_lock = PROJECT_ASSISTANT_TEST_LOCK.lock().await;
    run_project_assistant_start_then_input_keeps_session_alive(
        "gemini-cli",
        create_fake_gemini_cli,
        "assistant booted gemini",
    )
    .await;
}

#[tokio::test]
#[ignore = "requires test database and storage"]
async fn test_project_assistant_start_then_input_keeps_session_alive_cursor() {
    let _suite_lock = PROJECT_ASSISTANT_TEST_LOCK.lock().await;
    run_project_assistant_start_then_input_keeps_session_alive(
        "cursor-cli",
        create_fake_cursor_cli,
        "assistant booted cursor",
    )
    .await;
}

#[tokio::test]
#[ignore = "requires test database and storage"]
async fn test_project_assistant_force_new_archives_old_session() {
    let _suite_lock = PROJECT_ASSISTANT_TEST_LOCK.lock().await;
    let pool = setup_test_db().await;
    let _log_dir_guard =
        EnvVarGuard::set("ACPMS_LOG_DIR", create_test_log_dir().display().to_string());
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let first_session = create_assistant_session(&router, project_id, &token, false).await;
    let first_session_id: Uuid = first_session["id"]
        .as_str()
        .expect("missing first session id")
        .parse()
        .expect("invalid first session id");

    append_assistant_log(first_session_id, "assistant", "first session message", None)
        .await
        .expect("failed to append assistant log");

    let second_session = create_assistant_session(&router, project_id, &token, true).await;
    let second_session_id: Uuid = second_session["id"]
        .as_str()
        .expect("missing second session id")
        .parse()
        .expect("invalid second session id");

    assert_ne!(
        first_session_id, second_session_id,
        "force_new should create a new session"
    );

    let archived = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT status, s3_log_key FROM project_assistant_sessions WHERE id = $1",
    )
    .bind(first_session_id)
    .fetch_one(&pool)
    .await
    .expect("failed to fetch archived session");

    assert_eq!(archived.0, "ended");
    assert!(
        archived
            .1
            .as_deref()
            .map(str::trim)
            .map(|key| !key.is_empty())
            .unwrap_or(true),
        "expected archived session s3 log key to be absent or non-empty"
    );

    let (status, body) = make_request_with_string_headers(
        &router,
        "GET",
        &format!(
            "/api/v1/projects/{}/assistant/sessions/{}",
            project_id, first_session_id
        ),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;
    assert_eq!(status, 200, "get archived session failed: {}", body);

    let archived_session = parse_api_data(&body);
    let archived_messages = archived_session["messages"]
        .as_array()
        .expect("archived session response missing messages");
    assert!(
        archived_messages.iter().any(|message| {
            message["content"]
                .as_str()
                .map(|content| content.contains("first session message"))
                .unwrap_or(false)
        }),
        "expected archived session history to remain readable"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database and storage"]
async fn test_project_assistant_confirm_tool_is_idempotent() {
    let _suite_lock = PROJECT_ASSISTANT_TEST_LOCK.lock().await;
    let pool = setup_test_db().await;
    let _log_dir_guard =
        EnvVarGuard::set("ACPMS_LOG_DIR", create_test_log_dir().display().to_string());
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let session = create_assistant_session(&router, project_id, &token, false).await;
    let session_id: Uuid = session["id"]
        .as_str()
        .expect("missing session id")
        .parse()
        .expect("invalid session id");

    let requirement_title = format!("Assistant Requirement {}", Uuid::new_v4());
    let tool_call_id = format!("tc_{}", Uuid::new_v4());
    append_assistant_log(
        session_id,
        "assistant",
        "",
        Some(&json!({
            "tool_calls": [{
                "id": tool_call_id,
                "name": "create_requirement",
                "args": {
                    "title": requirement_title,
                    "content": "Created from assistant tool confirmation test",
                    "priority": "high"
                }
            }]
        })),
    )
    .await
    .expect("failed to append tool call log");

    let request_body = json!({
        "tool_call_id": tool_call_id,
        "confirmed": true,
    })
    .to_string();

    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/assistant/sessions/{}/confirm-tool",
            project_id, session_id
        ),
        Some(&request_body),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    assert_eq!(status, 200, "first confirm-tool failed: {}", body);
    assert_eq!(
        requirement_count(&pool, project_id, &requirement_title).await,
        1
    );

    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/assistant/sessions/{}/confirm-tool",
            project_id, session_id
        ),
        Some(&request_body),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    assert_eq!(status, 200, "second confirm-tool failed: {}", body);
    assert_eq!(
        requirement_count(&pool, project_id, &requirement_title).await,
        1
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
