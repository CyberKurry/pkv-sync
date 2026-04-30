//! End-to-end smoke test for the full middleware chain.

use ipnet::IpNet;
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::server;
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

async fn start_test_server() -> TestServer {
    let tmp = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let actual_bind = listener.local_addr().unwrap();

    let key = "k_testkey1234567890abcdef".to_string();
    let cfg = Arc::new(Config {
        server: ServerConfig {
            bind_addr: actual_bind,
            deployment_key: key.clone(),
            public_host: None,
        },
        storage: StorageConfig {
            data_dir: tmp.path().join("data"),
            db_path: tmp.path().join("data").join("metadata.db"),
        },
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
        addr: actual_bind,
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
    panic!("server did not start within 1s");
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
}

#[tokio::test]
async fn missing_ua_returns_404() {
    let server = start_test_server().await;
    let resp = client()
        .get(format!("http://{}/api/health", server.addr))
        .header("x-pkvsync-deployment-key", &server.key)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn missing_deployment_key_returns_404() {
    let server = start_test_server().await;
    let resp = client()
        .get(format!("http://{}/api/health", server.addr))
        .header("user-agent", "PKVSync-Plugin/0.1.0")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn full_stack_health_returns_ok() {
    let server = start_test_server().await;
    let resp = client()
        .get(format!("http://{}/api/health", server.addr))
        .header("user-agent", "PKVSync-Plugin/0.1.0")
        .header("x-pkvsync-deployment-key", &server.key)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn config_endpoint_reachable_through_stack() {
    let server = start_test_server().await;
    let resp = client()
        .get(format!("http://{}/api/config", server.addr))
        .header("user-agent", "PKVSync-Plugin/0.1.0")
        .header("x-pkvsync-deployment-key", &server.key)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let request_id = resp
        .headers()
        .get("x-request-id")
        .expect("request_id present");
    assert_eq!(request_id.to_str().unwrap().len(), 32);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["registration"], "disabled");
}

#[tokio::test]
async fn auth_register_route_reachable_through_stack() {
    let server = start_test_server().await;
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
