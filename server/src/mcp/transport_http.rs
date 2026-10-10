use crate::auth::AuthenticatedUser;
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
use std::collections::{HashSet, VecDeque};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{Interval, MissedTickBehavior};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tokio_stream::StreamExt;

use super::auth::{mcp_token_still_valid, TokenValidityCache};
use super::transport_stdio::{
    authenticate_token, handle_jsonrpc, jsonrpc_error, GENERIC_MCP_AUTH_ERROR,
    GENERIC_MCP_INTERNAL_ERROR,
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

fn mcp_internal_error_response(error: impl std::fmt::Display) -> Response {
    tracing::error!(error = %error, "mcp http transport failed internally");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(jsonrpc_error(
            Value::Null,
            -32603,
            GENERIC_MCP_INTERNAL_ERROR,
        )),
    )
        .into_response()
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
    let max_file_size = state.runtime_cfg.max_file_size().await;
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
    // Notification (no id): accepted → 202, no body (transport spec).
    if request.get("id").is_none() {
        return StatusCode::ACCEPTED.into_response();
    }

    let era = super::protocol::classify_request(&request);
    let is_modern = matches!(era, Ok(super::protocol::Era::Modern));
    let response = match era {
        Ok(super::protocol::Era::Modern) => {
            if let Err(reason) = super::protocol::validate_modern_headers(&headers, &request) {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(jsonrpc_error(
                        request.get("id").cloned().unwrap_or(Value::Null),
                        -32020,
                        &format!("Header mismatch: {reason}"),
                    )),
                )
                    .into_response();
            }
            if request.get("method").and_then(Value::as_str) == Some("subscriptions/listen") {
                let raw = bearer(&headers).expect("checked earlier");
                return listen_response(state, user, crate::auth::token::hash(raw), request).await;
            }
            handle_modern_jsonrpc(&state, &user, request).await
        }
        Ok(super::protocol::Era::Legacy) => handle_jsonrpc(&state, &user, None, request).await,
        Err(err) => super::protocol::error_response(request.get("id").unwrap_or(&Value::Null), err),
    };

    let status = match response["error"]["code"].as_i64() {
        Some(-32020 | -32021 | -32022 | -32602) => StatusCode::BAD_REQUEST,
        // Legacy JSON-RPC clients expect 200-wrapped errors, so only modern-era
        // method-not-found maps to 404.
        Some(-32601) if is_modern => StatusCode::NOT_FOUND,
        _ => StatusCode::OK,
    };
    (status, Json(response)).into_response()
}

