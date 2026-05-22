use ipnet::IpNet;
use pkv_sync_server::auth::{password, token};
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
use pkv_sync_server::server;
use pkv_sync_server::service::events::MAX_SSE_REPLAY_COMMITS;
use pkv_sync_server::service::sync::{self, PushChange, PushReq};
use pkv_sync_server::service::vault;
use pkv_sync_server::service::AppState;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

struct TestServer {
    addr: SocketAddr,
    key: String,
    _tmp: tempfile::TempDir,
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

async fn start_test_server() -> (TestServer, AppState, String, String, String) {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let key = "k_sse_replay_test_0123456789".to_string();
    let cfg = Arc::new(Config {
        server: ServerConfig {
            bind_addr: addr,
            deployment_key: key.clone(),
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
    });

    let db = pool::connect(&cfg.storage.db_path).await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    let state = AppState::new(db, data_dir.clone(), "t".into(), false)
        .await
        .unwrap();

    state
        .users
        .create(NewUser {
            username: "admin".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: true,
        })
        .await
        .unwrap();
    let user = state
        .users
        .create(NewUser {
            username: "u".into(),
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
            device_id: "device-sse-replay",
            device_name: "sse-replay",
        })
        .await
        .unwrap();
    let vault = vault::create_vault(&state, &user.id, "main").await.unwrap();
    let auth = pkv_sync_server::auth::AuthenticatedUser {
        user_id: user.id,
        username: user.username,
        is_admin: false,
        token_id: token_row.id,
        device_id: token_row.device_id,
    };

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

    let state_clone = state.clone();
    let limiter = pkv_sync_server::auth::LoginRateLimiter::new(
        10,
        Duration::from_secs(900),
        Duration::from_secs(900),
    );
    let cfg2 = cfg.clone();
    let handle = tokio::spawn(async move {
        let _ = server::run_with_listener_and_state(cfg2, listener, state_clone, limiter).await;
    });

    let ts = TestServer {
        addr,
        key,
        _tmp: tmp,
        handle,
    };

    for _ in 0..50 {
        let ready = reqwest::Client::new()
            .get(format!("http://{}/api/health", ts.addr))
            .header("user-agent", "PKVSync-Plugin/0.1.0")
            .header("x-pkvsync-deployment-key", &ts.key)
            .send()
            .await
            .map(|resp| resp.status().as_u16() == 200)
            .unwrap_or(false);
        if ready {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    (ts, state, raw, vault.id, first.new_commit)
}

fn auth_headers(req: reqwest::RequestBuilder, key: &str) -> reqwest::RequestBuilder {
    req.header("user-agent", "PKVSync-Plugin/0.1.0")
        .header("x-pkvsync-deployment-key", key)
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
async fn live_sse_commit_events_include_commit_id() {
    let (ts, state, raw, vid, first_commit) = start_test_server().await;
    let sse_url = format!("http://{}/api/vaults/{}/events", ts.addr, vid);

    let sse_resp = auth_headers(
        reqwest::Client::new().get(&sse_url).bearer_auth(&raw),
        &ts.key,
    )
    .send()
    .await
    .unwrap();

    let auth = {
        let user = state.users.find_by_username("u").await.unwrap().unwrap();
        let (token_row, _username) = state
            .tokens
            .find_by_hash(&token::hash(&raw))
            .await
            .unwrap()
            .unwrap();
        pkv_sync_server::auth::AuthenticatedUser {
            user_id: user.id,
            username: user.username,
            is_admin: false,
            token_id: token_row.id,
            device_id: token_row.device_id,
        }
    };
    let second = sync::push(
        &state,
        &auth,
        &vid,
        Some(&first_commit),
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

    let body = read_until(sse_resp, &[&format!("id: {}", second.new_commit), "b.md"]).await;

    assert!(
        body.contains(&format!("id: {}", second.new_commit)),
        "expected SSE id to be commit sha, got: {body}"
    );
}

#[tokio::test]
async fn reconnect_with_last_event_id_replays_missed_commits() {
    let (ts, state, raw, vid, first_commit) = start_test_server().await;
    let auth = {
        let user = state.users.find_by_username("u").await.unwrap().unwrap();
        let (token_row, _username) = state
            .tokens
            .find_by_hash(&token::hash(&raw))
            .await
            .unwrap()
            .unwrap();
        pkv_sync_server::auth::AuthenticatedUser {
            user_id: user.id,
            username: user.username,
            is_admin: false,
            token_id: token_row.id,
            device_id: token_row.device_id,
        }
    };

    let second = sync::push(
        &state,
        &auth,
        &vid,
        Some(&first_commit),
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
    let third = sync::push(
        &state,
        &auth,
        &vid,
        Some(&second.new_commit),
        None,
        PushReq {
            device_name: Some("third".into()),
            changes: vec![PushChange::Text {
                path: "c.md".into(),
                content: "three".into(),
            }],
        },
    )
    .await
    .unwrap();

    let sse_url = format!("http://{}/api/vaults/{}/events", ts.addr, vid);
    let sse_resp = auth_headers(
        reqwest::Client::new()
            .get(&sse_url)
            .bearer_auth(&raw)
            .header("Last-Event-ID", &first_commit),
        &ts.key,
    )
    .send()
    .await
    .unwrap();

    let body = read_until(
        sse_resp,
        &[
            &format!("id: {}", second.new_commit),
            &format!("id: {}", third.new_commit),
            "b.md",
            "c.md",
        ],
    )
    .await;

    assert!(
        body.contains(&format!("id: {}", second.new_commit)),
        "expected replay of second commit, got: {body}"
    );
    assert!(
        body.contains(&format!("id: {}", third.new_commit)),
        "expected replay of third commit, got: {body}"
    );
    assert!(body.find("b.md") < body.find("c.md"));
}

#[tokio::test]
async fn reconnect_too_far_behind_emits_lagged_instead_of_replaying_unbounded_history() {
    let (ts, state, raw, vid, first_commit) = start_test_server().await;
    let auth = {
        let user = state.users.find_by_username("u").await.unwrap().unwrap();
        let (token_row, _username) = state
            .tokens
            .find_by_hash(&token::hash(&raw))
            .await
            .unwrap()
            .unwrap();
        pkv_sync_server::auth::AuthenticatedUser {
            user_id: user.id,
            username: user.username,
            is_admin: false,
            token_id: token_row.id,
            device_id: token_row.device_id,
        }
    };

    let mut parent = first_commit.clone();
    for idx in 0..=MAX_SSE_REPLAY_COMMITS {
        let pushed = sync::push(
            &state,
            &auth,
            &vid,
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

    let sse_url = format!("http://{}/api/vaults/{}/events", ts.addr, vid);
    let sse_resp = auth_headers(
        reqwest::Client::new()
            .get(&sse_url)
            .bearer_auth(&raw)
            .header("Last-Event-ID", &first_commit),
        &ts.key,
    )
    .send()
    .await
    .unwrap();

    let body = read_until(sse_resp, &["event: lagged"]).await;

    assert!(
        body.contains("event: lagged"),
        "expected replay overflow to emit lagged, got: {body}"
    );
}
