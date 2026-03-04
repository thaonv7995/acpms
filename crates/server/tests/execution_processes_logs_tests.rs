//! Execution process logs API tests

#[path = "helpers.rs"]
mod helpers;
use helpers::*;

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires test database"]
async fn test_execution_process_logs_filter_by_window_and_log_type() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let project_id = create_test_project(&pool, user_id, Some("Process Logs Window Project")).await;
    let task_id =
        create_test_task(&pool, project_id, user_id, Some("Process Logs Window Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let now = Utc::now();
    let process_a_created_at = now - Duration::minutes(2);
    let process_b_created_at = now - Duration::minutes(1);

    let process_a = Uuid::new_v4();
    let process_b = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name, created_at)
        VALUES
          ($1, $3, NULL, '/tmp/a', 'branch-a', $4),
          ($2, $3, NULL, '/tmp/b', 'branch-b', $5)
        "#,
    )
    .bind(process_a)
    .bind(process_b)
    .bind(attempt_id)
    .bind(process_a_created_at)
    .bind(process_b_created_at)
    .execute(&pool)
    .await
    .expect("failed to seed execution processes");

    // Logs that belong to process A window [process_a.created_at, process_b.created_at)
    sqlx::query(
        r#"
        INSERT INTO agent_logs (id, attempt_id, log_type, content, created_at)
        VALUES
          ($1, $5, 'stdout', 'raw-a-stdout', $6),
          ($2, $5, 'stderr', 'raw-a-stderr', $7),
          ($3, $5, 'normalized', 'normalized-a', $8),
          ($4, $5, 'stdout', 'raw-b-stdout', $9)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(attempt_id)
    .bind(process_a_created_at + Duration::seconds(5))
    .bind(process_a_created_at + Duration::seconds(10))
    .bind(process_a_created_at + Duration::seconds(15))
    .bind(process_b_created_at + Duration::seconds(5))
    .execute(&pool)
    .await
    .expect("failed to seed agent logs");

    // Raw logs for process A should include only stdout/stderr before process B started.
    let raw_path = format!("/api/v1/execution-processes/{}/raw-logs", process_a);
    let (raw_status, raw_body) = make_request_with_string_headers(
        &router,
        "GET",
        &raw_path,
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(
        raw_status,
        StatusCode::OK,
        "unexpected raw status {raw_status}: {raw_body}"
    );

    let raw_response: serde_json::Value =
        serde_json::from_str(&raw_body).expect("failed to parse raw response");
    let raw_logs = raw_response
        .get("data")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let raw_messages: Vec<String> = raw_logs
        .iter()
        .filter_map(|log| {
            log.get("message")
                .and_then(|value| value.as_str())
                .map(ToString::to_string)
        })
        .collect();
    assert_eq!(raw_messages, vec!["raw-a-stdout", "raw-a-stderr"]);

    // Normalized logs for process A should include only normalized entries in process A window.
    let normalized_path = format!("/api/v1/execution-processes/{}/normalized-logs", process_a);
    let (normalized_status, normalized_body) = make_request_with_string_headers(
        &router,
        "GET",
        &normalized_path,
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(
        normalized_status,
        StatusCode::OK,
        "unexpected normalized status {normalized_status}: {normalized_body}"
    );

    let normalized_response: serde_json::Value =
        serde_json::from_str(&normalized_body).expect("failed to parse normalized response");
    let normalized_logs = normalized_response
        .get("data")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let normalized_messages: Vec<String> = normalized_logs
        .iter()
        .filter_map(|log| {
            log.get("message")
                .and_then(|value| value.as_str())
                .map(ToString::to_string)
        })
        .collect();
    assert_eq!(normalized_messages, vec!["normalized-a"]);

    // Process B raw logs should include only events after process B start.
    let raw_b_path = format!("/api/v1/execution-processes/{}/raw-logs", process_b);
    let (raw_b_status, raw_b_body) = make_request_with_string_headers(
        &router,
        "GET",
        &raw_b_path,
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;
    assert_eq!(
        raw_b_status,
        StatusCode::OK,
        "unexpected process B raw status {raw_b_status}: {raw_b_body}"
    );

    let raw_b_response: serde_json::Value =
        serde_json::from_str(&raw_b_body).expect("failed to parse process B raw response");
    let raw_b_logs = raw_b_response
        .get("data")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let raw_b_messages: Vec<String> = raw_b_logs
        .iter()
        .filter_map(|log| {
            log.get("message")
                .and_then(|value| value.as_str())
                .map(ToString::to_string)
        })
        .collect();
    assert_eq!(raw_b_messages, vec!["raw-b-stdout"]);

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_execution_process_logs_forbidden_for_non_member() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (owner_user_id, _) = create_test_user(&pool, None, None, None).await;
    let (outsider_user_id, _) = create_test_user(&pool, None, None, None).await;
    let outsider_token = generate_test_token(outsider_user_id);

    let project_id = create_test_project(
        &pool,
        owner_user_id,
        Some("Process Logs Visibility Project"),
    )
    .await;
    let task_id = create_test_task(
        &pool,
        project_id,
        owner_user_id,
        Some("Process Logs Visibility Task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let process_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name)
        VALUES ($1, $2, NULL, '/tmp/forbidden', 'forbidden-branch')
        "#,
    )
    .bind(process_id)
    .bind(attempt_id)
    .execute(&pool)
    .await
    .expect("failed to seed process");

    let raw_path = format!("/api/v1/execution-processes/{}/raw-logs", process_id);
    let (raw_status, raw_body) = make_request_with_string_headers(
        &router,
        "GET",
        &raw_path,
        None,
        vec![auth_header_bearer(&outsider_token)],
    )
    .await;
    assert_eq!(
        raw_status,
        StatusCode::FORBIDDEN,
        "expected forbidden for outsider raw logs, body: {}",
        raw_body
    );

    let normalized_path = format!("/api/v1/execution-processes/{}/normalized-logs", process_id);
    let (normalized_status, normalized_body) = make_request_with_string_headers(
        &router,
        "GET",
        &normalized_path,
        None,
        vec![auth_header_bearer(&outsider_token)],
    )
    .await;
    assert_eq!(
        normalized_status,
        StatusCode::FORBIDDEN,
        "expected forbidden for outsider normalized logs, body: {}",
        normalized_body
    );

    cleanup_test_data(&pool, owner_user_id, Some(project_id)).await;
    cleanup_test_data(&pool, outsider_user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_execution_process_logs_order_is_deterministic_for_same_timestamp() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let project_id = create_test_project(
        &pool,
        user_id,
        Some("Process Logs Deterministic Order Project"),
    )
    .await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("Process Logs Deterministic Order Task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let now = Utc::now();
    let process_a_created_at = now - Duration::minutes(2);
    let process_b_created_at = now - Duration::minutes(1);

    let process_a = Uuid::new_v4();
    let process_b = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name, created_at)
        VALUES
          ($1, $3, NULL, '/tmp/order-a', 'branch-a', $4),
          ($2, $3, NULL, '/tmp/order-b', 'branch-b', $5)
        "#,
    )
    .bind(process_a)
    .bind(process_b)
    .bind(attempt_id)
    .bind(process_a_created_at)
    .bind(process_b_created_at)
    .execute(&pool)
    .await
    .expect("failed to seed execution processes");

    let shared_timestamp = process_a_created_at + Duration::seconds(10);
    let log_id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")
        .expect("valid deterministic uuid a");
    let log_id_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb")
        .expect("valid deterministic uuid b");

    sqlx::query(
        r#"
        INSERT INTO agent_logs (id, attempt_id, log_type, content, created_at)
        VALUES
          ($1, $3, 'stdout', 'raw-order-b', $4),
          ($2, $3, 'stdout', 'raw-order-a', $4),
          ($5, $3, 'stdout', 'raw-order-next-process', $6)
        "#,
    )
    // Insert b before a to prove query ordering does not depend on insertion order.
    .bind(log_id_b)
    .bind(log_id_a)
    .bind(attempt_id)
    .bind(shared_timestamp)
    .bind(Uuid::new_v4())
    .bind(process_b_created_at + Duration::seconds(5))
    .execute(&pool)
    .await
    .expect("failed to seed deterministic ordering logs");

    let raw_path = format!("/api/v1/execution-processes/{}/raw-logs", process_a);
    let (raw_status, raw_body) = make_request_with_string_headers(
        &router,
        "GET",
        &raw_path,
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(
        raw_status,
        StatusCode::OK,
        "unexpected raw status {raw_status}: {raw_body}"
    );

    let raw_response: serde_json::Value =
        serde_json::from_str(&raw_body).expect("failed to parse raw response");
    let raw_logs = raw_response
        .get("data")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let raw_ids: Vec<String> = raw_logs
        .iter()
        .filter_map(|log| {
            log.get("id")
                .and_then(|value| value.as_str())
                .map(ToString::to_string)
        })
        .collect();
    assert_eq!(raw_ids, vec![log_id_a.to_string(), log_id_b.to_string()]);

    let raw_messages: Vec<String> = raw_logs
        .iter()
        .filter_map(|log| {
            log.get("message")
                .and_then(|value| value.as_str())
                .map(ToString::to_string)
        })
        .collect();
    assert_eq!(raw_messages, vec!["raw-order-a", "raw-order-b"]);

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
