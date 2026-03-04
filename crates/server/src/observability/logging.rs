use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize structured logging based on environment
pub fn init_logging() {
    let env = std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if env == "production" {
            "acpms_server=info,acpms_executors=info,tower_http=info".into()
        } else {
            "acpms_server=debug,acpms_executors=debug,acpms_services=debug,tower_http=debug".into()
        }
    });

    if env == "production" {
        // JSON structured logging for production
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().json())
            .init();
    } else {
        // Pretty logging for development
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().pretty())
            .init();
    }

    tracing::info!("Logging initialized for environment: {}", env);
}

/// Request ID for correlation
pub mod request_id {
    use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
    use uuid::Uuid;

    pub const REQUEST_ID_HEADER: &str = "x-request-id";

    /// Middleware to add request ID to all requests
    pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
        // Get or generate request ID
        let request_id = req
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // Add request ID to extensions for use in handlers
        req.extensions_mut().insert(request_id.clone());

        // Process request
        let mut response = next.run(req).await;

        // Add request ID to response headers
        if let Ok(header_value) = HeaderValue::from_str(&request_id) {
            response
                .headers_mut()
                .insert(REQUEST_ID_HEADER, header_value);
        }

        response
    }

    /// Extract request ID from request extensions
    #[allow(dead_code)]
    pub fn get_request_id(extensions: &axum::http::Extensions) -> Option<String> {
        extensions.get::<String>().cloned()
    }
}
