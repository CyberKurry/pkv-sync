use crate::db::repos::{
    RuntimeConfigCache, RuntimeConfigRepo, SqliteBlobRefRepo, SqliteIdempotencyRepo,
    SqliteInviteRepo, SqliteRuntimeConfigRepo, SqliteSyncActivityRepo, SqliteTokenRepo,
    SqliteUserRepo, SqliteVaultRepo,
};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

type VaultPushLocks = Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    /// Root directory for on-disk state. Plan C extends this with vault/blob helpers.
    pub data_dir: std::path::PathBuf,
    pub users: Arc<SqliteUserRepo>,
    pub tokens: Arc<SqliteTokenRepo>,
    pub invites: Arc<SqliteInviteRepo>,
    pub vaults: Arc<SqliteVaultRepo>,
    pub blob_refs: Arc<SqliteBlobRefRepo>,
    pub idempotency: Arc<SqliteIdempotencyRepo>,
    pub activities: Arc<SqliteSyncActivityRepo>,
    pub runtime_cfg_repo: Arc<SqliteRuntimeConfigRepo>,
    pub runtime_cfg: RuntimeConfigCache,
    /// Default server name override from config.toml, used as fallback.
    pub default_server_name: String,
    push_locks: VaultPushLocks,
}

impl AppState {
    pub async fn new(
        pool: SqlitePool,
        data_dir: std::path::PathBuf,
        default_server_name: String,
    ) -> Result<Self, sqlx::Error> {
        let users = Arc::new(SqliteUserRepo::new(pool.clone()));
        let tokens = Arc::new(SqliteTokenRepo::new(pool.clone()));
        let invites = Arc::new(SqliteInviteRepo::new(pool.clone()));
        let vaults = Arc::new(SqliteVaultRepo::new(pool.clone()));
        let blob_refs = Arc::new(SqliteBlobRefRepo::new(pool.clone()));
        let idempotency = Arc::new(SqliteIdempotencyRepo::new(pool.clone()));
        let activities = Arc::new(SqliteSyncActivityRepo::new(pool.clone()));
        let runtime_cfg_repo = Arc::new(SqliteRuntimeConfigRepo::new(pool.clone()));
        let mut cfg = runtime_cfg_repo.load().await?;
        if cfg.server_name == "PKV Sync" && !default_server_name.is_empty() {
            cfg.server_name = default_server_name.clone();
        }
        let runtime_cfg = RuntimeConfigCache::new(cfg);
        Ok(Self {
            pool,
            data_dir,
            users,
            tokens,
            invites,
            vaults,
            blob_refs,
            idempotency,
            activities,
            runtime_cfg_repo,
            runtime_cfg,
            default_server_name,
            push_locks: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn default_blob_root(&self) -> std::path::PathBuf {
        self.data_dir.join("blobs")
    }

    pub fn default_vault_root(&self) -> std::path::PathBuf {
        self.data_dir.join("vaults")
    }

    pub async fn vault_push_lock(&self, vault_id: &str) -> Arc<Mutex<()>> {
        let mut locks = self.push_locks.lock().await;
        locks
            .entry(vault_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::{BlobRefRepo, IdempotencyRepo, VaultRepo};

    #[tokio::test]
    async fn exposes_sync_repos_and_default_storage_roots() {
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let state = AppState::new(p, tmp.path().to_path_buf(), "test".into())
            .await
            .unwrap();

        let _ = state.vaults.list_for_user("missing").await.unwrap();
        let _ = state.blob_refs.all_hashes().await.unwrap();
        assert!(state
            .idempotency
            .get("missing", "missing")
            .await
            .unwrap()
            .is_none());
        assert_eq!(state.default_blob_root(), tmp.path().join("blobs"));
        assert_eq!(state.default_vault_root(), tmp.path().join("vaults"));
    }
}
