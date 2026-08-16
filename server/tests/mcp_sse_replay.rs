use futures_util::StreamExt;
use ipnet::IpNet;
use pkv_sync_server::auth::{password, token, LoginRateLimiter};
use pkv_sync_server::config::{
    Config, LoggingConfig, McpConfig, NetworkConfig, ServerConfig, StorageConfig,
};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
use pkv_sync_server::server;
use pkv_sync_server::service::sync::{push, PushChange, PushReq};
use pkv_sync_server::service::{vault, AppState};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const DEPLOYMENT_KEY: &str = "k_mcp_sse_replay";

struct TestServer {
    addr: SocketAddr,
    _tmp: tempfile::TempDir,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

async fn start_test_server() -> (
    TestServer,
    AppState,
    pkv_sync_server::auth::AuthenticatedUser,
    String,
) {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let cfg = Arc::new(Config {
        server: ServerConfig {
            bind_addr: addr,
            deployment_key: DEPLOYMENT_KEY.into(),
            public_host: None,
        },
        storage: StorageConfig {
            data_dir: data_dir.clone(),
            db_path: data_dir.join("metadata.db"),
        },
        network: NetworkConfig {
            trusted_proxies: vec!["127.0.0.1/32".parse::<IpNet>().unwrap()],
        },
        logging: LoggingConfig::default(),
        update_check: pkv_sync_server::config::UpdateCheckConfig {
            enabled: false,
            ..Default::default()
        },
        mcp: McpConfig {
            embed_in_serve: true,
        },
    });

    let db = pool::connect(&cfg.storage.db_path).await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    let state = AppState::new(db, data_dir.clone(), "t".into(), true)
        .await
        .unwrap();

    let user = state
        .users
        .create(NewUser {
            username: "mcp-user".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    let raw = token::generate();
    let token_row = state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&raw),
            device_id: "mcp-sse-device",
            device_name: "MCP SSE",
        })
        .await
        .unwrap();
    let auth = pkv_sync_server::auth::AuthenticatedUser {
        user_id: user.id,
        username: user.username,
        is_admin: false,
        token_id: token_row.id,
        device_id: token_row.device_id,
    };

    let state_clone = state.clone();
    let limiter = LoginRateLimiter::new(10, Duration::from_secs(900), Duration::from_secs(900));
    let cfg2 = cfg.clone();
    let handle = tokio::spawn(async move {
        let _ = server::run_with_listener_and_state(cfg2, listener, state_clone, limiter).await;
    });

    let ts = TestServer {
        addr,
        _tmp: tmp,
        handle: Some(handle),
    };

    for _ in 0..50 {
        let ready = client()
            .get(format!("http://{}/api/health", ts.addr))
            .header("user-agent", "PKVSync-Plugin/0.1.0")
            .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
            .send()
            .await
            .map(|resp| resp.status().as_u16() == 200)
            .unwrap_or(false);
        if ready {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    (ts, state, auth, raw)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

fn mcp_sse_request(
    addr: SocketAddr,
    raw: &str,
    last_event_id: Option<&str>,
) -> reqwest::RequestBuilder {
    let mut builder = client()
        .get(format!("http://{}/mcp", addr))
        .header("Accept", "text/event-stream")
        .header("Authorization", format!("Bearer {raw}"))
        .header("X-PKVSync-Deployment-Key", DEPLOYMENT_KEY);
    if let Some(id) = last_event_id {
        builder = builder.header("Last-Event-ID", id);
    }
    builder
}

async fn read_until(resp: reqwest::Response, needles: &[String]) -> String {
    let mut stream = resp.bytes_stream();
    let mut body = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
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
            Some(Err(_)) => break,
            None => break,
        }
    }
    body
}

async fn wait_for_file(path: &PathBuf) {
    for _ in 0..200 {
        if path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("marker file never appeared: {}", path.display());
}

struct EnvVarGuard {
    key: &'static str,
    value: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let prev = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, value: prev }
    }

    fn set_path(key: &'static str, path: &std::path::Path) -> Self {
        Self::set(key, &path.to_string_lossy())
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.value {
            Some(v) => std::env::set_var(self.key, v),
            None => std::env::remove_var(self.key),
        }
    }
}

#[tokio::test]
async fn mcp_sse_reconnect_does_not_miss_commit_landing_during_replay() {
    let (ts, state, auth, raw) = start_test_server().await;
    let vid = vault::create_vault(&state, &auth.user_id, "main")
        .await
        .unwrap();

    let mut parent: Option<String> = None;
    for idx in 0..3 {
        let pushed = push(
            &state,
            &auth,
            &vid.id,
            parent.as_deref(),
            None,
            PushReq {
                device_name: Some(format!("replay-backlog-{idx}")),
                changes: vec![PushChange::Text {
                    path: format!("backlog-{idx}.md"),
                    content: idx.to_string(),
                }],
            },
        )
        .await
        .unwrap();
        parent = Some(pushed.new_commit);
    }
    let first_commit = parent.clone().unwrap();

    let marker = ts._tmp.path().join("mcp-after-replay.marker");
    let _seam_env = EnvVarGuard::set("PKVSYNC_ENABLE_TEST_SEAMS", "1");
    let _marker_env = EnvVarGuard::set_path("PKVSYNC_TEST_SSE_PAUSE_AFTER_REPLAY_MARKER", &marker);
    let _pause_env = EnvVarGuard::set("PKVSYNC_TEST_SSE_PAUSE_AFTER_REPLAY_MS", "250");

    let reconnect = tokio::spawn({
        let addr = ts.addr;
        let raw = raw.clone();
        let first_commit = first_commit.clone();
        async move {
            mcp_sse_request(addr, &raw, Some(&first_commit))
                .send()
                .await
                .unwrap()
        }
    });

    wait_for_file(&marker).await;
    let during_reconnect = push(
        &state,
        &auth,
        &vid.id,
        parent.as_deref(),
        None,
        PushReq {
            device_name: Some("during-reconnect".into()),
            changes: vec![PushChange::Text {
                path: "during-reconnect.md".into(),
                content: "must not be missed".into(),
            }],
        },
    )
    .await
    .unwrap();

    let sse_resp = reconnect.await.unwrap();
    let body = read_until(sse_resp, &[format!("id: {}", during_reconnect.new_commit)]).await;

    assert!(
        body.contains(&format!("id: {}", during_reconnect.new_commit)),
        "expected reconnect MCP stream to include the commit that landed during replay, got: {body}"
    );
}