// `subscriptions/listen` is intercepted in `post_mcp` before this runs — its
// SSE response cannot be produced as a JSON-RPC value.
async fn handle_modern_jsonrpc(
    state: &AppState,
    user: &AuthenticatedUser,
    request: Value,
) -> Value {
    super::transport_stdio::handle_modern_jsonrpc(state, user, request).await
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
                return mcp_internal_error_response(err);
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
    // Subscribe BEFORE replaying: a commit landing between the history
    // snapshot and the live subscription would otherwise be neither replayed
    // nor delivered (BUG-R3-12). Overlapping commits are suppressed below by
    // id, mirroring the vault SSE handler.
    let mut streams = tokio_stream::StreamMap::new();
    for vault in &vaults {
        let vault_id = vault.id.clone();
        streams.insert(
            vault_id.clone(),
            BroadcastStream::new(state.events.subscribe(&vault_id)),
        );
    }
    crate::api::vaults::debug_pause_after_subscribe_for_tests().await;

    let replay_items: VecDeque<McpSseItem> = match mcp_last_event_id(&headers) {
        Some(commit) => match mcp_replay_events_after(&state, &vaults, &commit).await {
            Ok((McpReplayEvents::Events(events), unmatched_vaults)) => {
                let mut items: VecDeque<McpSseItem> = events
                    .into_iter()
                    .filter_map(|(_vault_id, event)| {
                        let commit = event.commit.clone();
                        let notification =
                            crate::mcp::notifications::vault_changed(commit.clone(), event);
                        let data = serde_json::to_string(&notification).ok()?;
                        Some(McpSseItem::VaultChanged { commit, data })
                    })
                    .collect();
                // A single Last-Event-ID only identifies one vault's position;
                // every other vault's missed commits are unknowable, so tell
                // the client to resync (BUG-R3-13).
                if unmatched_vaults {
                    items.push_back(McpSseItem::Lagged);
                }
                items
            }
            Ok((McpReplayEvents::Lagged, _)) => VecDeque::from([McpSseItem::Lagged]),
            Err(err) => {
                return mcp_internal_error_response(err);
            }
        },
        None => VecDeque::new(),
    };
    let replay_commit_ids: HashSet<String> = replay_items
        .iter()
        .filter_map(|item| match item {
            McpSseItem::VaultChanged { commit, .. } => Some(commit.clone()),
            McpSseItem::Lagged => None,
        })
        .collect();
    crate::api::vaults::debug_pause_after_replay_for_tests().await;
    let mut auth_interval = tokio::time::interval(Duration::from_secs(15));
    auth_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    auth_interval.tick().await;
    let (tx, rx) = mpsc::channel(16);
    let shutdown_rx = state.shutdown.subscribe();
    tokio::spawn(run_mcp_sse_stream(
        McpSseState {
            app: state,
            token_hash,
            user,
            validity_cache: TokenValidityCache::default(),
            replay_items,
            replay_commit_ids,
            streams,
            auth_interval,
            shutdown_rx,
            _guard: sse_guard,
        },
        tx,
    ));
    let mut response = Sse::new(ReceiverStream::new(rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response();
    response.headers_mut().insert(
        header::HeaderName::from_static("x-accel-buffering"),
        header::HeaderValue::from_static("no"),
    );
    response
}

struct McpSseState {
    app: AppState,
    token_hash: String,
    user: AuthenticatedUser,
    validity_cache: TokenValidityCache,
    replay_items: VecDeque<McpSseItem>,
    replay_commit_ids: HashSet<String>,
    streams: tokio_stream::StreamMap<String, BroadcastStream<crate::service::events::VaultEvent>>,
    auth_interval: Interval,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
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
            if !mcp_token_still_valid(
                &sse.app,
                &sse.token_hash,
                &sse.user,
                &mut sse.validity_cache,
            )
            .await
            {
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
            _ = sse.shutdown_rx.changed() => {
                break;
            }
            _ = sse.auth_interval.tick() => {
                if !mcp_token_still_valid(
                    &sse.app,
                    &sse.token_hash,
                    &sse.user,
                    &mut sse.validity_cache,
                ).await {
                    break;
                }
            }
            event = sse.streams.next() => {
                let item = match event {
                    Some((_vault_id, Ok(event))) => {
                        if sse.replay_commit_ids.contains(&event.commit) {
                            continue;
                        }
                        let commit = event.commit.clone();
                        let notification = crate::mcp::notifications::vault_changed(
                            commit.clone(),
                            event,
                        );
                        let Ok(data) = serde_json::to_string(&notification) else {
                            continue;
                        };
                        McpSseItem::VaultChanged { commit, data }
                    }
                    Some((_vault_id, Err(BroadcastStreamRecvError::Lagged(_)))) => {
                        McpSseItem::Lagged
                    }
                    None => break,
                };
                if !mcp_token_still_valid(
                    &sse.app,
                    &sse.token_hash,
                    &sse.user,
                    &mut sse.validity_cache,
                ).await {
                    break;
                }
                if tx
                    .send(Ok(item.into_event()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }
}

async fn listen_response(
    state: AppState,
    user: AuthenticatedUser,
    token_hash: String,
    request: Value,
) -> Response {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let requested_uris: Vec<String> = request
        .pointer("/params/notifications/resourceSubscriptions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let vaults =
        match crate::db::repos::VaultRepo::list_for_user(&*state.vaults, &user.user_id).await {
            Ok(vaults) => vaults,
            Err(err) => return mcp_internal_error_response(err),
        };
    let owned: HashSet<String> = vaults.iter().map(|v| v.id.clone()).collect();
    // One pass: strip the prefix, keep only vaults the token may see, and
    // carry (vault_id, uri) so the ack echo and the subscription set cannot
    // drift apart.
    let accepted: Vec<(String, String)> = requested_uris
        .iter()
        .filter_map(|uri| {
            let vault_id = uri.strip_prefix("pkv://vault/")?;
            owned
                .contains(vault_id)
                .then(|| (vault_id.to_string(), uri.clone()))
        })
        .collect();
    let mut ack_notifications = serde_json::Map::new();
    for switch in [
        ("toolsListChanged", "/params/notifications/toolsListChanged"),
        (
            "promptsListChanged",
            "/params/notifications/promptsListChanged",
        ),
        (
            "resourcesListChanged",
            "/params/notifications/resourcesListChanged",
        ),
    ] {
        if request.pointer(switch.1).and_then(Value::as_bool) == Some(true) {
            // Tool/prompt/resource lists are static; the switch is accepted
            // (echoed) but no list-changed notification is ever emitted.
            ack_notifications.insert(switch.0.into(), Value::Bool(true));
        }
    }
    if !accepted.is_empty() {
        ack_notifications.insert(
            "resourceSubscriptions".into(),
            serde_json::json!(accepted
                .iter()
                .map(|(_, uri)| uri)
                .cloned()
                .collect::<Vec<_>>()),
        );
    }

    let Some(sse_guard) = state.try_acquire_sse_subscriber(&user.user_id) else {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(jsonrpc_error(
                id,
                -63001,
                "too many concurrent SSE subscriptions",
            )),
        )
            .into_response();
    };
    let mut streams = tokio_stream::StreamMap::new();
    for (vault_id, _) in &accepted {
        streams.insert(
            vault_id.clone(),
            BroadcastStream::new(state.events.subscribe(vault_id)),
        );
    }
    let ack = crate::mcp::notifications::McpNotification {
        method: "notifications/subscriptions/acknowledged".into(),
        params: serde_json::json!({
            "notifications": ack_notifications,
            "_meta": {super::protocol::META_SUBSCRIPTION_ID: id},
        }),
    };
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(16);
    let mut auth_interval = tokio::time::interval(Duration::from_secs(15));
    auth_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    auth_interval.tick().await;
    tokio::spawn(run_listen_stream(
        ListenState {
            app: state,
            user,
            token_hash,
            id,
            streams,
            ack: Some(ack),
            validity_cache: TokenValidityCache::default(),
            auth_interval,
            _guard: sse_guard,
        },
        tx,
    ));
    let mut response = Sse::new(ReceiverStream::new(rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response();
    response.headers_mut().insert(
        header::HeaderName::from_static("x-accel-buffering"),
        header::HeaderValue::from_static("no"),
    );
    response
}

struct ListenState {
    app: AppState,
    user: AuthenticatedUser,
    token_hash: String,
    id: Value,
    streams: tokio_stream::StreamMap<String, BroadcastStream<crate::service::events::VaultEvent>>,
    ack: Option<crate::mcp::notifications::McpNotification>,
    validity_cache: TokenValidityCache,
    auth_interval: Interval,
    // The GET handler selects on `shutdown_rx`; listen streams are purely
    // connection-scoped (client disconnect ends them), so there is no
    // equivalent subscription here.
    _guard: crate::service::state::SseSubscriberGuard,
}

impl ListenState {
    async fn token_still_valid(&mut self) -> bool {
        mcp_token_still_valid(
            &self.app,
            &self.token_hash,
            &self.user,
            &mut self.validity_cache,
        )
        .await
    }
}

async fn run_listen_stream(mut listen: ListenState, tx: mpsc::Sender<Result<Event, Infallible>>) {
    loop {
        if tx.is_closed() {
            break;
        }
        if let Some(ack) = listen.ack.take() {
            let Ok(data) = serde_json::to_string(&ack) else {
                break;
            };
            if !listen.token_still_valid().await {
                break;
            }
            if tx.send(Ok(Event::default().data(data))).await.is_err() {
                break;
            }
            continue;
        }
        tokio::select! {
            _ = tx.closed() => {
                break;
            }
            _ = listen.auth_interval.tick() => {
                if !listen.token_still_valid().await {
                    break;
                }
            }
            event = listen.streams.next(), if !listen.streams.is_empty() => {
                match event {
                    Some((vault_id, Ok(event))) => {
                        let commit = event.commit.clone();
                        let notification = crate::mcp::notifications::resource_updated(
                            &vault_id,
                            commit,
                            event,
                            &listen.id,
                        );
                        let Ok(data) = serde_json::to_string(&notification) else {
                            continue;
                        };
                        if !listen.token_still_valid().await {
                            break;
                        }
                        if tx.send(Ok(Event::default().data(data))).await.is_err() {
                            break;
                        }
                    }
                    // No lag signal on the modern stream: the client's next
                    // read/listen re-syncs.
                    Some((_vault_id, Err(BroadcastStreamRecvError::Lagged(_)))) => {
                        continue;
                    }
                    None => break,
                }
            }
        }
    }
}

enum McpReplayEvents {
    Events(Vec<(String, crate::service::events::VaultEvent)>),
    Lagged,
}

async fn mcp_replay_events_after(
    state: &AppState,
    vaults: &[crate::db::repos::Vault],
    commit: &str,
) -> anyhow::Result<(McpReplayEvents, bool)> {
    let mut events_all: Vec<(String, crate::service::events::VaultEvent)> = Vec::new();
    for vault in vaults {
        let events =
            crate::service::events::replay_events_after(state.vault_root(), &vault.id, commit)
                .await?;
        match events {
            crate::service::events::ReplayEvents::Events(events) => {
                events_all.extend(events.into_iter().map(|event| (vault.id.clone(), event)));
            }
            crate::service::events::ReplayEvents::Lagged => {
                return Ok((McpReplayEvents::Lagged, false));
            }
        }
    }
    // Only the vault whose history contains `commit` yields events; with more
    // than one vault the others' positions are unknown and the caller appends
    // a lagged signal (BUG-R3-13).
    let unmatched_vaults = vaults.len() > 1;
    Ok((McpReplayEvents::Events(events_all), unmatched_vaults))
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
    use crate::db::repos::{TokenRepo, UserRepo};
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

    fn modern_post(raw: &str, method: &str, name: Option<&str>, token: &str) -> HttpRequest<Body> {
        let mut builder = HttpRequest::builder()
            .method("POST")
            .uri("/mcp")
            .header("content-type", "application/json")
            .header(deployment_key::HEADER, "k_test")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header("MCP-Protocol-Version", "2026-07-28")
            .header("Mcp-Method", method);
        if let Some(name) = name {
            builder = builder.header("Mcp-Name", name);
        }
        builder.body(Body::from(raw.to_string())).unwrap()
    }

    fn modern_body(method: &str) -> String {
        serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": method,
            "params": {"_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {}
            }}
        })
        .to_string()
    }

    async fn response_json(response: Response) -> Value {
        let body = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    async fn mcp_error_message(response: Response) -> String {
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        body["error"]["message"].as_str().unwrap().to_string()
    }

    async fn jsonrpc_error_message(response: Response) -> String {
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

    #[tokio::test]
    async fn mcp_internal_error_response_hides_raw_error() {
        let response = mcp_internal_error_response(anyhow::anyhow!(
            "database closed at C:\\secret\\db.sqlite"
        ));

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let message = jsonrpc_error_message(response).await;
        assert_eq!(message, "internal server error");
        assert!(!message.contains("database"));
        assert!(!message.contains("secret"));
    }

    #[test]
    fn mcp_json_body_limit_clamps_huge_max_file_size() {
        assert_ne!(mcp_json_body_limit_bytes(u64::MAX), usize::MAX);
        assert_eq!(mcp_json_body_limit_bytes(u64::MAX), 100 * 1024 * 1024);
    }

    #[test]
    fn high_audit_mcp_live_sse_lagged_broadcast_errors_emit_lagged_event() {
        let source = include_str!("transport_http.rs");
        let branch_start = source
            .find("event = sse.streams.next()")
            .expect("live event branch exists");
        let branch_end = source[branch_start..]
            .find("if !mcp_token_still_valid")
            .map(|idx| branch_start + idx)
            .expect("live event branch checks token validity after lagged handling");
        let branch = &source[branch_start..branch_end];

        assert!(branch.contains("BroadcastStreamRecvError::Lagged"));
        assert!(branch.contains("McpSseItem::Lagged"));
        assert!(!branch.contains("let Some((_vault_id, Ok(event))) = event else"));
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

    #[tokio::test]
    async fn modern_tools_list_returns_decorated_result() {
        let state = state().await;
        let (user_id, raw) = create_user_token(&state, "u1").await;
        let _ = user_id;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let response = app
            .oneshot(modern_post(
                &modern_body("tools/list"),
                "tools/list",
                None,
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 1 << 20)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["result"]["resultType"], "complete");
        assert_eq!(body["result"]["ttlMs"], 300_000);
        assert_eq!(body["result"]["cacheScope"], "private");
        assert_eq!(
            body["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            "cyberkurry pkv sync"
        );
    }

    #[tokio::test]
    async fn modern_server_discover_lists_both_eras() {
        let state = state().await;
        let (user_id, raw) = create_user_token(&state, "u1").await;
        let _ = user_id;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let response = app
            .oneshot(modern_post(
                &modern_body("server/discover"),
                "server/discover",
                None,
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(
            body["result"]["supportedVersions"],
            serde_json::json!(["2026-07-28", "2025-06-18"])
        );
        assert_eq!(
            body["result"]["capabilities"]["tools"],
            serde_json::json!({})
        );
        assert_eq!(body["result"]["resultType"], "complete");
        assert_eq!(body["result"]["ttlMs"], 3_600_000);
        assert_eq!(body["result"]["cacheScope"], "private");
        assert_eq!(
            body["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            "cyberkurry pkv sync"
        );
    }

    #[tokio::test]
    async fn modern_notification_returns_202_with_empty_body() {
        let state = state().await;
        let (user_id, raw) = create_user_token(&state, "u1").await;
        let _ = user_id;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let body = serde_json::json!({
            "jsonrpc": "2.0", "method": "notifications/cancelled", "params": {}
        });
        let response = app
            .oneshot(modern_post(
                &body.to_string(),
                "notifications/cancelled",
                None,
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        assert!(bytes.is_empty(), "expected empty body, got {bytes:?}");
    }

    #[tokio::test]
    async fn header_mismatch_is_400_header_mismatch() {
        let state = state().await;
        let (user_id, raw) = create_user_token(&state, "u1").await;
        let _ = user_id;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let mut request = modern_post(&modern_body("tools/list"), "tools/list", None, &raw);
        request.headers_mut().insert(
            "MCP-Protocol-Version",
            axum::http::HeaderValue::from_static("2025-06-18"),
        );
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], -32020);
        let message = body["error"]["message"].as_str().unwrap();
        assert!(
            message.contains("header"),
            "message must mention header: {message}"
        );
    }

    #[tokio::test]
    async fn missing_client_capabilities_is_400_invalid_params() {
        let state = state().await;
        let (user_id, raw) = create_user_token(&state, "u1").await;
        let _ = user_id;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/list",
            "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}}
        });
        let response = app
            .oneshot(modern_post(&body.to_string(), "tools/list", None, &raw))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn unsupported_version_is_400_with_supported_list() {
        let state = state().await;
        let (user_id, raw) = create_user_token(&state, "u1").await;
        let _ = user_id;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/list",
            "params": {"_meta": {
                "io.modelcontextprotocol/protocolVersion": "2099-01-01",
                "io.modelcontextprotocol/clientCapabilities": {}
            }}
        });
        let mut request = modern_post(&body.to_string(), "tools/list", None, &raw);
        request.headers_mut().insert(
            "MCP-Protocol-Version",
            axum::http::HeaderValue::from_static("2099-01-01"),
        );
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], -32022);
        assert_eq!(
            body["error"]["data"]["supported"],
            serde_json::json!(["2026-07-28", "2025-06-18"])
        );
    }

    #[tokio::test]
    async fn unknown_method_modern_is_404_method_not_found() {
        let state = state().await;
        let (user_id, raw) = create_user_token(&state, "u1").await;
        let _ = user_id;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let response = app
            .oneshot(modern_post(
                &modern_body("resources/list"),
                "resources/list",
                None,
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], -32601);
    }

    fn listen_body(filter: serde_json::Value) -> String {
        serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "subscriptions/listen",
            "params": {
                "notifications": filter,
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            }
        })
        .to_string()
    }

    fn listen_post(raw: &str, filter: serde_json::Value) -> HttpRequest<Body> {
        modern_post(&listen_body(filter), "subscriptions/listen", None, raw)
    }

    struct ListenServer {
        state: AppState,
        user_id: String,
        raw: String,
        vault_id: String,
    }

    async fn listen_server() -> ListenServer {
        let state = state().await;
        let (user_id, raw) = create_user_token(&state, "u1").await;
        let vault = crate::service::vault::create_vault(&state, &user_id, "main")
            .await
            .unwrap();
        ListenServer {
            state,
            user_id,
            raw,
            vault_id: vault.id,
        }
    }

    fn listen_router(state: &AppState) -> Router {
        router_with_rate_limiter(
            state.clone(),
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        )
    }

    fn auth_for(user_id: &str) -> crate::auth::AuthenticatedUser {
        crate::auth::AuthenticatedUser {
            user_id: user_id.into(),
            username: "u1".into(),
            is_admin: false,
            token_id: "listen-test-token".into(),
            device_id: "listen-test-device".into(),
        }
    }

    async fn commit_to_vault(ls: &ListenServer) -> String {
        use crate::service::sync::{push, PushChange, PushReq};
        push(
            &ls.state,
            &auth_for(&ls.user_id),
            &ls.vault_id,
            None,
            None,
            PushReq {
                device_name: Some("listen-test".into()),
                changes: vec![PushChange::Text {
                    path: "listen.md".into(),
                    content: "v".into(),
                }],
            },
        )
        .await
        .unwrap()
        .new_commit
    }

    fn parse_sse_data(body: &str) -> Vec<Value> {
        body.split("\n\n")
            .filter_map(|event| {
                let data = event
                    .lines()
                    .filter_map(|line| line.strip_prefix("data: "))
                    .collect::<Vec<_>>()
                    .join("\n");
                if data.is_empty() {
                    None
                } else {
                    Some(serde_json::from_str(&data).unwrap())
                }
            })
            .collect()
    }

    #[tokio::test]
    async fn legacy_initialize_echoes_requested_version() {
        let state = state().await;
        let (user_id, raw) = create_user_token(&state, "u1").await;
        let _ = user_id;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let post_initialize = |protocol_version: &str| {
            let body = serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {"protocolVersion": protocol_version}
            });
            HttpRequest::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .header(deployment_key::HEADER, "k_test")
                .header(header::AUTHORIZATION, format!("Bearer {raw}"))
                .body(Body::from(body.to_string()))
                .unwrap()
        };

        let known = app
            .clone()
            .oneshot(post_initialize("2024-11-05"))
            .await
            .unwrap();
        assert_eq!(known.status(), StatusCode::OK);
        let known = response_json(known).await;
        assert_eq!(known["result"]["protocolVersion"], "2024-11-05");

        let unknown = app.oneshot(post_initialize("1999-01-01")).await.unwrap();
        assert_eq!(unknown.status(), StatusCode::OK);
        let unknown = response_json(unknown).await;
        assert_eq!(unknown["result"]["protocolVersion"], "2025-06-18");
    }

    #[tokio::test]
    async fn listen_acknowledges_then_streams_resource_updates() {
        let ls = listen_server().await;
        let app = listen_router(&ls.state);

        let response = app
            .oneshot(listen_post(
                &ls.raw,
                serde_json::json!({
                    "resourceSubscriptions": [format!("pkv://vault/{}", ls.vault_id)]
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/event-stream")));
        assert_eq!(
            response
                .headers()
                .get("x-accel-buffering")
                .and_then(|value| value.to_str().ok()),
            Some("no")
        );

        let mut stream = response.into_body().into_data_stream();
        let mut body = String::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let ack: Value = loop {
            let chunk = tokio::select! {
                chunk = stream.next() => chunk,
                _ = tokio::time::sleep_until(deadline) => panic!("no ack within 5s, got: {body}"),
            };
            match chunk {
                Some(Ok(bytes)) => {
                    body.push_str(&String::from_utf8_lossy(&bytes));
                    if let Some(event) = parse_sse_data(&body).first() {
                        break event.clone();
                    }
                }
                Some(Err(err)) => panic!("body stream error: {err}"),
                None => panic!("stream ended before ack, got: {body}"),
            }
        };

        assert_eq!(
            ack["method"], "notifications/subscriptions/acknowledged",
            "first SSE event must be the ack"
        );
        assert_eq!(
            ack["params"]["notifications"]["resourceSubscriptions"],
            serde_json::json!([format!("pkv://vault/{}", ls.vault_id)])
        );
        assert_eq!(
            ack["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"],
            1
        );

        commit_to_vault(&ls).await;

        let mut body = String::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let update: Value = loop {
            let chunk = tokio::select! {
                chunk = stream.next() => chunk,
                _ = tokio::time::sleep_until(deadline) => panic!("no update within 5s, got: {body}"),
            };
            match chunk {
                Some(Ok(bytes)) => {
                    body.push_str(&String::from_utf8_lossy(&bytes));
                    if let Some(event) = parse_sse_data(&body).first() {
                        break event.clone();
                    }
                }
                Some(Err(err)) => panic!("body stream error: {err}"),
                None => panic!("stream ended before update, got: {body}"),
            }
        };

        assert_eq!(update["method"], "notifications/resources/updated");
        assert_eq!(
            update["params"]["uri"],
            format!("pkv://vault/{}", ls.vault_id)
        );
        assert_eq!(
            update["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"],
            1
        );
    }

    #[tokio::test]
    async fn listen_with_empty_filter_acknowledges_nothing_and_streams_nothing() {
        let ls = listen_server().await;
        let app = listen_router(&ls.state);

        let response = app
            .oneshot(listen_post(&ls.raw, serde_json::json!({})))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/event-stream")));

        let mut stream = response.into_body().into_data_stream();
        let mut body = String::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let ack: Value = loop {
            let chunk = tokio::select! {
                chunk = stream.next() => chunk,
                _ = tokio::time::sleep_until(deadline) => panic!("no ack within 5s, got: {body}"),
            };
            match chunk {
                Some(Ok(bytes)) => {
                    body.push_str(&String::from_utf8_lossy(&bytes));
                    if let Some(event) = parse_sse_data(&body).first() {
                        break event.clone();
                    }
                }
                Some(Err(err)) => panic!("body stream error: {err}"),
                None => panic!("stream ended before ack, got: {body}"),
            }
        };

        assert_eq!(
            ack["method"], "notifications/subscriptions/acknowledged",
            "first SSE event must be the ack"
        );
        assert_eq!(ack["params"]["notifications"], serde_json::json!({}));

        commit_to_vault(&ls).await;

        let mut new_bytes = String::new();
        let slept = tokio::time::timeout(Duration::from_millis(300), async {
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => new_bytes.push_str(&String::from_utf8_lossy(&bytes)),
                    Err(_) => break,
                }
            }
        })
        .await;
        assert!(slept.is_err(), "stream must stay open after a commit");
        assert!(
            parse_sse_data(&new_bytes).is_empty(),
            "commit on an unsubscribed vault must not stream anything, got: {new_bytes}"
        );
    }

    #[tokio::test]
    async fn listen_unknown_vault_uri_is_dropped_from_ack() {
        let ls = listen_server().await;
        let app = listen_router(&ls.state);

        let response = app
            .oneshot(listen_post(
                &ls.raw,
                serde_json::json!({
                    "resourceSubscriptions": [
                        "pkv://vault/unknown",
                        format!("pkv://vault/{}", ls.vault_id)
                    ]
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let mut stream = response.into_body().into_data_stream();
        let mut body = String::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let ack: Value = loop {
            let chunk = tokio::select! {
                chunk = stream.next() => chunk,
                _ = tokio::time::sleep_until(deadline) => panic!("no ack within 5s, got: {body}"),
            };
            match chunk {
                Some(Ok(bytes)) => {
                    body.push_str(&String::from_utf8_lossy(&bytes));
                    if let Some(event) = parse_sse_data(&body).first() {
                        break event.clone();
                    }
                }
                Some(Err(err)) => panic!("body stream error: {err}"),
                None => panic!("stream ended before ack, got: {body}"),
            }
        };

        assert_eq!(ack["method"], "notifications/subscriptions/acknowledged");
        assert_eq!(
            ack["params"]["notifications"]["resourceSubscriptions"],
            serde_json::json!([format!("pkv://vault/{}", ls.vault_id)]),
            "unknown vault URI must be dropped from the ack: {body}"
        );
    }

    #[tokio::test]
    async fn legacy_listen_is_method_not_found() {
        let state = state().await;
        let (user_id, raw) = create_user_token(&state, "u1").await;
        let _ = user_id;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "subscriptions/listen",
            "params": {"notifications": {}}
        });
        let request = HttpRequest::builder()
            .method("POST")
            .uri("/mcp")
            .header("content-type", "application/json")
            .header(deployment_key::HEADER, "k_test")
            .header(header::AUTHORIZATION, format!("Bearer {raw}"))
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], -32601);
    }
}
