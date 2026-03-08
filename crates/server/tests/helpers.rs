//! Test Helpers
//!
//! Common utilities for API integration tests.

use acpms_db::models::{SystemRole, UpdateSystemSettingsRequest};
use acpms_deployment::CloudflareClient;
use acpms_executors::{ExecutorOrchestrator, ProjectAssistantJob};
use acpms_preview::PreviewManager;
use acpms_services::{generate_jwt, hash_password};
use acpms_services::{
    BuildService, EncryptionService, GitLabOAuthService, GitLabService, GitLabSyncService,
    OpenClawGatewayEventService, PatchStore, ProductionDeployService, SprintService,
    StorageService, StreamService, SystemSettingsService, UserService, WebhookAdminService,
    WebhookManager,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower::ServiceExt;
use uuid::Uuid;

// Import from lib.rs (library crate)
use acpms_server::services::project_assistant_worker_pool::{
    ProjectAssistantJobHandler, ProjectAssistantWorkerPool,
};
use acpms_server::state::{AppState, OpenClawGatewayConfig};
// use acpms_server::routes; // Not needed, only create_router is used

// Re-export for convenience
pub use acpms_server::routes::create_router;

/// Setup test database connection
pub async fn setup_test_db() -> PgPool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/acpms_test".to_string());

    // Set default test env vars if not already set
    if std::env::var("ENCRYPTION_KEY").is_err() {
        // Generate a test key (32 bytes base64 encoded)
        // This is only for testing - never use in production!
        std::env::set_var(
            "ENCRYPTION_KEY",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        );
    }
    if std::env::var("JWT_SECRET").is_err() {
        std::env::set_var("JWT_SECRET", "test-jwt-secret-key-for-testing-only");
    }
    if std::env::var("S3_ENDPOINT").is_err() {
        std::env::set_var("S3_ENDPOINT", "http://localhost:9000");
    }
    if std::env::var("S3_PUBLIC_ENDPOINT").is_err() {
        std::env::set_var("S3_PUBLIC_ENDPOINT", "http://localhost:9000");
    }
    if std::env::var("S3_ACCESS_KEY").is_err() {
        std::env::set_var("S3_ACCESS_KEY", "admin");
    }
    if std::env::var("S3_SECRET_KEY").is_err() {
        std::env::set_var("S3_SECRET_KEY", "adminpassword123");
    }
    if std::env::var("S3_REGION").is_err() {
        std::env::set_var("S3_REGION", "us-east-1");
    }
    if std::env::var("S3_BUCKET_NAME").is_err() {
        std::env::set_var("S3_BUCKET_NAME", "acpms-media");
    }

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    // Run migrations
    // sqlx::migrate! resolves paths relative to crate root (crates/server/)
    // So from crates/server/ to crates/db/migrations is ../db/migrations
    // Ignore errors if migrations already applied
    let _ = sqlx::migrate!("../db/migrations").run(&pool).await;

    pool
}

