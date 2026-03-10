mod helpers;

use std::{
    sync::{Mutex, OnceLock},
    time::Duration,
};

use axum::http::StatusCode;
use helpers::{
    auth_header_bearer, create_router, create_test_admin, create_test_app_state, create_test_user,
    make_request_with_string_headers, setup_test_db,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

async fn test_database_ready() -> bool {
    let Some(database_url) = helpers::resolve_test_database_url() else {
        return false;
    };

    tokio::time::timeout(Duration::from_secs(2), PgPool::connect(&database_url))
        .await
        .ok()
        .and_then(Result::ok)
        .is_some()
}

#[tokio::test]
async fn openclaw_admin_can_list_clients() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    std::env::set_var("OPENCLAW_GATEWAY_ENABLED", "true");
    if !test_database_ready().await {
        eprintln!("skipping openclaw admin test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    sqlx::query("DELETE FROM openclaw_bootstrap_tokens")
        .execute(&pool)
        .await
        .expect("clear bootstrap tokens");
    sqlx::query("DELETE FROM openclaw_client_keys")
        .execute(&pool)
        .await
        .expect("clear openclaw client keys");
    sqlx::query("DELETE FROM openclaw_clients")
        .execute(&pool)
        .await
        .expect("clear openclaw clients");

    let (admin_id, _) = create_test_admin(&pool).await;
    let admin_token = helpers::generate_test_token(admin_id);

    let client_row_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO openclaw_clients (id, client_id, display_name, status)
        VALUES ($1, $2, $3, 'active')
        "#,
    )
    .bind(client_row_id)
    .bind("oc_client_prod")
    .bind("OpenClaw Production")
    .execute(&pool)
    .await
    .expect("insert openclaw client");

    sqlx::query(
        r#"
        INSERT INTO openclaw_client_keys (client_id, key_id, public_key, fingerprint)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(client_row_id)
    .bind("key_2026_03")
    .bind("public-key-value")
    .bind("ed25519:ab12cd34")
    .execute(&pool)
    .await
    .expect("insert openclaw client key");

    let (status, body): (StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/admin/openclaw/clients",
        None,
        vec![auth_header_bearer(&admin_token)],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "unexpected body: {body}");

    let response: Value = serde_json::from_str(&body).expect("parse response");
    let clients = response["data"]["clients"]
        .as_array()
        .expect("clients array");
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0]["client_id"], "oc_client_prod");
    assert_eq!(clients[0]["display_name"], "OpenClaw Production");
    assert_eq!(clients[0]["key_fingerprints"][0], "ed25519:ab12cd34");
}

#[tokio::test]
async fn openclaw_admin_lists_pending_bootstrap_tokens_as_waiting_connections() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    std::env::set_var("OPENCLAW_GATEWAY_ENABLED", "true");
    if !test_database_ready().await {
        eprintln!("skipping openclaw admin test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    sqlx::query("DELETE FROM openclaw_bootstrap_tokens")
        .execute(&pool)
        .await
        .expect("clear bootstrap tokens");
    sqlx::query("DELETE FROM openclaw_client_keys")
        .execute(&pool)
        .await
        .expect("clear openclaw client keys");
    sqlx::query("DELETE FROM openclaw_clients")
        .execute(&pool)
        .await
        .expect("clear openclaw clients");

    let (admin_id, _) = create_test_admin(&pool).await;
    let admin_token = helpers::generate_test_token(admin_id);

    sqlx::query(
        r#"
        INSERT INTO openclaw_bootstrap_tokens (
            token_hash,
            label,
            suggested_display_name,
            status,
            expires_at
        )
        VALUES ($1, $2, $3, 'active', NOW() + INTERVAL '15 minutes')
        "#,
    )
    .bind("hash_waiting_token")
    .bind("OpenClaw Waiting")
    .bind("OpenClaw Waiting")
    .execute(&pool)
    .await
    .expect("insert waiting bootstrap token");

    let (status, body): (StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/admin/openclaw/clients",
        None,
        vec![auth_header_bearer(&admin_token)],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "unexpected body: {body}");

    let response: Value = serde_json::from_str(&body).expect("parse response");
    let clients = response["data"]["clients"]
        .as_array()
        .expect("clients array");
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0]["display_name"], "OpenClaw Waiting");
    assert_eq!(clients[0]["status"], "waiting_connection");
    assert_eq!(clients[0]["kind"], "pending");
    assert!(clients[0]["expires_at"].is_string());
}

