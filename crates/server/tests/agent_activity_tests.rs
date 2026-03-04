//! Agent Activity API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_agent_status() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let _attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/agent-activity/status",
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"].is_array());

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_agent_logs() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    // Insert test log
    sqlx::query(
        r#"
            INSERT INTO agent_logs (id, attempt_id, log_type, content)
            VALUES ($1, $2, 'system', 'Test log message')
            "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(attempt_id)
    .execute(&pool)
    .await
    .expect("Failed to insert test log");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/agent-activity/logs",
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"].is_array());

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_agent_logs_filtered_by_attempt() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    // Insert test log
    sqlx::query(
        r#"
            INSERT INTO agent_logs (id, attempt_id, log_type, content)
            VALUES ($1, $2, 'system', 'Test log message')
            "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(attempt_id)
    .execute(&pool)
    .await
    .expect("Failed to insert test log");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/agent-activity/logs?attempt_id={}", attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"].is_array());

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_agent_logs_filtered_by_project() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    // Insert test log
    sqlx::query(
        r#"
            INSERT INTO agent_logs (id, attempt_id, log_type, content)
            VALUES ($1, $2, 'system', 'Test log message')
            "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(attempt_id)
    .execute(&pool)
    .await
    .expect("Failed to insert test log");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/agent-activity/logs?project_id={}", project_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"].is_array());

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_agent_status_unauthorized() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/agent-activity/status",
        None,
        vec![],
    )
    .await;

    assert_eq!(
        status, 401,
        "Expected 401 Unauthorized, got {}: {}",
        status, body
    );
}
// End test module
