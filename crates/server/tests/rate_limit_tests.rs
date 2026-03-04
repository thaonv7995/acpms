use acpms_server::middleware::rate_limit::auth_rate_limiter;
use axum::{body::Body, http::Request, routing::post, Router};
use tower::ServiceExt;

async fn ok_handler() -> &'static str {
    "ok"
}

fn build_request(ip: &str) -> Request<Body> {
    Request::builder()
        .uri("/auth/login")
        .method("POST")
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("request should build")
}

#[tokio::test]
async fn auth_rate_limiter_blocks_after_burst_limit() {
    let app = Router::new()
        .route("/auth/login", post(ok_handler))
        .layer(auth_rate_limiter());

    for _ in 0..5 {
        let response = app
            .clone()
            .oneshot(build_request("198.51.100.10"))
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    let response = app
        .clone()
        .oneshot(build_request("198.51.100.10"))
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn auth_rate_limiter_tracks_ips_independently() {
    let app = Router::new()
        .route("/auth/login", post(ok_handler))
        .layer(auth_rate_limiter());

    for _ in 0..5 {
        let response = app
            .clone()
            .oneshot(build_request("198.51.100.11"))
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    let blocked = app
        .clone()
        .oneshot(build_request("198.51.100.11"))
        .await
        .expect("request should succeed");
    assert_eq!(blocked.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);

    let different_ip = app
        .clone()
        .oneshot(build_request("198.51.100.12"))
        .await
        .expect("request should succeed");
    assert_eq!(different_ip.status(), axum::http::StatusCode::OK);
}
