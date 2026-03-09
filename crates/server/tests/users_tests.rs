//! Users API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module
use serde_json::json;

#[tokio::test]
#[ignore = "requires test database"]
async fn test_list_users() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/users",
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
async fn test_list_users_excludes_openclaw_service_account() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (admin_id, _) = create_test_admin(&pool).await;
    let admin_token = generate_test_token(admin_id);
    let (visible_user_id, _) =
        create_test_user(&pool, Some("visible@example.com"), None, None).await;
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

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        "/api/v1/users",
        None,
        vec![auth_header_bearer(&admin_token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    let users = response["data"].as_array().expect("users array");

    assert!(users
        .iter()
        .any(|user| user["email"].as_str() == Some("visible@example.com")));
    assert!(users
        .iter()
        .all(|user| user["email"].as_str() != Some("openclaw-gateway@acpms.local")));

    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(hidden_user_id)
        .execute(&pool)
        .await;
    cleanup_test_data(&pool, visible_user_id, None).await;
    cleanup_test_data(&pool, admin_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_user() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/users/{}", user_id),
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
        user_id.to_string()
    );

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_user_not_found() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let non_existent_id = uuid::Uuid::new_v4();

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/users/{}", non_existent_id),
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
async fn test_update_user() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let request_body = json!({
        "name": "Updated Name"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        &format!("/api/v1/users/{}", user_id),
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
    assert_eq!(response["data"]["name"].as_str().unwrap(), "Updated Name");

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_change_password() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let password = "oldpassword123";
    let (user_id, _) = create_test_user(&pool, None, Some(password), None).await;
    let token = generate_test_token(user_id);

    let request_body = json!({
        "current_password": password,
        "new_password": "newpassword123"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        &format!("/api/v1/users/{}/password", user_id),
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

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_change_password_invalid_current() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let request_body = json!({
        "current_password": "wrongpassword",
        "new_password": "newpassword123"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        &format!("/api/v1/users/{}/password", user_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 422,
        "Expected 422 Unprocessable Entity, got {}: {}",
        status, body
    );

    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_delete_user_admin() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (admin_id, _) = create_test_admin(&pool).await;
    let admin_token = generate_test_token(admin_id);

    let (target_user_id, _) = create_test_user(&pool, None, None, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "DELETE",
        &format!("/api/v1/users/{}", target_user_id),
        None,
        vec![auth_header_bearer(&admin_token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());

    cleanup_test_data(&pool, admin_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_avatar_upload_url() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let request_body = json!({
        "filename": "avatar.jpg",
        "content_type": "image/jpeg"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/users/avatar/upload-url",
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
    assert!(response["data"]["upload_url"].is_string());
    assert!(response["data"]["key"].is_string());

    cleanup_test_data(&pool, user_id, None).await;
}
// End test module
