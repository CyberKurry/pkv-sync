use crate::api::error::ApiError;
use crate::db::repos::{BlobRefRepo, IdempotencyRepo};
use crate::service::vault;
use crate::service::AppState;
use crate::storage::blob::{BlobStore, LocalFsBlobStore};
use crate::storage::git::{FileChange, Git2VaultStore, GitStoreError, GitVaultStore, StoredFile};
use crate::storage::path;
use crate::storage::text_kind::TextClassifier;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const IDEMPOTENCY_ROUTE_PUSH: &str = "push";

#[derive(Debug, Clone, Copy, Default)]
pub struct RequestMetadata<'a> {
    pub client_ip: Option<&'a str>,
    pub user_agent: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
pub struct UploadCheckReq {
    pub blob_hashes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct UploadCheckResp {
    pub missing: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind")]
pub enum PushChange {
    #[serde(rename = "text")]
    Text { path: String, content: String },
    #[serde(rename = "blob")]
    Blob {
        path: String,
        blob_hash: String,
        size: u64,
        mime: Option<String>,
    },
    #[serde(rename = "delete")]
    Delete { path: String },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PushReq {
    pub changes: Vec<PushChange>,
    #[serde(default)]
    pub device_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PushResp {
    pub new_commit: String,
    pub files_changed: usize,
}

#[derive(Debug, Serialize)]
pub struct StateResp {
    pub current_head: Option<String>,
    pub changed_since: bool,
}

#[derive(Debug, Serialize)]
pub struct PullResp {
    pub from: Option<String>,
    pub to: Option<String>,
    pub added: Vec<PullFile>,
    pub modified: Vec<PullFile>,
    pub deleted: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PullFile {
    pub path: String,
    pub file_type: &'static str,
    pub size: u64,
    pub content_inline: Option<String>,
    pub blob_hash: Option<String>,
}

pub fn blob_store(state: &AppState) -> LocalFsBlobStore {
    LocalFsBlobStore::new(state.default_blob_root())
}

pub async fn upload_check(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    hashes: Vec<String>,
) -> Result<UploadCheckResp, ApiError> {
    let _vault = vault::ensure_user_vault(state, user_id, vault_id).await?;
    let store = blob_store(state);
    let mut missing = Vec::new();
    for h in hashes {
        if !store
            .has(&h)
            .await
            .map_err(|e| ApiError::bad_request("invalid_hash", e.to_string()))?
        {
            missing.push(h);
        }
    }
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
    let max_file_size = state.runtime_cfg.snapshot().await.max_file_size;
    if body.len() as u64 > max_file_size {
        return Err(ApiError::bad_request(
            "file_too_large",
            format!("file exceeds max_file_size of {max_file_size} bytes"),
        ));
    }
    let store = blob_store(state);
    store
        .put_verified(hash, body)
        .await
        .map_err(|e| ApiError::bad_request("blob_upload_failed", e.to_string()))?;
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
        return Err(ApiError::not_found("blob not referenced by vault"));
    }
    let store = blob_store(state);
    store
        .get(hash)
        .await
        .map_err(|e| ApiError::bad_request("invalid_hash", e.to_string()))
}

fn git_write_error(e: GitStoreError) -> ApiError {
    let msg = e.to_string();
    if msg.contains("code=Locked")
        || msg.contains(".lock")
        || msg.contains("failed to write reference")
        || msg.contains("path is not a repository")
    {
        ApiError::conflict("head_mismatch", msg)
    } else {
        ApiError::internal(msg)
    }
}

pub async fn push(
    state: &AppState,
    user: &crate::auth::AuthenticatedUser,
    vault_id: &str,
    if_match: Option<&str>,
    idempotency_key: Option<&str>,
    req: PushReq,
) -> Result<PushResp, ApiError> {
    push_with_request_metadata(
        state,
        user,
        vault_id,
        if_match,
        idempotency_key,
        RequestMetadata::default(),
        req,
    )
    .await
}

pub async fn push_with_request_metadata(
    state: &AppState,
    user: &crate::auth::AuthenticatedUser,
    vault_id: &str,
    if_match: Option<&str>,
    idempotency_key: Option<&str>,
    request_metadata: RequestMetadata<'_>,
    req: PushReq,
) -> Result<PushResp, ApiError> {
    let _vault = vault::ensure_user_vault(state, &user.user_id, vault_id).await?;
    let push_lock = state.vault_push_lock(vault_id).await;
    let _push_guard = push_lock.lock().await;
    let request_hash = match idempotency_key {
        Some(_) => Some(push_request_hash(if_match, &req)?),
        None => None,
    };

    if let Some(key) = idempotency_key {
        if let Some(cached) = state.idempotency.get(key, &user.user_id).await? {
            if cached.vault_id != vault_id
                || cached.route != IDEMPOTENCY_ROUTE_PUSH
                || Some(cached.request_hash.as_str()) != request_hash.as_deref()
            {
                tracing::warn!(
                    user_id = %user.user_id,
                    vault_id = %vault_id,
                    idempotency_key = %key,
                    "idempotency key reused for a different request"
                );
                return Err(ApiError::conflict(
                    "idempotency_key_reused",
                    "idempotency key was already used for a different request",
                ));
            }
            let resp: PushResp = serde_json::from_str(&cached.response_json)
                .map_err(|e| ApiError::internal(e.to_string()))?;
            tracing::info!(
                user_id = %user.user_id,
                vault_id = %vault_id,
                idempotency_key = %key,
                commit = %resp.new_commit,
                "idempotent push replayed"
            );
            return Ok(resp);
        }
    }

    let git = Git2VaultStore::new(state.default_vault_root());
    git.ensure_repo(vault_id).await.map_err(git_write_error)?;
    let head = git.head(vault_id).await.map_err(git_write_error)?;
    if head.as_deref() != if_match {
        tracing::warn!(
            user_id = %user.user_id,
            vault_id = %vault_id,
            current_head = head.as_deref(),
            if_match,
            "push rejected due to head mismatch"
        );
        return Err(ApiError::conflict(
            "head_mismatch",
            format!("current head is {:?}", head),
        ));
    }

    let runtime_cfg = state.runtime_cfg.snapshot().await;
    let classifier = TextClassifier::new(runtime_cfg.text_extensions.iter().map(|s| s.as_str()));
    let blob_store = blob_store(state);
    let mut git_changes = Vec::new();
    let mut blob_hashes = Vec::new();

    for ch in req.changes {
        match ch {
            PushChange::Text { path, content } => {
                let p = path::normalize(&path)
                    .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
                if content.len() as u64 > runtime_cfg.max_file_size {
                    return Err(ApiError::bad_request(
                        "file_too_large",
                        format!(
                            "file exceeds max_file_size of {} bytes",
                            runtime_cfg.max_file_size
                        ),
                    ));
                }
                if !classifier.is_text_path(&p) {
                    return Err(ApiError::bad_request(
                        "wrong_file_kind",
                        "non-text path sent as text",
                    ));
                }
                git_changes.push(FileChange::Upsert {
                    path: p,
                    file: StoredFile::Text {
                        bytes: content.into_bytes(),
                    },
                });
            }
            PushChange::Blob {
                path,
                blob_hash,
                size,
                mime,
            } => {
                let p = path::normalize(&path)
                    .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
                if size > runtime_cfg.max_file_size {
                    return Err(ApiError::bad_request(
                        "file_too_large",
                        format!(
                            "file exceeds max_file_size of {} bytes",
                            runtime_cfg.max_file_size
                        ),
                    ));
                }
                let blob_bytes = match blob_store
                    .get(&blob_hash)
                    .await
                    .map_err(|e| ApiError::bad_request("invalid_hash", e.to_string()))?
                {
                    Some(bytes) => bytes,
                    None => {
                        return Err(ApiError::bad_request(
                            "missing_blob",
                            format!("blob {blob_hash} not uploaded"),
                        ))
                    }
                };
                let actual_size = blob_bytes.len() as u64;
                if actual_size != size {
                    return Err(ApiError::bad_request(
                        "blob_size_mismatch",
                        format!(
                            "declared size {size} does not match uploaded blob size {actual_size}"
                        ),
                    ));
                }
                blob_hashes.push(blob_hash.clone());
                git_changes.push(FileChange::Upsert {
                    path: p,
                    file: StoredFile::BlobPointer {
                        hash: blob_hash,
                        size,
                        mime,
                    },
                });
            }
            PushChange::Delete { path } => {
                let p = path::normalize(&path)
                    .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
                git_changes.push(FileChange::Delete { path: p });
            }
        }
    }

    let msg = format!(
        "sync: {}\n{} files changed",
        req.device_name.unwrap_or_else(|| user.username.clone()),
        git_changes.len()
    );
    let new_commit = git
        .commit_changes(vault_id, head.as_deref(), &git_changes, &msg)
        .await
        .map_err(git_write_error)?;
    let resp = PushResp {
        new_commit: new_commit.clone(),
        files_changed: git_changes.len(),
    };
    if let Err(err) = record_push_metadata(PushMetadataInput {
        state,
        user,
        vault_id,
        new_commit: &new_commit,
        blob_hashes: &blob_hashes,
        files_changed: git_changes.len(),
        idempotency_key,
        request_hash: request_hash.as_deref(),
        client_ip: request_metadata.client_ip,
        user_agent: request_metadata.user_agent,
        resp: &resp,
    })
    .await
    {
        tracing::error!(
            vault_id = %vault_id,
            commit = %new_commit,
            error = %err.message,
            "push committed to git but metadata transaction failed; attempting repair"
        );
        reconcile_vault_metadata(state, vault_id)
            .await
            .inspect_err(|repair_err| {
                tracing::error!(
                    vault_id = %vault_id,
                    commit = %new_commit,
                    error = %repair_err.message,
                    "metadata repair failed after committed push"
                );
            })?;
        if let (Some(key), Some(hash)) = (idempotency_key, request_hash.as_deref()) {
            if let Err(idem_err) = state
                .idempotency
                .put(
                    key,
                    &user.user_id,
                    vault_id,
                    IDEMPOTENCY_ROUTE_PUSH,
                    hash,
                    &serde_json::to_string(&resp).map_err(|e| ApiError::internal(e.to_string()))?,
                )
                .await
            {
                tracing::warn!(
                    vault_id = %vault_id,
                    commit = %new_commit,
                    error = %idem_err,
                    "idempotency cache write failed after metadata repair"
                );
            }
        }
    }
    tracing::info!(
        user_id = %user.user_id,
        vault_id = %vault_id,
        commit = %new_commit,
        files_changed = resp.files_changed,
        "push completed"
    );
    Ok(resp)
}

fn push_request_hash(if_match: Option<&str>, req: &PushReq) -> Result<String, ApiError> {
    let body = serde_json::json!({
        "if_match": if_match,
        "request": req,
    });
    let bytes = serde_json::to_vec(&body).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

struct PushMetadataInput<'a> {
    state: &'a AppState,
    user: &'a crate::auth::AuthenticatedUser,
    vault_id: &'a str,
    new_commit: &'a str,
    blob_hashes: &'a [String],
    files_changed: usize,
    idempotency_key: Option<&'a str>,
    request_hash: Option<&'a str>,
    client_ip: Option<&'a str>,
    user_agent: Option<&'a str>,
    resp: &'a PushResp,
}

async fn record_push_metadata(input: PushMetadataInput<'_>) -> Result<(), ApiError> {
    let state = input.state;
    let git = Git2VaultStore::new(state.default_vault_root());
    let tree = git
        .list_tree(input.vault_id, Some(input.new_commit))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let (size, file_count) = tree_stats(&tree);
    let now = chrono::Utc::now().timestamp();
    let details = serde_json::json!({ "files_changed": input.files_changed }).to_string();
    let response_json = match input.idempotency_key {
        Some(_) => {
            Some(serde_json::to_string(input.resp).map_err(|e| ApiError::internal(e.to_string()))?)
        }
        None => None,
    };

    let mut tx = state.pool.begin().await.map_err(ApiError::from)?;
    for h in input.blob_hashes {
        sqlx::query(
            "INSERT OR IGNORE INTO blob_refs (blob_hash, vault_id, commit_hash) VALUES (?, ?, ?)",
        )
        .bind(h)
        .bind(input.vault_id)
        .bind(input.new_commit)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;
    }
    sqlx::query("UPDATE vaults SET size_bytes = ?, file_count = ?, last_sync_at = ? WHERE id = ?")
        .bind(size)
        .bind(file_count)
        .bind(now)
        .bind(input.vault_id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;
    sqlx::query(
        "INSERT INTO sync_activity
         (user_id, vault_id, token_id, action, commit_hash, client_ip, user_agent, timestamp, details)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&input.user.user_id)
    .bind(input.vault_id)
    .bind(&input.user.token_id)
    .bind("push")
    .bind(input.new_commit)
    .bind(input.client_ip)
    .bind(input.user_agent)
    .bind(now)
    .bind(&details)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::from)?;
    if let (Some(key), Some(hash), Some(json)) =
        (input.idempotency_key, input.request_hash, response_json)
    {
        sqlx::query(
            "INSERT INTO idempotency_cache
             (user_id, key, vault_id, route, request_hash, response_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&input.user.user_id)
        .bind(key)
        .bind(input.vault_id)
        .bind(IDEMPOTENCY_ROUTE_PUSH)
        .bind(hash)
        .bind(json)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;
    }
    tx.commit().await.map_err(ApiError::from)?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct ReconcileReport {
    pub vault_id: String,
    pub head: Option<String>,
    pub size_bytes: i64,
    pub file_count: i64,
    pub blob_refs: usize,
}

pub async fn reconcile_vault_metadata(
    state: &AppState,
    vault_id: &str,
) -> Result<ReconcileReport, ApiError> {
    let git = Git2VaultStore::new(state.default_vault_root());
    let head = git
        .head(vault_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let (tree, blob_hashes) = match head.as_deref() {
        Some(head) => {
            let tree = git
                .list_tree(vault_id, Some(head))
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            let mut hashes = Vec::new();
            for entry in &tree {
                if !entry.is_blob_pointer {
                    continue;
                }
                match git
                    .read_file(vault_id, &entry.path, Some(head))
                    .await
                    .map_err(|e| ApiError::internal(e.to_string()))?
                {
                    Some(StoredFile::BlobPointer { hash, .. }) => hashes.push(hash),
                    _ => tracing::warn!(
                        vault_id = %vault_id,
                        path = %entry.path,
                        "tree entry marked as blob pointer but file decoded differently during repair"
                    ),
                }
            }
            (tree, hashes)
        }
        None => (Vec::new(), Vec::new()),
    };
    let (size_bytes, file_count) = tree_stats(&tree);
    let now = chrono::Utc::now().timestamp();
    let mut tx = state.pool.begin().await.map_err(ApiError::from)?;
    if let Some(head) = head.as_deref() {
        for hash in &blob_hashes {
            sqlx::query(
                "INSERT OR IGNORE INTO blob_refs (blob_hash, vault_id, commit_hash)
                 VALUES (?, ?, ?)",
            )
            .bind(hash)
            .bind(vault_id)
            .bind(head)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::from)?;
        }
    }
    match head {
        Some(_) => {
            sqlx::query(
                "UPDATE vaults SET size_bytes = ?, file_count = ?, last_sync_at = ? WHERE id = ?",
            )
            .bind(size_bytes)
            .bind(file_count)
            .bind(now)
            .bind(vault_id)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::from)?;
        }
        None => {
            sqlx::query(
                "UPDATE vaults SET size_bytes = ?, file_count = ?, last_sync_at = NULL WHERE id = ?",
            )
            .bind(size_bytes)
            .bind(file_count)
            .bind(vault_id)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::from)?;
        }
    }
    tx.commit().await.map_err(ApiError::from)?;

    Ok(ReconcileReport {
        vault_id: vault_id.into(),
        head,
        size_bytes,
        file_count,
        blob_refs: blob_hashes.len(),
    })
}

fn tree_stats(tree: &[crate::storage::git::TreeEntry]) -> (i64, i64) {
    let size = tree.iter().map(|e| e.size as i64).sum();
    (size, tree.len() as i64)
}

pub async fn state(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    head_since: Option<&str>,
) -> Result<StateResp, ApiError> {
    let _vault = vault::ensure_user_vault(state, user_id, vault_id).await?;
    let git = Git2VaultStore::new(state.default_vault_root());
    let head = git
        .head(vault_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StateResp {
        changed_since: head.as_deref() != head_since,
        current_head: head,
    })
}

pub async fn pull(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    since: Option<&str>,
) -> Result<PullResp, ApiError> {
    let _vault = vault::ensure_user_vault(state, user_id, vault_id).await?;
    let git = Git2VaultStore::new(state.default_vault_root());
    let head = git
        .head(vault_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if head.as_deref() == since {
        return Ok(PullResp {
            from: since.map(str::to_string),
            to: head,
            added: vec![],
            modified: vec![],
            deleted: vec![],
        });
    }

    let Some(h) = head.clone() else {
        return Ok(PullResp {
            from: since.map(str::to_string),
            to: None,
            added: vec![],
            modified: vec![],
            deleted: vec![],
        });
    };
    let current = git
        .list_tree_map(vault_id, Some(&h))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let base = match since {
        Some(s) => git
            .list_tree_map(vault_id, Some(s))
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?,
        None => std::collections::BTreeMap::new(),
    };
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();

    for (path, cur) in &current {
        match base.get(path) {
            None => added.push(file_to_pull(&git, vault_id, path, &h).await?),
            Some(old) if old.git_oid != cur.git_oid => {
                modified.push(file_to_pull(&git, vault_id, path, &h).await?)
            }
            Some(_) => {}
        }
    }
    for path in base.keys() {
        if !current.contains_key(path) {
            deleted.push(path.clone());
        }
    }
    tracing::info!(
        user_id = %user_id,
        vault_id = %vault_id,
        from = since,
        to = head.as_deref(),
        added = added.len(),
        modified = modified.len(),
        deleted = deleted.len(),
        "pull completed"
    );
    Ok(PullResp {
        from: since.map(str::to_string),
        to: head,
        added,
        modified,
        deleted,
    })
}

async fn file_to_pull(
    git: &Git2VaultStore,
    vault_id: &str,
    path: &str,
    head: &str,
) -> Result<PullFile, ApiError> {
    let f = git
        .read_file(vault_id, path, Some(head))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::internal("file disappeared during pull"))?;
    match f {
        StoredFile::Text { bytes } => {
            let content = if bytes.len() <= 64 * 1024 {
                Some(String::from_utf8_lossy(&bytes).to_string())
            } else {
                None
            };
            Ok(PullFile {
                path: path.into(),
                file_type: "text",
                size: bytes.len() as u64,
                content_inline: content,
                blob_hash: None,
            })
        }
        StoredFile::BlobPointer { hash, size, .. } => Ok(PullFile {
            path: path.into(),
            file_type: "blob",
            size,
            content_inline: None,
            blob_hash: Some(hash),
        }),
    }
}

pub async fn read_file(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    path: &str,
    at: Option<&str>,
) -> Result<Option<StoredFile>, ApiError> {
    let _vault = vault::ensure_user_vault(state, user_id, vault_id).await?;
    let p =
        path::normalize(path).map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
    let git = Git2VaultStore::new(state.default_vault_root());
    git.read_file(vault_id, &p, at)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{token, AuthenticatedUser};
    use crate::db::pool;
    use crate::db::repos::{
        BlobRefRepo, NewToken, NewUser, RuntimeConfigRepo, TokenRepo, UserRepo,
    };
    use crate::service::{vault, AppState};
    use crate::storage::blob::LocalFsBlobStore;
    use crate::storage::git::{Git2VaultStore, GitVaultStore, StoredFile};
    use bytes::Bytes;

    #[test]
    fn deserialize_text_change() {
        let v: PushReq = serde_json::from_value(serde_json::json!({
            "changes": [{"kind":"text","path":"note.md","content":"hi"}]
        }))
        .unwrap();
        assert!(matches!(v.changes[0], PushChange::Text { .. }));
    }

    #[test]
    fn deserialize_blob_change() {
        let v: PushReq = serde_json::from_value(serde_json::json!({
            "changes": [{"kind":"blob","path":"img.png","blob_hash":"a","size":1,"mime":"image/png"}]
        }))
        .unwrap();
        assert!(matches!(v.changes[0], PushChange::Blob { .. }));
    }

    async fn state_user_vault() -> (AppState, AuthenticatedUser, String, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let state = AppState::new(p, tmp.path().to_path_buf(), "t".into())
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
        let raw = token::generate();
        let token_row = state
            .tokens
            .create(NewToken {
                user_id: &user.id,
                token_hash: &token::hash(&raw),
                device_name: "d",
            })
            .await
            .unwrap();
        let vault = vault::create_vault(&state, &user.id, "main").await.unwrap();
        let auth = AuthenticatedUser {
            user_id: user.id,
            username: user.username,
            is_admin: false,
            token_id: token_row.id,
        };
        (state, auth, vault.id, tmp)
    }

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
    async fn push_rejects_runtime_oversize_text() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        state
            .runtime_cfg_repo
            .set_max_file_size(1024, None)
            .await
            .unwrap();
        let cfg = state.runtime_cfg_repo.load().await.unwrap();
        state.runtime_cfg.replace(cfg).await;

        let err = push(
            &state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: None,
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "x".repeat(1025),
                }],
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "file_too_large");
    }

    #[tokio::test]
    async fn push_rejects_runtime_oversize_blob_metadata() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        state
            .runtime_cfg_repo
            .set_max_file_size(1024, None)
            .await
            .unwrap();
        let cfg = state.runtime_cfg_repo.load().await.unwrap();
        state.runtime_cfg.replace(cfg).await;
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        let store = LocalFsBlobStore::new(state.default_blob_root());
        store.put_verified(&hash, data).await.unwrap();

        let err = push(
            &state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: None,
                changes: vec![PushChange::Blob {
                    path: "img.png".into(),
                    blob_hash: hash,
                    size: 1025,
                    mime: None,
                }],
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "file_too_large");
    }

