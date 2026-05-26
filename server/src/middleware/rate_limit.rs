use crate::auth::token;
use crate::middleware::real_ip::ClientIp;
use axum::extract::{MatchedPath, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use dashmap::DashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const WINDOW_SECS: u64 = 60;
const PRUNE_INTERVAL: Duration = Duration::from_secs(60);
pub const SYNC_API_REQUESTS_PER_WINDOW: u32 = 600;
pub const MCP_HTTP_REQUESTS_PER_WINDOW: u32 = 120;

#[derive(Clone)]
pub struct RequestRateLimiter {
    inner: Arc<DashMap<String, Window>>,
    max_requests: u32,
    window: Duration,
    last_pruned: Arc<Mutex<Instant>>,
}

#[derive(Clone)]
struct Window {
    started: Instant,
    count: u32,
}

impl RequestRateLimiter {
    pub fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            max_requests: max_requests.max(1),
            window,
            last_pruned: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub fn sync_api() -> Self {
        Self::new(
            SYNC_API_REQUESTS_PER_WINDOW,
            Duration::from_secs(WINDOW_SECS),
        )
    }

    pub fn mcp_http() -> Self {
        Self::new(
            MCP_HTTP_REQUESTS_PER_WINDOW,
            Duration::from_secs(WINDOW_SECS),
        )
    }

    pub fn check(&self, key: String) -> Result<(), Duration> {
        let now = Instant::now();
        self.prune_stale_if_due(now);
        let mut entry = self.inner.entry(key).or_insert(Window {
            started: now,
            count: 0,
        });
        let elapsed = now.duration_since(entry.started);
        if elapsed >= self.window {
            entry.started = now;
            entry.count = 0;
        }
        if entry.count >= self.max_requests {
            return Err(self.window.saturating_sub(elapsed));
        }
        entry.count += 1;
        Ok(())
    }

    pub fn prune_stale(&self) -> usize {
        let now = Instant::now();
        self.prune_stale_at(now)
    }

    fn prune_stale_if_due(&self, now: Instant) {
        let Ok(mut last_pruned) = self.last_pruned.try_lock() else {
            return;
        };
        if now.duration_since(*last_pruned) < PRUNE_INTERVAL {
            return;
        }
        *last_pruned = now;
        drop(last_pruned);
        self.prune_stale_at(now);
    }

    fn prune_stale_at(&self, now: Instant) -> usize {
        let stale = self
            .inner
            .iter()
            .filter_map(|entry| {
                (now.duration_since(entry.started) >= self.window).then(|| entry.key().clone())
            })
            .collect::<Vec<_>>();
        let removed = stale.len();
        for key in stale {
            self.inner.remove(&key);
        }
        removed
    }

    #[cfg(test)]
    fn entry_count_for_tests(&self) -> usize {
        self.inner.len()
    }
}

pub async fn rest_middleware(
    State(limiter): State<RequestRateLimiter>,
    req: Request,
    next: Next,
) -> Response {
    let key = request_key("sync_api", &req);
    if limiter.check(key).is_err() {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": {
                    "code": "rate_limited",
                    "message": "too many requests"
                }
            })),
        )
            .into_response();
    }
    next.run(req).await
}

pub fn request_key(scope: &str, req: &Request) -> String {
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or_else(|| req.uri().path());
    let ip = req
        .extensions()
        .get::<ClientIp>()
        .map(|ip| ip.0.to_string())
        .unwrap_or_else(|| "unknown".into());
    let bearer = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(token::hash)
        .unwrap_or_else(|| "anonymous".into());
    format!("{}:{}:{}:{}:{}", scope, req.method(), route, ip, bearer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    #[test]
    fn limiter_rejects_after_window_budget() {
        let limiter = RequestRateLimiter::new(1, Duration::from_secs(60));

        assert!(limiter.check("k".into()).is_ok());
        assert!(limiter.check("k".into()).is_err());
        assert!(limiter.check("other".into()).is_ok());
    }

    #[tokio::test]
    async fn rest_middleware_returns_429_after_budget() {
        let limiter = RequestRateLimiter::new(1, Duration::from_secs(60));
        let app = Router::new().route("/", get(|| async { "ok" })).layer(
            axum::middleware::from_fn_with_state(limiter, rest_middleware),
        );
        let req = || HttpRequest::builder().uri("/").body(Body::empty()).unwrap();

        let first = app.clone().oneshot(req()).await.unwrap();
        let second = app.oneshot(req()).await.unwrap();

        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn prune_stale_removes_expired_windows_without_touching_active_entries() {
        let limiter = RequestRateLimiter::new(10, Duration::from_millis(20));
        limiter.check("expired".into()).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        limiter.check("active".into()).unwrap();

        assert_eq!(limiter.entry_count_for_tests(), 2);
        assert_eq!(limiter.prune_stale(), 1);
        assert_eq!(limiter.entry_count_for_tests(), 1);
        assert!(limiter.check("expired".into()).is_ok());
    }
}
