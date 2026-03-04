use crate::middleware::{AuthUser, Permission, RbacChecker};
use crate::{error::ApiError, AppState};
use acpms_db::models::{PreviewInfo, ProjectType, SystemSettingsResponse};
use acpms_preview::{PreviewRuntimeLogs, PreviewRuntimeStatus};
use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::error;
use uuid::Uuid;

pub fn create_routes() -> Router<AppState> {
    Router::new()
        // Preview management
        .route(
            "/attempts/:id/preview",
            post(create_preview).get(get_preview_for_attempt),
        )
        .route(
            "/attempts/:id/preview/readiness",
            get(get_preview_readiness_for_attempt),
        )
        .route(
            "/attempts/:id/preview/runtime-status",
            get(get_preview_runtime_status_for_attempt),
        )
        .route(
            "/attempts/:id/preview/runtime-logs",
            get(get_preview_runtime_logs_for_attempt),
        )
        .route("/previews/:id", delete(cleanup_preview))
        .route("/previews", get(list_previews))
}

/// Create a preview environment for a task attempt
async fn create_preview(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(attempt_id): Path<Uuid>,
) -> Result<Json<PreviewInfo>, ApiError> {
    let attempt_context = load_attempt_context(&state, attempt_id).await?;

    RbacChecker::check_permission(
        auth_user.id,
        attempt_context.project_id,
        Permission::ExecuteTask,
        &state.db,
    )
    .await?;

    if !is_preview_supported_project_type(attempt_context.project_type) {
        return Err(ApiError::BadRequest(format!(
            "Preview not supported for project type '{}'",
            project_type_label(attempt_context.project_type)
        )));
    }

    if !attempt_context.preview_enabled {
        return Err(ApiError::BadRequest(
            "Preview is disabled in project settings".to_string(),
        ));
    }

    let settings = state
        .settings_service
        .get_response()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let missing_cloudflare_fields = missing_cloudflare_config_fields(&settings);
    if !missing_cloudflare_fields.is_empty() {
        return Err(ApiError::BadRequest(format!(
            "Preview unavailable: missing Cloudflare config: {}",
            missing_cloudflare_fields.join(", ")
        )));
    }
    if !state.preview_manager.runtime_enabled() {
        return Err(ApiError::BadRequest(
            "Preview unavailable: Docker preview runtime is disabled".to_string(),
        ));
    }

    if let Some(existing_preview) = get_existing_preview(&state, attempt_id).await? {
        // Start runtime in background; return preview URL immediately
        let pm = state.preview_manager.clone();
        let aid = attempt_id;
        let pt = attempt_context.project_type;
        tokio::spawn(async move {
            if let Err(e) = pm.start_preview_runtime(aid, pt).await {
                tracing::error!(
                    "Background preview runtime start failed for attempt {}: {}",
                    aid,
                    e
                );
            }
        });
        return Ok(Json(existing_preview));
    }

    let lock_key = preview_start_lock_key(attempt_id);
    let mut lock_connection = state
        .db
        .acquire()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query("SELECT pg_advisory_lock(hashtext($1))")
        .bind(&lock_key)
        .execute(&mut *lock_connection)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let preview_creation_result: Result<(PreviewInfo, bool), ApiError> =
        match get_existing_preview(&state, attempt_id).await {
            Ok(Some(existing_preview)) => Ok((existing_preview, false)),
            Ok(None) => match state
                .preview_manager
                .create_preview(attempt_id, &attempt_context.task_title)
                .await
            {
                Ok(created) => Ok((created, true)),
                Err(error) => Err(ApiError::Internal(error.to_string())),
            },
            Err(error) => Err(error),
        };

    if let Err(unlock_err) = sqlx::query("SELECT pg_advisory_unlock(hashtext($1))")
        .bind(&lock_key)
        .execute(&mut *lock_connection)
        .await
    {
        error!(
            "Failed to release preview start advisory lock for attempt {}: {}",
            attempt_id, unlock_err
        );
    }

    let (preview, preview_created_now) = preview_creation_result?;

    // Start runtime in background; return preview URL immediately (client can poll readiness)
    let pm = state.preview_manager.clone();
    let aid = attempt_id;
    let pt = attempt_context.project_type;
    tokio::spawn(async move {
        if let Err(e) = pm.start_preview_runtime(aid, pt).await {
            tracing::error!(
                "Background preview runtime start failed for attempt {}: {}",
                aid,
                e
            );
            if preview_created_now {
                if let Err(cleanup_e) = pm.cleanup_preview(aid).await {
                    tracing::warn!("Cleanup after runtime failure failed: {}", cleanup_e);
                }
            }
        }
    });

    Ok(Json(preview))
}