#[tokio::test]
async fn openclaw_admin_can_delete_waiting_installation() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    std::env::set_var("OPENCLAW_GATEWAY_ENABLED", "true");
    if !test_database_ready().await {
        eprintln!("skipping openclaw admin test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    sqlx::query("DELETE FROM openclaw_bootstrap_tokens")
        .execute(&pool)
        .await
        .expect("clear bootstrap tokens");

    let (admin_id, _) = create_test_admin(&pool).await;
    let admin_token = helpers::generate_test_token(admin_id);

    let pending_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO openclaw_bootstrap_tokens (
            token_hash,
            label,
            suggested_display_name,
            status,
            expires_at
        )
        VALUES ($1, $2, $3, 'active', NOW() + INTERVAL '15 minutes')
        RETURNING id
        "#,
    )
    .bind("hash_delete_waiting_token")
    .bind("OpenClaw Delete Me")
    .bind("OpenClaw Delete Me")
    .fetch_one(&pool)
    .await
    .expect("insert waiting bootstrap token");

    let path = format!("/api/v1/admin/openclaw/clients/pending:{pending_id}/delete");
    let (status, body): (StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &path,
        Some("{}"),
        vec![
            auth_header_bearer(&admin_token),
            ("content-type", "application/json".to_string()),
        ],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "unexpected body: {body}");

    let response: Value = serde_json::from_str(&body).expect("parse response");
    assert_eq!(response["data"]["deleted"]["status"], "waiting_connection");

    let remaining = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM openclaw_bootstrap_tokens WHERE id = $1",
    )
    .bind(pending_id)
    .fetch_one(&pool)
    .await
    .expect("count remaining bootstrap tokens");
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn openclaw_admin_can_generate_bootstrap_prompt() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    std::env::set_var("OPENCLAW_GATEWAY_ENABLED", "true");
    std::env::set_var("OPENCLAW_API_KEY", "oc_test_phase1_key");
    std::env::set_var("OPENCLAW_WEBHOOK_SECRET", "wh_sec_test_only");
    if !test_database_ready().await {
        eprintln!("skipping openclaw admin test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    sqlx::query("DELETE FROM openclaw_bootstrap_tokens")
        .execute(&pool)
        .await
        .expect("clear bootstrap tokens");

    let (admin_id, _) = create_test_admin(&pool).await;
    let admin_token = helpers::generate_test_token(admin_id);
    let payload = json!({
        "label": "OpenClaw Staging",
        "expires_in_minutes": 15,
        "suggested_display_name": "OpenClaw Staging",
        "metadata": {
            "environment": "staging"
        }
    });

    let (status, body): (StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/admin/openclaw/bootstrap-tokens",
        Some(&payload.to_string()),
        vec![
            auth_header_bearer(&admin_token),
            ("content-type", "application/json".to_string()),
            ("host", "acpms.example.com".to_string()),
            ("x-forwarded-proto", "https".to_string()),
        ],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "unexpected body: {body}");

    let response: Value = serde_json::from_str(&body).expect("parse response");
    let prompt_text = response["data"]["prompt_text"]
        .as_str()
        .expect("prompt text");
    assert!(prompt_text.contains("https://acpms.example.com"));
    assert!(prompt_text.contains("ACPMS connection bundle:"));
    assert!(prompt_text.contains("Guide Endpoint"));
    assert!(prompt_text.contains("OpenClaw enrollment bundle:"));
    assert!(prompt_text.contains("Single-use bootstrap token"));
    assert!(prompt_text.contains("API Key (Bearer): oc_test_phase1_key"));
    assert!(prompt_text.contains("Webhook Secret: wh_sec_test_only (optional)"));
    assert!(prompt_text.contains("Generate a local Ed25519 keypair"));
    assert!(prompt_text.contains("Never send the private key to ACPMS"));
    assert!(prompt_text.contains("X-OpenClaw-Client-Id: <OPENCLAW_CLIENT_ID>"));

    let stored = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM openclaw_bootstrap_tokens WHERE label = 'OpenClaw Staging'",
    )
    .fetch_one(&pool)
    .await
    .expect("count stored bootstrap tokens");
    assert_eq!(stored, 1);
}

