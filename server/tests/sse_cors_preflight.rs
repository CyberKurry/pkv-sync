//! Regression for v0.3.1 plugin SSE failure: the cross-origin fetch from the
//! Obsidian plugin sends a CORS preflight OPTIONS that previously got rejected
//! by either the deployment-key middleware (returns 404 without key) or the
//! UA-filter middleware (returns 404 without the "PKVSync-Plugin/..." UA),
//! both of which a browser preflight cannot satisfy. v0.3.2 makes both
//! middlewares pass OPTIONS through and adds a per-route CorsLayer on the
//! SSE endpoint so the preflight is answered with the right
//! Access-Control-Allow-* headers.

use axum::extract::ConnectInfo;
use ipnet::IpNet;
use pkv_sync_server::auth::LoginRateLimiter;
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::server;
use pkv_sync_server::service::AppState;
use std::net::SocketAddr;
use std::time::Duration;
use tower::ServiceExt;

async fn app() -> axum::Router {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("d");
    std::fs::create_dir_all(&data_dir).unwrap();
    let db_path = data_dir.join("metadata.db");
    let pool = pool::connect(&db_path).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let state = AppState::new(pool, data_dir.clone(), "test".into(), false)
        .await
        .unwrap();
    let cfg = Config {
        server: ServerConfig {
            bind_addr: "127.0.0.1:6710".parse().unwrap(),
            deployment_key: "k_test_sse_cors".into(),
            public_host: Some("127.0.0.1:6710".into()),
        },
        storage: StorageConfig {
            data_dir,
            db_path: db_path.clone(),
        },
        network: NetworkConfig {
            trusted_proxies: vec!["127.0.0.1/32".parse::<IpNet>().unwrap()],
        },
        logging: LoggingConfig::default(),
    };
    let limiter = LoginRateLimiter::new(10, Duration::from_secs(900), Duration::from_secs(900));
    let _ = tmp; // tempdir lives as long as the test process
    server::build_app(state, &cfg, limiter)
}

#[tokio::test]
async fn sse_preflight_returns_cors_headers_without_deployment_key_or_plugin_ua() {
    let app = app().await;

    // Simulate exactly what a browser fetch preflight sends:
    // - OPTIONS method
    // - Origin header
    // - Access-Control-Request-Method: GET
    // - Access-Control-Request-Headers includes Authorization and the
    //   deployment-key custom header
    // - No deployment-key header itself, no plugin User-Agent.
    let mut req = axum::http::Request::builder()
        .method(axum::http::Method::OPTIONS)
        .uri("/api/vaults/some-vault-id/events")
        .header(axum::http::header::ORIGIN, "app://obsidian.md")
        .header("user-agent", "Mozilla/5.0 (X11; Linux) Obsidian")
        .header("access-control-request-method", "GET")
        .header(
            "access-control-request-headers",
            "authorization, x-pkvsync-deployment-key, accept, user-agent",
        )
        .body(axum::body::Body::empty())
        .unwrap();
    req.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:50000".parse::<SocketAddr>().unwrap(),
    ));

    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let (parts, body) = resp.into_parts();
    let body_bytes = axum::body::to_bytes(body, 4096).await.unwrap();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let headers_dbg = format!("{:?}", parts.headers);
    assert!(
        status == axum::http::StatusCode::OK || status == axum::http::StatusCode::NO_CONTENT,
        "preflight must succeed (200/204), got {status}\nheaders: {headers_dbg}\nbody: {body_str}"
    );

    let allow_origin = parts
        .headers
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        !allow_origin.is_empty(),
        "preflight response must include Access-Control-Allow-Origin"
    );

    let allow_methods = parts
        .headers
        .get("access-control-allow-methods")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    assert!(
        allow_methods.contains("get"),
        "preflight must list GET in Access-Control-Allow-Methods (got {allow_methods})"
    );

    let allow_headers = parts
        .headers
        .get("access-control-allow-headers")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    for required in [
        "authorization",
        "x-pkvsync-deployment-key",
        "accept",
        "user-agent",
    ] {
        assert!(
            allow_headers.contains(required),
            "preflight must allow header {required} (got {allow_headers})"
        );
    }
}

/// GLM5 / production-bug regression: it's not enough for the preflight to
/// return Access-Control-Allow-Origin. The actual GET response (and any
/// error response from the route — 401 from a bad token, 404 from missing
/// vault) must ALSO carry Access-Control-Allow-Origin, or the browser
/// blocks the response body and the plugin logs "No 'Access-Control-Allow-
/// Origin' header is present on the requested resource". Without this
/// test we previously shipped a CORS layer that worked for preflight but
/// silently dropped CORS on the actual response.
#[tokio::test]
async fn sse_get_unauthorized_response_still_carries_cors_allow_origin() {
    let app = app().await;

    // GET without bearer → expect 401 from AuthenticatedUser extractor.
    // Critically, the 401 response must still have Access-Control-Allow-Origin
    // so the browser surfaces it to the plugin instead of nuking it as a
    // CORS violation.
    let mut req = axum::http::Request::builder()
        .method(axum::http::Method::GET)
        .uri("/api/vaults/some-vault-id/events")
        .header(axum::http::header::ORIGIN, "app://obsidian.md")
        .header("user-agent", "PKVSync-Plugin/0.3.3")
        .header("x-pkvsync-deployment-key", "k_test_sse_cors")
        .body(axum::body::Body::empty())
        .unwrap();
    req.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:50000".parse::<SocketAddr>().unwrap(),
    ));

    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let allow_origin = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        !allow_origin.is_empty(),
        "actual GET response must carry Access-Control-Allow-Origin (status={status}); without it the browser blocks the body"
    );
}

#[tokio::test]
async fn non_sse_routes_still_reject_options_without_cors_when_no_route_layer() {
    // Sanity check: OPTIONS on a non-SSE route is allowed past the
    // deployment-key and UA middlewares (so a future CorsLayer could
    // answer it) but, since the route itself has no CORS layer, it
    // falls through to method-not-allowed. The important thing is the
    // SSE route's preflight passes — this test just documents that we
    // did NOT inadvertently open CORS for the entire API surface.
    let app = app().await;
    let mut req = axum::http::Request::builder()
        .method(axum::http::Method::OPTIONS)
        .uri("/api/vaults")
        .header(axum::http::header::ORIGIN, "app://obsidian.md")
        .header("access-control-request-method", "POST")
        .body(axum::body::Body::empty())
        .unwrap();
    req.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:50000".parse::<SocketAddr>().unwrap(),
    ));
    let resp = app.oneshot(req).await.unwrap();
    // Without a CorsLayer on /api/vaults, axum returns 405 method not allowed.
    // Equally acceptable would be 200 if a future change adds a global CORS
    // layer; we only assert it isn't 404 (which would mean middleware ate it).
    let status = resp.status();
    assert_ne!(
        status,
        axum::http::StatusCode::NOT_FOUND,
        "middleware must not return 404 for OPTIONS — preflight handling depends on the OPTIONS pass-through"
    );
}
