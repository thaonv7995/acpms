#![cfg(unix)]

//! Preview lifecycle integration tests with mocked Cloudflare and mocked Docker command.

#[path = "helpers.rs"]
mod helpers;
use acpms_services::EncryptionService;
use axum::{
    extract::{Path, State},
    routing::{delete, post},
    Json, Router,
};
use helpers::*;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Debug, Default)]
struct MockCloudflareCounters {
    create_tunnel: usize,
    create_dns: usize,
    delete_tunnel: usize,
    delete_dns: usize,
}

#[derive(Clone)]
struct MockCloudflareState {
    counters: Arc<Mutex<MockCloudflareCounters>>,
    fail_dns_create: bool,
}

struct MockCloudflareHandle {
    base_url: String,
    state: MockCloudflareState,
    task: JoinHandle<()>,
}

impl Drop for MockCloudflareHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct EnvGuard {
    key: String,
    previous: Option<String>,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            std::env::set_var(&self.key, previous);
        } else {
            std::env::remove_var(&self.key);
        }
    }
}

fn set_env_guard(key: &str, value: &str) -> EnvGuard {
    let previous = std::env::var(key).ok();
    std::env::set_var(key, value);
    EnvGuard {
        key: key.to_string(),
        previous,
    }
}

async fn start_mock_cloudflare(fail_dns_create: bool) -> MockCloudflareHandle {
    let state = MockCloudflareState {
        counters: Arc::new(Mutex::new(MockCloudflareCounters::default())),
        fail_dns_create,
    };

    let app = Router::new()
        .route(
            "/client/v4/accounts/:account_id/cfd_tunnel",
            post(mock_create_tunnel),
        )
        .route(
            "/client/v4/accounts/:account_id/cfd_tunnel/:tunnel_id",
            delete(mock_delete_tunnel),
        )
        .route(
            "/client/v4/zones/:zone_id/dns_records",
            post(mock_create_dns),
        )
        .route(
            "/client/v4/zones/:zone_id/dns_records/:record_id",
            delete(mock_delete_dns),
        )
        .with_state(state.clone());

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind mock cloudflare listener");
    let addr = listener
        .local_addr()
        .expect("failed to get mock cloudflare local addr");
    let task = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("mock cloudflare server failed");
    });

    MockCloudflareHandle {
        base_url: format!("http://{}", addr),
        state,
        task,
    }
}

async fn mock_create_tunnel(
    Path(_account_id): Path<String>,
    State(state): State<MockCloudflareState>,
) -> Json<Value> {
    {
        let mut counters = state
            .counters
            .lock()
            .expect("failed to lock cloudflare counters");
        counters.create_tunnel += 1;
    }

    Json(json!({
        "success": true,
        "errors": [],
        "messages": [],
        "result": {
            "id": "mock-tunnel-123",
            "name": "mock-preview-tunnel",
            "secret": "bW9jay1zZWNyZXQ=",
            "created_at": "2026-02-27T00:00:00Z"
        }
    }))
}

async fn mock_delete_tunnel(
    Path((_account_id, _tunnel_id)): Path<(String, String)>,
    State(state): State<MockCloudflareState>,
) -> Json<Value> {
    {
        let mut counters = state
            .counters
            .lock()
            .expect("failed to lock cloudflare counters");
        counters.delete_tunnel += 1;
    }

    Json(json!({
        "success": true,
        "errors": [],
        "messages": [],
        "result": {}
    }))
}

async fn mock_create_dns(
    Path(_zone_id): Path<String>,
    State(state): State<MockCloudflareState>,
) -> Json<Value> {
    {
        let mut counters = state
            .counters
            .lock()
            .expect("failed to lock cloudflare counters");
        counters.create_dns += 1;
    }

    if state.fail_dns_create {
        Json(json!({
            "success": false,
            "errors": [
                { "code": 10013, "message": "mock dns create failure" }
            ],
            "messages": [],
            "result": {
                "id": "mock-dns-record-failed",
                "name": "task-mock.example.test",
                "content": "mock-tunnel-123.cfargotunnel.com",
                "type": "CNAME"
            }
        }))
    } else {
        Json(json!({
            "success": true,
            "errors": [],
            "messages": [],
            "result": {
                "id": "mock-dns-record-123",
                "name": "task-mock.example.test",
                "content": "mock-tunnel-123.cfargotunnel.com",
                "type": "CNAME"
            }
        }))
    }
}

