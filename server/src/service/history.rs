use crate::api::error::ApiError;
use crate::service::diff::{ChangeType, CommitChange};
use crate::service::{vault, AppState};
use crate::storage::git::Git2VaultStore;
use crate::storage::path;
use git2::{Oid, Repository};
use serde::Serialize;
use std::path::Path;

const MAX_FILE_HISTORY_LIMIT: usize = 200;
const MIN_FILE_HISTORY_COMMITS_INSPECTED: usize = 1_000;
const MAX_FILE_HISTORY_COMMITS_INSPECTED: usize = 10_000;
const FILE_HISTORY_SCAN_MULTIPLIER: usize = 100;

#[derive(Debug, Serialize)]
pub struct CommitSummary {
    pub commit: String,
    pub parent: Option<String>,
    pub message: String,
    pub timestamp: i64,
    pub author_device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_type: Option<ChangeType>,
}

#[derive(Debug, Serialize)]
pub struct CommitDetail {
    pub commit: String,
    pub parent: Option<String>,
    pub message: String,
    pub timestamp: i64,
    pub author_device: Option<String>,
    pub changes: Vec<CommitChange>,
}

pub async fn commits(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    limit: usize,
    path: Option<&str>,
) -> Result<Vec<CommitSummary>, ApiError> {
    if let Some(path) = path {
        return file_history(state, user_id, vault_id, path, limit).await;
    }
    let _ = vault::ensure_user_vault(state, user_id, vault_id).await?;
    let root = state.default_vault_root().join(vault_id);
    tokio::task::spawn_blocking(move || -> Result<Vec<CommitSummary>, ApiError> {
        let repo = Repository::open_bare(root).map_err(|e| ApiError::internal(e.to_string()))?;
        let mut walk = repo
            .revwalk()
            .map_err(|e| ApiError::internal(e.to_string()))?;
        walk.push_head()
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let mut out = Vec::new();
        for oid in walk.take(limit) {
            let oid = oid.map_err(|e| ApiError::internal(e.to_string()))?;
            let c = repo
                .find_commit(oid)
                .map_err(|e| ApiError::internal(e.to_string()))?;
            out.push(summary_from_commit(&c, None)?);
        }
        Ok(out)
    })
    .await
    .map_err(|_| ApiError::internal("blocking task panicked"))?
}

pub async fn commit_detail(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    commit: &str,
) -> Result<CommitDetail, ApiError> {
    let _ = vault::ensure_user_vault(state, user_id, vault_id).await?;
    let root = state.default_vault_root().join(vault_id);
    let vault_root = state.default_vault_root();
    let vault_id = vault_id.to_string();
    let commit = commit.to_string();
    let summary = tokio::task::spawn_blocking(move || -> Result<CommitSummary, ApiError> {
        let repo = Repository::open_bare(root).map_err(|e| ApiError::internal(e.to_string()))?;
        let oid = Oid::from_str(&commit)
            .map_err(|e| ApiError::bad_request("bad_commit", e.to_string()))?;
        let c = repo
            .find_commit(oid)
            .map_err(|e| ApiError::not_found(e.to_string()))?;
        summary_from_commit(&c, None)
    })
    .await
    .map_err(|_| ApiError::internal("blocking task panicked"))??;
    let store = Git2VaultStore::new(vault_root);
    let changes = store
        .tree_diff(&vault_id, summary.parent.as_deref(), &summary.commit)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(CommitDetail {
        commit: summary.commit,
        parent: summary.parent,
        message: summary.message,
        timestamp: summary.timestamp,
        author_device: summary.author_device,
        changes,
    })
}

pub async fn file_history(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    file_path: &str,
    limit: usize,
) -> Result<Vec<CommitSummary>, ApiError> {
    let _ = vault::ensure_user_vault(state, user_id, vault_id).await?;
    let file_path = path::normalize(file_path)
        .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
    let root = state.default_vault_root().join(vault_id);
    let limit = limit.min(MAX_FILE_HISTORY_LIMIT);
    let scan_budget = file_history_scan_budget(limit);
    tokio::task::spawn_blocking(move || -> Result<Vec<CommitSummary>, ApiError> {
        file_history_from_repo(&root, &file_path, limit, scan_budget)
    })
    .await
    .map_err(|_| ApiError::internal("blocking task panicked"))?
}

fn file_history_scan_budget(limit: usize) -> usize {
    if limit == 0 {
        return 0;
    }
    limit.saturating_mul(FILE_HISTORY_SCAN_MULTIPLIER).clamp(
        MIN_FILE_HISTORY_COMMITS_INSPECTED,
        MAX_FILE_HISTORY_COMMITS_INSPECTED,
    )
}

