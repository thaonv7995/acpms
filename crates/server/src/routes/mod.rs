pub mod agent;
pub mod agent_activity;
pub mod approvals;
pub mod auth;
pub mod health;
pub mod openclaw;
pub mod openclaw_admin;
pub mod project_assistant;
pub mod project_documents;
pub mod projects;
pub mod streams;
pub mod task_attempts;
pub mod task_contexts;
pub mod tasks;
pub mod users;
pub mod websocket;

use crate::{handlers, middleware, AppState};
use axum::{
    extract::DefaultBodyLimit,
    http::{header::ACCEPT, HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Redirect, Response},
    routing::{any, delete, get, patch, post, put},
    Router,
};
use std::path::Path;
use tower_http::services::{ServeDir, ServeFile};

pub mod dashboard;
pub mod deployments;
pub mod execution_processes;
pub mod gitlab;
pub mod preview;
pub mod requirement_breakdowns;
pub mod requirements;
pub mod reviews;
pub mod settings;
pub mod sprints;
pub mod templates;

#[path = "gitlab-oauth.rs"]
pub mod gitlab_oauth;

#[path = "webhooks-admin.rs"]
pub mod webhooks_admin;

fn should_enable_dev_frontend_redirect() -> bool {
    match std::env::var("APP_ENV") {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            normalized != "production"
        }
        Err(_) => true,
    }
}

fn build_dev_frontend_redirect_target(base: &str, uri: &Uri) -> String {
    let trimmed = base.trim().trim_end_matches('/');
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    format!("{}{}", trimmed, path_and_query)
}

async fn dev_frontend_fallback(method: Method, uri: Uri, headers: HeaderMap) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::NOT_FOUND.into_response();
    }

    let path = uri.path();
    if path.starts_with("/api/") || path.starts_with("/ws/") {
        return StatusCode::NOT_FOUND.into_response();
    }

    let accepts_html = headers
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.contains("text/html") || value.contains("*/*"))
        .unwrap_or(true);

    if !accepts_html {
        return StatusCode::NOT_FOUND.into_response();
    }

    let frontend_url = std::env::var("ACPMS_DEV_FRONTEND_URL")
        .unwrap_or_else(|_| "http://localhost:5173".to_string());
    let target = build_dev_frontend_redirect_target(&frontend_url, &uri);
    Redirect::temporary(&target).into_response()
}

pub(crate) fn build_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/refresh", post(auth::refresh_token))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/revoke/:user_id", post(auth::revoke_user_tokens))
        .layer(middleware::rate_limit::auth_rate_limiter())
}

