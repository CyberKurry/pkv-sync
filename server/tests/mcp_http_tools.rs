use axum::body::{to_bytes, Body};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode};
use pkv_sync_server::auth::{password, token};
use pkv_sync_server::db::repos::{
    NewToken, NewUser, RuntimeConfigRepo, TokenRepo, UserRepo, VaultRepo,
};
use pkv_sync_server::mcp::transport_http;
use pkv_sync_server::service::events::MAX_SSE_REPLAY_COMMITS;
use pkv_sync_server::service::sync::{self, PushChange, PushReq};
use pkv_sync_server::service::{vault, AppState};
use pkv_sync_server::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::time::Duration;
use tower::ServiceExt;

const DEPLOYMENT_KEY: &str = "k_mcp_http_test";

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
    post_mcp_from_addr(state, raw, body, "127.0.0.1:50000".parse().unwrap()).await
}

async fn post_mcp_from_addr(
    state: AppState,
    raw: Option<&str>,
    body: Value,
    peer: SocketAddr,
) -> (StatusCode, HeaderMap, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY);
    if let Some(raw) = raw {
        req = req.header("authorization", format!("Bearer {raw}"));
    }
    let mut req = req.body(Body::from(body.to_string())).unwrap();
    req.extensions_mut().insert(ConnectInfo(peer));
    let response = transport_http::router(state, DEPLOYMENT_KEY.into())
        .oneshot(req)
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}));
    (status, headers, json)
}

async fn post_mcp_raw(
    state: AppState,
    raw: Option<&str>,
    body: String,
    content_type: &str,
) -> (StatusCode, HeaderMap, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", content_type)
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY);
    if let Some(raw) = raw {
        req = req.header("authorization", format!("Bearer {raw}"));
    }
    let mut req = req.body(Body::from(body)).unwrap();
    req.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:50000".parse::<SocketAddr>().unwrap(),
    ));
    let response = transport_http::router(state, DEPLOYMENT_KEY.into())
        .oneshot(req)
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}));
    (status, headers, json)
}

#[tokio::test]
async fn http_mcp_auth_failures_are_limited_per_client_source() {
    let (state, _tmp) = test_state().await;
    state
        .mcp_auth_limiter
        .update_config(1, Duration::from_secs(60), Duration::from_secs(60));

    let first = post_mcp_from_addr(
        state.clone(),
        Some("not-a-token"),
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        }),
        "127.0.0.1:50000".parse().unwrap(),
    )
    .await;
    let second = post_mcp_from_addr(
        state,
        Some("not-a-token"),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        }),
        "127.0.0.2:50000".parse().unwrap(),
    )
    .await;

    assert_eq!(first.0, StatusCode::UNAUTHORIZED);
    assert_eq!(second.0, StatusCode::UNAUTHORIZED);
    assert_eq!(first.2["error"]["message"], "invalid token format");
    assert_eq!(second.2["error"]["message"], "invalid token format");
}

#[tokio::test]
async fn http_mcp_rejects_oversized_json_bodies_before_auth() {
    let (state, _tmp) = test_state().await;
    state
        .runtime_cfg_repo
        .set_max_file_size(1024, None)
        .await
        .unwrap();
    let cfg = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg).await;
    let body = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list","padding":"{}"}}"#,
        "x".repeat(2 * 1024 * 1024)
    );
    let mut req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
        .header("authorization", "Bearer not-a-token")
        .body(Body::from(body))
        .unwrap();
    req.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:50000".parse::<SocketAddr>().unwrap(),
    ));

    let response = transport_http::router(state, DEPLOYMENT_KEY.into())
        .oneshot(req)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

