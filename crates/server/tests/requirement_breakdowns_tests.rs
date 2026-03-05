//! Requirement Breakdown API Tests
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

use serde_json::json;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires test database"]
async fn test_breakdown_start_requires_permission() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (owner_id, _) = create_test_user(&pool, None, None, None).await;
    let owner_token = generate_test_token(owner_id);
    let project_id = create_test_project(&pool, owner_id, Some("Breakdown RBAC")).await;

    let (viewer_id, _) = create_test_user(&pool, None, None, None).await;
    let viewer_token = generate_test_token(viewer_id);
    sqlx::query(
        "INSERT INTO project_members (project_id, user_id, roles) VALUES ($1, $2, ARRAY['viewer']::project_role[])",
    )
    .bind(project_id)
    .bind(viewer_id)
    .execute(&pool)
    .await
    .expect("failed to insert viewer member");

    use acpms_services::RequirementService;
    let req = RequirementService::new(pool.clone())
        .create_requirement(
            owner_id,
            acpms_db::models::CreateRequirementRequest {
                project_id,
                title: "Need breakdown".to_string(),
                content: "Break this requirement into tasks".to_string(),
                sprint_id: None,
                priority: Some(acpms_db::models::RequirementPriority::Medium),
                due_date: None,
                metadata: None,
            },
        )
        .await
        .expect("failed to create requirement");

    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/requirements/{}/breakdown/start",
            project_id, req.id
        ),
        Some("{}"),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&viewer_token),
        ],
    )
    .await;

    assert_eq!(
        status,
        axum::http::StatusCode::FORBIDDEN,
        "Expected 403 for viewer, got {}: {}",
        status,
        body
    );

    let _ = owner_token;
    cleanup_test_data(&pool, owner_id, Some(project_id)).await;
    cleanup_test_data(&pool, viewer_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_breakdown_confirm_rejects_sprint_from_other_project() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Project A")).await;
    let other_project_id = create_test_project(&pool, user_id, Some("Project B")).await;

    use acpms_services::RequirementService;
    let requirement = RequirementService::new(pool.clone())
        .create_requirement(
            user_id,
            acpms_db::models::CreateRequirementRequest {
                project_id,
                title: "Requirement A".to_string(),
                content: "Need backend and frontend updates".to_string(),
                sprint_id: None,
                priority: Some(acpms_db::models::RequirementPriority::High),
                due_date: None,
                metadata: None,
            },
        )
        .await
        .expect("failed to create requirement");

    let foreign_sprint_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO sprints (id, project_id, sequence, name, status) VALUES ($1, $2, $3, $4, 'planning')",
    )
    .bind(foreign_sprint_id)
    .bind(other_project_id)
    .bind(1_i32)
    .bind("Other sprint")
    .execute(&pool)
    .await
    .expect("failed to insert foreign sprint");

    let session_id = Uuid::new_v4();
    let proposed_tasks = json!([
        {
            "title": "[Breakdown] Requirement A",
            "description": "Analysis task",
            "task_type": "spike",
            "kind": "analysis_session"
        },
        {
            "title": "Implement Requirement A",
            "description": "Implementation",
            "task_type": "feature",
            "kind": "implementation"
        },
        {
            "title": "Test Requirement A",
            "description": "Tests",
            "task_type": "test",
            "kind": "implementation"
        }
    ]);

    sqlx::query(
        r#"
        INSERT INTO requirement_breakdown_sessions (
            id, project_id, requirement_id, created_by, status,
            analysis, impact, plan, proposed_tasks
        )
        VALUES ($1, $2, $3, $4, 'review', $5, $6, $7, $8)
        "#,
    )
    .bind(session_id)
    .bind(project_id)
    .bind(requirement.id)
    .bind(user_id)
    .bind(json!({"summary":"analysis"}))
    .bind(json!([{"area":"backend","impact":"x","risk":"medium","mitigation":"y"}]))
    .bind(json!({"summary":"plan","steps":["a","b"]}))
    .bind(proposed_tasks)
    .execute(&pool)
    .await
    .expect("failed to insert breakdown session");

    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/requirements/{}/breakdown/{}/confirm",
            project_id, requirement.id, session_id
        ),
        Some(
            &json!({
                "assignment_mode": "selected",
                "sprint_id": foreign_sprint_id
            })
            .to_string(),
        ),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status,
        axum::http::StatusCode::BAD_REQUEST,
        "Expected 400 for foreign sprint, got {}: {}",
        status,
        body
    );
    assert!(
        body.contains("does not belong"),
        "Expected sprint ownership error, got {}",
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
    delete_test_project(&pool, other_project_id).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_breakdown_confirm_creates_todo_tasks_without_attempts() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Breakdown Confirm")).await;

    use acpms_services::RequirementService;
    let requirement = RequirementService::new(pool.clone())
        .create_requirement(
            user_id,
            acpms_db::models::CreateRequirementRequest {
                project_id,
                title: "Requirement Confirm".to_string(),
                content: "Need stable breakdown confirm flow".to_string(),
                sprint_id: None,
                priority: Some(acpms_db::models::RequirementPriority::Medium),
                due_date: None,
                metadata: None,
            },
        )
        .await
        .expect("failed to create requirement");

    let session_id = Uuid::new_v4();
    let proposed_tasks = json!([
        {
            "title": "[Breakdown] Requirement Confirm",
            "description": "Analysis session",
            "task_type": "spike",
            "kind": "analysis_session"
        },
        {
            "title": "Implement Requirement Confirm",
            "description": "Core feature",
            "task_type": "feature",
            "kind": "implementation"
        },
        {
            "title": "Test Requirement Confirm",
            "description": "Automated tests",
            "task_type": "test",
            "kind": "implementation"
        }
    ]);

    sqlx::query(
        r#"
        INSERT INTO requirement_breakdown_sessions (
            id, project_id, requirement_id, created_by, status,
            analysis, impact, plan, proposed_tasks
        )
        VALUES ($1, $2, $3, $4, 'review', $5, $6, $7, $8)
        "#,
    )
    .bind(session_id)
    .bind(project_id)
    .bind(requirement.id)
    .bind(user_id)
    .bind(json!({"summary":"analysis"}))
    .bind(json!([{"area":"backend","impact":"x","risk":"medium","mitigation":"y"}]))
    .bind(json!({"summary":"plan","steps":["a","b"]}))
    .bind(proposed_tasks)
    .execute(&pool)
    .await
    .expect("failed to insert breakdown session");

    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/requirements/{}/breakdown/{}/confirm",
            project_id, requirement.id, session_id
        ),
        Some(
            &json!({
                "assignment_mode": "backlog"
            })
            .to_string(),
        ),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "Expected 200 for confirm, got {}: {}",
        status,
        body
    );

    let response: serde_json::Value = serde_json::from_str(&body).expect("invalid JSON response");
    let tasks = response["data"]["tasks"]
        .as_array()
        .expect("tasks should be array");
    assert!(!tasks.is_empty(), "Expected created tasks");
    for task in tasks {
        assert_eq!(
            task["status"].as_str().unwrap_or_default(),
            "todo",
            "Expected task status=todo"
        );
    }

    let attempts_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM task_attempts ta
        INNER JOIN tasks t ON t.id = ta.task_id
        WHERE t.project_id = $1 AND t.requirement_id = $2
        "#,
    )
    .bind(project_id)
    .bind(requirement.id)
    .fetch_one(&pool)
    .await
    .expect("failed to count task attempts");
    assert_eq!(
        attempts_count, 0,
        "Breakdown confirm should not auto-create task attempts"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_breakdown_manual_confirm_creates_todo_tasks_without_attempts() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Breakdown Manual Confirm")).await;

    use acpms_services::RequirementService;
    let requirement = RequirementService::new(pool.clone())
        .create_requirement(
            user_id,
            acpms_db::models::CreateRequirementRequest {
                project_id,
                title: "Requirement Manual Confirm".to_string(),
                content: "Need manual breakdown flow".to_string(),
                sprint_id: None,
                priority: Some(acpms_db::models::RequirementPriority::Medium),
                due_date: None,
                metadata: None,
            },
        )
        .await
        .expect("failed to create requirement");

    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{}/requirements/{}/breakdown/manual/confirm",
            project_id, requirement.id
        ),
        Some(
            &json!({
                "assignment_mode": "backlog",
                "tasks": [
                    {
                        "title": "Plan scope and assumptions",
                        "description": "Prepare implementation scope notes",
                        "task_type": "spike",
                        "kind": "analysis_session"
                    },
                    {
                        "title": "Implement API flow",
                        "description": "Add endpoint and validation",
                        "task_type": "feature",
                        "kind": "implementation"
                    },
                    {
                        "title": "Add regression tests",
                        "description": "Cover critical paths",
                        "task_type": "test",
                        "kind": "implementation"
                    }
                ]
            })
            .to_string(),
        ),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "Expected 200 for manual confirm, got {}: {}",
        status,
        body
    );

    let response: serde_json::Value = serde_json::from_str(&body).expect("invalid JSON response");
    let tasks = response["data"]["tasks"]
        .as_array()
        .expect("tasks should be array");
    assert_eq!(tasks.len(), 3, "Expected exactly 3 created tasks");
    for task in tasks {
        assert_eq!(
            task["status"].as_str().unwrap_or_default(),
            "todo",
            "Expected task status=todo"
        );
    }

    let attempts_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM task_attempts ta
        INNER JOIN tasks t ON t.id = ta.task_id
        WHERE t.project_id = $1 AND t.requirement_id = $2
        "#,
    )
    .bind(project_id)
    .bind(requirement.id)
    .fetch_one(&pool)
    .await
    .expect("failed to count task attempts");
    assert_eq!(
        attempts_count, 0,
        "Manual breakdown confirm should not auto-create task attempts"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
