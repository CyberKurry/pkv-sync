use crate::api::error::ApiError;
use crate::db::repos::{NewActivity, SyncActivityRepo};
use crate::service::vault;
use crate::service::AppState;
use crate::storage::git::{
    parse_blob_pointer, storage_vault_path, GitStoreError, GitVaultStore, StoredFile, TreeEntry,
};
use crate::storage::path;
use git2::{Oid, Repository};
use std::path::Path;

use super::paths::{path_visible_on_read, sync_path_filter};
use super::{PullFile, PullResp, RequestMetadata, StateResp};

const MAX_PULL_TREE_ENTRIES: usize = 50_000;
const PULL_TEXT_INLINE_BYTES: u64 = 64 * 1024;
#[cfg(test)]
const MAX_PULL_INLINE_RESPONSE_BYTES: u64 = 64 * 1024;
#[cfg(not(test))]
const MAX_PULL_INLINE_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;

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
    let mut pull_paths = Vec::new();
    let mut deleted = Vec::new();

    for (path, cur) in &current {
        if !path_visible_on_read(&path_filter, path) {
            continue;
        }
        match base.get(path) {
            None => pull_paths.push(PullFileRequest::from_entry(PullFileBucket::Added, cur)),
            Some(old) if old.git_oid != cur.git_oid => {
                pull_paths.push(PullFileRequest::from_entry(PullFileBucket::Modified, cur));
            }
            Some(_) => {}
        }
    }
    for path in base.keys() {
        if !current.contains_key(path) && path_visible_on_read(&path_filter, path) {
            deleted.push(path.clone());
        }
    }
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

struct PullFileRequest {
    bucket: PullFileBucket,
    path: String,
    size: u64,
    is_blob_pointer: bool,
    blob_hash: Option<String>,
}

impl PullFileRequest {
    fn from_entry(bucket: PullFileBucket, entry: &TreeEntry) -> Self {
        Self {
            bucket,
            path: entry.path.clone(),
            size: entry.size,
            is_blob_pointer: entry.is_blob_pointer,
            blob_hash: entry.blob_hash.clone(),
        }
    }
}

