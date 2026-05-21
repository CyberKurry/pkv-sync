use crate::service::AppState;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::Value;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use super::transport_stdio::{authenticate_token, handle_jsonrpc, jsonrpc_error};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/mcp", post(post_mcp).get(get_mcp_sse))
        .with_state(state)
}

pub async fn run(state: AppState, bind: SocketAddr) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, router(state).into_make_service()).await?;
    Ok(())
}

async fn post_mcp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Response {
    let Some(raw) = bearer(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(jsonrpc_error(Value::Null, -32001, "missing bearer token")),
        )
            .into_response();
    };
    let user_id = match authenticate_token(&state, raw).await {
        Ok(user_id) => user_id,
        Err(err) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(jsonrpc_error(Value::Null, -32001, &err.to_string())),
            )
                .into_response();
        }
    };
    Json(handle_jsonrpc(&state, &user_id, None, request).await).into_response()
}

async fn get_mcp_sse(State(state): State<AppState>, headers: HeaderMap) -> Response {
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
    let user_id = match authenticate_token(&state, raw).await {
        Ok(user_id) => user_id,
        Err(err) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(jsonrpc_error(Value::Null, -32001, &err.to_string())),
            )
                .into_response();
        }
    };
    let vaults = match crate::db::repos::VaultRepo::list_for_user(&*state.vaults, &user_id).await {
        Ok(vaults) => vaults,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(jsonrpc_error(Value::Null, -32603, &err.to_string())),
            )
                .into_response();
        }
    };
    let Some(sse_guard) = state.try_acquire_sse_subscriber() else {
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
    let replay_items = match replay_events {
        McpReplayEvents::Events(events) => events
            .into_iter()
            .filter_map(|(_vault_id, event)| {
                let commit = event.commit.clone();
                let notification = crate::mcp::notifications::vault_changed(commit.clone(), event);
                let data = serde_json::to_string(&notification).ok()?;
                Some(Ok::<Event, Infallible>(
                    Event::default()
                        .event("vault_changed")
                        .id(commit)
                        .data(data),
                ))
            })
            .collect(),
        McpReplayEvents::Lagged => vec![Ok(Event::default().event("lagged").data(""))],
    };
    let replay_stream = tokio_stream::iter(replay_items);
    let live_stream = streams.filter_map(|(_vault_id, event)| {
        event.ok().and_then(|event| {
            let commit = event.commit.clone();
            let notification = crate::mcp::notifications::vault_changed(commit.clone(), event);
            let data = serde_json::to_string(&notification).ok()?;
            Some(Ok::<Event, Infallible>(
                Event::default()
                    .event("vault_changed")
                    .id(commit)
                    .data(data),
            ))
        })
    });
    let stream = {
        let guard = sse_guard;
        replay_stream.chain(live_stream).map(move |item| {
            let _keep_alive = &guard;
            item
        })
    };
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
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
