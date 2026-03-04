//! Deployment Environments API Tests
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

use serde_json::json;
use std::fs;

fn env_value_or_file(var_name: &str) -> Option<String> {
    if let Ok(value) = std::env::var(var_name) {
        if !value.trim().is_empty() {
            return Some(value);
        }
    }

    let path_var = format!("{}_PATH", var_name);
    let path = std::env::var(path_var).ok()?;
    let content = fs::read_to_string(path).ok()?;
    if content.trim().is_empty() {
        None
    } else {
        Some(content)
    }
}

async fn wait_for_terminal_run_status(router: &axum::Router, token: &str, run_id: &str) -> String {
    for _ in 0..40 {
        let (get_status, get_resp): (axum::http::StatusCode, String) =
            make_request_with_string_headers(
                router,
                "GET",
                &format!("/api/v1/deployment-runs/{}", run_id),
                None,
                vec![auth_header_bearer(token)],
            )
            .await;

        assert_eq!(get_status, 200, "Get run failed: {}", get_resp);
        let get_json: serde_json::Value =
            serde_json::from_str(&get_resp).expect("Failed to parse run response");
        let status = get_json["data"]["status"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        if ["success", "failed", "cancelled", "rolled_back"].contains(&status.as_str()) {
            return status;
        }

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    panic!("Run {} did not reach terminal status in time", run_id);
}

async fn get_deployment_audit_actions(pool: &sqlx::PgPool, run_id: &str) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT action
        FROM audit_logs
        WHERE resource_type = 'deployment_runs'
          AND resource_id = $1::uuid
        ORDER BY created_at ASC
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .expect("Failed to fetch deployment audit logs")
}

async fn get_environment_audit_actions(pool: &sqlx::PgPool, env_id: &str) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT action
        FROM audit_logs
        WHERE resource_type = 'deployment_environments'
          AND resource_id = $1::uuid
        ORDER BY created_at ASC
        "#,
    )
    .bind(env_id)
    .fetch_all(pool)
    .await
    .expect("Failed to fetch deployment environment audit logs")
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_and_list_deployment_environments() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let create_body = json!({
        "name": "staging-eu",
        "target_type": "local",
        "deploy_path": std::env::temp_dir().join("acpms-deploy-staging-eu").to_string_lossy(),
        "is_default": true,
        "domain_config": {}
    });

    let (create_status, create_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/deployment-environments", project_id),
            Some(&create_body.to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;

    assert_eq!(
        create_status, 201,
        "Expected 201 Created, got {}: {}",
        create_status, create_resp
    );

    let created_json: serde_json::Value =
        serde_json::from_str(&create_resp).expect("Failed to parse create response");
    assert!(created_json["success"].as_bool().unwrap_or(false));
    assert_eq!(created_json["data"]["name"].as_str(), Some("staging-eu"));

    let (list_status, list_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/projects/{}/deployment-environments", project_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;

    assert_eq!(
        list_status, 200,
        "Expected 200 OK, got {}: {}",
        list_status, list_resp
    );

    let list_json: serde_json::Value =
        serde_json::from_str(&list_resp).expect("Failed to parse list response");
    let environments = list_json["data"]
        .as_array()
        .expect("data should be an array");
    assert!(
        environments
            .iter()
            .any(|env| env["name"].as_str() == Some("staging-eu")),
        "Expected environment staging-eu in response: {}",
        list_resp
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_update_and_get_deployment_environment() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let create_body = json!({
        "name": "qa",
        "target_type": "local",
        "deploy_path": std::env::temp_dir().join("acpms-deploy-qa").to_string_lossy(),
        "domain_config": {}
    });

    let (create_status, create_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/deployment-environments", project_id),
            Some(&create_body.to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;

    assert_eq!(create_status, 201, "Create failed: {}", create_resp);
    let created_json: serde_json::Value =
        serde_json::from_str(&create_resp).expect("Failed to parse create response");
    let env_id = created_json["data"]["id"]
        .as_str()
        .expect("missing environment id")
        .to_string();

    let update_body = json!({
        "name": "qa-updated",
        "is_default": true,
        "healthcheck_timeout_secs": 120,
        "healthcheck_expected_status": 204
    });

    let (update_status, update_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "PATCH",
            &format!(
                "/api/v1/projects/{}/deployment-environments/{}",
                project_id, env_id
            ),
            Some(&update_body.to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;

    assert_eq!(update_status, 200, "Update failed: {}", update_resp);

    let (get_status, get_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!(
                "/api/v1/projects/{}/deployment-environments/{}",
                project_id, env_id
            ),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;

    assert_eq!(get_status, 200, "Get failed: {}", get_resp);
    let get_json: serde_json::Value =
        serde_json::from_str(&get_resp).expect("Failed to parse get response");

    assert_eq!(get_json["data"]["name"].as_str(), Some("qa-updated"));
    assert_eq!(get_json["data"]["is_default"].as_bool(), Some(true));
    assert_eq!(
        get_json["data"]["healthcheck_expected_status"].as_i64(),
        Some(204)
    );

    let audit_actions = get_environment_audit_actions(&pool, &env_id).await;
    assert!(
        audit_actions.contains(&"deployment_environments.create".to_string()),
        "missing deployment_environments.create audit action: {:?}",
        audit_actions
    );
    assert!(
        audit_actions.contains(&"deployment_environments.update".to_string()),
        "missing deployment_environments.update audit action: {:?}",
        audit_actions
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_delete_deployment_environment() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let create_body = json!({
        "name": "uat",
        "target_type": "local",
        "deploy_path": std::env::temp_dir().join("acpms-deploy-uat").to_string_lossy(),
        "domain_config": {}
    });

    let (_, create_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/deployment-environments", project_id),
        Some(&create_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    let created_json: serde_json::Value =
        serde_json::from_str(&create_resp).expect("Failed to parse create response");
    let env_id = created_json["data"]["id"]
        .as_str()
        .expect("missing environment id")
        .to_string();

    let (delete_status, delete_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "DELETE",
            &format!(
                "/api/v1/projects/{}/deployment-environments/{}",
                project_id, env_id
            ),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;

    assert_eq!(delete_status, 200, "Delete failed: {}", delete_resp);

    let (get_status, _): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!(
            "/api/v1/projects/{}/deployment-environments/{}",
            project_id, env_id
        ),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(get_status, 404, "Expected environment to be deleted");

    let audit_actions = get_environment_audit_actions(&pool, &env_id).await;
    assert!(
        audit_actions.contains(&"deployment_environments.create".to_string()),
        "missing deployment_environments.create audit action: {:?}",
        audit_actions
    );
    assert!(
        audit_actions.contains(&"deployment_environments.delete".to_string()),
        "missing deployment_environments.delete audit action: {:?}",
        audit_actions
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_connection_local_environment_success() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let deploy_dir = std::env::temp_dir().join(format!("acpms-deploy-{}", user_id));
    let _ = tokio::fs::create_dir_all(&deploy_dir).await;

    let create_body = json!({
        "name": "local-dev",
        "target_type": "local",
        "deploy_path": deploy_dir.to_string_lossy(),
        "domain_config": {}
    });

    let (_, create_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/deployment-environments", project_id),
        Some(&create_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    let created_json: serde_json::Value =
        serde_json::from_str(&create_resp).expect("Failed to parse create response");
    let env_id = created_json["data"]["id"]
        .as_str()
        .expect("missing environment id")
        .to_string();

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/deployment-environments/{}/test-connection",
            project_id, env_id
        ),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response body");
    assert_eq!(response["data"]["success"].as_bool(), Some(true));

    let _ = tokio::fs::remove_dir_all(&deploy_dir).await;
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_environment_secret_is_encrypted_and_rotatable() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let create_body = json!({
        "name": "secret-env",
        "target_type": "ssh_remote",
        "deploy_path": "/tmp/secret-env",
        "target_config": {
            "host": "127.0.0.1",
            "port": 22,
            "username": "ubuntu"
        },
        "domain_config": {},
        "secrets": [
            { "secret_type": "ssh_password", "value": "SuperSecret#1" }
        ]
    });

    let (create_status, create_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/deployment-environments", project_id),
            Some(&create_body.to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;

    assert_eq!(create_status, 201, "Create failed: {}", create_resp);

    let created_json: serde_json::Value =
        serde_json::from_str(&create_resp).expect("Failed to parse create response");
    let env_id = created_json["data"]["id"]
        .as_str()
        .expect("missing environment id")
        .to_string();

    let first_ciphertext: String = sqlx::query_scalar(
        "SELECT ciphertext FROM deployment_environment_secrets WHERE environment_id = $1 AND secret_type = 'ssh_password'",
    )
    .bind(uuid::Uuid::parse_str(&env_id).expect("invalid env id"))
    .fetch_one(&pool)
    .await
    .expect("failed to load first ciphertext");

    assert_ne!(first_ciphertext, "SuperSecret#1");

    let update_body = json!({
        "secrets": [
            { "secret_type": "ssh_password", "value": "SuperSecret#2" }
        ]
    });

    let (update_status, update_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "PATCH",
            &format!(
                "/api/v1/projects/{}/deployment-environments/{}",
                project_id, env_id
            ),
            Some(&update_body.to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;
    assert_eq!(update_status, 200, "Update failed: {}", update_resp);

    let second_ciphertext: String = sqlx::query_scalar(
        "SELECT ciphertext FROM deployment_environment_secrets WHERE environment_id = $1 AND secret_type = 'ssh_password'",
    )
    .bind(uuid::Uuid::parse_str(&env_id).expect("invalid env id"))
    .fetch_one(&pool)
    .await
    .expect("failed to load second ciphertext");

    assert_ne!(second_ciphertext, "SuperSecret#2");
    assert_ne!(
        first_ciphertext, second_ciphertext,
        "ciphertext should rotate when secret value changes"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_secret_values_are_not_exposed_in_api_responses() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let raw_secret = "DoNotLeakMe#123";
    let create_body = json!({
        "name": "secret-redaction-env",
        "target_type": "ssh_remote",
        "deploy_path": "/tmp/secret-redaction",
        "target_config": {
            "host": "127.0.0.1",
            "port": 22,
            "username": "ubuntu"
        },
        "domain_config": {},
        "secrets": [
            { "secret_type": "ssh_password", "value": raw_secret }
        ]
    });

    let (create_status, create_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/deployment-environments", project_id),
            Some(&create_body.to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;
    assert_eq!(create_status, 201, "Create failed: {}", create_resp);
    assert!(
        !create_resp.contains(raw_secret),
        "Create response leaked secret: {}",
        create_resp
    );

    let created_json: serde_json::Value =
        serde_json::from_str(&create_resp).expect("Failed to parse create response");
    let env_id = created_json["data"]["id"]
        .as_str()
        .expect("missing environment id")
        .to_string();

    let (get_status, get_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!(
                "/api/v1/projects/{}/deployment-environments/{}",
                project_id, env_id
            ),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(get_status, 200, "Get failed: {}", get_resp);
    assert!(
        !get_resp.contains(raw_secret),
        "Get response leaked secret: {}",
        get_resp
    );

    let (conn_status, conn_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!(
                "/api/v1/projects/{}/deployment-environments/{}/test-connection",
                project_id, env_id
            ),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(conn_status, 200, "Test connection failed: {}", conn_resp);
    assert!(
        !conn_resp.contains(raw_secret),
        "Connection test response leaked secret: {}",
        conn_resp
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_ssh_connection_requires_known_hosts_secret() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let create_body = json!({
        "name": "ssh-no-known-hosts",
        "target_type": "ssh_remote",
        "deploy_path": "/tmp/ssh-no-known-hosts",
        "target_config": {
            "host": "127.0.0.1",
            "port": 5432,
            "username": "ubuntu"
        },
        "domain_config": {},
        "secrets": [
            { "secret_type": "ssh_password", "value": "password-only" }
        ]
    });

    let (_, create_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/deployment-environments", project_id),
        Some(&create_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    let created_json: serde_json::Value =
        serde_json::from_str(&create_resp).expect("Failed to parse create response");
    let env_id = created_json["data"]["id"]
        .as_str()
        .expect("missing env id")
        .to_string();

    let (conn_status, conn_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!(
                "/api/v1/projects/{}/deployment-environments/{}/test-connection",
                project_id, env_id
            ),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(conn_status, 200, "Connection check failed: {}", conn_resp);

    let conn_json: serde_json::Value =
        serde_json::from_str(&conn_resp).expect("Failed to parse connection response");
    assert_eq!(conn_json["data"]["success"].as_bool(), Some(false));
    let checks = conn_json["data"]["checks"]
        .as_array()
        .expect("checks should be array");

    assert!(
        checks.iter().any(|check| {
            check["step"].as_str() == Some("ssh_credentials")
                && check["status"].as_str() == Some("fail")
                && check["message"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("known_hosts")
        }),
        "Expected known_hosts policy failure in checks: {}",
        conn_resp
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_ssh_deploy_run_fails_fast_without_known_hosts_secret() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let create_env_body = json!({
        "name": "ssh-run-no-known-hosts",
        "target_type": "ssh_remote",
        "deploy_path": "/tmp/ssh-run-no-known-hosts",
        "target_config": {
            "host": "127.0.0.1",
            "port": 5432,
            "username": "ubuntu"
        },
        "domain_config": {},
        "secrets": [
            { "secret_type": "ssh_password", "value": "password-only" }
        ]
    });

    let (_, create_env_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/deployment-environments", project_id),
        Some(&create_env_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    let env_json: serde_json::Value =
        serde_json::from_str(&create_env_resp).expect("Failed to parse env response");
    let env_id = env_json["data"]["id"]
        .as_str()
        .expect("missing environment id")
        .to_string();

    let (_, start_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/deployment-environments/{}/deploy",
            project_id, env_id
        ),
        Some(&json!({"source_type": "branch", "source_ref": "main"}).to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    let start_json: serde_json::Value =
        serde_json::from_str(&start_resp).expect("Failed to parse start response");
    let run_id = start_json["data"]["id"]
        .as_str()
        .expect("missing run id")
        .to_string();

    let final_status = wait_for_terminal_run_status(&router, &token, &run_id).await;
    assert_eq!(final_status, "failed");

    let (get_status, get_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/deployment-runs/{}", run_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(get_status, 200, "Get run failed: {}", get_resp);
    let get_json: serde_json::Value =
        serde_json::from_str(&get_resp).expect("Failed to parse get run response");
    assert!(
        get_json["data"]["error_message"]
            .as_str()
            .unwrap_or_default()
            .contains("known_hosts"),
        "Expected known_hosts error in run: {}",
        get_resp
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database and ssh test server"]
async fn test_ssh_deploy_run_succeeds_with_known_hosts_and_private_key() {
    let host = std::env::var("ACPMS_SSH_TEST_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("ACPMS_SSH_TEST_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(2222);
    let username = std::env::var("ACPMS_SSH_TEST_USER").unwrap_or_else(|_| "acpms".to_string());
    let deploy_path = std::env::var("ACPMS_SSH_TEST_DEPLOY_PATH")
        .unwrap_or_else(|_| "/config/acpms-deploy-target".to_string());
    let private_key = env_value_or_file("ACPMS_SSH_TEST_PRIVATE_KEY")
        .expect("Set ACPMS_SSH_TEST_PRIVATE_KEY or ACPMS_SSH_TEST_PRIVATE_KEY_PATH");
    let known_hosts = env_value_or_file("ACPMS_SSH_TEST_KNOWN_HOSTS")
        .expect("Set ACPMS_SSH_TEST_KNOWN_HOSTS or ACPMS_SSH_TEST_KNOWN_HOSTS_PATH");

    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let create_env_body = json!({
        "name": "ssh-run-success",
        "target_type": "ssh_remote",
        "deploy_path": deploy_path,
        "target_config": {
            "host": host,
            "port": port,
            "username": username
        },
        "domain_config": {},
        "secrets": [
            { "secret_type": "ssh_private_key", "value": private_key },
            { "secret_type": "known_hosts", "value": known_hosts }
        ]
    });

    let (create_status, create_env_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/deployment-environments", project_id),
            Some(&create_env_body.to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;
    assert_eq!(
        create_status, 201,
        "Create SSH environment failed: {}",
        create_env_resp
    );
    let env_json: serde_json::Value =
        serde_json::from_str(&create_env_resp).expect("Failed to parse env response");
    let env_id = env_json["data"]["id"]
        .as_str()
        .expect("missing environment id")
        .to_string();

    let (start_status, start_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!(
                "/api/v1/projects/{}/deployment-environments/{}/deploy",
                project_id, env_id
            ),
            Some(&json!({"source_type": "branch", "source_ref": "main"}).to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;
    assert_eq!(start_status, 201, "Start deployment failed: {}", start_resp);

    let start_json: serde_json::Value =
        serde_json::from_str(&start_resp).expect("Failed to parse start response");
    let run_id = start_json["data"]["id"]
        .as_str()
        .expect("missing run id")
        .to_string();

    let final_status = wait_for_terminal_run_status(&router, &token, &run_id).await;
    if final_status != "success" {
        let (_, get_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/deployment-runs/{}", run_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
        panic!(
            "Expected SSH run success, got {}. Run payload: {}",
            final_status, get_resp
        );
    }

    let (timeline_status, timeline_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/deployment-runs/{}/timeline", run_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(timeline_status, 200, "Timeline failed: {}", timeline_resp);
    let timeline_json: serde_json::Value =
        serde_json::from_str(&timeline_resp).expect("Failed to parse timeline response");
    let events = timeline_json["data"]
        .as_array()
        .expect("timeline should be an array");
    assert!(
        events
            .iter()
            .any(|event| event["step"].as_str() == Some("connect")),
        "Expected connect timeline event in SSH run: {}",
        timeline_resp
    );
    assert!(
        events
            .iter()
            .any(|event| event["step"].as_str() == Some("deploy")),
        "Expected deploy timeline event in SSH run: {}",
        timeline_resp
    );
    assert!(
        events.iter().any(|event| {
            event["step"].as_str() == Some("finalize")
                && event["message"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("completed successfully")
        }),
        "Expected successful finalize event in SSH run: {}",
        timeline_resp
    );

    let audit_actions = get_deployment_audit_actions(&pool, &run_id).await;
    assert!(
        audit_actions.contains(&"deployment_runs.start".to_string()),
        "missing deployment_runs.start audit action for ssh run: {:?}",
        audit_actions
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_domain_check_requires_primary_domain() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let create_body = json!({
        "name": "domain-check",
        "target_type": "local",
        "deploy_path": std::env::temp_dir().join("acpms-domain-check").to_string_lossy(),
        "domain_config": {}
    });

    let (_, create_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/deployment-environments", project_id),
        Some(&create_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    let created_json: serde_json::Value =
        serde_json::from_str(&create_resp).expect("Failed to parse create response");
    let env_id = created_json["data"]["id"]
        .as_str()
        .expect("missing environment id")
        .to_string();

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/deployment-environments/{}/test-domain",
            project_id, env_id
        ),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response body");
    assert_eq!(response["data"]["success"].as_bool(), Some(false));

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_domain_mapping_soft_fail_keeps_run_success() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let deploy_dir = std::env::temp_dir().join(format!("acpms-domain-soft-{}", user_id));
    let _ = tokio::fs::create_dir_all(&deploy_dir).await;

    let create_env_body = json!({
        "name": "domain-soft-fail",
        "target_type": "local",
        "deploy_path": deploy_dir.to_string_lossy(),
        "domain_config": {
            "primary_domain": "nonexistent-acpms.invalid",
            "proxy_provider": "nginx",
            "failure_policy": "soft_fail"
        }
    });

    let (_, create_env_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/deployment-environments", project_id),
        Some(&create_env_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    let env_json: serde_json::Value =
        serde_json::from_str(&create_env_resp).expect("Failed to parse env response");
    let env_id = env_json["data"]["id"]
        .as_str()
        .expect("missing env id")
        .to_string();

    let (_, start_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/deployment-environments/{}/deploy",
            project_id, env_id
        ),
        Some(&json!({"source_type": "branch", "source_ref": "main"}).to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    let start_json: serde_json::Value =
        serde_json::from_str(&start_resp).expect("Failed to parse start response");
    let run_id = start_json["data"]["id"]
        .as_str()
        .expect("missing run id")
        .to_string();

    let final_status = wait_for_terminal_run_status(&router, &token, &run_id).await;
    assert_eq!(final_status, "success");

    let staged_proxy_config = deploy_dir.join(".acpms-proxy-nginx.conf");
    assert!(
        tokio::fs::metadata(&staged_proxy_config).await.is_ok(),
        "Expected staged proxy template at {}",
        staged_proxy_config.to_string_lossy()
    );

    let (timeline_status, timeline_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/deployment-runs/{}/timeline", run_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(timeline_status, 200, "Timeline failed: {}", timeline_resp);
    let timeline_json: serde_json::Value =
        serde_json::from_str(&timeline_resp).expect("Failed to parse timeline response");
    let timeline_events = timeline_json["data"]
        .as_array()
        .expect("timeline should be array");
    assert!(
        timeline_events
            .iter()
            .any(|event| event["step"].as_str() == Some("domain_config")),
        "Expected domain_config events in timeline: {}",
        timeline_resp
    );

    let _ = tokio::fs::remove_dir_all(&deploy_dir).await;
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_domain_mapping_hard_fail_marks_run_failed() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let deploy_dir = std::env::temp_dir().join(format!("acpms-domain-hard-{}", user_id));
    let _ = tokio::fs::create_dir_all(&deploy_dir).await;

    let create_env_body = json!({
        "name": "domain-hard-fail",
        "target_type": "local",
        "deploy_path": deploy_dir.to_string_lossy(),
        "domain_config": {
            "primary_domain": "nonexistent-acpms.invalid",
            "proxy_provider": "nginx",
            "failure_policy": "hard_fail"
        }
    });

    let (_, create_env_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/deployment-environments", project_id),
        Some(&create_env_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    let env_json: serde_json::Value =
        serde_json::from_str(&create_env_resp).expect("Failed to parse env response");
    let env_id = env_json["data"]["id"]
        .as_str()
        .expect("missing env id")
        .to_string();

    let (_, start_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/deployment-environments/{}/deploy",
            project_id, env_id
        ),
        Some(&json!({"source_type": "branch", "source_ref": "main"}).to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    let start_json: serde_json::Value =
        serde_json::from_str(&start_resp).expect("Failed to parse start response");
    let run_id = start_json["data"]["id"]
        .as_str()
        .expect("missing run id")
        .to_string();

    let final_status = wait_for_terminal_run_status(&router, &token, &run_id).await;
    assert_eq!(final_status, "failed");

    let (timeline_status, timeline_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/deployment-runs/{}/timeline", run_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(timeline_status, 200, "Timeline failed: {}", timeline_resp);
    let timeline_json: serde_json::Value =
        serde_json::from_str(&timeline_resp).expect("Failed to parse timeline response");
    let timeline_events = timeline_json["data"]
        .as_array()
        .expect("timeline should be array");
    assert!(
        timeline_events.iter().any(|event| {
            event["step"].as_str() == Some("domain_config")
                && event["event_type"].as_str() == Some("error")
        }),
        "Expected hard-fail error event in domain_config timeline: {}",
        timeline_resp
    );

    let _ = tokio::fs::remove_dir_all(&deploy_dir).await;
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_start_list_get_and_cancel_deployment_run() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let create_env_body = json!({
        "name": "run-dev",
        "target_type": "local",
        "deploy_path": std::env::temp_dir().join("acpms-run-dev").to_string_lossy(),
        "domain_config": {}
    });

    let (_, create_env_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/deployment-environments", project_id),
        Some(&create_env_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    let env_json: serde_json::Value =
        serde_json::from_str(&create_env_resp).expect("Failed to parse create env response");
    let env_id = env_json["data"]["id"]
        .as_str()
        .expect("missing environment id")
        .to_string();

    let start_run_body = json!({
        "source_type": "branch",
        "source_ref": "main"
    });
    let (start_status, start_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!(
                "/api/v1/projects/{}/deployment-environments/{}/deploy",
                project_id, env_id
            ),
            Some(&start_run_body.to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;

    assert_eq!(start_status, 201, "Start run failed: {}", start_resp);
    let run_json: serde_json::Value =
        serde_json::from_str(&start_resp).expect("Failed to parse start run response");
    let run_id = run_json["data"]["id"]
        .as_str()
        .expect("missing run id")
        .to_string();
    assert_eq!(run_json["data"]["status"].as_str(), Some("queued"));

    let (list_status, list_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/projects/{}/deployment-runs", project_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(list_status, 200, "List runs failed: {}", list_resp);

    let list_json: serde_json::Value =
        serde_json::from_str(&list_resp).expect("Failed to parse list runs response");
    assert!(list_json["data"]
        .as_array()
        .expect("runs should be array")
        .iter()
        .any(|r| r["id"].as_str() == Some(run_id.as_str())));

    let (get_status, get_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/deployment-runs/{}", run_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(get_status, 200, "Get run failed: {}", get_resp);

    let (cancel_status, cancel_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!("/api/v1/deployment-runs/{}/cancel", run_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(cancel_status, 200, "Cancel run failed: {}", cancel_resp);

    let cancel_json: serde_json::Value =
        serde_json::from_str(&cancel_resp).expect("Failed to parse cancel run response");
    assert_eq!(cancel_json["data"]["status"].as_str(), Some("cancelled"));

    let audit_actions = get_deployment_audit_actions(&pool, &run_id).await;
    assert!(
        audit_actions.contains(&"deployment_runs.start".to_string()),
        "missing deployment_runs.start audit action: {:?}",
        audit_actions
    );
    assert!(
        audit_actions.contains(&"deployment_runs.cancel".to_string()),
        "missing deployment_runs.cancel audit action: {:?}",
        audit_actions
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_retry_deployment_run_from_cancelled() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let create_env_body = json!({
        "name": "retry-env",
        "target_type": "local",
        "deploy_path": std::env::temp_dir().join("acpms-retry-env").to_string_lossy(),
        "domain_config": {}
    });

    let (_, create_env_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/deployment-environments", project_id),
        Some(&create_env_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    let env_json: serde_json::Value =
        serde_json::from_str(&create_env_resp).expect("Failed to parse create env response");
    let env_id = env_json["data"]["id"]
        .as_str()
        .expect("missing environment id")
        .to_string();

    let (start_status, start_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!(
                "/api/v1/projects/{}/deployment-environments/{}/deploy",
                project_id, env_id
            ),
            Some(&json!({"source_type": "branch", "source_ref": "release/v1"}).to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;
    assert_eq!(start_status, 201, "Start run failed: {}", start_resp);

    let start_json: serde_json::Value =
        serde_json::from_str(&start_resp).expect("Failed to parse start run response");
    let run_id = start_json["data"]["id"]
        .as_str()
        .expect("missing run id")
        .to_string();

    let (cancel_status, cancel_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!("/api/v1/deployment-runs/{}/cancel", run_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(cancel_status, 200, "Cancel run failed: {}", cancel_resp);

    let (retry_status, retry_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!("/api/v1/deployment-runs/{}/retry", run_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(retry_status, 201, "Retry run failed: {}", retry_resp);

    let retry_json: serde_json::Value =
        serde_json::from_str(&retry_resp).expect("Failed to parse retry response");
    let retry_status = retry_json["data"]["status"]
        .as_str()
        .expect("retry status should exist");
    assert!(
        ["queued", "running", "success"].contains(&retry_status),
        "Unexpected retry status: {}",
        retry_status
    );
    assert_eq!(retry_json["data"]["trigger_type"].as_str(), Some("retry"));
    let retry_run_id = retry_json["data"]["id"]
        .as_str()
        .expect("missing retry run id")
        .to_string();

    let original_audit_actions = get_deployment_audit_actions(&pool, &run_id).await;
    assert!(
        original_audit_actions.contains(&"deployment_runs.start".to_string()),
        "missing deployment_runs.start audit action for original run: {:?}",
        original_audit_actions
    );
    assert!(
        original_audit_actions.contains(&"deployment_runs.cancel".to_string()),
        "missing deployment_runs.cancel audit action for original run: {:?}",
        original_audit_actions
    );

    let retry_audit_actions = get_deployment_audit_actions(&pool, &retry_run_id).await;
    assert!(
        retry_audit_actions.contains(&"deployment_runs.retry".to_string()),
        "missing deployment_runs.retry audit action for retry run: {:?}",
        retry_audit_actions
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_local_deploy_run_reaches_success_and_has_timeline() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let deploy_dir = std::env::temp_dir().join(format!("acpms-success-run-{}", user_id));
    let _ = tokio::fs::create_dir_all(&deploy_dir).await;

    let create_env_body = json!({
        "name": "success-env",
        "target_type": "local",
        "deploy_path": deploy_dir.to_string_lossy(),
        "healthcheck_url": "http://localhost/health",
        "domain_config": {}
    });

    let (_, create_env_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/deployment-environments", project_id),
        Some(&create_env_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    let env_json: serde_json::Value =
        serde_json::from_str(&create_env_resp).expect("Failed to parse env response");
    let env_id = env_json["data"]["id"]
        .as_str()
        .expect("missing env id")
        .to_string();

    let (start_status, start_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!(
                "/api/v1/projects/{}/deployment-environments/{}/deploy",
                project_id, env_id
            ),
            Some(&json!({"source_type": "branch", "source_ref": "main"}).to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;
    assert_eq!(start_status, 201, "Start failed: {}", start_resp);
    let start_json: serde_json::Value =
        serde_json::from_str(&start_resp).expect("Failed to parse start response");
    let run_id = start_json["data"]["id"]
        .as_str()
        .expect("missing run id")
        .to_string();

    let final_status = wait_for_terminal_run_status(&router, &token, &run_id).await;
    assert_eq!(
        final_status, "success",
        "Expected run to finish with success, got {}",
        final_status
    );

    let (timeline_status, timeline_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/deployment-runs/{}/timeline", run_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(timeline_status, 200, "Timeline failed: {}", timeline_resp);

    let timeline_json: serde_json::Value =
        serde_json::from_str(&timeline_resp).expect("Failed to parse timeline response");
    let events = timeline_json["data"]
        .as_array()
        .expect("timeline data should be array");
    assert!(
        events.len() >= 4,
        "Expected timeline events, got {}",
        timeline_resp
    );
    assert!(events
        .iter()
        .any(|e| e["step"].as_str() == Some("precheck")));
    assert!(events.iter().any(|e| e["step"].as_str() == Some("deploy")));
    assert!(events
        .iter()
        .any(|e| e["step"].as_str() == Some("finalize")));

    let marker_file = deploy_dir.join(".acpms-last-deploy.txt");
    assert!(
        tokio::fs::metadata(&marker_file).await.is_ok(),
        "Expected marker file to exist at {}",
        marker_file.to_string_lossy()
    );

    let _ = tokio::fs::remove_dir_all(&deploy_dir).await;
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_list_and_get_deployment_releases_after_successful_run() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let deploy_dir = std::env::temp_dir().join(format!("acpms-release-test-{}", user_id));
    let _ = tokio::fs::create_dir_all(&deploy_dir).await;

    let create_env_body = json!({
        "name": "release-env",
        "target_type": "local",
        "deploy_path": deploy_dir.to_string_lossy(),
        "domain_config": {}
    });
    let (_, create_env_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/deployment-environments", project_id),
        Some(&create_env_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    let env_json: serde_json::Value =
        serde_json::from_str(&create_env_resp).expect("Failed to parse env response");
    let env_id = env_json["data"]["id"]
        .as_str()
        .expect("missing env id")
        .to_string();

    let (_, start_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/deployment-environments/{}/deploy",
            project_id, env_id
        ),
        Some(&json!({"source_type": "commit", "source_ref": "abc123"}).to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    let start_json: serde_json::Value =
        serde_json::from_str(&start_resp).expect("Failed to parse start response");
    let run_id = start_json["data"]["id"]
        .as_str()
        .expect("missing run id")
        .to_string();

    let final_status = wait_for_terminal_run_status(&router, &token, &run_id).await;
    assert_eq!(final_status, "success");

    let (list_status, list_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!(
                "/api/v1/projects/{}/deployment-environments/{}/releases",
                project_id, env_id
            ),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(list_status, 200, "List releases failed: {}", list_resp);

    let list_json: serde_json::Value =
        serde_json::from_str(&list_resp).expect("Failed to parse list releases response");
    let releases = list_json["data"]
        .as_array()
        .expect("releases should be array");
    assert!(
        !releases.is_empty(),
        "Expected at least one release: {}",
        list_resp
    );

    let release_id = releases[0]["id"]
        .as_str()
        .expect("missing release id")
        .to_string();
    assert_eq!(releases[0]["status"].as_str(), Some("active"));

    let (get_status, get_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/deployment-releases/{}", release_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(get_status, 200, "Get release failed: {}", get_resp);

    let get_json: serde_json::Value =
        serde_json::from_str(&get_resp).expect("Failed to parse get release response");
    assert_eq!(get_json["data"]["id"].as_str(), Some(release_id.as_str()));
    assert_eq!(get_json["data"]["run_id"].as_str(), Some(run_id.as_str()));

    let _ = tokio::fs::remove_dir_all(&deploy_dir).await;
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_rollback_run_reactivates_previous_release() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let deploy_dir = std::env::temp_dir().join(format!("acpms-rollback-test-{}", user_id));
    let _ = tokio::fs::create_dir_all(&deploy_dir).await;

    let create_env_body = json!({
        "name": "rollback-env",
        "target_type": "local",
        "deploy_path": deploy_dir.to_string_lossy(),
        "domain_config": {}
    });
    let (_, create_env_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/deployment-environments", project_id),
        Some(&create_env_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    let env_json: serde_json::Value =
        serde_json::from_str(&create_env_resp).expect("Failed to parse env response");
    let env_id = env_json["data"]["id"]
        .as_str()
        .expect("missing env id")
        .to_string();

    let (_, run1_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/deployment-environments/{}/deploy",
            project_id, env_id
        ),
        Some(&json!({"source_type": "commit", "source_ref": "commit-A"}).to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    let run1_json: serde_json::Value =
        serde_json::from_str(&run1_resp).expect("Failed to parse run1 response");
    let run1_id = run1_json["data"]["id"]
        .as_str()
        .expect("missing run1 id")
        .to_string();
    assert_eq!(
        wait_for_terminal_run_status(&router, &token, &run1_id).await,
        "success"
    );

    let (_, run2_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/deployment-environments/{}/deploy",
            project_id, env_id
        ),
        Some(&json!({"source_type": "commit", "source_ref": "commit-B"}).to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    let run2_json: serde_json::Value =
        serde_json::from_str(&run2_resp).expect("Failed to parse run2 response");
    let run2_id = run2_json["data"]["id"]
        .as_str()
        .expect("missing run2 id")
        .to_string();
    assert_eq!(
        wait_for_terminal_run_status(&router, &token, &run2_id).await,
        "success"
    );

    let (rollback_status, rollback_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            &format!("/api/v1/deployment-runs/{}/rollback", run1_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(rollback_status, 201, "Rollback failed: {}", rollback_resp);

    let rollback_json: serde_json::Value =
        serde_json::from_str(&rollback_resp).expect("Failed to parse rollback response");
    let rollback_run_id = rollback_json["data"]["id"]
        .as_str()
        .expect("missing rollback run id")
        .to_string();
    assert_eq!(
        wait_for_terminal_run_status(&router, &token, &rollback_run_id).await,
        "rolled_back"
    );

    let (timeline_status, timeline_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/deployment-runs/{}/timeline", rollback_run_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(timeline_status, 200, "Timeline failed: {}", timeline_resp);
    let timeline_json: serde_json::Value =
        serde_json::from_str(&timeline_resp).expect("Failed to parse rollback timeline");
    let timeline_events = timeline_json["data"]
        .as_array()
        .expect("timeline should be array");
    assert!(timeline_events
        .iter()
        .any(|event| event["step"].as_str() == Some("rollback")));

    let (releases_status, releases_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!(
                "/api/v1/projects/{}/deployment-environments/{}/releases",
                project_id, env_id
            ),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(
        releases_status, 200,
        "List releases failed: {}",
        releases_resp
    );
    let releases_json: serde_json::Value =
        serde_json::from_str(&releases_resp).expect("Failed to parse releases response");
    let releases = releases_json["data"]
        .as_array()
        .expect("releases should be array");

    let run1_release = releases
        .iter()
        .find(|release| release["run_id"].as_str() == Some(run1_id.as_str()))
        .expect("missing release for run1");
    let run2_release = releases
        .iter()
        .find(|release| release["run_id"].as_str() == Some(run2_id.as_str()))
        .expect("missing release for run2");

    assert_eq!(run1_release["status"].as_str(), Some("active"));
    assert_eq!(run2_release["status"].as_str(), Some("rolled_back"));

    let rollback_audit_actions = get_deployment_audit_actions(&pool, &rollback_run_id).await;
    assert!(
        rollback_audit_actions.contains(&"deployment_runs.rollback".to_string()),
        "missing deployment_runs.rollback audit action: {:?}",
        rollback_audit_actions
    );

    let rollback_marker = deploy_dir.join(".acpms-last-rollback.txt");
    assert!(
        tokio::fs::metadata(&rollback_marker).await.is_ok(),
        "Expected rollback marker to exist at {}",
        rollback_marker.to_string_lossy()
    );

    let _ = tokio::fs::remove_dir_all(&deploy_dir).await;
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_deployment_run_stream_returns_timeline_and_completed_event() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let deploy_dir = std::env::temp_dir().join(format!("acpms-stream-test-{}", user_id));
    let _ = tokio::fs::create_dir_all(&deploy_dir).await;

    let create_env_body = json!({
        "name": "stream-env",
        "target_type": "local",
        "deploy_path": deploy_dir.to_string_lossy(),
        "domain_config": {}
    });
    let (_, create_env_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/deployment-environments", project_id),
        Some(&create_env_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    let env_json: serde_json::Value =
        serde_json::from_str(&create_env_resp).expect("Failed to parse env response");
    let env_id = env_json["data"]["id"]
        .as_str()
        .expect("missing env id")
        .to_string();

    let (_, start_resp): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/deployment-environments/{}/deploy",
            project_id, env_id
        ),
        Some(&json!({"source_type": "branch", "source_ref": "main"}).to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    let start_json: serde_json::Value =
        serde_json::from_str(&start_resp).expect("Failed to parse start response");
    let run_id = start_json["data"]["id"]
        .as_str()
        .expect("missing run id")
        .to_string();

    assert_eq!(
        wait_for_terminal_run_status(&router, &token, &run_id).await,
        "success"
    );

    let (stream_status, stream_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "GET",
            &format!("/api/v1/deployment-runs/{}/stream", run_id),
            None,
            vec![auth_header_bearer(&token)],
        )
        .await;
    assert_eq!(stream_status, 200, "Stream failed: {}", stream_body);
    assert!(
        stream_body.contains("event: timeline"),
        "Expected timeline events in stream body: {}",
        stream_body
    );
    assert!(
        stream_body.contains("event: completed"),
        "Expected completed event in stream body: {}",
        stream_body
    );

    let _ = tokio::fs::remove_dir_all(&deploy_dir).await;
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
