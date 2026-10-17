use crate::cli::backup::{
    component_stats, copy_dir_if_exists, ensure_absent_or_empty, file_component_stats,
    read_manifest, remove_dir_contents, ComponentManifest,
};
use crate::cli::verify::{self, VerifyReport};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct RestoreReport {
    pub verify: VerifyReport,
}

/// V12: does the directory carry any artifact a PKV Sync restore would
/// itself have written? Anything else non-empty is treated as foreign.
fn looks_like_pkv_data_dir(dir: &Path) -> bool {
    [
        "metadata.db",
        "vaults",
        "blobs",
        "config.toml",
        "manifest.json",
    ]
    .iter()
    .any(|name| dir.join(name).exists())
}

pub fn run(
    backup_dir: &Path,
    target_data_dir: &Path,
    force: bool,
) -> anyhow::Result<RestoreReport> {
    let manifest = read_manifest(backup_dir)?;
    if manifest.manifest_schema != 1 {
        anyhow::bail!(
            "unsupported backup manifest version: {}",
            manifest.manifest_schema
        );
    }
    verify_manifest_component(
        "metadata.db",
        manifest.components.get("metadata.db"),
        || file_component_stats(&backup_dir.join("metadata.db")),
    )?;
    verify_manifest_component("vaults", manifest.components.get("vaults"), || {
        component_stats(&backup_dir.join("vaults"))
    })?;
    verify_manifest_component("blobs", manifest.components.get("blobs"), || {
        component_stats(&backup_dir.join("blobs"))
    })?;
    if let Some(expected) = manifest.components.get("config.toml") {
        verify_component(
            "config.toml",
            expected,
            &file_component_stats(&backup_dir.join("config.toml"))?,
        )?;
    }

    if force {
        // V12 hardening: --force clears the target before the restore
        // copies over it, so one mistyped --data-dir must not be able to
        // wipe an unrelated non-empty directory. A directory that looks
        // like a PKV Sync data dir (any of the restored component names)
        // is fair game; anything else non-empty is refused outright —
        // there is no override flag.
        if target_data_dir.exists() && !looks_like_pkv_data_dir(target_data_dir) {
            let non_empty = fs::read_dir(target_data_dir)
                .map(|mut entries| entries.next().is_some())
                .unwrap_or(false);
            if non_empty {
                anyhow::bail!(
                    "refusing to clear {}: it does not look like a PKV Sync data \
                     directory (no metadata.db/vaults/blobs found). Move its contents \
                     or point --data-dir at a PKV Sync data directory",
                    target_data_dir.display()
                );
            }
        }
        fs::create_dir_all(target_data_dir)?;
        remove_dir_contents(target_data_dir)?;
    } else {
        ensure_absent_or_empty(target_data_dir, "target data directory")?;
        fs::create_dir_all(target_data_dir)?;
    }

    fs::copy(
        backup_dir.join("metadata.db"),
        target_data_dir.join("metadata.db"),
    )?;
    copy_dir_if_exists(&backup_dir.join("vaults"), &target_data_dir.join("vaults"))?;
    copy_dir_if_exists(&backup_dir.join("blobs"), &target_data_dir.join("blobs"))?;
    if backup_dir.join("config.toml").exists() {
        fs::copy(
            backup_dir.join("config.toml"),
            target_data_dir.join("config.toml"),
        )?;
    }

    migrate_restored_metadata(&target_data_dir.join("metadata.db"))?;

    let cfg = verify::config_for_data_dir(target_data_dir);
    let verify = verify::run(&cfg)?;
    verify.print();
    if !verify.should_exit_success(false) {
        anyhow::bail!("restore completed but verification failed");
    }
    println!("restore completed into {}", target_data_dir.display());
    Ok(RestoreReport { verify })
}

fn migrate_restored_metadata(db_path: &Path) -> anyhow::Result<()> {
    let db_path = db_path.to_path_buf();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async move {
            let pool = crate::db::pool::connect(&db_path).await?;
            crate::db::pool::migrate_up(&pool).await?;
            pool.close().await;
            Ok::<_, anyhow::Error>(())
        })
    })
    .join()
    .map_err(|_| anyhow::anyhow!("SQLite restore migration task panicked"))?
}

fn verify_manifest_component<F>(
    name: &str,
    expected: Option<&ComponentManifest>,
    actual: F,
) -> anyhow::Result<()>
where
    F: FnOnce() -> anyhow::Result<ComponentManifest>,
{
    let Some(expected) = expected else {
        anyhow::bail!("manifest missing component: {name}");
    };
    verify_component(name, expected, &actual()?)
}

fn verify_component(
    name: &str,
    expected: &ComponentManifest,
    actual: &ComponentManifest,
) -> anyhow::Result<()> {
    if expected.sha256 != actual.sha256 {
        anyhow::bail!(
            "hash mismatch for {name}: expected {}, got {}",
            expected.sha256,
            actual.sha256
        );
    }
    if expected.size != actual.size || expected.count != actual.count {
        anyhow::bail!(
            "manifest mismatch for {name}: expected size={} count={}, got size={} count={}",
            expected.size,
            expected.count,
            actual.size,
            actual.count
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_runs_pending_db_migrations_after_copying_metadata() {
        let source = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();

        runtime.block_on(async {
            let db_path = source.path().join("metadata.db");
            let pool = crate::db::pool::connect(&db_path).await.unwrap();
            crate::db::pool::migrate_up(&pool).await.unwrap();
            sqlx::query("DELETE FROM runtime_config WHERE key LIKE 'update_check.%'")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 2")
                .execute(&pool)
                .await
                .unwrap();
            pool.close().await;
        });

        let cfg = verify::config_for_data_dir(source.path());
        crate::cli::backup::run(&cfg, None, backup.path(), false).unwrap();

        run(backup.path(), target.path(), false).unwrap();

        runtime.block_on(async {
            let pool = crate::db::pool::connect(&target.path().join("metadata.db"))
                .await
                .unwrap();
            let (enabled,): (String,) = sqlx::query_as(
                "SELECT value FROM runtime_config WHERE key = 'update_check.enabled'",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(enabled, "true");
            pool.close().await;
        });
    }
}
