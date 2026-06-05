use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;
use subtle::ConstantTimeEq;

use super::cors_aware_reject;

pub const HEADER: &str = "x-pkvsync-deployment-key";

#[derive(Clone)]
pub struct DeploymentKey(pub Arc<String>);

impl DeploymentKey {
    pub fn new(key: String) -> Self {
        Self(Arc::new(key))
    }
}

pub async fn middleware(
    State(expected): State<DeploymentKey>,
    req: Request,
    next: Next,
) -> Response {
    // CORS preflight requests never carry custom headers (the browser sends
    // them as OPTIONS with Origin and Access-Control-Request-* only), so
    // they cannot supply the deployment key. Let them pass so the
    // downstream CorsLayer on the SSE route can respond with the correct
    // Access-Control-Allow-* headers. The real request that follows the
    // preflight is still checked here.
    if req.method() == Method::OPTIONS {
        return next.run(req).await;
    }
    let supplied = req
        .headers()
        .get(HEADER)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let a = supplied.as_bytes();
    let b = expected.0.as_bytes();
    if a.len() == b.len() && bool::from(a.ct_eq(b)) {
        return next.run(req).await;
    }
    cors_aware_reject(&req, StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{HeaderValue, Request as HttpRequest, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn app(key: &str) -> Router {
        Router::new().route("/", get(|| async { "ok" })).layer(
            axum::middleware::from_fn_with_state(DeploymentKey::new(key.to_string()), middleware),
        )
    }

    fn req(key: Option<&str>) -> HttpRequest<Body> {
        let mut b = HttpRequest::builder().uri("/");
        if let Some(k) = key {
            b = b.header(HEADER, HeaderValue::from_str(k).unwrap());
        }
        b.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn allows_correct_key() {
        let resp = app("k_abc").oneshot(req(Some("k_abc"))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_wrong_key() {
        let resp = app("k_abc").oneshot(req(Some("k_wrong"))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn rejects_missing_header() {
        let resp = app("k_abc").oneshot(req(None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn rejects_empty_string() {
        let resp = app("k_abc").oneshot(req(Some(""))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