async fn files_to_pull(
    vault_root: &Path,
    vault_id: &str,
    head: &str,
    requests: Vec<PullFileRequest>,
) -> Result<Vec<(PullFileBucket, PullFile)>, ApiError> {
    if requests.is_empty() {
        return Ok(Vec::new());
    }
    let repo_path =
        storage_vault_path(vault_root, vault_id).map_err(|e| ApiError::internal(e.to_string()))?;
    let head = head.to_string();
    tokio::task::spawn_blocking(
        move || -> Result<Vec<(PullFileBucket, PullFile)>, GitStoreError> {
            let repo = Repository::open_bare(&repo_path)?;
            let oid = Oid::from_str(&head)?;
            let commit = repo.find_commit(oid)?;
            let tree = commit.tree()?;
            let mut files = Vec::with_capacity(requests.len());
            let mut inline_budget_remaining = MAX_PULL_INLINE_RESPONSE_BYTES;
            for request in requests {
                if request.is_blob_pointer {
                    let hash = request
                        .blob_hash
                        .ok_or_else(|| git2::Error::from_str("blob pointer missing hash"))?;
                    files.push((
                        request.bucket,
                        blob_pull_file(&request.path, hash, request.size),
                    ));
                    continue;
                }
                if request.size > PULL_TEXT_INLINE_BYTES || request.size > inline_budget_remaining {
                    files.push((
                        request.bucket,
                        text_pull_file_metadata(&request.path, request.size),
                    ));
                    continue;
                }
                inline_budget_remaining -= request.size;
                let entry = tree
                    .get_path(Path::new(&request.path))
                    .map_err(|_| GitStoreError::NotFound)?;
                let blob = repo.find_blob(entry.id())?;
                let file = decode_pull_file(&request.path, blob.content().to_vec());
                let file = match file {
                    StoredFile::Text { bytes } => text_pull_file(&request.path, bytes),
                    StoredFile::BlobPointer { hash, size, .. } => {
                        blob_pull_file(&request.path, hash, size)
                    }
                };
                files.push((request.bucket, file));
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
    let size = bytes.len() as u64;
    let content = if size <= PULL_TEXT_INLINE_BYTES {
        String::from_utf8(bytes).ok()
    } else {
        None
    };
    PullFile {
        path: path.into(),
        file_type: "text",
        size,
        content_inline: content,
        blob_hash: None,
    }
}

fn text_pull_file_metadata(path: &str, size: u64) -> PullFile {
    PullFile {
        path: path.into(),
        file_type: "text",
        size,
        content_inline: None,
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
    if let Some(file) = parse_blob_pointer(&bytes).and_then(|p| p.into_file_for_path(path)) {
        return file;
    }
    StoredFile::Text { bytes }
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
    use crate::db::repos::RuntimeConfigRepo;
    use crate::service::sync::tests::state_user_vault;
    use crate::service::sync::{push, PushChange, PushReq};
    use crate::storage::git::{FileChange, Git2VaultStore};

    #[test]
    fn pull_for_user_batches_added_and_modified_file_reads() {
        let source = include_str!("pull.rs");
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

    #[test]
    fn files_to_pull_uses_storage_vault_path_guard() {
        let source = include_str!("pull.rs");
        let fn_start = source.find("async fn files_to_pull").unwrap();
        let next_fn = source[fn_start + 1..]
            .find("\nfn text_pull_file")
            .map(|idx| fn_start + 1 + idx)
            .unwrap();
        let implementation = &source[fn_start..next_fn];
        let raw_join = ["vault_root", ".join(vault_id)"].concat();
        let guarded_join = ["storage_vault", "_path(vault_root, vault_id)"].concat();

        assert!(
            !implementation.contains(&raw_join),
            "files_to_pull should not join unvalidated vault ids directly"
        );
        assert!(
            implementation.contains(&guarded_join),
            "files_to_pull should use the shared storage vault path guard"
        );
    }

    #[test]
    fn decode_pull_file_parses_pointer_json_once() {
        let source = include_str!("pull.rs");
        let fn_start = source.find("fn decode_pull_file").unwrap();
        let next_fn = source[fn_start + 1..]
            .find("\n/// Read a normalized vault path")
            .or_else(|| source[fn_start + 1..].find("\nfn "))
            .map(|idx| fn_start + 1 + idx)
            .unwrap();
        let decode_impl = &source[fn_start..next_fn];

        assert_eq!(
            decode_impl.matches("parse_blob_pointer(").count(),
            1,
            "decode_pull_file should parse pointer JSON at most once via shared helper"
        );
    }

    #[test]
    fn text_pull_file_omits_invalid_utf8_from_inline_content() {
        let file = text_pull_file("note.md", vec![0xff; 8]);

        assert_eq!(file.file_type, "text");
        assert_eq!(file.size, 8);
        assert!(file.content_inline.is_none());
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
    async fn pull_caps_inline_payload_over_response_byte_budget() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let git = state.git_store();
        let changes = (0..3)
            .map(|idx| crate::storage::git::FileChange::Upsert {
                path: format!("note-{idx}.md"),
                file: StoredFile::Text {
                    bytes: vec![b'x'; 32 * 1024],
                },
            })
            .collect::<Vec<_>>();
        git.commit_changes(&vid, None, &changes, "seed")
            .await
            .unwrap();

        let pulled = pull_for_user(
            &state,
            &user.user_id,
            None,
            &vid,
            None,
            RequestMetadata::default(),
            MAX_PULL_TREE_ENTRIES,
        )
        .await
        .unwrap();

        assert_eq!(pulled.added.len(), 3);
        let inline_bytes: usize = pulled
            .added
            .iter()
            .filter_map(|file| file.content_inline.as_ref())
            .map(|content| content.len())
            .sum();
        assert!(inline_bytes <= MAX_PULL_INLINE_RESPONSE_BYTES as usize);
        assert!(
            pulled
                .added
                .iter()
                .any(|file| file.content_inline.is_none()),
            "expected at least one text file to be returned as metadata-only"
        );
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
