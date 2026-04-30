use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use once_cell::sync::Lazy;
use regex::Regex;

/// Pattern PKV Sync plugin UAs must match.
static PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^PKVSync-Plugin/\d+\.\d+\.\d+\b").expect("valid UA regex"));

pub async fn middleware(req: Request, next: Next) -> Response {
    let ok = req
        .headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|ua| PATTERN.is_match(ua))
        .unwrap_or(false);
    if !ok {
        return StatusCode::NOT_FOUND.into_response();
    }
    next.run(req).await
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
}