/// Get preview info for an attempt
async fn get_preview_for_attempt(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(attempt_id): Path<Uuid>,
) -> Result<Json<Option<PreviewInfo>>, ApiError> {
    let attempt_context = load_attempt_context(&state, attempt_id).await?;

    RbacChecker::check_permission(
        auth_user.id,
        attempt_context.project_id,
        Permission::ViewProject,
        &state.db,
    )
    .await?;

    let preview = get_existing_preview(&state, attempt_id).await?;
    Ok(Json(preview))
}

/// Get preview readiness for an attempt (project type + project settings + cloudflare config)
async fn get_preview_readiness_for_attempt(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(attempt_id): Path<Uuid>,
) -> Result<Json<PreviewReadinessResponse>, ApiError> {
    let attempt_context = load_attempt_context(&state, attempt_id).await?;

    RbacChecker::check_permission(
        auth_user.id,
        attempt_context.project_id,
        Permission::ViewProject,
        &state.db,
    )
    .await?;

    let preview_supported = is_preview_supported_project_type(attempt_context.project_type);
    let runtime_enabled = state.preview_manager.runtime_enabled();

    let settings = state
        .settings_service
        .get_response()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let missing_cloudflare_fields = if preview_supported && attempt_context.preview_enabled {
        missing_cloudflare_config_fields(&settings)
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let cloudflare_ready = missing_cloudflare_fields.is_empty();
    let ready =
        preview_supported && attempt_context.preview_enabled && cloudflare_ready && runtime_enabled;

    let reason = if !preview_supported {
        Some(format!(
            "Preview not supported for project type '{}'",
            project_type_label(attempt_context.project_type)
        ))
    } else if !attempt_context.preview_enabled {
        Some("Preview is disabled in project settings".to_string())
    } else if !runtime_enabled {
        Some("Preview unavailable: Docker preview runtime is disabled".to_string())
    } else if !cloudflare_ready {
        Some(format!(
            "Preview unavailable: missing Cloudflare config: {}",
            missing_cloudflare_fields.join(", ")
        ))
    } else {
        None
    };

    Ok(Json(PreviewReadinessResponse {
        attempt_id,
        project_type: project_type_label(attempt_context.project_type).to_string(),
        preview_supported,
        preview_enabled: attempt_context.preview_enabled,
        runtime_enabled,
        cloudflare_ready,
        ready,
        missing_cloudflare_fields,
        reason,
    }))
}

/// Get Docker runtime status for preview attempt.
async fn get_preview_runtime_status_for_attempt(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(attempt_id): Path<Uuid>,
) -> Result<Json<PreviewRuntimeStatus>, ApiError> {
    let attempt_context = load_attempt_context(&state, attempt_id).await?;

    RbacChecker::check_permission(
        auth_user.id,
        attempt_context.project_id,
        Permission::ViewProject,
        &state.db,
    )
    .await?;

    let status = state
        .preview_manager
        .get_preview_runtime_status(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(status))
}

#[derive(Debug, Deserialize)]
struct PreviewRuntimeLogsQuery {
    tail: Option<u32>,
}

/// Get Docker runtime logs for preview attempt (debug/ops).
async fn get_preview_runtime_logs_for_attempt(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(attempt_id): Path<Uuid>,
    Query(query): Query<PreviewRuntimeLogsQuery>,
) -> Result<Json<PreviewRuntimeLogs>, ApiError> {
    let attempt_context = load_attempt_context(&state, attempt_id).await?;

    RbacChecker::check_permission(
        auth_user.id,
        attempt_context.project_id,
        Permission::ViewProject,
        &state.db,
    )
    .await?;

    let logs = state
        .preview_manager
        .get_preview_runtime_logs(attempt_id, query.tail)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(logs))
}

