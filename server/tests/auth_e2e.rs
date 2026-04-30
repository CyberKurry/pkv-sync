use ipnet::IpNet;
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::repos::{RegistrationMode, RuntimeConfigRepo, SqliteRuntimeConfigRepo};
use pkv_sync_server::{db::pool, server};
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

async fn start_server(mode: Option<RegistrationMode>) -> TestServer {
    let tmp = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let key = "k_authe2e".to_string();
    let data_dir = tmp.path().join("data");
    let db_path = data_dir.join("metadata.db");

    if let Some(mode) = mode {
        let pool = pool::connect(&db_path).await.unwrap();
        pool::migrate_up(&pool).await.unwrap();
        SqliteRuntimeConfigRepo::new(pool)
            .set_registration_mode(mode, None)
            .await
            .unwrap();
    }

    let cfg = Arc::new(Config {
        server: ServerConfig {
            bind_addr: addr,
            deployment_key: key.clone(),
            public_host: None,
        },
        storage: StorageConfig { data_dir, db_path },
        network: NetworkConfig {
            trusted_proxies: vec!["127.0.0.1/32".parse::<IpNet>().unwrap()],
        },
        logging: LoggingConfig::default(),
    });
    let cfg2 = cfg.clone();
    let handle = tokio::spawn(async move {
        let _ = server::run_with_listener(cfg2, listener).await;
    });

    let server = TestServer {
        addr,
        key,
        _tmp: tmp,
        handle,
    };
    for _ in 0..50 {
        let ready = client()
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
    panic!("server did not start");
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

#[tokio::test]
async fn registration_disabled_by_default() {
    let server = start_server(None).await;
    let resp = client()
        .post(format!("http://{}/api/auth/register", server.addr))
        .header("user-agent", "PKVSync-Plugin/0.1.0")
        .header("x-pkvsync-deployment-key", &server.key)
        .json(&serde_json::json!({
            "username": "alice",
            "password": "passw0rd!!",
            "device_name": "desktop"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn open_registration_register_login_me_roundtrip() {
    let server = start_server(Some(RegistrationMode::Open)).await;
    let c = client();

    let register_resp = c
        .post(format!("http://{}/api/auth/register", server.addr))
        .header("user-agent", "PKVSync-Plugin/0.1.0")
        .header("x-pkvsync-deployment-key", &server.key)
        .json(&serde_json::json!({
            "username": "alice",
            "password": "passw0rd!!",
            "device_name": "desktop"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(register_resp.status(), 201);

    let login_resp = c
        .post(format!("http://{}/api/auth/login", server.addr))
        .header("user-agent", "PKVSync-Plugin/0.1.0")
        .header("x-pkvsync-deployment-key", &server.key)
        .json(&serde_json::json!({
            "username": "alice",
            "password": "passw0rd!!",
            "device_name": "laptop"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(login_resp.status(), 200);
    let login_body: serde_json::Value = login_resp.json().await.unwrap();
    let token = login_body["token"].as_str().unwrap();

    let me_resp = c
        .get(format!("http://{}/api/me", server.addr))
        .header("user-agent", "PKVSync-Plugin/0.1.0")
        .header("x-pkvsync-deployment-key", &server.key)
        .header("authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(me_resp.status(), 200);
    let me_body: serde_json::Value = me_resp.json().await.unwrap();
    assert_eq!(me_body["username"], "alice");
    assert!(me_body["vaults"].as_array().unwrap().is_empty());
}