    #[tokio::test]
    async fn push_rejects_blob_declared_size_mismatch() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        let store = LocalFsBlobStore::new(state.default_blob_root());
        store.put_verified(&hash, data).await.unwrap();

        let err = push(
            &state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: None,
                changes: vec![PushChange::Blob {
                    path: "img.png".into(),
                    blob_hash: hash,
                    size: 6,
                    mime: None,
                }],
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "blob_size_mismatch");
    }

    #[tokio::test]
    async fn download_blob_requires_blob_ref_for_vault() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        upload_blob(&state, &user.user_id, &vid, &hash, data.clone())
            .await
            .unwrap();

        assert!(download_blob(&state, &user.user_id, &vid, &hash)
            .await
            .is_err());
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

    #[tokio::test]
    async fn push_text_commit_is_idempotent() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        fn same_req() -> PushReq {
            PushReq {
                device_name: Some("test".into()),
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "hello".into(),
                }],
            }
        }
        let resp = push(&state, &user, &vid, None, Some("idem1"), same_req())
            .await
            .unwrap();
        assert_eq!(resp.files_changed, 1);

        let again = push(&state, &user, &vid, None, Some("idem1"), same_req())
            .await
            .unwrap();
        assert_eq!(again.new_commit, resp.new_commit);

        let git = Git2VaultStore::new(state.default_vault_root());
        let got = git.read_file(&vid, "note.md", None).await.unwrap().unwrap();
        assert_eq!(
            got,
            StoredFile::Text {
                bytes: b"hello".to_vec()
            }
        );
    }

    #[tokio::test]
    async fn push_records_request_ip_and_user_agent() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        push_with_request_metadata(
            &state,
            &user,
            &vid,
            None,
            None,
            RequestMetadata {
                client_ip: Some("203.0.113.10"),
                user_agent: Some("PKVSync-Plugin/0.1.0"),
            },
            PushReq {
                device_name: Some("test".into()),
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "hello".into(),
                }],
            },
        )
        .await
        .unwrap();

        let row: (Option<String>, Option<String>) =
            sqlx::query_as("SELECT client_ip, user_agent FROM sync_activity WHERE vault_id = ?")
                .bind(&vid)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(row.0.as_deref(), Some("203.0.113.10"));
        assert_eq!(row.1.as_deref(), Some("PKVSync-Plugin/0.1.0"));
    }

    #[tokio::test]
    async fn push_rejects_idempotency_key_reuse_for_different_request() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let _resp = push(
            &state,
            &user,
            &vid,
            None,
            Some("idem1"),
            PushReq {
                device_name: Some("test".into()),
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "hello".into(),
                }],
            },
        )
        .await
        .unwrap();

        let err = push(
            &state,
            &user,
            &vid,
            None,
            Some("idem1"),
            PushReq {
                device_name: Some("test".into()),
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "changed".into(),
                }],
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.status, axum::http::StatusCode::CONFLICT);
        assert_eq!(err.code, "idempotency_key_reused");
    }

    #[tokio::test]
    async fn push_if_match_mismatch_conflicts() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let err = push(
            &state,
            &user,
            &vid,
            Some("bogus"),
            None,
            PushReq {
                device_name: None,
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "hello".into(),
                }],
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn state_pull_and_read_file_return_current_tree() {
        let (app_state, user, vid, _tmp) = state_user_vault().await;
        let pushed = push(
            &app_state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: None,
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "hello".into(),
                }],
            },
        )
        .await
        .unwrap();

        let st = state(&app_state, &user.user_id, &vid, None).await.unwrap();
        assert_eq!(st.current_head.as_deref(), Some(pushed.new_commit.as_str()));
        assert!(st.changed_since);

        let pulled = pull(&app_state, &user.user_id, &vid, None).await.unwrap();
        assert_eq!(pulled.to.as_deref(), Some(pushed.new_commit.as_str()));
        assert_eq!(pulled.added.len(), 1);
        assert_eq!(pulled.added[0].path, "note.md");
        assert_eq!(pulled.added[0].content_inline.as_deref(), Some("hello"));

        let got = read_file(&app_state, &user.user_id, &vid, "note.md", None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            got,
            StoredFile::Text {
                bytes: b"hello".to_vec()
            }
        );
    }

    #[tokio::test]
    async fn pull_since_reports_added_modified_and_deleted() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let first = push(
            &state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: None,
                changes: vec![
                    PushChange::Text {
                        path: "note.md".into(),
                        content: "v1".into(),
                    },
                    PushChange::Text {
                        path: "old.md".into(),
                        content: "old".into(),
                    },
                ],
            },
        )
        .await
        .unwrap();

        let second = push(
            &state,
            &user,
            &vid,
            Some(&first.new_commit),
            None,
            PushReq {
                device_name: None,
                changes: vec![
                    PushChange::Text {
                        path: "note.md".into(),
                        content: "v2".into(),
                    },
                    PushChange::Text {
                        path: "new.md".into(),
                        content: "new".into(),
                    },
                    PushChange::Delete {
                        path: "old.md".into(),
                    },
                ],
            },
        )
        .await
        .unwrap();

        let pulled = pull(&state, &user.user_id, &vid, Some(&first.new_commit))
            .await
            .unwrap();
        assert_eq!(pulled.to.as_deref(), Some(second.new_commit.as_str()));
        assert_eq!(
            pulled
                .added
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>(),
            vec!["new.md"]
        );
        assert_eq!(
            pulled
                .modified
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>(),
            vec!["note.md"]
        );
        assert_eq!(pulled.deleted, vec!["old.md"]);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::auth::{password, token};
    use crate::db::pool;
    use crate::db::repos::{BlobRefRepo, NewToken, NewUser, TokenRepo, UserRepo, VaultRepo};
    use crate::service::vault;
    use crate::storage::blob::{BlobStore, LocalFsBlobStore};
    use bytes::Bytes;
    use tempfile::TempDir;

    async fn setup() -> (AppState, crate::auth::AuthenticatedUser, String, TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect(&tmp.path().join("metadata.db"))
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into())
            .await
            .unwrap();
        let h = password::hash("passw0rd!!").unwrap();
        let u = state
            .users
            .create(NewUser {
                username: "u".into(),
                password_hash: h,
                is_admin: false,
            })
            .await
            .unwrap();
        let raw = token::generate();
        let tr = state
            .tokens
            .create(NewToken {
                user_id: &u.id,
                token_hash: &token::hash(&raw),
                device_name: "d",
            })
            .await
            .unwrap();
        let user = crate::auth::AuthenticatedUser {
            user_id: u.id.clone(),
            username: u.username.clone(),
            is_admin: false,
            token_id: tr.id,
        };
        let v = vault::create_vault(&state, &u.id, "main").await.unwrap();
        (state, user, v.id, tmp)
    }

    #[tokio::test]
    async fn push_then_pull_text() {
        let (state, user, vid, _tmp) = setup().await;
        let resp = push(
            &state,
            &user,
            &vid,
            None,
            Some("idem1"),
            PushReq {
                device_name: Some("test".into()),
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "hello".into(),
                }],
            },
        )
        .await
        .unwrap();
        assert_eq!(resp.files_changed, 1);
        let pulled = pull(&state, &user.user_id, &vid, None).await.unwrap();
        assert_eq!(pulled.added.len(), 1);
        assert_eq!(pulled.added[0].path, "note.md");
        assert_eq!(pulled.added[0].content_inline.as_deref(), Some("hello"));
    }

    #[tokio::test]
    async fn push_same_idempotency_key_reuses_response() {
        let (state, user, vid, _tmp) = setup().await;
        fn same_req() -> PushReq {
            PushReq {
                device_name: None,
                changes: vec![PushChange::Text {
                    path: "a.md".into(),
                    content: "a".into(),
                }],
            }
        }
        let r1 = push(&state, &user, &vid, None, Some("same"), same_req())
            .await
            .unwrap();
        let r2 = push(&state, &user, &vid, None, Some("same"), same_req())
            .await
            .unwrap();
        assert_eq!(r1.new_commit, r2.new_commit);
    }

    #[tokio::test]
    async fn if_match_mismatch_conflicts() {
        let (state, user, vid, _tmp) = setup().await;
        let _r1 = push(
            &state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: None,
                changes: vec![PushChange::Text {
                    path: "a.md".into(),
                    content: "a".into(),
                }],
            },
        )
        .await
        .unwrap();
        let err = push(
            &state,
            &user,
            &vid,
            Some("bogus"),
            None,
            PushReq {
                device_name: None,
                changes: vec![PushChange::Text {
                    path: "b.md".into(),
                    content: "b".into(),
                }],
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn concurrent_pushes_one_succeeds_one_conflicts() {
        let (state, user, vid, _tmp) = setup().await;
        let s1 = state.clone();
        let s2 = state.clone();
        let u1 = user.clone();
        let u2 = user.clone();
        let v1 = vid.clone();
        let v2 = vid.clone();
        let a = tokio::spawn(async move {
            push(
                &s1,
                &u1,
                &v1,
                None,
                None,
                PushReq {
                    device_name: Some("a".into()),
                    changes: vec![PushChange::Text {
                        path: "a.md".into(),
                        content: "a".into(),
                    }],
                },
            )
            .await
        });
        let b = tokio::spawn(async move {
            push(
                &s2,
                &u2,
                &v2,
                None,
                None,
                PushReq {
                    device_name: Some("b".into()),
                    changes: vec![PushChange::Text {
                        path: "b.md".into(),
                        content: "b".into(),
                    }],
                },
            )
            .await
        });
        let r1 = a.await.unwrap();
        let r2 = b.await.unwrap();
        let successes = [r1.is_ok(), r2.is_ok()].into_iter().filter(|x| *x).count();
        let conflicts = [r1.err(), r2.err()]
            .into_iter()
            .flatten()
            .filter(|e| e.status == axum::http::StatusCode::CONFLICT)
            .count();
        assert_eq!(successes, 1);
        assert_eq!(conflicts, 1);
    }

    #[tokio::test]
    async fn push_updates_vault_stats_and_activity_atomically() {
        let (state, user, vid, _tmp) = setup().await;
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        let store = LocalFsBlobStore::new(state.default_blob_root());
        store.put_verified(&hash, data.clone()).await.unwrap();

        let _resp = push(
            &state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: Some("test".into()),
                changes: vec![
                    PushChange::Text {
                        path: "note.md".into(),
                        content: "hello".into(),
                    },
                    PushChange::Blob {
                        path: "img.png".into(),
                        blob_hash: hash.clone(),
                        size: 5,
                        mime: Some("image/png".into()),
                    },
                ],
            },
        )
        .await
        .unwrap();

        let vault = state.vaults.find_by_id(&vid).await.unwrap().unwrap();
        assert_eq!(vault.file_count, 2);
        assert!(vault.size_bytes > 0);
        assert!(vault.last_sync_at.is_some());

        let (activity_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sync_activity WHERE vault_id = ? AND action = 'push'",
        )
        .bind(&vid)
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(activity_count, 1);

        let has_blob_ref: bool = state
            .blob_refs
            .is_referenced_by_vault(&vid, &hash)
            .await
            .unwrap();
        assert!(has_blob_ref);
    }

    #[tokio::test]
    async fn reconcile_vault_metadata_rebuilds_stats_and_current_blob_refs() {
        let (state, _user, vid, _tmp) = setup().await;
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        let store = LocalFsBlobStore::new(state.default_blob_root());
        store.put_verified(&hash, data).await.unwrap();

        let git = Git2VaultStore::new(state.default_vault_root());
        let commit = git
            .commit_changes(
                &vid,
                None,
                &[
                    FileChange::Upsert {
                        path: "note.md".into(),
                        file: StoredFile::Text {
                            bytes: b"hello".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "img.png".into(),
                        file: StoredFile::BlobPointer {
                            hash: hash.clone(),
                            size: 5,
                            mime: Some("image/png".into()),
                        },
                    },
                ],
                "manual git commit",
            )
            .await
            .unwrap();

        let report = reconcile_vault_metadata(&state, &vid).await.unwrap();

        assert_eq!(report.head.as_deref(), Some(commit.as_str()));
        assert_eq!(report.file_count, 2);
        assert_eq!(report.size_bytes, 10);
        let vault = state.vaults.find_by_id(&vid).await.unwrap().unwrap();
        assert_eq!(vault.file_count, 2);
        assert_eq!(vault.size_bytes, 10);
        assert!(state
            .blob_refs
            .is_referenced_by_vault(&vid, &hash)
            .await
            .unwrap());
    }
}
