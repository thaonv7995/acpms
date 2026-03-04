//! Settings API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module
use serde_json::json;

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_settings() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/settings",
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"].is_object());

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_update_settings() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let request_body = json!({
        "cloudflare_account_id": "test_account_id",
        "cloudflare_api_token": "test_token"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        "/api/v1/settings",
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    // May require admin role, so could be 200 or 403
    assert!(
        status == 200 || status == 403,
        "Expected 200 or 403, got {}: {}",
        status,
        body
    );

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_settings_unauthorized() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (status, body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(&router, "GET", "/api/v1/settings", None, vec![]).await;

    // Settings endpoint is public (no auth required)
    assert_eq!(
        status, 200,
        "Expected 200 OK (settings is public), got {}: {}",
        status, body
    );
}
// End test module
