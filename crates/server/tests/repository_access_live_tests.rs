//! Live repository access integration tests.
//!
//! These tests exercise the real HTTP routes against live GitHub/GitLab repositories using
//! environment-provided repository URLs and PATs. They are intentionally ignored by default
//! because they require:
//! - a working test database (`DATABASE_URL`)
//! - network access to the provider
//! - provider credentials with enough scope for metadata lookup and clone verification
//!
//! Run example:
//! `cargo test -p acpms-server --test repository_access_live_tests -- --ignored --nocapture`

#[allow(dead_code)]
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

use serde_json::{json, Value};
use uuid::Uuid;

struct LiveProviderConfig {
    base_url: String,
    pat: String,
    public_upstream_url: String,
    writable_repo_url: String,
}

impl LiveProviderConfig {
    fn from_env(prefix: &'static str, default_base_url: &'static str) -> Option<Self> {
        if std::env::var("ACPMS_LIVE_PROVIDER_TESTS").ok().as_deref() != Some("1") {
            eprintln!(
                "Skipping live {} provider tests because ACPMS_LIVE_PROVIDER_TESTS=1 is not set",
                prefix.to_ascii_lowercase()
            );
            return None;
        }

        let base_url = std::env::var(format!("ACPMS_LIVE_{}_BASE_URL", prefix))
            .unwrap_or_else(|_| default_base_url.to_string());
        let pat = match std::env::var(format!("ACPMS_LIVE_{}_PAT", prefix)) {
            Ok(value) if !value.trim().is_empty() => value,
            _ => {
                eprintln!(
                    "Skipping live {} provider tests because ACPMS_LIVE_{}_PAT is missing",
                    prefix.to_ascii_lowercase(),
                    prefix
                );
                return None;
            }
        };
        let public_upstream_url = match std::env::var(format!(
            "ACPMS_LIVE_{}_PUBLIC_UPSTREAM_URL",
            prefix
        )) {
            Ok(value) if !value.trim().is_empty() => value,
            _ => {
                eprintln!(
                        "Skipping live {} provider tests because ACPMS_LIVE_{}_PUBLIC_UPSTREAM_URL is missing",
                        prefix.to_ascii_lowercase(),
                        prefix
                    );
                return None;
            }
        };
        let writable_repo_url = match std::env::var(format!("ACPMS_LIVE_{}_WRITABLE_URL", prefix)) {
            Ok(value) if !value.trim().is_empty() => value,
            _ => {
                eprintln!(
                    "Skipping live {} provider tests because ACPMS_LIVE_{}_WRITABLE_URL is missing",
                    prefix.to_ascii_lowercase(),
                    prefix
                );
                return None;
            }
        };

        Some(Self {
            base_url,
            pat,
            public_upstream_url,
            writable_repo_url,
        })
    }
}

async fn import_project(
    router: &axum::Router,
    token: &str,
    name: &str,
    repository_url: &str,
) -> (Uuid, Value) {
    let request_body = json!({
        "name": name,
        "repository_url": repository_url,
        "auto_create_init_task": false
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        router,
        "POST",
        "/api/v1/projects/import",
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(token),
        ],
    )
    .await;

    assert_eq!(
        status, 201,
        "Expected import to succeed, got {}: {}",
        status, body
    );

    let response: Value = serde_json::from_str(&body).expect("Failed to parse import response");
    let project_id: Uuid = response["data"]["project"]["id"]
        .as_str()
        .expect("Missing imported project id")
        .parse()
        .expect("Invalid imported project id");

    (project_id, response)
}

