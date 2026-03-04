//! Health Check API Tests

// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

#[tokio::test]
#[ignore = "requires test database"]
async fn test_health_check() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool).await;
    let router = create_router(state);

    let (status, body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(&router, "GET", "/health", None, vec![]).await;

    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body).expect("Failed to parse JSON");
    assert_eq!(json["status"], "healthy");
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_health_ready() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool).await;
    let router = create_router(state);

    let (status, body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(&router, "GET", "/health/ready", None, vec![]).await;

    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body).expect("Failed to parse JSON");
    assert_eq!(json["status"], "healthy");
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_health_live() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool).await;
    let router = create_router(state);

    let (status, body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(&router, "GET", "/health/live", None, vec![]).await;

    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body).expect("Failed to parse JSON");
    assert_eq!(json["status"], "healthy");
}
