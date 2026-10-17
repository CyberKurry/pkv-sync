use crate::api::error::ApiError;
use crate::db::repos::{BlobRefRepo, BlobUploadRepo};
use crate::service::vault;
use crate::service::AppState;
use crate::storage::blob::BlobStore;
use bytes::Bytes;

use super::{blob_store, UploadCheckResp};

const MAX_UPLOAD_CHECK_HASHES: usize = 10_000;

/// How many blob-existence probes run at once. Each probe is one stat;
/// the cap keeps a 10k-hash request from spawning 10k tasks while still
/// overlapping the round trips of a network-mounted blob directory.
const BLOB_PROBE_CONCURRENCY: usize = 32;

/// Probe which of `hashes` exist in `store`, with bounded concurrency.
/// Errors count as "not present": re-uploading is safe and self-healing.
async fn probe_many(
    store: &crate::storage::blob::LocalFsBlobStore,
    hashes: &[&String],
) -> std::collections::HashSet<String> {
    use crate::storage::blob::BlobStore;
    let mut present = std::collections::HashSet::with_capacity(hashes.len());
    for chunk in hashes.chunks(BLOB_PROBE_CONCURRENCY) {
        let mut set = tokio::task::JoinSet::new();
        for hash in chunk {
            let store = store.clone();
            let hash = (*hash).clone();
            set.spawn(async move {
                let found = store.has(&hash).await.unwrap_or(false);
                (hash, found)
            });
        }
        while let Some(joined) = set.join_next().await {
            if let Ok((hash, true)) = joined {
                present.insert(hash);
            }
        }
    }
    present
}

pub async fn upload_check(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    hashes: Vec<String>,
) -> Result<UploadCheckResp, ApiError> {
    let _vault = vault::ensure_user_vault(state, user_id, vault_id).await?;
    if hashes.len() > MAX_UPLOAD_CHECK_HASHES {
        return Err(ApiError::bad_request(
            "too_many_blob_hashes",
            format!("upload check exceeds limit of {MAX_UPLOAD_CHECK_HASHES} hashes"),
        ));
    }
    for h in &hashes {
        if !crate::storage::blob::is_sha256_hex(h) {
            return Err(ApiError::bad_request("invalid_hash", "invalid hash"));
        }
    }
    let refs = state
        .blob_refs
        .referenced_hashes_for_vault(vault_id, &hashes)
        .await?;
    let uploads = state
        .blob_uploads
        .uploaded_hashes_for_vault(vault_id, &hashes)
        .await?;
    // K4: the metadata DB can outlive the blob directory (partial restores,
    // manual cleanup). Trusting the DB alone told the client "nothing
    // missing", so it never re-uploaded and every push retried the same
    // batch forever on missing_blob. A blob is only genuinely available
    // when BOTH the vault's records and the store agree; an I/O error
    // counts as missing (re-uploading is safe and self-healing).
    let store = blob_store(state);
    let candidates: Vec<&String> = hashes
        .iter()
        .filter(|h| refs.contains(*h) || uploads.contains(*h))
        .collect();
    let present = probe_many(&store, &candidates).await;
    // R7R2-2: the probes run with bounded concurrency — over a
    // network-mounted blob directory the serial version paid one round
    // trip per hash. Response order still follows the request.
    let missing = hashes
        .into_iter()
        .filter(|h| !present.contains(h))
        .collect();
    Ok(UploadCheckResp { missing })
}

pub async fn upload_blob(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    hash: &str,
    body: Bytes,
) -> Result<(), ApiError> {
    let _vault = vault::ensure_user_vault(state, user_id, vault_id).await?;
    if !crate::storage::blob::is_sha256_hex(hash) {
        return Err(ApiError::bad_request("invalid_hash", "invalid hash"));
    }
    let max_file_size = state.runtime_cfg.max_file_size().await;
    if body.len() as u64 > max_file_size {
        return Err(ApiError::bad_request(
            "file_too_large",
            format!("file exceeds max_file_size of {max_file_size} bytes"),
        ));
    }
    let _storage_guard = crate::service::acquire_storage_mutation_guard(state).await?;
    let store = blob_store(state);
    store
        .put_verified(hash, body)
        .await
        .map_err(|e| ApiError::bad_request("blob_upload_failed", e.to_string()))?;
    state
        .blob_uploads
        .record_upload(vault_id, hash, chrono::Utc::now().timestamp())
        .await?;
    tracing::info!(
        user_id = %user_id,
        vault_id = %vault_id,
        blob_hash = %hash,
        "blob uploaded"
    );
    Ok(())
}

