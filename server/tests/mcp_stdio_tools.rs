mod common;

use common::{create_user_with_token, test_state};
use pkv_sync_server::auth::token;
use pkv_sync_server::db::repos::{TokenRepo, VaultRepo};
use pkv_sync_server::mcp::transport_stdio::StdioSession;
use pkv_sync_server::service::events::{EventKind, VaultEvent};
use pkv_sync_server::service::AppState;
use pkv_sync_server::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::mpsc;

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

async fn commit_and_broadcast(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    tag: &str,
) -> String {
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
            commit: commit.clone(),
            parent: None,
            source_device_id: user.device_id,
            at: 1,
            kind: EventKind::Commit,
            changes: vec![],
        },
    );
    commit
}

async fn assert_no_line(rx: &mut mpsc::Receiver<String>, window_ms: u64) {
    if let Ok(line) = tokio::time::timeout(Duration::from_millis(window_ms), rx.recv()).await {
        panic!("expected silence, got line: {line:?}");
    }
}

/// Next line as JSON, with a bounded wait so a regression that emits nothing
/// fails the test instead of hanging the harness.
async fn expect_line(rx: &mut mpsc::Receiver<String>, what: &str) -> Value {
    match tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
        Ok(Some(line)) => serde_json::from_str(&line)
            .unwrap_or_else(|err| panic!("{what}: unparsable line {line:?}: {err}")),
        Ok(None) => panic!("{what}: channel closed with no line"),
        Err(_) => panic!("{what}: no line within 2s"),
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
async fn stdio_modern_session_rechecks_token_revocation_before_each_request() {
    let (state, _tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "stdio-modern-revoke").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let token_row = state
        .tokens
        .find_by_hash(&token::hash(&raw))
        .await
        .unwrap()
        .unwrap()
        .0;
    let session = StdioSession::authenticate(state.clone(), vault.id.clone(), raw)
        .await
        .unwrap();

    state
        .tokens
        .revoke(&token_row.id, chrono::Utc::now().timestamp())
        .await
        .unwrap();

    let lines = send(
        &session,
        modern_request(json!("after-revoke"), "tools/list"),
    )
    .await;
    assert_eq!(lines.len(), 1);
    let resp = &lines[0];
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], "after-revoke");
    assert_eq!(resp["error"]["code"], -32000);
    assert!(resp["error"]["message"]
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
async fn stdio_run_on_survives_drain_larger_than_outbound_channel() {
    // CR-1: the production outbound channel holds 64 lines, but a single
    // `drain_listens` call can produce 65 (a Lagged marker plus every event
    // still in the 64-slot broadcast ring). When the only consumer of the
    // channel is the same select loop that awaits `drain_listens`, the 65th
    // `tx.send().await` waits for a slot that nothing can free — the session
    // deadlocks and never answers stdin again. Drive `run_on` itself (not a
    // 1024-slot test channel) over a duplex pair and require a subsequent
    // request/response roundtrip to complete within a timeout.
    //
    // R2-1: the burst must not be published before the listen is actually
    // registered. `VaultEventBus::publish` drops events on the floor while
    // `receiver_count() == 0`, and on a current-thread runtime the duplex
    // `write_all` above completes without yielding, so publishing straight
    // after writing the listen line loses all 120 events and the overflow
    // this test exists to trigger never happens — the test then passes on
    // the pre-CR-1 implementation too. `StdioSession::listen` registers the
    // broadcast receiver *before* it queues the ack, so reading the ack back
    // proves a receiver exists; only then is the burst published. The
    // dropped marker is asserted as well, so a future regression that stops
    // the overflow from being produced fails here instead of passing
    // vacuously.
    use tokio::io::{duplex, AsyncBufReadExt, AsyncWriteExt};

    let (session_state, _tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&session_state, "stdio-drain-cap").await;
    let vault = session_state.vaults.create(&user_id, "main").await.unwrap();
    let vault_id = vault.id.clone();

    let (client, server) = duplex(64 * 1024);
    let (server_read, server_write) = tokio::io::split(server);
    let run_handle = tokio::spawn(pkv_sync_server::mcp::transport_stdio::run_on(
        session_state.clone(),
        vault_id.clone(),
        raw,
        server_read,
        server_write,
    ));

    let (client_read, mut client_write) = tokio::io::split(client);
    // Drain stdout from the very start: the ack has to be observed before the
    // burst is published, and reading is also what frees duplex backpressure,
    // so it must keep running for the roundtrip below to be able to finish.
    let mut reader = tokio::io::BufReader::new(client_read);
    let (line_tx, mut line_rx) = mpsc::channel::<String>(256);
    let read_handle = tokio::spawn(async move {
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    if line_tx.send(line).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let listen = listen_request(json!(13), vec![&format!("pkv://vault/{vault_id}")]);
    client_write
        .write_all(format!("{listen}\n").as_bytes())
        .await
        .unwrap();

    let mut stdout = String::new();
    let ack_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut saw_ack = false;
    while tokio::time::Instant::now() < ack_deadline {
        match tokio::time::timeout_at(ack_deadline, line_rx.recv()).await {
            Ok(Some(line)) => {
                stdout.push_str(&line);
                if line.contains("notifications/subscriptions/acknowledged") {
                    saw_ack = true;
                    break;
                }
            }
            Ok(None) => break, // reader hit EOF before the ack
            Err(_) => break,   // deadline
        }
    }
    if !saw_ack {
        run_handle.abort();
        let _ = read_handle.await;
        panic!("listen was never acknowledged; stdout so far: {stdout}");
    }

    // Publish 120 events (audit's lagged-test volume) now that the receiver is
    // registered — the publishes are synchronous, so no drain tick can
    // interleave and shrink the pending backlog below the ring capacity. The
    // next 50ms drain tick must collect a dropped marker plus the 64 retained
    // ring events: one line more than the outbound channel holds.
    for idx in 0..120 {
        session_state.events.publish(
            &vault_id,
            VaultEvent {
                commit: format!("cap-{idx}"),
                parent: None,
                source_device_id: "dev".into(),
                at: 1,
                kind: EventKind::Commit,
                changes: vec![],
            },
        );
    }

    // The oversized drain must actually reach the client: the dropped marker
    // (then the 64 retained ring events). On the pre-CR-1 single-loop
    // implementation the batch is produced but never written — the 65th
    // `tx.send().await` pins the loop inside the drain arm, and the same loop
    // is the only writer, so the client sees nothing more. Waiting for the
    // marker before probing keeps the failure deterministic instead of racing
    // the probe past the blocked drain.
    let mut saw_dropped = false;
    let drain_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < drain_deadline {
        match tokio::time::timeout_at(drain_deadline, line_rx.recv()).await {
            Ok(Some(line)) => {
                stdout.push_str(&line);
                if line.contains("notifications/resources/dropped") {
                    saw_dropped = true;
                    break;
                }
            }
            Ok(None) => break, // reader hit EOF before the deadline
            Err(_) => break,   // deadline
        }
    }
    if !saw_dropped {
        run_handle.abort();
        let _ = read_handle.await;
        panic!(
            "an oversized drain must emit notifications/resources/dropped to the client; \
             stdout so far: {stdout}"
        );
    }

    // The follow-up request is the deadlock probe: a loop pinned inside the
    // drain never parses stdin again, so this response never arrives.
    let probe = json!({"jsonrpc": "2.0", "id": 99, "method": "tools/list", "params": {}});
    client_write
        .write_all(format!("{probe}\n").as_bytes())
        .await
        .unwrap();

    let mut saw_probe = false;
    let probe_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < probe_deadline {
        match tokio::time::timeout_at(probe_deadline, line_rx.recv()).await {
            Ok(Some(line)) => {
                if line.contains("\"id\":99") {
                    saw_probe = true;
                    break;
                }
                stdout.push_str(&line);
            }
            Ok(None) => break, // reader hit EOF before the deadline
            Err(_) => break,   // deadline
        }
    }
    if !saw_probe {
        run_handle.abort();
        let _ = read_handle.await;
        panic!(
            "run_on never answered the request sent after an oversized drain; stdout so far: {stdout}"
        );
    }

    client_write.shutdown().await.unwrap();
    drop(client_write);
    let result = tokio::time::timeout(Duration::from_secs(5), run_handle)
        .await
        .expect("run_on must finish within 5s of stdin EOF")
        .unwrap();
    assert!(result.is_ok(), "run_on must exit cleanly: {result:?}");
}

#[tokio::test]
async fn stdio_listen_notifications_carry_jsonrpc_2_0() {
    // M-1: strict MCP clients validate every inbound message as JSON-RPC 2.0
    // and drop ones without the version field, so the ack and every push
    // line must serialize with "jsonrpc":"2.0".
    let (session, state, user_id, vault_id, _tmp) = session_with_vault("stdio-jsonrpc-tag").await;
    let scoped = format!("pkv://vault/{vault_id}");

    let (tx, mut rx) = mpsc::channel::<String>(16);
    let mut tx = tx;
    session
        .pump(
            &listen_request(json!(15), vec![scoped.as_str()]).to_string(),
            &mut tx,
        )
        .await;
    let ack_line = rx.recv().await.unwrap();
    assert!(
        ack_line.starts_with("{\"jsonrpc\":\"2.0\""),
        "ack must lead with the JSON-RPC version field: {ack_line}"
    );

    commit_and_broadcast(&state, &user_id, &vault_id, "tagged").await;
    session.drain_listens(&mut tx).await;
    let update_line = rx.recv().await.unwrap();
    let update: Value = serde_json::from_str(&update_line).unwrap();
    assert_eq!(update["jsonrpc"], "2.0", "line: {update_line}");
    assert_eq!(update["method"], "notifications/resources/updated");
}

#[tokio::test]
async fn stdio_drain_listens_stops_pushing_after_token_revocation() {
    // M-2: stdio rechecks the token only on the request path, so a revoked
    // session keeps pushing listen updates until EOF. The drain must
    // revalidate (through the 5s validity cache), drop the listens, and push
    // nothing more. R2-3: dropping an active subscription is terminal and
    // used to be silent — the client kept waiting for events that could
    // never come — so the drain first emits one `resources/dropped` per live
    // listen (with its subscriptionId), then goes quiet.
    let (state, _tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "stdio-drain-revoke").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let vault_id = vault.id.clone();
    let session = StdioSession::authenticate(state.clone(), vault.id, raw.clone())
        .await
        .unwrap();
    let scoped = format!("pkv://vault/{vault_id}");

    let (tx, mut rx) = mpsc::channel::<String>(16);
    let mut tx = tx;
    session
        .pump(
            &listen_request(json!(17), vec![scoped.as_str()]).to_string(),
            &mut tx,
        )
        .await;
    let _ack: Value = serde_json::from_str(&rx.recv().await.unwrap()).unwrap();

    commit_and_broadcast(&state, &user_id, &vault_id, "before-revoke").await;
    session.drain_listens(&mut tx).await;
    let before: Value = serde_json::from_str(&rx.recv().await.unwrap()).unwrap();
    assert_eq!(before["method"], "notifications/resources/updated");

    let (token_row, _) = state
        .tokens
        .find_by_hash(&token::hash(&raw))
        .await
        .unwrap()
        .unwrap();
    state
        .tokens
        .revoke(&token_row.id, chrono::Utc::now().timestamp())
        .await
        .unwrap();

    commit_and_broadcast(&state, &user_id, &vault_id, "after-revoke").await;
    session.drain_listens(&mut tx).await;

    let dropped = expect_line(&mut rx, "terminal notice for the revoked listen").await;
    assert_eq!(dropped["method"], "notifications/resources/dropped");
    assert_eq!(dropped["params"]["uri"], scoped);
    assert_eq!(
        dropped["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"],
        17
    );

    // The cleared registry must stay cleared: a later commit reaches no
    // listener even though the session keeps draining.
    commit_and_broadcast(&state, &user_id, &vault_id, "after-revoke-2").await;
    session.drain_listens(&mut tx).await;
    assert_no_line(&mut rx, 150).await;
}

#[tokio::test]
async fn stdio_drain_listens_survives_transient_token_lookup_errors() {
    // R2-3: a transient database error during the drain's token recheck used
    // to be indistinguishable from a revocation (`let Ok(Some(..)) = ... else
    // cache false`). One hiccup therefore cleared every listen and cached the
    // "revoked" answer for 5s: the session went permanently silent, with no
    // error, no notification and no closed channel — the client could not
    // tell it from "no events happened". The drain must skip the tick, keep
    // the subscriptions, and push again once the database answers.
    let (state, _tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "stdio-drain-transient").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let vault_id = vault.id.clone();
    let session = StdioSession::authenticate(state.clone(), vault.id, raw)
        .await
        .unwrap();
    let scoped = format!("pkv://vault/{vault_id}");

    let (tx, mut rx) = mpsc::channel::<String>(16);
    let mut tx = tx;
    session
        .pump(
            &listen_request(json!(23), vec![scoped.as_str()]).to_string(),
            &mut tx,
        )
        .await;
    let _ack: Value = serde_json::from_str(&rx.recv().await.unwrap()).unwrap();

    let commit = commit_and_broadcast(&state, &user_id, &vault_id, "during-outage").await;

    // The validity cache still holds the `Valid` answer from the listen
    // registration moments ago (std Instant, 5s TTL), so the drain below
    // must be forced to actually query: revoking ANOTHER user's token bumps
    // the global token epoch, invalidating the cache without touching this
    // session's row.
    let (other_id, _other_raw) = create_user_with_token(&state, "stdio-drain-other").await;
    state
        .tokens
        .revoke_all_for_user(&other_id, chrono::Utc::now().timestamp(), None)
        .await
        .unwrap();

    // Genuine repository errors, not "row missing": the lookup table is
    // absent for the duration of the outage (a closed pool would work too,
    // but it cannot be reopened to prove recovery).
    sqlx::query("ALTER TABLE tokens RENAME TO tokens_transient_outage")
        .execute(&state.pool)
        .await
        .unwrap();
    session.drain_listens(&mut tx).await;
    assert_no_line(&mut rx, 200).await;

    // The outage ends before the 5s validity-cache window does: an
    // implementation that cached the failure would still refuse to serve
    // this tick.
    sqlx::query("ALTER TABLE tokens_transient_outage RENAME TO tokens")
        .execute(&state.pool)
        .await
        .unwrap();
    session.drain_listens(&mut tx).await;

    let update = expect_line(&mut rx, "push after the transient outage").await;
    assert_eq!(update["method"], "notifications/resources/updated");
    assert_eq!(update["params"]["commit"], commit);

    // And it is a subscription, not a one-off: later commits keep flowing.
    let next = commit_and_broadcast(&state, &user_id, &vault_id, "after-outage").await;
    session.drain_listens(&mut tx).await;
    let update = expect_line(&mut rx, "push for the next commit").await;
    assert_eq!(update["params"]["commit"], next);
}

#[tokio::test]
async fn stdio_drain_listens_bounds_a_transient_token_outage() {
    // R3-8: the R2-3 tolerance is bounded. While the database is unreachable
    // a revoked session is indistinguishable from a valid one, so an
    // uncapped retry loop kept every subscription alive for as long as the
    // error lasted — an open-ended fail-open for exactly the session that
    // should have been cut off. The two ticks below prove R2-3's guarantee
    // survives (nothing is dropped inside the tolerance); the third proves
    // the cap closes it.
    let (state, _tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "stdio-drain-outage-cap").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let vault_id = vault.id.clone();
    let session = StdioSession::authenticate(state.clone(), vault.id, raw.clone())
        .await
        .unwrap();
    session.set_transient_token_max_ticks_for_tests(2);
    let scoped = format!("pkv://vault/{vault_id}");

    let (tx, mut rx) = mpsc::channel::<String>(16);
    let mut tx = tx;
    session
        .pump(
            &listen_request(json!(31), vec![scoped.as_str()]).to_string(),
            &mut tx,
        )
        .await;
    let _ack: Value = serde_json::from_str(&rx.recv().await.unwrap()).unwrap();

    // The session really is revoked...
    let (token_row, _) = state
        .tokens
        .find_by_hash(&token::hash(&raw))
        .await
        .unwrap()
        .unwrap();
    state
        .tokens
        .revoke(&token_row.id, chrono::Utc::now().timestamp())
        .await
        .unwrap();
    // ...but the drain cannot see it: the lookup table is gone. Revoking
    // another user's token bumps the global token epoch, so the 5s validity
    // cache cannot answer from its stale `Valid` entry.
    let (other_id, _other_raw) = create_user_with_token(&state, "stdio-drain-cap-other").await;
    state
        .tokens
        .revoke_all_for_user(&other_id, chrono::Utc::now().timestamp(), None)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE tokens RENAME TO tokens_outage_cap")
        .execute(&state.pool)
        .await
        .unwrap();

    // Inside the tolerance nothing is dropped and no listen is cleared.
    session.drain_listens(&mut tx).await;
    session.drain_listens(&mut tx).await;
    assert_no_line(&mut rx, 200).await;

    // The next tick exceeds it: the session is treated as no longer valid,
    // so the active listen gets its dropped marker and the registry clears
    // instead of holding the subscription open for the whole outage.
    session.drain_listens(&mut tx).await;
    let dropped = expect_line(&mut rx, "terminal notice once the tolerance is exceeded").await;
    assert_eq!(dropped["method"], "notifications/resources/dropped");
    assert_eq!(dropped["params"]["uri"], scoped);
    assert_eq!(
        dropped["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"],
        31
    );

    // And it stays cleared: a commit after the cap reaches no listener.
    commit_and_broadcast(&state, &user_id, &vault_id, "after-cap").await;
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
async fn stdio_batch_array_line_answers_invalid_request() {
    // L-4: a JSON-RPC batch (array) line used to parse as Value::Array,
    // fall into the no-id branch, and produce silence — a client that sent
    // a batch waited forever. Answer with -32600 instead.
    let (session, _state, _user_id, _vault_id, _tmp) =
        session_with_vault("stdio-batch-array").await;

    let lines = send_line(&session, r#"[{"jsonrpc":"2.0"}]"#).await;
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["error"]["code"], -32600);
    assert!(
        lines[0]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("batch"),
        "error must name batch support, got: {}",
        lines[0]
    );
}

#[tokio::test]
async fn stdio_batch_array_with_ids_answers_invalid_request() {
    // L-4 variant: batches that DO carry member ids still cannot be
    // answered member-wise; the whole line gets the single -32600 error.
    let (session, _state, _user_id, _vault_id, _tmp) = session_with_vault("stdio-batch-ids").await;

    let lines = send_line(
        &session,
        r#"[{"jsonrpc":"2.0","id":1,"method":"tools/list"},{"jsonrpc":"2.0","id":2,"method":"tools/list"}]"#,
    )
    .await;
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["error"]["code"], -32600);
}

#[tokio::test]
async fn stdio_tools_call_array_arguments_is_invalid_params() {
    // L-5: non-object `arguments` used to fail the per-tool from_value and
    // surface as -32000 "internal server error" (plus an ERROR log) —
    // misreporting a client mistake as server breakage. Both eras must
    // answer -32602 invalid params instead.
    let (session, _state, _user_id, _vault_id, _tmp) = session_with_vault("stdio-args-array").await;

    let legacy = send(
        &session,
        json!({
            "jsonrpc": "2.0",
            "id": "legacy-array-args",
            "method": "tools/call",
            "params": {"name": "list_files", "arguments": [1, 2]}
        }),
    )
    .await;
    assert_eq!(legacy.len(), 1);
    assert_eq!(legacy[0]["id"], "legacy-array-args");
    assert_eq!(legacy[0]["error"]["code"], -32602);
    assert_eq!(
        legacy[0]["error"]["message"], "invalid params: arguments must be an object",
        "got: {}",
        legacy[0]
    );

    let modern = send(
        &session,
        json!({
            "jsonrpc": "2.0",
            "id": "modern-array-args",
            "method": "tools/call",
            "params": {
                "name": "list_files",
                "arguments": [1, 2],
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            }
        }),
    )
    .await;
    assert_eq!(modern.len(), 1);
    assert_eq!(modern[0]["id"], "modern-array-args");
    assert_eq!(modern[0]["error"]["code"], -32602);
    assert_eq!(
        modern[0]["error"]["message"], "invalid params: arguments must be an object",
        "got: {}",
        modern[0]
    );
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

#[tokio::test]
async fn stdio_eof_drains_pending_response_before_exit() {
    // N-5: a client that sends a request and immediately closes its write
    // half (without reading the response first) must still get the response.
    // `tokio::select!` is randomly fair, so when stdin EOF and a ready
    // response line are both pending the EOF arm can win and a bare `break`
    // would drop the response still queued in the channel.
    use tokio::io::{duplex, AsyncBufReadExt, AsyncWriteExt};

    let tmp = tempfile::tempdir().unwrap();
    let p = pkv_sync_server::db::pool::connect(&tmp.path().join("run.db"))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    let state = AppState::new(p, tmp.path().to_path_buf(), "test".into(), true)
        .await
        .unwrap();
    let (user_id, raw) = create_user_with_token(&state, "stdio-eof-drain").await;
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

    // Send one tools/list request, then shut down the write half WITHOUT
    // reading the response first — this is exactly the dropping scenario.
    // The line lands in the duplex buffer before the shutdown, so the server
    // always parses and answers it before observing EOF; the only race left
    // is which select arm the loop settles on once both are ready.
    let (client_read, mut client_write) = tokio::io::split(client);
    let request = json!({"jsonrpc": "2.0", "id": 7, "method": "tools/list", "params": {}});
    client_write
        .write_all(format!("{request}\n").as_bytes())
        .await
        .unwrap();
    client_write.flush().await.unwrap();
    client_write.shutdown().await.unwrap();
    drop(client_write);

    // Drain stdout concurrently: the tools/list response is larger than the
    // duplex buffer, so the server's write would block if nobody read it.
    let mut reader = tokio::io::BufReader::new(client_read);
    let reader_handle = tokio::spawn(async move {
        let mut stdout = String::new();
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => stdout.push_str(&line),
                Err(err) => panic!("stdout read error: {err}"),
            }
        }
        stdout
    });

    let result = tokio::time::timeout(Duration::from_secs(5), run_handle)
        .await
        .expect("run_on must finish within 5s of stdin EOF")
        .unwrap();
    assert!(
        result.is_ok(),
        "stdin EOF must end the loop cleanly: {result:?}"
    );

    let stdout = tokio::time::timeout(Duration::from_secs(10), reader_handle)
        .await
        .expect("stdout must reach EOF within 10s of run_on exiting")
        .unwrap();

    let response: Value = serde_json::from_str(stdout.trim()).unwrap_or(Value::Null);
    assert_eq!(
        response["id"],
        json!(7),
        "the response queued before stdin EOF must be flushed on exit, got: {stdout}"
    );
}

/// Writer half whose every poll_* returns the same error, so the first line
/// the dedicated writer task attempts is guaranteed to fail.
struct FailingWriter;

impl tokio::io::AsyncWrite for FailingWriter {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &[u8],
    ) -> std::task::Poll<std::result::Result<usize, std::io::Error>> {
        std::task::Poll::Ready(Err(std::io::Error::other("writer is broken")))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        std::task::Poll::Ready(Err(std::io::Error::other("writer is broken")))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        std::task::Poll::Ready(Err(std::io::Error::other("writer is broken")))
    }
}

#[tokio::test]
async fn stdio_run_on_propagates_writer_errors() {
    // P0a follow-up: the pre-deadlock-fix loop did `write_all(...).await?`,
    // so a broken pipe made `run_on` return Err. The dedicated writer task
    // swallowed those errors (`break` and exit Ok), so a session whose
    // stdout died returned Ok(()) even though every write failed. The
    // writer must report its final outcome and `run_on` must map it into
    // its Result.
    use tokio::io::AsyncWriteExt;

    let tmp = tempfile::tempdir().unwrap();
    let p = pkv_sync_server::db::pool::connect(&tmp.path().join("run.db"))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    let state = AppState::new(p, tmp.path().to_path_buf(), "test".into(), true)
        .await
        .unwrap();
    let (user_id, raw) = create_user_with_token(&state, "stdio-writer-err").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();

    // A duplex read half supplies stdin: one request (so the writer task has
    // a line to attempt), then EOF.
    let (mut client_write, server_read) = tokio::io::duplex(64);
    let run_handle = tokio::spawn(pkv_sync_server::mcp::transport_stdio::run_on(
        state.clone(),
        vault.id.clone(),
        raw,
        server_read,
        FailingWriter,
    ));

    let request = json!({"jsonrpc": "2.0", "id": 31, "method": "tools/list", "params": {}});
    client_write
        .write_all(format!("{request}\n").as_bytes())
        .await
        .unwrap();
    client_write.flush().await.unwrap();
    drop(client_write);

    let result = tokio::time::timeout(Duration::from_secs(5), run_handle)
        .await
        .expect("run_on must finish within 5s of a failing writer")
        .unwrap();
    let err = result
        .expect_err("run_on must return Err when the writer half is broken")
        .to_string();
    assert!(
        err.contains("broken pipe") || err.contains("writer"),
        "error should name the writer failure, got: {err}"
    );
}
