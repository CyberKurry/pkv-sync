use crate::api::error::ApiError;
use crate::db::repos::BlobRefRepo;
use crate::service::AppState;
use crate::storage::blob::{BlobStore, LocalFsBlobStore};
use serde::Serialize;
use std::collections::HashSet;
use std::time::{Duration, SystemTime};

#[derive(Debug, Serialize)]
pub struct GcReport {
    pub deleted: usize,
    pub kept_referenced: usize,
    pub candidates: usize,
}

const DEFAULT_GRACE_SECONDS: u64 = 7 * 24 * 60 * 60;

pub async fn run_blob_gc(state: &AppState) -> Result<GcReport, ApiError> {
    run_blob_gc_with_grace(state, DEFAULT_GRACE_SECONDS).await
}

pub async fn run_blob_gc_with_grace(
    state: &AppState,
    grace_seconds: u64,
) -> Result<GcReport, ApiError> {
    let store = LocalFsBlobStore::new(state.default_blob_root());
    let on_disk = store
        .list_hashes_with_mtime()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let refs: HashSet<String> = state.blob_refs.all_hashes().await?;
    let now = SystemTime::now();
    let grace = Duration::from_secs(grace_seconds);
    let mut deleted = 0;
    let mut candidates = 0;
    for (hash, mtime) in on_disk.iter() {
        if refs.contains(hash) {
            continue;
        }
        candidates += 1;
        let age = now.duration_since(*mtime).unwrap_or(Duration::ZERO);
        if age < grace {
            continue;
        }
        if store
            .delete(hash)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
        {
            deleted += 1;
        }
    }
    Ok(GcReport {
        deleted,
        candidates,
        kept_referenced: refs.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::{BlobRefRepo, NewUser, UserRepo, VaultRepo};
    use crate::storage::blob::{BlobStore, LocalFsBlobStore};
    use bytes::Bytes;

    #[tokio::test]
    async fn gc_does_not_delete_blob_referenced_by_old_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = crate::db::pool::connect(&tmp.path().join("metadata.db"))
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into())
            .await
            .unwrap();
        let user = state
            .users
            .create(NewUser {
                username: "u".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let vault = state.vaults.create(&user.id, "main").await.unwrap();
        let store = LocalFsBlobStore::new(state.default_blob_root());
        let data = Bytes::from_static(b"old attachment");
        let hash = LocalFsBlobStore::sha256(&data);
        store.put_verified(&hash, data).await.unwrap();
        state
            .blob_refs
            .add_refs(&vault.id, "oldcommit", std::slice::from_ref(&hash))
            .await
            .unwrap();
        let report = run_blob_gc(&state).await.unwrap();
        assert_eq!(report.deleted, 0);
        assert!(store.has(&hash).await.unwrap());
    }

    #[tokio::test]
    async fn gc_keeps_recent_orphan_within_grace_period() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = crate::db::pool::connect(&tmp.path().join("metadata.db"))
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into())
            .await
            .unwrap();
        let store = LocalFsBlobStore::new(state.default_blob_root());
        let data = Bytes::from_static(b"recent orphan");
        let hash = LocalFsBlobStore::sha256(&data);
        store.put_verified(&hash, data).await.unwrap();

        let report = run_blob_gc(&state).await.unwrap();
        assert_eq!(report.deleted, 0);
        assert_eq!(report.candidates, 1);
        assert!(store.has(&hash).await.unwrap());
    }

    #[tokio::test]
    async fn gc_deletes_old_orphan_past_grace_period() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = crate::db::pool::connect(&tmp.path().join("metadata.db"))
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into())
            .await
            .unwrap();
        let store = LocalFsBlobStore::new(state.default_blob_root());
        let data = Bytes::from_static(b"old orphan");
        let hash = LocalFsBlobStore::sha256(&data);
        store.put_verified(&hash, data).await.unwrap();

        let report = run_blob_gc_with_grace(&state, 0).await.unwrap();
        assert_eq!(report.deleted, 1);
        assert!(!store.has(&hash).await.unwrap());
    }
}
