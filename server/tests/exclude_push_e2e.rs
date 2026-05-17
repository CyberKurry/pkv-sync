use ipnet::IpNet;
use pkv_sync_server::auth::{password, token};
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, RuntimeConfigRepo, TokenRepo, UserRepo};
use pkv_sync_server::server;
use pkv_sync_server::service::AppState;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

struct TestServer {
    addr: SocketAddr,
    key: String,
    token: String,
    _tmp: tempfile::TempDir,
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

async fn start_server_with_excludes(globs: Vec<String>) -> TestServer {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("d");
    std::fs::create_dir_all(&data_dir).unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let bind = listener.local_addr().unwrap();
    let key = "k_excl".to_string();
    let cfg = Arc::new(Config {
        server: ServerConfig {
            bind_addr: bind,
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
    let state = AppState::new(db, data_dir, "test".into(), false)
        .await
        .unwrap();
    let user = state
        .users
        .create(NewUser {
            username: "alice".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    let plaintext_token = token::generate();
    state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&plaintext_token),
            device_id: "device-excl",
            device_name: "excl",
        })
        .await
        .unwrap();
    state
        .runtime_cfg_repo
        .set_extra_exclude_globs(globs, None)
        .await
        .unwrap();
    drop(state);

    let cfg2 = cfg.clone();
    let handle = tokio::spawn(async move {
        let _ = server::run_with_listener(cfg2, listener).await;
    });
    let server = TestServer {
        addr: bind,
        key,
        token: plaintext_token,
        _tmp: tmp,
        handle,
    };
    for _ in 0..50 {
        let ready = c()
            .get(format!("http://{}/api/health", server.addr))
            .header("user-agent", "PKVSync-Plugin/0.1.0")
            .header("x-pkvsync-deployment-key", &server.key)
            .send()
            .await
            .map(|resp| resp.status().as_u16() == 200)
            .unwrap_or(false);
        if ready {
            return server;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("server not started");
}

fn c() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

fn headers(r: reqwest::RequestBuilder, key: &str) -> reqwest::RequestBuilder {
    r.header("user-agent", "PKVSync-Plugin/0.1.0")
        .header("x-pkvsync-deployment-key", key)
}

async fn create_vault(server: &TestServer) -> String {
    let client = c();
    let create = headers(
        client.post(format!("http://{}/api/vaults", server.addr)),
        &server.key,
    )
    .bearer_auth(&server.token)
    .json(&serde_json::json!({"name":"main"}))
    .send()
    .await
    .unwrap();
    assert_eq!(create.status(), 201);
    let body: serde_json::Value = create.json().await.unwrap();
    body["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn push_rejects_path_matching_exclude_glob() {
    let server = start_server_with_excludes(vec!["*.tmp".into()]).await;
    let vault_id = create_vault(&server).await;
    let client = c();

    let push = headers(
        client.post(format!("http://{}/api/vaults/{vault_id}/push", server.addr)),
        &server.key,
    )
    .bearer_auth(&server.token)
    .header("idempotency-key", "excl-1")
    .json(&serde_json::json!({
        "device_name":"test",
        "changes":[
            {"kind":"text","path":"scratch/draft.tmp","content":"throwaway"}
        ]
    }))
    .send()
    .await
    .unwrap();

    assert_eq!(push.status(), 400);
    let body: serde_json::Value = push.json().await.unwrap();
    assert_eq!(body["error"]["code"], "path_excluded");
}

#[tokio::test]
async fn push_rejects_path_matching_nested_exclude_glob() {
    let server = start_server_with_excludes(vec!["build/**".into()]).await;
    let vault_id = create_vault(&server).await;
    let client = c();

    let push = headers(
        client.post(format!("http://{}/api/vaults/{vault_id}/push", server.addr)),
        &server.key,
    )
    .bearer_auth(&server.token)
    .header("idempotency-key", "excl-2")
    .json(&serde_json::json!({
        "device_name":"test",
        "changes":[
            {"kind":"text","path":"build/output/main.js","content":"compiled"}
        ]
    }))
    .send()
    .await
    .unwrap();

    assert_eq!(push.status(), 400);
    let body: serde_json::Value = push.json().await.unwrap();
    assert_eq!(body["error"]["code"], "path_excluded");
}

#[tokio::test]
async fn push_accepts_non_matching_path_when_excludes_configured() {
    let server = start_server_with_excludes(vec!["*.tmp".into()]).await;
    let vault_id = create_vault(&server).await;
    let client = c();

    let push = headers(
        client.post(format!("http://{}/api/vaults/{vault_id}/push", server.addr)),
        &server.key,
    )
    .bearer_auth(&server.token)
    .header("idempotency-key", "excl-3")
    .json(&serde_json::json!({
        "device_name":"test",
        "changes":[
            {"kind":"text","path":"note.md","content":"keep me"}
        ]
    }))
    .send()
    .await
    .unwrap();

    assert_eq!(push.status(), 200);
}
