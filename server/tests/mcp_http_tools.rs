mod common;

use axum::body::{to_bytes, Body};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode};
use common::{auth_for as authenticated_user, create_user_with_token, test_state};
use pkv_sync_server::auth::token;
use pkv_sync_server::db::repos::{NewToken, RuntimeConfigRepo, TokenRepo, VaultRepo};
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
    assert_eq!(first.2["error"]["message"], "invalid or revoked token");
    assert_eq!(second.2["error"]["message"], "invalid or revoked token");
}

#[tokio::test]
async fn http_mcp_rejects_oversized_json_bodies() {
    // M-4 reordered the handler: authentication (header-only) runs before
    // the body is buffered, so a bad token now gets 401 without any body
    // work — covered by the in-module
    // `post_mcp_authenticates_before_reading_the_body` test. What this
    // test still pins is the size limit itself: an *authenticated* request
    // above `min(6*max_file_size+1 MiB, 100 MiB)` is rejected 413 before
    // the JSON parse.
    let (state, _tmp) = test_state().await;
    state
        .runtime_cfg_repo
        .set_max_file_size(1024, None)
        .await
        .unwrap();
    let cfg = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg).await;
    let (_user_id, raw) = create_user_with_token(&state, "oversized-body").await;
    let body = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list","padding":"{}"}}"#,
        "x".repeat(2 * 1024 * 1024)
    );
    let mut req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
        .header("authorization", format!("Bearer {raw}"))
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
    assert_eq!(body["error"]["code"], -63002);
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
async fn http_mcp_batch_array_is_rejected_not_accepted() {
    // R2-4 (the HTTP half of L-4): a JSON-RPC batch is a JSON array, so it has
    // no top-level `id` and used to fall into the notification branch: `202
    // Accepted`, no body, nothing executed. 202 is not an error a client would
    // retry or report, so a batching client waited forever. stdio already
    // answers `-32600 "batch requests are not supported"`; HTTP answers the
    // same JSON-RPC line.
    //
    // R3-6: the status is 200, not 400. An array cannot carry `params._meta`,
    // so it always classifies as legacy, and the legacy contract is "every
    // error comes back in a 200 JSON-RPC envelope". The transport spec agrees:
    // a POST body batching *requests* is answered with a JSON object, and only
    // input consisting solely of responses/notifications is rejected with an
    // HTTP error status.
    let (state, _tmp) = test_state().await;
    let (_user_id, raw) = create_user_with_token(&state, "http-batch").await;

    let batches = [
        json!([
            {"jsonrpc": "2.0", "id": 1, "method": "tools/list"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"}
        ]),
        // An empty batch is still a batch, not a notification.
        json!([]),
    ];
    for batch in batches {
        let (status, _headers, body) = post_mcp(state.clone(), Some(&raw), batch.clone()).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "batch {batch} must be rejected with a JSON-RPC error, got body: {body}"
        );
        assert_eq!(body["error"]["code"], -32600, "batch {batch}: {body}");
        assert!(
            body["error"]["message"].as_str().unwrap().contains("batch"),
            "the error must name the unsupported shape, got: {body}"
        );
    }
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
async fn http_mcp_write_files_oversized_file_reports_public_max_file_size_error() {
    // M-5 (HTTP surface): the batch write tool's size rejection must reach
    // the client as the whitelisted max_file_size message, not the generic
    // internal-error masking.
    let (state, _tmp) = test_state().await;
    state
        .runtime_cfg_repo
        .set_max_file_size(1024, None)
        .await
        .unwrap();
    let cfg = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg).await;
    let (user_id, raw) = create_user_with_token(&state, "http-write-files-oversize").await;
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
            "id": "write-files-oversize",
            "method": "tools/call",
            "params": {
                "name": "write_files",
                "arguments": {
                    "vault_id": vault.id,
                    "parent_commit": head,
                    "writes": [
                        { "path": "small.md", "content": "ok" },
                        { "path": "large.md", "content": "a".repeat(2048) }
                    ]
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["error"]["code"], -32000);
    let message = body["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("max_file_size"),
        "must be the public size error, got: {message}"
    );
    assert_ne!(message, "internal server error");
}

#[tokio::test]
async fn http_mcp_read_file_at_commit_with_bogus_commit_reports_public_invalid_commit() {
    // M-5 (HTTP surface): a bogus commit id must classify as
    // invalid_commit, not internal server error.
    let (state, _tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "http-read-bad-commit").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    git.commit_changes(
        &vault.id,
        None,
        &[FileChange::Upsert {
            path: "top.md".into(),
            file: StoredFile::Text {
                bytes: b"t".to_vec(),
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
            "id": "bad-commit",
            "method": "tools/call",
            "params": {
                "name": "read_file_at_commit",
                "arguments": {
                    "vault_id": vault.id,
                    "path": "top.md",
                    "commit": "not-a-commit-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["error"]["code"], -32000);
    let message = body["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("invalid_commit"),
        "must be invalid_commit, got: {message}"
    );
    assert_ne!(message, "internal server error");
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

/// Read an SSE response all the way to EOF within `timeout`, returning the
/// body plus whether EOF was actually reached: unlike `read_until`, hitting
/// the timeout is reported to the caller, because the assertion under test
/// is precisely that the stream ENDS.
async fn read_to_end_with_timeout(resp: reqwest::Response, timeout: Duration) -> (String, bool) {
    let mut body = String::new();
    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let chunk = tokio::select! {
            chunk = stream.next() => chunk,
            _ = tokio::time::sleep_until(deadline) => return (body, false),
        };
        match chunk {
            Some(Ok(bytes)) => body.push_str(&String::from_utf8_lossy(&bytes)),
            Some(Err(_)) => return (body, true),
            None => return (body, true),
        }
    }
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
        // A hang guard, not a performance assertion: CI runners run these
        // binaries in parallel on a cold cache with on-access virus scanning,
        // where one push can exceed a tight timeout (the same saturation that
        // produced `Db(PoolTimedOut)`). A test that should fail still fails.
        .timeout(Duration::from_secs(30))
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

/// R2-6: a token that can see no vaults has nothing to be behind on. The
/// existence probe used to run anyway and report "unlocatable", so every
/// reconnect of a zero-vault user emitted `lagged` — a full-resync client
/// would spin forever.
#[tokio::test]
async fn http_mcp_sse_zero_vault_reconnect_does_not_emit_lagged() {
    let (state, tmp) = test_state().await;
    let (_user_id, raw) = create_user_with_token(&state, "http-sse-zero-vault").await;
    // No vault created on purpose.

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

    // A well-formed but unlocatable resume position (40 hex zeros).
    let resp = reqwest::Client::new()
        .get(format!("http://{addr}/mcp"))
        .bearer_auth(&raw)
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
        .header("accept", "text/event-stream")
        .header("last-event-id", "0".repeat(40))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Give the stream a moment: nothing must arrive (no lagged event).
    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    let saw_anything = tokio::time::timeout(std::time::Duration::from_millis(300), async {
        while let Some(chunk) = stream.next().await {
            let text = String::from_utf8_lossy(&chunk.unwrap()).to_string();
            if text.contains("lagged") {
                return true;
            }
        }
        false
    })
    .await;
    handle.abort();
    drop(tmp);

    let lagged_seen = saw_anything.unwrap_or(false);
    assert!(
        !lagged_seen,
        "a zero-vault reconnect must not be told to resync"
    );
}

/// R3-2: an undecidable probe must NOT be answered with a plain live stream.
/// `replay_events_after` came back empty, so if the resume position cannot be
/// verified locally the client would silently lose every event between that
/// position and now — the invisible divergence M-3 removed. R2-6 introduced
/// the `Missing`/`Unknown` split (which is kept, for logging) but pointed
/// `Unknown` at silence; the loud direction is the correct one, and the
/// zero-vault case R2-6 was really about is covered by
/// `http_mcp_sse_zero_vault_reconnect_does_not_emit_lagged` above.
#[tokio::test]
async fn http_mcp_sse_probe_failure_emits_lagged() {
    let (state, tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "http-sse-probe-err").await;
    let vault = vault::create_vault(&state, &user_id, "main").await.unwrap();
    let auth = authenticated_user(&state, &raw, "http-sse-probe-err").await;
    let pushed = sync::push(
        &state,
        &auth,
        &vault.id,
        None,
        None,
        PushReq {
            device_name: Some("seed".into()),
            changes: vec![PushChange::Text {
                path: "a.md".into(),
                content: "one".into(),
            }],
        },
    )
    .await
    .unwrap();

    // Break the vault's storage so the probe cannot open the repository:
    // replace the bare repo path with a regular file.
    let vault_path = state.default_vault_root().join(&vault.id);
    std::fs::remove_dir_all(&vault_path).unwrap();
    std::fs::write(&vault_path, b"not a repo").unwrap();

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
        .header("last-event-id", &pushed.new_commit)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    let saw_lagged = tokio::time::timeout(std::time::Duration::from_millis(300), async {
        while let Some(chunk) = stream.next().await {
            let text = String::from_utf8_lossy(&chunk.unwrap()).to_string();
            if text.contains("event: lagged") {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    handle.abort();
    drop(tmp);

    assert!(
        saw_lagged,
        "an unverifiable resume position must ask for a resync instead of \
         silently streaming live (M-3/R3-2)"
    );
}

#[tokio::test]
async fn http_mcp_listen_stream_ends_on_server_shutdown() {
    // H-2: an open `subscriptions/listen` SSE stream is exactly the kind of
    // connection that outlives a SIGTERM, so axum's graceful shutdown never
    // finishes while the stream stays open. The listen loop must select on
    // the app shutdown watch (like the GET stream already does).
    let (state, tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "http-listen-shutdown").await;
    let vault = vault::create_vault(&state, &user_id, "main").await.unwrap();
    let scoped = format!("pkv://vault/{}", vault.id);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let mut shutdown_rx = state.shutdown.subscribe();
    let server_state = state.clone();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            transport_http::router(server_state, DEPLOYMENT_KEY.into()).into_make_service(),
        )
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        })
        .await;
    });

    let resp = reqwest::Client::new()
        .post(format!("http://{addr}/mcp"))
        .bearer_auth(&raw)
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
        .header("content-type", "application/json")
        .header("accept", "text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", "subscriptions/listen")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 31,
            "method": "subscriptions/listen",
            "params": {
                "notifications": {"resourceSubscriptions": [scoped]},
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    state.shutdown.trigger();

    let (body, ended) = read_to_end_with_timeout(resp, Duration::from_secs(5)).await;
    handle.abort();
    drop(tmp);
    assert!(
        ended,
        "listen SSE stream must END after shutdown instead of hanging open; body so far: {body}"
    );
    assert!(
        body.contains("notifications/subscriptions/acknowledged"),
        "listen must ack before shutdown ends the stream, got: {body}"
    );
    assert!(
        body.contains("\"jsonrpc\":\"2.0\""),
        "SSE notification payloads must carry the JSON-RPC version field: {body}"
    );
}

/// Serve the MCP router on an ephemeral port WITHOUT a graceful-shutdown
/// future, so a request still reaches the handler after
/// `state.shutdown.trigger()` has fired. The R2-2 window (signal first,
/// `subscribe()` second) cannot be reproduced behind a real graceful
/// shutdown: the listener would already be closed and the request would
/// never be served.
async fn serve_without_graceful_shutdown(
    state: AppState,
) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            transport_http::router(state, DEPLOYMENT_KEY.into()).into_make_service(),
        )
        .await;
    });
    (addr, handle)
}

#[tokio::test]
async fn http_mcp_get_stream_ends_when_shutdown_precedes_request() {
    // R2-2: shutdown can land in the window between the SIGTERM handler
    // calling `shutdown.trigger()` and this request subscribing to the watch
    // (token auth, vault listing, SSE-guard acquisition all happen in
    // between). A receiver created AFTER the trigger already has `true`
    // marked as seen, so `changed()` never fires — the SSE body never ends
    // and `with_graceful_shutdown` still waits for TimeoutStopSec/SIGKILL,
    // which is exactly what H-2 set out to remove. The existing
    // `..._ends_on_server_shutdown` test only covers signal-after-subscribe.
    let (state, tmp) = test_state().await;
    let (_user_id, raw) = create_user_with_token(&state, "http-sse-pre-shutdown-get").await;
    let (addr, handle) = serve_without_graceful_shutdown(state.clone()).await;

    // The real server subscribes to this watch at startup and holds the
    // receiver for its whole lifetime (`shutdown_signal` does
    // `state.shutdown.subscribe()` before it awaits the signal), and
    // `watch::Sender::send` is a no-op while `receiver_count() == 0`. Keeping
    // an equivalent receiver alive here is what makes `trigger()` latch: the
    // SSE handler's later `subscribe()` then starts out with `true` already
    // marked as seen, which is the R2-2 window.
    let _server_receiver = state.shutdown.subscribe();
    state.shutdown.trigger();
    assert!(
        *_server_receiver.borrow(),
        "precondition: the shutdown signal must latch while a receiver is live"
    );

    let resp = reqwest::Client::new()
        .get(format!("http://{addr}/mcp"))
        .bearer_auth(&raw)
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
        .header("accept", "text/event-stream")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let (body, ended) = read_to_end_with_timeout(resp, Duration::from_secs(5)).await;
    handle.abort();
    drop(tmp);
    assert!(
        ended,
        "GET SSE stream must END when shutdown was already triggered before the request; body so far: {body}"
    );
}

#[tokio::test]
async fn http_mcp_listen_stream_ends_when_shutdown_precedes_request() {
    // R2-2, listen side: same window as the GET test above, on the stream
    // that H-2's fix added an arm to. A triggered-before-subscribe receiver
    // must still close the body.
    let (state, tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "http-listen-pre-shutdown").await;
    let vault = vault::create_vault(&state, &user_id, "main").await.unwrap();
    let scoped = format!("pkv://vault/{}", vault.id);
    let (addr, handle) = serve_without_graceful_shutdown(state.clone()).await;

    // Same latch precondition as the GET test above: a live receiver (the
    // real server's `shutdown_signal` subscription) is what makes the
    // pre-request `trigger()` observable to a receiver that subscribes later.
    let _server_receiver = state.shutdown.subscribe();
    state.shutdown.trigger();
    assert!(
        *_server_receiver.borrow(),
        "precondition: the shutdown signal must latch while a receiver is live"
    );

    let resp = reqwest::Client::new()
        .post(format!("http://{addr}/mcp"))
        .bearer_auth(&raw)
        .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
        .header("content-type", "application/json")
        .header("accept", "text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", "subscriptions/listen")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 32,
            "method": "subscriptions/listen",
            "params": {
                "notifications": {"resourceSubscriptions": [scoped]},
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let (body, ended) = read_to_end_with_timeout(resp, Duration::from_secs(5)).await;
    handle.abort();
    drop(tmp);
    assert!(
        ended,
        "listen SSE stream must END when shutdown was already triggered before the request; body so far: {body}"
    );
}