#[tokio::test]
async fn openclaw_admin_can_disable_enable_and_revoke_client() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    std::env::set_var("OPENCLAW_GATEWAY_ENABLED", "true");
    if !test_database_ready().await {
        eprintln!("skipping openclaw admin test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    sqlx::query("DELETE FROM openclaw_client_keys")
        .execute(&pool)
        .await
        .expect("clear openclaw client keys");
    sqlx::query("DELETE FROM openclaw_clients")
        .execute(&pool)
        .await
        .expect("clear openclaw clients");

    let (admin_id, _) = create_test_admin(&pool).await;
    let admin_token = helpers::generate_test_token(admin_id);
    let client_row_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO openclaw_clients (id, client_id, display_name, status)
        VALUES ($1, $2, $3, 'active')
        "#,
    )
    .bind(client_row_id)
    .bind("oc_client_toggle")
    .bind("OpenClaw Toggle")
    .execute(&pool)
    .await
    .expect("insert openclaw client");

    sqlx::query(
        r#"
        INSERT INTO openclaw_client_keys (client_id, key_id, public_key, fingerprint)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(client_row_id)
    .bind("key_main")
    .bind("public-key")
    .bind("ed25519:zz99yy88")
    .execute(&pool)
    .await
    .expect("insert client key");

    for (path, expected_status) in [
        (
            "/api/v1/admin/openclaw/clients/oc_client_toggle/disable",
            "disabled",
        ),
        (
            "/api/v1/admin/openclaw/clients/oc_client_toggle/enable",
            "active",
        ),
        (
            "/api/v1/admin/openclaw/clients/oc_client_toggle/revoke",
            "revoked",
        ),
    ] {
        let (status, body): (StatusCode, String) = make_request_with_string_headers(
            &router,
            "POST",
            path,
            Some("{}"),
            vec![
                auth_header_bearer(&admin_token),
                ("content-type", "application/json".to_string()),
            ],
        )
        .await;

        assert_eq!(status, StatusCode::OK, "unexpected body: {body}");
        let response: Value = serde_json::from_str(&body).expect("parse response");
        assert_eq!(response["data"]["client"]["status"], expected_status);
    }

    let stored_key_status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM openclaw_client_keys WHERE client_id = $1 LIMIT 1",
    )
    .bind(client_row_id)
    .fetch_one(&pool)
    .await
    .expect("load client key status");
    assert_eq!(stored_key_status, "revoked");
}

#[tokio::test]
async fn openclaw_admin_routes_require_system_admin() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    std::env::set_var("OPENCLAW_GATEWAY_ENABLED", "true");
    if !test_database_ready().await {
        eprintln!("skipping openclaw admin test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (viewer_id, _) = create_test_user(&pool, None, None, None).await;
    let viewer_token = helpers::generate_test_token(viewer_id);

    let (status, _body): (StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/admin/openclaw/clients",
        None,
        vec![auth_header_bearer(&viewer_token)],
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn openclaw_admin_routes_are_hidden_when_gateway_disabled() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    std::env::set_var("OPENCLAW_GATEWAY_ENABLED", "false");
    if !test_database_ready().await {
        eprintln!("skipping openclaw admin test because DATABASE_URL is not reachable");
        return;
    }

    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (admin_id, _) = create_test_admin(&pool).await;
    let admin_token = helpers::generate_test_token(admin_id);

    let (status, body): (StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/admin/openclaw/clients",
        None,
        vec![auth_header_bearer(&admin_token)],
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND, "unexpected body: {body}");
}
