//! Sprints API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module
use serde_json::json;

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_sprint() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let request_body = json!({
        "project_id": project_id.to_string(),
        "name": "Sprint 1",
        "start_date": "2026-01-01T00:00:00Z",
        "end_date": "2026-01-14T00:00:00Z"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/sprints", project_id),
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
    assert_eq!(response["data"]["name"].as_str().unwrap(), "Sprint 1");

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_sprint_defaults_to_planning_status() {
    let pool = setup_test_db().await;
    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let project_id = create_test_project(&pool, user_id, None).await;

    let sprint_service = acpms_services::SprintService::new(pool.clone());
    let sprint = sprint_service
        .create_sprint(acpms_db::models::CreateSprintRequest {
            project_id,
            sequence: None,
            goal: None,
            name: "Sprint Planning Default".to_string(),
            description: None,
            start_date: None,
            end_date: None,
        })
        .await
        .expect("Failed to create sprint");

    assert_eq!(sprint.status, acpms_db::models::SprintStatus::Planned);

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_activating_sprint_completes_previous_active_sprint() {
    let pool = setup_test_db().await;
    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let project_id = create_test_project(&pool, user_id, None).await;

    let sprint_service = acpms_services::SprintService::new(pool.clone());
    let sprint_a = sprint_service
        .create_sprint(acpms_db::models::CreateSprintRequest {
            project_id,
            sequence: None,
            goal: None,
            name: "Sprint A".to_string(),
            description: None,
            start_date: None,
            end_date: None,
        })
        .await
        .expect("Failed to create sprint A");
    let sprint_b = sprint_service
        .create_sprint(acpms_db::models::CreateSprintRequest {
            project_id,
            sequence: None,
            goal: None,
            name: "Sprint B".to_string(),
            description: None,
            start_date: None,
            end_date: None,
        })
        .await
        .expect("Failed to create sprint B");

    sprint_service
        .update_sprint(
            sprint_a.id,
            acpms_db::models::UpdateSprintRequest {
                name: None,
                description: None,
                goal: None,
                status: Some(acpms_db::models::SprintStatus::Active),
                start_date: None,
                end_date: None,
            },
        )
        .await
        .expect("Failed to activate sprint A");

    sprint_service
        .update_sprint(
            sprint_b.id,
            acpms_db::models::UpdateSprintRequest {
                name: None,
                description: None,
                goal: None,
                status: Some(acpms_db::models::SprintStatus::Active),
                start_date: None,
                end_date: None,
            },
        )
        .await
        .expect("Failed to activate sprint B");

    let sprint_a_after = sprint_service
        .get_sprint(sprint_a.id)
        .await
        .expect("Failed to fetch sprint A")
        .expect("Sprint A not found");
    let sprint_b_after = sprint_service
        .get_sprint(sprint_b.id)
        .await
        .expect("Failed to fetch sprint B")
        .expect("Sprint B not found");

    assert_eq!(
        sprint_a_after.status,
        acpms_db::models::SprintStatus::Closed
    );
    assert_eq!(
        sprint_b_after.status,
        acpms_db::models::SprintStatus::Active
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_db_guard_prevents_multiple_active_sprints_per_project() {
    let pool = setup_test_db().await;
    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let project_id = create_test_project(&pool, user_id, None).await;

    let sprint_service = acpms_services::SprintService::new(pool.clone());
    let sprint_a = sprint_service
        .create_sprint(acpms_db::models::CreateSprintRequest {
            project_id,
            sequence: None,
            goal: None,
            name: "Sprint A".to_string(),
            description: None,
            start_date: None,
            end_date: None,
        })
        .await
        .expect("Failed to create sprint A");
    let sprint_b = sprint_service
        .create_sprint(acpms_db::models::CreateSprintRequest {
            project_id,
            sequence: None,
            goal: None,
            name: "Sprint B".to_string(),
            description: None,
            start_date: None,
            end_date: None,
        })
        .await
        .expect("Failed to create sprint B");

    sqlx::query("UPDATE sprints SET status = 'active' WHERE id = $1")
        .bind(sprint_a.id)
        .execute(&pool)
        .await
        .expect("Failed to activate sprint A");

    let second_activate = sqlx::query("UPDATE sprints SET status = 'active' WHERE id = $1")
        .bind(sprint_b.id)
        .execute(&pool)
        .await;

    assert!(
        second_activate.is_err(),
        "Expected unique active-sprint DB guard to reject second active sprint"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_list_project_sprints() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    // Create sprint via service
    use acpms_services::SprintService;
    let sprint_service = SprintService::new(pool.clone());
    let sprint = sprint_service
        .create_sprint(acpms_db::models::CreateSprintRequest {
            description: Some("Test Sprint Description".to_string()),
            project_id,
            sequence: None,
            goal: None,
            name: "Test Sprint".to_string(),
            start_date: Some(chrono::Utc::now()),
            end_date: Some(chrono::Utc::now() + chrono::Duration::days(14)),
        })
        .await
        .expect("Failed to create sprint");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/sprints", project_id),
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
async fn test_get_sprint() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    use acpms_services::SprintService;
    let sprint_service = SprintService::new(pool.clone());
    let sprint = sprint_service
        .create_sprint(acpms_db::models::CreateSprintRequest {
            description: Some("Test Sprint Description".to_string()),
            project_id,
            sequence: None,
            goal: None,
            name: "Test Sprint".to_string(),
            start_date: Some(chrono::Utc::now()),
            end_date: Some(chrono::Utc::now() + chrono::Duration::days(14)),
        })
        .await
        .expect("Failed to create sprint");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/sprints/{}", project_id, sprint.id),
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
        sprint.id.to_string()
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_update_sprint() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    use acpms_services::SprintService;
    let sprint_service = SprintService::new(pool.clone());
    let sprint = sprint_service
        .create_sprint(acpms_db::models::CreateSprintRequest {
            description: Some("Test Sprint Description".to_string()),
            project_id,
            sequence: None,
            goal: None,
            name: "Test Sprint".to_string(),
            start_date: Some(chrono::Utc::now()),
            end_date: Some(chrono::Utc::now() + chrono::Duration::days(14)),
        })
        .await
        .expect("Failed to create sprint");

    let request_body = json!({
        "name": "Updated Sprint Name"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        &format!("/api/v1/projects/{}/sprints/{}", project_id, sprint.id),
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
        response["data"]["name"].as_str().unwrap(),
        "Updated Sprint Name"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_delete_sprint() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    use acpms_services::SprintService;
    let sprint_service = SprintService::new(pool.clone());
    let sprint = sprint_service
        .create_sprint(acpms_db::models::CreateSprintRequest {
            description: Some("Test Sprint Description".to_string()),
            project_id,
            sequence: None,
            goal: None,
            name: "Test Sprint".to_string(),
            start_date: Some(chrono::Utc::now()),
            end_date: Some(chrono::Utc::now() + chrono::Duration::days(14)),
        })
        .await
        .expect("Failed to create sprint");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "DELETE",
        &format!("/api/v1/projects/{}/sprints/{}", project_id, sprint.id),
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
async fn test_get_active_sprint() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    // Create sprint then activate it
    let sprint_service = acpms_services::SprintService::new(pool.clone());
    let sprint = sprint_service
        .create_sprint(acpms_db::models::CreateSprintRequest {
            project_id,
            sequence: None,
            goal: None,
            name: "Active Sprint".to_string(),
            description: None,
            start_date: Some(chrono::Utc::now()),
            end_date: Some(chrono::Utc::now() + chrono::Duration::days(14)),
        })
        .await
        .expect("Failed to create sprint");
    sprint_service
        .update_sprint(
            sprint.id,
            acpms_db::models::UpdateSprintRequest {
                name: None,
                description: None,
                goal: None,
                status: Some(acpms_db::models::SprintStatus::Active),
                start_date: None,
                end_date: None,
            },
        )
        .await
        .expect("Failed to activate sprint");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/sprints/active", project_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"].is_object());
    assert_eq!(
        response["data"]["id"].as_str().unwrap(),
        sprint.id.to_string()
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_generate_sprints() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let request_body = json!({
        "project_id": project_id.to_string(),
        "start_date": "2026-01-01T00:00:00Z",
        "duration_weeks": 2,
        "count": 4
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/sprints/generate", project_id),
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
    assert!(response["data"].is_array());

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
// End test module
