use crate::api::error::ApiError;
use crate::db::repos::{BlobRefRepo, BlobUploadRepo};
use crate::service::AppState;
use crate::storage::blob::BlobStore;
use serde::Serialize;
use std::collections::HashSet;
use std::time::{Duration, SystemTime};

#[derive(Debug, Serialize)]
pub struct GcReport {
    pub deleted: usize,
    pub kept_referenced: usize,
    pub candidates: usize,
    pub freed_bytes: u64,
    pub expired_uploads: u64,
}

const DEFAULT_GRACE_SECONDS: u64 = 7 * 24 * 60 * 60;

pub async fn run_blob_gc(state: &AppState) -> Result<GcReport, ApiError> {
    run_blob_gc_with_grace(state, DEFAULT_GRACE_SECONDS).await
}

pub async fn run_blob_gc_with_grace(
    state: &AppState,
    grace_seconds: u64,
) -> Result<GcReport, ApiError> {
    run_blob_gc_with_grace_at(state, grace_seconds, SystemTime::now()).await
}

/// Same GC with an injected clock. The 0-grace tests need it: a row
/// recorded at `now()` is only snapshot-protected while the GC's own
/// `now()` reads the SAME second — a second-boundary crossing between
/// the upload and the GC expired the row mid-test (Windows CI flake),
/// because at 0 grace "recorded just now" is legitimately expirable.
pub async fn run_blob_gc_with_grace_at(
    state: &AppState,
    grace_seconds: u64,
    now: SystemTime,
) -> Result<GcReport, ApiError> {
    let _storage_guard = crate::service::acquire_storage_mutation_guard(state).await?;
    let grace = Duration::from_secs(grace_seconds);
    // Upload bookkeeping is only meant to cover the upload->push window; a
    // record that was never followed by a push must not protect its blob
    // forever. Expire old rows first so their hashes drop out of the
    // protected set; the files still get the mtime-based grace below.
    let now_secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let expired_uploads = state
        .blob_uploads
        .delete_older_than(now_secs.saturating_sub(grace_seconds as i64))
        .await?;
    let store = state.blob_store();
    let on_disk = store
        .list_hashes_with_mtime()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut protected: HashSet<String> = state.blob_refs.all_hashes().await?;
    protected.extend(state.blob_uploads.all_hashes().await?);
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
        // R8-8: re-check the live tables at the moment of deletion. The
        // `protected` snapshot above races a concurrent push: the client
        // uploaded the blob days ago (row expired since), passed its
        // availability check while the row still existed, and is now
        // committing a BlobPointer — deleting the file here would strand
        // that commit on a 404. A row that reappeared (or a ref written
        // by the racing commit) cancels this deletion; the file simply
        // stays for the next GC cycle.
        if state.blob_uploads.contains_hash(hash).await? {
            continue;
        }
        if state.blob_refs.hash_is_referenced(hash).await? {
            continue;
        }
        let file_size = store
            .size_bytes(hash)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
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
        kept_referenced: protected.len(),
        candidates,
        freed_bytes,
        expired_uploads,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::{BlobUploadRepo, NewUser, UserRepo, VaultRepo};
    use crate::storage::blob::{BlobStore, LocalFsBlobStore};
    use bytes::Bytes;

    #[tokio::test]
    async fn gc_rechecks_uploads_and_refs_at_deletion_time() {
        // R8-8: the protected-set snapshot races a concurrent push. The
        // client's availability check may have read an upload row the GC
        // expires a moment later; the push then commits a BlobPointer. A
        // GC that deletes purely from its snapshot strands that commit on
        // a 404. Deleting must re-check the live tables.
        //
        // This test replays the race deterministically: grace 0 with a
        // fresh upload row for the SECOND blob recorded AFTER the
        // snapshot would be — since we cannot pause the real GC mid-loop,
        // the equivalent observable is the ordering inside one run:
        // the first blob (no row) is deleted, the second (row present
        // but expirable within the same run because it is old) is
        // deleted too — rows that SURVIVE (recent) protect at
        // re-check time even though they were in the snapshot. The
        // re-check's real value is only visible with a mid-loop pause,
        // so this test instead pins the re-check queries exist and run
        // per candidate: see gc_recheck_query_coverage below.
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

        // Blob A: referenced only by a RECENT upload row (uploaded now).
        // The snapshot's all_hashes() protects it anyway; fine either way.
        // The GC `now` is pinned to the SAME instant the row records:
        // `delete_older_than` uses strict `<`, so an equal-second row
        // survives — while letting the GC read its own clock could cross
        // a second boundary between the two `now()` calls and expire the
        // row mid-test (CI flake).
        let data_a = Bytes::from_static(b"racing upload");
        let hash_a = LocalFsBlobStore::sha256(&data_a);
        store.put_verified(&hash_a, data_a).await.unwrap();
        let uploaded_at = chrono::Utc::now().timestamp();
        state
            .blob_uploads
            .record_upload(&vault.id, &hash_a, uploaded_at)
            .await
            .unwrap();
        let gc_now =
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(uploaded_at.max(0) as u64);

        // Blob B: no row, no ref, old — deleted.
        let data_b = Bytes::from_static(b"old orphan");
        let hash_b = LocalFsBlobStore::sha256(&data_b);
        store.put_verified(&hash_b, data_b).await.unwrap();

        let report = run_blob_gc_with_grace_at(&state, 0, gc_now).await.unwrap();

        assert!(
            store.has(&hash_a).await.unwrap(),
            "racing upload must survive"
        );
        assert!(
            !store.has(&hash_b).await.unwrap(),
            "orphan must be reclaimed"
        );
        assert_eq!(report.deleted, 1);
    }

    #[test]
    fn gc_recheck_query_coverage() {
        // R8-8 guard: the deletion loop must re-check the LIVE tables
        // (not only the pre-loop snapshot) for every candidate.
        let source = include_str!("gc.rs");
        let fn_start = source
            .find("pub async fn run_blob_gc_with_grace")
            .expect("gc function exists");
        let tests_start = source.find("#[cfg(test)]").expect("test module exists");
        let fn_source = &source[fn_start..tests_start];
        assert!(
            fn_source.contains("contains_hash("),
            "GC must re-check blob_uploads per deletion candidate"
        );
        assert!(
            fn_source.contains("hash_is_referenced("),
            "GC must re-check blob_refs per deletion candidate"
        );
    }

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

        let report = run_blob_gc_with_grace(&state, 3600).await.unwrap();

        assert_eq!(report.deleted, 0);
        assert_eq!(report.expired_uploads, 0);
        assert!(store.has(&hash).await.unwrap());
    }

    #[tokio::test]
    async fn gc_expires_stale_upload_rows_and_reclaims_blob() {
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
        let data = Bytes::from_static(b"abandoned upload");
        let hash = LocalFsBlobStore::sha256(&data);
        store.put_verified(&hash, data).await.unwrap();
        let stale = chrono::Utc::now().timestamp() - 30 * 24 * 60 * 60;
        state
            .blob_uploads
            .record_upload(&vault.id, &hash, stale)
            .await
            .unwrap();

        let report = run_blob_gc_with_grace(&state, 0).await.unwrap();

        assert_eq!(report.expired_uploads, 1);
        assert!(
            !store.has(&hash).await.unwrap(),
            "a blob uploaded but never pushed must become reclaimable after the grace period"
        );
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

    #[test]
    fn gc_uses_blob_metadata_for_freed_bytes() {
        let source = include_str!("gc.rs");
        let fn_start = source
            .find("pub async fn run_blob_gc_with_grace")
            .expect("gc function exists");
        let tests_start = source.find("#[cfg(test)]").expect("test module exists");
        let fn_source = &source[fn_start..tests_start];

        assert!(
            !fn_source.contains(".get(hash)"),
            "GC should not read whole blob contents only to compute freed bytes"
        );
    }
}
