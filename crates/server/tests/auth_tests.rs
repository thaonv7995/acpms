//! Authentication API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module
use serde_json::json;

#[tokio::test]
#[ignore = "requires test database"]
async fn test_register_success() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let request_body = json!({
        "email": "newuser@example.com",
        "name": "New User",
        "password": "password123"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/auth/register",
        Some(&request_body.to_string()),
        vec![("content-type", "application/json".to_string())],
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
    assert!(response["data"]["access_token"].is_string());
    assert!(response["data"]["refresh_token"].is_string());
    assert!(response["data"]["user"]["email"].as_str().unwrap() == "newuser@example.com");

    // Cleanup
    let user_id: uuid::Uuid = response["data"]["user"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_register_rejects_global_roles_field() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let request_body = json!({
        "email": format!("role-escalation-{}@example.com", uuid::Uuid::new_v4()),
        "name": "Escalation Attempt",
        "password": "password123",
        "global_roles": ["admin"]
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/auth/register",
        Some(&request_body.to_string()),
        vec![("content-type", "application/json".to_string())],
    )
    .await;

    assert_eq!(
        status, 400,
        "Expected 400 Bad Request when sending global_roles, got {}: {}",
        status, body
    );
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_register_duplicate_email() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    // Use unique email to avoid conflicts from previous test runs
    let email = format!("duplicate-{}@example.com", uuid::Uuid::new_v4());
    let (user_id, _) = create_test_user(&pool, Some(&email), None, None).await;

    let request_body = json!({
        "email": email,
        "name": "Duplicate User",
        "password": "password123"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/auth/register",
        Some(&request_body.to_string()),
        vec![("content-type", "application/json".to_string())],
    )
    .await;

    assert_eq!(
        status, 409,
        "Expected 409 Conflict, got {}: {}",
        status, body
    );

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(!response["success"].as_bool().unwrap());
    // Code can be "4090" (Conflict) or "4091" (ResourceAlreadyExists) depending on implementation
    let code = response["code"].as_str().unwrap();
    assert!(
        code == "4090" || code == "4091",
        "Expected code 4090 or 4091, got {}",
        code
    );

    // Cleanup: delete the user we created
    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_register_validation_error() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    // Password too short (validation error)
    let request_body = json!({
        "email": format!("validation-test-{}@example.com", uuid::Uuid::new_v4()),
        "name": "Test User",
        "password": "short"  // Less than 8 characters
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/auth/register",
        Some(&request_body.to_string()),
        vec![("content-type", "application/json".to_string())],
    )
    .await;

    // Validation errors return 422 Unprocessable Entity
    assert_eq!(
        status, 422,
        "Expected 422 Unprocessable Entity, got {}: {}",
        status, body
    );

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(!response["success"].as_bool().unwrap());
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_login_success() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let email = "login@example.com";
    let password = "password123";
    let (user_id, _) = create_test_user(&pool, Some(email), Some(password), None).await;

    let request_body = json!({
        "email": email,
        "password": password
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/auth/login",
        Some(&request_body.to_string()),
        vec![("content-type", "application/json".to_string())],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"]["access_token"].is_string());
    assert!(response["data"]["refresh_token"].is_string());

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_login_invalid_credentials() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let request_body = json!({
        "email": "nonexistent@example.com",
        "password": "wrongpassword"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/auth/login",
        Some(&request_body.to_string()),
        vec![("content-type", "application/json".to_string())],
    )
    .await;

    assert_eq!(
        status, 401,
        "Expected 401 Unauthorized, got {}: {}",
        status, body
    );

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(!response["success"].as_bool().unwrap());
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_refresh_token_success() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    // Create user with unique email
    let email = format!("refresh-test-{}@example.com", uuid::Uuid::new_v4());
    let password = "testpassword123";
    let (user_id, _) = create_test_user(&pool, Some(&email), Some(password), None).await;

    let login_body = json!({
        "email": email,
        "password": password
    });

    let (_, login_response): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/auth/login",
        Some(&login_body.to_string()),
        vec![("content-type", "application/json".to_string())],
    )
    .await;

    let login_data: serde_json::Value =
        serde_json::from_str(&login_response).expect("Failed to parse login response");

    let refresh_token = login_data["data"]["refresh_token"].as_str().unwrap();

    // Now refresh
    let refresh_body = json!({
        "refresh_token": refresh_token
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/auth/refresh",
        Some(&refresh_body.to_string()),
        vec![("content-type", "application/json".to_string())],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"]["access_token"].is_string());

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_refresh_token_invalid() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let refresh_body = json!({
        "refresh_token": "invalid-refresh-token"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/auth/refresh",
        Some(&refresh_body.to_string()),
        vec![("content-type", "application/json".to_string())],
    )
    .await;

    assert_eq!(
        status, 401,
        "Expected 401 Unauthorized, got {}: {}",
        status, body
    );

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(!response["success"].as_bool().unwrap());
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_logout_success() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    // Create user with unique email
    let email = format!("logout-test-{}@example.com", uuid::Uuid::new_v4());
    let password = "testpassword123";
    let (user_id, _) = create_test_user(&pool, Some(&email), Some(password), None).await;
    let token = generate_test_token(user_id);

    let login_body = json!({
        "email": email,
        "password": password
    });

    let (_, login_response): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/auth/login",
        Some(&login_body.to_string()),
        vec![("content-type", "application/json".to_string())],
    )
    .await;

    let login_data: serde_json::Value =
        serde_json::from_str(&login_response).expect("Failed to parse login response");

    let refresh_token = login_data["data"]["refresh_token"].as_str().unwrap();

    // Logout
    let logout_body = json!({
        "refresh_token": refresh_token
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/auth/logout",
        Some(&logout_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            ("authorization", format!("Bearer {}", token)),
        ],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());

    cleanup_test_data(&pool, user_id, None).await;
}
// End test module