async fn post_mcp_without_deployment_key(
    state: AppState,
    raw: Option<&str>,
    body: Value,
) -> StatusCode {
    let mut req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json");
    if let Some(raw) = raw {
        req = req.header("authorization", format!("Bearer {raw}"));
    }
    transport_http::router(state, DEPLOYMENT_KEY.into())
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
        .status()
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
async fn http_mcp_requires_deployment_key() {
    let (state, _tmp) = test_state().await;
    let (_user_id, raw) = create_user_with_token(&state, "http-deployment-key").await;

    let status = post_mcp_without_deployment_key(
        state,
        Some(&raw),
        json!({
            "jsonrpc": "2.0",
            "id": "tools",
            "method": "tools/list"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
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
async fn http_mcp_write_file_allows_body_above_one_mib_within_max_file_size() {
    let (state, _tmp) = test_state().await;
    state
        .runtime_cfg_repo
        .set_max_file_size(2 * 1024 * 1024, None)
        .await
        .unwrap();
    let cfg = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg).await;
    let (user_id, raw) = create_user_with_token(&state, "http-large-write").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    let head = git
        .commit_changes(
            &vault.id,
            None,
            &[FileChange::Upsert {
                path: "seed.md".into(),
                file: StoredFile::Text {
                    bytes: b"seed".to_vec(),
                },
            }],
            "seed",
        )
        .await
        .unwrap();

    let (status, _headers, body) = post_mcp(
        state,
        Some(&raw),
        json!({
            "jsonrpc": "2.0",
            "id": "large-write",
            "method": "tools/call",
            "params": {
                "name": "write_file",
                "arguments": {
                    "vault_id": vault.id,
                    "path": "large.md",
                    "content": "a".repeat(1024 * 1024 + 128 * 1024),
                    "parent_commit": head
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.get("error").is_none(), "unexpected MCP error: {body}");
    assert!(body["result"]["structuredContent"]["commit"].is_string());
}

#[tokio::test]
async fn http_mcp_write_file_allows_escaped_json_body_within_max_file_size() {
    let (state, _tmp) = test_state().await;
    state
        .runtime_cfg_repo
        .set_max_file_size(1024 * 1024, None)
        .await
        .unwrap();
    let cfg = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg).await;
    let (user_id, raw) = create_user_with_token(&state, "http-escaped-write").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    let head = git
        .commit_changes(
            &vault.id,
            None,
            &[FileChange::Upsert {
                path: "seed.md".into(),
                file: StoredFile::Text {
                    bytes: b"seed".to_vec(),
                },
            }],
            "seed",
        )
        .await
        .unwrap();
    let escaped_content = "\\u0061".repeat(512 * 1024);
    let body = format!(
        r#"{{"jsonrpc":"2.0","id":"escaped-write","method":"tools/call","params":{{"name":"write_file","arguments":{{"vault_id":"{}","path":"escaped.md","content":"{}","parent_commit":"{}"}}}}}}"#,
        vault.id, escaped_content, head
    );

    let (status, _headers, body) = post_mcp_raw(state, Some(&raw), body, "application/json").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.get("error").is_none(), "unexpected MCP error: {body}");
    assert!(body["result"]["structuredContent"]["commit"].is_string());
}

#[tokio::test]
async fn http_mcp_rejects_non_json_content_type() {
    let (state, _tmp) = test_state().await;
    let (_user_id, raw) = create_user_with_token(&state, "http-content-type").await;

    let (status, _headers, _body) = post_mcp_raw(
        state,
        Some(&raw),
        r#"{"jsonrpc":"2.0","id":"ct","method":"tools/list"}"#.into(),
        "text/plain",
    )
    .await;

    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn http_mcp_accepts_case_insensitive_json_suffix_content_type() {
    let (state, _tmp) = test_state().await;
    let (_user_id, raw) = create_user_with_token(&state, "http-json-suffix").await;

    let (status, _headers, body) = post_mcp_raw(
        state,
        Some(&raw),
        r#"{"jsonrpc":"2.0","id":"suffix","method":"tools/list"}"#.into(),
        "application/problem+JSON; charset=utf-8",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["result"]["tools"].is_array());
}

#[tokio::test]
async fn http_mcp_write_file_reports_tool_error_above_max_file_size() {
    let (state, _tmp) = test_state().await;
    state
        .runtime_cfg_repo
        .set_max_file_size(1024, None)
        .await
        .unwrap();
    let cfg = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg).await;
    let (user_id, raw) = create_user_with_token(&state, "http-too-large-write").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    let head = git
        .commit_changes(
            &vault.id,
            None,
            &[FileChange::Upsert {
                path: "seed.md".into(),
                file: StoredFile::Text {
                    bytes: b"seed".to_vec(),
                },
            }],
            "seed",
        )
        .await
        .unwrap();

    let (status, _headers, body) = post_mcp(
        state,
        Some(&raw),
        json!({
            "jsonrpc": "2.0",
            "id": "too-large-write",
            "method": "tools/call",
            "params": {
                "name": "write_file",
                "arguments": {
                    "vault_id": vault.id,
                    "path": "too-large.md",
                    "content": "a".repeat(2048),
                    "parent_commit": head
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
        .contains("file exceeds max_file_size of 1024 bytes"));
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
            transport_http::router(server_state, DEPLOYMENT_KEY.into()).into_make_service(),
        )
        .await;
    });

    let resp = reqwest::Client::new()
        .get(format!("http://{addr}/mcp"))
        .bearer_auth(&raw)
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
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
async fn http_mcp_sse_rejects_when_subscriber_limit_is_reached() {
    let (state, tmp) = test_state().await;
    state.set_sse_per_user_limit_for_tests(1);
    let (user_id, raw) = create_user_with_token(&state, "http-sse-limit").await;
    let _vault = vault::create_vault(&state, &user_id, "main").await.unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_state = state.clone();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            transport_http::router(server_state, DEPLOYMENT_KEY.into()).into_make_service(),
        )
        .await;
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();

    let first = client
        .get(format!("http://{addr}/mcp"))
        .bearer_auth(&raw)
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
        .header("accept", "text/event-stream")
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = client
        .get(format!("http://{addr}/mcp"))
        .bearer_auth(&raw)
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
        .header("accept", "text/event-stream")
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);

    drop(first);
    tokio::time::sleep(Duration::from_millis(50)).await;

    let third = client
        .get(format!("http://{addr}/mcp"))
        .bearer_auth(&raw)
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
        .header("accept", "text/event-stream")
        .send()
        .await
        .unwrap();
    assert_eq!(third.status(), StatusCode::OK);

    handle.abort();
    drop(tmp);
}

#[tokio::test]
async fn http_mcp_sse_closes_without_live_event_after_token_revoked() {
    let (state, tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "http-sse-revoked").await;
    let writer_raw = token::generate();
    state
        .tokens
        .create(NewToken {
            user_id: &user_id,
            token_hash: &token::hash(&writer_raw),
            device_id: "http-writer-device",
            device_name: "writer",
        })
        .await
        .unwrap();
    let vault = vault::create_vault(&state, &user_id, "main").await.unwrap();
    let sse_auth = authenticated_user(&state, &raw, "http-sse-revoked").await;
    let writer_auth = authenticated_user(&state, &writer_raw, "http-sse-revoked").await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_state = state.clone();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            transport_http::router(server_state, DEPLOYMENT_KEY.into()).into_make_service(),
        )
        .await;
    });

    let resp = reqwest::Client::new()
        .get(format!("http://{addr}/mcp"))
        .bearer_auth(&raw)
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
        .header("accept", "text/event-stream")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    state
        .tokens
        .revoke(&sse_auth.token_id, chrono::Utc::now().timestamp())
        .await
        .unwrap();
    let pushed = sync::push(
        &state,
        &writer_auth,
        &vault.id,
        None,
        None,
        PushReq {
            device_name: Some("writer".into()),
            changes: vec![PushChange::Text {
                path: "after-revoke.md".into(),
                content: "must not leak".into(),
            }],
        },
    )
    .await
    .unwrap();

    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let next = tokio::time::timeout(Duration::from_secs(2), stream.next()).await;
    handle.abort();
    drop(tmp);

    match next {
        Ok(None) => {}
        Ok(Some(Err(_))) => {}
        Ok(Some(Ok(bytes))) => panic!(
            "revoked MCP SSE stream leaked event {}: {}",
            pushed.new_commit,
            String::from_utf8_lossy(&bytes)
        ),
        Err(_) => panic!("revoked MCP SSE stream stayed open after a live event"),
    }
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
            transport_http::router(server_state, DEPLOYMENT_KEY.into()).into_make_service(),
        )
        .await;
    });

    let resp = reqwest::Client::new()
        .get(format!("http://{addr}/mcp"))
        .bearer_auth(&raw)
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
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
