use crate::middleware::{AuthUser, RbacChecker};
use crate::routes::agent;
use crate::{api::ApiResponse, error::ApiError, AppState};
use acpms_deployment::cloudflare::CloudflareClient;
use acpms_db::models::{SystemSettingsResponse, UpdateSystemSettingsRequest};
use acpms_services::{cloudflare_token_looks_masked_or_corrupted, CloudflareConfigOverrides};
use axum::{extract::State, routing::{get, post}, Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub fn create_routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(get_settings).put(update_settings))
        .route("/settings/cloudflare/check", post(check_cloudflare_settings))
}

#[derive(Debug, Deserialize)]
pub struct CloudflareConnectionCheckRequest {
    pub cloudflare_account_id: Option<String>,
    pub cloudflare_api_token: Option<String>,
    pub cloudflare_zone_id: Option<String>,
    pub cloudflare_base_domain: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CloudflareConnectionCheckResponse {
    pub status: String,
    pub ok: bool,
    pub config_complete: bool,
    pub connection_ok: bool,
    pub tunnel_create_ok: bool,
    pub dns_record_ok: Option<bool>,
    pub cleanup_ok: bool,
    pub missing_fields: Vec<String>,
    pub message: String,
    pub details: Vec<String>,
    pub checked_at: String,
    pub preview_url_example: Option<String>,
}

/// Get current system settings (safe DTO)
async fn get_settings(
    auth_user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<SystemSettingsResponse>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    let settings = state
        .settings_service
        .get_response()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(ApiResponse::success(
        settings,
        "Settings retrieved successfully",
    )))
}

#[utoipa::path(
    put,
    path = "/api/v1/settings",
    request_body = UpdateSystemSettingsRequest,
    responses(
        (status = 200, description = "Update system settings", body = ApiResponse<SystemSettingsResponse>),
    ),
    tag = "Settings"
)]
pub async fn update_settings(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Json(mut payload): Json<UpdateSystemSettingsRequest>,
) -> Result<Json<ApiResponse<SystemSettingsResponse>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    if let Some(provider) = payload.agent_cli_provider.take() {
        let normalized = agent::normalize_agent_cli_provider(&provider);
        let status = agent::check_provider_status(&normalized).await;
        if !status.available {
            return Err(ApiError::BadRequest(format!(
                "Cannot set default provider '{}' because it is not available: {}",
                normalized, status.message
            )));
        }
        payload.agent_cli_provider = Some(normalized);
    }

    let had_worktrees_update = payload.worktrees_path.is_some();
    let settings = state
        .settings_service
        .update(payload)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Sync worktrees_path to in-memory state so it applies immediately (no restart)
    if had_worktrees_update {
        let new_path = PathBuf::from(&settings.worktrees_path);
        *state.worktrees_path.write().await = new_path;
    }

    Ok(Json(ApiResponse::success(
        settings,
        "Settings updated successfully",
    )))
}

