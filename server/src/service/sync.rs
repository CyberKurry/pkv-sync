use crate::api::error::ApiError;
use crate::db::repos::{
    BlobRefRepo, BlobUploadRepo, IdempotencyRepo, NewActivity, RuntimeConfig, SyncActivityRepo,
};
use crate::db::SQLITE_SAFE_BIND_LIMIT;
use crate::service::events::{EventChange, EventKind, VaultEvent};
use crate::service::exclude::SyncPathFilter;
use crate::service::merge::MergeOutcome;
use crate::service::AppState;
use crate::service::{vault, vault_settings};
use crate::storage::blob::{BlobStore, LocalFsBlobStore};
use crate::storage::git::{
    FileChange, Git2VaultStore, GitStoreError, GitVaultStore, StoredFile, POINTER_MAGIC_KEY,
    POINTER_VERSION,
};
use crate::storage::path;
use crate::storage::text_kind::TextClassifier;
use bytes::Bytes;
use git2::{Oid, Repository};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{QueryBuilder, Sqlite};
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

const IDEMPOTENCY_ROUTE_PUSH: &str = "push";
const MAX_IDEMPOTENCY_KEY_LEN: usize = 256;
const MAX_PUSH_CHANGES: usize = 1000;
const MAX_PUSH_DEVICE_NAME_BYTES: usize = 4096;
const MAX_SSE_INLINE_PUSH_BYTES: usize = 64 * 1024;
const MAX_UPLOAD_CHECK_HASHES: usize = 10_000;
const MAX_COMMIT_DEVICE_NAME_CHARS: usize = 128;
const MAX_PULL_TREE_ENTRIES: usize = 50_000;
const BLOB_REF_BINDS_PER_ROW: usize = 3;
const BLOB_UPLOAD_DELETE_SHARED_BINDS: usize = 1;
#[cfg(test)]
const VAULT_PUSH_LOCK_TIMEOUT: Duration = Duration::from_millis(50);
#[cfg(not(test))]
const VAULT_PUSH_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, Default)]
pub struct RequestMetadata<'a> {
    pub client_ip: Option<&'a str>,
    pub user_agent: Option<&'a str>,
}

pub async fn record_view(
    state: &AppState,
    user: &crate::auth::AuthenticatedUser,
    vault_id: &str,
    action: &str,
    path: Option<&str>,
    request_metadata: RequestMetadata<'_>,
) -> Result<(), ApiError> {
    let details = path.map(|path| serde_json::json!({ "path": path }).to_string());
    state
        .activities
        .insert(NewActivity {
            user_id: &user.user_id,
            vault_id: Some(vault_id),
            token_id: Some(user.token_id.as_str()),
            action,
            commit_hash: None,
            client_ip: request_metadata.client_ip,
            user_agent: request_metadata.user_agent,
            details: details.as_deref(),
        })
        .await?;
    Ok(())
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

#[derive(Debug, Clone, Serialize)]
pub struct CasConflict {
    pub current_head: Option<String>,
}

#[derive(Clone, Copy)]
enum StalePushMode {
    AllowAutoMerge,
    StrictCas,
}

enum PushApplyOutcome {
    Applied(PushResp),
    Conflict(CasConflict),
}

struct PreparedPush {
    git_changes: Vec<FileChange>,
    blob_hashes: Vec<String>,
    event_changes: Vec<EventChange>,
    text_changes: u64,
    blob_changes: u64,
    delete_changes: u64,
}

struct CommitPushInput<'a> {
    state: &'a AppState,
    user: &'a crate::auth::AuthenticatedUser,
    vault_id: &'a str,
    parent: Option<String>,
    device_name: Option<String>,
    prepared: PreparedPush,
    idempotency_key: Option<&'a str>,
    request_hash: Option<&'a str>,
    request_metadata: RequestMetadata<'a>,
}

struct AutoMergePushInput<'a> {
    state: &'a AppState,
    user: &'a crate::auth::AuthenticatedUser,
    vault_id: &'a str,
    base_commit: &'a str,
    current_head: &'a str,
    req: PushReq,
    runtime_cfg: &'a RuntimeConfig,
    classifier: &'a TextClassifier,
    path_filter: &'a SyncPathFilter,
    idempotency_key: Option<&'a str>,
    request_hash: Option<&'a str>,
    request_metadata: RequestMetadata<'a>,
}

struct PushInternalInput<'a> {
    state: &'a AppState,
    user: &'a crate::auth::AuthenticatedUser,
    vault_id: &'a str,
    if_match: Option<&'a str>,
    idempotency_key: Option<&'a str>,
    request_metadata: RequestMetadata<'a>,
    req: PushReq,
    stale_mode: StalePushMode,
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

pub(crate) fn blob_store(state: &AppState) -> LocalFsBlobStore {
    state.blob_store()
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
    let mut missing = Vec::new();
    for h in hashes {
        let available_to_vault = refs.contains(&h) || uploads.contains(&h);
        if !available_to_vault {
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

fn git_write_error(e: GitStoreError) -> ApiError {
    match e {
        GitStoreError::PathConflict(path) => ApiError::bad_request(
            "path_conflict",
            format!("path conflicts with an existing file or directory: {path}"),
        ),
        GitStoreError::Git(err) => match err.code() {
            git2::ErrorCode::Locked => {
                ApiError::conflict("head_mismatch", "concurrent write in progress")
            }
            git2::ErrorCode::NotFound | git2::ErrorCode::InvalidSpec => {
                ApiError::conflict("head_mismatch", "commit or ref not found")
            }
            _ => ApiError::internal("git operation failed"),
        },
        GitStoreError::Json(_)
        | GitStoreError::Io(_)
        | GitStoreError::NotFound
        | GitStoreError::InvalidVaultId => ApiError::internal("storage operation failed"),
        GitStoreError::Panic => ApiError::internal("storage worker failed"),
    }
}

async fn sync_path_filter(
    state: &AppState,
    vault_id: &str,
    runtime_exclude_globs: &[String],
) -> Result<crate::service::exclude::SyncPathFilter, ApiError> {
    let settings = vault_settings::load(state, vault_id).await?;
    let user_excludes =
        match crate::service::exclude::EffectiveExcludes::compile(runtime_exclude_globs) {
            Ok(set) => set,
            Err(err) => {
                tracing::warn!(
                    vault_id = %vault_id,
                    error = %err,
                    "extra_exclude_globs failed to compile; ignoring all configured exclude globs"
                );
                crate::service::exclude::EffectiveExcludes::compile(&[]).unwrap()
            }
        };
    let vault_allowlist =
        match crate::service::exclude::EffectiveExcludes::compile(&settings.extra_sync_globs) {
            Ok(set) => set,
            Err(err) => {
                tracing::warn!(
                    vault_id = %vault_id,
                    error = %err,
                    "extra_sync_globs failed to compile; ignoring vault allowlist"
                );
                crate::service::exclude::EffectiveExcludes::compile(&[]).unwrap()
            }
        };
    Ok(crate::service::exclude::SyncPathFilter::new(
        user_excludes,
        vault_allowlist,
    ))
}

/// Build the SyncPathFilter for a vault using the current runtime exclude globs.
/// Read surfaces (REST and MCP) use this to hide filter-rejected paths.
pub(crate) async fn vault_path_filter(
    state: &AppState,
    vault_id: &str,
) -> Result<crate::service::exclude::SyncPathFilter, ApiError> {
    let rc = state.runtime_cfg.snapshot().await;
    sync_path_filter(state, vault_id, &rc.extra_exclude_globs).await
}

pub(crate) async fn ensure_path_visible_for_sync_api(
    state: &AppState,
    vault_id: &str,
    path: &str,
) -> Result<String, ApiError> {
    let normalized =
        path::normalize(path).map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
    let rc = state.runtime_cfg.snapshot().await;
    let path_filter = sync_path_filter(state, vault_id, &rc.extra_exclude_globs).await?;
    if path_visible_on_read(&path_filter, &normalized) {
        Ok(normalized)
    } else {
        Err(ApiError::not_found("file not found"))
    }
}

pub(crate) fn path_visible_on_read(
    filter: &crate::service::exclude::SyncPathFilter,
    path: &str,
) -> bool {
    // Read APIs may expose vault-allowlisted hidden paths and generated
    // conflict sidecars. MCP mutating/graph/history tools layer on a stricter
    // hidden-path check so LLM agents cannot address hidden files for actions.
    filter.path_accepts(path) || is_generated_conflict_sidecar(path)
}

fn reject_filtered_push_path(
    filter: &crate::service::exclude::SyncPathFilter,
    path: &str,
) -> Result<(), ApiError> {
    if filter.path_accepts(path) {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "path_excluded",
            format!("path '{}' is excluded by server configuration", path),
        ))
    }
}

fn generated_push_path_is_valid(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.as_bytes().contains(&0) || path.len() > 512
    {
        return false;
    }
    path.split('/').all(|part| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && !part.eq_ignore_ascii_case(".git")
            && !part.contains('\\')
            && part.len() <= 255
    })
}

