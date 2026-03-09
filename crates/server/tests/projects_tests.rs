//! Projects API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module
use serde_json::json;

fn analysis_only_repository_context() -> serde_json::Value {
    json!({
        "provider": "github",
        "access_mode": "analysis_only",
        "verification_status": "verified",
        "can_clone": true,
        "can_push": false,
        "can_open_change_request": false,
        "can_merge": false,
        "can_manage_webhooks": false,
        "can_fork": true,
        "upstream_repository_url": "https://github.com/acme/upstream-repo",
        "effective_clone_url": "https://github.com/acme/upstream-repo",
        "default_branch": "main",
        "verified_at": "2026-03-04T00:00:00Z"
    })
}

fn direct_gitops_repository_context() -> serde_json::Value {
    json!({
        "provider": "github",
        "access_mode": "direct_gitops",
        "verification_status": "verified",
        "can_clone": true,
        "can_push": true,
        "can_open_change_request": true,
        "can_merge": true,
        "can_manage_webhooks": true,
        "can_fork": true,
        "writable_repository_url": "https://github.com/example/direct-repo",
        "effective_clone_url": "https://github.com/example/direct-repo",
        "default_branch": "main",
        "verified_at": "2026-03-04T00:00:00Z"
    })
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_project() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let request_body = json!({
        "name": "Test Project",
        "description": "Test Description",
        "project_type": "Web"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/projects",
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 201,
        "Expected 201 Created, got {}: {}",
        status, body
    );

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert_eq!(response["data"]["name"].as_str().unwrap(), "Test Project");

    let project_id: uuid::Uuid = response["data"]["id"].as_str().unwrap().parse().unwrap();

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_list_projects() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/projects",
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"].is_array());
    assert!(response["data"].as_array().unwrap().len() > 0);

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_project() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}", project_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert_eq!(
        response["data"]["id"].as_str().unwrap(),
        project_id.to_string()
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_project_not_found() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let non_existent_id = uuid::Uuid::new_v4();

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}", non_existent_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(
        status, 404,
        "Expected 404 Not Found, got {}: {}",
        status, body
    );

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_list_project_members_excludes_openclaw_service_account() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Hidden Member Project")).await;
    let hidden_user_id = uuid::Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO users (id, email, name, password_hash, global_roles)
        VALUES ($1, $2, $3, NULL, $4)
        "#,
    )
    .bind(hidden_user_id)
    .bind("openclaw-gateway@acpms.local")
    .bind("OpenClaw Gateway")
    .bind(vec![acpms_db::models::SystemRole::Admin])
    .execute(&pool)
    .await
    .expect("create hidden service account");

    sqlx::query(
        r#"
        INSERT INTO project_members (project_id, user_id, roles)
        VALUES ($1, $2, ARRAY['developer']::project_role[])
        "#,
    )
    .bind(project_id)
    .bind(hidden_user_id)
    .execute(&pool)
    .await
    .expect("attach hidden service account to project");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/members", project_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    let members = response["data"].as_array().expect("members array");

    assert!(members
        .iter()
        .all(|member| member["email"].as_str() != Some("openclaw-gateway@acpms.local")));

    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(hidden_user_id)
        .execute(&pool)
        .await;
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_recheck_project_repository_access_requires_repository_url() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/repository-context/recheck", project_id),
        Some("{}"),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 400,
        "Expected 400 Bad Request, got {}: {}",
        status, body
    );
    assert!(body.contains("Project has no repository URL to re-check"));

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_link_existing_fork_rejects_same_upstream_repository_url() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let upstream_repository_url = "https://github.com/acme/upstream-repo";
    seed_project_repository_context(
        &pool,
        project_id,
        Some(upstream_repository_url),
        analysis_only_repository_context(),
    )
    .await;

    let request_body = json!({
        "repository_url": upstream_repository_url
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/repository-context/link-fork",
            project_id
        ),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 400,
        "Expected 400 Bad Request, got {}: {}",
        status, body
    );
    assert!(body.contains("Fork URL must be different from the upstream repository URL"));

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_project_fork_rejects_when_project_cannot_fork() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let repository_context = json!({
        "provider": "github",
        "access_mode": "analysis_only",
        "verification_status": "verified",
        "can_clone": true,
        "can_push": false,
        "can_open_change_request": false,
        "can_merge": false,
        "can_manage_webhooks": false,
        "can_fork": false,
        "upstream_repository_url": "https://github.com/acme/upstream-repo",
        "effective_clone_url": "https://github.com/acme/upstream-repo",
        "default_branch": "main",
        "verified_at": "2026-03-04T00:00:00Z"
    });
    seed_project_repository_context(
        &pool,
        project_id,
        Some("https://github.com/acme/upstream-repo"),
        repository_context,
    )
    .await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/repository-context/create-fork",
            project_id
        ),
        Some("{}"),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 400,
        "Expected 400 Bad Request, got {}: {}",
        status, body
    );
    assert!(body.contains("cannot create a fork"));

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_project_fork_rejects_when_project_already_supports_gitops() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    seed_project_repository_context(
        &pool,
        project_id,
        Some("https://github.com/example/direct-repo"),
        direct_gitops_repository_context(),
    )
    .await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/repository-context/create-fork",
            project_id
        ),
        Some("{}"),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 400,
        "Expected 400 Bad Request, got {}: {}",
        status, body
    );
    assert!(body.contains("Project already supports full GitOps"));

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_import_project_preflight_rejects_disallowed_repository_host() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let request_body = json!({
        "repository_url": "https://bitbucket.org/acme/upstream-repo"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/projects/import/preflight",
        Some(&request_body.to_string()),
        vec![("content-type", "application/json".to_string())],
    )
    .await;

    assert_eq!(
        status, 400,
        "Expected 400 Bad Request, got {}: {}",
        status, body
    );
    assert!(body.contains("Repository URL host 'bitbucket.org' is not allowed"));
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_update_project() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let request_body = json!({
        "name": "Updated Project Name",
        "description": "Updated Description"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        &format!("/api/v1/projects/{}", project_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert_eq!(
        response["data"]["name"].as_str().unwrap(),
        "Updated Project Name"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_delete_project() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "DELETE",
        &format!("/api/v1/projects/{}", project_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_project_settings() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/settings", project_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"]["settings"]["require_review"].is_boolean());

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_update_project_settings() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let request_body = json!({
        "require_review": false,
        "timeout_mins": 120,
        "max_retries": 5
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        &format!("/api/v1/projects/{}/settings", project_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert_eq!(
        response["data"]["settings"]["require_review"]
            .as_bool()
            .unwrap(),
        false
    );
    assert_eq!(
        response["data"]["settings"]["timeout_mins"]
            .as_i64()
            .unwrap(),
        120
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_patch_single_project_setting_supports_new_keys() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let request_body = json!({
        "value": 5
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PATCH",
        &format!("/api/v1/projects/{}/settings/max_concurrent", project_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    assert!(response["success"].as_bool().unwrap_or(false));
    assert_eq!(
        response["data"]["settings"]["max_concurrent"]
            .as_i64()
            .unwrap_or_default(),
        5
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_patch_single_project_setting_supports_auto_execute_priority() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let request_body = json!({
        "value": "high"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PATCH",
        &format!(
            "/api/v1/projects/{}/settings/auto_execute_priority",
            project_id
        ),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    assert!(response["success"].as_bool().unwrap_or(false));
    assert_eq!(
        response["data"]["settings"]["auto_execute_priority"]
            .as_str()
            .unwrap_or_default(),
        "high"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_patch_single_project_setting_supports_retry_backoff() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let request_body = json!({
        "value": "fixed"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PATCH",
        &format!("/api/v1/projects/{}/settings/retry_backoff", project_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    assert!(response["success"].as_bool().unwrap_or(false));
    assert_eq!(
        response["data"]["settings"]["retry_backoff"]
            .as_str()
            .unwrap_or_default(),
        "fixed"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_architecture() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/architecture", project_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"].is_object());

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_update_architecture() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let request_body = json!({
        "config": {
            "components": ["frontend", "backend"],
            "dependencies": ["react", "node"]
        }
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        &format!("/api/v1/projects/{}/architecture", project_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
// End test module
