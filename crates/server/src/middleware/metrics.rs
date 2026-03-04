use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::time::Instant;

use crate::observability::Metrics;

/// Middleware to collect HTTP request metrics
pub async fn metrics_middleware(
    State(metrics): State<Metrics>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().to_string();

    // Increment in-flight requests
    metrics.http_requests_in_flight.inc();

    // Start timing
    let start = Instant::now();

    // Process request
    let response = next.run(req).await;

    // Record duration
    let duration = start.elapsed().as_secs_f64();
    metrics
        .http_request_duration_seconds
        .with_label_values(&[&method, &path])
        .observe(duration);

    // Record request count
    let status = response.status().as_u16().to_string();
    metrics
        .http_requests_total
        .with_label_values(&[&method, &path, &status])
        .inc();

    // Decrement in-flight requests
    metrics.http_requests_in_flight.dec();

    response
}

/// Metrics endpoint handler
#[allow(dead_code)]
pub async fn metrics_handler(State(metrics): State<Metrics>) -> Result<String, StatusCode> {
    metrics
        .encode()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
