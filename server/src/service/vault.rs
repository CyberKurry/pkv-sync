use crate::api::error::ApiError;
use crate::db::repos::{Vault, VaultRepo};
use crate::service::AppState;

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
    if state
        .vaults
        .list_for_user(user_id)
        .await?
        .iter()
        .any(|v| v.name == name)
    {
        return Err(ApiError::conflict(
            "vault_name_taken",
            "vault name already exists",
        ));
    }
    Ok(state.vaults.create(user_id, name).await?)
}

pub async fn delete_vault_for_user(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
) -> Result<bool, ApiError> {
    let deleted = state.vaults.delete_for_user(user_id, vault_id).await?;
    if !deleted {
        return Ok(false);
    }
    state.remove_vault_push_lock(vault_id).await;
    remove_vault_storage(state, vault_id).await?;
    Ok(true)
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

    async fn state_and_user() -> (AppState, String, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let state = AppState::new(p, tmp.path().to_path_buf(), "t".into())
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
    async fn duplicate_name_conflicts() {
        let (s, uid, _tmp) = state_and_user().await;
        create_vault(&s, &uid, "main").await.unwrap();
        let err = create_vault(&s, &uid, "main").await.unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::CONFLICT);
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
        let _ = s.vault_push_lock(&v.id).await;

        assert!(delete_vault_for_user(&s, &uid, &v.id).await.unwrap());

        assert!(s.vaults.find_by_id(&v.id).await.unwrap().is_none());
        assert!(!tokio::fs::try_exists(&repo_dir).await.unwrap());
        assert_eq!(s.vault_push_lock_count_for_tests().await, 0);
    }
}
