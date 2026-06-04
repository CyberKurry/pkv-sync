use axum::extract::Request;
use axum::http::{header, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use regex::Regex;
use std::sync::LazyLock;

use super::{SSE_CORS_ALLOW_HEADERS, SSE_PLUGIN_HEADER};

/// Pattern PKV Sync plugin UAs must match.
static PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^PKVSync-Plugin/\d+\.\d+\.\d+\b").expect("valid UA regex"));

/// If the request targets the SSE events endpoint and carries a cross-origin
/// `Origin` header, the rejection response must include CORS headers;
/// otherwise the browser blocks it and the Obsidian plugin reports a
/// generic "Failed to fetch" / CORS error instead of a useful status.
fn cors_aware_reject(req: &Request, status: StatusCode) -> Response {
    let mut resp = status.into_response();
    if req.uri().path().ends_with("/events") && req.headers().get(header::ORIGIN).is_some() {
        let h = resp.headers_mut();
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
        h.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            "GET, OPTIONS".parse().unwrap(),
        );
        h.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            SSE_CORS_ALLOW_HEADERS.parse().unwrap(),
        );
    }
    resp
}

pub async fn middleware(req: Request, next: Next) -> Response {
    if ua_exempt_path(req.uri().path()) {
        return next.run(req).await;
    }
    // Browser CORS preflight requests carry the browser's own User-Agent
    // (e.g. "Mozilla/..."), not the plugin's "PKVSync-Plugin/X.Y.Z". Let
    // them through so the downstream CorsLayer on the SSE route can answer
    // the preflight. The actual request is validated below by plugin UA or
    // by the SSE-only plugin identity header.
    if req.method() == Method::OPTIONS {
        return next.run(req).await;
    }
    let ua_ok = req
        .headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|ua| PATTERN.is_match(ua))
        .unwrap_or(false);
    let plugin_header_ok = req.uri().path().ends_with("/events")
        && req
            .headers()
            .get(SSE_PLUGIN_HEADER)
            .and_then(|h| h.to_str().ok())
            .map(|ua| PATTERN.is_match(ua))
            .unwrap_or(false);
    let ok = ua_ok || plugin_header_ok;
    if !ok {
        return cors_aware_reject(&req, StatusCode::NOT_FOUND);
    }
    next.run(req).await
}

fn ua_exempt_path(path: &str) -> bool {
    matches!(path, "/api/health" | "/metrics")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{HeaderValue, Request as HttpRequest, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn app() -> Router {
        Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(middleware))
    }

    fn req(ua: Option<&str>) -> HttpRequest<Body> {
        let mut b = HttpRequest::builder().uri("/");
        if let Some(u) = ua {
            b = b.header("user-agent", HeaderValue::from_str(u).unwrap());
        }
        b.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn allows_valid_ua() {
        let resp = app()
            .oneshot(req(Some("PKVSync-Plugin/0.1.0 (...)")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_curl() {
        let resp = app().oneshot(req(Some("curl/8.4.0"))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn rejects_missing_ua() {
        let resp = app().oneshot(req(None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn rejects_wrong_format() {
        let resp = app().oneshot(req(Some("PKVSync-Plugin"))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn allows_health_and_metrics_without_plugin_ua() {
        let app = Router::new()
            .route("/api/health", get(|| async { "health" }))
            .route("/metrics", get(|| async { "metrics" }))
            .layer(axum::middleware::from_fn(middleware));

        let health = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let metrics = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(health.status(), StatusCode::OK);
        assert_eq!(metrics.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn sse_events_requires_plugin_ua() {
        let app = Router::new()
            .route("/api/vaults/:id/events", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(middleware));
        let req_no_ua = HttpRequest::builder()
            .uri("/api/vaults/abc/events")
            .header("user-agent", "Mozilla/5.0")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req_no_ua).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn sse_events_allows_plugin_ua() {
        let app = Router::new()
            .route("/api/vaults/:id/events", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(middleware));
        let req_with_ua = HttpRequest::builder()
            .uri("/api/vaults/abc/events")
            .header("user-agent", "PKVSync-Plugin/0.3.3")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req_with_ua).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
