//! Integration tests for backup/restore/verify operational CLI helpers.

use pkv_sync_server::cli::{backup, restore, verify};
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewUser, UserRepo, VaultRepo};
use pkv_sync_server::service::{sync, AppState};
use pkv_sync_server::storage::blob::{BlobStore, LocalFsBlobStore};
use pkv_sync_server::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};
use pkv_sync_server::storage::lock::{acquire_shared_storage_lock, acquire_storage_write_lock};

use bytes::Bytes;
use ipnet::IpNet;
use serde_json::Value;
use std::path::Path;

const TEST_CONFIG_TOML: &str = "# copied by backup tests\n";

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

fn compose_service<'a>(compose: &'a str, service: &str) -> &'a str {
    let needle = format!("  {service}:");
    let start = compose.find(&needle).expect("service exists");
    let after_start = &compose[start + needle.len()..];
    let end = after_start
        .lines()
        .scan(start + needle.len(), |offset, line| {
            let line_start = *offset + 1;
            *offset += line.len() + 1;
            Some((line_start, line))
        })
        .find_map(|(line_start, line)| {
            (line.starts_with("  ") && !line.starts_with("    ")).then_some(line_start)
        })
        .unwrap_or(compose.len());
    &compose[start..end]
}

#[test]
fn traefik_compose_uses_socket_proxy_instead_of_direct_docker_socket() {
    let compose = include_str!("../../deploy/traefik/docker-compose.traefik.yml");
    let traefik = compose_service(compose, "traefik");
    let socket_proxy = compose_service(compose, "socket-proxy");

    assert!(traefik.contains("--providers.docker.endpoint=tcp://socket-proxy:2375"));
    assert!(!traefik.contains("/var/run/docker.sock"));
    assert!(socket_proxy.contains("tecnativa/docker-socket-proxy"));
    assert!(socket_proxy.contains("/var/run/docker.sock:/var/run/docker.sock:ro"));
}

#[test]
fn docker_images_have_runtime_healthchecks() {
    for (name, dockerfile) in [
        ("Dockerfile", include_str!("../../Dockerfile")),
        (
            "Dockerfile.release",
            include_str!("../../Dockerfile.release"),
        ),
    ] {
        assert!(
            dockerfile.contains("HEALTHCHECK"),
            "{name} missing HEALTHCHECK"
        );
        assert!(
            dockerfile.contains("http://127.0.0.1:6710/api/health"),
            "{name} healthcheck should hit the local health endpoint"
        );
    }
}

#[test]
fn caddy_proxy_sets_security_headers_and_body_limit() {
    let caddyfile = include_str!("../../deploy/caddy/Caddyfile");

    for directive in [
        "request_body",
        "max_size 110MB",
        "Strict-Transport-Security",
        "Content-Security-Policy",
        "X-Content-Type-Options",
        "X-Frame-Options",
        "Referrer-Policy",
        "Permissions-Policy",
    ] {
        assert!(caddyfile.contains(directive), "missing {directive}");
    }
}

#[test]
fn compose_examples_avoid_latest_and_floating_proxy_tags() {
    for (name, compose) in [
        (
            "docker-compose.yml",
            include_str!("../../docker-compose.yml"),
        ),
        (
            "compose.updater.yml",
            include_str!("../../deploy/updater/compose.updater.yml"),
        ),
        (
            "docker-compose.traefik.yml",
            include_str!("../../deploy/traefik/docker-compose.traefik.yml"),
        ),
    ] {
        assert!(!compose.contains(":latest"), "{name} uses :latest");
    }

    let root_compose = include_str!("../../docker-compose.yml");
    assert!(root_compose.contains("ghcr.io/cyberkurry/pkv-sync:${PKV_SYNC_TAG:-1.4.1}"));
    assert!(root_compose.contains("caddy:2.10.2"));

    let updater = include_str!("../../deploy/updater/compose.updater.yml");
    assert!(updater.contains("tecnativa/docker-socket-proxy:0.4.2"));
    assert!(updater.contains("docker:27.5.1-cli"));

    let traefik = include_str!("../../deploy/traefik/docker-compose.traefik.yml");
    assert!(traefik.contains("tecnativa/docker-socket-proxy:0.4.2"));
    assert!(traefik.contains("traefik:v3.0.4"));
    assert!(traefik.contains("ghcr.io/cyberkurry/pkv-sync:${PKV_SYNC_TAG:-1.4.1}"));
}

#[test]
fn high_audit_updater_systemd_unit_has_root_service_hardening() {
    let unit = include_str!("../../deploy/updater/pkv-sync-updater.service");

    for directive in [
        "NoNewPrivileges=true",
        "PrivateTmp=true",
        "ProtectHome=true",
        "ProtectSystem=full",
        "ReadWritePaths=/usr/local/bin /var/lib/pkv-sync",
    ] {
        assert!(unit.contains(directive), "missing {directive}");
    }
}

#[test]
fn high_audit_docker_updater_invokes_compose_without_shell_word_splitting() {
    let script = include_str!("../../deploy/updater/docker-updater.sh");

    assert!(script.contains("compose() {"));
    assert!(
        script.contains("docker compose -f \"$COMPOSE_FILE\" -f \"$COMPOSE_UPDATER_FILE\" \"$@\"")
    );
    assert!(!script.contains("$COMPOSE pull"));
    assert!(!script.contains("$COMPOSE up"));
}

