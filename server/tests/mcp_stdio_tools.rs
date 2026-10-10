use pkv_sync_server::auth::{password, token};
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo, VaultRepo};
use pkv_sync_server::mcp::transport_stdio::StdioSession;
use pkv_sync_server::service::events::{EventKind, VaultEvent};
use pkv_sync_server::service::AppState;
use pkv_sync_server::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::mpsc;

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
            device_id: "stdio-device",
            device_name: "stdio",
        })
        .await
        .unwrap();
    (user.id, raw)
}

fn modern_request(id: Value, method: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientCapabilities": {}
        }}
    })
}

fn listen_request(id: Value, resource_subscriptions: Vec<&str>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "subscriptions/listen",
        "params": {
            "notifications": {"resourceSubscriptions": resource_subscriptions},
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {}
            }
        }
    })
}

async fn session_with_vault(
    username: &str,
) -> (StdioSession, AppState, String, String, tempfile::TempDir) {
    let (state, tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, username).await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let vault_id = vault.id.clone();
    let session = StdioSession::authenticate(state.clone(), vault.id, raw)
        .await
        .unwrap();
    (session, state, user_id, vault_id, tmp)
}

async fn send(session: &StdioSession, msg: Value) -> Vec<Value> {
    send_line(session, &msg.to_string()).await
}

async fn send_line(session: &StdioSession, line: &str) -> Vec<Value> {
    let (tx, mut rx) = mpsc::channel(16);
    let mut tx = tx;
    session.pump(line, &mut tx).await;
    drop(tx);
    let mut lines = Vec::new();
    while let Some(line) = rx.recv().await {
        lines.push(serde_json::from_str(&line).unwrap());
    }
    lines
}

fn auth_for(user_id: &str) -> pkv_sync_server::auth::AuthenticatedUser {
    pkv_sync_server::auth::AuthenticatedUser {
        user_id: user_id.into(),
        username: "stdio-listen".into(),
        is_admin: false,
        token_id: "stdio-listen-token".into(),
        device_id: "stdio-listen-device".into(),
    }
}

async fn commit_and_broadcast(state: &AppState, user_id: &str, vault_id: &str, tag: &str) {
    let user = auth_for(user_id);
    let git = Git2VaultStore::new(state.default_vault_root());
    let commit = git
        .commit_changes(
            vault_id,
            None,
            &[FileChange::Upsert {
                path: format!("{tag}.md"),
                file: StoredFile::Text {
                    bytes: format!("{tag}-body").into_bytes(),
                },
            }],
            "stdio-listen-test",
        )
        .await
        .unwrap();
    state.events.publish(
        vault_id,
        VaultEvent {
            commit,
            parent: None,
            source_device_id: user.device_id,
            at: 1,
            kind: EventKind::Commit,
            changes: vec![],
        },
    );
}

async fn assert_no_line(rx: &mut mpsc::Receiver<String>, window_ms: u64) {
    if let Ok(line) = tokio::time::timeout(Duration::from_millis(window_ms), rx.recv()).await {
        panic!("expected silence, got line: {line:?}");
    }
}

#[tokio::test]
async fn stdio_session_authenticates_token_and_exposes_only_requested_vault() {
    let (state, _tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "stdio-owner").await;
    let vault_a = state.vaults.create(&user_id, "alpha").await.unwrap();
    let _vault_b = state.vaults.create(&user_id, "beta").await.unwrap();

    let session = StdioSession::authenticate(state, vault_a.id.clone(), raw)
        .await
        .unwrap();
    let response = session
        .handle_jsonrpc(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "list_vaults",
                "arguments": {}
            }
        }))
        .await;

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert_eq!(
        response["result"]["structuredContent"]["vaults"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        response["result"]["structuredContent"]["vaults"][0]["id"],
        vault_a.id
    );
}

#[tokio::test]
async fn stdio_session_rejects_cross_vault_tool_arguments_without_panicking() {
    let (state, _tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "stdio-scoped").await;
    let allowed = state.vaults.create(&user_id, "allowed").await.unwrap();
    let denied = state.vaults.create(&user_id, "denied").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    git.commit_changes(
        &denied.id,
        None,
        &[FileChange::Upsert {
            path: "secret.md".into(),
            file: StoredFile::Text {
                bytes: b"secret".to_vec(),
            },
        }],
        "seed",
    )
    .await
    .unwrap();

    let session = StdioSession::authenticate(state, allowed.id, raw)
        .await
        .unwrap();
    let response = session
        .handle_jsonrpc(json!({
            "jsonrpc": "2.0",
            "id": "cross-vault",
            "method": "tools/call",
            "params": {
                "name": "list_files",
                "arguments": {
                    "vault_id": denied.id
                }
            }
        }))
        .await;

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], "cross-vault");
    assert_eq!(response["error"]["code"], -32000);
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("vault not available"));
}

