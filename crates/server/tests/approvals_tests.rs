//! Approvals API Tests

#[path = "helpers.rs"]
mod helpers;
use helpers::*;

use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires test database"]
async fn test_approval_respond_concurrent_requests_single_winner() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id =
        create_test_project(&pool, user_id, Some("Approvals Concurrency Project")).await;
    let task_id = create_test_task(&pool, project_id, user_id, Some("Approvals Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let approval_id = Uuid::new_v4();
    let tool_use_id = format!("tool-use-{}", approval_id);
    sqlx::query(
        r#"
        INSERT INTO tool_approvals (id, attempt_id, tool_use_id, tool_name, tool_input, status)
        VALUES ($1, $2, $3, $4, $5, 'pending'::approval_status)
        "#,
    )
    .bind(approval_id)
    .bind(attempt_id)
    .bind(tool_use_id)
    .bind("Bash")
    .bind(json!({"command": "echo hello"}))
    .execute(&pool)
    .await
    .expect("failed to create pending approval");

    let body = json!({
        "decision": "approve"
    })
    .to_string();

    let path = format!("/api/v1/approvals/{}/respond", approval_id);
    let headers = vec![
        ("content-type", "application/json".to_string()),
        auth_header_bearer(&token),
    ];

    let request_one =
        make_request_with_string_headers(&router, "POST", &path, Some(&body), headers.clone());
    let request_two =
        make_request_with_string_headers(&router, "POST", &path, Some(&body), headers);

    let ((status_one, body_one), (status_two, body_two)) = tokio::join!(request_one, request_two);

    let statuses = [status_one, status_two];
    let ok_count = statuses
        .iter()
        .filter(|status| **status == StatusCode::OK)
        .count();
    let conflict_count = statuses
        .iter()
        .filter(|status| **status == StatusCode::CONFLICT)
        .count();

    assert_eq!(
        ok_count, 1,
        "Expected exactly one 200 OK, got statuses {:?}; bodies: [{}, {}]",
        statuses, body_one, body_two
    );
    assert_eq!(
        conflict_count, 1,
        "Expected exactly one 409 Conflict, got statuses {:?}; bodies: [{}, {}]",
        statuses, body_one, body_two
    );

    let row: (String, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT status::text, approved_by, responded_at FROM tool_approvals WHERE id = $1",
    )
    .bind(approval_id)
    .fetch_one(&pool)
    .await
    .expect("failed to fetch approval row after concurrent response");

    assert_eq!(row.0, "approved");
    assert_eq!(row.1, Some(user_id));
    assert!(row.2.is_some(), "expected responded_at to be set");

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_approval_respond_concurrent_decisions_multi_user_single_winner() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (owner_user_id, _) = create_test_user(&pool, None, None, None).await;
    let (reviewer_user_id, _) = create_test_user(&pool, None, None, None).await;
    let owner_token = generate_test_token(owner_user_id);
    let reviewer_token = generate_test_token(reviewer_user_id);

    let project_id =
        create_test_project(&pool, owner_user_id, Some("Approvals Multi User Project")).await;
    sqlx::query(
        r#"
        INSERT INTO project_members (project_id, user_id, roles)
        VALUES ($1, $2, ARRAY['owner']::project_role[])
        "#,
    )
    .bind(project_id)
    .bind(reviewer_user_id)
    .execute(&pool)
    .await
    .expect("failed to add reviewer user to project");

    let task_id = create_test_task(
        &pool,
        project_id,
        owner_user_id,
        Some("Approvals Multi User Task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let approval_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO tool_approvals (id, attempt_id, tool_use_id, tool_name, tool_input, status)
        VALUES ($1, $2, $3, $4, $5, 'pending'::approval_status)
        "#,
    )
    .bind(approval_id)
    .bind(attempt_id)
    .bind(format!("tool-use-{}", approval_id))
    .bind("Bash")
    .bind(json!({"command": "npm run test"}))
    .execute(&pool)
    .await
    .expect("failed to create pending approval");

    let path = format!("/api/v1/approvals/{}/respond", approval_id);
    let approve_payload = json!({"decision": "approve"}).to_string();
    let deny_payload = json!({"decision": "deny", "reason": "needs more context"}).to_string();

    let approve_request = make_request_with_string_headers(
        &router,
        "POST",
        &path,
        Some(&approve_payload),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&owner_token),
        ],
    );
    let deny_request = make_request_with_string_headers(
        &router,
        "POST",
        &path,
        Some(&deny_payload),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&reviewer_token),
        ],
    );

    let ((approve_status, approve_body), (deny_status, deny_body)) =
        tokio::join!(approve_request, deny_request);

    let statuses = [approve_status, deny_status];
    let ok_count = statuses
        .iter()
        .filter(|status| **status == StatusCode::OK)
        .count();
    let conflict_count = statuses
        .iter()
        .filter(|status| **status == StatusCode::CONFLICT)
        .count();

    assert_eq!(
        ok_count, 1,
        "Expected exactly one 200 OK, got statuses {:?}; bodies: [{}, {}]",
        statuses, approve_body, deny_body
    );
    assert_eq!(
        conflict_count, 1,
        "Expected exactly one 409 Conflict, got statuses {:?}; bodies: [{}, {}]",
        statuses, approve_body, deny_body
    );

    let row: (String, Option<Uuid>, Option<String>) = sqlx::query_as(
        "SELECT status::text, approved_by, denied_reason FROM tool_approvals WHERE id = $1",
    )
    .bind(approval_id)
    .fetch_one(&pool)
    .await
    .expect("failed to fetch approval row after multi-user race");

    assert!(
        row.0 == "approved" || row.0 == "denied",
        "expected final status approved/denied, got {}",
        row.0
    );
    assert!(
        row.1 == Some(owner_user_id) || row.1 == Some(reviewer_user_id),
        "expected approved_by to match winner user, got {:?}",
        row.1
    );
    if row.0 == "denied" {
        assert_eq!(row.2.as_deref(), Some("needs more context"));
    }

    cleanup_test_data(&pool, owner_user_id, Some(project_id)).await;
    cleanup_test_data(&pool, reviewer_user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_approval_respond_concurrent_decisions_same_user_multi_tab_single_winner() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(
        &pool,
        user_id,
        Some("Approvals Same User Multi Tab Project"),
    )
    .await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("Approvals Same User Multi Tab Task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let approval_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO tool_approvals (id, attempt_id, tool_use_id, tool_name, tool_input, status)
        VALUES ($1, $2, $3, $4, $5, 'pending'::approval_status)
        "#,
    )
    .bind(approval_id)
    .bind(attempt_id)
    .bind(format!("tool-use-{}", approval_id))
    .bind("Bash")
    .bind(json!({"command": "npm run lint"}))
    .execute(&pool)
    .await
    .expect("failed to create pending approval");

    let path = format!("/api/v1/approvals/{}/respond", approval_id);
    let approve_payload = json!({"decision": "approve"}).to_string();
    let deny_payload = json!({"decision": "deny", "reason": "reject from other tab"}).to_string();

    let approve_request = make_request_with_string_headers(
        &router,
        "POST",
        &path,
        Some(&approve_payload),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    );
    let deny_request = make_request_with_string_headers(
        &router,
        "POST",
        &path,
        Some(&deny_payload),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    );

    let ((approve_status, approve_body), (deny_status, deny_body)) =
        tokio::join!(approve_request, deny_request);

    let statuses = [approve_status, deny_status];
    let ok_count = statuses
        .iter()
        .filter(|status| **status == StatusCode::OK)
        .count();
    let conflict_count = statuses
        .iter()
        .filter(|status| **status == StatusCode::CONFLICT)
        .count();

    assert_eq!(
        ok_count, 1,
        "Expected exactly one 200 OK, got statuses {:?}; bodies: [{}, {}]",
        statuses, approve_body, deny_body
    );
    assert_eq!(
        conflict_count, 1,
        "Expected exactly one 409 Conflict, got statuses {:?}; bodies: [{}, {}]",
        statuses, approve_body, deny_body
    );

    let row: (String, Option<Uuid>, Option<String>) = sqlx::query_as(
        "SELECT status::text, approved_by, denied_reason FROM tool_approvals WHERE id = $1",
    )
    .bind(approval_id)
    .fetch_one(&pool)
    .await
    .expect("failed to fetch approval row after same-user race");

    assert!(
        row.0 == "approved" || row.0 == "denied",
        "expected final status approved/denied, got {}",
        row.0
    );
    assert_eq!(
        row.1,
        Some(user_id),
        "expected approved_by to match single user in multi-tab race"
    );
    if row.0 == "denied" {
        assert_eq!(row.2.as_deref(), Some("reject from other tab"));
    }

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_pending_approvals_for_process_is_strictly_process_scoped() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Approvals Scope Project")).await;
    let task_id = create_test_task(&pool, project_id, user_id, Some("Approvals Scope Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let process_a = Uuid::new_v4();
    let process_b = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name)
        VALUES
          ($1, $3, NULL, '/tmp/a', 'branch-a'),
          ($2, $3, NULL, '/tmp/b', 'branch-b')
        "#,
    )
    .bind(process_a)
    .bind(process_b)
    .bind(attempt_id)
    .execute(&pool)
    .await
    .expect("failed to seed execution processes");

    let scoped_id = Uuid::new_v4();
    let other_scoped_id = Uuid::new_v4();
    let legacy_null_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO tool_approvals (id, attempt_id, execution_process_id, tool_use_id, tool_name, tool_input, status)
        VALUES
          ($1, $4, $5, $6, 'Bash', '{"command":"echo a"}'::jsonb, 'pending'::approval_status),
          ($2, $4, $7, $8, 'Bash', '{"command":"echo b"}'::jsonb, 'pending'::approval_status),
          ($3, $4, NULL, $9, 'Bash', '{"command":"echo legacy"}'::jsonb, 'pending'::approval_status)
        "#,
    )
    .bind(scoped_id)
    .bind(other_scoped_id)
    .bind(legacy_null_id)
    .bind(attempt_id)
    .bind(process_a)
    .bind(format!("tool-use-{}", scoped_id))
    .bind(process_b)
    .bind(format!("tool-use-{}", other_scoped_id))
    .bind(format!("tool-use-{}", legacy_null_id))
    .execute(&pool)
    .await
    .expect("failed to seed tool approvals");

    let path = format!("/api/v1/execution-processes/{process_a}/approvals/pending");
    let (status, body) = make_request_with_string_headers(
        &router,
        "GET",
        &path,
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "unexpected status {status}: {body}");

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("failed to parse json body");
    let approvals = response
        .get("data")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    assert_eq!(
        approvals.len(),
        1,
        "expected exactly one process-scoped approval, got body: {}",
        body
    );
    let returned_id = approvals[0]
        .get("id")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    assert_eq!(returned_id, scoped_id.to_string());

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_process_pending_approvals_reflect_response_lifecycle() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id =
        create_test_project(&pool, user_id, Some("Approvals Process Lifecycle Project")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("Approvals Process Lifecycle Task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let process_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name)
        VALUES ($1, $2, NULL, '/tmp/process-lifecycle', 'process-lifecycle-branch')
        "#,
    )
    .bind(process_id)
    .bind(attempt_id)
    .execute(&pool)
    .await
    .expect("failed to seed execution process");

    let approval_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO tool_approvals (id, attempt_id, execution_process_id, tool_use_id, tool_name, tool_input, status)
        VALUES ($1, $2, $3, $4, 'Bash', '{"command":"echo lifecycle"}'::jsonb, 'pending'::approval_status)
        "#,
    )
    .bind(approval_id)
    .bind(attempt_id)
    .bind(process_id)
    .bind(format!("tool-use-{}", approval_id))
    .execute(&pool)
    .await
    .expect("failed to seed pending approval");

    let pending_path = format!("/api/v1/execution-processes/{process_id}/approvals/pending");
    let (before_status, before_body) = make_request_with_string_headers(
        &router,
        "GET",
        &pending_path,
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;
    assert_eq!(
        before_status,
        StatusCode::OK,
        "unexpected status before approval response: {before_body}"
    );
    let before_json: serde_json::Value =
        serde_json::from_str(&before_body).expect("valid pending approvals json before response");
    let before_items = before_json
        .get("data")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        before_items.len(),
        1,
        "expected one pending approval before response, body: {before_body}"
    );

    let respond_path = format!("/api/v1/approvals/{approval_id}/respond");
    let (respond_status, respond_body) = make_request_with_string_headers(
        &router,
        "POST",
        &respond_path,
        Some(&json!({ "decision": "approve" }).to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;
    assert_eq!(
        respond_status,
        StatusCode::OK,
        "unexpected respond status: {respond_body}"
    );

    let (after_status, after_body) = make_request_with_string_headers(
        &router,
        "GET",
        &pending_path,
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;
    assert_eq!(
        after_status,
        StatusCode::OK,
        "unexpected status after approval response: {after_body}"
    );
    let after_json: serde_json::Value =
        serde_json::from_str(&after_body).expect("valid pending approvals json after response");
    let after_items = after_json
        .get("data")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        after_items.is_empty(),
        "expected no pending approvals after response, body: {after_body}"
    );

    let row: (String, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT status::text, approved_by, responded_at FROM tool_approvals WHERE id = $1",
    )
    .bind(approval_id)
    .fetch_one(&pool)
    .await
    .expect("failed to fetch approval row");
    assert_eq!(row.0, "approved");
    assert_eq!(row.1, Some(user_id));
    assert!(
        row.2.is_some(),
        "expected responded_at after approval response"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_respond_to_approval_supports_legacy_tool_use_id_with_deny_reason() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id =
        create_test_project(&pool, user_id, Some("Approvals Legacy Ref Project")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("Approvals Legacy Ref Task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let approval_id = Uuid::new_v4();
    let legacy_tool_use_id = format!("legacy-tool-use-{}", approval_id);

    sqlx::query(
        r#"
        INSERT INTO tool_approvals (id, attempt_id, tool_use_id, tool_name, tool_input, status)
        VALUES ($1, $2, $3, $4, $5, 'pending'::approval_status)
        "#,
    )
    .bind(approval_id)
    .bind(attempt_id)
    .bind(&legacy_tool_use_id)
    .bind("Bash")
    .bind(json!({"command": "rm -rf /tmp/demo"}))
    .execute(&pool)
    .await
    .expect("failed to seed approval");

    let path = format!("/api/v1/approvals/{}/respond", legacy_tool_use_id);
    let payload = json!({
        "decision": "deny",
        "reason": "command is unsafe"
    })
    .to_string();

    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &path,
        Some(&payload),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "expected deny via legacy ref to succeed, body: {}",
        body
    );

    let row: (String, Option<Uuid>, Option<String>, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as(
            "SELECT status::text, approved_by, denied_reason, responded_at FROM tool_approvals WHERE id = $1",
        )
        .bind(approval_id)
        .fetch_one(&pool)
        .await
        .expect("failed to fetch approval");

    assert_eq!(row.0, "denied");
    assert_eq!(row.1, Some(user_id));
    assert_eq!(row.2.as_deref(), Some("command is unsafe"));
    assert!(row.3.is_some(), "expected responded_at to be set");

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_pending_approvals_for_process_forbidden_for_non_member() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (owner_user_id, _) = create_test_user(&pool, None, None, None).await;
    let (outsider_user_id, _) = create_test_user(&pool, None, None, None).await;
    let outsider_token = generate_test_token(outsider_user_id);

    let project_id =
        create_test_project(&pool, owner_user_id, Some("Approvals Visibility Project")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        owner_user_id,
        Some("Approvals Visibility Task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;
    let process_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name)
        VALUES ($1, $2, NULL, '/tmp/visibility', 'visibility-branch')
        "#,
    )
    .bind(process_id)
    .bind(attempt_id)
    .execute(&pool)
    .await
    .expect("failed to seed execution process");

    sqlx::query(
        r#"
        INSERT INTO tool_approvals (id, attempt_id, execution_process_id, tool_use_id, tool_name, tool_input, status)
        VALUES ($1, $2, $3, $4, 'Bash', '{"command":"echo hidden"}'::jsonb, 'pending'::approval_status)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(attempt_id)
    .bind(process_id)
    .bind(format!("tool-use-{}", Uuid::new_v4()))
    .execute(&pool)
    .await
    .expect("failed to seed scoped approval");

    let path = format!(
        "/api/v1/execution-processes/{}/approvals/pending",
        process_id
    );
    let (status, body) = make_request_with_string_headers(
        &router,
        "GET",
        &path,
        None,
        vec![auth_header_bearer(&outsider_token)],
    )
    .await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "expected non-member to be forbidden, body: {}",
        body
    );

    cleanup_test_data(&pool, owner_user_id, Some(project_id)).await;
    cleanup_test_data(&pool, outsider_user_id, None).await;
}
