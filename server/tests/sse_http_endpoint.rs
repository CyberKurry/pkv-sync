use ipnet::IpNet;
use pkv_sync_server::auth::{password, token};
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
use pkv_sync_server::server;
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

async fn start_test_server() -> (TestServer, AppState, String, String) {
    start_test_server_with_sse_limit(None).await
}

async fn start_test_server_with_sse_limit(
    sse_limit: Option<usize>,
) -> (TestServer, AppState, String, String) {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let key = "k_sse_test_key_0123456789ab".to_string();
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
        update_check: pkv_sync_server::config::UpdateCheckConfig {
            enabled: false,
            ..Default::default()
        },
        mcp: Default::default(),
    });

    let db = pool::connect(&cfg.storage.db_path).await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    let state = AppState::new(db, data_dir.clone(), "t".into(), false)
        .await
        .unwrap();
    if let Some(limit) = sse_limit {
        state.set_sse_per_user_limit_for_tests(limit);
    }

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
    state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&raw),
            device_id: "device-sse-http",
            device_name: "sse-http",
        })
        .await
        .unwrap();
    let vault = vault::create_vault(&state, &user.id, "main").await.unwrap();

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
        let ready = client()
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

    (ts, state, raw, vault.id)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

fn auth_headers(req: reqwest::RequestBuilder, key: &str) -> reqwest::RequestBuilder {
    req.header("user-agent", "PKVSync-Plugin/0.1.0")
        .header("x-pkvsync-deployment-key", key)
}

#[tokio::test]
async fn sse_endpoint_requires_auth() {
    let (ts, _state, _raw, vid) = start_test_server().await;

    let resp = auth_headers(
        client().get(format!("http://{}/api/vaults/{}/events", ts.addr, vid)),
        &ts.key,
    )
    .send()
    .await
    .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sse_endpoint_returns_event_stream_headers() {
    let (ts, _state, raw, vid) = start_test_server().await;

    let resp = auth_headers(
        client()
            .get(format!("http://{}/api/vaults/{}/events", ts.addr, vid))
            .bearer_auth(&raw),
        &ts.key,
    )
    .timeout(Duration::from_secs(2))
    .send()
    .await
    .unwrap();

    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        ct.contains("text/event-stream"),
        "expected text/event-stream, got: {ct}"
    );
    assert!(resp
        .headers()
        .get("cache-control")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("no-cache"));
    assert_eq!(
        resp.headers()
            .get("x-accel-buffering")
            .unwrap()
            .to_str()
            .unwrap(),
        "no"
    );
}

#[tokio::test]
async fn sse_endpoint_returns_cors_header_on_successful_subscription() {
    let (ts, _state, raw, vid) = start_test_server().await;

    let resp = auth_headers(
        client()
            .get(format!("http://{}/api/vaults/{}/events", ts.addr, vid))
            .header("origin", "app://obsidian.md")
            .bearer_auth(&raw),
        &ts.key,
    )
    .timeout(Duration::from_secs(2))
    .send()
    .await
    .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let allow_origin = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        !allow_origin.is_empty(),
        "successful SSE subscription must carry Access-Control-Allow-Origin"
    );
}

#[tokio::test]
async fn sse_endpoint_rejects_when_subscriber_limit_is_reached() {
    let (ts, _state, raw, vid) = start_test_server_with_sse_limit(Some(1)).await;
    let sse_url = format!("http://{}/api/vaults/{}/events", ts.addr, vid);

    let first = auth_headers(client().get(&sse_url).bearer_auth(&raw), &ts.key)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), reqwest::StatusCode::OK);

    let second = auth_headers(client().get(&sse_url).bearer_auth(&raw), &ts.key)
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);
    let body = second.text().await.unwrap();
    assert!(body.contains("rate_limited"), "unexpected body: {body}");

    drop(first);
    tokio::time::sleep(Duration::from_millis(50)).await;

    let third = auth_headers(client().get(&sse_url).bearer_auth(&raw), &ts.key)
        .send()
        .await
        .unwrap();
    assert_eq!(third.status(), reqwest::StatusCode::OK);
}

