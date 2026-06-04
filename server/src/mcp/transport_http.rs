use crate::auth::AuthenticatedUser;
use crate::db::repos::{TokenRepo, UserRepo};
use crate::middleware::deployment_key;
use crate::service::AppState;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Request, State};
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

use super::transport_stdio::{authenticate_token, handle_jsonrpc, jsonrpc_error};

#[derive(Clone, Debug)]
struct McpAuthLimitKey(String);

const MCP_JSON_BODY_LIMIT_BYTES: usize = 1024 * 1024;

pub fn router(state: AppState, deployment_key: String) -> Router {
    router_with_rate_limiter(
        state,
        crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
        deployment_key,
    )
}

fn router_with_rate_limiter(
    state: AppState,
    limiter: crate::middleware::rate_limit::RequestRateLimiter,
    deployment_key_value: String,
) -> Router {
    Router::new()
        .route("/mcp", post(post_mcp).get(get_mcp_sse))
        .layer(DefaultBodyLimit::max(MCP_JSON_BODY_LIMIT_BYTES))
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

pub async fn run(state: AppState, bind: SocketAddr, deployment_key: String) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(
        listener,
        router(state, deployment_key).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

async fn post_mcp(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Extension(auth_limit_key): axum::extract::Extension<McpAuthLimitKey>,
    Json(request): Json<Value>,
) -> Response {
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
            return (
                StatusCode::UNAUTHORIZED,
                Json(jsonrpc_error(Value::Null, -32001, &err.to_string())),
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
            return (
                StatusCode::UNAUTHORIZED,
                Json(jsonrpc_error(Value::Null, -32001, &err.to_string())),
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
        let events = crate::service::events::replay_events_after(
            state.default_vault_root(),
            &vault.id,
            commit,
        )
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
    use crate::db::pool;
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
}