async fn create_feature_task(
    router: &axum::Router,
    token: &str,
    project_id: Uuid,
    title: &str,
) -> Uuid {
    let request_body = json!({
        "project_id": project_id.to_string(),
        "title": title,
        "description": "Live repository access test task",
        "task_type": "Feature"
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        router,
        "POST",
        "/api/v1/tasks",
        Some(&request_body.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(token),
        ],
    )
    .await;

    assert_eq!(
        status, 201,
        "Expected task creation to succeed, got {}: {}",
        status, body
    );

    let response: Value = serde_json::from_str(&body).expect("Failed to parse task response");
    response["data"]["id"]
        .as_str()
        .expect("Missing task id")
        .parse()
        .expect("Invalid task id")
}

async fn assert_analysis_only_import_and_attempt_block(
    provider: &str,
    router: &axum::Router,
    token: &str,
    repository_url: &str,
) -> Uuid {
    let preflight_request = json!({
        "repository_url": repository_url
    });

    let (status, body): (axum::http::StatusCode, String) = make_request_with_string_headers(
        router,
        "POST",
        "/api/v1/projects/import/preflight",
        Some(&preflight_request.to_string()),
        vec![
            ("content-type", "application/json".to_string()),
            auth_header_bearer(token),
        ],
    )
    .await;

    assert_eq!(
        status, 200,
        "Expected preflight to succeed, got {}: {}",
        status, body
    );

    let preflight: Value = serde_json::from_str(&body).expect("Failed to parse preflight response");
    assert_eq!(
        preflight["data"]["repository_context"]["provider"].as_str(),
        Some(provider),
        "Expected provider {} in preflight response: {}",
        provider,
        body
    );
    assert_eq!(
        preflight["data"]["repository_context"]["access_mode"].as_str(),
        Some("analysis_only"),
        "Expected analysis_only in preflight response: {}",
        body
    );

    let (project_id, import_response) = import_project(
        router,
        token,
        &format!("Live {} Analysis Only", provider),
        repository_url,
    )
    .await;

    assert_eq!(
        import_response["data"]["project"]["repository_context"]["access_mode"].as_str(),
        Some("analysis_only"),
        "Expected imported project to remain analysis_only: {}",
        import_response
    );

    let task_id = create_feature_task(
        router,
        token,
        project_id,
        &format!("Live {} read-only task", provider),
    )
    .await;

    let (attempt_status, attempt_body): (axum::http::StatusCode, String) =
        make_request_with_string_headers(
            router,
            "POST",
            &format!("/api/v1/tasks/{}/attempts", task_id),
            Some("{}"),
            vec![
                ("content-type", "application/json".to_string()),
                auth_header_bearer(token),
            ],
        )
        .await;

    assert_eq!(
        attempt_status, 409,
        "Expected read-only attempt creation to be blocked, got {}: {}",
        attempt_status, attempt_body
    );
    assert!(
        attempt_body.contains("not writable for coding attempts"),
        "Expected actionable repository guard error, got {}",
        attempt_body
    );

    project_id
}

async fn assert_direct_gitops_import(
    provider: &str,
    router: &axum::Router,
    token: &str,
    repository_url: &str,
) -> Uuid {
    let (project_id, import_response) = import_project(
        router,
        token,
        &format!("Live {} Writable", provider),
        repository_url,
    )
    .await;

    assert_eq!(
        import_response["data"]["project"]["repository_context"]["provider"].as_str(),
        Some(provider),
        "Expected provider {} in import response: {}",
        provider,
        import_response
    );
    assert_eq!(
        import_response["data"]["project"]["repository_context"]["access_mode"].as_str(),
        Some("direct_gitops"),
        "Expected direct_gitops for writable repo: {}",
        import_response
    );

    project_id
}

#[tokio::test]
#[ignore = "requires test database and live GitHub setup"]
async fn test_live_github_repository_modes() {
    let Some(config) = LiveProviderConfig::from_env("GITHUB", "https://github.com") else {
        return;
    };

    let pool = setup_test_db().await;
    configure_test_system_settings(&pool, &config.base_url, &config.pat).await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let analysis_project_id = assert_analysis_only_import_and_attempt_block(
        "github",
        &router,
        &token,
        &config.public_upstream_url,
    )
    .await;
    let direct_project_id =
        assert_direct_gitops_import("github", &router, &token, &config.writable_repo_url).await;

    delete_test_project(&pool, analysis_project_id).await;
    delete_test_project(&pool, direct_project_id).await;
    cleanup_test_data(&pool, user_id, None).await;
}

#[tokio::test]
#[ignore = "requires test database and live GitLab setup"]
async fn test_live_gitlab_repository_modes() {
    let Some(config) = LiveProviderConfig::from_env("GITLAB", "https://gitlab.com") else {
        return;
    };

    let pool = setup_test_db().await;
    configure_test_system_settings(&pool, &config.base_url, &config.pat).await;
    let state = create_test_app_state(pool.clone()).await;
    let router = create_router(state);

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let token = generate_test_token(user_id);

    let analysis_project_id = assert_analysis_only_import_and_attempt_block(
        "gitlab",
        &router,
        &token,
        &config.public_upstream_url,
    )
    .await;
    let direct_project_id =
        assert_direct_gitops_import("gitlab", &router, &token, &config.writable_repo_url).await;

    delete_test_project(&pool, analysis_project_id).await;
    delete_test_project(&pool, direct_project_id).await;
    cleanup_test_data(&pool, user_id, None).await;
}
