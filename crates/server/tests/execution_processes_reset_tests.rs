//! Execution process reset API tests

#[path = "helpers.rs"]
mod helpers;
use helpers::*;

use axum::http::StatusCode;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

fn run_git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .expect("failed to execute git command");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_status_porcelain(path: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("status")
        .arg("--porcelain")
        .output()
        .expect("failed to execute git status --porcelain");
    assert!(
        output.status.success(),
        "git status --porcelain failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn create_temp_git_repo(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("acpms-reset-test-{prefix}-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("failed to create temp git repo dir");

    run_git(&dir, &["init"]);
    run_git(&dir, &["config", "user.email", "reset-test@example.com"]);
    run_git(&dir, &["config", "user.name", "Reset Test"]);

    let file_path = dir.join("README.md");
    fs::write(&file_path, "initial\n").expect("failed to seed repository file");

    run_git(&dir, &["add", "README.md"]);
    run_git(&dir, &["commit", "-m", "initial commit"]);
    dir
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_reset_execution_process_rejects_missing_worktree_path_when_git_reset_requested() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let project_id =
        create_test_project(&pool, user_id, Some("Reset Missing Worktree Project")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("Reset Missing Worktree Task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    let process_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name)
        VALUES ($1, $2, NULL, NULL, NULL)
        "#,
    )
    .bind(process_id)
    .bind(attempt_id)
    .execute(&pool)
    .await
    .expect("failed to seed execution process");

    let path = format!("/api/v1/execution-processes/{process_id}/reset");
    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &path,
        Some(&json!({ "perform_git_reset": true }).to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "unexpected status {status}: {body}"
    );
    assert!(
        body.contains("no worktree path"),
        "expected missing worktree error, got {body}"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_reset_execution_process_requires_force_when_worktree_dirty() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let project_id =
        create_test_project(&pool, user_id, Some("Reset Dirty Worktree Project")).await;
    let task_id = create_test_task(
        &pool,
        project_id,
        user_id,
        Some("Reset Dirty Worktree Task"),
    )
    .await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    let repo_path = create_temp_git_repo("dirty-reject");
    fs::write(repo_path.join("README.md"), "dirty change\n").expect("failed to dirty repo");

    let process_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name)
        VALUES ($1, $2, NULL, $3, NULL)
        "#,
    )
    .bind(process_id)
    .bind(attempt_id)
    .bind(repo_path.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .expect("failed to seed execution process");

    let path = format!("/api/v1/execution-processes/{process_id}/reset");
    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &path,
        Some(
            &json!({
                "perform_git_reset": true,
                "force_when_dirty": false
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
        StatusCode::BAD_REQUEST,
        "unexpected status {status}: {body}"
    );
    assert!(
        body.contains("force_when_dirty"),
        "expected force_when_dirty guard error, got {body}"
    );
    assert!(
        !git_status_porcelain(&repo_path).trim().is_empty(),
        "repo should remain dirty when reset is rejected"
    );

    let _ = fs::remove_dir_all(&repo_path);
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_reset_execution_process_force_resets_dirty_repo() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let project_id = create_test_project(&pool, user_id, Some("Reset Force Project")).await;
    let task_id = create_test_task(&pool, project_id, user_id, Some("Reset Force Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    let repo_path = create_temp_git_repo("dirty-force");
    fs::write(repo_path.join("README.md"), "dirty change\n").expect("failed to dirty repo");

    let process_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO execution_processes (id, attempt_id, process_id, worktree_path, branch_name)
        VALUES ($1, $2, NULL, $3, NULL)
        "#,
    )
    .bind(process_id)
    .bind(attempt_id)
    .bind(repo_path.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .expect("failed to seed execution process");

    let path = format!("/api/v1/execution-processes/{process_id}/reset");
    let (status, body) = make_request_with_string_headers(
        &router,
        "POST",
        &path,
        Some(
            &json!({
                "perform_git_reset": true,
                "force_when_dirty": true
            })
            .to_string(),
        ),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "unexpected status {status}: {body}");
    assert!(
        git_status_porcelain(&repo_path).trim().is_empty(),
        "repo should be clean after forced reset"
    );

    let payload: serde_json::Value =
        serde_json::from_str(&body).expect("reset response should be valid json");
    assert_eq!(
        payload
            .pointer("/data/git_reset_applied")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        payload
            .pointer("/data/worktree_was_dirty")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        payload
            .pointer("/data/force_when_dirty")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    let requested_by = payload
        .pointer("/data/requested_by_user_id")
        .and_then(|value| value.as_str())
        .expect("expected requested_by_user_id metadata");
    assert_eq!(requested_by, user_id.to_string());
    let requested_at = payload
        .pointer("/data/requested_at")
        .and_then(|value| value.as_str())
        .expect("expected requested_at metadata");
    assert!(
        chrono::DateTime::parse_from_rfc3339(requested_at).is_ok(),
        "requested_at should be RFC3339 timestamp, got {requested_at}"
    );

    let _ = fs::remove_dir_all(&repo_path);
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