/// List all active previews
async fn list_previews(
    auth_user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<PreviewInfo>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    let previews = state
        .preview_manager
        .list_active_previews()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(previews))
}

/// Cleanup a preview environment (manual delete)
/// DB soft-delete runs inline (fast); Docker + Cloudflare cleanup runs in background.
/// Returns 200 on success (or 404 if not found). Errors from mark_preview_deleted are returned to client.
async fn cleanup_preview(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(preview_identifier): Path<Uuid>,
) -> Result<(), ApiError> {
    #[derive(sqlx::FromRow)]
    struct PreviewAccessContext {
        attempt_id: Uuid,
        project_id: Uuid,
    }

    let access_context = sqlx::query_as::<_, PreviewAccessContext>(
        r#"
        SELECT ta.id AS attempt_id, t.project_id
        FROM task_attempts ta
        JOIN tasks t ON t.id = ta.task_id
        WHERE ta.id = $1
        UNION ALL
        SELECT ta.id AS attempt_id, t.project_id
        FROM cloudflare_tunnels ct
        JOIN task_attempts ta ON ta.id = ct.attempt_id
        JOIN tasks t ON t.id = ta.task_id
        WHERE ct.id = $1
          AND ct.deleted_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(preview_identifier)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    let access_context =
        access_context.ok_or_else(|| ApiError::NotFound("Preview not found".to_string()))?;

    RbacChecker::check_permission(
        auth_user.id,
        access_context.project_id,
        Permission::ManageProject,
        &state.db,
    )
    .await?;

    // DB soft-delete inline (fast); return errors to client
    let resources = state
        .preview_manager
        .mark_preview_deleted(access_context.attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Docker + Cloudflare cleanup in background (best effort)
    if let Some((tunnel_id, dns_record_id)) = resources {
        let pm = state.preview_manager.clone();
        let attempt_id = access_context.attempt_id;
        tokio::spawn(async move {
            pm.cleanup_preview_resources(attempt_id, tunnel_id, dns_record_id)
                .await;
        });
    }

    Ok(())
}

fn is_preview_supported_project_type(project_type: ProjectType) -> bool {
    matches!(
        project_type,
        ProjectType::Web | ProjectType::Api | ProjectType::Microservice
    )
}

fn project_type_label(project_type: ProjectType) -> &'static str {
    match project_type {
        ProjectType::Web => "web",
        ProjectType::Mobile => "mobile",
        ProjectType::Desktop => "desktop",
        ProjectType::Extension => "extension",
        ProjectType::Api => "api",
        ProjectType::Microservice => "microservice",
    }
}

fn preview_start_lock_key(attempt_id: Uuid) -> String {
    format!("preview_start:{attempt_id}")
}

fn missing_cloudflare_config_fields(settings: &SystemSettingsResponse) -> Vec<&'static str> {
    let mut missing = Vec::new();

    if settings
        .cloudflare_account_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .is_none()
    {
        missing.push("cloudflare_account_id");
    }

    if !settings.cloudflare_api_token_configured {
        missing.push("cloudflare_api_token");
    }

    if settings
        .cloudflare_zone_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .is_none()
    {
        missing.push("cloudflare_zone_id");
    }

    if settings
        .cloudflare_base_domain
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .is_none()
    {
        missing.push("cloudflare_base_domain");
    }

    missing
}

#[derive(sqlx::FromRow)]
struct AttemptContext {
    project_id: Uuid,
    task_title: String,
    project_type: ProjectType,
    preview_enabled: bool,
}

