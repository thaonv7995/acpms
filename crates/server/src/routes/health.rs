use axum::{extract::State, http::StatusCode, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub components: HashMap<String, ComponentHealth>,
}

/// Basic health check - always returns healthy if service is running
#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    responses(
        (status = 200, description = "Service is running", body = HealthResponse)
    )
)]
pub async fn health_check() -> Json<HealthResponse> {
    let mut components = HashMap::new();
    components.insert(
        "service".to_string(),
        ComponentHealth {
            status: HealthStatus::Healthy,
            message: Some("Service is running".to_string()),
            latency_ms: None,
        },
    );

    Json(HealthResponse {
        status: HealthStatus::Healthy,
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: Utc::now(),
        components,
    })
}

/// Readiness check - checks if service can handle requests
/// Verifies database and other critical dependencies
#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "Health",
    responses(
        (status = 200, description = "Service is ready", body = HealthResponse),
        (status = 503, description = "Service is not ready", body = HealthResponse)
    )
)]
pub async fn readiness_check(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let mut components = HashMap::new();

    // Check database connectivity
    let db_start = Instant::now();
    let db_status = match sqlx::query("SELECT 1").fetch_one(&state.db).await {
        Ok(_) => ComponentHealth {
            status: HealthStatus::Healthy,
            latency_ms: Some(db_start.elapsed().as_millis() as u64),
            message: Some("Database connection OK".to_string()),
        },
        Err(e) => ComponentHealth {
            status: HealthStatus::Unhealthy,
            latency_ms: None,
            message: Some(format!("Database error: {}", e)),
        },
    };
    components.insert("database".to_string(), db_status);

    // Check worker pool status
    if let Some(worker_pool) = &state.worker_pool {
        let queue_depth = worker_pool.queue_depth();
        let worker_status = if queue_depth < 100 {
            HealthStatus::Healthy
        } else if queue_depth < 500 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        };

        components.insert(
            "worker_queue".to_string(),
            ComponentHealth {
                status: worker_status,
                latency_ms: None,
                message: Some(format!("Queue depth: {}", queue_depth)),
            },
        );
    }

    // Determine overall status
    let overall = if components
        .values()
        .any(|c| matches!(c.status, HealthStatus::Unhealthy))
    {
        HealthStatus::Unhealthy
    } else if components
        .values()
        .any(|c| matches!(c.status, HealthStatus::Degraded))
    {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    let status_code = match overall {
        HealthStatus::Healthy => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK,
        HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
    };

    (
        status_code,
        Json(HealthResponse {
            status: overall,
            version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp: Utc::now(),
            components,
        }),
    )
}

/// Liveness check - checks if service is alive (for Kubernetes)
/// Returns healthy if the service can respond
#[utoipa::path(
    get,
    path = "/health/live",
    tag = "Health",
    responses(
        (status = 200, description = "Service is alive", body = HealthResponse)
    )
)]
pub async fn liveness_check() -> Json<HealthResponse> {
    let mut components = HashMap::new();
    components.insert(
        "service".to_string(),
        ComponentHealth {
            status: HealthStatus::Healthy,
            message: Some("Service is alive".to_string()),
            latency_ms: None,
        },
    );

    Json(HealthResponse {
        status: HealthStatus::Healthy,
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: Utc::now(),
        components,
    })
}
