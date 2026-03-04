//! Dashboard API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_dashboard() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let _project_id = create_test_project(&pool, user_id, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/dashboard",
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"]["stats"].is_object());
    assert!(response["data"]["projects"].is_array());
    assert!(response["data"]["agentLogs"].is_array());
    assert!(response["data"]["humanTasks"].is_array());

    // Verify stats structure
    let stats = &response["data"]["stats"];
    assert!(stats["activeProjects"].is_object());
    assert!(stats["agentsOnline"].is_object());
    assert!(stats["systemLoad"].is_object());
    assert!(stats["pendingPRs"].is_object());

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_dashboard_unauthorized() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (status, body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(&router, "GET", "/api/v1/dashboard", None, vec![]).await;

    assert_eq!(
        status, 401,
        "Expected 401 Unauthorized, got {}: {}",
        status, body
    );
}
// End test module