#[tokio::test]
async fn stdio_session_rechecks_token_revocation_before_each_request() {
    let (state, _tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "stdio-revoke").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let token_row = state
        .tokens
        .find_by_hash(&token::hash(&raw))
        .await
        .unwrap()
        .unwrap()
        .0;
    let session = StdioSession::authenticate(state.clone(), vault.id, raw)
        .await
        .unwrap();

    state
        .tokens
        .revoke(&token_row.id, chrono::Utc::now().timestamp())
        .await
        .unwrap();

    let response = session
        .handle_jsonrpc(json!({
            "jsonrpc": "2.0",
            "id": "after-revoke",
            "method": "tools/list"
        }))
        .await;

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], "after-revoke");
    assert_eq!(response["error"]["code"], -32000);
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("invalid or revoked token"));
}

#[tokio::test]
async fn stdio_session_rejects_invalid_token() {
    let (state, _tmp) = test_state().await;
    let (user_id, _raw) = create_user_with_token(&state, "stdio-invalid").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();

    let err = StdioSession::authenticate(state, vault.id, "not-a-token".into())
        .await
        .unwrap_err();

    assert!(err.to_string().contains("invalid or revoked token"));
}

#[tokio::test]
async fn stdio_session_rate_limits_repeated_invalid_tokens() {
    let (state, _tmp) = test_state().await;
    let (user_id, _raw) = create_user_with_token(&state, "stdio-rate-limit").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let mut saw_rate_limit = false;

    for idx in 0..35 {
        let fake = format!("pks_{idx:064x}");
        let err = StdioSession::authenticate(state.clone(), vault.id.clone(), fake)
            .await
            .unwrap_err();
        if err.to_string().contains("rate_limited") {
            saw_rate_limit = true;
            break;
        }
        assert!(err.to_string().contains("invalid or revoked token"));
    }

    assert!(
        saw_rate_limit,
        "rotating invalid MCP stdio tokens were not rate limited"
    );
}

#[tokio::test]
async fn stdio_modern_tools_list_has_result_type_and_ttl() {
    let (session, _state, _user_id, _vault_id, _tmp) =
        session_with_vault("stdio-modern-tools").await;

    let lines = send(&session, modern_request(json!(1), "tools/list")).await;
    assert_eq!(lines.len(), 1);
    let resp = &lines[0];
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["resultType"], "complete");
    assert_eq!(resp["result"]["ttlMs"], 300_000);
    assert_eq!(resp["result"]["cacheScope"], "private");
    assert_eq!(
        resp["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        pkv_sync_server::mcp::protocol::SERVER_NAME
    );
}

#[tokio::test]
async fn stdio_modern_discover_lists_modern_version_only() {
    let (session, _state, _user_id, _vault_id, _tmp) =
        session_with_vault("stdio-modern-discover").await;

    let lines = send(&session, modern_request(json!(1), "server/discover")).await;
    assert_eq!(lines.len(), 1);
    let resp = &lines[0];
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["supportedVersions"], json!(["2026-07-28"]));
    assert_eq!(resp["result"]["resultType"], "complete");
    assert_eq!(resp["result"]["ttlMs"], 3_600_000);
}

