use governor::middleware::NoOpMiddleware;
use std::sync::Arc;
use tower_governor::{
    errors::GovernorError,
    governor::{GovernorConfig, GovernorConfigBuilder},
    key_extractor::{KeyExtractor, SmartIpKeyExtractor},
    GovernorLayer,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafeSmartIpKeyExtractor;

impl KeyExtractor for SafeSmartIpKeyExtractor {
    type Key = String;

    fn extract<T>(&self, req: &axum::http::Request<T>) -> Result<Self::Key, GovernorError> {
        if let Ok(ip) = SmartIpKeyExtractor.extract(req) {
            return Ok(ip.to_string());
        }

        // Fallback key prevents auth/login endpoints from returning 500
        // when client IP cannot be resolved (e.g. local tests or missing proxy headers).
        Ok("unknown-client".to_string())
    }
}

fn build_governor_config(
    per_millisecond: u64,
    burst_size: u32,
) -> Arc<GovernorConfig<SafeSmartIpKeyExtractor, NoOpMiddleware>> {
    let mut primary_builder =
        GovernorConfigBuilder::default().key_extractor(SafeSmartIpKeyExtractor);
    primary_builder.per_millisecond(per_millisecond);
    primary_builder.burst_size(burst_size);

    if let Some(config) = primary_builder.finish() {
        return Arc::new(config);
    }

    tracing::error!(
        "Failed to build rate limiter config ({}ms/{}) - using default fallback",
        per_millisecond,
        burst_size
    );

    let mut fallback_builder =
        GovernorConfigBuilder::default().key_extractor(SafeSmartIpKeyExtractor);
    if let Some(config) = fallback_builder.finish() {
        return Arc::new(config);
    }

    let mut emergency_builder =
        GovernorConfigBuilder::default().key_extractor(SafeSmartIpKeyExtractor);
    emergency_builder.per_millisecond(1);
    emergency_builder.burst_size(1);

    match emergency_builder.finish() {
        Some(config) => Arc::new(config),
        None => {
            panic!("tower_governor failed to build emergency rate-limit config");
        }
    }
}

/// Create rate limiting layer for authentication endpoints
/// 5 requests per minute per IP address
#[allow(dead_code)]
pub fn auth_rate_limiter() -> GovernorLayer<SafeSmartIpKeyExtractor, NoOpMiddleware> {
    let governor_conf = build_governor_config(12000, 5); // 5 requests per minute

    GovernorLayer {
        config: governor_conf,
    }
}

/// Create rate limiting layer for general API endpoints
/// 100 requests per minute per IP address
pub fn api_rate_limiter() -> GovernorLayer<SafeSmartIpKeyExtractor, NoOpMiddleware> {
    let governor_conf = build_governor_config(600, 100); // 100 requests per minute

    GovernorLayer {
        config: governor_conf,
    }
}
