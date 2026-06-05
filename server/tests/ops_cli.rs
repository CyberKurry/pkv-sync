//! Integration tests for backup/restore/verify operational CLI helpers.

use pkv_sync_server::cli::{backup, restore, verify};
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{BlobRefRepo, NewUser, UserRepo, VaultRepo};
use pkv_sync_server::service::AppState;
use pkv_sync_server::storage::blob::{BlobStore, LocalFsBlobStore};
use pkv_sync_server::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};

use bytes::Bytes;
use ipnet::IpNet;
use serde_json::Value;
use std::path::Path;

#[cfg(unix)]
fn symlink_file_for_test(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink_file_for_test(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(unix)]
fn symlink_dir_for_test(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink_dir_for_test(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

async fn setup_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();

    let db_path = data_dir.join("metadata.db");
    let db = pool::connect(&db_path).await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();

    let state = AppState::new(db, data_dir, "test".into(), true)
        .await
        .unwrap();
    (state, tmp)
}

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
async fn backup_writes_manifest_and_copies_components_without_config_by_default() {
    let (state, tmp) = setup_state().await;
    let cfg = make_config(&state.data_dir);
    std::fs::write(
        tmp.path().join("config.toml"),
        toml::to_string(&cfg).unwrap(),
    )
    .unwrap();

    let blobs = LocalFsBlobStore::new(state.default_blob_root());
    let blob = Bytes::from_static(b"binary attachment");
    let hash = LocalFsBlobStore::sha256(&blob);
    blobs.put_verified(&hash, blob).await.unwrap();

    let git = Git2VaultStore::new(state.default_vault_root());
    git.commit_changes(
        "vault1",
        None,
        &[FileChange::Upsert {
            path: "note.md".into(),
            file: StoredFile::Text {
                bytes: b"hello".to_vec(),
            },
        }],
        "initial",
    )
    .await
    .unwrap();

    let out = tmp.path().join("backup");
    backup::run(&cfg, None, &out, false).unwrap();

    assert!(out.join("metadata.db").exists());
    assert!(out.join("vaults").join("vault1").join("HEAD").exists());
    assert!(out
        .join("blobs")
        .join(&hash[0..2])
        .join(&hash[2..4])
        .join(&hash)
        .exists());
    assert!(!out.join("config.toml").exists());

    let manifest: Value =
        serde_json::from_slice(&std::fs::read(out.join("MANIFEST.json")).unwrap()).unwrap();
    assert_eq!(manifest["manifest_schema"], 1);
    assert_eq!(manifest["pkvsyncd_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        manifest["source_data_dir"].as_str(),
        Some(state.data_dir.to_string_lossy().as_ref())
    );
    assert_eq!(manifest["components"]["metadata.db"]["count"], 1);
    assert!(
        manifest["components"]["vaults"]["count"].as_u64().unwrap() > 0,
        "{manifest}"
    );
    assert_eq!(manifest["components"]["blobs"]["count"], 1);
    assert!(manifest["components"].get("config.toml").is_none());
}

#[tokio::test]
async fn backup_includes_config_when_requested() {
    let (state, tmp) = setup_state().await;
    let cfg = make_config(&state.data_dir);
    let config_path = tmp.path().join("config.toml");
    std::fs::write(&config_path, toml::to_string(&cfg).unwrap()).unwrap();

    let out = tmp.path().join("backup-with-config");
    backup::run(&cfg, Some(&config_path), &out, false).unwrap();

    assert!(out.join("config.toml").exists());
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(out.join("MANIFEST.json")).unwrap()).unwrap();
    assert_eq!(manifest["components"]["config.toml"]["count"], 1);
}

#[tokio::test]
async fn backup_rejects_non_empty_output_dir() {
    let (state, tmp) = setup_state().await;
    let cfg = make_config(&state.data_dir);
    let out = tmp.path().join("backup");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("existing"), b"x").unwrap();

    let err = backup::run(&cfg, None, &out, false).unwrap_err();
    assert!(err.to_string().contains("not empty"), "{err}");
}

#[tokio::test]
async fn backup_handles_output_paths_with_quotes() {
    let (state, tmp) = setup_state().await;
    let cfg = make_config(&state.data_dir);
    let out = tmp.path().join("backup 'quoted'");

    backup::run(&cfg, None, &out, false).unwrap();

    assert!(out.join("metadata.db").exists());
    assert!(out.join("MANIFEST.json").exists());
}

#[tokio::test]
async fn backup_gzip_writes_archive() {
    let (state, tmp) = setup_state().await;
    let cfg = make_config(&state.data_dir);
    let out = tmp.path().join("backup.tar.gz");

    backup::run(&cfg, None, &out, true).unwrap();

    assert!(out.exists());
    assert!(std::fs::metadata(&out).unwrap().len() > 0);
}

#[test]
fn copy_dir_if_exists_skips_symlinked_files() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    let dst = tmp.path().join("dst");
    let secret = tmp.path().join("secret.txt");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("regular.txt"), b"regular").unwrap();
    std::fs::write(&secret, b"secret").unwrap();

    if symlink_file_for_test(&secret, &src.join("linked-secret.txt")).is_err() {
        return;
    }

    backup::copy_dir_if_exists(&src, &dst).unwrap();

    assert_eq!(std::fs::read(dst.join("regular.txt")).unwrap(), b"regular");
    assert!(!dst.join("linked-secret.txt").exists());
}