#[tokio::test]
async fn stdio_modern_tools_call_carries_result_type_and_server_info() {
    let (session, _state, _user_id, _vault_id, _tmp) =
        session_with_vault("stdio-modern-call").await;

    let lines = send(
        &session,
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "list_vaults",
                "arguments": {},
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            }
        }),
    )
    .await;
    assert_eq!(lines.len(), 1);
    let resp = &lines[0];
    assert_eq!(resp["id"], 7);
    assert_eq!(resp["result"]["resultType"], "complete");
    assert_eq!(
        resp["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        pkv_sync_server::mcp::protocol::SERVER_NAME
    );
    assert_eq!(
        resp["result"]["structuredContent"]["vaults"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn stdio_initialize_echoes_legacy_version() {
    let (session, _state, _user_id, _vault_id, _tmp) =
        session_with_vault("stdio-legacy-init").await;

    let resp = session
        .handle_jsonrpc(json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "initialize",
            "params": {"protocolVersion": "2025-03-26"}
        }))
        .await;

    assert_eq!(resp["id"], 4);
    assert_eq!(resp["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(
        resp["result"]["serverInfo"]["name"],
        pkv_sync_server::mcp::protocol::SERVER_NAME
    );
}

#[tokio::test]
async fn stdio_listen_acknowledges_accepted_vault_only() {
    let (session, _state, _user_id, vault_id, _tmp) = session_with_vault("stdio-listen-ack").await;
    let scoped = format!("pkv://vault/{vault_id}");

    let lines = send(
        &session,
        listen_request(json!(3), vec![scoped.as_str(), "pkv://vault/other-vault"]),
    )
    .await;
    assert_eq!(lines.len(), 1);
    let ack = &lines[0];
    assert_eq!(ack["method"], "notifications/subscriptions/acknowledged");
    assert_eq!(
        ack["params"]["notifications"]["resourceSubscriptions"],
        json!([scoped])
    );
    assert_eq!(
        ack["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"],
        3
    );
}

#[tokio::test]
async fn stdio_listen_rejects_cross_vault_uri_without_subscribing() {
    let (session, state, _user_id, _vault_id, _tmp) =
        session_with_vault("stdio-listen-cross").await;
    let foreign_vault = "not-this-sessions-vault";

    let (tx, mut rx) = mpsc::channel::<String>(16);
    let mut tx = tx;
    session
        .pump(
            &listen_request(json!(5), vec!["pkv://vault/not-this-sessions-vault"]).to_string(),
            &mut tx,
        )
        .await;
    let ack: Value = serde_json::from_str(&rx.recv().await.unwrap()).unwrap();
    assert!(
        ack["params"]["notifications"]["resourceSubscriptions"].is_null(),
        "foreign vault must not be echoed: {ack}"
    );

    state.events.publish(
        foreign_vault,
        VaultEvent {
            commit: "c0".into(),
            parent: None,
            source_device_id: "dev".into(),
            at: 1,
            kind: EventKind::Commit,
            changes: vec![],
        },
    );
    session.drain_listens(&mut tx).await;
    assert_no_line(&mut rx, 150).await;
}

#[tokio::test]
async fn stdio_listen_streams_resource_updates_until_cancelled() {
    let (session, state, user_id, vault_id, _tmp) = session_with_vault("stdio-listen-stream").await;
    let scoped = format!("pkv://vault/{vault_id}");

    let (tx, mut rx) = mpsc::channel::<String>(16);
    let mut tx = tx;
    session
        .pump(
            &listen_request(json!(9), vec![scoped.as_str()]).to_string(),
            &mut tx,
        )
        .await;
    let ack: Value = serde_json::from_str(&rx.recv().await.unwrap()).unwrap();
    assert_eq!(ack["method"], "notifications/subscriptions/acknowledged");
    assert_eq!(
        ack["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"],
        9
    );

    commit_and_broadcast(&state, &user_id, &vault_id, "first").await;
    session.drain_listens(&mut tx).await;
    let update: Value = serde_json::from_str(
        &rx.recv()
            .await
            .expect("resource update must arrive after commit"),
    )
    .unwrap();
    assert_eq!(update["method"], "notifications/resources/updated");
    assert_eq!(update["params"]["uri"], scoped);
    assert_eq!(
        update["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"],
        9
    );

    session
        .pump(
            &json!({
                "jsonrpc": "2.0",
                "method": "notifications/cancelled",
                "params": {"requestId": 9}
            })
            .to_string(),
            &mut tx,
        )
        .await;

    commit_and_broadcast(&state, &user_id, &vault_id, "second").await;
    session.drain_listens(&mut tx).await;
    assert_no_line(&mut rx, 150).await;
}

#[tokio::test]
async fn stdio_listen_lagged_broadcast_emits_resources_dropped() {
    let (session, state, _user_id, vault_id, _tmp) = session_with_vault("stdio-listen-lag").await;
    let scoped = format!("pkv://vault/{vault_id}");

    let (tx, mut rx) = mpsc::channel::<String>(1024);
    let mut tx = tx;
    session
        .pump(
            &listen_request(json!(11), vec![scoped.as_str()]).to_string(),
            &mut tx,
        )
        .await;
    let ack: Value = serde_json::from_str(&rx.recv().await.unwrap()).unwrap();
    assert_eq!(ack["method"], "notifications/subscriptions/acknowledged");

    // stdio has no replay path: the drain loop polls on a 50ms tick, so
    // publishing faster than it drains overflows the 64-slot broadcast ring
    // and the next try_recv must report Lagged.
    for idx in 0..70 {
        state.events.publish(
            &vault_id,
            VaultEvent {
                commit: format!("lag-{idx}"),
                parent: None,
                source_device_id: "dev".into(),
                at: 1,
                kind: EventKind::Commit,
                changes: vec![],
            },
        );
    }
    session.drain_listens(&mut tx).await;

    let mut saw_dropped = false;
    while let Ok(Some(line)) = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
        let notification: Value = serde_json::from_str(&line).unwrap();
        if notification["method"] == "notifications/resources/dropped" {
            assert_eq!(notification["params"]["uri"], scoped);
            assert_eq!(
                notification["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"],
                11
            );
            saw_dropped = true;
        }
    }
    assert!(
        saw_dropped,
        "drain after a broadcast overflow must emit a resources/dropped notification"
    );
}

#[tokio::test]
async fn stdio_unknown_notification_is_silence() {
    let (session, _state, _user_id, _vault_id, _tmp) =
        session_with_vault("stdio-notify-silence").await;

    let (tx, mut rx) = mpsc::channel::<String>(16);
    let mut tx = tx;
    session
        .pump(
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
            &mut tx,
        )
        .await;
    assert_no_line(&mut rx, 150).await;
}

#[tokio::test]
async fn stdio_parse_error_writes_minus_32700() {
    let (session, _state, _user_id, _vault_id, _tmp) =
        session_with_vault("stdio-parse-error").await;

    let lines = send_line(&session, "{not json").await;
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["error"]["code"], -32700);
}

