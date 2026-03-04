//! Requirements API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module
use serde_json::json;

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_requirement() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let request_body = json!({
        "project_id": project_id.to_string(),
        "title": "Requirement Title",
        "content": "Requirement Content",
        "priority": "High"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/requirements", project_id),
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
        response["data"]["title"].as_str().unwrap(),
        "Requirement Title"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_list_project_requirements() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    // Create requirement via service
    use acpms_services::RequirementService;
    let requirement_service = RequirementService::new(pool.clone());
    let _requirement = requirement_service
        .create_requirement(
            user_id,
            acpms_db::models::CreateRequirementRequest {
                project_id,
                title: "Test Requirement".to_string(),
                content: "Test Requirement Content".to_string(),
                sprint_id: None,
                priority: Some(acpms_db::models::RequirementPriority::Medium),
                due_date: None,
                metadata: None,
            },
        )
        .await
        .expect("Failed to create requirement");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/requirements", project_id),
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
async fn test_get_requirement() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    use acpms_services::RequirementService;
    let requirement_service = RequirementService::new(pool.clone());
    let requirement = requirement_service
        .create_requirement(
            user_id,
            acpms_db::models::CreateRequirementRequest {
                project_id,
                title: "Test Requirement".to_string(),
                content: "Test Requirement Content".to_string(),
                sprint_id: None,
                priority: Some(acpms_db::models::RequirementPriority::Medium),
                due_date: None,
                metadata: None,
            },
        )
        .await
        .expect("Failed to create requirement");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!(
            "/api/v1/projects/{}/requirements/{}",
            project_id, requirement.id
        ),
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
        requirement.id.to_string()
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_update_requirement() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    use acpms_services::RequirementService;
    let requirement_service = RequirementService::new(pool.clone());
    let requirement = requirement_service
        .create_requirement(
            user_id,
            acpms_db::models::CreateRequirementRequest {
                project_id,
                title: "Test Requirement".to_string(),
                content: "Test Requirement Content".to_string(),
                sprint_id: None,
                priority: Some(acpms_db::models::RequirementPriority::Medium),
                due_date: None,
                metadata: None,
            },
        )
        .await
        .expect("Failed to create requirement");

    let request_body = json!({
        "title": "Updated Requirement Title",
        "priority": "High"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        &format!(
            "/api/v1/projects/{}/requirements/{}",
            project_id, requirement.id
        ),
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
    assert_eq!(
        response["data"]["title"].as_str().unwrap(),
        "Updated Requirement Title"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_delete_requirement() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    use acpms_services::RequirementService;
    let requirement_service = RequirementService::new(pool.clone());
    let requirement = requirement_service
        .create_requirement(
            user_id,
            acpms_db::models::CreateRequirementRequest {
                project_id,
                title: "Test Requirement".to_string(),
                content: "Test Requirement Content".to_string(),
                sprint_id: None,
                priority: Some(acpms_db::models::RequirementPriority::Medium),
                due_date: None,
                metadata: None,
            },
        )
        .await
        .expect("Failed to create requirement");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "DELETE",
        &format!(
            "/api/v1/projects/{}/requirements/{}",
            project_id, requirement.id
        ),
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
// End test module