pub(crate) fn build_business_api_routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(users::list_users).post(users::create_user))
        .route(
            "/users/:id",
            get(users::get_user)
                .put(users::update_user)
                .delete(users::delete_user),
        )
        .route("/users/:id/password", put(users::change_password))
        .route(
            "/users/avatar/upload-url",
            post(users::get_avatar_upload_url),
        )
        // Dashboard routes
        .route("/dashboard", get(dashboard::get_dashboard))
        // Projects routes
        .route(
            "/projects",
            get(projects::list_projects).post(projects::create_project),
        )
        .route(
            "/projects/import/preflight",
            post(projects::import_project_preflight),
        )
        .route(
            "/projects/import/create-fork",
            post(projects::import_project_create_fork),
        )
        .route("/projects/import", post(projects::import_project))
        .route(
            "/projects/init-refs/upload-url",
            post(projects::get_init_ref_upload_url),
        )
        .route(
            "/projects/:id",
            get(projects::get_project)
                .put(projects::update_project)
                .delete(projects::delete_project),
        )
        .route(
            "/projects/:id/repository-context/recheck",
            post(projects::recheck_project_repository_access),
        )
        .route(
            "/projects/:id/repository-context/link-fork",
            post(projects::link_existing_fork),
        )
        .route(
            "/projects/:id/repository-context/create-fork",
            post(projects::create_project_fork),
        )
        .route(
            "/projects/:id/sync",
            post(projects::sync_project_repository),
        )
        .route(
            "/projects/:id/assistant/attachments/upload-url",
            post(project_assistant::get_assistant_attachment_upload_url),
        )
        .route(
            "/projects/:project_id/documents",
            get(project_documents::list_project_documents)
                .post(project_documents::create_or_upsert_project_document),
        )
        .route(
            "/projects/:project_id/documents/upload-url",
            post(project_documents::get_project_document_upload_url),
        )
        .route(
            "/projects/:project_id/documents/download-url",
            post(project_documents::get_project_document_download_url),
        )
        .route(
            "/projects/:project_id/documents/:document_id",
            get(project_documents::get_project_document)
                .patch(project_documents::update_project_document)
                .delete(project_documents::delete_project_document),
        )
        .route(
            "/projects/:id/assistant/sessions",
            post(project_assistant::create_session).get(project_assistant::list_sessions),
        )
        .route(
            "/projects/:id/assistant/sessions/:session_id",
            get(project_assistant::get_session),
        )
        .route(
            "/projects/:id/assistant/sessions/:session_id/start",
            post(project_assistant::start_session),
        )
        .route(
            "/projects/:id/assistant/sessions/:session_id/status",
            get(project_assistant::get_session_status),
        )
        .route(
            "/projects/:id/assistant/sessions/:session_id/messages",
            post(project_assistant::post_message),
        )
        .route(
            "/projects/:id/assistant/sessions/:session_id/input",
            post(project_assistant::post_input),
        )
        .route(
            "/projects/:id/assistant/sessions/:session_id/confirm-tool",
            post(project_assistant::confirm_tool),
        )
        .route(
            "/projects/:id/assistant/sessions/:session_id/end",
            post(project_assistant::end_session),
        )
        .route(
            "/projects/:id/inviteable-users",
            get(projects::list_inviteable_users),
        )
        .route(
            "/projects/:id/members",
            get(projects::list_project_members).post(projects::add_project_member),
        )
        .route(
            "/projects/:id/members/:user_id",
            put(projects::update_project_member).delete(projects::remove_project_member),
        )
        .route(
            "/projects/:id/architecture",
            get(projects::get_architecture).put(projects::update_architecture),
        )
        // Project Settings routes
        .route(
            "/projects/:id/settings",
            get(projects::get_project_settings).put(projects::update_project_settings),
        )
        .route(
            "/projects/:id/settings/:key",
            patch(projects::update_single_project_setting),
        )
        // Sprints routes
        .route(
            "/projects/:project_id/sprints",
            get(sprints::list_project_sprints).post(sprints::create_sprint),
        )
        .route(
            "/projects/:project_id/sprints/generate",
            post(sprints::generate_sprints),
        )
        .route(
            "/projects/:project_id/sprints/active",
            get(sprints::get_active_sprint),
        )
        .route(
            "/projects/:project_id/sprints/:sprint_id/activate",
            post(sprints::activate_sprint),
        )
        .route(
            "/projects/:project_id/sprints/:sprint_id/close",
            post(sprints::close_sprint),
        )
        .route(
            "/projects/:project_id/sprints/:sprint_id/overview",
            get(sprints::get_sprint_overview),
        )
        .route(
            "/projects/:project_id/sprints/:sprint_id",
            get(sprints::get_sprint)
                .put(sprints::update_sprint)
                .delete(sprints::delete_sprint),
        )
        // Requirements routes
        .route(
            "/projects/:project_id/requirements",
            get(requirements::list_project_requirements).post(requirements::create_requirement),
        )
        .route(
            "/projects/:project_id/requirements/attachments/upload-url",
            post(requirements::get_requirement_attachment_upload_url),
        )
        .route(
            "/projects/:project_id/requirements/attachments/download-url",
            post(requirements::get_requirement_attachment_download_url),
        )
        .route(
            "/projects/:project_id/requirements/:id",
            get(requirements::get_requirement)
                .put(requirements::update_requirement)
                .delete(requirements::delete_requirement),
        )
        .route(
            "/projects/:project_id/requirements/:requirement_id/breakdown/start",
            post(requirement_breakdowns::start_requirement_breakdown),
        )
        .route(
            "/projects/:project_id/requirements/:requirement_id/breakdown/manual/confirm",
            post(requirement_breakdowns::confirm_requirement_breakdown_manual),
        )
        .route(
            "/projects/:project_id/requirements/:requirement_id/tasks/start-sequential",
            post(requirement_breakdowns::start_requirement_task_sequence),
        )
        .route(
            "/projects/:project_id/requirements/:requirement_id/breakdown/:session_id",
            get(requirement_breakdowns::get_requirement_breakdown_session),
        )
        .route(
            "/projects/:project_id/requirements/:requirement_id/breakdown/:session_id/confirm",
            post(requirement_breakdowns::confirm_requirement_breakdown),
        )
        .route(
            "/projects/:project_id/requirements/:requirement_id/breakdown/:session_id/cancel",
            post(requirement_breakdowns::cancel_requirement_breakdown),
        )
        // Tasks routes
        .route(
            "/tasks/attachments/upload-url",
            post(tasks::get_task_attachment_upload_url),
        )
        .route("/tasks", get(tasks::list_tasks).post(tasks::create_task))
        .route(
            "/tasks/:id",
            get(tasks::get_task)
                .put(tasks::update_task)
                .delete(tasks::delete_task),
        )
        .route("/tasks/:id/status", put(tasks::update_task_status))
        .route("/tasks/:id/children", get(tasks::get_task_children))
        .route("/tasks/:id/assign", post(tasks::assign_task))
        .route("/tasks/:id/metadata", put(tasks::update_task_metadata))
        .route(
            "/tasks/:task_id/contexts",
            get(task_contexts::list_task_contexts).post(task_contexts::create_task_context),
        )
        .route(
            "/tasks/:task_id/contexts/:context_id",
            patch(task_contexts::update_task_context).delete(task_contexts::delete_task_context),
        )
        .route(
            "/tasks/:task_id/context-attachments/upload-url",
            post(task_contexts::get_task_context_attachment_upload_url),
        )
        .route(
            "/tasks/:task_id/contexts/:context_id/attachments",
            post(task_contexts::create_task_context_attachment),
        )
        .route(
            "/tasks/:task_id/contexts/:context_id/attachments/:attachment_id",
            delete(task_contexts::delete_task_context_attachment),
        )
        .route(
            "/tasks/:task_id/context-attachments/download-url",
            post(task_contexts::get_task_context_attachment_download_url),
        )
        // Task Attempts routes
        .route(
            "/tasks/:task_id/attempts",
            get(task_attempts::get_task_attempts).post(task_attempts::create_task_attempt),
        )
        .route(
            "/tasks/:task_id/attempts/from-edit",
            post(task_attempts::create_task_attempt_from_edit),
        )
        .route("/attempts/:id", get(task_attempts::get_attempt))
        .route(
            "/attempts/:id/skills",
            get(task_attempts::get_attempt_skills),
        )
        .route("/attempts/:id/logs", get(task_attempts::get_attempt_logs))
        .route(
            "/attempts/:id/logs/:log_id",
            patch(task_attempts::patch_attempt_log),
        )
        .route(
            "/attempts/:id/processes",
            get(task_attempts::get_attempt_execution_processes),
        )
        .route(
            "/execution-processes",
            get(execution_processes::list_execution_processes),
        )
        .route(
            "/execution-processes/:id",
            get(execution_processes::get_execution_process),
        )
        .route(
            "/execution-processes/:id/raw-logs",
            get(execution_processes::get_execution_process_raw_logs),
        )
        .route(
            "/execution-processes/:id/normalized-logs",
            get(execution_processes::get_execution_process_normalized_logs),
        )
        .route(
            "/execution-processes/:id/follow-up",
            post(execution_processes::follow_up_execution_process),
        )
        .route(
            "/execution-processes/:id/reset",
            post(execution_processes::reset_execution_process),
        )
        // Structured logs and subagent tree
        .route(
            "/attempts/:id/structured-logs",
            get(task_attempts::get_structured_logs),
        )
        .route(
            "/attempts/:id/subagent-tree",
            get(task_attempts::get_subagent_tree),
        )
        // SSE JSON Patch streaming (Phase 3)
        .route("/attempts/:id/stream", get(streams::stream_attempt_sse))
        .route(
            "/attempts/:id/input",
            post(task_attempts::send_attempt_input),
        )
        .route(
            "/projects/:project_id/agents/active",
            get(websocket::get_project_active_agents),
        )
        .route(
            "/attempts/:id/cancel",
            post(task_attempts::cancel_attempt).layer(middleware::rate_limit::api_rate_limiter()),
        )
        // Retry routes
        .route("/attempts/:id/retry", post(task_attempts::retry_attempt))
        .route(
            "/attempts/:id/retry-info",
            get(task_attempts::get_retry_info),
        )
        // Review flow routes
        .route("/attempts/:id/diff", get(task_attempts::get_attempt_diff))
        .route("/attempts/:id/diffs", get(task_attempts::get_attempt_diff)) // Alias for frontend compatibility
        .route(
            "/attempts/:id/diff-summary",
            get(task_attempts::get_attempt_diff_summary),
        )
        .route(
            "/attempts/:id/branch-status",
            get(task_attempts::get_branch_status),
        )
        .route(
            "/attempts/:id/approve",
            post(task_attempts::approve_attempt).layer(middleware::rate_limit::api_rate_limiter()),
        )
        .route(
            "/attempts/:id/reject",
            post(task_attempts::reject_attempt).layer(middleware::rate_limit::api_rate_limiter()),
        )
        .route("/attempts/:id/rebase", post(task_attempts::rebase_attempt))
        // Review comments routes
        .route(
            "/attempts/:id/comments",
            get(reviews::list_comments).post(reviews::add_comment),
        )
        .route(
            "/attempts/:id/request-changes",
            post(reviews::request_changes).layer(middleware::rate_limit::api_rate_limiter()),
        )
        .route("/comments/:id", delete(reviews::delete_comment))
        .route("/comments/:id/resolve", patch(reviews::resolve_comment))
        .route("/comments/:id/unresolve", patch(reviews::unresolve_comment))
        // Agent Activity routes (global dashboard)
        .route(
            "/agent-activity/status",
            get(agent_activity::get_agent_status),
        )
        .route("/agent-activity/logs", get(agent_activity::get_agent_logs))
        // Agent Provider routes
        .route("/agent/status", get(agent::get_agent_status))
        .route("/agent/providers/status", get(agent::get_provider_statuses))
        .route("/agent/auth/initiate", post(agent::initiate_agent_auth))
        .route(
            "/agent/auth/submit-code",
            post(agent::submit_agent_auth_code),
        )
        .route("/agent/auth/cancel", post(agent::cancel_agent_auth))
        .route(
            "/agent/auth/sessions/:id",
            get(agent::get_agent_auth_session),
        )
        // GitLab routes
        .merge(gitlab::create_routes())
        // GitLab OAuth routes
        .merge(gitlab_oauth::create_routes())
        // Settings routes
        .merge(settings::create_routes())
        // Preview routes
        .merge(preview::create_routes())
        // Templates routes
        .merge(templates::create_routes())
        // Deployment routes
        .merge(deployments::create_routes())
        // Admin routes
        .nest(
            "/admin",
            webhooks_admin::create_routes().merge(openclaw_admin::create_routes()),
        )
        // Approval routes (SDK mode)
        .route(
            "/execution-processes/:id/approvals/pending",
            get(approvals::get_pending_approvals_for_process),
        )
        .route(
            "/approvals/:approval_ref/respond",
            post(approvals::respond_to_approval),
        )
}

