use crate::middleware::deployment_key;
use crate::service::AppState;
use axum::body::Body;
use axum::extract::Request;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
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

pub async fn run(state: AppState, bind: SocketAddr, deployment_key: String) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, router(state, deployment_key).into_make_service()).await?;
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
    let user = match authenticate_token(&state, raw).await {
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
    req: Request,
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
    next.run(req).await
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
    let user = match authenticate_token(&state, raw).await {
        Ok(user) => user,
        Err(err) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(jsonrpc_error(Value::Null, -32001, &err.to_string())),
            )
                .into_response();
        }
    };
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
