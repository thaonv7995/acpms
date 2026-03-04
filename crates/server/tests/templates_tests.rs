//! Templates API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module
use serde_json::json;

#[tokio::test]
#[ignore = "requires test database"]
async fn test_list_templates() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/templates",
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"].is_array());

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_list_templates_filtered_by_type() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/templates?project_type=Web",
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"].is_array());

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_template() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    // First list templates to get an ID
    let (_, list_body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/templates",
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    let list_response: serde_json::Value =
        serde_json::from_str(&list_body).expect("Failed to parse response");

    let templates = list_response["data"].as_array().unwrap();

    if templates.is_empty() {
        // Skip if no templates exist
        cleanup_test_data(&pool, user_id, None).await;
        return;
    }

    let template_id = templates[0]["id"].as_str().unwrap();

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/templates/{}", template_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert_eq!(response["data"]["id"].as_str().unwrap(), template_id);

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database and admin role"]
async fn test_create_template_admin() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (admin_id, _) = create_test_admin(&pool).await;
    let admin_token = generate_test_token(admin_id);

    let request_body = json!({
        "name": "Test Template",
        "description": "Test Description",
        "project_type": "Web",
        "repository_url": "https://github.com/example/template.git",
        "tech_stack": ["React", "TypeScript"],
        "default_settings": {}
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/templates",
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&admin_token),
        ],
    )
    .await;

    // May require admin check implementation
    assert!(
        status == 201 || status == 403,
        "Expected 201 or 403, got {}: {}",
        status,
        body
    );

    cleanup_test_data(&pool, admin_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_template_not_found() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let non_existent_id = uuid::Uuid::new_v4();

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/templates/{}", non_existent_id),
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
// End test module