fn build_api_websocket_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:id/assistant/sessions/:session_id/logs/ws",
            get(websocket::assistant_logs_ws_handler),
        )
        .route("/attempts/:id/logs/ws", get(websocket::ws_handler))
        .route(
            "/execution-processes/:id/raw-logs/ws",
            get(websocket::execution_process_raw_logs_ws_handler),
        )
        .route(
            "/execution-processes/:id/normalized-logs/ws",
            get(websocket::execution_process_normalized_logs_ws_handler),
        )
        .route(
            "/execution-processes/stream/attempt/ws",
            get(websocket::execution_processes_ws_handler),
        )
        .route(
            "/execution-processes/stream/session/ws",
            get(websocket::execution_processes_session_ws_handler),
        )
        .route("/ws/attempts/:id/logs", get(websocket::ws_handler))
        .route(
            "/ws/projects/:project_id/agents",
            get(websocket::project_ws_handler),
        )
        .route(
            "/ws/agent-activity/status",
            get(websocket::agent_activity_ws_handler),
        )
        .route(
            "/attempts/:id/stream/ws",
            get(websocket::attempt_stream_ws_handler),
        )
        .route(
            "/projects/:project_id/agents/ws",
            get(websocket::project_ws_handler),
        )
        .route(
            "/agent/auth/sessions/:id/ws",
            get(websocket::agent_auth_session_ws_handler),
        )
        .route("/approvals/stream/ws", get(websocket::approvals_ws_handler))
}

