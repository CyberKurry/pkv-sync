use crate::api::error::ApiError;
use crate::service::{vault, AppState};
use crate::storage::git::{Git2VaultStore, GitVaultStore};
use git2::{Oid, Repository};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CommitSummary {
    pub commit: String,
    pub message: String,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
pub struct CommitDetail {
    pub commit: String,
    pub message: String,
    pub timestamp: i64,
    pub changed_files: Vec<String>,
}

pub async fn commits(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    limit: usize,
) -> Result<Vec<CommitSummary>, ApiError> {
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
            out.push(CommitSummary {
                commit: oid.to_string(),
                message: c.message().unwrap_or("").to_string(),
                timestamp: c.time().seconds(),
            });
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
        Ok(CommitSummary {
            commit: oid.to_string(),
            message: c.message().unwrap_or("").to_string(),
            timestamp: c.time().seconds(),
        })
    })
    .await
    .map_err(|_| ApiError::internal("blocking task panicked"))??;
    let store = Git2VaultStore::new(vault_root);
    let files = store
        .list_tree(&vault_id, Some(&summary.commit))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .into_iter()
        .map(|entry| entry.path)
        .collect();
    Ok(CommitDetail {
        commit: summary.commit,
        message: summary.message,
        timestamp: summary.timestamp,
        changed_files: files,
    })
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

        let list = commits(&state, &user.user_id, &vid, 10).await.unwrap();
        assert_eq!(list[0].commit, pushed.new_commit);
        let detail = commit_detail(&state, &user.user_id, &vid, &pushed.new_commit)
            .await
            .unwrap();
        assert_eq!(detail.commit, pushed.new_commit);
        assert_eq!(detail.changed_files, vec!["note.md"]);
    }
}
