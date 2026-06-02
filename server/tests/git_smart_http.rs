//! Integration tests for the Git smart HTTP endpoints.

use base64::Engine;
use ipnet::IpNet;
use pkv_sync_server::auth::{password, token};
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, RuntimeConfigRepo, TokenRepo, UserRepo};
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
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let key = "k_git_test_key_0123456789a".to_string();
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

    // Enable git smart HTTP in runtime config
    let state = AppState::new(db, data_dir.clone(), "t".into(), true)
        .await
        .unwrap();
    state
        .runtime_cfg_repo
        .set_enable_git_smart_http(true, None)
        .await
        .unwrap();
    let cfg_snapshot = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg_snapshot).await;

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
            device_id: "device-git-http",
            device_name: "git-http",
        })
        .await
        .unwrap();
    let v = vault::create_vault(&state, &user.id, "main").await.unwrap();

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

    // Wait for server to be ready
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

    (ts, state, raw, v.id)
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

fn basic_auth_header(token_val: &str) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(format!(":{token_val}"));
    format!("Basic {encoded}")
}

/// Push a text file to a vault via the sync API so that the bare git repo is
/// created on disk. Without at least one push, `git upload-pack` will fail
/// because the repo directory doesn't exist yet.
async fn push_text_file(ts: &TestServer, raw: &str, vault_id: &str, path: &str, content: &str) {
    let push_body = serde_json::json!({
        "device_name": "test",
        "changes": [{"kind":"text","path":path,"content":content}]
    });
    let resp = auth_headers(
        client()
            .post(format!("http://{}/api/vaults/{}/push", ts.addr, vault_id))
            .bearer_auth(raw)
            .header("content-type", "application/json")
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
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::OK,
        "push should succeed"
    );
}

