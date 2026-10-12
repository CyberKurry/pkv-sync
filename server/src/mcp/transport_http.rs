use crate::auth::bearer::bearer_token;
use crate::auth::AuthenticatedUser;
use crate::middleware::deployment_key;
use crate::service::AppState;
use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time::{Interval, MissedTickBehavior};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tokio_stream::StreamExt;

use super::auth::{mcp_token_still_valid, TokenValidityCache};
use super::protocol::{ERR_AUTH, ERR_RATE_LIMIT};
use super::transport_stdio::{
    authenticate_token, handle_jsonrpc, handle_modern_jsonrpc, jsonrpc_error,
    GENERIC_MCP_AUTH_ERROR, GENERIC_MCP_INTERNAL_ERROR,
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

async fn post_mcp(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Extension(auth_limit_key): axum::extract::Extension<McpAuthLimitKey>,
    body: Body,
) -> Response {
    if !is_json_content_type(&headers) {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    // Header-only checks first, body work only after authentication (M-4):
    // a bearer check needs nothing from the stream, and reading the body
    // costs up to ~100 MiB of buffering plus a JSON parse. The old order
    // answered invalid credentials with the parse error (and did all the
    // work first), so unauthenticated callers could make the server buffer
    // and parse arbitrarily large bodies.
    let (user, raw) = match bearer_user(&state, &headers, &auth_limit_key.0).await {
        Ok(authenticated) => authenticated,
        Err(response) => return *response,
    };
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
    // Notification (no id): accepted → 202, no body (transport spec).
    // Exception: `notifications/cancelled` cannot work over HTTP — there is
    // no server-side registry keyed by requestId, and the only supported
    // cancellation is the client dropping the SSE stream. A 202 here would
    // pretend a cancellation happened, so answer 405 with guidance instead.
    //
    // Three corner semantics of running this branch BEFORE era
    // classification and header validation (audited N-2; all deliberate):
    //
    // (a) Legacy-era `notifications/cancelled` (no `_meta`) gets the same
    //     405: the guidance text only fits subscriptions/listen semantics,
    //     but legacy HTTP clients had no working cancel either — no
    //     requestId registry ever existed on this transport, so the old
    //     202 "accepted" was equally fictional for them.
    // (b) Header validation is skipped for notifications: the 2026-07-28
    //     spec leaves notification-POST header requirements undefined, so
    //     a mismatched header must not turn a side-effect-free notification
    //     into a hard 400.
    // (c) The error envelope omits `id` entirely rather than echoing
    //     `id: null`: the message is a notification, so there is nothing to
    //     correlate. Strict JSON-RPC 2.0 §5.1 readers may warn about a
    //     missing id; accepted trade-off.
    if request.get("id").is_none() {
        if request.get("method").and_then(Value::as_str) == Some("notifications/cancelled") {
            // Hand-built envelope: a notification has no id to echo, and the
            // shared helper would serialize `"id": null` into it.
            return (
                StatusCode::METHOD_NOT_ALLOWED,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32601,
                        "message": "cancellation on Streamable HTTP is performed by closing the SSE response stream"
                    }
                })),
            )
                .into_response();
        }
        return StatusCode::ACCEPTED.into_response();
    }

    let era = super::protocol::classify_request(&request);
    // `is_modern` covers requests that target the modern era, including ones
    // whose `_meta` is malformed (classification error): only those map
    // protocol errors onto HTTP statuses. Legacy-era requests always get
    // 200-wrapped JSON-RPC envelopes, whatever their error code.
    let is_modern = !matches!(era, Ok(super::protocol::Era::Legacy));
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
                // `raw` was extracted and validated before the body was
                // read (M-4); reuse it instead of re-parsing the header.
                return listen_response(state, user, crate::auth::token::hash(raw), request).await;
            }
            handle_modern_jsonrpc(&state, &user, None, request).await
        }
        Ok(super::protocol::Era::Legacy) => {
            // The header names the modern version but the body has no modern
            // `_meta`: the spec forbids silently downgrading a modern client,
            // so refuse instead of serving the legacy era.
            if super::protocol::header_names_modern_version(&headers) {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(jsonrpc_error(
                        request.get("id").cloned().unwrap_or(Value::Null),
                        -32020,
                        "MCP-Protocol-Version header names a modern version but the request body lacks modern _meta",
                    )),
                )
                    .into_response();
            }
            handle_jsonrpc(&state, &user, None, request).await
        }
        Err(err) => super::protocol::error_response(request.get("id").unwrap_or(&Value::Null), err),
    };

    let status = match response["error"]["code"].as_i64() {
        // Legacy JSON-RPC clients expect 200-wrapped errors, so only
        // modern-era errors map to transport-level statuses; the -32601 →
        // 404 arm below is era-gated the same way. (-32021
        // MissingRequiredClientCapability is a defined spec code we never
        // emit — no handler requires a client capability — so it has no
        // mapping here.)
        Some(-32020 | -32022 | -32602) if is_modern => StatusCode::BAD_REQUEST,
        Some(-32601) if is_modern => StatusCode::NOT_FOUND,
        _ => StatusCode::OK,
    };
    (status, Json(response)).into_response()
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
            Json(jsonrpc_error(
                Value::Null,
                ERR_RATE_LIMIT,
                "too many requests",
            )),
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
    let (user, raw) = match bearer_user(&state, &headers, &auth_limit_key.0).await {
        Ok(authenticated) => authenticated,
        Err(response) => return *response,
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
                ERR_RATE_LIMIT,
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
        // Invalid values skip replay entirely: the client asked to resume
        // from a position the server cannot interpret, so the first event
        // is the documented lagged signal (full resync) — never a plain
        // live stream that leaves the client believing it is caught up.
        LastEventId::Invalid(_) => VecDeque::from([McpSseItem::Lagged { vault_id: None }]),
        LastEventId::Commit(commit) => {
            match mcp_replay_events_after(&state, &vaults, &commit).await {
                Ok((Some(events), unmatched_vaults)) => {
                    let mut items: VecDeque<McpSseItem> = events
                        .into_iter()
                        .filter_map(|(vault_id, event)| {
                            let commit = event.commit.clone();
                            let notification =
                                crate::mcp::notifications::vault_changed(commit.clone(), event);
                            let data = serde_json::to_string(&notification).ok()?;
                            Some(McpSseItem::VaultChanged {
                                vault_id,
                                commit,
                                data,
                            })
                        })
                        .collect();
                    // A single Last-Event-ID only identifies one vault's position;
                    // every other vault's missed commits are unknowable, so tell
                    // the client to resync (BUG-R3-13). Vault-agnostic by
                    // nature: no single vault can be named.
                    if unmatched_vaults {
                        items.push_back(McpSseItem::Lagged { vault_id: None });
                    }
                    items
                }
                Ok((None, _)) => VecDeque::from([McpSseItem::Lagged { vault_id: None }]),
                Err(err) => {
                    return mcp_internal_error_response(err);
                }
            }
        }
        LastEventId::Absent => VecDeque::new(),
    };
    // Suppression is keyed by (vault_id, commit): a bare commit id is a git
    // OID, and identical root commits across vaults (same template, same
    // second) collide, so vault A's replay would suppress vault B's live
    // event (BUG-5).
    let replay_commit_ids = replay_suppression_set(&replay_items);
    crate::api::vaults::debug_pause_after_replay_for_tests().await;
    let auth_interval = auth_interval().await;
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
    sse_response(rx)
}

