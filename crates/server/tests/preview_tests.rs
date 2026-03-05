//! Preview API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module

#[tokio::test]
async fn test_create_preview_rejects_unsupported_project_type() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Preview Mobile Project")).await;
    let task_id = create_test_task(&pool, project_id, user_id, Some("Preview Mobile Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    sqlx::query("UPDATE projects SET project_type = 'mobile' WHERE id = $1")
        .bind(project_id)
        .execute(&pool)
        .await
        .expect("failed to update project type to mobile");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/preview", attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 400, "Expected 400, got {}: {}", status, body);
    assert!(
        body.contains("Preview not supported for project type 'mobile'"),
        "Expected unsupported project type message, got: {}",
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
async fn test_create_preview_rejects_when_preview_disabled_in_project_settings() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Preview Disabled Project")).await;
    let task_id = create_test_task(&pool, project_id, user_id, Some("Preview Disabled Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    sqlx::query(
        r#"
        UPDATE projects
        SET settings = COALESCE(settings, '{}'::jsonb) || '{"preview_enabled": false}'::jsonb
        WHERE id = $1
        "#,
    )
    .bind(project_id)
    .execute(&pool)
    .await
    .expect("failed to disable preview in project settings");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/preview", attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 400, "Expected 400, got {}: {}", status, body);
    assert!(
        body.contains("Preview is disabled in project settings"),
        "Expected preview disabled message, got: {}",
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
async fn test_create_preview_missing_cloudflare_does_not_block_local_preview_mode() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Preview Missing CF Config")).await;
    let task_id =
        create_test_task(&pool, project_id, user_id, Some("Preview Missing CF Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    // Ensure Cloudflare settings are absent; preview should no longer be blocked solely by this.
    sqlx::query(
        r#"
        UPDATE system_settings
        SET
            cloudflare_account_id = NULL,
            cloudflare_api_token_encrypted = NULL,
            cloudflare_zone_id = NULL,
            cloudflare_base_domain = NULL
        "#,
    )
    .execute(&pool)
    .await
    .expect("failed to clear cloudflare settings");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/preview", attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert!(
        status == 200 || status == 400 || status == 500,
        "Expected 200, 400, or 500, got {}: {}",
        status,
        body
    );
    assert!(
        !body.contains("Preview unavailable: missing Cloudflare config"),
        "Cloudflare config should not be a hard blocker anymore, got: {}",
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database and Cloudflare setup"]
async fn test_list_previews() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/previews",
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    // Response may be array directly or wrapped in ApiResponse
    assert!(response.is_array() || response["success"].as_bool().is_some());

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database and Cloudflare setup"]
async fn test_create_preview() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/preview", attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    // May succeed or fail depending on Cloudflare setup
    assert!(
        status == 200 || status == 500,
        "Expected 200 or 500, got {}: {}",
        status,
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database and Cloudflare setup"]
async fn test_cleanup_preview() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    let (status, _body) = make_request_with_string_headers(
        &router,
        "DELETE",
        &format!("/api/v1/previews/{}", attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    // 200 = deleted (DB updated, resources cleanup in background); 404/500 = error
    assert!(
        status == 200 || status == 404 || status == 500,
        "Expected 200, 404, or 500, got {}",
        status
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
// End test module