#[tokio::test]
async fn info_refs_returns_503_when_disabled() {
    let (ts, state, raw, vid) = start_test_server().await;

    // First push to create the bare repo on disk
    push_text_file(&ts, &raw, &vid, "note.md", "hello").await;

    // Disable git smart HTTP
    state
        .runtime_cfg_repo
        .set_enable_git_smart_http(false, None)
        .await
        .unwrap();
    let cfg_snapshot = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg_snapshot).await;

    let resp = auth_headers(
        client().get(format!(
            "http://{}/git/{}/info/refs?service=git-upload-pack",
            ts.addr, vid
        )),
        &ts.key,
    )
    .header("authorization", basic_auth_header(&raw))
    .send()
    .await
    .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn info_refs_rejects_no_auth() {
    let (ts, _state, _raw, vid) = start_test_server().await;

    let resp = auth_headers(
        client().get(format!(
            "http://{}/git/{}/info/refs?service=git-upload-pack",
            ts.addr, vid
        )),
        &ts.key,
    )
    .send()
    .await
    .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn info_refs_rejects_wrong_token() {
    let (ts, _state, _raw, vid) = start_test_server().await;

    let resp = auth_headers(
        client().get(format!(
            "http://{}/git/{}/info/refs?service=git-upload-pack",
            ts.addr, vid
        )),
        &ts.key,
    )
    .header(
        "authorization",
        basic_auth_header("pks_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    )
    .send()
    .await
    .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn git_smart_http_routes_are_rate_limited() {
    let (ts, _state, _raw, vid) = start_test_server().await;
    let mut saw_rate_limit = false;

    for _ in 0..130 {
        let resp = auth_headers(
            client().get(format!(
                "http://{}/git/{}/info/refs?service=git-upload-pack",
                ts.addr, vid
            )),
            &ts.key,
        )
        .header(
            "authorization",
            basic_auth_header(
                "pks_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
        )
        .send()
        .await
        .unwrap();
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            saw_rate_limit = true;
            break;
        }
        assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);
    }

    assert!(
        saw_rate_limit,
        "git smart HTTP auth attempts were not rate limited"
    );
}

#[tokio::test]
async fn info_refs_rejects_disabled_user_like_invalid_credentials() {
    let (ts, state, raw, vid) = start_test_server().await;
    let user = state.users.find_by_username("u").await.unwrap().unwrap();
    state.users.set_active(&user.id, false).await.unwrap();

    let resp = auth_headers(
        client().get(format!(
            "http://{}/git/{}/info/refs?service=git-upload-pack",
            ts.addr, vid
        )),
        &ts.key,
    )
    .header("authorization", basic_auth_header(&raw))
    .send()
    .await
    .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);
    let body = resp.text().await.unwrap().to_lowercase();
    assert!(!body.contains("disabled"));
    assert!(!body.contains("forbidden"));
}

#[tokio::test]
async fn upload_pack_rejects_disabled_user_like_invalid_credentials() {
    let (ts, state, raw, vid) = start_test_server().await;
    let user = state.users.find_by_username("u").await.unwrap().unwrap();
    state.users.set_active(&user.id, false).await.unwrap();

    let resp = auth_headers(
        client()
            .post(format!("http://{}/git/{}/git-upload-pack", ts.addr, vid))
            .body(Vec::<u8>::new()),
        &ts.key,
    )
    .header("authorization", basic_auth_header(&raw))
    .send()
    .await
    .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);
    let body = resp.text().await.unwrap().to_lowercase();
    assert!(!body.contains("disabled"));
    assert!(!body.contains("forbidden"));
}

#[tokio::test]
async fn info_refs_rejects_wrong_service() {
    let (ts, _state, raw, vid) = start_test_server().await;

    // Push to create the bare repo
    push_text_file(&ts, &raw, &vid, "note.md", "hello").await;

    let resp = auth_headers(
        client().get(format!(
            "http://{}/git/{}/info/refs?service=git-receive-pack",
            ts.addr, vid
        )),
        &ts.key,
    )
    .header("authorization", basic_auth_header(&raw))
    .send()
    .await
    .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn info_refs_rejects_malformed_vault_id() {
    let (ts, _state, raw, _vid) = start_test_server().await;

    let resp = auth_headers(
        client().get(format!(
            "http://{}/git/not-a-vault-id/info/refs?service=git-upload-pack",
            ts.addr
        )),
        &ts.key,
    )
    .header("authorization", basic_auth_header(&raw))
    .send()
    .await
    .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upload_pack_rejects_malformed_vault_id() {
    let (ts, _state, raw, _vid) = start_test_server().await;

    let resp = auth_headers(
        client()
            .post(format!(
                "http://{}/git/not-a-vault-id/git-upload-pack",
                ts.addr
            ))
            .body(Vec::<u8>::new()),
        &ts.key,
    )
    .header("authorization", basic_auth_header(&raw))
    .send()
    .await
    .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn info_refs_rejects_vault_not_owned() {
    let (ts, state, raw, vid) = start_test_server().await;

    // Push to create the bare repo
    push_text_file(&ts, &raw, &vid, "note.md", "hello").await;

    // Create another user with a different token
    let other_user = state
        .users
        .create(NewUser {
            username: "other".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    let other_raw = token::generate();
    state
        .tokens
        .create(NewToken {
            user_id: &other_user.id,
            token_hash: &token::hash(&other_raw),
            device_id: "device-other",
            device_name: "other",
        })
        .await
        .unwrap();

    let resp = auth_headers(
        client().get(format!(
            "http://{}/git/{}/info/refs?service=git-upload-pack",
            ts.addr, vid
        )),
        &ts.key,
    )
    .header("authorization", basic_auth_header(&other_raw))
    .send()
    .await
    .unwrap();

    // Should be 404 (vault not found for this user)
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::NOT_FOUND,
        "expected 404 for vault not owned by user, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn info_refs_returns_advertisement_for_owned_vault() {
    let (ts, _state, raw, vid) = start_test_server().await;

    // Push a file first so the bare git repo exists on disk
    push_text_file(&ts, &raw, &vid, "note.md", "hello from pkv").await;

    let resp = auth_headers(
        client().get(format!(
            "http://{}/git/{}/info/refs?service=git-upload-pack",
            ts.addr, vid
        )),
        &ts.key,
    )
    .header("authorization", basic_auth_header(&raw))
    .send()
    .await
    .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(
        ct, "application/x-git-upload-pack-advertisement",
        "expected correct content-type, got: {ct}"
    );

    let body = resp.bytes().await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    // Should contain the pkt-line service header
    assert!(
        body_str.contains("service=git-upload-pack"),
        "expected service header in response, got: {body_str}"
    );
}
