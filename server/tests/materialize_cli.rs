//! Integration tests for `pkvsyncd materialize` CLI subcommand.
//!
//! Uses `AppState` + `Git2VaultStore` + `LocalFsBlobStore` to set up a vault
//! with text and binary files, then calls `materialize::run()` directly and
//! verifies the output directory contains the correct files.

use pkv_sync_server::cli::materialize;
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::service::AppState;
use pkv_sync_server::storage::blob::{BlobStore, LocalFsBlobStore};
use pkv_sync_server::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};

use bytes::Bytes;
use ipnet::IpNet;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Helper: create a fully initialized `AppState` with a temp data directory.
async fn setup_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("d");
    std::fs::create_dir_all(&data_dir).unwrap();

    let db_path = data_dir.join("metadata.db");
    let db = pool::connect(&db_path).await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();

    let state = AppState::new(db, data_dir, "test".into(), true)
        .await
        .unwrap();
    (state, tmp)
}

/// Helper: compute the SHA-256 hex digest of bytes.
fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Helper: build a `Config` pointing at the given data_dir.
fn make_config(data_dir: &Path) -> Config {
    Config {
        server: ServerConfig {
            bind_addr: "127.0.0.1:6710".parse().unwrap(),
            deployment_key: "k_test".to_string(),
            public_host: None,
        },
        storage: StorageConfig {
            data_dir: data_dir.to_path_buf(),
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
    }
}

#[tokio::test]
async fn materialize_text_file() {
    let (state, _tmp) = setup_state().await;
    let store = Git2VaultStore::new(state.default_vault_root());

    // Create vault with a text file
    store.ensure_repo("v1").await.unwrap();
    let _c1 = store
        .commit_changes(
            "v1",
            None,
            &[FileChange::Upsert {
                path: "notes/hello.md".into(),
                file: StoredFile::Text {
                    bytes: b"hello world".to_vec(),
                },
            }],
            "initial",
        )
        .await
        .unwrap();

    // Materialize
    let cfg = make_config(&state.data_dir);
    let out_dir = tempfile::tempdir().unwrap();
    materialize::run(&cfg, "v1", out_dir.path(), None).unwrap();

    // Verify
    let content = std::fs::read(out_dir.path().join("notes/hello.md")).unwrap();
    assert_eq!(content, b"hello world");
}

#[tokio::test]
async fn materialize_blob_pointer_file() {
    let (state, _tmp) = setup_state().await;
    let store = Git2VaultStore::new(state.default_vault_root());
    let blobs = LocalFsBlobStore::new(state.default_blob_root());

    // Create vault with a blob pointer
    store.ensure_repo("v2").await.unwrap();

    let binary_data = Bytes::from_static(b"\x89PNG\r\n\x1a\nfake-image-data");
    let hash = sha256_hex(&binary_data);

    // Store the actual blob
    blobs
        .put_verified(&hash, binary_data.clone())
        .await
        .unwrap();

    let _c1 = store
        .commit_changes(
            "v2",
            None,
            &[FileChange::Upsert {
                path: "images/photo.png".into(),
                file: StoredFile::BlobPointer {
                    hash: hash.clone(),
                    size: binary_data.len() as u64,
                    mime: Some("image/png".into()),
                },
            }],
            "add image",
        )
        .await
        .unwrap();

    // Materialize
    let cfg = make_config(&state.data_dir);
    let out_dir = tempfile::tempdir().unwrap();
    materialize::run(&cfg, "v2", out_dir.path(), None).unwrap();

    // Verify the binary content matches
    let content = std::fs::read(out_dir.path().join("images/photo.png")).unwrap();
    assert_eq!(content, &binary_data[..]);
}

#[tokio::test]
async fn materialize_mixed_text_and_blob() {
    let (state, _tmp) = setup_state().await;
    let store = Git2VaultStore::new(state.default_vault_root());
    let blobs = LocalFsBlobStore::new(state.default_blob_root());

    store.ensure_repo("v3").await.unwrap();

    let binary_data = Bytes::from_static(b"binary-content-here");
    let hash = sha256_hex(&binary_data);
    blobs
        .put_verified(&hash, binary_data.clone())
        .await
        .unwrap();

    let _c1 = store
        .commit_changes(
            "v3",
            None,
            &[
                FileChange::Upsert {
                    path: "readme.md".into(),
                    file: StoredFile::Text {
                        bytes: b"# My Vault".to_vec(),
                    },
                },
                FileChange::Upsert {
                    path: "assets/logo.png".into(),
                    file: StoredFile::BlobPointer {
                        hash: hash.clone(),
                        size: binary_data.len() as u64,
                        mime: Some("image/png".into()),
                    },
                },
            ],
            "mixed",
        )
        .await
        .unwrap();

    // Materialize
    let cfg = make_config(&state.data_dir);
    let out_dir = tempfile::tempdir().unwrap();
    materialize::run(&cfg, "v3", out_dir.path(), None).unwrap();

    // Verify text file
    let text = std::fs::read(out_dir.path().join("readme.md")).unwrap();
    assert_eq!(text, b"# My Vault");

    // Verify blob file
    let bin = std::fs::read(out_dir.path().join("assets/logo.png")).unwrap();
    assert_eq!(bin, &binary_data[..]);
}

#[tokio::test]
async fn materialize_at_specific_commit() {
    let (state, _tmp) = setup_state().await;
    let store = Git2VaultStore::new(state.default_vault_root());

    store.ensure_repo("v4").await.unwrap();

    // Commit 1: file with "v1"
    let c1 = store
        .commit_changes(
            "v4",
            None,
            &[FileChange::Upsert {
                path: "doc.md".into(),
                file: StoredFile::Text {
                    bytes: b"version 1".to_vec(),
                },
            }],
            "c1",
        )
        .await
        .unwrap();

    // Commit 2: file updated to "v2"
    let _c2 = store
        .commit_changes(
            "v4",
            Some(&c1),
            &[FileChange::Upsert {
                path: "doc.md".into(),
                file: StoredFile::Text {
                    bytes: b"version 2".to_vec(),
                },
            }],
            "c2",
        )
        .await
        .unwrap();

    // Materialize at c1
    let cfg = make_config(&state.data_dir);
    let out_dir = tempfile::tempdir().unwrap();
    materialize::run(&cfg, "v4", out_dir.path(), Some(&c1)).unwrap();

    let content = std::fs::read(out_dir.path().join("doc.md")).unwrap();
    assert_eq!(content, b"version 1");
}

#[tokio::test]
async fn materialize_rejects_non_empty_output_dir() {
    let (state, _tmp) = setup_state().await;
    let store = Git2VaultStore::new(state.default_vault_root());

    store.ensure_repo("v5").await.unwrap();

    let _c1 = store
        .commit_changes(
            "v5",
            None,
            &[FileChange::Upsert {
                path: "a.md".into(),
                file: StoredFile::Text {
                    bytes: b"a".to_vec(),
                },
            }],
            "c1",
        )
        .await
        .unwrap();

    // Create output dir with a file in it
    let out_dir = tempfile::tempdir().unwrap();
    std::fs::write(out_dir.path().join("existing.txt"), b"x").unwrap();

    let cfg = make_config(&state.data_dir);
    let result = materialize::run(&cfg, "v5", out_dir.path(), None);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("output directory exists and is not empty"),
        "unexpected error: {err_msg}"
    );
}