fn file_history_from_repo(
    root: &Path,
    file_path: &str,
    limit: usize,
    scan_budget: usize,
) -> Result<Vec<CommitSummary>, ApiError> {
    if limit == 0 || scan_budget == 0 {
        return Ok(Vec::new());
    }
    let repo = Repository::open_bare(root).map_err(|e| ApiError::internal(e.to_string()))?;
    let mut walk = repo
        .revwalk()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    walk.push_head()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut out = Vec::new();
    for oid in walk.take(scan_budget) {
        let oid = oid.map_err(|e| ApiError::internal(e.to_string()))?;
        let commit = repo
            .find_commit(oid)
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let tree = commit
            .tree()
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let current = tree.get_path(Path::new(file_path)).ok().map(|e| e.id());
        let parent = if commit.parent_count() > 0 {
            let parent = commit
                .parent(0)
                .map_err(|e| ApiError::internal(e.to_string()))?;
            let parent_tree = parent
                .tree()
                .map_err(|e| ApiError::internal(e.to_string()))?;
            parent_tree
                .get_path(Path::new(file_path))
                .ok()
                .map(|e| e.id())
        } else {
            None
        };
        if current == parent {
            continue;
        }
        let change_type = match (parent, current) {
            (None, Some(_)) => Some(ChangeType::Added),
            (Some(_), Some(_)) => Some(ChangeType::Modified),
            (Some(_), None) => Some(ChangeType::Deleted),
            (None, None) => None,
        };
        if let Some(change_type) = change_type {
            out.push(summary_from_commit(&commit, Some(change_type))?);
            if out.len() >= limit {
                break;
            }
        }
    }
    Ok(out)
}

fn summary_from_commit(
    commit: &git2::Commit<'_>,
    change_type: Option<ChangeType>,
) -> Result<CommitSummary, ApiError> {
    let message = commit.message().unwrap_or("").to_string();
    Ok(CommitSummary {
        commit: commit.id().to_string(),
        parent: if commit.parent_count() == 0 {
            None
        } else {
            Some(
                commit
                    .parent_id(0)
                    .map_err(|e| ApiError::internal(e.to_string()))?
                    .to_string(),
            )
        },
        author_device: parse_author_device(&message),
        message,
        timestamp: commit.time().seconds(),
        change_type,
    })
}

fn parse_author_device(message: &str) -> Option<String> {
    message
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("sync: "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{token, AuthenticatedUser};
    use crate::db::pool;
    use crate::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
    use crate::service::{sync, vault, AppState};

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
                device_id: "device-history",
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
    async fn lists_commits_and_details_files() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let pushed = sync::push(
            &state,
            &user,
            &vid,
            None,
            None,
            sync::PushReq {
                device_name: Some("test".into()),
                changes: vec![sync::PushChange::Text {
                    path: "note.md".into(),
                    content: "hello".into(),
                }],
            },
        )
        .await
        .unwrap();

        let list = commits(&state, &user.user_id, &vid, 10, None)
            .await
            .unwrap();
        assert_eq!(list[0].commit, pushed.new_commit);
        let detail = commit_detail(&state, &user.user_id, &vid, &pushed.new_commit)
            .await
            .unwrap();
        assert_eq!(detail.commit, pushed.new_commit);
        assert_eq!(detail.changes.len(), 1);
        assert_eq!(detail.changes[0].path, "note.md");
        assert_eq!(detail.changes[0].change_type, ChangeType::Added);
    }

    #[tokio::test]
    async fn file_history_stops_after_scan_budget() {
        let (state, user, vid, _tmp) = state_user_vault().await;
        let first = sync::push(
            &state,
            &user,
            &vid,
            None,
            None,
            sync::PushReq {
                device_name: Some("test".into()),
                changes: vec![sync::PushChange::Text {
                    path: "target.md".into(),
                    content: "v1".into(),
                }],
            },
        )
        .await
        .unwrap();
        let mut head = first.new_commit;
        for index in 0..3 {
            let pushed = sync::push(
                &state,
                &user,
                &vid,
                Some(&head),
                None,
                sync::PushReq {
                    device_name: Some("test".into()),
                    changes: vec![sync::PushChange::Text {
                        path: "other.md".into(),
                        content: format!("v{index}"),
                    }],
                },
            )
            .await
            .unwrap();
            head = pushed.new_commit;
        }

        let root = state.default_vault_root().join(&vid);
        let rows = file_history_from_repo(&root, "target.md", 1, 2).unwrap();

        assert!(rows.is_empty());
    }
}