#[test]
fn remove_dir_contents_removes_symlink_without_touching_target() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("external");
    let root = tmp.path().join("root");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(target.join("keep.txt"), b"keep").unwrap();

    if symlink_dir_for_test(&target, &root.join("linked-external")).is_err() {
        return;
    }

    backup::remove_dir_contents(&root).unwrap();

    assert!(target.join("keep.txt").exists());
    assert!(!root.join("linked-external").exists());
}

#[tokio::test]
async fn restore_refuses_non_empty_target_without_force() {
    let (state, tmp) = setup_state().await;
    let cfg = make_config(&state.data_dir);
    let backup_dir = tmp.path().join("backup");
    backup::run(&cfg, None, &backup_dir, false).unwrap();

    let target = tmp.path().join("restore-target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("existing"), b"x").unwrap();

    let err = restore::run(&backup_dir, &target, false).unwrap_err();
    assert!(err.to_string().contains("not empty"), "{err}");
}

#[tokio::test]
async fn restore_rebuilds_target_and_verify_reports_clean() {
    let (state, tmp) = setup_state().await;
    let cfg = make_config(&state.data_dir);

    let blobs = LocalFsBlobStore::new(state.default_blob_root());
    let blob = Bytes::from_static(b"restore me");
    let hash = LocalFsBlobStore::sha256(&blob);
    blobs.put_verified(&hash, blob.clone()).await.unwrap();

    let git = Git2VaultStore::new(state.default_vault_root());
    git.commit_changes(
        "vault1",
        None,
        &[FileChange::Upsert {
            path: "img.bin".into(),
            file: StoredFile::BlobPointer {
                hash: hash.clone(),
                size: blob.len() as u64,
                mime: None,
            },
        }],
        "blob",
    )
    .await
    .unwrap();

    let backup_dir = tmp.path().join("backup");
    backup::run(&cfg, None, &backup_dir, false).unwrap();

    let target = tmp.path().join("restore-target");
    let report = restore::run(&backup_dir, &target, false).unwrap();
    assert!(report.verify.is_clean());
    assert!(target.join("metadata.db").exists());
    assert!(target.join("vaults").join("vault1").join("HEAD").exists());
    assert!(target
        .join("blobs")
        .join(&hash[0..2])
        .join(&hash[2..4])
        .join(&hash)
        .exists());
}

#[tokio::test]
async fn restore_rejects_manifest_hash_mismatch() {
    let (state, tmp) = setup_state().await;
    let cfg = make_config(&state.data_dir);
    let backup_dir = tmp.path().join("backup");
    backup::run(&cfg, None, &backup_dir, false).unwrap();
    std::fs::write(backup_dir.join("metadata.db"), b"tampered").unwrap();

    let target = tmp.path().join("restore-target");
    let err = restore::run(&backup_dir, &target, false).unwrap_err();
    assert!(err.to_string().contains("hash mismatch"), "{err}");
}

#[tokio::test]
async fn verify_fails_missing_referenced_blob() {
    let (state, _tmp) = setup_state().await;
    let blob = Bytes::from_static(b"missing");
    let hash = LocalFsBlobStore::sha256(&blob);
    let user = state
        .users
        .create(NewUser {
            username: "missing-user".into(),
            password_hash: "h".into(),
            is_admin: false,
        })
        .await
        .unwrap();
    let vault = state.vaults.create(&user.id, "main").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    git.commit_changes(
        &vault.id,
        None,
        &[FileChange::Upsert {
            path: "img.bin".into(),
            file: StoredFile::BlobPointer {
                hash: hash.clone(),
                size: blob.len() as u64,
                mime: None,
            },
        }],
        "blob",
    )
    .await
    .unwrap();
    state
        .blob_refs
        .add_refs(&vault.id, "commit-with-missing-blob", &[hash])
        .await
        .unwrap();

    let cfg = make_config(&state.data_dir);
    let report = verify::run(&cfg, false).unwrap();
    assert_eq!(report.missing_blobs.len(), 1);
    assert!(report.render().contains("missing files: 1"));
    assert!(report.render().contains("Verdict: verification failed"));
    assert!(!report.should_exit_success(false));
}

#[tokio::test]
async fn verify_reads_references_from_blob_refs_table() {
    let (state, _tmp) = setup_state().await;
    let user = state
        .users
        .create(NewUser {
            username: "alice".into(),
            password_hash: "h".into(),
            is_admin: false,
        })
        .await
        .unwrap();
    let vault = state.vaults.create(&user.id, "main").await.unwrap();

    let blobs = LocalFsBlobStore::new(state.default_blob_root());
    let blob = Bytes::from_static(b"db referenced");
    let hash = LocalFsBlobStore::sha256(&blob);
    blobs.put_verified(&hash, blob).await.unwrap();
    state
        .blob_refs
        .add_refs(&vault.id, "commit-from-db", std::slice::from_ref(&hash))
        .await
        .unwrap();

    let cfg = make_config(&state.data_dir);
    let report = verify::run(&cfg, false).unwrap();
    assert!(report.referenced_blobs.contains(&hash));
    assert!(report.orphan_blobs.is_empty());
    assert!(report.render().contains("references: 1"));
    assert!(report.should_exit_success(false));
}

#[tokio::test]
async fn verify_succeeds_with_orphan_only() {
    let (state, _tmp) = setup_state().await;
    let blobs = LocalFsBlobStore::new(state.default_blob_root());
    let blob = Bytes::from_static(b"orphan");
    let hash = LocalFsBlobStore::sha256(&blob);
    blobs.put_verified(&hash, blob).await.unwrap();

    let cfg = make_config(&state.data_dir);
    let report = verify::run(&cfg, false).unwrap();
    assert_eq!(report.orphan_blobs, vec![hash]);
    assert!(report.render().contains("orphans: 1"));
    assert!(report
        .render()
        .contains("Verdict: 1 orphan blob(s) found; not blocking."));
    assert!(report.should_exit_success(false));
}

#[tokio::test]
async fn verify_no_fail_overrides_corrupt_blob_failure() {
    let (state, _tmp) = setup_state().await;
    let blobs = LocalFsBlobStore::new(state.default_blob_root());
    let expected = LocalFsBlobStore::sha256(b"expected");
    blobs
        .put_verified(&expected, Bytes::from_static(b"expected"))
        .await
        .unwrap();
    let blob_path = state
        .default_blob_root()
        .join(&expected[0..2])
        .join(&expected[2..4])
        .join(&expected);
    std::fs::write(blob_path, b"corrupt").unwrap();

    let cfg = make_config(&state.data_dir);
    let report = verify::run(&cfg, true).unwrap();
    assert_eq!(report.corrupt_blobs.len(), 1);
    assert!(!report.should_exit_success(false));
    assert!(report.should_exit_success(true));
}