fn build_root_ws_routes() -> Router<AppState> {
    Router::new()
        .route("/attempts/:id/logs", get(websocket::ws_handler))
        .route("/attempts/:id/diffs", get(websocket::ws_handler))
        .route(
            "/projects/:project_id/agents",
            get(websocket::project_ws_handler),
        )
        .route(
            "/agent-activity/status",
            get(websocket::agent_activity_ws_handler),
        )
}

fn build_openclaw_ws_routes() -> Router<AppState> {
    Router::new()
        .route("/attempts/:id/logs", get(websocket::openclaw_ws_handler))
        .route("/attempts/:id/diffs", get(websocket::openclaw_ws_handler))
        .route(
            "/attempts/:id/stream",
            get(websocket::openclaw_attempt_stream_ws_handler),
        )
        .route(
            "/projects/:project_id/assistant/sessions/:session_id/logs",
            get(websocket::openclaw_assistant_logs_ws_handler),
        )
        .route(
            "/projects/:project_id/agents",
            get(websocket::openclaw_project_ws_handler),
        )
        .route(
            "/agent-activity/status",
            get(websocket::openclaw_agent_activity_ws_handler),
        )
        .route(
            "/execution-processes/:id/raw-logs",
            get(websocket::openclaw_execution_process_raw_logs_ws_handler),
        )
        .route(
            "/execution-processes/:id/normalized-logs",
            get(websocket::openclaw_execution_process_normalized_logs_ws_handler),
        )
        .route(
            "/execution-processes/stream/attempt",
            get(websocket::openclaw_execution_processes_ws_handler),
        )
        .route(
            "/execution-processes/stream/session",
            get(websocket::openclaw_execution_processes_session_ws_handler),
        )
        .route(
            "/agent/auth/sessions/:id",
            get(websocket::openclaw_agent_auth_session_ws_handler),
        )
        .route(
            "/approvals/stream",
            get(websocket::openclaw_approvals_ws_handler),
        )
}

