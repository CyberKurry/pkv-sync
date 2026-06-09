use crate::auth::AuthenticatedUser;
use crate::db::repos::{TokenRepo, UserRepo};
use crate::middleware::deployment_key;
use crate::middleware::real_ip::TrustedProxies;
use crate::service::AppState;
use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::Value;
use std::collections::VecDeque;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{Interval, MissedTickBehavior};
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tokio_stream::StreamExt;

use super::transport_stdio::{
    authenticate_token, handle_jsonrpc, jsonrpc_error, GENERIC_MCP_AUTH_ERROR,
};

#[derive(Clone, Debug)]
struct McpAuthLimitKey(String);

const MCP_JSON_BODY_OVERHEAD_BYTES: u64 = 1024 * 1024;
const MCP_JSON_BODY_LIMIT_CEILING_BYTES: u64 = 100 * 1024 * 1024;

fn mcp_json_body_limit_bytes(max_file_size: u64) -> usize {
    max_file_size
        .saturating_mul(6)
        .saturating_add(MCP_JSON_BODY_OVERHEAD_BYTES)
        .min(MCP_JSON_BODY_LIMIT_CEILING_BYTES)
        .try_into()
        .unwrap_or(usize::MAX / 2)
}

fn mcp_auth_error_public_message(message: &str) -> &str {
    if message.starts_with("rate_limited:") {
        message
    } else {
        GENERIC_MCP_AUTH_ERROR
    }
}

fn is_json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .is_some_and(|value| {
            value.eq_ignore_ascii_case("application/json")
                || value.rsplit_once('/').is_some_and(|(_, subtype)| {
                    subtype.eq_ignore_ascii_case("json")
                        || subtype
                            .rsplit_once('+')
                            .is_some_and(|(_, suffix)| suffix.eq_ignore_ascii_case("json"))
                })
        })
}

pub fn router(state: AppState, deployment_key: String) -> Router {
    router_with_rate_limiter(
        state,
        crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
        deployment_key,
    )
}

/// Wraps `router` with the `real_ip` middleware so that standalone MCP HTTP
/// deployments behind a reverse proxy resolve the actual client IP before
/// rate-limit keys are computed. The embedded path (embed_in_serve=true)
/// already receives the real_ip layer from the outer application router and
/// must NOT call this function.
pub fn standalone_router(
    state: AppState,
    deployment_key: String,
    trusted_proxies: TrustedProxies,
) -> Router {
    router(state, deployment_key).layer(axum::middleware::from_fn_with_state(
        trusted_proxies,
        crate::middleware::real_ip::middleware,
    ))
}

fn router_with_rate_limiter(
    state: AppState,
    limiter: crate::middleware::rate_limit::RequestRateLimiter,
    deployment_key_value: String,
) -> Router {
    Router::new()
        .route("/mcp", post(post_mcp).get(get_mcp_sse))
        .route_layer(axum::middleware::from_fn_with_state(
            limiter,
            mcp_rate_limit,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            deployment_key::DeploymentKey::new(deployment_key_value),
            deployment_key::middleware,
        ))
        .with_state(state)
}

pub async fn run(
    state: AppState,
    bind: SocketAddr,
    deployment_key: String,
    trusted_proxies: TrustedProxies,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(
        listener,
        standalone_router(state, deployment_key, trusted_proxies)
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

async fn post_mcp(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Extension(auth_limit_key): axum::extract::Extension<McpAuthLimitKey>,
    body: Body,
) -> Response {
    if !is_json_content_type(&headers) {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    let max_file_size = state.runtime_cfg.snapshot().await.max_file_size;
    let body = match to_bytes(body, mcp_json_body_limit_bytes(max_file_size)).await {
        Ok(body) => body,
        Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    };
    let request = match serde_json::from_slice::<Value>(&body) {
        Ok(request) => request,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(jsonrpc_error(Value::Null, -32700, "parse error")),
            )
                .into_response();
        }
    };
    let Some(raw) = bearer(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(jsonrpc_error(Value::Null, -32001, "missing bearer token")),
        )
            .into_response();
    };
    let user = match authenticate_token(&state, raw, &auth_limit_key.0).await {
        Ok(user) => user,
        Err(err) => {
            let message = err.to_string();
            return (
                StatusCode::UNAUTHORIZED,
                Json(jsonrpc_error(
                    Value::Null,
                    -32001,
                    mcp_auth_error_public_message(&message),
                )),
            )
                .into_response();
        }
    };
    Json(handle_jsonrpc(&state, &user, None, request).await).into_response()
}

