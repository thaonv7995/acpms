//! Tasks API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module
use serde_json::json;

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_task() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let request_body = json!({
        "project_id": project_id.to_string(),
        "title": "Test Task",
        "description": "Test Description",
        "task_type": "Feature"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/tasks",
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
    assert_eq!(response["data"]["title"].as_str().unwrap(), "Test Task");

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_list_tasks() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let _task_id = create_test_task(&pool, project_id, user_id, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/tasks?project_id={}", project_id),
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
async fn test_list_tasks_orders_by_priority_metadata() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;

    let high_task_id = create_test_task(&pool, project_id, user_id, Some("High Task")).await;
    let low_task_id = create_test_task(&pool, project_id, user_id, Some("Low Task")).await;

    sqlx::query("UPDATE tasks SET metadata = $2::jsonb WHERE id = $1")
        .bind(high_task_id)
        .bind(serde_json::json!({ "priority": "high" }))
        .execute(&pool)
        .await
        .expect("Failed to set high task priority");
    sqlx::query("UPDATE tasks SET metadata = $2::jsonb WHERE id = $1")
        .bind(low_task_id)
        .bind(serde_json::json!({ "priority": "low" }))
        .execute(&pool)
        .await
        .expect("Failed to set low task priority");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/tasks?project_id={}", project_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);
    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    let titles: Vec<String> = response["data"]
        .as_array()
        .expect("data should be array")
        .iter()
        .filter_map(|item| item["title"].as_str().map(ToString::to_string))
        .collect();

    let high_idx = titles
        .iter()
        .position(|title| title == "High Task")
        .expect("High Task missing");
    let low_idx = titles
        .iter()
        .position(|title| title == "Low Task")
        .expect("Low Task missing");
    assert!(
        high_idx < low_idx,
        "Expected High Task before Low Task, got {:?}",
        titles
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_task() {
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
        &format!("/api/v1/tasks/{}", task_id),
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
        task_id.to_string()
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_update_task_status() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;

    let request_body = json!({
        "status": "in_progress"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        &format!("/api/v1/tasks/{}/status", task_id),
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
    assert_eq!(response["data"]["status"].as_str().unwrap(), "in_progress");

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_assign_task() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let (assignee_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;

    // Add assignee as project member
    sqlx::query(
        r#"
            INSERT INTO project_members (project_id, user_id, roles)
            VALUES ($1, $2, ARRAY['developer']::project_role[])
            "#,
    )
    .bind(project_id)
    .bind(assignee_id)
    .execute(&pool)
    .await
    .expect("Failed to add project member");

    let request_body = json!({
        "user_id": assignee_id.to_string()
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/tasks/{}/assign", task_id),
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
        response["data"]["assigned_to"].as_str().unwrap(),
        assignee_id.to_string()
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
    cleanup_test_data(&pool, assignee_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_delete_task() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "DELETE",
        &format!("/api/v1/tasks/{}", task_id),
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
async fn test_create_task_rejects_sprint_from_another_project() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_a = create_test_project(&pool, user_id, Some("Project A")).await;
    let project_b = create_test_project(&pool, user_id, Some("Project B")).await;

    let sprint_service = acpms_services::SprintService::new(pool.clone());
    let sprint_b = sprint_service
        .create_sprint(acpms_db::models::CreateSprintRequest {
            project_id: project_b,
            sequence: None,
            goal: None,
            name: "Sprint B".to_string(),
            description: None,
            start_date: None,
            end_date: None,
        })
        .await
        .expect("Failed to create sprint in project B");

    let request_body = json!({
        "project_id": project_a.to_string(),
        "title": "Cross-project sprint",
        "description": "Should fail",
        "task_type": "Feature",
        "sprint_id": sprint_b.id.to_string()
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/tasks",
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 500,
        "Expected 500 Internal Server Error, got {}: {}",
        status, body
    );
    assert!(
        body.to_lowercase().contains("same project"),
        "Expected cross-project validation message, got: {}",
        body
    );

    let _ = sqlx::query("DELETE FROM project_members WHERE project_id = $1")
        .bind(project_b)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(project_b)
        .execute(&pool)
        .await;
    cleanup_test_data(&pool, user_id, Some(project_a)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_task_rejects_parent_task_from_another_project() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_a = create_test_project(&pool, user_id, Some("Project A")).await;
    let project_b = create_test_project(&pool, user_id, Some("Project B")).await;
    let parent_in_b = create_test_task(&pool, project_b, user_id, Some("Parent in B")).await;

    let request_body = json!({
        "project_id": project_a.to_string(),
        "title": "Child in A",
        "description": "Should fail",
        "task_type": "Feature",
        "parent_task_id": parent_in_b.to_string()
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/tasks",
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 500,
        "Expected 500 Internal Server Error, got {}: {}",
        status, body
    );
    let created_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE project_id = $1 AND title = $2")
            .bind(project_a)
            .bind("Child in A")
            .fetch_one(&pool)
            .await
            .expect("Failed to count tasks in project A");
    assert_eq!(
        created_count, 0,
        "Task should not be created on invalid parent"
    );

    let _ = sqlx::query("DELETE FROM tasks WHERE project_id = $1")
        .bind(project_b)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM project_members WHERE project_id = $1")
        .bind(project_b)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(project_b)
        .execute(&pool)
        .await;
    cleanup_test_data(&pool, user_id, Some(project_a)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_task_rejects_requirement_from_another_project() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_a = create_test_project(&pool, user_id, Some("Project A")).await;
    let project_b = create_test_project(&pool, user_id, Some("Project B")).await;

    let requirement_service = acpms_services::RequirementService::new(pool.clone());
    let requirement_in_b = requirement_service
        .create_requirement(
            user_id,
            acpms_db::models::CreateRequirementRequest {
                project_id: project_b,
                title: "Req in B".to_string(),
                content: "Cross project".to_string(),
                priority: None,
                due_date: None,
                metadata: None,
                sprint_id: None,
            },
        )
        .await
        .expect("Failed to create requirement in project B");

    let request_body = json!({
        "project_id": project_a.to_string(),
        "title": "Task in A",
        "description": "Should fail",
        "task_type": "Feature",
        "requirement_id": requirement_in_b.id.to_string()
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        "/api/v1/tasks",
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 500,
        "Expected 500 Internal Server Error, got {}: {}",
        status, body
    );
    let created_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE project_id = $1 AND title = $2")
            .bind(project_a)
            .bind("Task in A")
            .fetch_one(&pool)
            .await
            .expect("Failed to count tasks in project A");
    assert_eq!(
        created_count, 0,
        "Task should not be created with cross-project requirement"
    );

    let _ = sqlx::query("DELETE FROM requirements WHERE project_id = $1")
        .bind(project_b)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM tasks WHERE project_id = $1")
        .bind(project_b)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM project_members WHERE project_id = $1")
        .bind(project_b)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(project_b)
        .execute(&pool)
        .await;
    cleanup_test_data(&pool, user_id, Some(project_a)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_assign_task_rejects_non_member_assignee() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (owner_id, _) = create_test_user(&pool, None, None, None).await;
    let (outsider_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(owner_id);
    let project_id = create_test_project(&pool, owner_id, None).await;
    let task_id = create_test_task(&pool, project_id, owner_id, None).await;

    let request_body = json!({
        "user_id": outsider_id.to_string()
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/tasks/{}/assign", task_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 500,
        "Expected 500 Internal Server Error, got {}: {}",
        status, body
    );
    assert!(
        body.to_lowercase().contains("not a member"),
        "Expected non-member validation error, got: {}",
        body
    );

    cleanup_test_data(&pool, owner_id, Some(project_id)).await;
    cleanup_test_data(&pool, outsider_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_update_task_rejects_sprint_from_another_project() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_a = create_test_project(&pool, user_id, Some("Project A")).await;
    let project_b = create_test_project(&pool, user_id, Some("Project B")).await;
    let task_id = create_test_task(&pool, project_a, user_id, Some("Task A")).await;

    let sprint_service = acpms_services::SprintService::new(pool.clone());
    let sprint_b = sprint_service
        .create_sprint(acpms_db::models::CreateSprintRequest {
            project_id: project_b,
            sequence: None,
            goal: None,
            name: "Sprint B".to_string(),
            description: None,
            start_date: None,
            end_date: None,
        })
        .await
        .expect("Failed to create sprint in project B");

    let request_body = json!({
        "sprint_id": sprint_b.id.to_string()
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        &format!("/api/v1/tasks/{}", task_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 500,
        "Expected 500 Internal Server Error, got {}: {}",
        status, body
    );
    let sprint_id_after: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT sprint_id FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_one(&pool)
            .await
            .expect("Failed to load task sprint assignment");
    assert!(
        sprint_id_after.is_none(),
        "Task sprint should remain unchanged on invalid cross-project update"
    );

    let _ = sqlx::query("DELETE FROM sprints WHERE project_id = $1")
        .bind(project_b)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM tasks WHERE project_id = $1")
        .bind(project_b)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM project_members WHERE project_id = $1")
        .bind(project_b)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(project_b)
        .execute(&pool)
        .await;
    cleanup_test_data(&pool, user_id, Some(project_a)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_update_task_rejects_invalid_status_transition() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;

    // todo -> in_review is invalid in TaskService::validate_status_transition
    let request_body = json!({
        "status": "in_review"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        &format!("/api/v1/tasks/{}", task_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 500,
        "Expected 500 Internal Server Error, got {}: {}",
        status, body
    );
    assert!(
        body.to_lowercase().contains("invalid status transition"),
        "Expected invalid-transition error, got: {}",
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_update_task_status_rejects_invalid_transition() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;

    // todo -> in_review is invalid transition
    let request_body = json!({
        "status": "in_review"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        &format!("/api/v1/tasks/{}/status", task_id),
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
        body.to_lowercase().contains("invalid status transition"),
        "Expected invalid-transition error, got: {}",
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_delete_task_nullifies_child_parent_reference() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let parent_id = create_test_task(&pool, project_id, user_id, Some("Parent Task")).await;
    let child_id = uuid::Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO tasks (id, project_id, title, description, task_type, status, created_by, parent_task_id, metadata)
        VALUES ($1, $2, $3, $4, 'feature', 'todo', $5, $6, '{}'::jsonb)
        "#,
    )
    .bind(child_id)
    .bind(project_id)
    .bind("Child Task")
    .bind("Child task description")
    .bind(user_id)
    .bind(parent_id)
    .execute(&pool)
    .await
    .expect("Failed to create child task");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "DELETE",
        &format!("/api/v1/tasks/{}", parent_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let child_parent_id: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT parent_task_id FROM tasks WHERE id = $1")
            .bind(child_id)
            .fetch_one(&pool)
            .await
            .expect("Failed to load child task parent reference");
    assert!(
        child_parent_id.is_none(),
        "Expected child parent_task_id to be null after parent deletion"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_task_children_returns_expected_children() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let parent_id = create_test_task(&pool, project_id, user_id, Some("Parent Task")).await;
    let child_id = uuid::Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO tasks (id, project_id, title, description, task_type, status, created_by, parent_task_id, metadata)
        VALUES ($1, $2, $3, $4, 'feature', 'todo', $5, $6, '{}'::jsonb)
        "#,
    )
    .bind(child_id)
    .bind(project_id)
    .bind("Child Task")
    .bind("Child task description")
    .bind(user_id)
    .bind(parent_id)
    .execute(&pool)
    .await
    .expect("Failed to create child task");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/tasks/{}/children", parent_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);
    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    let ids: Vec<String> = response["data"]
        .as_array()
        .expect("data should be an array")
        .iter()
        .filter_map(|item| item["id"].as_str().map(ToString::to_string))
        .collect();

    assert!(
        ids.contains(&child_id.to_string()),
        "Expected child task to be returned, got ids: {:?}",
        ids
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_update_task_metadata_endpoint() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;

    let request_body = json!({
        "metadata": {
            "priority": "critical",
            "estimate": 8
        }
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "PUT",
        &format!("/api/v1/tasks/{}/metadata", task_id),
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
    assert_eq!(
        response["data"]["metadata"]["priority"]
            .as_str()
            .unwrap_or_default(),
        "critical"
    );
    assert_eq!(
        response["data"]["metadata"]["estimate"]
            .as_i64()
            .unwrap_or_default(),
        8
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_list_tasks_filters_by_sprint_id() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_in_sprint_a =
        create_test_task(&pool, project_id, user_id, Some("Task Sprint A")).await;
    let task_in_sprint_b =
        create_test_task(&pool, project_id, user_id, Some("Task Sprint B")).await;

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

    sqlx::query("UPDATE tasks SET sprint_id = $2 WHERE id = $1")
        .bind(task_in_sprint_a)
        .bind(sprint_a.id)
        .execute(&pool)
        .await
        .expect("Failed to assign task to sprint A");
    sqlx::query("UPDATE tasks SET sprint_id = $2 WHERE id = $1")
        .bind(task_in_sprint_b)
        .bind(sprint_b.id)
        .execute(&pool)
        .await
        .expect("Failed to assign task to sprint B");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!(
            "/api/v1/tasks?project_id={}&sprint_id={}",
            project_id, sprint_a.id
        ),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);
    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    let ids: Vec<String> = response["data"]
        .as_array()
        .expect("data should be array")
        .iter()
        .filter_map(|item| item["id"].as_str().map(ToString::to_string))
        .collect();

    assert!(
        ids.contains(&task_in_sprint_a.to_string()),
        "Expected sprint A task to be returned, got ids: {:?}",
        ids
    );
    assert!(
        !ids.contains(&task_in_sprint_b.to_string()),
        "Expected sprint B task to be filtered out, got ids: {:?}",
        ids
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_requirement_status_auto_syncs_from_linked_task_status() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Requirement status sync")).await;

    let requirement = acpms_services::RequirementService::new(pool.clone())
        .create_requirement(
            user_id,
            acpms_db::models::CreateRequirementRequest {
                project_id,
                title: "Requirement sync".to_string(),
                content: "Status should follow linked task progress".to_string(),
                sprint_id: None,
                priority: Some(acpms_db::models::RequirementPriority::Medium),
                due_date: None,
                metadata: None,
            },
        )
        .await
        .expect("failed to create requirement");

    let create_task_body = json!({
        "project_id": project_id.to_string(),
        "requirement_id": requirement.id.to_string(),
        "title": "Linked task",
        "description": "Task connected to requirement",
        "task_type": "Feature"
    });

    let (create_status, create_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "POST",
            "/api/v1/tasks",
            Some(&create_task_body.to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;
    assert_eq!(create_status, 201, "create task failed: {}", create_resp);
    let create_json: serde_json::Value =
        serde_json::from_str(&create_resp).expect("invalid create response json");
    let task_id = create_json["data"]["id"]
        .as_str()
        .expect("missing task id")
        .to_string();

    let update_to_progress = json!({ "status": "in_progress" });
    let (progress_status, progress_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "PUT",
            &format!("/api/v1/tasks/{}/status", task_id),
            Some(&update_to_progress.to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;
    assert_eq!(
        progress_status, 200,
        "update to in_progress failed: {}",
        progress_resp
    );

    let requirement_status_after_progress: String =
        sqlx::query_scalar("SELECT status::text FROM requirements WHERE id = $1")
            .bind(requirement.id)
            .fetch_one(&pool)
            .await
            .expect("failed to fetch requirement status after in_progress");
    assert_eq!(
        requirement_status_after_progress, "in_progress",
        "Expected requirement status to become in_progress when linked task starts"
    );

    let update_to_done = json!({ "status": "done" });
    let (done_status, done_resp): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            &router,
            "PUT",
            &format!("/api/v1/tasks/{}/status", task_id),
            Some(&update_to_done.to_string()),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(&token),
            ],
        )
        .await;
    assert_eq!(done_status, 200, "update to done failed: {}", done_resp);

    let requirement_status_after_done: String =
        sqlx::query_scalar("SELECT status::text FROM requirements WHERE id = $1")
            .bind(requirement.id)
            .fetch_one(&pool)
            .await
            .expect("failed to fetch requirement status after done");
    assert_eq!(
        requirement_status_after_done, "done",
        "Expected requirement status to become done when all linked tasks are done"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
// End test module