pub fn create_router(state: AppState) -> Router {
    let health_routes = Router::new()
        .route("/health", get(health::health_check))
        .route("/health/ready", get(health::readiness_check))
        .route("/health/live", get(health::liveness_check))
        .with_state(state.clone());

    let api_routes = build_business_api_routes()
        .merge(build_auth_routes())
        .merge(build_api_websocket_routes())
        .with_state(state.clone());

    let openclaw_routes = openclaw::create_router(state.clone());
    let openclaw_ws_routes = build_openclaw_ws_routes().with_state(state.clone());

    let ws_routes = build_root_ws_routes().with_state(state.clone());

    // S3 proxy: path /{bucket}/*path (e.g. /acpms-media/avatars/...) so presigned URL path matches forwarded path; no body limit.
    let s3_bucket = std::env::var("S3_BUCKET_NAME").unwrap_or_else(|_| "acpms-media".to_string());
    let s3_routes = Router::new()
        .route(
            &format!("/{}/*path", s3_bucket),
            any(handlers::s3_proxy::s3_proxy_handler),
        )
        .layer(DefaultBodyLimit::disable());

    let mut app = Router::new()
        .merge(health_routes)
        .nest("/api/v1", api_routes)
        .nest("/api/openclaw", openclaw_routes)
        .nest("/api/openclaw/ws", openclaw_ws_routes)
        .nest("/ws", ws_routes)
        .merge(s3_routes);

    // Static file serving (Single Binary mode): fallback for SPA
    let frontend_dir =
        std::env::var("ACPMS_FRONTEND_DIR").unwrap_or_else(|_| "./frontend/dist".to_string());
    let frontend_path = Path::new(&frontend_dir);
    let index_path = frontend_path.join("index.html");
    if frontend_path.exists() && frontend_path.is_dir() && index_path.is_file() {
        // Use `fallback` (not `not_found_service`) so SPA routes return 200 with index.html.
        let serve_dir = ServeDir::new(frontend_path).fallback(ServeFile::new(index_path));
        app = app.fallback_service(serve_dir);
    } else if should_enable_dev_frontend_redirect() {
        tracing::info!(
            frontend_dir,
            index_path = %index_path.display(),
            "Frontend dist missing/invalid; enabling dev frontend fallback redirect"
        );
        app = app.fallback(any(dev_frontend_fallback));
    }

    app
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    use uuid::Uuid;

    #[test]
    fn build_dev_frontend_redirect_target_keeps_path_and_query() {
        let uri: Uri = "/projects?tab=settings".parse().expect("valid uri");
        let target = build_dev_frontend_redirect_target("http://localhost:5173/", &uri);
        assert_eq!(target, "http://localhost:5173/projects?tab=settings");
    }

    #[test]
    fn build_dev_frontend_redirect_target_defaults_to_root() {
        let uri: Uri = "/".parse().expect("valid uri");
        let target = build_dev_frontend_redirect_target("http://localhost:5173", &uri);
        assert_eq!(target, "http://localhost:5173/");
    }

    #[tokio::test]
    async fn spa_static_fallback_returns_ok_for_client_routes() {
        let temp_dir = std::env::temp_dir().join(format!("acpms-spa-fallback-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let index_path = temp_dir.join("index.html");
        std::fs::write(&index_path, "<!doctype html><html><body>ok</body></html>")
            .expect("write index file");

        let service = ServeDir::new(&temp_dir).fallback(ServeFile::new(&index_path));
        let response = service
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/projects/eccd0980-0bdd-449f-a7e8-18f4cdc447ee")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("serve response");

        assert_eq!(response.status(), StatusCode::OK);

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
