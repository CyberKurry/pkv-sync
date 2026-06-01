use crate::api::error::ApiError;
use crate::db::repos::{self, NewActivity, SyncActivityRepo, Vault, VaultRepo};
use crate::service::events::{EventKind, VaultEvent};
use crate::service::{vault_settings, AppState};
use crate::storage::git::{Git2VaultStore, GitStoreError, GitVaultStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackResult {
    pub from_commit: Option<String>,
    pub to_commit: String,
    pub rolled_back: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RollbackError {
    #[error("vault not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("unknown commit: {commit}")]
    UnknownCommit { commit: String },
    #[error("rollback failed: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Copy)]
pub struct RollbackActor<'a> {
    pub user_id: &'a str,
    pub is_admin: bool,
    pub token_id: Option<&'a str>,
    pub device_id: &'a str,
}

impl<'a> From<&'a crate::auth::AuthenticatedUser> for RollbackActor<'a> {
    fn from(user: &'a crate::auth::AuthenticatedUser) -> Self {
        Self {
            user_id: &user.user_id,
            is_admin: user.is_admin,
            token_id: Some(user.token_id.as_str()),
            device_id: user.device_id.as_str(),
        }
    }
}

pub async fn rollback_to_commit(
    state: &AppState,
    user: &crate::auth::AuthenticatedUser,
    vault_id: &str,
    target_commit: &str,
) -> Result<RollbackResult, RollbackError> {
    rollback_to_commit_as(state, RollbackActor::from(user), vault_id, target_commit).await
}

pub async fn rollback_to_commit_as(
    state: &AppState,
    actor: RollbackActor<'_>,
    vault_id: &str,
    target_commit: &str,
) -> Result<RollbackResult, RollbackError> {
    let vault = state
        .vaults
        .find_by_id(vault_id)
        .await
        .map_err(rollback_db_error)?
        .ok_or(RollbackError::NotFound)?;
    if !actor.is_admin && vault.user_id != actor.user_id {
        return Err(RollbackError::Forbidden);
    }

    let push_lock = state.vault_push_lock(vault_id);
    let _push_guard = push_lock.lock().await;
    let git = Git2VaultStore::new(state.default_vault_root());
    let from_commit = git.head(vault_id).await.map_err(rollback_git_error)?;
    let reachable = git
        .commit_reachable_from_head(vault_id, target_commit)
        .await
        .map_err(rollback_git_error)?;
    if !reachable {
        return Err(RollbackError::UnknownCommit {
            commit: target_commit.to_string(),
        });
    }
    let target = target_commit.to_string();
    if from_commit.as_deref() == Some(target_commit) {
        return Ok(RollbackResult {
            from_commit,
            to_commit: target,
            rolled_back: false,
        });
    }

    git.set_main_ref(
        vault_id,
        target_commit,
        &format!(
            "rollback: {} -> {}",
            from_commit.as_deref().unwrap_or("none"),
            target_commit
        ),
    )
    .await
    .map_err(rollback_git_error)?;

    let from = from_commit
        .clone()
        .ok_or_else(|| RollbackError::Internal("missing source head".into()))?;
    record_rollback_activity(state, actor, vault_id, &from, target_commit).await?;
    state.events.publish(
        vault_id,
        VaultEvent {
            commit: target.clone(),
            parent: Some(from.clone()),
            source_device_id: actor.device_id.to_string(),
            at: chrono::Utc::now().timestamp(),
            kind: EventKind::Rollback {
                from_commit: from,
                to_commit: target.clone(),
            },
            changes: Vec::new(),
        },
    );

    Ok(RollbackResult {
        from_commit,
        to_commit: target,
        rolled_back: true,
    })
}

async fn record_rollback_activity(
    state: &AppState,
    actor: RollbackActor<'_>,
    vault_id: &str,
    from_commit: &str,
    to_commit: &str,
) -> Result<(), RollbackError> {
    let details = serde_json::json!({
        "from_commit": from_commit,
        "to_commit": to_commit,
    })
    .to_string();
    state
        .activities
        .insert(NewActivity {
            user_id: actor.user_id,
            vault_id: Some(vault_id),
            token_id: actor.token_id,
            action: "vault_rollback",
            commit_hash: Some(to_commit),
            client_ip: None,
            user_agent: None,
            details: Some(&details),
        })
        .await
        .map_err(rollback_db_error)?;
    Ok(())
}

fn rollback_db_error(err: sqlx::Error) -> RollbackError {
    RollbackError::Internal(err.to_string())
}

fn rollback_git_error(err: GitStoreError) -> RollbackError {
    match err {
        GitStoreError::Git(git_err)
            if matches!(
                git_err.code(),
                git2::ErrorCode::InvalidSpec | git2::ErrorCode::NotFound
            ) =>
        {
            RollbackError::UnknownCommit {
                commit: git_err.to_string(),
            }
        }
        other => RollbackError::Internal(other.to_string()),
    }
}

pub fn validate_vault_name(name: &str) -> Result<(), ApiError> {
    if name.trim().is_empty() || name.len() > 64 {
        return Err(ApiError::bad_request(
            "invalid_vault_name",
            "vault name length must be 1-64",
        ));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(ApiError::bad_request(
            "invalid_vault_name",
            "vault name cannot contain path separators",
        ));
    }
    Ok(())
}

pub async fn create_vault(state: &AppState, user_id: &str, name: &str) -> Result<Vault, ApiError> {
    validate_vault_name(name)?;
    let vault = state.vaults.create(user_id, name).await.map_err(|err| {
        if repos::vault::is_user_name_unique_error(&err) {
            ApiError::conflict("vault_name_taken", "vault name already exists")
        } else {
            ApiError::from(err)
        }
    })?;
    vault_settings::save(
        state,
        &vault.id,
        &vault_settings::VaultSettings {
            extra_sync_globs: vault_settings::starter_extra_sync_globs(),
        },
    )
    .await?;
    Ok(vault)
}

pub async fn delete_vault_for_user(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
) -> Result<bool, ApiError> {
    if state
        .vaults
        .find_for_user(user_id, vault_id)
        .await?
        .is_none()
    {
        return Ok(false);
    }
    let push_lock = state.vault_push_lock(vault_id);
    let push_guard = push_lock.lock().await;
    let deleted = state.vaults.delete_for_user(user_id, vault_id).await?;
    if !deleted {
        drop(push_guard);
        drop(push_lock);
        state.remove_vault_push_lock(vault_id);
        return Ok(false);
    }
    let storage_result = remove_vault_storage(state, vault_id).await;
    drop(push_guard);
    drop(push_lock);
    state.remove_vault_push_lock(vault_id);
    state.events.remove(vault_id);
    storage_result?;
    Ok(true)
}

pub async fn record_lifecycle_activity(
    state: &AppState,
    actor_user_id: &str,
    token_id: Option<&str>,
    action: &str,
    vault: &Vault,
    client_ip: Option<&str>,
    user_agent: Option<&str>,
) -> Result<(), ApiError> {
    let details = serde_json::json!({
        "vault_id": vault.id,
        "vault_name": vault.name,
        "owner_user_id": vault.user_id,
    })
    .to_string();
    state
        .activities
        .insert(NewActivity {
            user_id: actor_user_id,
            vault_id: None,
            token_id,
            action,
            commit_hash: None,
            client_ip,
            user_agent,
            details: Some(&details),
        })
        .await?;
    Ok(())
}

async fn remove_vault_storage(state: &AppState, vault_id: &str) -> Result<(), ApiError> {
    let path = state.default_vault_root().join(vault_id);
    match tokio::fs::remove_dir_all(&path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => {
            tracing::error!(vault_id = %vault_id, path = %path.display(), error = %e, "failed to remove vault storage");
            Err(ApiError::internal("failed to remove vault storage"))
        }
    }
}

pub async fn ensure_user_vault(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
) -> Result<Vault, ApiError> {
    state
        .vaults
        .find_for_user(user_id, vault_id)
        .await?
        .ok_or_else(|| ApiError::not_found("vault not found"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::{NewUser, UserRepo};
    use crate::service::vault_settings;

    fn expected_starter_extra_sync_globs() -> Vec<String> {
        [
            ".obsidian/themes/**",
            ".obsidian/snippets/**",
            ".obsidian/hotkeys.json",
            ".obsidian/app.json",
            ".obsidian/appearance.json",
            ".obsidian/community-plugins.json",
            ".obsidian/core-plugins.json",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    }

    async fn state_and_user() -> (AppState, String, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let state = AppState::new(p, tmp.path().to_path_buf(), "t".into(), true)
            .await
            .unwrap();
        let u = state
            .users
            .create(NewUser {
                username: "u".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        (state, u.id, tmp)
    }

    #[tokio::test]
    async fn create_vault_ok() {
        let (s, uid, _tmp) = state_and_user().await;
        let v = create_vault(&s, &uid, "main").await.unwrap();
        assert_eq!(v.name, "main");
    }

    #[tokio::test]
    async fn create_vault_saves_starter_settings() {
        let (s, uid, _tmp) = state_and_user().await;
        let v = create_vault(&s, &uid, "main").await.unwrap();

        let settings = vault_settings::load(&s, &v.id).await.unwrap();

        assert_eq!(
            settings.extra_sync_globs,
            expected_starter_extra_sync_globs()
        );
    }

    #[tokio::test]
    async fn duplicate_name_conflicts() {
        let (s, uid, _tmp) = state_and_user().await;
        create_vault(&s, &uid, "main").await.unwrap();
        let err = create_vault(&s, &uid, "main").await.unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::CONFLICT);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_duplicate_name_conflicts_from_database_constraint() {
        let (s, uid, _tmp) = state_and_user().await;
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));

        let first_state = s.clone();
        let first_user = uid.clone();
        let first_barrier = barrier.clone();
        let first = tokio::spawn(async move {
            first_barrier.wait().await;
            create_vault(&first_state, &first_user, "main").await
        });

        let second_state = s.clone();
        let second = tokio::spawn(async move {
            barrier.wait().await;
            create_vault(&second_state, &uid, "main").await
        });

        let results = [first.await.unwrap(), second.await.unwrap()];
        let successes = results.iter().filter(|r| r.is_ok()).count();
        let conflicts: Vec<_> = results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .filter(|err| {
                err.status == axum::http::StatusCode::CONFLICT && err.code == "vault_name_taken"
            })
            .collect();

        assert_eq!(successes, 1, "{results:?}");
        assert_eq!(conflicts.len(), 1, "{results:?}");
    }

    #[tokio::test]
    async fn same_name_is_allowed_for_different_users() {
        let (s, first_uid, _tmp) = state_and_user().await;
        let second = s
            .users
            .create(NewUser {
                username: "v".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();

        create_vault(&s, &first_uid, "main").await.unwrap();
        let second_vault = create_vault(&s, &second.id, "main").await.unwrap();

        assert_eq!(second_vault.name, "main");
        assert_eq!(second_vault.user_id, second.id);
    }

    #[tokio::test]
    async fn delete_vault_for_user_removes_database_row_storage_and_push_lock() {
        let (s, uid, _tmp) = state_and_user().await;
        let v = create_vault(&s, &uid, "main").await.unwrap();
        let repo_dir = s.default_vault_root().join(&v.id);
        tokio::fs::create_dir_all(&repo_dir).await.unwrap();
        tokio::fs::write(repo_dir.join("HEAD"), b"ref: main")
            .await
            .unwrap();
        let lock = s.vault_push_lock(&v.id);
        let _ = s.events.subscribe(&v.id);

        assert!(delete_vault_for_user(&s, &uid, &v.id).await.unwrap());

        assert!(s.vaults.find_by_id(&v.id).await.unwrap().is_none());
        assert!(!tokio::fs::try_exists(&repo_dir).await.unwrap());
        assert_eq!(s.vault_push_lock_count_for_tests(), 1);
        drop(lock);
        s.remove_vault_push_lock(&v.id);
        assert_eq!(s.vault_push_lock_count_for_tests(), 0);
        assert_eq!(s.events.len_for_tests(), 0);
    }

    #[tokio::test]
    async fn delete_vault_for_user_cascades_settings() {
        let (s, uid, _tmp) = state_and_user().await;
        let v = create_vault(&s, &uid, "main").await.unwrap();
        vault_settings::save(
            &s,
            &v.id,
            &vault_settings::VaultSettings {
                extra_sync_globs: vec!["notes/**".into()],
            },
        )
        .await
        .unwrap();

        assert!(delete_vault_for_user(&s, &uid, &v.id).await.unwrap());

        assert!(vault_settings::load_raw_for_tests(&s, &v.id)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn delete_vault_for_user_removes_push_lock_when_storage_delete_fails() {
        let (s, uid, _tmp) = state_and_user().await;
        let v = create_vault(&s, &uid, "main").await.unwrap();
        let repo_path = s.default_vault_root().join(&v.id);
        tokio::fs::create_dir_all(s.default_vault_root())
            .await
            .unwrap();
        tokio::fs::write(&repo_path, b"not a directory")
            .await
            .unwrap();
        let lock = s.vault_push_lock(&v.id);

        let err = delete_vault_for_user(&s, &uid, &v.id).await.unwrap_err();

        assert_eq!(err.status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(s.vault_push_lock_count_for_tests(), 1);
        drop(lock);
        s.remove_vault_push_lock(&v.id);
        assert_eq!(s.vault_push_lock_count_for_tests(), 0);
    }
}
