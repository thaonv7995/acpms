//! S3 Reverse Proxy: forwards /{bucket}/*path to MinIO for presigned URL support.
//! Presigned URLs are generated with path /bucket/key (no /s3 prefix) so the signature
//! matches when we forward with the same path. We preserve the original Host header so
//! MinIO's signature validation succeeds.

use axum::{
    body::Body,
    extract::{Path, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use reqwest::Client;
use std::env;

pub async fn s3_proxy_handler(Path(path): Path<String>, req: Request) -> impl IntoResponse {
    let s3_internal_url =
        env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:9000".to_string());
    let bucket = env::var("S3_BUCKET_NAME").unwrap_or_else(|_| "acpms-media".to_string());

    // Preserve Host from the original request so MinIO's signature validation (which uses Host + path) succeeds.
    let host_header = req
        .headers()
        .get("Host")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let query_string = req
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();
    let target_url = format!(
        "{}/{}/{}{}",
        s3_internal_url.trim_end_matches('/'),
        bucket,
        path,
        query_string
    );
    let method = req.method().clone();

    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Failed to read request body: {}", e),
            )
                .into_response()
        }
    };

    let client = Client::new();
    let mut proxy_req = client
        .request(method.clone(), &target_url)
        .body(body_bytes.to_vec());
    if let Some(h) = host_header {
        proxy_req = proxy_req.header("Host", h);
    }
    let res = proxy_req.send().await;

    match res {
        Ok(response) => {
            let mut builder = Response::builder().status(response.status());
            for (key, value) in response.headers().iter() {
                let k = key.as_str().to_lowercase();
                if k != "transfer-encoding" && k != "connection" {
                    builder = builder.header(key, value);
                }
            }
            match response.bytes().await {
                Ok(bytes) => match builder.body(Body::from(bytes)) {
                    Ok(r) => r.into_response(),
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Stream error: {}", e),
                    )
                        .into_response(),
                },
                Err(e) => (
                    StatusCode::BAD_GATEWAY,
                    format!("Failed to read MinIO response: {}", e),
                )
                    .into_response(),
            }
        }
        Err(e) => (StatusCode::BAD_GATEWAY, format!("MinIO unreachable: {}", e)).into_response(),
    }
}
