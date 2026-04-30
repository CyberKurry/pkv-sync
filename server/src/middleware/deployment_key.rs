use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use subtle::ConstantTimeEq;

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
    StatusCode::NOT_FOUND.into_response()
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