/// Create test AppState
pub async fn create_test_app_state(pool: PgPool) -> AppState {
    let (broadcast_tx, _) = broadcast::channel(100);
    let worktrees_path = Arc::new(tokio::sync::RwLock::new(
        std::env::temp_dir().join(format!("acpms-test-worktrees-{}", Uuid::new_v4())),
    ));
    let worktrees_dir = worktrees_path.read().await.clone();
    tokio::fs::create_dir_all(&worktrees_dir)
        .await
        .expect("Failed to create test worktrees directory");

    // Initialize encryption service (use test key if ENCRYPTION_KEY not set)
    let encryption_key = std::env::var("ENCRYPTION_KEY").unwrap_or_else(|_| {
        // Generate a test key (32 bytes base64 encoded)
        // This is only for testing - never use in production!
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()
    });
    let encryption_service = Arc::new(
        EncryptionService::new(&encryption_key).expect("Failed to create encryption service"),
    );

    // Initialize StorageService (async, no params, reads from env)
    // For tests, we'll try to create it but allow graceful failure
    let storage_service = match StorageService::new().await {
        Ok(service) => Arc::new(service),
        Err(_) => {
            // If StorageService fails (e.g., S3 not configured), we can't continue
            // Tests that need StorageService should set up S3 env vars
            panic!("StorageService initialization failed. Set S3_ENDPOINT, S3_ACCESS_KEY, S3_SECRET_KEY, S3_BUCKET_NAME env vars for tests");
        }
    };

    acpms_executors::init_agent_log_buffer(pool.clone());

    // Initialize ExecutorOrchestrator (not async, returns Result)
    let orchestrator = Arc::new(
        ExecutorOrchestrator::new(
            pool.clone(),
            worktrees_path.clone(),
            broadcast_tx.clone(),
            storage_service.clone() as Arc<dyn acpms_executors::DiffStorageUploader>,
        )
        .expect("Failed to create orchestrator")
        .with_skill_knowledge(acpms_executors::SkillKnowledgeHandle::disabled()),
    );
    let metrics = acpms_server::observability::Metrics::new()
        .expect("Failed to initialize metrics for tests");
    let openclaw_gateway = Arc::new(OpenClawGatewayConfig::from_env());
    let openclaw_event_service = Arc::new(
        OpenClawGatewayEventService::new(pool.clone(), openclaw_gateway.event_retention_hours)
            .with_optional_webhook(
                openclaw_gateway.webhook_url.clone(),
                openclaw_gateway.webhook_secret.clone(),
            )
            .with_metrics_observer(Arc::new(metrics.clone())),
    );

    // Initialize Services (GitLabService returns Result)
    let gitlab_service =
        Arc::new(GitLabService::new(pool.clone()).expect("Failed to create GitLab service"));
    let gitlab_sync_service = Arc::new(
        GitLabSyncService::new(pool.clone(), (*gitlab_service).clone())
            .with_openclaw_events(openclaw_event_service.clone()),
    );
    let user_service = UserService::new(pool.clone());
    let sprint_service = SprintService::new(pool.clone());
    let webhook_manager = Arc::new(
        WebhookManager::new(pool.clone()).with_openclaw_events(openclaw_event_service.clone()),
    );

    // GitLabOAuthService - set test env vars if not already set
    if std::env::var("GITLAB_CLIENT_ID").is_err() {
        std::env::set_var("GITLAB_CLIENT_ID", "test_client_id");
    }
    if std::env::var("GITLAB_CLIENT_SECRET").is_err() {
        std::env::set_var("GITLAB_CLIENT_SECRET", "test_client_secret");
    }
    if std::env::var("GITLAB_REDIRECT_URI").is_err() {
        std::env::set_var("GITLAB_REDIRECT_URI", "http://localhost:3000/callback");
    }

    let gitlab_oauth_service = Arc::new(
        GitLabOAuthService::from_env(pool.clone()).expect("Failed to create GitLabOAuthService"),
    );

    let webhook_admin_service = Arc::new(WebhookAdminService::new(pool.clone()));

    // SystemSettingsService returns Result and needs EncryptionService
    let settings_service = Arc::new(
        SystemSettingsService::new(pool.clone()).expect("Failed to create settings service"),
    );

    // Initialize Cloudflare client for preview manager
    let cloudflare_client = CloudflareClient::new(
        std::env::var("CLOUDFLARE_API_TOKEN").unwrap_or_default(),
        std::env::var("CLOUDFLARE_ACCOUNT_ID").unwrap_or_default(),
    )
    .expect("Failed to create CloudflareClient");

    // PreviewManager::new is not async, takes CloudflareClient, EncryptionService, SystemSettingsService, pool, preview_ttl_days
    let preview_manager = Arc::new(PreviewManager::new(
        cloudflare_client,
        (*encryption_service).clone(),
        (*settings_service).clone(),
        pool.clone(),
        Some(7), // TTL 7 days
        worktrees_path.clone(),
    ));

    // Initialize Build and Deploy Services
    let build_service = Arc::new(BuildService::new(
        (*storage_service).clone(),
        pool.clone(),
        worktrees_path.clone(),
    ));

    let deploy_service = Arc::new(ProductionDeployService::new(
        pool.clone(),
        (*settings_service).clone(),
        (*encryption_service).clone(),
    ));

    // Initialize Stream Service components
    let patch_store = Arc::new(PatchStore::new(100)); // Keep last 100 patches
    let stream_service = Arc::new(StreamService::new(
        patch_store.clone(),
        broadcast_tx.clone(),
        pool.clone(),
    ));
    let mut state = AppState {
        worktrees_path: worktrees_path.clone(),
        db: pool,
        metrics,
        orchestrator,
        worker_pool: None,
        deployment_worker_pool: None,
        project_assistant_worker_pool: None,
        broadcast_tx,
        gitlab_service,
        gitlab_sync_service,
        user_service,
        sprint_service,
        webhook_manager,
        gitlab_oauth_service,
        webhook_admin_service,
        settings_service,
        preview_manager,
        storage_service,
        build_service,
        deploy_service,
        patch_store,
        stream_service,
        auth_session_store: Arc::new(acpms_server::services::agent_auth::AuthSessionStore::new()),
        openclaw_gateway,
        openclaw_event_service: openclaw_event_service.clone(),
    };

    openclaw_event_service
        .clone()
        .spawn_agent_event_bridge(state.broadcast_tx.subscribe());

    let project_assistant_handler_state = state.clone();
    let project_assistant_handler: ProjectAssistantJobHandler =
        Arc::new(move |job: ProjectAssistantJob| {
            let handler_state = project_assistant_handler_state.clone();
            Box::pin(async move {
                acpms_server::routes::project_assistant::process_project_assistant_job(
                    handler_state,
                    job,
                )
                .await;
            }) as futures::future::BoxFuture<'static, ()>
        });
    let project_assistant_worker_pool = Arc::new(ProjectAssistantWorkerPool::new(
        1,
        project_assistant_handler,
    ));
    project_assistant_worker_pool.start();
    state.project_assistant_worker_pool = Some(project_assistant_worker_pool);

    state
}