#[tokio::test]
async fn sse_receives_commit_event_after_push() {
    let (ts, _state, raw, vid) = start_test_server().await;

    let sse_url = format!("http://{}/api/vaults/{}/events", ts.addr, vid);
    let push_url = format!("http://{}/api/vaults/{}/push", ts.addr, vid);

    let sse_resp = auth_headers(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap()
            .get(&sse_url)
            .bearer_auth(&raw),
        &ts.key,
    )
    .send()
    .await
    .unwrap();

    let ct = sse_resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        ct.contains("text/event-stream"),
        "expected SSE content type"
    );

    tokio::time::sleep(Duration::from_millis(100)).await;

    let push_body = serde_json::json!({
        "device_name": "test",
        "changes": [{"kind":"text","path":"note.md","content":"hello"}]
    });

    let push_resp = auth_headers(
        reqwest::Client::new()
            .post(&push_url)
            .bearer_auth(&raw)
            .header(
                "idempotency-key",
                format!("push-{}", uuid::Uuid::new_v4().simple()),
            )
            .json(&push_body),
        &ts.key,
    )
    .send()
    .await
    .unwrap();

    assert_eq!(push_resp.status(), reqwest::StatusCode::OK);

    let mut body = String::new();
    let mut stream = sse_resp.bytes_stream();
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
                if body.contains("commit") && body.contains("note.md") {
                    break;
                }
            }
            _ => break,
        }
    }

    assert!(
        body.contains("commit"),
        "expected commit event in SSE stream, got: {body}"
    );
    assert!(
        body.contains("note.md"),
        "expected note.md path in SSE event, got: {body}"
    );
}

#[tokio::test]
async fn sse_stream_closes_after_token_revoked_without_forwarding_inline_text() {
    let (ts, state, raw, vid) = start_test_server().await;
    let (old_token, user_id) = state
        .tokens
        .find_by_hash(&token::hash(&raw))
        .await
        .unwrap()
        .unwrap();
    let replacement_raw = token::generate();
    state
        .tokens
        .create(NewToken {
            user_id: &user_id,
            token_hash: &token::hash(&replacement_raw),
            device_id: "device-sse-http-replacement",
            device_name: "sse-http replacement",
        })
        .await
        .unwrap();

    let sse_url = format!("http://{}/api/vaults/{}/events", ts.addr, vid);
    let push_url = format!("http://{}/api/vaults/{}/push", ts.addr, vid);

    let sse_resp = auth_headers(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap()
            .get(&sse_url)
            .bearer_auth(&raw),
        &ts.key,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(sse_resp.status(), reqwest::StatusCode::OK);

    state
        .tokens
        .revoke(&old_token.id, chrono::Utc::now().timestamp())
        .await
        .unwrap();

    let push_body = serde_json::json!({
        "device_name": "replacement",
        "changes": [{"kind":"text","path":"secret.md","content":"secret-inline"}]
    });
    let push_resp = auth_headers(
        reqwest::Client::new()
            .post(&push_url)
            .bearer_auth(&replacement_raw)
            .header(
                "idempotency-key",
                format!("push-{}", uuid::Uuid::new_v4().simple()),
            )
            .json(&push_body),
        &ts.key,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(push_resp.status(), reqwest::StatusCode::OK);

    use futures_util::StreamExt;
    let mut body = String::new();
    let mut stream = sse_resp.bytes_stream();
    let mut closed = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let chunk = tokio::select! {
            chunk = stream.next() => chunk,
            _ = tokio::time::sleep_until(deadline) => break,
        };
        match chunk {
            Some(Ok(bytes)) => {
                body.push_str(&String::from_utf8_lossy(&bytes));
                if body.contains("secret-inline") {
                    break;
                }
            }
            Some(Err(err)) => panic!("unexpected SSE stream error: {err}"),
            None => {
                closed = true;
                break;
            }
        }
    }

    assert!(
        !body.contains("secret-inline"),
        "revoked token stream received inline file content: {body}"
    );
    assert!(
        closed,
        "revoked token stream should close promptly instead of staying open; body: {body}"
    );
}

#[tokio::test]
async fn sse_records_subscribed_activity() {
    let (ts, state, raw, vid) = start_test_server().await;

    let _resp = auth_headers(
        client()
            .get(format!("http://{}/api/vaults/{}/events", ts.addr, vid))
            .bearer_auth(&raw),
        &ts.key,
    )
    .timeout(Duration::from_secs(2))
    .send()
    .await
    .unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    let row: (String,) = sqlx::query_as(
        "SELECT action FROM sync_activity WHERE vault_id = ? AND action = 'sse_subscribed'",
    )
    .bind(&vid)
    .fetch_one(&state.pool)
    .await
    .unwrap();

    assert_eq!(row.0, "sse_subscribed");
}