pub async fn download_blob(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    hash: &str,
) -> Result<Option<Bytes>, ApiError> {
    let _vault = vault::ensure_user_vault(state, user_id, vault_id).await?;
    if !state
        .blob_refs
        .is_referenced_by_vault(vault_id, hash)
        .await?
    {
        return Err(ApiError::not_found("blob not found"));
    }
    let store = blob_store(state);
    store
        .get(hash)
        .await
        .map_err(|e| ApiError::bad_request("invalid_hash", e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::RuntimeConfigRepo;
    use crate::service::sync::tests::state_user_vault;
    use crate::storage::blob::LocalFsBlobStore;

    #[tokio::test]
    async fn upload_check_reports_missing_and_existing_blobs() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        upload_blob(&state, &user.user_id, &vid, &hash, data)
            .await
            .unwrap();
        let missing_hash = "0".repeat(64);

        let resp = upload_check(
            &state,
            &user.user_id,
            &vid,
            vec![hash, missing_hash.clone()],
        )
        .await
        .unwrap();

        assert_eq!(resp.missing, vec![missing_hash]);
    }

    #[tokio::test]
    async fn upload_check_reports_blob_missing_when_only_uploaded_to_another_vault() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let other = vault::create_vault(&state, &user.user_id, "other")
            .await
            .unwrap();
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        upload_blob(&state, &user.user_id, &other.id, &hash, data)
            .await
            .unwrap();

        let resp = upload_check(&state, &user.user_id, &vid, vec![hash.clone()])
            .await
            .unwrap();

        assert_eq!(resp.missing, vec![hash]);
    }

    #[tokio::test]
    async fn upload_check_reports_blob_missing_when_only_referenced_in_another_vault() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let other = vault::create_vault(&state, &user.user_id, "other")
            .await
            .unwrap();
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        let store = LocalFsBlobStore::new(state.default_blob_root());
        store.put_verified(&hash, data).await.unwrap();
        state
            .blob_refs
            .add_refs(&other.id, "commit-other", std::slice::from_ref(&hash))
            .await
            .unwrap();

        let resp = upload_check(&state, &user.user_id, &vid, vec![hash.clone()])
            .await
            .unwrap();

        assert_eq!(resp.missing, vec![hash]);
    }

    #[tokio::test]
    async fn upload_check_reports_upload_row_without_blob_file_as_missing() {
        // K4a: this used to trust the vault's upload row alone, so a
        // metadata DB that outlived the blob directory told the client
        // "nothing missing" — the client never re-uploaded and every push
        // retried the same batch forever on missing_blob. A row without
        // the file is not availability.
        let (state, user, vid, _tmp) = state_user_vault().await;
        let hash = "1".repeat(64);
        state
            .blob_uploads
            .record_upload(&vid, &hash, chrono::Utc::now().timestamp())
            .await
            .unwrap();

        let resp = upload_check(&state, &user.user_id, &vid, vec![hash.clone()])
            .await
            .unwrap();

        assert_eq!(resp.missing, vec![hash]);
    }

    #[tokio::test]
    async fn upload_check_preserves_request_order_across_parallel_probes() {
        // R7R2-2: the probes now run with bounded concurrency. The answer
        // must stay identical to the serial version — same membership AND
        // the same order as the request (the client pairs them by index).
        let (state, user, vid, _tmp) = state_user_vault().await;
        let store = LocalFsBlobStore::new(state.default_blob_root());
        let mut hashes = Vec::new();
        for i in 0..40u8 {
            let data = Bytes::from(vec![i]);
            let hash = LocalFsBlobStore::sha256(&data);
            store.put_verified(&hash, data).await.unwrap();
            state
                .blob_uploads
                .record_upload(&vid, &hash, chrono::Utc::now().timestamp())
                .await
                .unwrap();
            hashes.push(hash);
        }
        // Remove every third file from disk; its row survives.
        let mut expected_missing = Vec::new();
        for (idx, hash) in hashes.iter().enumerate() {
            if idx % 3 == 0 {
                store.delete(hash).await.unwrap();
                expected_missing.push(hash.clone());
            }
        }

        let resp = upload_check(&state, &user.user_id, &vid, hashes)
            .await
            .unwrap();

        assert_eq!(resp.missing, expected_missing);
    }

    #[tokio::test]
    async fn upload_check_rejects_too_many_hashes() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let hashes = vec!["0".repeat(64); 10_001];

        let err = upload_check(&state, &user.user_id, &vid, hashes)
            .await
            .unwrap_err();

        assert_eq!(err.code, "too_many_blob_hashes");
    }

    #[tokio::test]
    async fn upload_blob_rejects_runtime_oversize_body() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        state
            .runtime_cfg_repo
            .set_max_file_size(1024, None)
            .await
            .unwrap();
        let cfg = state.runtime_cfg_repo.load().await.unwrap();
        state.runtime_cfg.replace(cfg).await;

        let data = Bytes::from(vec![b'x'; 1025]);
        let hash = LocalFsBlobStore::sha256(&data);
        let err = upload_blob(&state, &user.user_id, &vid, &hash, data)
            .await
            .unwrap_err();

        assert_eq!(err.code, "file_too_large");
    }

    #[tokio::test]
    async fn upload_blob_rejects_non_sha256_hash_before_recording_upload() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let invalid_hash = "not-a-sha256";

        let err = upload_blob(
            &state,
            &user.user_id,
            &vid,
            invalid_hash,
            Bytes::from_static(b"hello"),
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "invalid_hash");
        assert!(state
            .blob_uploads
            .uploaded_hashes_for_vault(&vid, &[invalid_hash.to_string()])
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn download_blob_requires_blob_ref_for_vault() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        upload_blob(&state, &user.user_id, &vid, &hash, data.clone())
            .await
            .unwrap();

        let err = download_blob(&state, &user.user_id, &vid, &hash)
            .await
            .unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::NOT_FOUND);
        assert_eq!(err.message, "blob not found");
        state
            .blob_refs
            .add_refs(&vid, "commit1", std::slice::from_ref(&hash))
            .await
            .unwrap();
        assert_eq!(
            download_blob(&state, &user.user_id, &vid, &hash)
                .await
                .unwrap()
                .unwrap(),
            data
        );
    }
}
