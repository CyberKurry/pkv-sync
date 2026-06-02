use crate::api::error::ApiError;
use crate::db::repos::{BlobRefRepo, BlobUploadRepo};
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
    pub freed_bytes: u64,
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
    let mut protected: HashSet<String> = state.blob_refs.all_hashes().await?;
    protected.extend(state.blob_uploads.all_hashes().await?);
    let now = SystemTime::now();
    let grace = Duration::from_secs(grace_seconds);
    let mut deleted = 0;
    let mut candidates = 0;
    let mut freed_bytes = 0u64;
    for (hash, mtime) in on_disk.iter() {
        if protected.contains(hash) {
            continue;
        }
        candidates += 1;
        let age = now.duration_since(*mtime).unwrap_or(Duration::ZERO);
        if age < grace {
            continue;
        }
        let file_size = store
            .get(hash)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
            .map(|bytes| bytes.len() as u64)
            .unwrap_or(0);
        if store
            .delete(hash)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
        {
            deleted += 1;
            freed_bytes = freed_bytes.saturating_add(file_size);
        }
    }
    state
        .metrics
        .blob_gc_last_run_unix_seconds
        .set(chrono::Utc::now().timestamp());
    if freed_bytes > 0 {
        state
            .metrics
            .blob_gc_freed_bytes_total
            .with_label_values(&["gc"])
            .inc_by(freed_bytes);
    }
    Ok(GcReport {
        deleted,
        candidates,
        kept_referenced: protected.len(),
        freed_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::{BlobRefRepo, BlobUploadRepo, NewUser, UserRepo, VaultRepo};
    use crate::storage::blob::{BlobStore, LocalFsBlobStore};
    use bytes::Bytes;

    #[tokio::test]
    async fn gc_does_not_delete_blob_referenced_by_old_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = crate::db::pool::connect(&tmp.path().join("metadata.db"))
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
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
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
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
    async fn gc_keeps_uploaded_blob_before_push_records_refs() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = crate::db::pool::connect(&tmp.path().join("metadata.db"))
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
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
        let data = Bytes::from_static(b"pending upload");
        let hash = LocalFsBlobStore::sha256(&data);
        store.put_verified(&hash, data).await.unwrap();
        state
            .blob_uploads
            .record_upload(&vault.id, &hash, chrono::Utc::now().timestamp())
            .await
            .unwrap();

        let report = run_blob_gc_with_grace(&state, 0).await.unwrap();

        assert_eq!(report.deleted, 0);
        assert!(store.has(&hash).await.unwrap());
    }

    #[tokio::test]
    async fn gc_deletes_old_orphan_past_grace_period() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = crate::db::pool::connect(&tmp.path().join("metadata.db"))
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap();
        let store = LocalFsBlobStore::new(state.default_blob_root());
        let data = Bytes::from_static(b"old orphan");
        let hash = LocalFsBlobStore::sha256(&data);
        store.put_verified(&hash, data).await.unwrap();

        let report = run_blob_gc_with_grace(&state, 0).await.unwrap();
        assert_eq!(report.deleted, 1);
        assert_eq!(report.freed_bytes, b"old orphan".len() as u64);
        assert!(!store.has(&hash).await.unwrap());
    }
}