async fn mcp_rate_limit(
    State(limiter): State<crate::middleware::rate_limit::RequestRateLimiter>,
    mut req: Request,
    next: Next,
) -> Response {
    let key = crate::middleware::rate_limit::request_key("mcp_http", &req);
    if limiter.check(key).is_err() {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(jsonrpc_error(Value::Null, -32029, "too many requests")),
        )
            .into_response();
    }
    let auth_key = crate::middleware::rate_limit::request_key("mcp_auth", &req);
    req.extensions_mut().insert(McpAuthLimitKey(auth_key));
    next.run(req).await
}

async fn get_mcp_sse(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Extension(auth_limit_key): axum::extract::Extension<McpAuthLimitKey>,
) -> Response {
    let accepts_sse = headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|part| part.trim() == "text/event-stream")
        });
    if !accepts_sse {
        return (StatusCode::METHOD_NOT_ALLOWED, Body::empty()).into_response();
    }
    let Some(raw) = bearer(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(jsonrpc_error(Value::Null, -32001, "missing bearer token")),
        )
            .into_response();
    };
    let user = match authenticate_token(&state, raw, &auth_limit_key.0).await {
        Ok(user) => user,
        Err(err) => {
            let message = err.to_string();
            return (
                StatusCode::UNAUTHORIZED,
                Json(jsonrpc_error(
                    Value::Null,
                    -32001,
                    mcp_auth_error_public_message(&message),
                )),
            )
                .into_response();
        }
    };
    let token_hash = crate::auth::token::hash(raw);
    let vaults =
        match crate::db::repos::VaultRepo::list_for_user(&*state.vaults, &user.user_id).await {
            Ok(vaults) => vaults,
            Err(err) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(jsonrpc_error(Value::Null, -32603, &err.to_string())),
                )
                    .into_response();
            }
        };
    let Some(sse_guard) = state.try_acquire_sse_subscriber(&user.user_id) else {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(jsonrpc_error(
                Value::Null,
                -32029,
                "too many concurrent SSE subscriptions",
            )),
        )
            .into_response();
    };
    let replay_events = match mcp_last_event_id(&headers) {
        Some(commit) => match mcp_replay_events_after(&state, &vaults, &commit).await {
            Ok(events) => events,
            Err(err) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(jsonrpc_error(Value::Null, -32603, &err.to_string())),
                )
                    .into_response();
            }
        },
        _ => McpReplayEvents::Events(Vec::new()),
    };
    let mut streams = tokio_stream::StreamMap::new();
    for vault in vaults {
        let vault_id = vault.id;
        streams.insert(
            vault_id.clone(),
            BroadcastStream::new(state.events.subscribe(&vault_id)),
        );
    }
    let replay_items: VecDeque<McpSseItem> = match replay_events {
        McpReplayEvents::Events(events) => events
            .into_iter()
            .filter_map(|(_vault_id, event)| {
                let commit = event.commit.clone();
                let notification = crate::mcp::notifications::vault_changed(commit.clone(), event);
                let data = serde_json::to_string(&notification).ok()?;
                Some(McpSseItem::VaultChanged { commit, data })
            })
            .collect(),
        McpReplayEvents::Lagged => VecDeque::from([McpSseItem::Lagged]),
    };
    let mut auth_interval = tokio::time::interval(Duration::from_secs(15));
    auth_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    auth_interval.tick().await;
    let (tx, rx) = mpsc::channel(16);
    tokio::spawn(run_mcp_sse_stream(
        McpSseState {
            app: state,
            token_hash,
            user,
            replay_items,
            streams,
            auth_interval,
            _guard: sse_guard,
        },
        tx,
    ));
    Sse::new(ReceiverStream::new(rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}

struct McpSseState {
    app: AppState,
    token_hash: String,
    user: AuthenticatedUser,
    replay_items: VecDeque<McpSseItem>,
    streams: tokio_stream::StreamMap<String, BroadcastStream<crate::service::events::VaultEvent>>,
    auth_interval: Interval,
    _guard: crate::service::state::SseSubscriberGuard,
}

enum McpSseItem {
    VaultChanged { commit: String, data: String },
    Lagged,
}

impl McpSseItem {
    fn into_event(self) -> Event {
        match self {
            Self::VaultChanged { commit, data } => Event::default()
                .event("vault_changed")
                .id(commit)
                .data(data),
            Self::Lagged => Event::default().event("lagged").data(""),
        }
    }
}

async fn run_mcp_sse_stream(mut sse: McpSseState, tx: mpsc::Sender<Result<Event, Infallible>>) {
    loop {
        if tx.is_closed() {
            break;
        }
        if let Some(item) = sse.replay_items.pop_front() {
            if !mcp_sse_token_still_valid(&sse.app, &sse.token_hash, &sse.user).await {
                break;
            }
            if tx.send(Ok(item.into_event())).await.is_err() {
                break;
            }
            continue;
        }
        tokio::select! {
            _ = tx.closed() => {
                break;
            }
            _ = sse.auth_interval.tick() => {
                if !mcp_sse_token_still_valid(&sse.app, &sse.token_hash, &sse.user).await {
                    break;
                }
            }
            event = sse.streams.next() => {
                let Some((_vault_id, Ok(event))) = event else {
                    break;
                };
                if !mcp_sse_token_still_valid(&sse.app, &sse.token_hash, &sse.user).await {
                    break;
                }
                let commit = event.commit.clone();
                let notification = crate::mcp::notifications::vault_changed(commit.clone(), event);
                let Ok(data) = serde_json::to_string(&notification) else {
                    continue;
                };
                if tx
                    .send(Ok(McpSseItem::VaultChanged { commit, data }.into_event()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }
}

async fn mcp_sse_token_still_valid(
    state: &AppState,
    token_hash: &str,
    user: &AuthenticatedUser,
) -> bool {
    let Ok(Some((row, user_id))) = state.tokens.find_by_hash(token_hash).await else {
        return false;
    };
    if row.id != user.token_id || user_id != user.user_id {
        return false;
    }
    let Ok(Some(db_user)) = state.users.find_by_id(&user_id).await else {
        return false;
    };
    db_user.is_active
}

enum McpReplayEvents {
    Events(Vec<(String, crate::service::events::VaultEvent)>),
    Lagged,
}

async fn mcp_replay_events_after(
    state: &AppState,
    vaults: &[crate::db::repos::Vault],
    commit: &str,
) -> anyhow::Result<McpReplayEvents> {
    for vault in vaults {
        let events =
            crate::service::events::replay_events_after(state.vault_root(), &vault.id, commit)
                .await?;
        match events {
            crate::service::events::ReplayEvents::Events(events) if !events.is_empty() => {
                return Ok(McpReplayEvents::Events(
                    events
                        .into_iter()
                        .map(|event| (vault.id.clone(), event))
                        .collect(),
                ));
            }
            crate::service::events::ReplayEvents::Events(_) => {}
            crate::service::events::ReplayEvents::Lagged => return Ok(McpReplayEvents::Lagged),
        }
    }
    Ok(McpReplayEvents::Events(Vec::new()))
}

fn mcp_last_event_id(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())?
        .trim();
    if raw.is_empty() || raw.contains(':') {
        return None;
    }
    Some(raw.to_string())
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::token;
    use crate::db::pool;
    use crate::db::repos::{NewToken, NewUser};
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use tower::ServiceExt;

    async fn state() -> AppState {
        let tmp = tempfile::tempdir().unwrap();
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        AppState::new(p, tmp.path().to_path_buf(), "t".into(), true)
            .await
            .unwrap()
    }

    async fn create_user_token(state: &AppState, username: &str) -> (String, String) {
        let user = state
            .users
            .create(NewUser {
                username: username.into(),
                password_hash: "hash".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let raw = token::generate();
        state
            .tokens
            .create(NewToken {
                user_id: &user.id,
                token_hash: &token::hash(&raw),
                device_id: "mcp-http-test",
                device_name: "MCP HTTP Test",
            })
            .await
            .unwrap();
        (user.id, raw)
    }

    fn mcp_post_with_bearer(raw: &str) -> HttpRequest<Body> {
        HttpRequest::builder()
            .method("POST")
            .uri("/mcp")
            .header("content-type", "application/json")
            .header(deployment_key::HEADER, "k_test")
            .header(header::AUTHORIZATION, format!("Bearer {raw}"))
            .body(Body::from("{}"))
            .unwrap()
    }

    async fn mcp_error_message(response: Response) -> String {
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        body["error"]["message"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn mcp_http_auth_failures_use_generic_message_for_disabled_and_bad_tokens() {
        let state = state().await;
        let (user_id, disabled_token) = create_user_token(&state, "disabled-mcp").await;
        state.users.set_active(&user_id, false).await.unwrap();
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::new(
                100,
                std::time::Duration::from_secs(60),
            ),
            "k_test".into(),
        );

        let disabled = app
            .clone()
            .oneshot(mcp_post_with_bearer(&disabled_token))
            .await
            .unwrap();
        let bad = app
            .oneshot(mcp_post_with_bearer(&token::generate()))
            .await
            .unwrap();

        assert_eq!(
            mcp_error_message(disabled).await,
            "invalid or revoked token"
        );
        assert_eq!(mcp_error_message(bad).await, "invalid or revoked token");
    }

    #[test]
    fn mcp_auth_error_public_message_only_preserves_rate_limit_errors() {
        assert_eq!(
            mcp_auth_error_public_message("rate_limited: retry later"),
            "rate_limited: retry later"
        );
        assert_eq!(
            mcp_auth_error_public_message("database unavailable"),
            GENERIC_MCP_AUTH_ERROR
        );
    }

    #[test]
    fn mcp_json_body_limit_clamps_huge_max_file_size() {
        assert_ne!(mcp_json_body_limit_bytes(u64::MAX), usize::MAX);
        assert_eq!(mcp_json_body_limit_bytes(u64::MAX), 100 * 1024 * 1024);
    }

    #[tokio::test]
    async fn mcp_http_routes_are_rate_limited() {
        let app = router_with_rate_limiter(
            state().await,
            crate::middleware::rate_limit::RequestRateLimiter::new(
                1,
                std::time::Duration::from_secs(60),
            ),
            "k_test".into(),
        );
        let req = || {
            HttpRequest::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .header(deployment_key::HEADER, "k_test")
                .body(Body::from("{}"))
                .unwrap()
        };

        let first = app.clone().oneshot(req()).await.unwrap();
        let second = app.oneshot(req()).await.unwrap();

        assert_eq!(first.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    /// Build a request that simulates arriving through a reverse proxy at
    /// 127.0.0.1 with the given X-Forwarded-For value.
    fn proxy_req(xff: &str) -> HttpRequest<Body> {
        use axum::extract::ConnectInfo;
        let mut r = HttpRequest::builder()
            .method("POST")
            .uri("/mcp")
            .header("content-type", "application/json")
            .header(deployment_key::HEADER, "k_test")
            .header("x-forwarded-for", xff)
            .body(Body::from("{}"))
            .unwrap();
        // ConnectInfo is needed by real_ip::middleware
        let socket: std::net::SocketAddr = "127.0.0.1:9999".parse().unwrap();
        r.extensions_mut().insert(ConnectInfo(socket));
        r
    }

    /// standalone_router with real_ip: two requests from different real IPs
    /// (same proxy ConnectInfo) must not collapse into the same rate-limit bucket.
    #[tokio::test]
    async fn standalone_router_uses_real_ip_for_rate_limit() {
        let trusted = TrustedProxies::from_vec(vec!["127.0.0.1/32".parse().unwrap()]);
        let limiter = crate::middleware::rate_limit::RequestRateLimiter::new(
            1,
            std::time::Duration::from_secs(60),
        );
        // Use router_with_rate_limiter so we can inject a tight limiter, then
        // wrap it with standalone_router's real_ip layer manually.
        let inner = router_with_rate_limiter(state().await, limiter, "k_test".into());
        let app = inner.layer(axum::middleware::from_fn_with_state(
            trusted,
            crate::middleware::real_ip::middleware,
        ));

        // Two different real IPs, same proxy peer — each gets their own bucket.
        let resp_a1 = app.clone().oneshot(proxy_req("203.0.113.1")).await.unwrap();
        let resp_b1 = app.clone().oneshot(proxy_req("203.0.113.2")).await.unwrap();

        // Both hit auth (bucket not exhausted) because IPs differ.
        assert_eq!(
            resp_a1.status(),
            StatusCode::UNAUTHORIZED,
            "first request from 203.0.113.1 should reach auth, not be rate-limited"
        );
        assert_eq!(
            resp_b1.status(),
            StatusCode::UNAUTHORIZED,
            "first request from 203.0.113.2 should reach auth, not be rate-limited"
        );

        // Second request from the same real IP IS rate-limited.
        let resp_a2 = app.clone().oneshot(proxy_req("203.0.113.1")).await.unwrap();
        assert_eq!(
            resp_a2.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "second request from same real IP should be rate-limited"
        );
    }

    /// Without real_ip middleware the two distinct XFF IPs collapse to the
    /// proxy peer IP, so the second request (different real IP) is still
    /// rate-limited — demonstrating the bug that standalone_router fixes.
    #[tokio::test]
    async fn plain_router_collapses_xff_ips_to_proxy_bucket() {
        let limiter = crate::middleware::rate_limit::RequestRateLimiter::new(
            1,
            std::time::Duration::from_secs(60),
        );
        let app = router_with_rate_limiter(state().await, limiter, "k_test".into());

        let resp_a = app.clone().oneshot(proxy_req("203.0.113.1")).await.unwrap();
        let resp_b = app.clone().oneshot(proxy_req("203.0.113.2")).await.unwrap();

        assert_eq!(
            resp_a.status(),
            StatusCode::UNAUTHORIZED,
            "first request should reach auth"
        );
        // Without real_ip, both XFF values resolve to the same proxy peer IP,
        // so the second request with a *different* real IP is wrongly throttled.
        assert_eq!(
            resp_b.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "without real_ip layer, different XFF IPs collapse to the same bucket"
        );
    }

    /// When the ConnectInfo peer is NOT in trusted_proxies the XFF header is
    /// ignored and the peer IP itself is used as the rate-limit key.
    #[tokio::test]
    async fn standalone_router_untrusted_proxy_ignores_xff() {
        // 10.0.0.1 is NOT in the trusted list.
        let trusted = TrustedProxies::from_vec(vec!["192.168.1.1/32".parse().unwrap()]);
        let limiter = crate::middleware::rate_limit::RequestRateLimiter::new(
            1,
            std::time::Duration::from_secs(60),
        );
        let inner = router_with_rate_limiter(state().await, limiter, "k_test".into());
        let app = inner.layer(axum::middleware::from_fn_with_state(
            trusted,
            crate::middleware::real_ip::middleware,
        ));

        // Two requests from the *same* peer (10.0.0.1) but different XFF values.
        // Because the peer is untrusted, XFF is ignored and both map to 10.0.0.1.
        use axum::extract::ConnectInfo;
        let make_req = |xff: &str| {
            let mut r = HttpRequest::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .header(deployment_key::HEADER, "k_test")
                .header("x-forwarded-for", xff)
                .body(Body::from("{}"))
                .unwrap();
            let socket: std::net::SocketAddr = "10.0.0.1:9999".parse().unwrap();
            r.extensions_mut().insert(ConnectInfo(socket));
            r
        };

        let resp1 = app.clone().oneshot(make_req("203.0.113.1")).await.unwrap();
        let resp2 = app.clone().oneshot(make_req("203.0.113.99")).await.unwrap();

        assert_eq!(resp1.status(), StatusCode::UNAUTHORIZED);
        // Second request from the same untrusted peer must be rate-limited
        // regardless of the (ignored) XFF value.
        assert_eq!(
            resp2.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "untrusted peer: both requests map to the same IP bucket"
        );
    }
}
