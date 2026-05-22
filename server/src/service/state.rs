use crate::db::repos::{
    RuntimeConfigCache, RuntimeConfigRepo, SqliteBlobRefRepo, SqliteBlobUploadRepo,
    SqliteIdempotencyRepo, SqliteInviteRepo, SqliteRuntimeConfigRepo, SqliteSyncActivityRepo,
    SqliteTokenRepo, SqliteUserRepo, SqliteVaultRepo, SqliteVaultSettingsRepo,
};
use crate::service::events::VaultEventBus;
use crate::service::metrics::Metrics;
use dashmap::DashMap;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

type VaultPushLocks = Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>;
const DEFAULT_SSE_PER_USER_LIMIT: usize = 16;
const DEFAULT_SSE_GLOBAL_CEILING: usize = 1024;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    /// Root directory for on-disk state. Plan C extends this with vault/blob helpers.
    pub data_dir: std::path::PathBuf,
    pub users: Arc<SqliteUserRepo>,
    pub tokens: Arc<SqliteTokenRepo>,
    pub invites: Arc<SqliteInviteRepo>,
    pub vaults: Arc<SqliteVaultRepo>,
    pub vault_settings: Arc<SqliteVaultSettingsRepo>,
    pub blob_refs: Arc<SqliteBlobRefRepo>,
    pub blob_uploads: Arc<SqliteBlobUploadRepo>,
    pub idempotency: Arc<SqliteIdempotencyRepo>,
    pub activities: Arc<SqliteSyncActivityRepo>,
    pub runtime_cfg_repo: Arc<SqliteRuntimeConfigRepo>,
    pub runtime_cfg: RuntimeConfigCache,
    /// Default server name override from config.toml, used as fallback.
    pub default_server_name: String,
    pub events: VaultEventBus,
    pub metrics: Arc<Metrics>,
    pub mcp_write_limiter: crate::auth::McpWriteRateLimiter,
    pub git_available: bool,
    sse_per_user_limit: Arc<AtomicUsize>,
    sse_global_ceiling: Arc<AtomicUsize>,
    sse_per_user_counts: Arc<DashMap<String, AtomicUsize>>,
    sse_global_count: Arc<AtomicUsize>,
    push_locks: VaultPushLocks,
}

