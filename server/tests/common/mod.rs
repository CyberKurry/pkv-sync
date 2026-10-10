//! Shared scaffolding for the `mcp_*` integration tests: a file-backed
//! AppState, a user with a bearer token, an AuthenticatedUser handle, and
//! SSE reading helpers. Kept as a plain module next to the tests so each
//! binary picks what it needs with `mod common;`; not every binary uses
//! every helper.
#![allow(dead_code)]

use futures_util::StreamExt;
use pkv_sync_server::auth::{password, token, AuthenticatedUser};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
use pkv_sync_server::service::AppState;
use serde_json::Value;
use std::time::Duration;

/// File-backed state in a fresh temp dir (matches the per-test isolation the
/// mcp_* suites were written against; `connect_memory` would share one
/// in-process database across every test in the binary).
pub async fn test_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let p = pool::connect(&tmp.path().join("test.db")).await.unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    let state = AppState::new(p, tmp.path().to_path_buf(), "test".into(), true)
        .await
        .unwrap();
    (state, tmp)
}

/// Create a user plus a fresh bearer token; returns (user_id, raw token).
pub async fn create_user_with_token(state: &AppState, username: &str) -> (String, String) {
    let user = state
        .users
        .create(NewUser {
            username: username.into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
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
            device_id: "mcp-test-device",
            device_name: "mcp-test",
        })
        .await
        .unwrap();
    (user.id, raw)
}

/// Resolve the AuthenticatedUser a request with this bearer token would
/// authenticate as (user + token row looked up from the database).
pub async fn auth_for(state: &AppState, raw: &str, username: &str) -> AuthenticatedUser {
    let user = state
        .users
        .find_by_username(username)
        .await
        .unwrap()
        .unwrap();
    let (token_row, _username) = state
        .tokens
        .find_by_hash(&token::hash(raw))
        .await
        .unwrap()
        .unwrap();
    AuthenticatedUser {
        user_id: user.id,
        username: user.username,
        is_admin: user.is_admin,
        token_id: token_row.id,
        device_id: token_row.device_id,
    }
}

/// Split an SSE body into its `data:` payloads. Incomplete trailing events
/// (a chunk boundary mid-event) are ignored, mirroring the per-file parsers
/// the mcp suites used.
pub fn parse_sse_data(body: &str) -> Vec<Value> {
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
                serde_json::from_str(&data).ok()
            }
        })
        .collect()
}

/// Read an axum response body stream until the first complete SSE event and
/// return it as JSON. `partial` accumulates the raw bytes (for panic
/// messages and later reads). Panics on error, EOF, or a 5s timeout.
pub async fn first_sse_event(
    stream: &mut axum::body::BodyDataStream,
    partial: &mut String,
) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let chunk = tokio::select! {
            chunk = stream.next() => chunk,
            _ = tokio::time::sleep_until(deadline) => {
                panic!("no SSE event within 5s, got: {partial}")
            }
        };
        match chunk {
            Some(Ok(bytes)) => {
                partial.push_str(&String::from_utf8_lossy(&bytes));
                if let Some(event) = parse_sse_data(partial).first() {
                    return event.clone();
                }
            }
            Some(Err(err)) => panic!("body stream error: {err}"),
            None => panic!("stream ended before an SSE event, got: {partial}"),
        }
    }
}
