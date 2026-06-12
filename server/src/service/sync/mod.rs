use crate::api::error::ApiError;
use crate::db::repos::{NewActivity, RuntimeConfig, SyncActivityRepo};
use crate::service::events::EventChange;
use crate::service::exclude::SyncPathFilter;
use crate::service::AppState;
use crate::storage::blob::LocalFsBlobStore;
use crate::storage::git::FileChange;
use crate::storage::text_kind::TextClassifier;
use serde::{Deserialize, Serialize};

mod blobs;
mod events;
mod merge_apply;
mod paths;
mod pull;
mod push;
mod reconcile;

pub use blobs::{download_blob, upload_blob, upload_check};
pub(crate) use merge_apply::is_generated_conflict_sidecar;
pub(crate) use paths::{ensure_path_visible_for_sync_api, path_visible_on_read, vault_path_filter};
pub use pull::{pull, pull_with_request_metadata, read_file, state};
pub(crate) use push::reconcile_vault_metadata_unlocked;
pub use push::{push, push_with_cas, push_with_request_metadata};
pub use reconcile::{reconcile_vault_metadata, ReconcileReport};

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PushStatsDelta {
    size_delta: i64,
    file_count_delta: i64,
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

/// Compile-time assertion that the `service::sync` public surface survives
/// refactors. The X2 plan depends on every path below; if a future change
/// removes or renames one of them, this module fails to compile.
///
/// Lives in-crate (instead of `tests/`) because the crate has no lib target,
/// and this also lets us pin `pub(crate)` re-exports.
#[cfg(test)]
mod surface_guard {
    #[allow(unused_imports)]
    use crate::service::sync::{
        blob_store, download_blob, ensure_path_visible_for_sync_api, is_generated_conflict_sidecar,
        path_visible_on_read, pull, pull_with_request_metadata, push, push_with_cas,
        push_with_request_metadata, read_file, reconcile_vault_metadata,
        reconcile_vault_metadata_unlocked, record_view, state, upload_blob, upload_check,
        vault_path_filter, CasConflict, PullFile, PullResp, PushChange, PushReq, PushResp,
        ReconcileReport, RequestMetadata, StateResp, UploadCheckReq, UploadCheckResp,
    };

    #[test]
    fn sync_public_surface_compiles() {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{token, AuthenticatedUser};
    use crate::db::pool;
    use crate::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
    use crate::service::vault;

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

    pub(super) async fn state_user_vault(
    ) -> (AppState, AuthenticatedUser, String, tempfile::TempDir) {
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
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::auth::{password, token};
    use crate::db::pool;
    use crate::db::repos::{BlobRefRepo, NewToken, NewUser, TokenRepo, UserRepo, VaultRepo};
    use crate::service::vault;
    use crate::storage::blob::{BlobStore, LocalFsBlobStore};
    use crate::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};
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
    async fn push_stats_handle_overwrite_delete_and_blob_pointer_size() {
        let (state, user, vid, _tmp) = setup().await;
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        upload_blob(&state, &user.user_id, &vid, &hash, data)
            .await
            .unwrap();

        let first = push(
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
                        content: "old text".into(),
                    },
                    PushChange::Text {
                        path: "remove.md".into(),
                        content: "delete me".into(),
                    },
                ],
            },
        )
        .await
        .unwrap();

        let _second = push(
            &state,
            &user,
            &vid,
            Some(&first.new_commit),
            None,
            PushReq {
                device_name: Some("test".into()),
                changes: vec![
                    PushChange::Text {
                        path: "note.md".into(),
                        content: "new".into(),
                    },
                    PushChange::Delete {
                        path: "remove.md".into(),
                    },
                    PushChange::Blob {
                        path: "img.png".into(),
                        blob_hash: hash,
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
        assert_eq!(vault.size_bytes, 8);
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