/// Create test router
pub async fn create_test_router() -> Router {
    let pool = setup_test_db().await;
    let state = create_test_app_state(pool).await;
    create_router(state)
}

/// Create a test user in database
pub async fn create_test_user(
    pool: &PgPool,
    email: Option<&str>,
    password: Option<&str>,
    roles: Option<Vec<SystemRole>>,
) -> (Uuid, String) {
    let user_id = Uuid::new_v4();
    let default_email = format!("test-{}@example.com", user_id);
    let email = email.as_deref().unwrap_or(&default_email);
    let password = password.unwrap_or("testpassword123");
    let password_hash = hash_password(password).expect("Failed to hash password");
    let roles = roles.unwrap_or_else(|| vec![SystemRole::Viewer]);

    sqlx::query(
        r#"
        INSERT INTO users (id, email, name, password_hash, global_roles)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(user_id)
    .bind(email)
    .bind("Test User")
    .bind(&password_hash)
    .bind(&roles)
    .execute(pool)
    .await
    .expect("Failed to create test user");

    (user_id, password.to_string())
}

/// Create a test admin user
pub async fn create_test_admin(pool: &PgPool) -> (Uuid, String) {
    create_test_user(pool, None, None, Some(vec![SystemRole::Admin])).await
}

/// Generate JWT token for test user
pub fn generate_test_token(user_id: Uuid) -> String {
    generate_jwt(user_id).expect("Failed to generate test token")
}

/// Create authorization header with token
pub fn auth_header(token: &str) -> (&'static str, &str) {
    // Return a tuple that can be used directly in Vec<(&str, &str)>
    // Note: This requires the token to live long enough, so we'll use a different approach
    ("authorization", token)
}

// Helper to create auth header with Bearer prefix
pub fn auth_header_bearer(token: &str) -> (&'static str, String) {
    ("authorization", format!("Bearer {}", token))
}

/// Create a test project
pub async fn create_test_project(pool: &PgPool, created_by: Uuid, name: Option<&str>) -> Uuid {
    let project_id = Uuid::new_v4();
    let name = name.unwrap_or("Test Project");

    sqlx::query(
        r#"
        INSERT INTO projects (id, name, description, created_by, metadata, architecture_config, require_review, project_type)
        VALUES ($1, $2, $3, $4, '{}'::jsonb, '{}'::jsonb, true, 'web')
        "#
    )
    .bind(project_id)
    .bind(name)
    .bind("Test Description")
    .bind(created_by)
    .execute(pool)
    .await
    .expect("Failed to create test project");

    // Add user as project member with owner role (using roles array)
    sqlx::query(
        r#"
        INSERT INTO project_members (project_id, user_id, roles)
        VALUES ($1, $2, ARRAY['owner']::project_role[])
        "#,
    )
    .bind(project_id)
    .bind(created_by)
    .execute(pool)
    .await
    .expect("Failed to add project member");

    project_id
}

/// Configure test system settings for a live Git provider host and PAT.
pub async fn configure_test_system_settings(pool: &PgPool, gitlab_url: &str, gitlab_pat: &str) {
    let service = SystemSettingsService::new(pool.clone())
        .expect("Failed to create SystemSettingsService for tests");

    service
        .update(UpdateSystemSettingsRequest {
            gitlab_url: Some(gitlab_url.to_string()),
            gitlab_pat: Some(gitlab_pat.to_string()),
            gitlab_auto_sync: None,
            agent_cli_provider: None,
            cloudflare_account_id: None,
            cloudflare_api_token: None,
            cloudflare_zone_id: None,
            cloudflare_base_domain: None,
            notifications_email_enabled: None,
            notifications_slack_enabled: None,
            notifications_slack_webhook_url: None,
            worktrees_path: None,
            preferred_agent_language: None,
        })
        .await
        .expect("Failed to configure test system settings");
}

/// Seed repository URL and repository_context JSON for a test project.
pub async fn seed_project_repository_context(
    pool: &PgPool,
    project_id: Uuid,
    repository_url: Option<&str>,
    repository_context: Value,
) {
    sqlx::query(
        r#"
        UPDATE projects
        SET repository_url = $2,
            repository_context = $3::jsonb
        WHERE id = $1
        "#,
    )
    .bind(project_id)
    .bind(repository_url)
    .bind(repository_context)
    .execute(pool)
    .await
    .expect("Failed to seed project repository context");
}

/// Create a test task
pub async fn create_test_task(
    pool: &PgPool,
    project_id: Uuid,
    created_by: Uuid,
    title: Option<&str>,
) -> Uuid {
    let task_id = Uuid::new_v4();
    let title = title.unwrap_or("Test Task");

    sqlx::query(
        r#"
        INSERT INTO tasks (id, project_id, title, description, task_type, status, created_by, metadata)
        VALUES ($1, $2, $3, $4, 'feature', 'todo', $5, '{}'::jsonb)
        "#
    )
    .bind(task_id)
    .bind(project_id)
    .bind(title)
    .bind("Test Task Description")
    .bind(created_by)
    .execute(pool)
    .await
    .expect("Failed to create test task");

    task_id
}

/// Create a test task attempt
pub async fn create_test_attempt(pool: &PgPool, task_id: Uuid, status: Option<&str>) -> Uuid {
    let attempt_id = Uuid::new_v4();
    let status = status.unwrap_or("queued");

    sqlx::query(
        r#"
        INSERT INTO task_attempts (id, task_id, status, metadata)
        VALUES ($1, $2, $3::attempt_status, '{}'::jsonb)
        "#,
    )
    .bind(attempt_id)
    .bind(task_id)
    .bind(status)
    .execute(pool)
    .await
    .expect("Failed to create test attempt");

    attempt_id
}

/// Cleanup test data
pub async fn cleanup_test_data(pool: &PgPool, user_id: Uuid, project_id: Option<Uuid>) {
    if let Some(pid) = project_id {
        delete_test_project(pool, pid).await;
    }

    let _ = sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await;

    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
}

/// Delete all test data associated with a single project.
pub async fn delete_test_project(pool: &PgPool, project_id: Uuid) {
    let _ = sqlx::query("DELETE FROM requirement_breakdown_sessions WHERE project_id = $1")
        .bind(project_id)
        .execute(pool)
        .await;

    let _ = sqlx::query("DELETE FROM project_assistant_sessions WHERE project_id = $1")
        .bind(project_id)
        .execute(pool)
        .await;

    let _ = sqlx::query("DELETE FROM requirements WHERE project_id = $1")
        .bind(project_id)
        .execute(pool)
        .await;

    let _ = sqlx::query("DELETE FROM tasks WHERE project_id = $1")
        .bind(project_id)
        .execute(pool)
        .await;

    let _ = sqlx::query("DELETE FROM project_members WHERE project_id = $1")
        .bind(project_id)
        .execute(pool)
        .await;

    let _ = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(project_id)
        .execute(pool)
        .await;
}

/// Make HTTP request to test router
pub async fn make_request(
    router: &Router,
    method: &str,
    path: &str,
    body: Option<&str>,
    headers: Vec<(&str, &str)>,
) -> (StatusCode, String) {
    make_request_with_string_headers(
        router,
        method,
        path,
        body,
        headers
            .into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect(),
    )
    .await
}

/// Make HTTP request to test router with String headers (for Bearer tokens)
pub async fn make_request_with_string_headers(
    router: &Router,
    method: &str,
    path: &str,
    body: Option<&str>,
    headers: Vec<(&str, String)>,
) -> (StatusCode, String) {
    use axum::http::Method;

    let method = match method {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "DELETE" => Method::DELETE,
        "PATCH" => Method::PATCH,
        _ => panic!("Unsupported method: {}", method),
    };

    let mut request_builder = Request::builder().method(method).uri(path);

    for (key, value) in headers {
        request_builder = request_builder.header(key, value.as_str());
    }

    let body = if let Some(b) = body {
        Body::from(b.to_string())
    } else {
        Body::empty()
    };

    let request = request_builder.body(body).expect("Failed to build request");

    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    let status = response.status();
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");
    let body_str =
        String::from_utf8(body_bytes.to_vec()).expect("Failed to convert body to string");

    (status, body_str)
}