/// 15s revalidation cadence for authenticated SSE streams: skipped (never
/// burst) after a long stall, with the immediate first tick consumed by the
/// caller so the first revalidation lands one full period after connect.
async fn auth_interval() -> Interval {
    let mut interval = tokio::time::interval(Duration::from_secs(15));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    interval.tick().await;
    interval
}

/// SSE response shared by both MCP streams: 15s keep-alive comment frames
/// and `X-Accel-Buffering: no` so reverse proxies do not buffer events.
fn sse_response(rx: mpsc::Receiver<Result<Event, Infallible>>) -> Response {
    let mut response = Sse::new(ReceiverStream::new(rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response();
    response.headers_mut().insert(
        header::HeaderName::from_static("x-accel-buffering"),
        header::HeaderValue::from_static("no"),
    );
    response
}

/// Resolves once the shutdown watch reports `true`, **immediately** when the
/// current value is already `true`.
///
/// `Receiver::changed()` alone is not enough (R2-2): both SSE handlers
/// authenticate, list vaults and claim an SSE slot *before* they subscribe,
/// so a shutdown that lands in that window leaves the new receiver with
/// `true` already marked as seen — `changed()` would then wait forever, the
/// SSE body would never end, and `with_graceful_shutdown` would still block
/// until TimeoutStopSec/SIGKILL, which is exactly the symptom H-2 removed.
/// `wait_for` checks the current value first, so a pre-triggered signal ends
/// the stream on the first poll.
///
/// The `wait_for` guard is deliberately dropped inside this helper: carrying
/// the `Ref` out as the arm's output would keep a mutable borrow of the
/// receiver alive across the `select!` arm body and conflict with the
/// per-frame state borrows there.
async fn shutdown_requested(rx: &mut watch::Receiver<bool>) {
    let _ = rx.wait_for(|requested| *requested).await;
}

struct McpSseState {
    app: AppState,
    token_hash: String,
    user: AuthenticatedUser,
    validity_cache: TokenValidityCache,
    replay_items: VecDeque<McpSseItem>,
    replay_commit_ids: HashSet<(String, String)>,
    streams: tokio_stream::StreamMap<String, BroadcastStream<crate::service::events::VaultEvent>>,
    auth_interval: Interval,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    _guard: crate::service::state::SseSubscriberGuard,
}

impl McpSseState {
    /// Same recheck as the listen stream's `ListenState::token_still_valid`
    /// (R-3): both SSE loops must drop the connection the moment the token
    /// is revoked, expired or the account is disabled.
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

/// One queued SSE frame: a replayed vault change, or a lag marker.
///
/// `Lagged` carries the affected vault id when known: a live broadcast
/// overflow names the vault so multi-vault clients can resync just that one
/// (aligning with the listen path's `notifications/resources/dropped`,
/// N-3), while the replay-level lag marker stays `None` — a single
/// `Last-Event-ID` cannot identify which vault fell behind, so the event
/// keeps the empty data of the documented legacy contract.
enum McpSseItem {
    VaultChanged {
        vault_id: String,
        commit: String,
        data: String,
    },
    Lagged {
        vault_id: Option<String>,
    },
}

/// Commits carried by a replay batch, keyed by `(vault_id, commit)` so the
/// live-stream suppression check can never match another vault's event with
/// an identical git OID (BUG-5). `Lagged` markers contribute nothing.
fn replay_suppression_set(replay_items: &VecDeque<McpSseItem>) -> HashSet<(String, String)> {
    replay_items
        .iter()
        .filter_map(|item| match item {
            McpSseItem::VaultChanged {
                vault_id, commit, ..
            } => Some((vault_id.clone(), commit.clone())),
            McpSseItem::Lagged { .. } => None,
        })
        .collect()
}

impl McpSseItem {
    fn into_event(self) -> Event {
        match self {
            Self::VaultChanged { commit, data, .. } => Event::default()
                .event("vault_changed")
                .id(commit)
                .data(data),
            // The event name stays `lagged` (legacy clients match on it);
            // the data names the vault when the signal comes from a live
            // broadcast overflow and is empty when the lag is replay-level
            // and vault-agnostic.
            Self::Lagged { vault_id } => Event::default()
                .event("lagged")
                .data(vault_id.unwrap_or_default()),
        }
    }
}

async fn run_mcp_sse_stream(mut sse: McpSseState, tx: mpsc::Sender<Result<Event, Infallible>>) {
    loop {
        if tx.is_closed() {
            break;
        }
        if let Some(item) = sse.replay_items.pop_front() {
            if !sse.token_still_valid().await {
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
            // `shutdown_requested`, not `changed()`: a receiver created after
            // the shutdown signal already fired has the current `true` value
            // marked as seen, so `changed()` never becomes ready and this
            // stream would hold graceful shutdown open until SIGKILL (R2-2).
            _ = shutdown_requested(&mut sse.shutdown_rx) => {
                break;
            }
            _ = sse.auth_interval.tick() => {
                if !sse.token_still_valid().await {
                    break;
                }
            }
            event = sse.streams.next(), if !sse.streams.is_empty() => {
                let item = match event {
                    Some((_vault_id, Ok(event))) => {
                        if sse
                            .replay_commit_ids
                            .contains(&(_vault_id.clone(), event.commit.clone()))
                        {
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
                        McpSseItem::VaultChanged {
                            vault_id: _vault_id,
                            commit,
                            data,
                        }
                    }
                    Some((_vault_id, Err(BroadcastStreamRecvError::Lagged(_)))) => {
                        McpSseItem::Lagged {
                            vault_id: Some(_vault_id),
                        }
                    }
                    None => break,
                };
                if !sse.token_still_valid().await {
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
    let filter_json = request
        .pointer("/params/notifications")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let mut lr = super::listen_filter::parse_listen_request(&filter_json);
    let vaults =
        match crate::db::repos::VaultRepo::list_for_user(&*state.vaults, &user.user_id).await {
            Ok(vaults) => vaults,
            Err(err) => return mcp_internal_error_response(err),
        };
    let owned: HashSet<String> = vaults.iter().map(|v| v.id.clone()).collect();
    // Keep only vaults the token may see. URIs were deduplicated (first
    // occurrence order) by the shared parse, so the ack echo and the
    // subscription set cannot drift apart.
    lr.accepted_uris.retain(|uri| {
        uri.strip_prefix("pkv://vault/")
            .is_some_and(|vault_id| owned.contains(vault_id))
    });
    let accepted: Vec<(String, String)> = lr
        .accepted_uris
        .iter()
        .filter_map(|uri| {
            let vault_id = uri.strip_prefix("pkv://vault/")?;
            Some((vault_id.to_string(), uri.clone()))
        })
        .collect();

    let Some(sse_guard) = state.try_acquire_sse_subscriber(&user.user_id) else {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(jsonrpc_error(
                id,
                ERR_RATE_LIMIT,
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
    let ack = super::listen_filter::acknowledged(&id, &lr);
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(16);
    let auth_interval = auth_interval().await;
    let shutdown_rx = state.shutdown.subscribe();
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
            shutdown_rx,
            _guard: sse_guard,
        },
        tx,
    ));
    sse_response(rx)
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
    // Listen streams are connection-scoped, which is exactly why they need
    // the shutdown watch: a client holding the SSE open (the documented
    // cancel path) would otherwise keep axum's graceful shutdown waiting
    // forever. Same arm as the GET stream (BUG-R3-11 sibling).
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
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
            // Same pre-triggered-signal race as the GET stream (R2-2): the
            // listen handler authenticates and claims an SSE slot before it
            // subscribes, so a shutdown that lands in that window must still
            // end this body immediately.
            _ = shutdown_requested(&mut listen.shutdown_rx) => {
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
                    Some((vault_id, Err(BroadcastStreamRecvError::Lagged(_)))) => {
                        // Lagged means the receiver already skipped to the
                        // oldest retained event; the modern stream has no
                        // replay, so surface the gap instead of continuing
                        // silently.
                        let notification =
                            crate::mcp::notifications::resources_dropped(&vault_id, &listen.id);
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
                    None => break,
                }
            }
        }
    }
}

/// Parsed `Last-Event-ID` header (M-3). `Absent` means no resume was
/// requested (plain live stream, no replay). A present-but-unusable value
/// (`Invalid`: empty, contains `:`, or not a 40-hex OID) must NOT degrade
/// to "no resume requested" — the client explicitly asked to resume, so it
/// gets the documented `lagged` signal and no replay. `Commit` is a
/// syntactically valid OID that replay will try to locate.
enum LastEventId {
    Absent,
    /// The offending value is carried for debuggability (matching the
    /// audit's enum shape) but deliberately never echoed into any
    /// response: the lagged contract carries no payload.
    #[allow(dead_code)]
    Invalid(String),
    Commit(String),
}

fn mcp_last_event_id(headers: &HeaderMap) -> LastEventId {
    let Some(header_value) = headers.get("last-event-id") else {
        return LastEventId::Absent;
    };
    // The public docs reject `:`-bearing values as resume positions, and a
    // commit id here is a bare SHA-1 OID (40 hex digits).
    let raw = header_value.to_str().ok().unwrap_or_default().trim();
    if raw.len() == 40 && !raw.contains(':') && raw.bytes().all(|b| b.is_ascii_hexdigit()) {
        LastEventId::Commit(raw.to_string())
    } else {
        LastEventId::Invalid(raw.to_string())
    }
}

/// Whether `commit` exists in the vault's object database (M-3). An empty
/// replay batch is ambiguous — "nothing was missed" and "cannot locate the
/// commit" (GC'd history, rollback, state imported from another server)
/// both come back as `Events(vec![])` — so this distinguishes them and lets
/// an unlocatable resume position surface `lagged` instead of a silent
/// plain live stream. Any failure counts as absent: the honest direction
/// for a repo we cannot inspect is a resync, never silence.
async fn commit_exists_in_vault(state: &AppState, vault_id: &str, commit: &str) -> bool {
    let Ok(vault_path) = crate::storage::git::storage_vault_path(state.vault_root(), vault_id)
    else {
        return false;
    };
    let Ok(oid) = git2::Oid::from_str(commit) else {
        return false;
    };
    tokio::task::spawn_blocking(move || {
        git2::Repository::open_bare(vault_path)
            .map(|repo| repo.find_commit(oid).is_ok())
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

async fn mcp_replay_events_after(
    state: &AppState,
    vaults: &[crate::db::repos::Vault],
    commit: &str,
) -> anyhow::Result<(
    Option<Vec<(String, crate::service::events::VaultEvent)>>,
    bool,
)> {
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
                return Ok((None, false));
            }
        }
    }
    // A non-empty batch proves some vault located the commit, so the
    // existence check only runs in the empty case (the common caught-up
    // reconnect): one git oid-exists lookup per vault until one claims it.
    // If no vault the token can see contains it, the resume position is
    // unlocatable — report it like a replay lag rather than silence (M-3).
    if events_all.is_empty() {
        let mut known_to_any_vault = false;
        for vault in vaults {
            if commit_exists_in_vault(state, &vault.id, commit).await {
                known_to_any_vault = true;
                break;
            }
        }
        if !known_to_any_vault {
            return Ok((None, false));
        }
    }
    // Only the vault whose history contains `commit` yields events; with more
    // than one vault the others' positions are unknown and the caller appends
    // a lagged signal (BUG-R3-13).
    let unmatched_vaults = vaults.len() > 1;
    Ok((Some(events_all), unmatched_vaults))
}

/// Authenticate the request's `Authorization` header, mapping both failures
/// (missing header, rejected token) onto the 401 envelope the POST and SSE
/// entry points share (R-1). Returns the parsed credential alongside the
/// user so callers that need the token hash (the listen path) do not parse
/// the header twice.
async fn bearer_user<'a>(
    state: &AppState,
    headers: &'a HeaderMap,
    limit_key: &str,
) -> Result<(AuthenticatedUser, &'a str), Box<Response>> {
    let Some(raw) = bearer_token(headers) else {
        return Err(Box::new(
            (
                StatusCode::UNAUTHORIZED,
                Json(jsonrpc_error(Value::Null, ERR_AUTH, "missing bearer token")),
            )
                .into_response(),
        ));
    };
    match authenticate_token(state, raw, limit_key).await {
        Ok(user) => Ok((user, raw)),
        Err(err) => {
            let message = err.to_string();
            Err(Box::new(
                (
                    StatusCode::UNAUTHORIZED,
                    Json(jsonrpc_error(
                        Value::Null,
                        ERR_AUTH,
                        mcp_auth_error_public_message(&message),
                    )),
                )
                    .into_response(),
            ))
        }
    }
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

    #[test]
    fn replay_suppression_set_keys_by_vault_and_commit_and_skips_lagged() {
        // BUG-5 regression guard: a commit id is a git OID, and two vaults
        // initialized from the same template in the same second with the
        // same message produce the same root OID, so keying suppression on
        // the commit alone let vault A's replay suppress vault B's live
        // event carrying the identical id. Drives the production
        // constructor with constructed items — no real git OIDs involved —
        // so the assertions run unconditionally on every runner.
        let replay_items = VecDeque::from([
            McpSseItem::VaultChanged {
                vault_id: "vault-a".into(),
                commit: "c1".into(),
                data: "{}".into(),
            },
            // Trailing lag marker exactly as the GET handler assembles a
            // replay batch (replay-level lag is vault-agnostic, `None`).
            McpSseItem::Lagged { vault_id: None },
        ]);
        let replay_set = replay_suppression_set(&replay_items);
        assert!(
            replay_set.contains(&("vault-a".to_string(), "c1".to_string())),
            "the replayed (vault, commit) pair must be in the suppression set"
        );
        assert!(
            !replay_set.contains(&("vault-b".to_string(), "c1".to_string())),
            "vault B's live event with the identical commit id must not be \
             suppressed by vault A's replay"
        );
        assert_eq!(
            replay_set.len(),
            1,
            "Lagged markers must not enter the suppression set: {replay_set:?}"
        );
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

    #[tokio::test]
    async fn modern_tools_list_returns_decorated_result() {
        let state = state().await;
        let (_user_id, raw) = create_user_token(&state, "u1").await;
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
        let (_user_id, raw) = create_user_token(&state, "u1").await;
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
            serde_json::json!(["2026-07-28"])
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
    async fn modern_cancelled_notification_is_405_with_sse_guidance() {
        let state = state().await;
        let (_user_id, raw) = create_user_token(&state, "u1").await;
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
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        let body = response_json(response).await;
        assert!(
            body.get("id").is_none(),
            "notification error envelope must not carry an id: {body}"
        );
        let message = body["error"]["message"].as_str().unwrap();
        assert!(
            message.contains("closing the SSE"),
            "message must point at closing the SSE stream: {message}"
        );
    }

    #[tokio::test]
    async fn modern_initialized_notification_returns_202_with_empty_body() {
        let state = state().await;
        let (_user_id, raw) = create_user_token(&state, "u1").await;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let body = serde_json::json!({
            "jsonrpc": "2.0", "method": "notifications/initialized", "params": {}
        });
        let response = app
            .oneshot(modern_post(
                &body.to_string(),
                "notifications/initialized",
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
        let (_user_id, raw) = create_user_token(&state, "u1").await;
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
        let (_user_id, raw) = create_user_token(&state, "u1").await;
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
        let (_user_id, raw) = create_user_token(&state, "u1").await;
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
            serde_json::json!(["2026-07-28"])
        );
    }

    #[tokio::test]
    async fn unknown_method_modern_is_404_method_not_found() {
        let state = state().await;
        let (_user_id, raw) = create_user_token(&state, "u1").await;
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

    #[tokio::test]
    async fn modern_header_with_legacy_body_is_rejected_not_downgraded() {
        let state = state().await;
        let (_user_id, raw) = create_user_token(&state, "u1").await;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}
        });
        let request = HttpRequest::builder()
            .method("POST")
            .uri("/mcp")
            .header("content-type", "application/json")
            .header(deployment_key::HEADER, "k_test")
            .header(header::AUTHORIZATION, format!("Bearer {raw}"))
            .header("MCP-Protocol-Version", "2026-07-28")
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], -32020);
        let message = body["error"]["message"].as_str().unwrap();
        assert!(
            message.contains("modern version") && message.contains("_meta"),
            "message must explain the header/body era mismatch: {message}"
        );
    }

    #[tokio::test]
    async fn legacy_tools_call_invalid_params_stays_200_wrapped() {
        let state = state().await;
        let (_user_id, raw) = create_user_token(&state, "u1").await;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": 5, "arguments": {}}
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
        assert_eq!(body["error"]["code"], -32602);
        let message = body["error"]["message"].as_str().unwrap();
        assert!(
            message.starts_with("invalid params"),
            "expected invalid-params envelope, got: {message}"
        );
    }

    #[tokio::test]
    async fn zero_vault_get_sse_stream_stays_open() {
        let state = state().await;
        let (_user_id, raw) = create_user_token(&state, "zero-vault-sse").await;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let request = HttpRequest::builder()
            .method("GET")
            .uri("/mcp")
            .header(header::ACCEPT, "text/event-stream")
            .header(deployment_key::HEADER, "k_test")
            .header(header::AUTHORIZATION, format!("Bearer {raw}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut stream = response.into_body().into_data_stream();
        let mut received: Vec<Result<axum::body::Bytes, axum::Error>> = Vec::new();
        let read = tokio::time::timeout(Duration::from_millis(1500), async {
            while let Some(chunk) = stream.next().await {
                received.push(chunk);
            }
        })
        .await;
        assert!(
            read.is_err(),
            "zero-vault GET /mcp stream must stay open, got EOF with body: {received:?}"
        );
    }

    #[tokio::test]
    async fn get_sse_live_lagged_broadcast_emits_lagged_event() {
        let ls = listen_server().await;
        let app = listen_router(&ls.state);

        let request = HttpRequest::builder()
            .method("GET")
            .uri("/mcp")
            .header(header::ACCEPT, "text/event-stream")
            .header(deployment_key::HEADER, "k_test")
            .header(header::AUTHORIZATION, format!("Bearer {}", ls.raw))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut stream = response.into_body().into_data_stream();

        // Forcing a real broadcast overflow needs the stream task parked off
        // its receiver: fill the 16-slot outbound channel without reading so
        // the task blocks inside `tx.send`, then burst past the 64-slot ring.
        // Reading afterwards releases the parked send and the next poll of
        // the broadcast receiver reports the lag.
        let lag_burst = |start: usize| crate::service::events::VaultEvent {
            commit: format!("get-lag-{start}"),
            parent: None,
            source_device_id: "dev".into(),
            at: 1,
            kind: crate::service::events::EventKind::Commit,
            changes: vec![],
        };
        for idx in 0..20 {
            ls.state.events.publish(&ls.vault_id, lag_burst(idx));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        for idx in 20..120 {
            ls.state.events.publish(&ls.vault_id, lag_burst(idx));
        }

        let mut body = String::new();
        read_sse_until(
            &mut body,
            &mut stream,
            |body| body.contains("event: lagged"),
            "lagged event",
        )
        .await;

        // N-3: the lag signal must carry the affected vault's identity so
        // multi-vault clients know which vault to resync instead of rereading
        // everything.
        let expected = format!("data: {}", ls.vault_id);
        assert!(
            body.contains(&expected),
            "live lag event must name the lagging vault ({expected}), got: {body}"
        );
    }

    fn get_sse(raw: &str, last_event_id: Option<&str>) -> HttpRequest<Body> {
        let mut builder = HttpRequest::builder()
            .method("GET")
            .uri("/mcp")
            .header(header::ACCEPT, "text/event-stream")
            .header(deployment_key::HEADER, "k_test")
            .header(header::AUTHORIZATION, format!("Bearer {raw}"));
        if let Some(id) = last_event_id {
            builder = builder.header("Last-Event-ID", id);
        }
        builder.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn get_sse_unparsable_last_event_id_signals_lagged() {
        // M-3: a Last-Event-ID containing ':' cannot name a resume
        // position. It used to be read as "no resume requested" — a plain
        // live stream with no lag marker, so a client that asked to resume
        // believed it was caught up. The stream's first event must be the
        // documented `lagged` signal (full resync).
        let ls = listen_server().await;
        let app = listen_router(&ls.state);
        // A commit exists and must NOT be replayed: replay is skipped
        // entirely when the resume position is unusable.
        commit_to_vault(&ls).await;

        let response = app
            .oneshot(get_sse(&ls.raw, Some("bad:value")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut stream = response.into_body().into_data_stream();
        let mut body = String::new();
        read_sse_until(
            &mut body,
            &mut stream,
            |body| body.contains("event: lagged"),
            "lagged event",
        )
        .await;

        assert!(
            body.trim_start().starts_with("event: lagged"),
            "lagged must be the first event, got: {body}"
        );
        assert!(
            body.contains("event: lagged\ndata: \n"),
            "replay-level lag is vault-agnostic and keeps empty data, got: {body}"
        );
        assert!(
            !body.contains("event: vault_changed"),
            "an unusable resume position must not replay anything, got: {body}"
        );
    }

    #[tokio::test]
    async fn get_sse_unknown_commit_last_event_id_signals_lagged() {
        // M-3: a well-formed 40-hex OID that no vault the token can see
        // contains (GC'd history, rollback, or state imported from another
        // server) used to replay as an empty batch with no marker — a
        // silent permanent fork for single-vault clients. It must surface
        // `lagged` too.
        let ls = listen_server().await;
        let app = listen_router(&ls.state);
        commit_to_vault(&ls).await;

        let unknown = "0".repeat(40);
        let response = app.oneshot(get_sse(&ls.raw, Some(&unknown))).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut stream = response.into_body().into_data_stream();
        let mut body = String::new();
        read_sse_until(
            &mut body,
            &mut stream,
            |body| body.contains("event: lagged"),
            "lagged event",
        )
        .await;

        assert!(
            body.trim_start().starts_with("event: lagged"),
            "lagged must be the first event, got: {body}"
        );
        assert!(
            !body.contains("event: vault_changed"),
            "an unlocatable resume position must not replay anything, got: {body}"
        );
    }

    #[tokio::test]
    async fn get_sse_caught_up_last_event_id_signals_no_lag_and_stays_live() {
        // M-3 control: resuming at the current head is the normal
        // caught-up reconnect — no lagged marker may be emitted (that
        // would force a pointless full resync), and the live stream must
        // keep delivering later commits.
        let ls = listen_server().await;
        let app = listen_router(&ls.state);
        let head = commit_to_vault(&ls).await;

        let response = app.oneshot(get_sse(&ls.raw, Some(&head))).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut stream = response.into_body().into_data_stream();
        let mut body = String::new();

        let next = commit_to_vault_after(&ls, Some(&head)).await;
        read_sse_until(
            &mut body,
            &mut stream,
            |body| body.contains(&format!("id: {next}")),
            "live commit after caught-up reconnect",
        )
        .await;

        assert!(
            !body.contains("event: lagged"),
            "a caught-up reconnect must not signal lag, got: {body}"
        );
        assert!(
            !body.contains(&format!("id: {head}")),
            "the resume commit itself must not be replayed, got: {body}"
        );
    }

    #[tokio::test]
    async fn post_mcp_authenticates_before_reading_the_body() {
        // M-4: authentication needs only headers, but the handler used to
        // buffer (up to ~100 MiB) and parse the body first — so a request
        // with invalid credentials and an unparsable body got the 400
        // parse error instead of 401, and unauthenticated clients could
        // make the server do the body work. 401 must win.
        let state = state().await;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let request = HttpRequest::builder()
            .method("POST")
            .uri("/mcp")
            .header("content-type", "application/json")
            .header(deployment_key::HEADER, "k_test")
            .header(header::AUTHORIZATION, "Bearer not-a-real-token")
            .body(Body::from("{not json"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn bearer_scheme_match_is_case_insensitive() {
        // L-1: RFC 9110 §11.1 makes the auth-scheme case-insensitive;
        // `bearer <token>` used to be rejected as "missing bearer token".
        let state = state().await;
        let (_user_id, raw) = create_user_token(&state, "u1").await;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}
        })
        .to_string();
        for scheme in ["bearer", "BEARER", "BeArEr"] {
            let request = HttpRequest::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .header(deployment_key::HEADER, "k_test")
                .header(header::AUTHORIZATION, format!("{scheme} {raw}"))
                .body(Body::from(body.clone()))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "scheme {scheme:?} must authenticate like Bearer"
            );
        }
    }

    #[tokio::test]
    async fn legacy_tools_call_array_arguments_is_invalid_params() {
        // L-5 (HTTP legacy era): `arguments: [1,2]` used to fail inside
        // the per-tool from_value and surface as -32000 "internal server
        // error" with an ERROR log — a client mistake misreported as
        // server breakage. It must be -32602 like other params errors.
        let state = state().await;
        let (_user_id, raw) = create_user_token(&state, "u1").await;
        let app = router_with_rate_limiter(
            state,
            crate::middleware::rate_limit::RequestRateLimiter::mcp_http(),
            "k_test".into(),
        );
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "list_files", "arguments": [1, 2]}
        })
        .to_string();
        let request = HttpRequest::builder()
            .method("POST")
            .uri("/mcp")
            .header("content-type", "application/json")
            .header(deployment_key::HEADER, "k_test")
            .header(header::AUTHORIZATION, format!("Bearer {raw}"))
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], -32602);
        assert_eq!(
            body["error"]["message"], "invalid params: arguments must be an object",
            "got: {body}"
        );
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
        commit_to_vault_after(ls, None).await
    }

    async fn commit_to_vault_after(ls: &ListenServer, parent: Option<&str>) -> String {
        use crate::service::sync::{push, PushChange, PushReq};
        push(
            &ls.state,
            &auth_for(&ls.user_id),
            &ls.vault_id,
            parent,
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

    /// Accumulate SSE chunks into `body` with a 5s deadline until `predicate`
    /// matches the accumulated buffer. Panics on timeout (message includes
    /// the buffer), on body-stream errors, and on EOF before the predicate
    /// matches; `what` names the awaited thing in those panic messages.
    async fn read_sse_until(
        body: &mut String,
        stream: &mut axum::body::BodyDataStream,
        predicate: impl Fn(&str) -> bool,
        what: &str,
    ) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let chunk = tokio::select! {
                chunk = stream.next() => chunk,
                _ = tokio::time::sleep_until(deadline) => {
                    panic!("no {what} within 5s, got: {body}")
                }
            };
            match chunk {
                Some(Ok(bytes)) => {
                    body.push_str(&String::from_utf8_lossy(&bytes));
                    if predicate(body) {
                        return;
                    }
                }
                Some(Err(err)) => panic!("body stream error: {err}"),
                None => panic!("stream ended before {what}, got: {body}"),
            }
        }
    }

    /// Accumulate SSE chunks into `body` until it holds at least one
    /// parseable `data:` JSON event and return that first event. Same panic
    /// semantics as [`read_sse_until`].
    async fn next_sse_event(
        body: &mut String,
        stream: &mut axum::body::BodyDataStream,
        what: &str,
    ) -> Value {
        read_sse_until(body, stream, |body| !parse_sse_data(body).is_empty(), what).await;
        parse_sse_data(body)
            .into_iter()
            .next()
            .expect("read_sse_until returned without a parsed event")
    }

    #[tokio::test]
    async fn legacy_initialize_echoes_requested_version() {
        let state = state().await;
        let (_user_id, raw) = create_user_token(&state, "u1").await;
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

        let advertised = app
            .clone()
            .oneshot(post_initialize(crate::mcp::protocol::MODERN_VERSION))
            .await
            .unwrap();
        assert_eq!(advertised.status(), StatusCode::OK);
        let advertised = response_json(advertised).await;
        assert_eq!(
            advertised["result"]["protocolVersion"],
            crate::mcp::protocol::MODERN_VERSION
        );

        let unknown = app.oneshot(post_initialize("1999-01-01")).await.unwrap();
        assert_eq!(unknown.status(), StatusCode::OK);
        let unknown = response_json(unknown).await;
        assert_eq!(unknown["result"]["protocolVersion"], "2025-11-25");
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
        let ack: Value = next_sse_event(&mut body, &mut stream, "ack").await;

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
        let update: Value = next_sse_event(&mut body, &mut stream, "update").await;

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
        let ack: Value = next_sse_event(&mut body, &mut stream, "ack").await;

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
    async fn listen_duplicate_uri_is_acked_once() {
        let ls = listen_server().await;
        let app = listen_router(&ls.state);
        let scoped = format!("pkv://vault/{}", ls.vault_id);

        let response = app
            .oneshot(listen_post(
                &ls.raw,
                serde_json::json!({
                    "resourceSubscriptions": [scoped.as_str(), scoped.as_str()]
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let mut stream = response.into_body().into_data_stream();
        let mut body = String::new();
        let ack: Value = next_sse_event(&mut body, &mut stream, "ack").await;

        assert_eq!(ack["method"], "notifications/subscriptions/acknowledged");
        assert_eq!(
            ack["params"]["notifications"]["resourceSubscriptions"],
            serde_json::json!([scoped]),
            "duplicate URIs must be acked exactly once: {body}"
        );
    }

    #[tokio::test]
    async fn listen_disconnect_releases_the_sse_subscriber_slot() {
        // SPEC-3: dropping the client side of a listen stream must end the
        // server task and release its SseSubscriberGuard — asserted by
        // re-acquiring the per-user slot the guard protects.
        let ls = listen_server().await;
        ls.state.set_sse_per_user_limit_for_tests(1);
        let app = listen_router(&ls.state);

        let response = app
            .oneshot(listen_post(
                &ls.raw,
                serde_json::json!({"resourceSubscriptions": [format!("pkv://vault/{}", ls.vault_id)]}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Read the ack event, then drop the response body: the client hung up.
        let body = response.into_body();
        let mut stream = body.into_data_stream();
        let mut received = String::new();
        read_sse_until(
            &mut received,
            &mut stream,
            |received| received.contains("notifications/subscriptions/acknowledged"),
            "ack",
        )
        .await;
        drop(stream);

        // The guard is released once the spawned stream task observes the
        // closed channel; poll within a window instead of assuming immediacy.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if ls.state.try_acquire_sse_subscriber(&ls.user_id).is_some() {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "SSE subscriber slot was not released within 5s of client disconnect"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
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
        let ack: Value = next_sse_event(&mut body, &mut stream, "ack").await;

        assert_eq!(ack["method"], "notifications/subscriptions/acknowledged");
        assert_eq!(
            ack["params"]["notifications"]["resourceSubscriptions"],
            serde_json::json!([format!("pkv://vault/{}", ls.vault_id)]),
            "unknown vault URI must be dropped from the ack: {body}"
        );
    }

    #[tokio::test]
    async fn listen_stream_lagged_broadcast_emits_resources_dropped_notification() {
        fn parse_complete_sse_data(body: &str) -> Vec<Value> {
            let mut segments = body.split("\n\n").collect::<Vec<_>>();
            if !body.ends_with("\n\n") {
                segments.pop();
            }
            segments
                .into_iter()
                .filter_map(|event| {
                    let data = event
                        .lines()
                        .filter_map(|line| line.strip_prefix("data: "))
                        .collect::<Vec<_>>()
                        .join("\n");
                    if data.is_empty() {
                        None
                    } else {
                        serde_json::from_str(&data).ok()
                    }
                })
                .collect()
        }

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
        let mut stream = response.into_body().into_data_stream();
        let mut body = String::new();
        let ack: Value = next_sse_event(&mut body, &mut stream, "ack").await;
        assert_eq!(
            ack["method"], "notifications/subscriptions/acknowledged",
            "first SSE event must be the ack, got: {body}"
        );

        // Forcing a real broadcast overflow needs the SSE task parked off its
        // receiver: fill the 16-slot outbound channel without reading so the
        // task blocks inside `tx.send`, then burst past the 64-slot ring.
        // Reading afterwards releases the parked send and the next poll of
        // the broadcast receiver reports the lag.
        for idx in 0..20 {
            ls.state.events.publish(
                &ls.vault_id,
                crate::service::events::VaultEvent {
                    commit: format!("lag-{idx}"),
                    parent: None,
                    source_device_id: "dev".into(),
                    at: 1,
                    kind: crate::service::events::EventKind::Commit,
                    changes: vec![],
                },
            );
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        for idx in 20..120 {
            ls.state.events.publish(
                &ls.vault_id,
                crate::service::events::VaultEvent {
                    commit: format!("lag-{idx}"),
                    parent: None,
                    source_device_id: "dev".into(),
                    at: 1,
                    kind: crate::service::events::EventKind::Commit,
                    changes: vec![],
                },
            );
        }
        let mut tail = String::new();
        // Deliberately not `read_sse_until`/`next_sse_event`: this loop reads
        // past the ack into a second buffer, only counts *complete* events
        // (parse_complete_sse_data drops a trailing partial segment), and its
        // EOF/timeout panics carry different text (`.expect("stream ended
        // without a dropped signal")`, `{body}{tail}` in the timeout message).
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let dropped = loop {
            let chunk = tokio::select! {
                chunk = stream.next() => chunk.expect("stream ended without a dropped signal"),
                _ = tokio::time::sleep_until(deadline) => {
                    panic!("no resources/dropped within 5s, got: {body}{tail}")
                }
            };
            match chunk {
                Ok(bytes) => {
                    tail.push_str(&String::from_utf8_lossy(&bytes));
                    if let Some(event) = parse_complete_sse_data(&tail)
                        .into_iter()
                        .find(|event| event["method"] == "notifications/resources/dropped")
                    {
                        break event;
                    }
                }
                Err(err) => panic!("body stream error: {err}"),
            }
        };

        assert_eq!(
            dropped["params"]["uri"],
            format!("pkv://vault/{}", ls.vault_id)
        );
        assert_eq!(
            dropped["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"],
            1
        );
    }

    #[tokio::test]
    async fn legacy_listen_is_method_not_found() {
        let state = state().await;
        let (_user_id, raw) = create_user_token(&state, "u1").await;
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
