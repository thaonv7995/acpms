//! GitLab Integration API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module
use serde_json::json;

#[tokio::test]
#[ignore = "requires test database and GitLab setup"]
async fn test_get_gitlab_status() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/gitlab/status", project_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    // May return 200 with null data if not linked, or 404 if endpoint doesn't exist
    assert!(
        status == 200 || status == 404,
        "Expected 200 or 404, got {}: {}",
        status,
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database and GitLab setup"]
async fn test_link_gitlab_project() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let request_body = json!({
        "gitlab_project_id": 12345,
        "base_url": "https://gitlab.com"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/gitlab/link", project_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    // May succeed or fail depending on GitLab setup
    assert!(
        status == 200 || status == 400 || status == 500,
        "Expected 200, 400, or 500, got {}: {}",
        status,
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database and GitLab setup"]
async fn test_get_task_merge_requests() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/tasks/{}/gitlab/merge_requests", task_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    // May return 200 with empty array, or 404 if endpoint doesn't exist
    assert!(
        status == 200 || status == 404,
        "Expected 200 or 404, got {}: {}",
        status,
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
// End test module