#[test]
fn high_audit_backup_artifacts_are_written_with_owner_only_permissions() {
    let source = include_str!("../src/cli/backup.rs");

    assert!(source.contains("const PRIVATE_DIR_MODE: u32 = 0o700"));
    assert!(source.contains("const PRIVATE_FILE_MODE: u32 = 0o600"));

    let run_to_dir = source
        .split("fn run_to_dir")
        .nth(1)
        .and_then(|tail| tail.split("pub fn ensure_absent_or_empty").next())
        .expect("run_to_dir body exists");
    assert!(run_to_dir.contains("create_private_dir_all(output)?"));
    assert!(run_to_dir.contains("restrict_private_file(&metadata_out)?"));
    assert!(run_to_dir.contains("copy_private_file(config_path, &output.join(\"config.toml\"))?"));
    assert!(run_to_dir.contains("write_private_file(&manifest_path"));

    let copy_dir = source
        .split("pub fn copy_dir_if_exists")
        .nth(1)
        .and_then(|tail| tail.split("pub fn remove_dir_contents").next())
        .expect("copy_dir_if_exists body exists");
    assert!(copy_dir.contains("create_private_dir_all(&out)?"));
    assert!(copy_dir.contains("copy_private_file(entry.path(), &out)?"));

    let write_tar_gz = source
        .split("fn write_tar_gz")
        .nth(1)
        .and_then(|tail| tail.split("fn vacuum_into").next())
        .expect("write_tar_gz body exists");
    assert!(write_tar_gz.contains("fs::create_dir_all(parent)?"));
    assert!(write_tar_gz.contains("create_private_file(output)?"));
    assert!(write_tar_gz.contains("restrict_private_file(output)?"));

    let vacuum_into = source
        .split("fn vacuum_into")
        .nth(1)
        .expect("vacuum_into body exists");
    assert!(vacuum_into.contains("create_private_dir_all(parent)?"));
    assert!(vacuum_into.contains("restrict_private_file(&output)?"));
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

async fn add_blob_refs(state: &AppState, vault_id: &str, commit_hash: &str, hashes: &[String]) {
    for hash in hashes {
        sqlx::query(
            "INSERT OR IGNORE INTO blob_refs (blob_hash, vault_id, commit_hash) VALUES (?, ?, ?)",
        )
        .bind(hash)
        .bind(vault_id)
        .bind(commit_hash)
        .execute(&state.pool)
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn backup_writes_manifest_and_copies_components_without_config_by_default() {
    let (state, tmp) = setup_state().await;
    let cfg = make_config(&state.data_dir);
    std::fs::write(tmp.path().join("config.toml"), TEST_CONFIG_TOML).unwrap();

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
    std::fs::write(&config_path, TEST_CONFIG_TOML).unwrap();

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

#[tokio::test]
async fn backup_takes_exclusive_storage_lock_across_snapshot() {
    let (state, tmp) = setup_state().await;
    let cfg = make_config(&state.data_dir);
    let writer_lock = acquire_shared_storage_lock(&state.data_dir).unwrap();
    let backup_cfg = cfg.clone();
    let output = tmp.path().join("backup");

    let backup =
        tokio::task::spawn_blocking(move || backup::run(&backup_cfg, None, &output, false));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(
        !backup.is_finished(),
        "backup should wait for active writers"
    );

    drop(writer_lock);
    backup.await.unwrap().unwrap();
}

#[tokio::test]
async fn storage_writer_waits_while_backup_lock_is_held() {
    let (state, _tmp) = setup_state().await;
    let backup_lock = acquire_storage_write_lock(&state.data_dir).unwrap();
    let writer_data_dir = state.data_dir.clone();

    let writer = tokio::task::spawn_blocking(move || acquire_shared_storage_lock(&writer_data_dir));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!writer.is_finished(), "writer should wait for backup lock");

    drop(backup_lock);
    writer.await.unwrap().unwrap();
}

#[tokio::test]
async fn blob_upload_waits_while_backup_lock_is_held() {
    let (state, _tmp) = setup_state().await;
    let user = state
        .users
        .create(NewUser {
            username: "backup-lock-user".into(),
            password_hash: "h".into(),
            is_admin: false,
        })
        .await
        .unwrap();
    let vault = state.vaults.create(&user.id, "main").await.unwrap();
    let data = Bytes::from_static(b"blob during backup");
    let hash = LocalFsBlobStore::sha256(&data);
    let backup_lock = acquire_storage_write_lock(&state.data_dir).unwrap();
    let write_state = state.clone();
    let user_id = user.id.clone();
    let vault_id = vault.id.clone();

    let writer = tokio::spawn(async move {
        sync::upload_blob(&write_state, &user_id, &vault_id, &hash, data).await
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(
        !writer.is_finished(),
        "blob upload should wait for backup lock"
    );

    drop(backup_lock);
    writer.await.unwrap().unwrap();
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
    add_blob_refs(&state, &vault.id, "commit-with-missing-blob", &[hash]).await;

    let cfg = make_config(&state.data_dir);
    let report = verify::run(&cfg).unwrap();
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
    add_blob_refs(
        &state,
        &vault.id,
        "commit-from-db",
        std::slice::from_ref(&hash),
    )
    .await;

    let cfg = make_config(&state.data_dir);
    let report = verify::run(&cfg).unwrap();
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
    let report = verify::run(&cfg).unwrap();
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
    let report = verify::run(&cfg).unwrap();
    assert_eq!(report.corrupt_blobs.len(), 1);
    assert!(!report.should_exit_success(false));
    assert!(report.should_exit_success(true));
}
