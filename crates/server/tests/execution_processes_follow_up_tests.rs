//! Execution process follow-up API tests

#[path = "helpers.rs"]
mod helpers;
use helpers::*;

use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires test database"]
async fn test_follow_up_execution_process_carries_source_context_to_new_process() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let project_id = create_test_project(&pool, user_id, Some("Follow-up Process Project")).await;
    let task_id =
        create_test_task(&pool, project_id, user_id, Some("Follow-up Process Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    let source_process_id = Uuid::new_v4();
    let source_worktree = format!("/tmp/worktree-{}", source_process_id);
    let source_branch = format!("feature/{source_process_id}");

    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name)
        VALUES ($1, $2, NULL, $3, $4)
        "#,
    )
    .bind(source_process_id)
    .bind(attempt_id)
    .bind(&source_worktree)
    .bind(&source_branch)
    .execute(&pool)
    .await
    .expect("failed to seed source execution process");

    let request_body = json!({
        "prompt": "continue with follow-up tasks"
    })
    .to_string();

    let path = format!("/api/v1/execution-processes/{source_process_id}/follow-up");
    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &path,
        Some(&request_body),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "unexpected status {status}: {body}");

    let rows: Vec<(Uuid, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT id, worktree_path, branch_name
        FROM execution_processes
        WHERE attempt_id = $1
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .bind(attempt_id)
    .fetch_all(&pool)
    .await
    .expect("failed to load execution processes after follow-up");

    assert_eq!(rows.len(), 2, "expected source+follow-up process rows");

    let follow_up_row = rows
        .last()
        .expect("expected follow-up execution process row");
    assert_ne!(follow_up_row.0, source_process_id);
    assert_eq!(follow_up_row.1.as_deref(), Some(source_worktree.as_str()));
    assert_eq!(follow_up_row.2.as_deref(), Some(source_branch.as_str()));

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_follow_up_execution_process_multi_turn_creates_distinct_process_chain() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let project_id =
        create_test_project(&pool, user_id, Some("Follow-up Multi Turn Project")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("Follow-up Multi Turn Task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    let source_process_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name)
        VALUES ($1, $2, NULL, $3, $4)
        "#,
    )
    .bind(source_process_id)
    .bind(attempt_id)
    .bind(format!("/tmp/worktree-{}", source_process_id))
    .bind(format!("feature/{source_process_id}"))
    .execute(&pool)
    .await
    .expect("failed to seed source process");

    let request_headers = vec![
        ("content-type", "application/json".to_string()),
        auth_header_bearer(&token),
    ];

    let first_path = format!("/api/v1/execution-processes/{source_process_id}/follow-up");
    let (first_status, first_body) = make_request_with_string_headers(
        &router,
        "POST",
        &first_path,
        Some(&json!({ "prompt": "follow-up #1" }).to_string()),
        request_headers.clone(),
    )
    .await;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "first follow-up failed: {first_body}"
    );

    let first_follow_up_process_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT id
        FROM execution_processes
        WHERE attempt_id = $1 AND id <> $2
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(attempt_id)
    .bind(source_process_id)
    .fetch_one(&pool)
    .await
    .expect("failed to resolve first follow-up process id");

    // Simulate completion between turns so second follow-up is allowed.
    sqlx::query(
        "UPDATE task_attempts SET status = 'success'::attempt_status, completed_at = NOW() WHERE id = $1",
    )
    .bind(attempt_id)
    .execute(&pool)
    .await
    .expect("failed to mark attempt completed between follow-up turns");

    let second_path = format!("/api/v1/execution-processes/{first_follow_up_process_id}/follow-up");
    let (second_status, second_body) = make_request_with_string_headers(
        &router,
        "POST",
        &second_path,
        Some(&json!({ "prompt": "follow-up #2" }).to_string()),
        request_headers,
    )
    .await;
    assert_eq!(
        second_status,
        StatusCode::OK,
        "second follow-up failed: {second_body}"
    );

    let process_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM execution_processes
        WHERE attempt_id = $1
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .bind(attempt_id)
    .fetch_all(&pool)
    .await
    .expect("failed to load process chain");

    assert_eq!(
        process_ids.len(),
        3,
        "expected exactly 3 processes in chain (source + 2 follow-ups)"
    );
    assert_eq!(process_ids[0], source_process_id);
    assert_ne!(process_ids[1], process_ids[0]);
    assert_ne!(process_ids[2], process_ids[1]);
    assert_ne!(process_ids[2], process_ids[0]);

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_follow_up_execution_process_creates_new_attempt_when_task_is_done() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let project_id = create_test_project(&pool, user_id, Some("Done Follow-up Project")).await;
    let task_id = create_test_task(&pool, project_id, user_id, Some("Done Follow-up Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    sqlx::query("UPDATE tasks SET status = 'done'::task_status WHERE id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .expect("failed to mark task done");

    let source_process_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name)
        VALUES ($1, $2, NULL, $3, $4)
        "#,
    )
    .bind(source_process_id)
    .bind(attempt_id)
    .bind(format!("/tmp/worktree-{}", source_process_id))
    .bind(format!("feat/attempt-{}", attempt_id))
    .execute(&pool)
    .await
    .expect("failed to seed source process");

    let path = format!("/api/v1/execution-processes/{source_process_id}/follow-up");
    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &path,
        Some(&json!({ "prompt": "continue after merge" }).to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "unexpected status {status}: {body}");

    let payload: serde_json::Value =
        serde_json::from_str(&body).expect("response body should be valid json");
    let new_attempt_id = payload["data"]["id"]
        .as_str()
        .expect("response should include new attempt id");
    assert_ne!(new_attempt_id, attempt_id.to_string());

    let attempt_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM task_attempts WHERE task_id = $1")
            .bind(task_id)
            .fetch_one(&pool)
            .await
            .expect("failed to count attempts");
    assert_eq!(
        attempt_count, 2,
        "expected original + new follow-up attempt"
    );

    let linked_follow_up_attempt_id: Option<String> = sqlx::query_scalar(
        "SELECT metadata->>'follow_up_attempt_id' FROM task_attempts WHERE id = $1",
    )
    .bind(attempt_id)
    .fetch_one(&pool)
    .await
    .expect("failed to load follow-up linkage");
    assert_eq!(linked_follow_up_attempt_id.as_deref(), Some(new_attempt_id));

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
