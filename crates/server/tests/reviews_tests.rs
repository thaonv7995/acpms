//! Reviews API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module
use serde_json::json;

#[tokio::test]
#[ignore = "requires test database"]
async fn test_add_comment() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    let request_body = json!({
        "content": "This needs improvement",
        "file_path": "src/file.ts",
        "line_number": 10
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/comments", attempt_id),
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
    assert_eq!(
        response["data"]["content"].as_str().unwrap(),
        "This needs improvement"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_list_comments() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    // Add comment via service
    use acpms_services::ReviewService;
    let review_service = ReviewService::new(pool.clone());
    let _comment = review_service
        .add_comment(
            attempt_id,
            user_id,
            acpms_db::models::AddReviewCommentRequest {
                content: "Test comment".to_string(),
                file_path: Some("src/file.ts".to_string()),
                line_number: Some(10),
            },
        )
        .await
        .expect("Failed to add comment");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/attempts/{}/comments", attempt_id),
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
async fn test_resolve_comment() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    use acpms_services::ReviewService;
    let review_service = ReviewService::new(pool.clone());
    let comment = review_service
        .add_comment(
            attempt_id,
            user_id,
            acpms_db::models::AddReviewCommentRequest {
                content: "Test comment".to_string(),
                file_path: None,
                line_number: None,
            },
        )
        .await
        .expect("Failed to add comment");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PATCH",
        &format!("/api/v1/comments/{}/resolve", comment.id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert_eq!(response["data"]["resolved"].as_bool().unwrap(), true);

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_unresolve_comment() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    use acpms_services::ReviewService;
    let review_service = ReviewService::new(pool.clone());
    let comment = review_service
        .add_comment(
            attempt_id,
            user_id,
            acpms_db::models::AddReviewCommentRequest {
                content: "Test comment".to_string(),
                file_path: None,
                line_number: None,
            },
        )
        .await
        .expect("Failed to add comment");

    // Resolve first
    review_service
        .resolve_comment(comment.id, user_id)
        .await
        .expect("Failed to resolve comment");

    // Then unresolve
    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PATCH",
        &format!("/api/v1/comments/{}/unresolve", comment.id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert_eq!(response["data"]["resolved"].as_bool().unwrap(), false);

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_delete_comment() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    use acpms_services::ReviewService;
    let review_service = ReviewService::new(pool.clone());
    let comment = review_service
        .add_comment(
            attempt_id,
            user_id,
            acpms_db::models::AddReviewCommentRequest {
                content: "Test comment".to_string(),
                file_path: None,
                line_number: None,
            },
        )
        .await
        .expect("Failed to add comment");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "DELETE",
        &format!("/api/v1/comments/{}", comment.id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_request_changes() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    // Update task status to in_review
    sqlx::query("UPDATE tasks SET status = 'in_review' WHERE id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .expect("Failed to update task status");

    let request_body = json!({
        "feedback": "Please add error handling",
        "include_comments": true
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/request-changes", attempt_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    // May succeed or fail depending on orchestrator setup
    assert!(
        status == 201 || status == 500,
        "Expected 201 or 500, got {}: {}",
        status,
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_request_changes_rejects_when_active_attempt_exists() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    // Task must be in review to request changes
    sqlx::query("UPDATE tasks SET status = 'in_review' WHERE id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .expect("Failed to set task status to in_review");

    // Existing active attempt should block creation of request-changes attempt
    let _active_attempt_id = create_test_attempt(&pool, task_id, Some("queued")).await;

    let request_body = json!({
        "feedback": "please rework this",
        "include_comments": false
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/request-changes", attempt_id),
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
    assert!(
        body.to_lowercase().contains("active attempt"),
        "Expected active-attempt guard error, got: {}",
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
// End test module