fn ensure_generated_push_path(path: &str) -> Result<(), ApiError> {
    if generated_push_path_is_valid(path) {
        Ok(())
    } else {
        Err(ApiError::internal("generated path is invalid"))
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
    match push_with_request_metadata_internal(PushInternalInput {
        state,
        user,
        vault_id,
        if_match,
        idempotency_key,
        request_metadata,
        req,
        stale_mode: StalePushMode::AllowAutoMerge,
    })
    .await?
    {
        PushApplyOutcome::Applied(resp) => Ok(resp),
        PushApplyOutcome::Conflict(conflict) => Err(ApiError::conflict(
            "head_mismatch",
            format!("current head is {:?}", conflict.current_head),
        )),
    }
}

pub async fn push_with_cas(
    state: &AppState,
    user: &crate::auth::AuthenticatedUser,
    vault_id: &str,
    parent_commit: Option<&str>,
    request_metadata: RequestMetadata<'_>,
    req: PushReq,
) -> Result<Result<PushResp, CasConflict>, ApiError> {
    match push_with_request_metadata_internal(PushInternalInput {
        state,
        user,
        vault_id,
        if_match: parent_commit,
        idempotency_key: None,
        request_metadata,
        req,
        stale_mode: StalePushMode::StrictCas,
    })
    .await?
    {
        PushApplyOutcome::Applied(resp) => Ok(Ok(resp)),
        PushApplyOutcome::Conflict(conflict) => Ok(Err(conflict)),
    }
}

async fn push_with_request_metadata_internal(
    input: PushInternalInput<'_>,
) -> Result<PushApplyOutcome, ApiError> {
    let PushInternalInput {
        state,
        user,
        vault_id,
        if_match,
        idempotency_key,
        request_metadata,
        req,
        stale_mode,
    } = input;
    if req.changes.len() > MAX_PUSH_CHANGES {
        return Err(ApiError::bad_request(
            "too_many_changes",
            format!("push changes exceed limit of {MAX_PUSH_CHANGES}"),
        ));
    }
    if let Some(device_name) = &req.device_name {
        if device_name.len() > MAX_PUSH_DEVICE_NAME_BYTES {
            return Err(ApiError::bad_request(
                "invalid_device_name",
                format!("device_name exceeds limit of {MAX_PUSH_DEVICE_NAME_BYTES} bytes"),
            ));
        }
        if device_name.chars().any(char::is_control) {
            return Err(ApiError::bad_request(
                "invalid_device_name",
                "device_name cannot contain control characters",
            ));
        }
    }
    if let Some(key) = idempotency_key {
        if key.len() > MAX_IDEMPOTENCY_KEY_LEN {
            return Err(ApiError::bad_request(
                "invalid_idempotency_key",
                format!("idempotency key exceeds limit of {MAX_IDEMPOTENCY_KEY_LEN} bytes"),
            ));
        }
    }
    let _vault = vault::ensure_user_vault(state, &user.user_id, vault_id).await?;
    let _push_guard = acquire_vault_push_lock(state, vault_id).await?;
    // Re-check after acquiring the per-vault lock so a queued push cannot race
    // with vault deletion or ownership changes while it waited.
    let _vault = vault::ensure_user_vault(state, &user.user_id, vault_id).await?;
    let request_hash = match idempotency_key {
        Some(_) => Some(push_request_hash(if_match, &req)?),
        None => None,
    };

    if let Some(key) = idempotency_key {
        if let Some(cached) = state
            .idempotency
            .get(key, &user.user_id, vault_id, IDEMPOTENCY_ROUTE_PUSH)
            .await?
        {
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
            return Ok(PushApplyOutcome::Applied(resp));
        }
    }

    let runtime_cfg = state.runtime_cfg.snapshot().await;
    let classifier = runtime_cfg.text_classifier.clone();
    let path_filter = sync_path_filter(state, vault_id, &runtime_cfg.extra_exclude_globs).await?;
    let git = state.git_store();
    git.ensure_repo(vault_id).await.map_err(git_write_error)?;
    let head = git.head(vault_id).await.map_err(git_write_error)?;
    if head.as_deref() != if_match {
        if matches!(stale_mode, StalePushMode::AllowAutoMerge) && runtime_cfg.enable_auto_merge {
            if let (Some(base_commit), Some(current_head)) = (if_match, head.as_deref()) {
                if let Some(resp) = try_auto_merge_push(AutoMergePushInput {
                    state,
                    user,
                    vault_id,
                    base_commit,
                    current_head,
                    req,
                    runtime_cfg: &runtime_cfg,
                    classifier: classifier.as_ref(),
                    path_filter: &path_filter,
                    idempotency_key,
                    request_hash: request_hash.as_deref(),
                    request_metadata,
                })
                .await?
                {
                    return Ok(PushApplyOutcome::Applied(resp));
                }
            }
        }
        tracing::warn!(
            user_id = %user.user_id,
            vault_id = %vault_id,
            current_head = head.as_deref(),
            if_match,
            "push rejected due to head mismatch"
        );
        return Ok(PushApplyOutcome::Conflict(CasConflict {
            current_head: head,
        }));
    }

    let blob_store = blob_store(state);
    let mut git_changes = Vec::new();
    let mut blob_hashes = Vec::new();
    let blob_candidates: Vec<String> = req
        .changes
        .iter()
        .filter_map(|change| match change {
            PushChange::Blob { blob_hash, .. } => Some(blob_hash.clone()),
            _ => None,
        })
        .collect();
    let mut blob_availability = None;
    let inline_max = runtime_cfg.inline_content_max_bytes as usize;
    let mut inline_budget = SseInlineBudget::new(inline_max);
    let mut event_changes = Vec::new();
    let mut text_changes = 0u64;
    let mut blob_changes = 0u64;
    let mut delete_changes = 0u64;

    for ch in req.changes {
        match ch {
            PushChange::Text { path, content } => {
                text_changes += 1;
                let p = path::normalize(&path)
                    .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
                reject_filtered_push_path(&path_filter, &p)?;
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
                let event_path = p.clone();
                event_changes.push(text_event_with_budget(
                    &event_path,
                    &content,
                    &mut inline_budget,
                ));
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
                blob_changes += 1;
                let p = path::normalize(&path)
                    .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
                reject_filtered_push_path(&path_filter, &p)?;
                if size > runtime_cfg.max_file_size {
                    return Err(ApiError::bad_request(
                        "file_too_large",
                        format!(
                            "file exceeds max_file_size of {} bytes",
                            runtime_cfg.max_file_size
                        ),
                    ));
                }
                if !crate::storage::blob::is_sha256_hex(&blob_hash) {
                    return Err(ApiError::bad_request("invalid_hash", "invalid hash"));
                }
                if blob_availability.is_none() {
                    blob_availability = Some((
                        state
                            .blob_refs
                            .referenced_hashes_for_vault(vault_id, &blob_candidates)
                            .await?,
                        state
                            .blob_uploads
                            .uploaded_hashes_for_vault(vault_id, &blob_candidates)
                            .await?,
                    ));
                }
                let (referenced_blobs, uploaded_blobs) = blob_availability
                    .as_ref()
                    .expect("blob availability is initialized above");
                if !blob_available_to_vault(referenced_blobs, uploaded_blobs, &blob_hash) {
                    return Err(ApiError::bad_request(
                        "missing_blob",
                        format!("blob {blob_hash} not uploaded for this vault"),
                    ));
                }
                let actual_size = match blob_store
                    .size_bytes(&blob_hash)
                    .await
                    .map_err(|e| ApiError::bad_request("invalid_hash", e.to_string()))?
                {
                    Some(size) => size,
                    None => {
                        return Err(ApiError::bad_request(
                            "missing_blob",
                            format!("blob {blob_hash} not uploaded"),
                        ))
                    }
                };
                if actual_size != size {
                    return Err(ApiError::bad_request(
                        "blob_size_mismatch",
                        format!(
                            "declared size {size} does not match uploaded blob size {actual_size}"
                        ),
                    ));
                }
                let event_blob_hash = blob_hash.clone();
                blob_hashes.push(blob_hash.clone());
                event_changes.push(EventChange::Blob {
                    path: p.clone(),
                    blob_hash: event_blob_hash,
                    size,
                });
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
                delete_changes += 1;
                let p = path::normalize(&path)
                    .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
                reject_filtered_push_path(&path_filter, &p)?;
                event_changes.push(EventChange::Delete { path: p.clone() });
                git_changes.push(FileChange::Delete { path: p });
            }
        }
    }

    let resp = commit_prepared_push(CommitPushInput {
        state,
        user,
        vault_id,
        parent: head,
        device_name: req.device_name,
        prepared: PreparedPush {
            git_changes,
            blob_hashes,
            event_changes,
            text_changes,
            blob_changes,
            delete_changes,
        },
        idempotency_key,
        request_hash: request_hash.as_deref(),
        request_metadata,
    })
    .await?;
    Ok(PushApplyOutcome::Applied(resp))
}

async fn commit_prepared_push(input: CommitPushInput<'_>) -> Result<PushResp, ApiError> {
    let state = input.state;
    let user = input.user;
    let vault_id = input.vault_id;
    let prepared = input.prepared;
    let device_name = input.device_name.as_deref().unwrap_or(&user.username);
    let device_name = safe_commit_device_name(device_name);
    let msg = format!(
        "sync: {}\n{} files changed",
        device_name,
        prepared.git_changes.len()
    );
    let git = state.git_store();
    let new_commit = git
        .commit_changes(
            vault_id,
            input.parent.as_deref(),
            &prepared.git_changes,
            &msg,
        )
        .await
        .map_err(git_write_error)?;
    if prepared.text_changes > 0 {
        state
            .metrics
            .push_changes_total
            .with_label_values(&["text"])
            .inc_by(prepared.text_changes);
    }
    if prepared.blob_changes > 0 {
        state
            .metrics
            .push_changes_total
            .with_label_values(&["blob"])
            .inc_by(prepared.blob_changes);
    }
    if prepared.delete_changes > 0 {
        state
            .metrics
            .push_changes_total
            .with_label_values(&["delete"])
            .inc_by(prepared.delete_changes);
    }
    // Publish to SSE subscribers IMMEDIATELY after the commit lands in git, before
    // any of the metadata-side bookkeeping (idempotency cache write, activity log).
    // Plan J explicitly requires this ordering so subscribers do not pay for DB
    // writes in the sub-second latency budget. Failures elsewhere (metadata,
    // idempotency) do not roll back the commit — the broadcast is therefore
    // honest about what state the git tree is in.
    state.events.publish(
        vault_id,
        VaultEvent {
            commit: new_commit.clone(),
            parent: input.parent.clone(),
            source_device_id: user.device_id.clone(),
            at: chrono::Utc::now().timestamp(),
            kind: EventKind::Commit,
            changes: prepared.event_changes,
        },
    );
    let resp = PushResp {
        new_commit: new_commit.clone(),
        files_changed: prepared.git_changes.len(),
    };
    if let Err(err) = record_push_metadata(PushMetadataInput {
        state,
        user,
        vault_id,
        new_commit: &new_commit,
        blob_hashes: &prepared.blob_hashes,
        files_changed: prepared.git_changes.len(),
        idempotency_key: input.idempotency_key,
        request_hash: input.request_hash,
        client_ip: input.request_metadata.client_ip,
        user_agent: input.request_metadata.user_agent,
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
        if let Err(repair_err) = reconcile_vault_metadata_unlocked(state, vault_id).await {
            tracing::error!(
                vault_id = %vault_id,
                commit = %new_commit,
                error = %repair_err.message,
                "metadata repair failed after committed push"
            );
            if let Err(protect_err) =
                protect_committed_blob_refs(state, vault_id, &new_commit, &prepared.blob_hashes)
                    .await
            {
                tracing::error!(
                    vault_id = %vault_id,
                    commit = %new_commit,
                    error = %protect_err.message,
                    "failed to preserve blob references after metadata repair failure"
                );
            }
        }
        if let (Some(key), Some(hash)) = (input.idempotency_key, input.request_hash) {
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

async fn try_auto_merge_push(input: AutoMergePushInput<'_>) -> Result<Option<PushResp>, ApiError> {
    let git = input.state.git_store();
    let inline_max = input.runtime_cfg.inline_content_max_bytes as usize;
    let mut inline_budget = SseInlineBudget::new(inline_max);
    let mut git_changes = Vec::new();
    let mut event_changes = Vec::new();
    let mut clean_merges = 0;
    let mut conflict_merges = 0;
    let PushReq {
        changes,
        device_name,
    } = input.req;
    let conflict_device_name = device_name.as_deref().unwrap_or(&input.user.username);

    for change in changes {
        let PushChange::Text { path, content } = change else {
            return Ok(None);
        };

        let normalized = path::normalize(&path)
            .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
        reject_filtered_push_path(input.path_filter, &normalized)?;
        if content.len() as u64 > input.runtime_cfg.max_file_size {
            return Err(ApiError::bad_request(
                "file_too_large",
                format!(
                    "file exceeds max_file_size of {} bytes",
                    input.runtime_cfg.max_file_size
                ),
            ));
        }
        if !input.classifier.is_text_path(&normalized) {
            return Err(ApiError::bad_request(
                "wrong_file_kind",
                "non-text path sent as text",
            ));
        }

        let Some(base_bytes) =
            read_merge_text(&git, input.vault_id, &normalized, input.base_commit).await?
        else {
            return Ok(None);
        };
        let Some(remote_bytes) =
            read_merge_text(&git, input.vault_id, &normalized, input.current_head).await?
        else {
            return Ok(None);
        };

        match crate::service::merge::three_way_merge_bytes(
            &base_bytes,
            content.as_bytes(),
            &remote_bytes,
        ) {
            MergeOutcome::Clean(merged) => {
                event_changes.push(text_event_with_budget(
                    &normalized,
                    &merged,
                    &mut inline_budget,
                ));
                git_changes.push(FileChange::Upsert {
                    path: normalized,
                    file: StoredFile::Text {
                        bytes: merged.into_bytes(),
                    },
                });
                clean_merges += 1;
            }
            MergeOutcome::Conflicted(marked) => {
                let conflict_path = conflict_path_for(&normalized, conflict_device_name);
                ensure_generated_push_path(&conflict_path)?;
                event_changes.push(text_event_with_budget(
                    &conflict_path,
                    &marked,
                    &mut inline_budget,
                ));
                git_changes.push(FileChange::Upsert {
                    path: conflict_path,
                    file: StoredFile::Text {
                        bytes: marked.into_bytes(),
                    },
                });
                conflict_merges += 1;
            }
            MergeOutcome::Binary => {
                let conflict_path = conflict_path_for(&normalized, conflict_device_name);
                ensure_generated_push_path(&conflict_path)?;
                event_changes.push(text_event_with_budget(
                    &conflict_path,
                    &content,
                    &mut inline_budget,
                ));
                git_changes.push(FileChange::Upsert {
                    path: conflict_path,
                    file: StoredFile::Text {
                        bytes: content.into_bytes(),
                    },
                });
                conflict_merges += 1;
            }
        }
    }

    if git_changes.is_empty() {
        return Ok(None);
    }

    let resp = commit_prepared_push(CommitPushInput {
        state: input.state,
        user: input.user,
        vault_id: input.vault_id,
        parent: Some(input.current_head.to_string()),
        device_name,
        prepared: PreparedPush {
            text_changes: git_changes.len() as u64,
            blob_changes: 0,
            delete_changes: 0,
            git_changes,
            blob_hashes: Vec::new(),
            event_changes,
        },
        idempotency_key: input.idempotency_key,
        request_hash: input.request_hash,
        request_metadata: input.request_metadata,
    })
    .await?;

    if clean_merges > 0 {
        input
            .state
            .metrics
            .auto_merge_clean_total
            .inc_by(clean_merges);
    }
    if conflict_merges > 0 {
        input
            .state
            .metrics
            .auto_merge_conflict_total
            .inc_by(conflict_merges);
    }

    tracing::info!(
        user_id = %input.user.user_id,
        vault_id = %input.vault_id,
        clean_merges,
        conflict_merges,
        "stale push handled by auto-merge"
    );
    Ok(Some(resp))
}

async fn read_merge_text(
    git: &Git2VaultStore,
    vault_id: &str,
    path: &str,
    at: &str,
) -> Result<Option<Vec<u8>>, ApiError> {
    let file = match git.read_file(vault_id, path, Some(at)).await {
        Ok(file) => file,
        Err(GitStoreError::Git(_)) => return Ok(None),
        Err(err) => return Err(ApiError::bad_request("bad_commit", err.to_string())),
    };
    match file {
        Some(StoredFile::Text { bytes }) => Ok(Some(bytes)),
        Some(StoredFile::BlobPointer { .. }) => Ok(None),
        None => Ok(Some(Vec::new())),
    }
}

struct SseInlineBudget {
    per_file_max: usize,
    remaining: usize,
}

impl SseInlineBudget {
    fn new(per_file_max: usize) -> Self {
        Self {
            per_file_max,
            remaining: MAX_SSE_INLINE_PUSH_BYTES,
        }
    }
}

fn text_event_with_budget(path: &str, content: &str, budget: &mut SseInlineBudget) -> EventChange {
    if content.len() <= budget.per_file_max && content.len() <= budget.remaining {
        budget.remaining = budget.remaining.saturating_sub(content.len());
        EventChange::TextInline {
            path: path.to_string(),
            content: content.to_string(),
        }
    } else {
        EventChange::TextRef {
            path: path.to_string(),
            size: content.len() as u64,
        }
    }
}

fn conflict_path_for(original: &str, device_name: &str) -> String {
    let stamp = chrono::Utc::now().format("%Y-%m-%d-%H%M%S");
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let nonce = &nonce[..8];
    let device = safe_conflict_device_name(device_name);
    let slash = original.rfind('/');
    let (dir, file) = match slash {
        Some(idx) => (&original[..=idx], &original[idx + 1..]),
        None => ("", original),
    };
    let candidate = match file.rfind('.') {
        Some(dot) if dot > 0 => format!(
            "{}{}.conflict-{}-{}-{}{}",
            dir,
            &file[..dot],
            stamp,
            nonce,
            device,
            &file[dot..]
        ),
        _ => format!("{dir}{file}.conflict-{stamp}-{nonce}-{device}"),
    };
    if generated_push_path_is_valid(&candidate) {
        return candidate;
    }
    let ext = match file.rfind('.') {
        Some(dot) if dot > 0 && file[dot..].len() <= 16 => &file[dot..],
        _ => ".md",
    };
    let fallback = format!("{dir}conflict.conflict-{stamp}-{nonce}{ext}");
    if generated_push_path_is_valid(&fallback) {
        fallback
    } else {
        format!("conflict.conflict-{stamp}-{nonce}{ext}")
    }
}

/// A server-generated auto-merge conflict sidecar must remain visible on
/// read/pull surfaces even when its name matches a user exclude glob.
pub(crate) fn is_generated_conflict_sidecar(path: &str) -> bool {
    let file = path.rsplit('/').next().unwrap_or(path);
    match file.find(".conflict-") {
        Some(idx) => {
            let after = &file[idx + ".conflict-".len()..];
            after.len() > 4 && after.as_bytes()[..4].iter().all(u8::is_ascii_digit)
        }
        None => false,
    }
}

fn safe_conflict_device_name(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in name.trim().chars() {
        let safe = ch.is_ascii_alphanumeric() || ch == '_' || ch == '-';
        let next = if safe { ch } else { '-' };
        if next == '-' {
            if !last_dash {
                out.push(next);
            }
            last_dash = true;
        } else {
            out.push(next);
            last_dash = false;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "device".to_string()
    } else {
        trimmed.to_string()
    }
}

fn safe_commit_device_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len().min(MAX_COMMIT_DEVICE_NAME_CHARS));
    let mut last_was_space = false;
    let mut char_count = 0;
    for ch in name.chars() {
        if char_count >= MAX_COMMIT_DEVICE_NAME_CHARS {
            break;
        }
        if ch.is_ascii_whitespace() {
            if !out.is_empty() && !last_was_space {
                out.push(' ');
                last_was_space = true;
                char_count += 1;
            }
            continue;
        }
        if !ch.is_ascii_graphic() {
            continue;
        }
        out.push(ch);
        last_was_space = false;
        char_count += 1;
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

fn blob_available_to_vault(
    referenced_blobs: &HashSet<String>,
    uploaded_blobs: &HashSet<String>,
    hash: &str,
) -> bool {
    referenced_blobs.contains(hash) || uploaded_blobs.contains(hash)
}

fn push_request_hash(if_match: Option<&str>, req: &PushReq) -> Result<String, ApiError> {
    let mut hasher = Sha256::new();
    hash_len_prefixed(&mut hasher, "if_match");
    match if_match {
        Some(value) => {
            hasher.update([1]);
            hash_len_prefixed(&mut hasher, value);
        }
        None => hasher.update([0]),
    }
    hash_len_prefixed(&mut hasher, "device_name");
    match &req.device_name {
        Some(value) => {
            hasher.update([1]);
            hash_len_prefixed(&mut hasher, value);
        }
        None => hasher.update([0]),
    }
    hash_len_prefixed(&mut hasher, "changes");
    hasher.update((req.changes.len() as u64).to_be_bytes());
    for change in &req.changes {
        match change {
            PushChange::Text { path, content } => {
                hasher.update([b'T']);
                hash_len_prefixed(&mut hasher, path);
                hash_len_prefixed(&mut hasher, content);
            }
            PushChange::Blob {
                path,
                blob_hash,
                size,
                mime,
            } => {
                hasher.update([b'B']);
                hash_len_prefixed(&mut hasher, path);
                hash_len_prefixed(&mut hasher, blob_hash);
                hasher.update(size.to_be_bytes());
                match mime {
                    Some(value) => {
                        hasher.update([1]);
                        hash_len_prefixed(&mut hasher, value);
                    }
                    None => hasher.update([0]),
                }
            }
            PushChange::Delete { path } => {
                hasher.update([b'D']);
                hash_len_prefixed(&mut hasher, path);
            }
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

fn hash_len_prefixed(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
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
    let git = state.git_store();
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
    insert_blob_refs_in_tx(&mut tx, input.vault_id, input.new_commit, input.blob_hashes)
        .await
        .map_err(ApiError::from)?;
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
    delete_blob_uploads_in_tx(&mut tx, input.vault_id, input.blob_hashes)
        .await
        .map_err(ApiError::from)?;
    tx.commit().await.map_err(ApiError::from)?;
    Ok(())
}

async fn insert_blob_refs_in_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    vault_id: &str,
    commit_hash: &str,
    hashes: &[String],
) -> Result<(), sqlx::Error> {
    if hashes.is_empty() {
        return Ok(());
    }
    let chunk_size = SQLITE_SAFE_BIND_LIMIT / BLOB_REF_BINDS_PER_ROW;
    for chunk in hashes.chunks(chunk_size) {
        let mut query = QueryBuilder::<Sqlite>::new(
            "INSERT OR IGNORE INTO blob_refs (blob_hash, vault_id, commit_hash) ",
        );
        query.push_values(chunk, |mut row, hash| {
            row.push_bind(hash)
                .push_bind(vault_id)
                .push_bind(commit_hash);
        });
        query.build().execute(&mut **tx).await?;
    }
    Ok(())
}

async fn delete_blob_uploads_in_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    vault_id: &str,
    hashes: &[String],
) -> Result<(), sqlx::Error> {
    if hashes.is_empty() {
        return Ok(());
    }
    let chunk_size = SQLITE_SAFE_BIND_LIMIT - BLOB_UPLOAD_DELETE_SHARED_BINDS;
    for chunk in hashes.chunks(chunk_size) {
        let mut query = QueryBuilder::<Sqlite>::new("DELETE FROM blob_uploads WHERE vault_id = ");
        query.push_bind(vault_id);
        query.push(" AND blob_hash IN (");
        let mut separated = query.separated(", ");
        for hash in chunk {
            separated.push_bind(hash);
        }
        separated.push_unseparated(")");
        query.build().execute(&mut **tx).await?;
    }
    Ok(())
}

async fn protect_committed_blob_refs(
    state: &AppState,
    vault_id: &str,
    commit: &str,
    blob_hashes: &[String],
) -> Result<(), ApiError> {
    if blob_hashes.is_empty() {
        return Ok(());
    }
    let mut tx = state.pool.begin().await.map_err(ApiError::from)?;
    insert_blob_refs_in_tx(&mut tx, vault_id, commit, blob_hashes)
        .await
        .map_err(ApiError::from)?;
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
    let push_lock = state.vault_push_lock(vault_id);
    let _push_guard = push_lock.lock().await;
    reconcile_vault_metadata_unlocked(state, vault_id).await
}

pub(crate) async fn reconcile_vault_metadata_unlocked(
    state: &AppState,
    vault_id: &str,
) -> Result<ReconcileReport, ApiError> {
    let git = state.git_store();
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
            let hashes = tree
                .iter()
                .filter_map(|entry| entry.blob_hash.clone())
                .collect();
            (tree, hashes)
        }
        None => (Vec::new(), Vec::new()),
    };
    let (size_bytes, file_count) = tree_stats(&tree);
    let now = chrono::Utc::now().timestamp();
    let mut tx = state.pool.begin().await.map_err(ApiError::from)?;
    sqlx::query("DELETE FROM blob_refs WHERE vault_id = ?")
        .bind(vault_id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;
    if let Some(head) = head.as_deref() {
        insert_blob_refs_in_tx(&mut tx, vault_id, head, &blob_hashes)
            .await
            .map_err(ApiError::from)?;
    }
    let last_sync_at = head.as_ref().map(|_| now);
    sqlx::query("UPDATE vaults SET size_bytes = ?, file_count = ?, last_sync_at = ? WHERE id = ?")
        .bind(size_bytes)
        .bind(file_count)
        .bind(last_sync_at)
        .bind(vault_id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;
    tx.commit().await.map_err(ApiError::from)?;

    Ok(ReconcileReport {
        vault_id: vault_id.into(),
        head,
        size_bytes,
        file_count,
        blob_refs: blob_hashes.len(),
    })
}

async fn acquire_vault_push_lock(
    state: &AppState,
    vault_id: &str,
) -> Result<tokio::sync::OwnedMutexGuard<()>, ApiError> {
    let lock = state.vault_push_lock(vault_id);
    tokio::time::timeout(VAULT_PUSH_LOCK_TIMEOUT, lock.lock_owned())
        .await
        .map_err(|_| {
            ApiError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "vault_busy",
                "vault is busy; retry later",
            )
        })
}

fn tree_stats(tree: &[crate::storage::git::TreeEntry]) -> (i64, i64) {
    let size = tree.iter().fold(0i64, |acc, entry| {
        acc.saturating_add(entry.size.min(i64::MAX as u64) as i64)
    });
    let file_count = tree.len().min(i64::MAX as usize) as i64;
    (size, file_count)
}

pub async fn state(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    head_since: Option<&str>,
) -> Result<StateResp, ApiError> {
    let _vault = vault::ensure_user_vault(state, user_id, vault_id).await?;
    let git = state.git_store();
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
    pull_for_user(
        state,
        user_id,
        None,
        vault_id,
        since,
        RequestMetadata::default(),
        MAX_PULL_TREE_ENTRIES,
    )
    .await
}

pub async fn pull_with_request_metadata(
    state: &AppState,
    user: &crate::auth::AuthenticatedUser,
    vault_id: &str,
    since: Option<&str>,
    request_metadata: RequestMetadata<'_>,
) -> Result<PullResp, ApiError> {
    pull_for_user(
        state,
        &user.user_id,
        Some(&user.token_id),
        vault_id,
        since,
        request_metadata,
        MAX_PULL_TREE_ENTRIES,
    )
    .await
}

async fn pull_for_user(
    state: &AppState,
    user_id: &str,
    token_id: Option<&str>,
    vault_id: &str,
    since: Option<&str>,
    request_metadata: RequestMetadata<'_>,
    max_tree_entries: usize,
) -> Result<PullResp, ApiError> {
    let _vault = vault::ensure_user_vault(state, user_id, vault_id).await?;
    let git = state.git_store();
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
    if current.len() > max_tree_entries || base.len() > max_tree_entries {
        return Err(ApiError::new(
            axum::http::StatusCode::PAYLOAD_TOO_LARGE,
            "pull_too_large",
            format!("vault has too many files for one pull response; limit is {max_tree_entries}"),
        ));
    }
    let rc = state.runtime_cfg.snapshot().await;
    let path_filter = sync_path_filter(state, vault_id, &rc.extra_exclude_globs).await?;
    let mut added_paths = Vec::new();
    let mut modified_paths = Vec::new();
    let mut deleted = Vec::new();

    for (path, cur) in &current {
        if !path_visible_on_read(&path_filter, path) {
            continue;
        }
        match base.get(path) {
            None => added_paths.push(path.clone()),
            Some(old) if old.git_oid != cur.git_oid => {
                modified_paths.push(path.clone());
            }
            Some(_) => {}
        }
    }
    for path in base.keys() {
        if !current.contains_key(path) && path_visible_on_read(&path_filter, path) {
            deleted.push(path.clone());
        }
    }
    let mut pull_paths = Vec::with_capacity(added_paths.len() + modified_paths.len());
    pull_paths.extend(
        added_paths
            .into_iter()
            .map(|path| (PullFileBucket::Added, path)),
    );
    pull_paths.extend(
        modified_paths
            .into_iter()
            .map(|path| (PullFileBucket::Modified, path)),
    );
    let pulled = files_to_pull(state.vault_root(), vault_id, &h, pull_paths).await?;
    let mut added = Vec::new();
    let mut modified = Vec::new();
    for (bucket, file) in pulled {
        match bucket {
            PullFileBucket::Added => added.push(file),
            PullFileBucket::Modified => modified.push(file),
        }
    }
    if !added.is_empty() {
        state
            .metrics
            .pull_files_total
            .with_label_values(&["added"])
            .inc_by(added.len() as u64);
    }
    if !modified.is_empty() {
        state
            .metrics
            .pull_files_total
            .with_label_values(&["modified"])
            .inc_by(modified.len() as u64);
    }
    if !deleted.is_empty() {
        state
            .metrics
            .pull_files_total
            .with_label_values(&["deleted"])
            .inc_by(deleted.len() as u64);
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
    let details = serde_json::json!({
        "added": added.len(),
        "modified": modified.len(),
        "deleted": deleted.len(),
    })
    .to_string();
    state
        .activities
        .insert(NewActivity {
            user_id,
            vault_id: Some(vault_id),
            token_id,
            action: "pull",
            commit_hash: Some(&h),
            client_ip: request_metadata.client_ip,
            user_agent: request_metadata.user_agent,
            details: Some(&details),
        })
        .await?;
    Ok(PullResp {
        from: since.map(str::to_string),
        to: head,
        added,
        modified,
        deleted,
    })
}

#[derive(Clone, Copy)]
enum PullFileBucket {
    Added,
    Modified,
}

async fn files_to_pull(
    vault_root: &Path,
    vault_id: &str,
    head: &str,
    paths: Vec<(PullFileBucket, String)>,
) -> Result<Vec<(PullFileBucket, PullFile)>, ApiError> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let repo_path = vault_root.join(vault_id);
    let head = head.to_string();
    tokio::task::spawn_blocking(
        move || -> Result<Vec<(PullFileBucket, PullFile)>, GitStoreError> {
            let repo = Repository::open_bare(&repo_path)?;
            let oid = Oid::from_str(&head)?;
            let commit = repo.find_commit(oid)?;
            let tree = commit.tree()?;
            let mut files = Vec::with_capacity(paths.len());
            for (bucket, path) in paths {
                let entry = tree
                    .get_path(Path::new(&path))
                    .map_err(|_| GitStoreError::NotFound)?;
                let blob = repo.find_blob(entry.id())?;
                let file = decode_pull_file(&path, blob.content().to_vec());
                let file = match file {
                    StoredFile::Text { bytes } => text_pull_file(&path, bytes),
                    StoredFile::BlobPointer { hash, size, .. } => blob_pull_file(&path, hash, size),
                };
                files.push((bucket, file));
            }
            Ok(files)
        },
    )
    .await
    .map_err(|_| ApiError::internal("pull file read task panicked"))?
    .map_err(|e| {
        if matches!(e, GitStoreError::NotFound) {
            ApiError::internal("file disappeared during pull")
        } else {
            ApiError::internal(e.to_string())
        }
    })
}

fn text_pull_file(path: &str, bytes: Vec<u8>) -> PullFile {
    let content = if bytes.len() <= 64 * 1024 {
        Some(String::from_utf8_lossy(&bytes).to_string())
    } else {
        None
    };
    PullFile {
        path: path.into(),
        file_type: "text",
        size: bytes.len() as u64,
        content_inline: content,
        blob_hash: None,
    }
}

fn blob_pull_file(path: &str, hash: String, size: u64) -> PullFile {
    PullFile {
        path: path.into(),
        file_type: "blob",
        size,
        content_inline: None,
        blob_hash: Some(hash),
    }
}

fn decode_pull_file(path: &str, bytes: Vec<u8>) -> StoredFile {
    if let Some(pointer) = pointer_bytes(&bytes, true) {
        return pointer;
    }
    if !TextClassifier::default_ref().is_text_path(path) {
        if let Some(pointer) = pointer_bytes(&bytes, false) {
            return pointer;
        }
    }
    StoredFile::Text { bytes }
}

fn pointer_bytes(bytes: &[u8], require_magic: bool) -> Option<StoredFile> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if require_magic && value.get(POINTER_MAGIC_KEY)?.as_u64()? != POINTER_VERSION {
        return None;
    }
    let hash = value.get("blob")?.as_str()?.to_string();
    if !crate::storage::blob::is_sha256_hex(&hash) {
        return None;
    }
    let size = value.get("size")?.as_u64()?;
    let mime = value
        .get("mime")
        .and_then(|m| m.as_str())
        .map(str::to_string);
    Some(StoredFile::BlobPointer { hash, size, mime })
}

/// Read a normalized vault path after the caller has enforced read visibility.
///
/// REST and MCP handlers must call `ensure_path_visible_for_sync_api` (or an
/// equivalent read-surface filter) before reaching this helper.
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
    let git = state.git_store();
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
        BlobUploadRepo, NewToken, NewUser, RuntimeConfigRepo, TokenRepo, UserRepo, VaultRepo,
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

    #[test]
    fn push_request_hash_distinguishes_text_content_without_json_serialization() {
        let left = PushReq {
            device_name: Some("device".to_string()),
            changes: vec![PushChange::Text {
                path: "note.md".to_string(),
                content: "left".to_string(),
            }],
        };
        let right = PushReq {
            device_name: Some("device".to_string()),
            changes: vec![PushChange::Text {
                path: "note.md".to_string(),
                content: "right".to_string(),
            }],
        };

        assert_ne!(
            push_request_hash(Some("parent"), &left).unwrap(),
            push_request_hash(Some("parent"), &right).unwrap()
        );

        let source = include_str!("sync.rs");
        let fn_start = source
            .find("fn push_request_hash")
            .expect("push_request_hash implementation exists");
        let next_struct = source[fn_start..]
            .find("\nstruct PushMetadataInput")
            .map(|idx| fn_start + idx)
            .expect("metadata input follows hash helper");
        let implementation = &source[fn_start..next_struct];

        assert!(!implementation.contains("serde_json::json!"));
        assert!(!implementation.contains("serde_json::to_vec"));
    }

    #[test]
    fn push_request_hash_distinguishes_blob_metadata_and_parent() {
        let mut req = PushReq {
            device_name: None,
            changes: vec![PushChange::Blob {
                path: "img.png".to_string(),
                blob_hash: "a".repeat(64),
                size: 10,
                mime: Some("image/png".to_string()),
            }],
        };
        let base = push_request_hash(Some("parent-a"), &req).unwrap();
        assert_ne!(base, push_request_hash(Some("parent-b"), &req).unwrap());

        let PushChange::Blob { mime, .. } = &mut req.changes[0] else {
            panic!("expected blob change");
        };
        *mime = Some("application/octet-stream".to_string());
        assert_ne!(base, push_request_hash(Some("parent-a"), &req).unwrap());
    }

    #[test]
    fn blob_push_size_validation_uses_metadata_not_full_blob_read() {
        let source = include_str!("sync.rs");
        let branch_start = source
            .find("PushChange::Blob")
            .expect("blob push branch exists");
        let delete_start = source[branch_start..]
            .find("PushChange::Delete")
            .map(|idx| branch_start + idx)
            .expect("delete branch follows blob branch");
        let blob_branch = &source[branch_start..delete_start];

        assert!(blob_branch.contains(".size_bytes(&blob_hash)"));
        assert!(!blob_branch.contains(".get(&blob_hash)"));
    }

    #[test]
    fn safe_commit_device_name_tracks_char_count_without_recounting_output() {
        let source = include_str!("sync.rs");
        let fn_start = source
            .find("fn safe_commit_device_name")
            .expect("safe_commit_device_name implementation exists");
        let next_fn = source[fn_start + 1..]
            .find("\nfn ")
            .map(|idx| fn_start + 1 + idx)
            .expect("following function exists");
        let implementation = &source[fn_start..next_fn];

        assert!(implementation.contains("char_count"));
        assert!(!implementation.contains("out.chars().count()"));
    }

    #[test]
    fn safe_commit_device_name_strips_invisible_unicode() {
        let sanitized = safe_commit_device_name("Desk\u{200b}\u{202e}Hidden");

        assert_eq!(sanitized, "DeskHidden");
        assert!(!sanitized.contains('\u{200b}'));
        assert!(!sanitized.contains('\u{202e}'));
    }

    #[test]
    fn git_write_error_hides_storage_paths_in_conflict_responses() {
        let err = GitStoreError::Git(git2::Error::new(
            git2::ErrorCode::Locked,
            git2::ErrorClass::Reference,
            "failed to write reference /var/lib/pkv-sync/vaults/secret/main.lock",
        ));

        let api_err = git_write_error(err);

        assert_eq!(api_err.status, axum::http::StatusCode::CONFLICT);
        assert_eq!(api_err.code, "head_mismatch");
        assert!(!api_err.message.contains("/var/lib/pkv-sync"));
        assert!(!api_err.message.contains("secret"));
        assert!(!api_err.message.contains(".lock"));
    }

    #[test]
    fn conflict_paths_are_unique_even_for_same_second_inputs() {
        let first = conflict_path_for("notes/daily.md", "Laptop");
        let second = conflict_path_for("notes/daily.md", "Laptop");

        assert_ne!(first, second);
        assert!(first.starts_with("notes/daily.conflict-"));
        assert!(first.ends_with(".md"));
    }

    #[test]
    fn conflict_path_for_long_input_stays_within_storage_limits() {
        let original = format!("{}/{}.md", "d".repeat(255), "a".repeat(252));
        assert_eq!(path::normalize(&original).unwrap().len(), 511);

        let conflict = conflict_path_for(&original, &"device".repeat(100));

        assert!(
            conflict.len() <= 512,
            "conflict path should fit storage path limit: {}",
            conflict.len()
        );
        path::normalize(&conflict).expect("generated conflict path should remain valid");
        assert!(conflict.contains(".conflict-"));
        assert!(conflict.ends_with(".md"));
    }

    #[test]
    fn conflict_sidecar_validation_accepts_normalized_percent_literals() {
        for (raw, normalized) in [
            ("note%252E.md", "note%2E.md"),
            ("%252E%252E/foo.md", "%2E%2E/foo.md"),
            ("%252Egit/foo.md", "%2Egit/foo.md"),
        ] {
            let original = path::normalize(raw).unwrap();
            assert_eq!(original, normalized);

            let conflict = conflict_path_for(&original, "Laptop");

            assert!(
                conflict.starts_with(normalized.trim_end_matches(".md"))
                    || conflict.starts_with("%2E%2E/foo.conflict-")
                    || conflict.starts_with("%2Egit/foo.conflict-"),
                "conflict path should preserve normalized literal percent escape near original file: {conflict}"
            );
            assert!(
                !conflict.starts_with("conflict.conflict-"),
                "conflict path should not fall back to vault root for {normalized}: {conflict}"
            );
            ensure_generated_push_path(&conflict)
                .expect("generated sidecar with literal percent escape should remain valid");
        }
    }

    #[test]
    fn generated_push_path_rejects_backslash_parts() {
        assert!(!generated_push_path_is_valid("notes\\daily.md"));
        assert!(!generated_push_path_is_valid("notes/daily\\todo.md"));
    }

    #[test]
    fn generated_conflict_sidecar_detection_is_narrow() {
        assert!(is_generated_conflict_sidecar(
            "notes/daily.conflict-2026-06-09-123456-abcd1234-Laptop.md"
        ));
        assert!(is_generated_conflict_sidecar(
            "conflict.conflict-2026-06-09-123456-abcd1234.md"
        ));
        assert!(!is_generated_conflict_sidecar(
            "notes/daily.conflict-Laptop.md"
        ));
        assert!(!is_generated_conflict_sidecar(
            "notes/conflict-2026-06-09.md"
        ));
    }

    #[test]
    fn tree_stats_saturates_oversized_entries() {
        let entries = vec![
            crate::storage::git::TreeEntry {
                path: "huge.bin".into(),
                git_oid: "0".repeat(40),
                size: i64::MAX as u64,
                is_blob_pointer: true,
                blob_hash: Some("a".repeat(64)),
            },
            crate::storage::git::TreeEntry {
                path: "another.bin".into(),
                git_oid: "1".repeat(40),
                size: 1,
                is_blob_pointer: true,
                blob_hash: Some("b".repeat(64)),
            },
        ];

        assert_eq!(tree_stats(&entries), (i64::MAX, 2));
    }

    #[test]
    fn reconcile_uses_tree_pointer_hashes_without_per_file_git_reads() {
        let source = include_str!("sync.rs");
        let fn_start = source
            .find("pub(crate) async fn reconcile_vault_metadata_unlocked")
            .expect("reconcile implementation exists");
        let next_fn = source[fn_start + 1..]
            .find("\nasync fn acquire_vault_push_lock")
            .map(|idx| fn_start + 1 + idx)
            .expect("following function exists");
        let implementation = &source[fn_start..next_fn];

        assert!(implementation.contains("entry.blob_hash"));
        assert!(!implementation.contains(".read_file("));
    }

    #[test]
    fn blob_ref_metadata_writes_use_batched_helpers() {
        let source = include_str!("sync.rs");
        assert!(source.contains("async fn insert_blob_refs_in_tx"));

        let record_start = source
            .find("async fn record_push_metadata")
            .expect("record_push_metadata exists");
        let protect_start = source
            .find("async fn protect_committed_blob_refs")
            .expect("protect_committed_blob_refs exists");
        let reconcile_start = source
            .find("pub(crate) async fn reconcile_vault_metadata_unlocked")
            .expect("reconcile exists");
        let record_impl = &source[record_start..protect_start];
        let protect_impl = &source[protect_start..reconcile_start];

        assert!(!record_impl.contains("for h in input.blob_hashes"));
        assert!(!protect_impl.contains("for hash in blob_hashes"));
        assert!(record_impl.contains("insert_blob_refs_in_tx"));
        assert!(protect_impl.contains("insert_blob_refs_in_tx"));
    }

    #[test]
    fn pull_for_user_batches_added_and_modified_file_reads() {
        let source = include_str!("sync.rs");
        let fn_start = source.find("async fn pull_for_user").unwrap();
        let next_fn = source[fn_start + 1..]
            .find("\nasync fn files_to_pull")
            .map(|idx| fn_start + 1 + idx)
            .unwrap();
        let implementation = &source[fn_start..next_fn];

        assert_eq!(
            implementation.matches("files_to_pull(").count(),
            1,
            "pull_for_user should read added and modified files in one files_to_pull call"
        );
    }

    async fn state_user_vault() -> (AppState, AuthenticatedUser, String, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let state = AppState::new(p, tmp.path().to_path_buf(), "t".into(), true)
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
                device_id: "device-sync",
                device_name: "d",
            })
            .await
            .unwrap();
        let vault = vault::create_vault(&state, &user.id, "main").await.unwrap();
        let device_id = token_row.device_id.clone();
        let auth = AuthenticatedUser {
            user_id: user.id,
            username: user.username,
            is_admin: false,
            token_id: token_row.id,
            device_id,
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
    async fn upload_check_trusts_vault_upload_row_when_blob_file_is_missing() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let hash = "1".repeat(64);
        state
            .blob_uploads
            .record_upload(&vid, &hash, chrono::Utc::now().timestamp())
            .await
            .unwrap();

        let resp = upload_check(&state, &user.user_id, &vid, vec![hash])
            .await
            .unwrap();

        assert!(resp.missing.is_empty());
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
    async fn pull_rejects_vaults_over_tree_entry_budget() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let git = state.git_store();
        let changes = (0..4)
            .map(|idx| crate::storage::git::FileChange::Upsert {
                path: format!("note-{idx}.md"),
                file: StoredFile::Text {
                    bytes: idx.to_string().into_bytes(),
                },
            })
            .collect::<Vec<_>>();
        git.commit_changes(&vid, None, &changes, "seed")
            .await
            .unwrap();

        let err = pull_for_user(
            &state,
            &user.user_id,
            None,
            &vid,
            None,
            RequestMetadata::default(),
            3,
        )
        .await
        .unwrap_err();

        assert_eq!(err.status, axum::http::StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(err.code, "pull_too_large");
    }

    #[tokio::test]
    async fn push_rejects_too_many_changes() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let changes = (0..1001)
            .map(|i| PushChange::Text {
                path: format!("note-{i}.md"),
                content: "x".into(),
            })
            .collect();

        let err = push(
            &state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: None,
                changes,
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "too_many_changes");
    }

    #[tokio::test]
    async fn push_preserves_change_order_before_batch_blob_availability_check() {
        let (state, user, vid, _tmp) = state_user_vault().await;

        let err = push(
            &state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: None,
                changes: vec![
                    PushChange::Text {
                        path: "../bad.md".into(),
                        content: "x".into(),
                    },
                    PushChange::Blob {
                        path: "img.png".into(),
                        blob_hash: "not-a-sha".into(),
                        size: 5,
                        mime: None,
                    },
                ],
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "invalid_path");
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
        upload_blob(&state, &user.user_id, &vid, &hash, data)
            .await
            .unwrap();

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
        upload_blob(&state, &user.user_id, &vid, &hash, data)
            .await
            .unwrap();

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
    async fn push_rejects_blob_uploaded_only_to_another_vault() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let other = vault::create_vault(&state, &user.user_id, "other")
            .await
            .unwrap();
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        upload_blob(&state, &user.user_id, &other.id, &hash, data)
            .await
            .unwrap();

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
                    size: 5,
                    mime: None,
                }],
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "missing_blob");
    }

    #[tokio::test]
    async fn push_rejects_blob_referenced_only_in_another_vault() {
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
                    size: 5,
                    mime: None,
                }],
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "missing_blob");
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
    async fn push_sanitizes_device_name_in_commit_message() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let resp = push(
            &state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: Some("  Laptop    Workstation  ".into()),
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "hello".into(),
                }],
            },
        )
        .await
        .unwrap();

        let repo = Repository::open_bare(state.default_vault_root().join(&vid)).unwrap();
        let commit = repo
            .find_commit(Oid::from_str(&resp.new_commit).unwrap())
            .unwrap();
        let message = commit.message().unwrap();

        assert_eq!(message, "sync: Laptop Workstation\n1 files changed");
    }

    #[tokio::test]
    async fn push_truncates_device_name_in_commit_message() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let resp = push(
            &state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: Some("A".repeat(300)),
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "hello".into(),
                }],
            },
        )
        .await
        .unwrap();

        let repo = Repository::open_bare(state.default_vault_root().join(&vid)).unwrap();
        let commit = repo
            .find_commit(Oid::from_str(&resp.new_commit).unwrap())
            .unwrap();
        let subject = commit.message().unwrap().lines().next().unwrap();

        assert_eq!(
            subject,
            format!("sync: {}", "A".repeat(MAX_COMMIT_DEVICE_NAME_CHARS))
        );
    }

    #[tokio::test]
    async fn push_rejects_device_name_over_hard_limit() {
        let (state, user, vid, _tmp) = state_user_vault().await;

        let err = push(
            &state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: Some("A".repeat(MAX_PUSH_DEVICE_NAME_BYTES + 1)),
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "hello".into(),
                }],
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "invalid_device_name");
    }

    #[tokio::test]
    async fn push_rejects_device_name_control_characters() {
        let (state, user, vid, _tmp) = state_user_vault().await;

        let err = push(
            &state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: Some("desk\nhidden".into()),
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "hello".into(),
                }],
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "invalid_device_name");
    }

    #[tokio::test]
    async fn pull_records_activity_with_request_metadata() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let pushed = push(
            &state,
            &user,
            &vid,
            None,
            None,
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

        let pulled = pull_with_request_metadata(
            &state,
            &user,
            &vid,
            None,
            RequestMetadata {
                client_ip: Some("203.0.113.11"),
                user_agent: Some("PKVSync-Plugin/0.1.0"),
            },
        )
        .await
        .unwrap();
        assert_eq!(pulled.to.as_deref(), Some(pushed.new_commit.as_str()));

        let row: (String, String, Option<String>, Option<String>, String) = sqlx::query_as(
            "SELECT action, token_id, client_ip, user_agent, details
             FROM sync_activity WHERE vault_id = ? AND action = 'pull'",
        )
        .bind(&vid)
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(row.0, "pull");
        assert_eq!(row.1, user.token_id);
        assert_eq!(row.2.as_deref(), Some("203.0.113.11"));
        assert_eq!(row.3.as_deref(), Some("PKVSync-Plugin/0.1.0"));
        let details: serde_json::Value = serde_json::from_str(&row.4).unwrap();
        assert_eq!(details["added"], 1);
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
    async fn push_rejects_oversized_idempotency_key() {
        let (state, user, vid, _tmp) = state_user_vault().await;

        let err = push(
            &state,
            &user,
            &vid,
            None,
            Some(&"k".repeat(257)),
            PushReq {
                device_name: Some("test".into()),
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "hello".into(),
                }],
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "invalid_idempotency_key");
    }

    #[tokio::test]
    async fn queued_push_rechecks_vault_after_delete_removes_storage() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let lock = state.vault_push_lock(&vid);
        let guard = lock.lock().await;
        let repo_dir = state.default_vault_root().join(&vid);

        let queued_state = state.clone();
        let queued_user = user.clone();
        let queued_vid = vid.clone();
        let queued = tokio::spawn(async move {
            push(
                &queued_state,
                &queued_user,
                &queued_vid,
                None,
                None,
                PushReq {
                    device_name: Some("test".into()),
                    changes: vec![PushChange::Text {
                        path: "note.md".into(),
                        content: "hello".into(),
                    }],
                },
            )
            .await
        });

        tokio::task::yield_now().await;
        assert!(state
            .vaults
            .delete_for_user(&user.user_id, &vid)
            .await
            .unwrap());
        state.remove_vault_push_lock(&vid);
        state.events.remove(&vid);

        drop(guard);
        let err = queued.await.unwrap().unwrap_err();

        assert_eq!(err.status, axum::http::StatusCode::NOT_FOUND);
        assert!(!tokio::fs::try_exists(&repo_dir).await.unwrap());
        assert!(state.vaults.find_by_id(&vid).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn push_times_out_when_vault_push_lock_is_stuck() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let lock = state.vault_push_lock(&vid);
        let _guard = lock.lock().await;

        let err = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            push(
                &state,
                &user,
                &vid,
                None,
                None,
                PushReq {
                    device_name: Some("test".into()),
                    changes: vec![PushChange::Text {
                        path: "note.md".into(),
                        content: "hello".into(),
                    }],
                },
            ),
        )
        .await
        .expect("push should return a lock timeout error")
        .unwrap_err();

        assert_eq!(err.status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(err.code, "vault_busy");
    }

    #[tokio::test]
    async fn non_member_push_does_not_wait_for_vault_push_lock() {
        let (state, _owner, vid, _tmp) = state_user_vault().await;
        let intruder = state
            .users
            .create(NewUser {
                username: "mallory".into(),
                password_hash: "hash".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let token_row = state
            .tokens
            .create(NewToken {
                user_id: &intruder.id,
                token_hash: &token::hash(&token::generate()),
                device_id: "device-intruder",
                device_name: "intruder",
            })
            .await
            .unwrap();
        let intruder = AuthenticatedUser {
            user_id: intruder.id,
            username: intruder.username,
            is_admin: false,
            token_id: token_row.id,
            device_id: token_row.device_id,
        };
        let lock = state.vault_push_lock(&vid);
        let _guard = lock.lock().await;

        let err = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            push(
                &state,
                &intruder,
                &vid,
                None,
                None,
                PushReq {
                    device_name: Some("intruder".into()),
                    changes: vec![PushChange::Text {
                        path: "note.md".into(),
                        content: "blocked".into(),
                    }],
                },
            ),
        )
        .await
        .expect("non-member push should fail before waiting on the push lock")
        .unwrap_err();

        assert_eq!(err.status, axum::http::StatusCode::NOT_FOUND);
        assert_eq!(err.code, "not_found");
    }

    #[tokio::test]
    async fn idempotency_keys_are_scoped_per_vault() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let other = vault::create_vault(&state, &user.user_id, "other")
            .await
            .unwrap();

        let first = push(
            &state,
            &user,
            &vid,
            None,
            Some("shared-idem"),
            PushReq {
                device_name: Some("test".into()),
                changes: vec![PushChange::Text {
                    path: "one.md".into(),
                    content: "one".into(),
                }],
            },
        )
        .await
        .unwrap();
        let second = push(
            &state,
            &user,
            &other.id,
            None,
            Some("shared-idem"),
            PushReq {
                device_name: Some("test".into()),
                changes: vec![PushChange::Text {
                    path: "two.md".into(),
                    content: "two".into(),
                }],
            },
        )
        .await
        .unwrap();

        assert_ne!(second.new_commit, first.new_commit);
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
    async fn pull_returns_conflict_sidecar_even_when_excluded() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        state
            .runtime_cfg_repo
            .set_extra_exclude_globs(vec!["*.conflict-*".into()], None)
            .await
            .unwrap();
        state
            .runtime_cfg
            .replace(state.runtime_cfg_repo.load().await.unwrap())
            .await;

        let base = push(
            &state,
            &user,
            &vid,
            None,
            None,
            PushReq {
                device_name: Some("base".into()),
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "base\n".into(),
                }],
            },
        )
        .await
        .unwrap();
        let current = push(
            &state,
            &user,
            &vid,
            Some(&base.new_commit),
            None,
            PushReq {
                device_name: Some("remote".into()),
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "remote\n".into(),
                }],
            },
        )
        .await
        .unwrap();

        let merged = push(
            &state,
            &user,
            &vid,
            Some(&base.new_commit),
            None,
            PushReq {
                device_name: Some("Laptop".into()),
                changes: vec![PushChange::Text {
                    path: "note.md".into(),
                    content: "local\n".into(),
                }],
            },
        )
        .await
        .unwrap();
        assert_ne!(merged.new_commit, current.new_commit);

        let pulled = pull(&state, &user.user_id, &vid, Some(&base.new_commit))
            .await
            .unwrap();

        assert!(
            pulled
                .added
                .iter()
                .any(|file| file.path.contains(".conflict-")),
            "expected generated conflict sidecar in pull added set, got {:?}",
            pulled
                .added
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>()
        );
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
    #[ignore = "benchmark-flavored check for pull filter-first behavior"]
    async fn pull_filter_first_benchmark_skips_excluded_candidates() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        state
            .runtime_cfg_repo
            .set_extra_exclude_globs(vec!["excluded/**".into()], None)
            .await
            .unwrap();
        state
            .runtime_cfg
            .replace(state.runtime_cfg_repo.load().await.unwrap())
            .await;

        let mut changes = Vec::new();
        for i in 0..100 {
            changes.push(FileChange::Upsert {
                path: format!("excluded/{i}.md"),
                file: StoredFile::Text {
                    bytes: vec![b'x'; 256 * 1024],
                },
            });
        }
        changes.push(FileChange::Upsert {
            path: "included.md".into(),
            file: StoredFile::Text {
                bytes: b"keep".to_vec(),
            },
        });

        let git = Git2VaultStore::new(state.default_vault_root());
        let commit = git
            .commit_changes(&vid, None, &changes, "seed mostly excluded pull")
            .await
            .unwrap();

        let pulled = pull(&state, &user.user_id, &vid, None).await.unwrap();

        assert_eq!(pulled.to.as_deref(), Some(commit.as_str()));
        assert_eq!(pulled.added.len(), 1);
        assert_eq!(pulled.added[0].path, "included.md");
        assert_eq!(pulled.added[0].content_inline.as_deref(), Some("keep"));
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
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into(), true)
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
                device_id: "device-sync-admin",
                device_name: "d",
            })
            .await
            .unwrap();
        let device_id = tr.device_id.clone();
        let user = crate::auth::AuthenticatedUser {
            user_id: u.id.clone(),
            username: u.username.clone(),
            is_admin: false,
            token_id: tr.id,
            device_id,
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
        let rejected_statuses: Vec<_> = [&r1, &r2]
            .into_iter()
            .filter_map(|result| result.as_ref().err().map(|err| err.status))
            .collect();
        assert!(
            successes == 1,
            "expected exactly one successful push, got {successes} successes and rejected statuses {rejected_statuses:?}"
        );
        assert_eq!(rejected_statuses.len(), 1);
        assert!(
            matches!(
                rejected_statuses[0],
                axum::http::StatusCode::CONFLICT | axum::http::StatusCode::SERVICE_UNAVAILABLE
            ),
            "expected the competing push to be rejected as conflict or busy, got {:?}",
            rejected_statuses[0]
        );
    }

    #[tokio::test]
    async fn push_updates_vault_stats_and_activity_atomically() {
        let (state, user, vid, _tmp) = setup().await;
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        upload_blob(&state, &user.user_id, &vid, &hash, data.clone())
            .await
            .unwrap();

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
    async fn committed_push_preserves_blob_refs_when_metadata_repair_fails() {
        let (state, user, vid, _tmp) = setup().await;
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        upload_blob(&state, &user.user_id, &vid, &hash, data)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TRIGGER fail_vault_stats_update
             BEFORE UPDATE OF size_bytes, file_count, last_sync_at ON vaults
             BEGIN
                 SELECT RAISE(FAIL, 'metadata blocked');
             END",
        )
        .execute(&state.pool)
        .await
        .unwrap();

        let resp = push(
            &state,
            &user,
            &vid,
            None,
            Some("metadata-fail-once"),
            PushReq {
                device_name: Some("test".into()),
                changes: vec![PushChange::Blob {
                    path: "img.png".into(),
                    blob_hash: hash.clone(),
                    size: 5,
                    mime: Some("image/png".into()),
                }],
            },
        )
        .await
        .unwrap();

        let git = Git2VaultStore::new(state.default_vault_root());
        assert_eq!(
            git.head(&vid).await.unwrap().as_deref(),
            Some(resp.new_commit.as_str())
        );
        assert!(state
            .blob_refs
            .is_referenced_by_vault(&vid, &hash)
            .await
            .unwrap());
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

    #[tokio::test]
    async fn reconcile_vault_metadata_removes_stale_blob_refs_for_vault() {
        let (state, _user, vid, _tmp) = setup().await;
        let old_hash = "a".repeat(64);
        let current_hash = "b".repeat(64);
        let git = Git2VaultStore::new(state.default_vault_root());
        let old_commit = git
            .commit_changes(
                &vid,
                None,
                &[FileChange::Upsert {
                    path: "old.png".into(),
                    file: StoredFile::BlobPointer {
                        hash: old_hash.clone(),
                        size: 5,
                        mime: Some("image/png".into()),
                    },
                }],
                "old blob",
            )
            .await
            .unwrap();
        let current_commit = git
            .commit_changes(
                &vid,
                Some(&old_commit),
                &[
                    FileChange::Delete {
                        path: "old.png".into(),
                    },
                    FileChange::Upsert {
                        path: "current.png".into(),
                        file: StoredFile::BlobPointer {
                            hash: current_hash.clone(),
                            size: 7,
                            mime: Some("image/png".into()),
                        },
                    },
                ],
                "current blob",
            )
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO blob_refs (blob_hash, vault_id, commit_hash) VALUES (?, ?, ?), (?, ?, ?)",
        )
        .bind(&old_hash)
        .bind(&vid)
        .bind(&old_commit)
        .bind(&current_hash)
        .bind(&vid)
        .bind(&current_commit)
        .execute(&state.pool)
        .await
        .unwrap();

        let report = reconcile_vault_metadata(&state, &vid).await.unwrap();

        assert_eq!(report.blob_refs, 1);
        assert!(!state
            .blob_refs
            .is_referenced_by_vault(&vid, &old_hash)
            .await
            .unwrap());
        assert!(state
            .blob_refs
            .is_referenced_by_vault(&vid, &current_hash)
            .await
            .unwrap());
    }
}