impl AppState {
    pub async fn new(
        pool: SqlitePool,
        data_dir: std::path::PathBuf,
        default_server_name: String,
        git_available: bool,
    ) -> Result<Self, sqlx::Error> {
        let users = Arc::new(SqliteUserRepo::new(pool.clone()));
        let tokens = Arc::new(SqliteTokenRepo::new(pool.clone()));
        let invites = Arc::new(SqliteInviteRepo::new(pool.clone()));
        let vaults = Arc::new(SqliteVaultRepo::new(pool.clone()));
        let vault_settings = Arc::new(SqliteVaultSettingsRepo::new(pool.clone()));
        let blob_refs = Arc::new(SqliteBlobRefRepo::new(pool.clone()));
        let blob_uploads = Arc::new(SqliteBlobUploadRepo::new(pool.clone()));
        let idempotency = Arc::new(SqliteIdempotencyRepo::new(pool.clone()));
        let activities = Arc::new(SqliteSyncActivityRepo::new(pool.clone()));
        let runtime_cfg_repo = Arc::new(SqliteRuntimeConfigRepo::new(pool.clone()));
        let mut cfg = runtime_cfg_repo.load().await?;
        if cfg.server_name == "PKV Sync" && !default_server_name.is_empty() {
            cfg.server_name = default_server_name.clone();
        }
        let runtime_cfg = RuntimeConfigCache::new(cfg);
        let state = Self {
            pool,
            data_dir,
            users,
            tokens,
            invites,
            vaults,
            vault_settings,
            blob_refs,
            blob_uploads,
            idempotency,
            activities,
            runtime_cfg_repo,
            runtime_cfg,
            default_server_name,
            events: VaultEventBus::new(64),
            metrics: Metrics::new(),
            mcp_write_limiter: crate::auth::McpWriteRateLimiter::new(
                60,
                std::time::Duration::from_secs(60),
            ),
            git_available,
            sse_per_user_limit: Arc::new(AtomicUsize::new(DEFAULT_SSE_PER_USER_LIMIT)),
            sse_global_ceiling: Arc::new(AtomicUsize::new(DEFAULT_SSE_GLOBAL_CEILING)),
            sse_per_user_counts: Arc::new(DashMap::new()),
            sse_global_count: Arc::new(AtomicUsize::new(0)),
            push_locks: Arc::new(Mutex::new(HashMap::new())),
        };
        state.spawn_metrics_refresh_task();
        Ok(state)
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

    pub async fn remove_vault_push_lock(&self, vault_id: &str) {
        self.push_locks.lock().await.remove(vault_id);
    }

    pub fn set_sse_per_user_limit_for_tests(&self, limit: usize) {
        self.sse_per_user_limit
            .store(limit.max(1), Ordering::Release);
    }

    pub fn try_acquire_sse_subscriber(&self, user_id: &str) -> Option<SseSubscriberGuard> {
        let user_id = user_id.to_string();
        let per_user_limit = self.sse_per_user_limit.load(Ordering::Acquire).max(1);
        let global_ceiling = self.sse_global_ceiling.load(Ordering::Acquire).max(1);
        {
            let entry = self
                .sse_per_user_counts
                .entry(user_id.clone())
                .or_insert_with(|| AtomicUsize::new(0));
            loop {
                let current = entry.load(Ordering::Acquire);
                if current >= per_user_limit {
                    return None;
                }
                if entry
                    .compare_exchange_weak(
                        current,
                        current + 1,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    break;
                }
            }
        }
        loop {
            let current = self.sse_global_count.load(Ordering::Acquire);
            if current >= global_ceiling {
                if let Some(count) = self.sse_per_user_counts.get(&user_id) {
                    count.fetch_sub(1, Ordering::AcqRel);
                }
                return None;
            }
            if self
                .sse_global_count
                .compare_exchange_weak(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
        self.metrics.sse_subscribers.inc();
        Some(SseSubscriberGuard {
            user_id,
            per_user_counts: self.sse_per_user_counts.clone(),
            global_count: self.sse_global_count.clone(),
            metrics: self.metrics.clone(),
        })
    }

    pub async fn refresh_metrics_gauges(&self) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        let (active_tokens,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM tokens WHERE revoked_at IS NULL AND expires_at > ?",
        )
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        let (vaults_total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM vaults")
            .fetch_one(&self.pool)
            .await?;
        let (blobs_total,): (i64,) =
            sqlx::query_as("SELECT COUNT(DISTINCT blob_hash) FROM blob_refs")
                .fetch_one(&self.pool)
                .await?;
        self.metrics.active_tokens.set(active_tokens);
        self.metrics.vaults_total.set(vaults_total);
        self.metrics.blobs_total.set(blobs_total);
        let vault_root = self.default_vault_root();
        match tokio::task::spawn_blocking(move || directory_size_bytes(&vault_root)).await {
            Ok(Ok(bytes)) => self.metrics.git_repo_size_bytes.set(bytes as f64),
            Ok(Err(err)) => tracing::debug!(error = %err, "failed to refresh git repo size metric"),
            Err(err) => tracing::debug!(error = %err, "git repo size metric task failed"),
        }
        Ok(())
    }

    fn spawn_metrics_refresh_task(&self) {
        let state = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                if let Err(err) = state.refresh_metrics_gauges().await {
                    tracing::debug!(error = %err, "failed to refresh metrics gauges");
                }
                interval.tick().await;
            }
        });
    }

    #[cfg(test)]
    pub async fn vault_push_lock_count_for_tests(&self) -> usize {
        self.push_locks.lock().await.len()
    }
}

pub struct SseSubscriberGuard {
    user_id: String,
    per_user_counts: Arc<DashMap<String, AtomicUsize>>,
    global_count: Arc<AtomicUsize>,
    metrics: Arc<Metrics>,
}

impl Drop for SseSubscriberGuard {
    fn drop(&mut self) {
        if let Some(count) = self.per_user_counts.get(&self.user_id) {
            count.fetch_sub(1, Ordering::AcqRel);
        }
        self.global_count.fetch_sub(1, Ordering::AcqRel);
        self.metrics.sse_subscribers.dec();
    }
}

fn directory_size_bytes(root: &std::path::Path) -> std::io::Result<u64> {
    if !root.exists() {
        return Ok(0);
    }
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry?;
        if entry.file_type().is_file() {
            total = total.saturating_add(entry.metadata()?.len());
        }
    }
    Ok(total)
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
        let state = AppState::new(p, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap();

        let _ = state.vaults.list_for_user("missing").await.unwrap();
        let _ = state.blob_refs.all_hashes().await.unwrap();
        assert!(state
            .idempotency
            .get("missing", "missing", "missing", "push")
            .await
            .unwrap()
            .is_none());
        assert_eq!(state.default_blob_root(), tmp.path().join("blobs"));
        assert_eq!(state.default_vault_root(), tmp.path().join("vaults"));
    }
}
