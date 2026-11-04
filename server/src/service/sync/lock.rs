use crate::service::AppState;

#[derive(Debug)]
pub(crate) struct VaultLockAcquireError {
    pub timeout: std::time::Duration,
    /// Pre-rendered: "vault {id}: push lock busy for over {t:?}, {action} skipped"
    pub message: String,
}

/// E-1: the one bounded push-lock acquisition. Five call sites used to
/// each hand-roll timeout + acquire + their own message (push 503
/// vault_busy; reconcile internal skip; cleanup git-gc warn skip;
/// rollback and delete internal skip). The shared producer owns the
/// timeout (vault_push_lock_timeout: 50ms test / 30s production) and the
/// message rendering; each call site keeps its error MAPPING so every
/// status code and message stays byte-identical. Both variants return an
/// OwnedMutexGuard (the lock lives in an Arc in AppState, so an owned
/// guard is the borrow-free shape every call site can hold across
/// awaits).
pub(crate) async fn acquire_vault_push_lock_bounded(
    state: &AppState,
    vault_id: &str,
    action: &'static str,
) -> Result<tokio::sync::OwnedMutexGuard<()>, VaultLockAcquireError> {
    let timeout = super::vault_push_lock_timeout();
    let lock = state.vault_push_lock(vault_id);
    match tokio::time::timeout(timeout, lock.lock_owned()).await {
        Ok(guard) => Ok(guard),
        Err(_) => Err(VaultLockAcquireError {
            timeout,
            message: format!(
                "vault {vault_id}: push lock busy for over {timeout:?}, {action} skipped"
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;

    async fn test_state() -> (AppState, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into(), true)
            .await
            .unwrap();
        (state, tmp)
    }

    #[tokio::test]
    async fn bounded_acquire_times_out_with_action_in_message() {
        let (state, _tmp) = test_state().await;
        let lock = state.vault_push_lock("v1");
        let _held = lock.lock_owned().await;
        let err = match acquire_vault_push_lock_bounded(&state, "v1", "reconcile").await {
            Ok(_) => panic!("must time out (50ms test timeout)"),
            Err(e) => e,
        };
        assert!(err.message.contains("vault v1"), "{}", err.message);
        assert!(err.message.contains("reconcile skipped"), "{}", err.message);
        assert_eq!(err.timeout, std::time::Duration::from_millis(50));
    }

    #[tokio::test]
    async fn acquire_returns_guard_when_free() {
        let (state, _tmp) = test_state().await;
        let guard = acquire_vault_push_lock_bounded(&state, "v2", "push")
            .await
            .expect("free lock acquires");
        drop(guard);
    }
}
