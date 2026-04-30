use ipnet::IpNet;
use pkv_sync_server::auth::{password, token};
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
use pkv_sync_server::server;
use pkv_sync_server::service::AppState;
use sha2::Digest;
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

async fn start_server_with_seeded_user() -> TestServer {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("d");
    std::fs::create_dir_all(&data_dir).unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let bind = listener.local_addr().unwrap();
    let key = "k_synce2e".to_string();
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
    let state = AppState::new(db, data_dir, "test".into()).await.unwrap();
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
            device_name: "sync-e2e",
        })
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

#[tokio::test]
async fn full_http_upload_push_pull_blob_and_text() {
    let server = start_server_with_seeded_user().await;
    let client = c();

    let create_vault = headers(
        client.post(format!("http://{}/api/vaults", server.addr)),
        &server.key,
    )
    .bearer_auth(&server.token)
    .json(&serde_json::json!({"name":"main"}))
    .send()
    .await
    .unwrap();
    assert_eq!(create_vault.status(), 201);
    let vault: serde_json::Value = create_vault.json().await.unwrap();
    let vault_id = vault["id"].as_str().unwrap();

    let blob_bytes = b"hello blob".to_vec();
    let blob_hash = hex::encode(sha2::Sha256::digest(&blob_bytes));

    let check = headers(
        client.post(format!(
            "http://{}/api/vaults/{vault_id}/upload/check",
            server.addr
        )),
        &server.key,
    )
    .bearer_auth(&server.token)
    .json(&serde_json::json!({"blob_hashes":[blob_hash]}))
    .send()
    .await
    .unwrap();
    assert_eq!(check.status(), 200);
    let check_body: serde_json::Value = check.json().await.unwrap();
    assert_eq!(check_body["missing"].as_array().unwrap().len(), 1);

    let upload = headers(
        client.post(format!(
            "http://{}/api/vaults/{vault_id}/upload/blob",
            server.addr
        )),
        &server.key,
    )
    .bearer_auth(&server.token)
    .header("content-hash", &blob_hash)
    .body(blob_bytes.clone())
    .send()
    .await
    .unwrap();
    assert_eq!(upload.status(), 201);

    let push = headers(
        client.post(format!("http://{}/api/vaults/{vault_id}/push", server.addr)),
        &server.key,
    )
    .bearer_auth(&server.token)
    .header("idempotency-key", "sync-e2e-1")
    .json(&serde_json::json!({
        "device_name":"test",
        "changes":[
            {"kind":"text","path":"folder/note.md","content":"hello text"},
            {"kind":"blob","path":"attachments/blob.bin","blob_hash":blob_hash,"size":blob_bytes.len(),"mime":"application/octet-stream"}
        ]
    }))
    .send()
    .await
    .unwrap();
    assert_eq!(push.status(), 200);
    let push_body: serde_json::Value = push.json().await.unwrap();
    let commit = push_body["new_commit"].as_str().unwrap();

    let pull = headers(
        client.get(format!("http://{}/api/vaults/{vault_id}/pull", server.addr)),
        &server.key,
    )
    .bearer_auth(&server.token)
    .send()
    .await
    .unwrap();
    assert_eq!(pull.status(), 200);
    let pull_body: serde_json::Value = pull.json().await.unwrap();
    assert_eq!(pull_body["to"], commit);
    assert!(pull_body["added"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"] == "folder/note.md" && f["content_inline"] == "hello text"));
    assert!(pull_body["added"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"] == "attachments/blob.bin" && f["blob_hash"] == blob_hash));

    let blob = headers(
        client.get(format!(
            "http://{}/api/vaults/{vault_id}/blobs/{blob_hash}",
            server.addr
        )),
        &server.key,
    )
    .bearer_auth(&server.token)
    .send()
    .await
    .unwrap();
    assert_eq!(blob.status(), 200);
    assert_eq!(blob.bytes().await.unwrap().as_ref(), blob_bytes.as_slice());
}