pub async fn check_cloudflare_settings(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<CloudflareConnectionCheckRequest>,
) -> Result<Json<ApiResponse<CloudflareConnectionCheckResponse>>, ApiError> {
    RbacChecker::check_system_admin(auth_user.id, &state.db).await?;

    let resolved = state
        .settings_service
        .resolve_cloudflare_config(CloudflareConfigOverrides {
            account_id: payload.cloudflare_account_id,
            api_token: payload.cloudflare_api_token,
            zone_id: payload.cloudflare_zone_id,
            base_domain: payload.cloudflare_base_domain,
        })
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut missing_fields = Vec::new();
    if resolved.account_id.is_none() {
        missing_fields.push("cloudflare_account_id".to_string());
    }
    if resolved.api_token.is_none() {
        missing_fields.push("cloudflare_api_token".to_string());
    }
    if resolved.zone_id.is_none() {
        missing_fields.push("cloudflare_zone_id".to_string());
    }
    if resolved.base_domain.is_none() {
        missing_fields.push("cloudflare_base_domain".to_string());
    }

    if !missing_fields.is_empty() {
        return Ok(Json(ApiResponse::success(
            CloudflareConnectionCheckResponse {
                status: "warning".to_string(),
                ok: false,
                config_complete: false,
                connection_ok: false,
                tunnel_create_ok: false,
                dns_record_ok: None,
                cleanup_ok: true,
                missing_fields,
                message: "Cloudflare config is incomplete.".to_string(),
                details: vec![
                    "Fill in Account ID, API Token, Zone ID, and Base Domain before running the check."
                        .to_string(),
                ],
                checked_at: Utc::now().to_rfc3339(),
                preview_url_example: None,
            },
            "Cloudflare check completed",
        )));
    }

    let account_id = resolved.account_id.unwrap_or_default();
    let api_token = resolved.api_token.unwrap_or_default();
    let zone_id = resolved.zone_id.unwrap_or_default();
    let base_domain = resolved.base_domain.unwrap_or_default();

    if cloudflare_token_looks_masked_or_corrupted(&api_token) {
        return Ok(Json(ApiResponse::success(
            CloudflareConnectionCheckResponse {
                status: "error".to_string(),
                ok: false,
                config_complete: true,
                connection_ok: false,
                tunnel_create_ok: false,
                dns_record_ok: None,
                cleanup_ok: true,
                missing_fields: Vec::new(),
                message: "Stored Cloudflare API token is corrupted.".to_string(),
                details: vec![
                    "The saved token contains masked bullet characters (`••••`). Re-enter and save the raw Cloudflare API token in Settings."
                        .to_string(),
                ],
                checked_at: Utc::now().to_rfc3339(),
                preview_url_example: None,
            },
            "Cloudflare check completed",
        )));
    }

    let cloudflare = match CloudflareClient::new(api_token, account_id) {
        Ok(client) => client,
        Err(error) => {
            return Ok(Json(ApiResponse::success(
                CloudflareConnectionCheckResponse {
                    status: "error".to_string(),
                    ok: false,
                    config_complete: true,
                    connection_ok: false,
                    tunnel_create_ok: false,
                    dns_record_ok: None,
                    cleanup_ok: true,
                    missing_fields: Vec::new(),
                    message: "Failed to initialize Cloudflare client.".to_string(),
                    details: vec![error.to_string()],
                    checked_at: Utc::now().to_rfc3339(),
                    preview_url_example: None,
                },
                "Cloudflare check completed",
            )));
        }
    };

    let mut details = vec![
        "Using temporary tunnel creation as the primary capability check.".to_string(),
    ];

    let probe_suffix = Uuid::new_v4().simple().to_string();
    let short_suffix = &probe_suffix[..8];
    let tunnel_name = format!("acpms-settings-probe-{}", short_suffix);

    let credentials = match cloudflare.create_tunnel(&tunnel_name).await {
        Ok(credentials) => credentials,
        Err(error) => {
            let error_text = error.to_string();
            let normalized = error_text.to_ascii_lowercase();
            let mut error_details = vec![error_text];
            if normalized.contains("invalid request headers") {
                error_details.push(
                    "Hint: paste the raw Cloudflare API token only. Do not include `Bearer ` or extra whitespace/newlines."
                        .to_string(),
                );
            }
            if normalized.contains("status 403") || normalized.contains("9109") {
                error_details.push(
                    "The token was accepted by Cloudflare but is not authorized for this resource. Grant Account > Cloudflare Tunnel > Edit and Zone > DNS > Edit, and scope the token to the same account and zone used in Settings."
                        .to_string(),
                );
            }
            return Ok(Json(ApiResponse::success(
                CloudflareConnectionCheckResponse {
                    status: "error".to_string(),
                    ok: false,
                    config_complete: true,
                    connection_ok: normalized.contains("status 400")
                        || normalized.contains("status 401")
                        || normalized.contains("status 403"),
                    tunnel_create_ok: false,
                    dns_record_ok: None,
                    cleanup_ok: true,
                    missing_fields: Vec::new(),
                    message: "Cloudflare tunnel creation failed.".to_string(),
                    details: error_details,
                    checked_at: Utc::now().to_rfc3339(),
                    preview_url_example: None,
                },
                "Cloudflare check completed",
            )));
        }
    };
    details.push("Cloudflare API token can create a tunnel successfully.".to_string());
    details.push(format!(
        "Temporary tunnel `{}` was created successfully.",
        tunnel_name
    ));

    let probe_subdomain = format!("acpms-probe-{}", short_suffix);
    let dns_target = format!("{}.cfargotunnel.com", credentials.tunnel_id);
    let probe_preview_url = format!("https://{}.{}", probe_subdomain, base_domain);
    let mut dns_record_id: Option<String> = None;
    let dns_record_ok;
    let mut status = "success".to_string();
    let mut ok = true;
    let mut message = "Cloudflare connection, tunnel creation, and DNS probe succeeded."
        .to_string();

    match cloudflare
        .create_dns_record(&zone_id, &probe_subdomain, &dns_target, "CNAME", true)
        .await
    {
        Ok(record_id) => {
            dns_record_id = Some(record_id);
            dns_record_ok = Some(true);
            details.push(format!(
                "Temporary DNS record `{}` was created successfully.",
                probe_preview_url
            ));
        }
        Err(error) => {
            dns_record_ok = Some(false);
            status = "warning".to_string();
            ok = false;
            message =
                "Cloudflare account and tunnel are valid, but DNS record creation failed."
                    .to_string();
            details.push(error.to_string());
        }
    }

    let mut cleanup_ok = true;
    if let Some(record_id) = dns_record_id.as_deref() {
        if let Err(error) = cloudflare.delete_dns_record(&zone_id, record_id).await {
            cleanup_ok = false;
            details.push(format!(
                "Warning: failed to delete probe DNS record during cleanup: {}",
                error
            ));
        } else {
            details.push("Temporary DNS probe was cleaned up successfully.".to_string());
        }
    }

    if let Err(error) = cloudflare.delete_tunnel(&credentials.tunnel_id).await {
        cleanup_ok = false;
        details.push(format!(
            "Warning: failed to delete probe tunnel during cleanup: {}",
            error
        ));
    } else {
        details.push("Temporary Cloudflare tunnel was cleaned up successfully.".to_string());
    }

    Ok(Json(ApiResponse::success(
        CloudflareConnectionCheckResponse {
            status,
            ok,
            config_complete: true,
            connection_ok: true,
            tunnel_create_ok: true,
            dns_record_ok,
            cleanup_ok,
            missing_fields: Vec::new(),
            message,
            details,
            checked_at: Utc::now().to_rfc3339(),
            preview_url_example: Some(probe_preview_url),
        },
        "Cloudflare check completed",
    )))
}