#[tokio::test]
async fn mcp_sse_reconnect_dedupes_commit_seen_by_replay_and_live_stream() {
    let (ts, state, auth, raw) = start_test_server().await;
    let vid = vault::create_vault(&state, &auth.user_id, "main")
        .await
        .unwrap();

    let first = push(
        &state,
        &auth,
        &vid.id,
        None,
        None,
        PushReq {
            device_name: Some("first".into()),
            changes: vec![PushChange::Text {
                path: "first.md".into(),
                content: "1".into(),
            }],
        },
    )
    .await
    .unwrap();
    let first_commit = first.new_commit.clone();

    let marker = ts._tmp.path().join("mcp-after-subscribe.marker");
    let _seam_env = EnvVarGuard::set("PKVSYNC_ENABLE_TEST_SEAMS", "1");
    let _marker_env =
        EnvVarGuard::set_path("PKVSYNC_TEST_SSE_PAUSE_AFTER_SUBSCRIBE_MARKER", &marker);
    let _pause_env = EnvVarGuard::set("PKVSYNC_TEST_SSE_PAUSE_AFTER_SUBSCRIBE_MS", "250");

    let reconnect = tokio::spawn({
        let addr = ts.addr;
        let raw = raw.clone();
        let first_commit = first_commit.clone();
        async move {
            mcp_sse_request(addr, &raw, Some(&first_commit))
                .send()
                .await
                .unwrap()
        }
    });

    wait_for_file(&marker).await;
    let during_reconnect = push(
        &state,
        &auth,
        &vid.id,
        Some(&first_commit),
        None,
        PushReq {
            device_name: Some("during-reconnect".into()),
            changes: vec![PushChange::Text {
                path: "during-reconnect.md".into(),
                content: "dedupe me".into(),
            }],
        },
    )
    .await
    .unwrap();

    let sse_resp = reconnect.await.unwrap();
    let body = read_until(sse_resp, &[format!("id: {}", during_reconnect.new_commit)]).await;

    assert_eq!(
        body.matches(&format!("id: {}", during_reconnect.new_commit))
            .count(),
        1,
        "commit seen by both replay and live stream must be emitted exactly once, got: {body}"
    );
}

#[tokio::test]
async fn mcp_sse_reconnect_with_multiple_vaults_signals_lagged() {
    let (ts, state, auth, raw) = start_test_server().await;
    let vault_a = vault::create_vault(&state, &auth.user_id, "a")
        .await
        .unwrap();
    let vault_b = vault::create_vault(&state, &auth.user_id, "b")
        .await
        .unwrap();

    let a1 = push(
        &state,
        &auth,
        &vault_a.id,
        None,
        None,
        PushReq {
            device_name: Some("a1".into()),
            changes: vec![PushChange::Text {
                path: "a1.md".into(),
                content: "a1".into(),
            }],
        },
    )
    .await
    .unwrap();
    let a2 = push(
        &state,
        &auth,
        &vault_a.id,
        Some(&a1.new_commit),
        None,
        PushReq {
            device_name: Some("a2".into()),
            changes: vec![PushChange::Text {
                path: "a2.md".into(),
                content: "a2".into(),
            }],
        },
    )
    .await
    .unwrap();
    let b1 = push(
        &state,
        &auth,
        &vault_b.id,
        None,
        None,
        PushReq {
            device_name: Some("b1".into()),
            changes: vec![PushChange::Text {
                path: "b1.md".into(),
                content: "b1".into(),
            }],
        },
    )
    .await
    .unwrap();

    let sse_resp = mcp_sse_request(ts.addr, &raw, Some(&a1.new_commit))
        .send()
        .await
        .unwrap();
    let body = read_until(
        sse_resp,
        &[format!("id: {}", a2.new_commit), "event: lagged".into()],
    )
    .await;

    assert!(
        body.contains(&format!("id: {}", a2.new_commit)),
        "expected vault A's missed commit to be replayed, got: {body}"
    );
    assert!(
        body.contains("event: lagged"),
        "expected a lagged signal for vault B whose position is unknown, got: {body}"
    );
    assert!(
        !body.contains(&format!("id: {}", b1.new_commit)),
        "vault B's commit predates the reconnection point and must not be replayed, got: {body}"
    );
}