#[tokio::test]
async fn stdio_run_returns_ok_after_stdin_eof_with_active_listen() {
    // SPEC-3: the real `run` loop (via `run_on` over a duplex pair) must
    // terminate cleanly on stdin EOF even with a registered listen — the
    // cleanup path the pump/drain tests never touch.
    use tokio::io::{duplex, AsyncBufReadExt, AsyncWriteExt};

    let tmp = tempfile::tempdir().unwrap();
    let p = pkv_sync_server::db::pool::connect(&tmp.path().join("run.db"))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    let state = AppState::new(p, tmp.path().to_path_buf(), "test".into(), true)
        .await
        .unwrap();
    let (user_id, raw) = create_user_with_token(&state, "stdio-run-eof").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();

    let (client, server) = duplex(1024);
    let (server_read, server_write) = tokio::io::split(server);
    let run_handle = tokio::spawn(pkv_sync_server::mcp::transport_stdio::run_on(
        state.clone(),
        vault.id.clone(),
        raw,
        server_read,
        server_write,
    ));

    // Register a listen, read its ack, and only then close our write half:
    // stdin EOF with an active listen must end the loop with Ok. The ack is
    // read first because a duplex write half that is shut down can no longer
    // be used, and the loop must still observe a *pending* stdin read before
    // EOF — closing before the server parsed the line would race.
    let (client_read, mut client_write) = tokio::io::split(client);
    let listen = listen_request(json!(21), vec![&format!("pkv://vault/{}", vault.id)]);
    client_write
        .write_all(format!("{listen}\n").as_bytes())
        .await
        .unwrap();
    client_write.flush().await.unwrap();

    let mut stdout = String::new();
    let mut reader = tokio::io::BufReader::new(client_read);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let mut line = String::new();
        let read = tokio::select! {
            read = reader.read_line(&mut line) => read,
            _ = tokio::time::sleep_until(deadline) => break,
        };
        match read {
            Ok(0) => break,
            Ok(_) => {
                stdout.push_str(&line);
                if stdout.contains("notifications/subscriptions/acknowledged") {
                    break;
                }
            }
            Err(err) => panic!("stdout read error: {err}"),
        }
    }
    assert!(
        stdout.contains("notifications/subscriptions/acknowledged"),
        "listen must be acked before EOF, got: {stdout}"
    );

    client_write.shutdown().await.unwrap();
    drop(client_write);

    let result = tokio::time::timeout(Duration::from_secs(5), run_handle)
        .await
        .expect("run_on must finish within 5s of stdin EOF")
        .unwrap();
    assert!(
        result.is_ok(),
        "stdin EOF must end the loop cleanly: {result:?}"
    );
}
