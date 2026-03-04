//! Live tests for repository context refresh after project creation.
//!
//! These tests verify the post-create automation used by from-scratch init:
//! once a repository URL exists, the orchestrator should classify provider
//! access and persist `projects.repository_context` automatically.
//!
//! Run example:
//! `cargo test -p acpms-server --test repository_context_refresh_live_tests -- --ignored --nocapture`

#[allow(dead_code)]
#[path = "helpers.rs"]
mod helpers;
use helpers::*;

use acpms_db::models::{RepositoryAccessMode, RepositoryContext, RepositoryProvider};

struct LiveWritableProviderConfig {
    base_url: String,
    pat: String,
    writable_repo_url: String,
}

impl LiveWritableProviderConfig {
    fn from_env(prefix: &'static str, default_base_url: &'static str) -> Option<Self> {
        if std::env::var("ACPMS_LIVE_PROVIDER_TESTS").ok().as_deref() != Some("1") {
            eprintln!(
                "Skipping live {} refresh tests because ACPMS_LIVE_PROVIDER_TESTS=1 is not set",
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
                    "Skipping live {} refresh tests because ACPMS_LIVE_{}_PAT is missing",
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
                    "Skipping live {} refresh tests because ACPMS_LIVE_{}_WRITABLE_URL is missing",
                    prefix.to_ascii_lowercase(),
                    prefix
                );
                return None;
            }
        };

        Some(Self {
            base_url,
            pat,
            writable_repo_url,
        })
    }
}

async fn assert_refresh_persists_direct_gitops_context(
    prefix: &'static str,
    expected_provider: RepositoryProvider,
    default_base_url: &'static str,
) {
    let Some(config) = LiveWritableProviderConfig::from_env(prefix, default_base_url) else {
        return;
    };

    let pool = setup_test_db().await;
    configure_test_system_settings(&pool, &config.base_url, &config.pat).await;
    let state = create_test_app_state(pool.clone()).await;

    let (user_id, _) = create_test_user(&pool, None, None, None).await;
    let project_id = create_test_project(&pool, user_id, Some("Live Refresh Project")).await;

    sqlx::query("UPDATE projects SET repository_url = $2, updated_at = NOW() WHERE id = $1")
        .bind(project_id)
        .bind(&config.writable_repo_url)
        .execute(&pool)
        .await
        .expect("Failed to seed repository URL for refresh test");

    let refreshed = state
        .orchestrator
        .refresh_repository_context_after_repo_creation(project_id, &config.writable_repo_url)
        .await
        .expect("Failed to refresh repository context after repo creation");

    assert_eq!(
        refreshed.provider, expected_provider,
        "Expected provider {:?}, got {:?}",
        expected_provider, refreshed.provider
    );
    assert_eq!(
        refreshed.access_mode,
        RepositoryAccessMode::DirectGitops,
        "Expected writable repo to classify as direct_gitops: {:?}",
        refreshed
    );
    assert!(
        refreshed.can_clone,
        "Expected writable repo to remain cloneable"
    );
    assert!(refreshed.can_push, "Expected writable repo to be pushable");
    assert!(
        refreshed.can_open_change_request,
        "Expected writable repo to allow PR/MR creation"
    );

    let persisted_context_json: serde_json::Value =
        sqlx::query_scalar("SELECT repository_context FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch persisted repository context");
    let persisted_context: RepositoryContext = serde_json::from_value(persisted_context_json)
        .expect("Failed to deserialize persisted repository context");

    assert_eq!(persisted_context.provider, expected_provider);
    assert_eq!(
        persisted_context.access_mode,
        RepositoryAccessMode::DirectGitops
    );
    assert_eq!(
        persisted_context.effective_clone_url.as_deref(),
        Some(config.writable_repo_url.as_str())
    );

    cleanup_test_data(&pool, user_id, Some(project_id)).await;
}

#[tokio::test]
#[ignore = "requires test database and live GitHub writable repository"]
async fn test_refresh_repository_context_after_creation_github() {
    assert_refresh_persists_direct_gitops_context(
        "GITHUB",
        RepositoryProvider::Github,
        "https://github.com",
    )
    .await;
}

#[tokio::test]
#[ignore = "requires test database and live GitLab writable repository"]
async fn test_refresh_repository_context_after_creation_gitlab() {
    assert_refresh_persists_direct_gitops_context(
        "GITLAB",
        RepositoryProvider::Gitlab,
        "https://gitlab.com",
    )
    .await;
}