#[tokio::test]
async fn materialize_rejects_unknown_vault() {
    let (state, _tmp) = setup_state().await;

    let cfg = make_config(&state.data_dir);
    let out_dir = tempfile::tempdir().unwrap();
    let result = materialize::run(&cfg, "nonexistent", out_dir.path(), None);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("vault not found"),
        "unexpected error: {err_msg}"
    );
}

#[tokio::test]
async fn materialize_rejects_vault_id_path_traversal() {
    let (state, _tmp) = setup_state().await;
    let store = Git2VaultStore::new(state.data_dir.join("outside-vaults"));

    store.ensure_repo("escaped").await.unwrap();
    store
        .commit_changes(
            "escaped",
            None,
            &[FileChange::Upsert {
                path: "escaped.md".into(),
                file: StoredFile::Text {
                    bytes: b"escaped".to_vec(),
                },
            }],
            "escaped",
        )
        .await
        .unwrap();

    let cfg = make_config(&state.data_dir);
    let out_dir = tempfile::tempdir().unwrap();
    let result = materialize::run(&cfg, "../outside-vaults/escaped", out_dir.path(), None);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("invalid vault id"),
        "unexpected error: {err_msg}"
    );
}

#[tokio::test]
async fn materialize_blob_missing_from_storage() {
    let (state, _tmp) = setup_state().await;
    let store = Git2VaultStore::new(state.default_vault_root());

    // Create vault with a blob pointer, but DON'T store the actual blob
    store.ensure_repo("v6").await.unwrap();

    let fake_hash = "a".repeat(64);
    let _c1 = store
        .commit_changes(
            "v6",
            None,
            &[FileChange::Upsert {
                path: "missing.png".into(),
                file: StoredFile::BlobPointer {
                    hash: fake_hash.clone(),
                    size: 100,
                    mime: Some("image/png".into()),
                },
            }],
            "broken",
        )
        .await
        .unwrap();

    let cfg = make_config(&state.data_dir);
    let out_dir = tempfile::tempdir().unwrap();
    let result = materialize::run(&cfg, "v6", out_dir.path(), None);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("blob file missing"),
        "unexpected error: {err_msg}"
    );
}

#[tokio::test]
async fn materialize_removes_output_after_mid_walk_error() {
    let (state, tmp) = setup_state().await;
    let store = Git2VaultStore::new(state.default_vault_root());

    store.ensure_repo("v7").await.unwrap();

    let fake_hash = "b".repeat(64);
    store
        .commit_changes(
            "v7",
            None,
            &[
                FileChange::Upsert {
                    path: "00-written-first.md".into(),
                    file: StoredFile::Text {
                        bytes: b"partial output".to_vec(),
                    },
                },
                FileChange::Upsert {
                    path: "missing.png".into(),
                    file: StoredFile::BlobPointer {
                        hash: fake_hash,
                        size: 100,
                        mime: Some("image/png".into()),
                    },
                },
            ],
            "broken mixed",
        )
        .await
        .unwrap();

    let cfg = make_config(&state.data_dir);
    let out_path = tmp.path().join("materialized-output");
    let result = materialize::run(&cfg, "v7", &out_path, None);

    assert!(result.is_err());
    assert!(
        !out_path.exists(),
        "failed materialize must not leave a half-written output directory"
    );
}
