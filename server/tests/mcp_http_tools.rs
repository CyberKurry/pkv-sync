use axum::body::{to_bytes, Body};
use axum::http::{HeaderMap, Request, StatusCode};
use pkv_sync_server::auth::{password, token};
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo, VaultRepo};
use pkv_sync_server::mcp::transport_http;
use pkv_sync_server::service::events::MAX_SSE_REPLAY_COMMITS;
use pkv_sync_server::service::sync::{self, PushChange, PushReq};
use pkv_sync_server::service::{vault, AppState};
use pkv_sync_server::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};
use serde_json::{json, Value};
use std::time::Duration;
use tower::ServiceExt;

async fn test_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let p = pkv_sync_server::db::pool::connect(&tmp.path().join("test.db"))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    let state = AppState::new(p, tmp.path().to_path_buf(), "test".into(), true)
        .await
        .unwrap();
    (state, tmp)
}

async fn create_user_with_token(state: &AppState, username: &str) -> (String, String) {
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
            device_id: "http-device",
            device_name: "http",
        })
        .await
        .unwrap();
    (user.id, raw)
}

async fn authenticated_user(
    state: &AppState,
    raw: &str,
    username: &str,
) -> pkv_sync_server::auth::AuthenticatedUser {
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
    pkv_sync_server::auth::AuthenticatedUser {
        user_id: user.id,
        username: user.username,
        is_admin: user.is_admin,
        token_id: token_row.id,
        device_id: token_row.device_id,
    }
}

async fn post_mcp(
    state: AppState,
    raw: Option<&str>,
    body: Value,
) -> (StatusCode, HeaderMap, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json");
    if let Some(raw) = raw {
        req = req.header("authorization", format!("Bearer {raw}"));
    }
    let response = transport_http::router(state)
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}));
    (status, headers, json)
}

#[tokio::test]
async fn http_mcp_requires_bearer_auth_per_post() {
    let (state, _tmp) = test_state().await;

    let (status, _headers, body) = post_mcp(
        state,
        None,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], -32001);
}