#[derive(Debug, Serialize)]
struct PreviewReadinessResponse {
    attempt_id: Uuid,
    project_type: String,
    preview_supported: bool,
    preview_enabled: bool,
    runtime_enabled: bool,
    cloudflare_ready: bool,
    ready: bool,
    missing_cloudflare_fields: Vec<String>,
    reason: Option<String>,
}

async fn load_attempt_context(
    state: &AppState,
    attempt_id: Uuid,
) -> Result<AttemptContext, ApiError> {
    let attempt_context = sqlx::query_as::<_, AttemptContext>(
        r#"
        SELECT
            t.project_id,
            t.title AS task_title,
            p.project_type,
            (
                COALESCE((p.settings->>'auto_deploy')::boolean, false)
                OR COALESCE((p.settings->>'preview_enabled')::boolean, true)
            ) AS preview_enabled
        FROM task_attempts ta
        JOIN tasks t ON t.id = ta.task_id
        JOIN projects p ON p.id = t.project_id
        WHERE ta.id = $1
        "#,
    )
    .bind(attempt_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    attempt_context.ok_or_else(|| ApiError::NotFound("Attempt not found".to_string()))
}

async fn get_existing_preview(
    state: &AppState,
    attempt_id: Uuid,
) -> Result<Option<PreviewInfo>, ApiError> {
    state
        .preview_manager
        .get_preview(attempt_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_settings(
        account_id: Option<&str>,
        token_configured: bool,
        zone_id: Option<&str>,
        base_domain: Option<&str>,
    ) -> SystemSettingsResponse {
        SystemSettingsResponse {
            gitlab_url: "https://gitlab.example.com".to_string(),
            gitlab_pat_configured: false,
            gitlab_auto_sync: false,
            agent_cli_provider: "openai-codex".to_string(),
            cloudflare_account_id: account_id.map(ToString::to_string),
            cloudflare_api_token_configured: token_configured,
            cloudflare_zone_id: zone_id.map(ToString::to_string),
            cloudflare_base_domain: base_domain.map(ToString::to_string),
            notifications_email_enabled: false,
            notifications_slack_enabled: false,
            notifications_slack_webhook_url: None,
            worktrees_path: "./worktrees".to_string(),
            preferred_agent_language: "en".to_string(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn missing_cloudflare_fields_reports_all_required_fields() {
        let settings = make_settings(None, false, None, None);
        assert_eq!(
            missing_cloudflare_config_fields(&settings),
            vec![
                "cloudflare_account_id",
                "cloudflare_api_token",
                "cloudflare_zone_id",
                "cloudflare_base_domain"
            ]
        );
    }

    #[test]
    fn missing_cloudflare_fields_treats_blank_values_as_missing() {
        let settings = make_settings(Some(" "), true, Some(""), Some("   "));
        assert_eq!(
            missing_cloudflare_config_fields(&settings),
            vec![
                "cloudflare_account_id",
                "cloudflare_zone_id",
                "cloudflare_base_domain"
            ]
        );
    }

    #[test]
    fn missing_cloudflare_fields_accepts_complete_settings() {
        let settings = make_settings(
            Some("acc_123"),
            true,
            Some("zone_123"),
            Some("preview.example.com"),
        );
        assert!(missing_cloudflare_config_fields(&settings).is_empty());
    }

    #[test]
    fn preview_support_project_type_matrix_is_correct() {
        assert!(is_preview_supported_project_type(ProjectType::Web));
        assert!(is_preview_supported_project_type(ProjectType::Api));
        assert!(is_preview_supported_project_type(ProjectType::Microservice));
        assert!(!is_preview_supported_project_type(ProjectType::Extension));
        assert!(!is_preview_supported_project_type(ProjectType::Mobile));
        assert!(!is_preview_supported_project_type(ProjectType::Desktop));
    }

    #[test]
    fn preview_start_lock_key_is_deterministic() {
        let attempt_id = Uuid::parse_str("12345678-1234-5678-9abc-def012345678").unwrap();
        assert_eq!(
            preview_start_lock_key(attempt_id),
            "preview_start:12345678-1234-5678-9abc-def012345678"
        );
    }
}