async fn mock_delete_dns(
    Path((_zone_id, _record_id)): Path<(String, String)>,
    State(state): State<MockCloudflareState>,
) -> Json<Value> {
    {
        let mut counters = state
            .counters
            .lock()
            .expect("failed to lock cloudflare counters");
        counters.delete_dns += 1;
    }

    Json(json!({
        "success": true,
        "errors": [],
        "messages": [],
        "result": {}
    }))
}

fn extract_data_from_response(body: &str) -> Value {
    let parsed: Value = serde_json::from_str(body).expect("failed to parse API response body");
    if parsed.get("success").is_some() && parsed.get("data").is_some() {
        parsed.get("data").cloned().expect("missing data payload")
    } else {
        parsed
    }
}

fn create_mock_docker_command_script(prefix: &str) -> (PathBuf, PathBuf) {
    let base_dir = std::env::temp_dir().join(format!(
        "acpms-preview-docker-mock-{prefix}-{}",
        Uuid::new_v4()
    ));
    fs::create_dir_all(&base_dir).expect("failed to create mock docker temp directory");

    let script_path = base_dir.join("mock-docker.sh");
    let log_path = base_dir.join("mock-docker.log");
    let script = r#"#!/bin/sh
set -eu

if [ -n "${PREVIEW_DOCKER_MOCK_LOG:-}" ]; then
  echo "$*" >> "${PREVIEW_DOCKER_MOCK_LOG}"
fi

case "$*" in
  *" compose "*" up "*)
    if [ "${PREVIEW_DOCKER_MOCK_FAIL_UP:-0}" = "1" ]; then
      echo "mock docker compose up failure" >&2
      exit 1
    fi
    exit 0
    ;;
  *" compose "*" down "*)
    exit 0
    ;;
  *" compose "*" ps "*)
    if [ "${PREVIEW_DOCKER_MOCK_PS_READY:-1}" = "1" ]; then
      printf "dev-server\ncloudflared\n"
    else
      printf "dev-server\n"
    fi
    exit 0
    ;;
esac

exit 0
"#;
    fs::write(&script_path, script).expect("failed to write mock docker script");
    let mut permissions = fs::metadata(&script_path)
        .expect("failed to stat mock docker script")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).expect("failed to chmod mock docker script");

    (script_path, log_path)
}

async fn configure_cloudflare_settings(pool: &PgPool, base_domain: &str) {
    let encryption_key = std::env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY must be set");
    let encryption_service =
        EncryptionService::new(&encryption_key).expect("failed to create encryption service");
    let encrypted_token = encryption_service
        .encrypt("mock-cloudflare-token")
        .expect("failed to encrypt mock cloudflare token");

    sqlx::query(
        r#"
        UPDATE system_settings
        SET
            cloudflare_account_id = $1,
            cloudflare_api_token_encrypted = $2,
            cloudflare_zone_id = $3,
            cloudflare_base_domain = $4
        "#,
    )
    .bind("mock-account")
    .bind(encrypted_token)
    .bind("mock-zone")
    .bind(base_domain)
    .execute(pool)
    .await
    .expect("failed to configure cloudflare settings");
}

async fn configure_attempt_worktree(pool: &PgPool, attempt_id: Uuid) -> PathBuf {
    let worktree_path =
        std::env::temp_dir().join(format!("acpms-preview-worktree-{}", Uuid::new_v4()));
    fs::create_dir_all(&worktree_path).expect("failed to create worktree");

    sqlx::query(
        r#"
        UPDATE task_attempts
        SET metadata = COALESCE(metadata, '{}'::jsonb) || jsonb_build_object('worktree_path', $2::text)
        WHERE id = $1
        "#,
    )
    .bind(attempt_id)
    .bind(worktree_path.to_string_lossy().to_string())
    .execute(pool)
    .await
    .expect("failed to set attempt worktree metadata");

    worktree_path
}

