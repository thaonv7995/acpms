//! Task Attempts API Tests
// Include helpers module directly
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

// Test module
use serde_json::json;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

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

fn direct_gitops_repository_context() -> serde_json::Value {
    json!({
        "provider": "github",
        "access_mode": "direct_gitops",
        "verification_status": "verified",
        "can_clone": true,
        "can_push": true,
        "can_open_change_request": true,
        "can_merge": true,
        "can_manage_webhooks": true,
        "can_fork": true,
        "effective_clone_url": "https://github.com/example/test-repo",
        "writable_repository_url": "https://github.com/example/test-repo",
        "default_branch": "main",
        "verified_at": "2026-03-04T00:00:00Z"
    })
}

fn analysis_only_repository_context() -> serde_json::Value {
    json!({
        "provider": "github",
        "access_mode": "analysis_only",
        "verification_status": "verified",
        "can_clone": true,
        "can_push": false,
        "can_open_change_request": false,
        "can_merge": false,
        "can_manage_webhooks": false,
        "can_fork": true,
        "upstream_repository_url": "https://github.com/acme/upstream-repo",
        "effective_clone_url": "https://github.com/acme/upstream-repo",
        "default_branch": "main",
        "verified_at": "2026-03-04T00:00:00Z"
    })
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_task_attempt() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    seed_project_repository_context(
        &pool,
        project_id,
        Some("https://github.com/example/test-repo"),
        direct_gitops_repository_context(),
    )
    .await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/tasks/{}/attempts", task_id),
        Some("{}"),
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
    assert!(response["data"]["id"].is_string());
    assert_eq!(response["data"]["status"].as_str().unwrap(), "queued");

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_task_attempt_rejects_analysis_only_project() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    seed_project_repository_context(
        &pool,
        project_id,
        Some("https://github.com/acme/upstream-repo"),
        analysis_only_repository_context(),
    )
    .await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/tasks/{}/attempts", task_id),
        Some("{}"),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 409,
        "Expected 409 Conflict for analysis-only project, got {}: {}",
        status, body
    );
    assert!(body.contains("analysis-only"));
    assert!(body.contains("Link or create a writable fork"));

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_task_attempt_allows_init_task_for_analysis_only_project() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    seed_project_repository_context(
        &pool,
        project_id,
        Some("https://github.com/acme/upstream-repo"),
        analysis_only_repository_context(),
    )
    .await;
    let task_id = create_test_task(&pool, project_id, user_id, Some("Init Task")).await;

    sqlx::query("UPDATE tasks SET task_type = 'init'::task_type WHERE id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .expect("Failed to convert feature task into init task");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/tasks/{}/attempts", task_id),
        Some("{}"),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(
        status, 201,
        "Expected init task attempt to bypass read-only guard, got {}: {}",
        status, body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_task_attempts() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let _attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/tasks/{}/attempts", task_id),
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
async fn test_get_attempt() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/attempts/{}", attempt_id),
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
        attempt_id.to_string()
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_attempt_skills_from_metadata() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let seeded_chain = json!(["env-and-secrets-validate", "code-implement"]);
    sqlx::query(
        r#"
        UPDATE task_attempts
        SET metadata = jsonb_build_object(
            'resolved_skill_chain', $2::jsonb,
            'resolved_skill_chain_source', 'seeded_test',
            'knowledge_suggestions', jsonb_build_object(
                'status', 'ready',
                'detail', 'Seeded for test',
                'items', jsonb_build_array(
                    jsonb_build_object(
                        'skill_id', 'openai-docs',
                        'name', 'OpenAI Docs',
                        'description', 'Use official docs',
                        'score', 0.95,
                        'source_path', '/tmp/openai-docs/SKILL.md',
                        'origin', 'community-openai'
                    )
                )
            )
        )
        WHERE id = $1
        "#,
    )
    .bind(attempt_id)
    .bind(&seeded_chain)
    .execute(&pool)
    .await
    .expect("Failed to seed attempt skill metadata");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/attempts/{}/skills", attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    assert!(response["success"].as_bool().unwrap());
    assert_eq!(response["data"]["source"].as_str().unwrap(), "seeded_test");

    let returned_chain = response["data"]["resolved_skill_chain"]
        .as_array()
        .expect("resolved_skill_chain must be array");
    assert_eq!(returned_chain.len(), 2);
    assert_eq!(
        returned_chain[0].as_str().unwrap(),
        "env-and-secrets-validate"
    );
    assert_eq!(returned_chain[1].as_str().unwrap(), "code-implement");
    assert_eq!(
        response["data"]["knowledge_suggestions"]["status"]
            .as_str()
            .unwrap(),
        "ready"
    );
    assert_eq!(
        response["data"]["knowledge_suggestions"]["items"]
            .as_array()
            .expect("knowledge suggestion items must be array")
            .len(),
        1
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_attempt_skills_derived_and_persisted_when_missing() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/attempts/{}/skills", attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    assert!(response["success"].as_bool().unwrap());
    assert_eq!(
        response["data"]["source"].as_str().unwrap(),
        "attempt_skills_read_fallback"
    );

    let returned_chain = response["data"]["resolved_skill_chain"]
        .as_array()
        .expect("resolved_skill_chain must be array");
    assert!(
        !returned_chain.is_empty(),
        "Expected derived skill chain to be non-empty"
    );
    assert!(
        returned_chain
            .iter()
            .any(|item| item.as_str() == Some("env-and-secrets-validate")),
        "Expected baseline env skill in derived chain"
    );

    let persisted_chain: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT metadata->'resolved_skill_chain' FROM task_attempts WHERE id = $1",
    )
    .bind(attempt_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch persisted resolved_skill_chain");
    assert!(
        persisted_chain
            .as_ref()
            .is_some_and(serde_json::Value::is_array),
        "Expected resolved_skill_chain to be persisted into attempt metadata"
    );

    let persisted_knowledge: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT metadata->'knowledge_suggestions' FROM task_attempts WHERE id = $1",
    )
    .bind(attempt_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch persisted knowledge_suggestions");
    assert_eq!(
        response["data"]["knowledge_suggestions"]["status"]
            .as_str()
            .unwrap(),
        "disabled"
    );
    assert!(
        persisted_knowledge
            .as_ref()
            .is_some_and(serde_json::Value::is_object),
        "Expected knowledge_suggestions to be persisted into attempt metadata"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_attempt_logs() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    // Insert test log
    sqlx::query(
        r#"
            INSERT INTO agent_logs (id, attempt_id, log_type, content)
            VALUES ($1, $2, 'system', 'Test log message')
            "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(attempt_id)
    .execute(&pool)
    .await
    .expect("Failed to insert test log");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/attempts/{}/logs", attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"].is_array());

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_cancel_attempt() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let request_body = json!({
        "reason": "Test cancellation",
        "force": false
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/cancel", attempt_id),
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

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_cancel_attempt_cleans_up_worktree_without_force() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let worktrees_base = state.worktrees_path.read().await.clone();
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, Some("Cancel Cleanup Project")).await;
    let task_id = create_test_task(&pool, project_id, user_id, Some("Cancel Cleanup Task")).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let repo_relative_path = format!("cancel-cleanup-{}", project_id);
    let repo_path = worktrees_base.join(&repo_relative_path);
    fs::create_dir_all(&repo_path).expect("failed to create test repo dir");
    run_git(&repo_path, &["init", "-b", "main"]);
    run_git(
        &repo_path,
        &["config", "user.email", "cancel-test@example.com"],
    );
    run_git(&repo_path, &["config", "user.name", "Cancel Cleanup Test"]);
    fs::write(repo_path.join("README.md"), "initial\n").expect("failed to seed repository");
    run_git(&repo_path, &["add", "README.md"]);
    run_git(&repo_path, &["commit", "-m", "initial commit"]);

    let worktree_path = worktrees_base.join(format!("attempt-{attempt_id}"));
    let branch_name = format!("feat/attempt-{attempt_id}");
    run_git(
        &repo_path,
        &[
            "worktree",
            "add",
            "-b",
            &branch_name,
            worktree_path.to_str().expect("invalid worktree path"),
            "main",
        ],
    );

    sqlx::query(
        r#"
        UPDATE projects
        SET metadata = metadata || jsonb_build_object('repo_relative_path', $2)
        WHERE id = $1
        "#,
    )
    .bind(project_id)
    .bind(&repo_relative_path)
    .execute(&pool)
    .await
    .expect("failed to seed project repo_relative_path");

    let request_body = json!({
        "reason": "Cleanup without force",
        "force": false
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/cancel", attempt_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if !worktree_path.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timed out waiting for cancel cleanup to remove worktree");

    assert!(
        !worktree_path.exists(),
        "worktree directory should be removed after cancel"
    );

    let branch_exists = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .args(["branch", "--list", &branch_name])
        .output()
        .expect("failed to check branch existence");
    assert!(
        branch_exists.status.success(),
        "git branch --list failed: {}",
        String::from_utf8_lossy(&branch_exists.stderr)
    );
    assert!(
        String::from_utf8_lossy(&branch_exists.stdout)
            .trim()
            .is_empty(),
        "attempt branch should be deleted after cancel cleanup"
    );

    let _ = fs::remove_dir_all(&repo_path);
    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_attempt_diff() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/attempts/{}/diff", attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    // May return 200 with empty diff or 404 if diff not available
    assert!(
        status == 200 || status == 404,
        "Expected 200 or 404, got {}: {}",
        status,
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_create_task_attempt_rejects_when_active_attempt_exists() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let _existing_attempt_id = create_test_attempt(&pool, task_id, Some("queued")).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/tasks/{}/attempts", task_id),
        Some("{}"),
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
        body.to_lowercase().contains("active attempt"),
        "Expected active-attempt guard error, got: {}",
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_retry_cancelled_attempt_is_allowed() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let cancelled_attempt_id = create_test_attempt(&pool, task_id, Some("cancelled")).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/retry", cancelled_attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(
        status, 201,
        "Expected 201 Created, got {}: {}",
        status, body
    );

    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    assert!(response["success"].as_bool().unwrap_or(false));
    assert_eq!(
        response["data"]["retry_info"]["previous_attempt_id"]
            .as_str()
            .unwrap_or_default(),
        cancelled_attempt_id.to_string()
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_retry_attempt_rejects_success_status() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let success_attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/retry", success_attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(
        status, 400,
        "Expected 400 Bad Request, got {}: {}",
        status, body
    );
    assert!(
        body.to_lowercase().contains("only failed or cancelled"),
        "Expected retriable-state guard error, got: {}",
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_retry_attempt_rejects_when_max_retries_exceeded() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let failed_attempt_id = create_test_attempt(&pool, task_id, Some("failed")).await;

    sqlx::query(
        r#"
        UPDATE projects
        SET settings = settings || '{"max_retries": 1}'::jsonb
        WHERE id = $1
        "#,
    )
    .bind(project_id)
    .execute(&pool)
    .await
    .expect("Failed to set max_retries");

    sqlx::query(
        r#"
        UPDATE task_attempts
        SET metadata = metadata || '{"retry_count": 1}'::jsonb
        WHERE id = $1
        "#,
    )
    .bind(failed_attempt_id)
    .execute(&pool)
    .await
    .expect("Failed to set retry_count metadata");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/retry", failed_attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(
        status, 400,
        "Expected 400 Bad Request, got {}: {}",
        status, body
    );
    assert!(
        body.to_lowercase().contains("maximum retries (1) exceeded"),
        "Expected max-retries guard error, got: {}",
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_retry_info_for_cancelled_attempt() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let cancelled_attempt_id = create_test_attempt(&pool, task_id, Some("cancelled")).await;

    sqlx::query(
        r#"
        UPDATE task_attempts
        SET metadata = metadata || '{"retry_count": 1}'::jsonb
        WHERE id = $1
        "#,
    )
    .bind(cancelled_attempt_id)
    .execute(&pool)
    .await
    .expect("Failed to set retry_count metadata");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/attempts/{}/retry-info", cancelled_attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);
    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    assert!(response["success"].as_bool().unwrap_or(false));
    assert_eq!(
        response["data"]["retry_count"].as_i64().unwrap_or_default(),
        1
    );
    assert_eq!(response["data"]["can_retry"].as_bool(), Some(true));
    assert!(response["data"]["next_backoff_seconds"].is_null());

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_get_retry_info_for_failed_attempt_includes_backoff() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let failed_attempt_id = create_test_attempt(&pool, task_id, Some("failed")).await;

    sqlx::query(
        r#"
        UPDATE task_attempts
        SET metadata = metadata || '{"retry_count": 0}'::jsonb
        WHERE id = $1
        "#,
    )
    .bind(failed_attempt_id)
    .execute(&pool)
    .await
    .expect("Failed to seed retry_count");

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "GET",
        &format!("/api/v1/attempts/{}/retry-info", failed_attempt_id),
        None,
        vec![auth_header_bearer(&token)],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);
    let response: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse response");
    assert_eq!(
        response["data"]["next_backoff_seconds"]
            .as_u64()
            .unwrap_or_default(),
        60
    );
    assert_eq!(response["data"]["can_retry"].as_bool(), Some(true));

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_cancel_attempt_rejects_completed_attempt() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let success_attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    let request_body = json!({
        "reason": "Cannot cancel this",
        "force": false
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/cancel", success_attempt_id),
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
        body.to_lowercase()
            .contains("cannot cancel attempt in success"),
        "Expected non-cancellable-state error, got: {}",
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_cancel_attempt_sets_force_flag_and_resets_task_status() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    sqlx::query("UPDATE tasks SET status = 'in_progress' WHERE id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .expect("Failed to move task to in_progress");

    let request_body = json!({
        "reason": "Manual stop",
        "force": true
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/cancel", attempt_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let (attempt_status, error_message, force_kill, cancelled_by): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        r#"
        SELECT
            status::text,
            error_message,
            metadata->>'force_kill' as force_kill,
            metadata->>'cancelled_by' as cancelled_by
        FROM task_attempts
        WHERE id = $1
        "#,
    )
    .bind(attempt_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch cancelled attempt");

    let task_status: String = sqlx::query_scalar("SELECT status::text FROM tasks WHERE id = $1")
        .bind(task_id)
        .fetch_one(&pool)
        .await
        .expect("Failed to fetch task status");

    assert_eq!(attempt_status, "cancelled");
    assert_eq!(error_message.as_deref(), Some("Manual stop"));
    assert_eq!(force_kill.as_deref(), Some("true"));
    let expected_cancelled_by = user_id.to_string();
    assert_eq!(
        cancelled_by.as_deref(),
        Some(expected_cancelled_by.as_str())
    );
    assert_eq!(task_status, "todo");

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_cancel_attempt_reverts_to_previous_status_when_task_was_done() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;

    // First attempt: success → task was done
    let _success_attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;
    sqlx::query("UPDATE tasks SET status = 'done' WHERE id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .expect("Failed to set task to done");

    // Second attempt: follow-up running, then cancel
    let running_attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;
    sqlx::query("UPDATE tasks SET status = 'in_progress' WHERE id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .expect("Failed to set task to in_progress");

    let request_body = json!({ "reason": "User cancelled follow-up" });
    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/cancel", running_attempt_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let task_status: String = sqlx::query_scalar("SELECT status::text FROM tasks WHERE id = $1")
        .bind(task_id)
        .fetch_one(&pool)
        .await
        .expect("Failed to fetch task status");

    // Should revert to done (previous status), not todo
    assert_eq!(
        task_status, "done",
        "Task should revert to done when cancelling follow-up from a done task"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_cancel_attempt_reverts_to_stored_previous_task_status_without_prior_attempt() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let running_attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    sqlx::query("UPDATE tasks SET status = 'in_progress' WHERE id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .expect("Failed to set task to in_progress");

    sqlx::query(
        r#"
        UPDATE task_attempts
        SET metadata = metadata || '{"previous_task_status":"done"}'::jsonb
        WHERE id = $1
        "#,
    )
    .bind(running_attempt_id)
    .execute(&pool)
    .await
    .expect("Failed to seed previous_task_status");

    let request_body = json!({ "reason": "User cancelled rerun" });
    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/cancel", running_attempt_id),
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(&token),
        ],
    )
    .await;

    assert_eq!(status, 200, "Expected 200 OK, got {}: {}", status, body);

    let task_status: String = sqlx::query_scalar("SELECT status::text FROM tasks WHERE id = $1")
        .bind(task_id)
        .fetch_one(&pool)
        .await
        .expect("Failed to fetch task status");

    assert_eq!(
        task_status, "done",
        "Task should revert to stored previous_task_status when cancelling a rerun"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_resume_attempt_rejects_running_attempt() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let running_attempt_id = create_test_attempt(&pool, task_id, Some("running")).await;

    let request_body = json!({
        "prompt": "continue where you left off"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/resume", running_attempt_id),
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
        body.to_lowercase().contains("cannot resume attempt"),
        "Expected resume-state guard error, got: {}",
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_resume_attempt_requires_execution_process_context() {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;
    let completed_attempt_id = create_test_attempt(&pool, task_id, Some("success")).await;

    let request_body = json!({
        "prompt": "continue where you left off"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        &router,
        "POST",
        &format!("/api/v1/attempts/{}/resume", completed_attempt_id),
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
        body.to_lowercase().contains("execution process context"),
        "Expected execution-process-context guard error, got: {}",
        body
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database"]
async fn test_db_guard_prevents_multiple_active_attempts_for_same_task() {
    let pool = setup_test_db().await;
    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let project_id = create_test_project(&pool, user_id, None).await;
    let task_id = create_test_task(&pool, project_id, user_id, None).await;

    let _first_attempt = create_test_attempt(&pool, task_id, Some("queued")).await;

    let second_insert = sqlx::query(
        r#"
        INSERT INTO task_attempts (id, task_id, status, metadata)
        VALUES ($1, $2, 'running', '{}'::jsonb)
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(task_id)
    .execute(&pool)
    .await;

    assert!(
        second_insert.is_err(),
        "Expected unique active-attempt DB guard to reject second active attempt"
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}
// End test module