#[tokio::test]
async fn http_mcp_lists_tools_and_does_not_create_session_state() {
    let (state, _tmp) = test_state().await;
    let (_user_id, raw) = create_user_with_token(&state, "http-list").await;

    let (status, headers, body) = post_mcp(
        state,
        Some(&raw),
        json!({
            "jsonrpc": "2.0",
            "id": "tools",
            "method": "tools/list"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(headers.get("mcp-session-id").is_none());
    let tool_names = body["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"list_vaults"));
    assert!(tool_names.contains(&"read_file"));
    assert!(tool_names.contains(&"search"));
}

#[tokio::test]
async fn http_mcp_calls_read_only_tool_with_authenticated_user() {
    let (state, _tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "http-call").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    git.commit_changes(
        &vault.id,
        None,
        &[FileChange::Upsert {
            path: "note.md".into(),
            file: StoredFile::Text {
                bytes: b"hello mcp".to_vec(),
            },
        }],
        "seed",
    )
    .await
    .unwrap();

    let (status, headers, body) = post_mcp(
        state,
        Some(&raw),
        json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "tools/call",
            "params": {
                "name": "list_files",
                "arguments": {
                    "vault_id": vault.id
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(headers.get("mcp-session-id").is_none());
    assert_eq!(
        body["result"]["structuredContent"]["paths"],
        json!(["note.md"])
    );
}

#[tokio::test]
async fn http_mcp_tool_call_cannot_access_another_users_vault() {
    let (state, _tmp) = test_state().await;
    let (_alice_id, alice_raw) = create_user_with_token(&state, "http-alice").await;
    let (bob_id, _bob_raw) = create_user_with_token(&state, "http-bob").await;
    let bob_vault = state.vaults.create(&bob_id, "bob").await.unwrap();

    let (status, _headers, body) = post_mcp(
        state,
        Some(&alice_raw),
        json!({
            "jsonrpc": "2.0",
            "id": "forbidden",
            "method": "tools/call",
            "params": {
                "name": "list_files",
                "arguments": {
                    "vault_id": bob_vault.id
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["error"]["code"], -32000);
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("vault not found"));
}

async fn read_until(resp: reqwest::Response, needles: &[&str]) -> String {
    let mut body = String::new();
    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let chunk = tokio::select! {
            chunk = stream.next() => chunk,
            _ = tokio::time::sleep_until(deadline) => break,
        };
        match chunk {
            Some(Ok(bytes)) => {
                body.push_str(&String::from_utf8_lossy(&bytes));
                if needles.iter().all(|needle| body.contains(needle)) {
                    break;
                }
            }
            _ => break,
        }
    }
    body
}

#[tokio::test]
async fn http_mcp_sse_replays_after_last_event_id() {
    let (state, tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "http-sse-replay").await;
    let vault = vault::create_vault(&state, &user_id, "main").await.unwrap();
    let auth = authenticated_user(&state, &raw, "http-sse-replay").await;

    let first = sync::push(
        &state,
        &auth,
        &vault.id,
        None,
        None,
        PushReq {
            device_name: Some("first".into()),
            changes: vec![PushChange::Text {
                path: "a.md".into(),
                content: "one".into(),
            }],
        },
    )
    .await
    .unwrap();
    let second = sync::push(
        &state,
        &auth,
        &vault.id,
        Some(&first.new_commit),
        None,
        PushReq {
            device_name: Some("second".into()),
            changes: vec![PushChange::Text {
                path: "b.md".into(),
                content: "two".into(),
            }],
        },
    )
    .await
    .unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_state = state.clone();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            transport_http::router(server_state).into_make_service(),
        )
        .await;
    });

    let resp = reqwest::Client::new()
        .get(format!("http://{addr}/mcp"))
        .bearer_auth(&raw)
        .header("accept", "text/event-stream")
        .header("last-event-id", &first.new_commit)
        .send()
        .await
        .unwrap();

    let body = read_until(
        resp,
        &[
            &format!("id: {}", second.new_commit),
            "notifications/vault_changed",
            "b.md",
        ],
    )
    .await;
    handle.abort();
    drop(tmp);

    assert!(
        body.contains(&format!("id: {}", second.new_commit)),
        "expected replayed MCP SSE id, got: {body}"
    );
    assert!(
        body.contains("b.md"),
        "expected replayed change, got: {body}"
    );
}

#[tokio::test]
async fn http_mcp_sse_too_far_behind_emits_lagged() {
    let (state, tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "http-sse-lagged").await;
    let vault = vault::create_vault(&state, &user_id, "main").await.unwrap();
    let auth = authenticated_user(&state, &raw, "http-sse-lagged").await;

    let first = sync::push(
        &state,
        &auth,
        &vault.id,
        None,
        None,
        PushReq {
            device_name: Some("first".into()),
            changes: vec![PushChange::Text {
                path: "a.md".into(),
                content: "one".into(),
            }],
        },
    )
    .await
    .unwrap();

    let mut parent = first.new_commit.clone();
    for idx in 0..=MAX_SSE_REPLAY_COMMITS {
        let pushed = sync::push(
            &state,
            &auth,
            &vault.id,
            Some(&parent),
            None,
            PushReq {
                device_name: Some(format!("overflow-{idx}")),
                changes: vec![PushChange::Text {
                    path: format!("overflow-{idx}.md"),
                    content: idx.to_string(),
                }],
            },
        )
        .await
        .unwrap();
        parent = pushed.new_commit;
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_state = state.clone();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            transport_http::router(server_state).into_make_service(),
        )
        .await;
    });

    let resp = reqwest::Client::new()
        .get(format!("http://{addr}/mcp"))
        .bearer_auth(&raw)
        .header("accept", "text/event-stream")
        .header("last-event-id", &first.new_commit)
        .send()
        .await
        .unwrap();

    let body = read_until(resp, &["event: lagged"]).await;
    handle.abort();
    drop(tmp);

    assert!(
        body.contains("event: lagged"),
        "expected replay overflow to emit lagged, got: {body}"
    );
}