#[tokio::test]
#[ignore = "requires DATABASE_URL test db; run manually for mocked preview lifecycle"]
async fn test_preview_lifecycle_start_stop_with_mocked_cloudflare_and_docker() {
    let mock_cf = start_mock_cloudflare(false).await;
    let (docker_cmd_path, docker_log_path) = create_mock_docker_command_script("success");

    let _env_guards = vec![
        set_env_guard("PREVIEW_DOCKER_RUNTIME_ENABLED", "true"),
        set_env_guard(
            "CLOUDFLARE_API_BASE_URL",
            &format!("{}/client/v4", mock_cf.base_url),
        ),
        set_env_guard("PREVIEW_DOCKER_COMMAND", &docker_cmd_path.to_string_lossy()),
        set_env_guard(
            "PREVIEW_DOCKER_MOCK_LOG",
            &docker_log_path.to_string_lossy(),
        ),
        set_env_guard("PREVIEW_DOCKER_MOCK_FAIL_UP", "0"),
        set_env_guard("PREVIEW_DOCKER_MOCK_PS_READY", "1"),
        set_env_guard(
            "PREVIEW_DEV_COMMAND",
            "echo mocked-preview-dev-server && sleep 1",
        ),
    ];

    let pool = setup_test_db().await;
    configure_cloudflare_settings(&pool, "example.test").await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Preview Lifecycle Mocked")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("Preview lifecycle start/stop"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;
    let worktree = configure_attempt_worktree(&pool, attempt_id).await;

    let (start_status, start_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!("/api/v1/attempts/{}/preview", attempt_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(
        start_status, 200,
        "expected start success, got {}: {}",
        start_status, start_body
    );

    let start_payload = extract_data_from_response(&start_body);
    assert!(
        start_payload.get("preview_url").is_some(),
        "missing preview_url in response"
    );

    let (runtime_status_code, runtime_status_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/attempts/{}/preview/runtime-status", attempt_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(
        runtime_status_code, 200,
        "expected runtime status success, got {}: {}",
        runtime_status_code, runtime_status_body
    );
    let runtime_payload = extract_data_from_response(&runtime_status_body);
    assert_eq!(
        runtime_payload
            .get("runtime_ready")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(runtime_payload
        .get("docker_project_name")
        .and_then(Value::as_str)
        .is_some());

    let (stop_status, stop_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "DELETE",
            &format!("/api/v1/previews/{}", attempt_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(
        stop_status, 200,
        "expected stop success, got {}: {}",
        stop_status, stop_body
    );

    let counters = mock_cf
        .state
        .counters
        .lock()
        .expect("failed to lock cloudflare counters");
    assert_eq!(counters.create_tunnel, 1);
    assert_eq!(counters.create_dns, 1);
    assert_eq!(counters.delete_tunnel, 1);
    assert_eq!(counters.delete_dns, 1);
    drop(counters);

    let docker_log =
        fs::read_to_string(&docker_log_path).unwrap_or_else(|_| String::from("<no docker log>"));
    assert!(docker_log.contains(" compose ") && docker_log.contains(" up "));
    assert!(docker_log.contains(" compose ") && docker_log.contains(" down "));

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
    let _ = fs::remove_dir_all(worktree);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL test db; run manually for mocked preview lifecycle"]
async fn test_preview_start_rolls_back_tunnel_when_dns_creation_fails() {
    let mock_cf = start_mock_cloudflare(true).await;
    let (docker_cmd_path, docker_log_path) = create_mock_docker_command_script("dns-fail");

    let _env_guards = vec![
        set_env_guard("PREVIEW_DOCKER_RUNTIME_ENABLED", "true"),
        set_env_guard(
            "CLOUDFLARE_API_BASE_URL",
            &format!("{}/client/v4", mock_cf.base_url),
        ),
        set_env_guard("PREVIEW_DOCKER_COMMAND", &docker_cmd_path.to_string_lossy()),
        set_env_guard(
            "PREVIEW_DOCKER_MOCK_LOG",
            &docker_log_path.to_string_lossy(),
        ),
        set_env_guard("PREVIEW_DOCKER_MOCK_FAIL_UP", "0"),
        set_env_guard("PREVIEW_DOCKER_MOCK_PS_READY", "1"),
        set_env_guard(
            "PREVIEW_DEV_COMMAND",
            "echo mocked-preview-dev-server && sleep 1",
        ),
    ];

    let pool = setup_test_db().await;
    configure_cloudflare_settings(&pool, "example.test").await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Preview DNS rollback")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("Preview DNS rollback task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;
    let worktree = configure_attempt_worktree(&pool, attempt_id).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/preview", attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;
    assert_eq!(
        status, 500,
        "expected DNS failure to bubble up as 500, got {}: {}",
        status, body
    );

    let counters = mock_cf
        .state
        .counters
        .lock()
        .expect("failed to lock cloudflare counters");
    assert_eq!(counters.create_tunnel, 1);
    assert_eq!(counters.create_dns, 1);
    assert_eq!(
        counters.delete_tunnel, 1,
        "tunnel should be rolled back on DNS failure"
    );
    assert_eq!(counters.delete_dns, 0);
    drop(counters);

    let active_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM cloudflare_tunnels
        WHERE attempt_id = $1
          AND deleted_at IS NULL
        "#,
    )
    .bind(attempt_id)
    .fetch_one(&pool)
    .await
    .expect("failed to query active tunnel count");
    assert_eq!(
        active_count, 0,
        "no active tunnel should persist after DNS rollback"
    );

    let docker_log = fs::read_to_string(&docker_log_path).unwrap_or_default();
    assert!(
        docker_log.trim().is_empty(),
        "docker runtime should not start when DNS creation already fails"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
    let _ = fs::remove_dir_all(worktree);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL test db; run manually for mocked preview lifecycle"]
async fn test_preview_start_rolls_back_when_docker_up_fails() {
    let mock_cf = start_mock_cloudflare(false).await;
    let (docker_cmd_path, docker_log_path) = create_mock_docker_command_script("docker-up-fail");

    let _env_guards = vec![
        set_env_guard("PREVIEW_DOCKER_RUNTIME_ENABLED", "true"),
        set_env_guard(
            "CLOUDFLARE_API_BASE_URL",
            &format!("{}/client/v4", mock_cf.base_url),
        ),
        set_env_guard("PREVIEW_DOCKER_COMMAND", &docker_cmd_path.to_string_lossy()),
        set_env_guard(
            "PREVIEW_DOCKER_MOCK_LOG",
            &docker_log_path.to_string_lossy(),
        ),
        set_env_guard("PREVIEW_DOCKER_MOCK_FAIL_UP", "1"),
        set_env_guard("PREVIEW_DOCKER_MOCK_PS_READY", "1"),
        set_env_guard(
            "PREVIEW_DEV_COMMAND",
            "echo mocked-preview-dev-server && sleep 1",
        ),
    ];

    let pool = setup_test_db().await;
    configure_cloudflare_settings(&pool, "example.test").await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Preview docker rollback")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("Preview docker rollback task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;
    let worktree = configure_attempt_worktree(&pool, attempt_id).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/preview", attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;
    assert_eq!(
        status, 500,
        "expected docker failure to return 500, got {}: {}",
        status, body
    );

    let tunnel_row = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        r#"
        SELECT
            status::text AS status,
            deleted_at::text AS deleted_at
        FROM cloudflare_tunnels
        WHERE attempt_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(attempt_id)
    .fetch_optional(&pool)
    .await
    .expect("failed to load tunnel row")
    .expect("expected tunnel row to exist");
    assert_eq!(tunnel_row.0.as_deref(), Some("deleted"));
    assert!(tunnel_row.1.is_some(), "expected deleted_at to be set");

    let counters = mock_cf
        .state
        .counters
        .lock()
        .expect("failed to lock cloudflare counters");
    assert_eq!(counters.create_tunnel, 1);
    assert_eq!(counters.create_dns, 1);
    assert_eq!(counters.delete_tunnel, 1);
    assert_eq!(counters.delete_dns, 1);
    drop(counters);

    let docker_log = fs::read_to_string(&docker_log_path).unwrap_or_default();
    assert!(
        docker_log.contains(" up "),
        "expected docker compose up invocation"
    );
    assert!(
        docker_log.contains(" down "),
        "expected docker compose down invocation during cleanup"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
    let _ = fs::remove_dir_all(worktree);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL test db; run manually for mocked preview lifecycle"]
async fn test_preview_start_is_idempotent_under_concurrent_requests() {
    let mock_cf = start_mock_cloudflare(false).await;
    let (docker_cmd_path, docker_log_path) = create_mock_docker_command_script("concurrent");

    let _env_guards = vec![
        set_env_guard("PREVIEW_DOCKER_RUNTIME_ENABLED", "true"),
        set_env_guard(
            "CLOUDFLARE_API_BASE_URL",
            &format!("{}/client/v4", mock_cf.base_url),
        ),
        set_env_guard("PREVIEW_DOCKER_COMMAND", &docker_cmd_path.to_string_lossy()),
        set_env_guard(
            "PREVIEW_DOCKER_MOCK_LOG",
            &docker_log_path.to_string_lossy(),
        ),
        set_env_guard("PREVIEW_DOCKER_MOCK_FAIL_UP", "0"),
        set_env_guard("PREVIEW_DOCKER_MOCK_PS_READY", "1"),
        set_env_guard(
            "PREVIEW_DEV_COMMAND",
            "echo mocked-preview-dev-server && sleep 1",
        ),
    ];

    let pool = setup_test_db().await;
    configure_cloudflare_settings(&pool, "example.test").await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Preview Concurrent Start")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("Preview concurrent start task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;
    let worktree = configure_attempt_worktree(&pool, attempt_id).await;

    let route_path = format!("/api/v1/attempts/{}/preview", attempt_id);
    let router_a = router.clone();
    let router_b = router.clone();
    let token_a = token.clone();
    let token_b = token.clone();

    let start_a = tokio::spawn(async move {
        make_request_with_string_headers(
            &router_a,
            "POST",
            &route_path,
            None,
            vec![auth_header_bearer(&token_a)],
        )
        .await
    });

    let start_b = tokio::spawn(async move {
        make_request_with_string_headers(
            &router_b,
            "POST",
            &format!("/api/v1/attempts/{}/preview", attempt_id),
            None,
            vec![auth_header_bearer(&token_b)],
        )
        .await
    });

    let (result_a, result_b) = tokio::join!(start_a, start_b);
    let (status_a, body_a) = result_a.expect("start request A join failed");
    let (status_b, body_b) = result_b.expect("start request B join failed");

    assert_eq!(status_a, 200, "request A failed: {}", body_a);
    assert_eq!(status_b, 200, "request B failed: {}", body_b);

    let active_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM cloudflare_tunnels
        WHERE attempt_id = $1
          AND deleted_at IS NULL
        "#,
    )
    .bind(attempt_id)
    .fetch_one(&pool)
    .await
    .expect("failed to query active tunnel count");
    assert_eq!(
        active_count, 1,
        "concurrent start should keep one active tunnel row"
    );

    let counters = mock_cf
        .state
        .counters
        .lock()
        .expect("failed to lock cloudflare counters");
    assert_eq!(
        counters.create_tunnel, 1,
        "concurrent start should create one cloudflare tunnel"
    );
    assert_eq!(
        counters.create_dns, 1,
        "concurrent start should create one DNS record"
    );
    drop(counters);

    let docker_log = fs::read_to_string(&docker_log_path).unwrap_or_default();
    let compose_up_count = docker_log
        .lines()
        .filter(|line| line.contains(" compose ") && line.contains(" up "))
        .count();
    assert_eq!(
        compose_up_count, 1,
        "concurrent start should invoke docker compose up exactly once"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
    let _ = fs::remove_dir_all(worktree);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL test db; run manually for mocked preview lifecycle"]
async fn test_preview_start_rolls_back_when_runtime_readiness_times_out() {
    let mock_cf = start_mock_cloudflare(false).await;
    let (docker_cmd_path, docker_log_path) = create_mock_docker_command_script("readiness-timeout");

    let _env_guards = vec![
        set_env_guard("PREVIEW_DOCKER_RUNTIME_ENABLED", "true"),
        set_env_guard(
            "CLOUDFLARE_API_BASE_URL",
            &format!("{}/client/v4", mock_cf.base_url),
        ),
        set_env_guard("PREVIEW_DOCKER_COMMAND", &docker_cmd_path.to_string_lossy()),
        set_env_guard(
            "PREVIEW_DOCKER_MOCK_LOG",
            &docker_log_path.to_string_lossy(),
        ),
        set_env_guard("PREVIEW_DOCKER_MOCK_FAIL_UP", "0"),
        set_env_guard("PREVIEW_DOCKER_MOCK_PS_READY", "0"),
        set_env_guard("PREVIEW_RUNTIME_START_TIMEOUT_SECS", "1"),
        set_env_guard(
            "PREVIEW_DEV_COMMAND",
            "echo mocked-preview-dev-server && sleep 1",
        ),
    ];

    let pool = setup_test_db().await;
    configure_cloudflare_settings(&pool, "example.test").await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Preview readiness timeout")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("Preview readiness timeout task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;
    let worktree = configure_attempt_worktree(&pool, attempt_id).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/preview", attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;
    assert_eq!(
        status, 500,
        "expected readiness timeout to return 500, got {}: {}",
        status, body
    );

    let active_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM cloudflare_tunnels
        WHERE attempt_id = $1
          AND deleted_at IS NULL
        "#,
    )
    .bind(attempt_id)
    .fetch_one(&pool)
    .await
    .expect("failed to query active tunnel count");
    assert_eq!(
        active_count, 0,
        "no active tunnel should persist after readiness-timeout rollback"
    );

    let counters = mock_cf
        .state
        .counters
        .lock()
        .expect("failed to lock cloudflare counters");
    assert_eq!(counters.create_tunnel, 1);
    assert_eq!(counters.create_dns, 1);
    assert_eq!(
        counters.delete_tunnel, 1,
        "tunnel should be deleted during readiness-timeout rollback"
    );
    assert_eq!(
        counters.delete_dns, 1,
        "dns should be deleted during readiness-timeout rollback"
    );
    drop(counters);

    let docker_log = fs::read_to_string(&docker_log_path).unwrap_or_default();
    assert!(
        docker_log.contains(" up "),
        "expected docker compose up invocation"
    );
    assert!(
        docker_log.contains(" ps "),
        "expected docker compose ps readiness checks"
    );
    assert!(
        docker_log.contains(" down "),
        "expected docker compose down during rollback"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
    let _ = fs::remove_dir_all(worktree);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL test db; run manually for mocked preview lifecycle"]
async fn test_preview_stop_is_idempotent_for_attempt_id() {
    let mock_cf = start_mock_cloudflare(false).await;
    let (docker_cmd_path, docker_log_path) = create_mock_docker_command_script("stop-idempotent");

    let _env_guards = vec![
        set_env_guard("PREVIEW_DOCKER_RUNTIME_ENABLED", "true"),
        set_env_guard(
            "CLOUDFLARE_API_BASE_URL",
            &format!("{}/client/v4", mock_cf.base_url),
        ),
        set_env_guard("PREVIEW_DOCKER_COMMAND", &docker_cmd_path.to_string_lossy()),
        set_env_guard(
            "PREVIEW_DOCKER_MOCK_LOG",
            &docker_log_path.to_string_lossy(),
        ),
        set_env_guard("PREVIEW_DOCKER_MOCK_FAIL_UP", "0"),
        set_env_guard("PREVIEW_DOCKER_MOCK_PS_READY", "1"),
        set_env_guard(
            "PREVIEW_DEV_COMMAND",
            "echo mocked-preview-dev-server && sleep 1",
        ),
    ];

    let pool = setup_test_db().await;
    configure_cloudflare_settings(&pool, "example.test").await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Preview stop idempotent")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("Preview stop idempotent task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;
    let worktree = configure_attempt_worktree(&pool, attempt_id).await;

    let (start_status, start_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!("/api/v1/attempts/{}/preview", attempt_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(
        start_status, 200,
        "expected start success before stop idempotency check, got {}: {}",
        start_status, start_body
    );

    let (stop_1_status, stop_1_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "DELETE",
            &format!("/api/v1/previews/{}", attempt_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(
        stop_1_status, 200,
        "first stop call should succeed, got {}: {}",
        stop_1_status, stop_1_body
    );

    let (stop_2_status, stop_2_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "DELETE",
            &format!("/api/v1/previews/{}", attempt_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(
        stop_2_status, 200,
        "second stop call should stay idempotent, got {}: {}",
        stop_2_status, stop_2_body
    );

    let active_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM cloudflare_tunnels
        WHERE attempt_id = $1
          AND deleted_at IS NULL
        "#,
    )
    .bind(attempt_id)
    .fetch_one(&pool)
    .await
    .expect("failed to query active tunnel count");
    assert_eq!(
        active_count, 0,
        "no active tunnel should remain after repeated stop"
    );

    let counters = mock_cf
        .state
        .counters
        .lock()
        .expect("failed to lock cloudflare counters");
    assert_eq!(counters.create_tunnel, 1);
    assert_eq!(counters.create_dns, 1);
    assert_eq!(
        counters.delete_tunnel, 1,
        "cloudflare tunnel delete should happen once across repeated stop"
    );
    assert_eq!(
        counters.delete_dns, 1,
        "cloudflare dns delete should happen once across repeated stop"
    );
    drop(counters);

    let docker_log = fs::read_to_string(&docker_log_path).unwrap_or_default();
    assert!(
        docker_log.contains(" up "),
        "expected docker compose up invocation"
    );
    assert!(
        docker_log.contains(" down "),
        "expected docker compose down invocation"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
    let _ = fs::remove_dir_all(worktree);
}
