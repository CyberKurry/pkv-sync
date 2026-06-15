use crate::admin::session;
use crate::db::repos::{IdempotencyRepo, SyncActivityRepo, TokenRepo};
use crate::service::AppState;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const MAX_CONCURRENT_VAULT_RECONCILES: usize = 4;

#[derive(Debug, Serialize)]
pub struct CleanupReport {
    pub sessions_deleted: u64,
    pub tokens_deleted: u64,
    pub activity_deleted: u64,
    pub idempotency_deleted: u64,
    pub vaults_reconciled: usize,
    pub vault_reconcile_failed: usize,
    pub blobs_deleted: usize,
    pub git_gc_pruned: usize,
    pub git_gc_failed: usize,
}

pub async fn run_scheduled_cleanup(state: &AppState) -> CleanupReport {
    let now = chrono::Utc::now().timestamp();
    let thirty_days_ago = now - 30 * 24 * 60 * 60;
    let one_day_ago = now - 24 * 60 * 60;

    let sessions_deleted = session::cleanup_expired_sessions(state)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "session cleanup failed"))
        .unwrap_or(0);
    let tokens_deleted = state
        .tokens
        .delete_revoked_older_than(thirty_days_ago)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "revoked token cleanup failed"))
        .unwrap_or(0);
    let activity_deleted = state
        .activities
        .delete_older_than(thirty_days_ago)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "sync activity cleanup failed"))
        .unwrap_or(0);
    let idempotency_deleted = state
        .idempotency
        .delete_older_than(one_day_ago)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "idempotency cleanup failed"))
        .unwrap_or(0);

    let (vaults_reconciled, vault_reconcile_failed) = reconcile_all_vaults(state).await;

    let blobs_deleted = match crate::service::gc::run_blob_gc(state).await {
        Ok(report) => report.deleted,
        Err(e) => {
            tracing::warn!(error = %e.message, "blob cleanup failed");
            0
        }
    };

    let (git_gc_pruned, git_gc_failed) = if state.git_available {
        git_gc_all_vaults(state).await
    } else {
        (0, 0)
    };

    CleanupReport {
        sessions_deleted,
        tokens_deleted,
        activity_deleted,
        idempotency_deleted,
        vaults_reconciled,
        vault_reconcile_failed,
        blobs_deleted,
        git_gc_pruned,
        git_gc_failed,
    }
}

async fn reconcile_all_vaults(state: &AppState) -> (usize, usize) {
    let rows: Vec<(String,)> = match sqlx::query_as("SELECT id FROM vaults")
        .fetch_all(&state.pool)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(error = %e, "vault metadata reconcile list failed");
            return (0, 0);
        }
    };
    let mut ok = 0;
    let mut failed = 0;

    let limit = Arc::new(Semaphore::new(MAX_CONCURRENT_VAULT_RECONCILES));
    let mut tasks = JoinSet::new();

    for (vault_id,) in rows {
        let state = state.clone();
        let limit = Arc::clone(&limit);
        tasks.spawn(async move {
            let _permit = limit
                .acquire_owned()
                .await
                .expect("vault reconcile semaphore should remain open");
            match crate::service::sync::reconcile_vault_metadata(&state, &vault_id).await {
                Ok(_) => true,
                Err(e) => {
                    tracing::warn!(
                        vault_id = %vault_id,
                        error = %e.message,
                        "vault metadata reconcile failed"
                    );
                    false
                }
            }
        });
    }

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(true) => ok += 1,
            Ok(false) => failed += 1,
            Err(e) => {
                failed += 1;
                tracing::warn!(error = %e, "vault metadata reconcile task failed");
            }
        }
    }
    (ok, failed)
}

async fn git_gc_all_vaults(state: &AppState) -> (usize, usize) {
    let rows: Vec<(String,)> = match sqlx::query_as("SELECT id FROM vaults")
        .fetch_all(&state.pool)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(error = %e, "vault git gc list failed");
            return (0, 0);
        }
    };
    let mut ok = 0;
    let mut failed = 0;

    let limit = Arc::new(Semaphore::new(MAX_CONCURRENT_VAULT_RECONCILES));
    let mut tasks = JoinSet::new();

    for (vault_id,) in rows {
        let state = state.clone();
        let limit = Arc::clone(&limit);
        tasks.spawn(async move {
            let _permit = limit
                .acquire_owned()
                .await
                .expect("vault git gc semaphore should remain open");
            let push_lock = state.vault_push_lock(&vault_id);
            let _push_guard = push_lock.lock().await;
            let _storage_guard = match crate::service::acquire_storage_mutation_guard(&state).await
            {
                Ok(guard) => guard,
                Err(e) => {
                    tracing::warn!(
                        vault_id = %vault_id,
                        error = %e.message,
                        "vault git gc storage lock failed"
                    );
                    return false;
                }
            };
            match state.git_store().gc_prune_unreachable(&vault_id).await {
                Ok(_) => true,
                Err(e) => {
                    tracing::warn!(
                        vault_id = %vault_id,
                        error = %e,
                        "vault git gc failed"
                    );
                    false
                }
            }
        });
    }

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(true) => ok += 1,
            Ok(false) => failed += 1,
            Err(e) => {
                failed += 1;
                tracing::warn!(error = %e, "vault git gc task failed");
            }
        }
    }
    (ok, failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::{NewActivity, NewUser, UserRepo};
    use crate::storage::git::{FileChange, GitVaultStore, StoredFile};
    use git2::{Oid, Repository};

    async fn state_for_cleanup() -> (AppState, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect(&tmp.path().join("metadata.db"))
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap();
        (state, tmp)
    }

    #[tokio::test]
    async fn cleanup_deletes_expired_session() {
        let (state, _tmp) = state_for_cleanup().await;
        let user = state
            .users
            .create(NewUser {
                username: "u".into(),
                password_hash: "h".into(),
                is_admin: true,
            })
            .await
            .unwrap();

        // Insert an already-expired session directly
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO admin_sessions (id, user_id, created_at, expires_at, last_seen_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind("s_expired")
        .bind(&user.id)
        .bind(now - 100)
        .bind(now - 1) // already expired
        .bind(now - 100)
        .execute(&state.pool)
        .await
        .unwrap();

        let report = run_scheduled_cleanup(&state).await;
        assert_eq!(report.sessions_deleted, 1);
    }

    #[tokio::test]
    async fn cleanup_prunes_old_idempotency_entries() {
        let (state, _tmp) = state_for_cleanup().await;

        // Insert directly with a past timestamp so it's definitely older than the retention window.
        let past = chrono::Utc::now().timestamp() - 2 * 24 * 60 * 60;
        sqlx::query(
            "INSERT INTO idempotency_cache (user_id, key, vault_id, route, request_hash, response_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("user1")
        .bind("key1")
        .bind("vault1")
        .bind("push")
        .bind("hash1")
        .bind("{}")
        .bind(past)
        .execute(&state.pool)
        .await
        .unwrap();

        let report = run_scheduled_cleanup(&state).await;
        assert_eq!(report.idempotency_deleted, 1);
    }

    #[tokio::test]
    async fn cleanup_keeps_recent_idempotency_entries() {
        let (state, _tmp) = state_for_cleanup().await;

        let recent = chrono::Utc::now().timestamp() - 10;
        sqlx::query(
            "INSERT INTO idempotency_cache (user_id, key, vault_id, route, request_hash, response_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("user1")
        .bind("recent-key")
        .bind("vault1")
        .bind("push")
        .bind("hash1")
        .bind("{}")
        .bind(recent)
        .execute(&state.pool)
        .await
        .unwrap();

        let report = run_scheduled_cleanup(&state).await;

        assert_eq!(report.idempotency_deleted, 0);
    }

    #[tokio::test]
    async fn scheduled_reconcile_waits_for_vault_push_lock() {
        let (state, _tmp) = state_for_cleanup().await;
        let user = state
            .users
            .create(NewUser {
                username: "u".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let vault = crate::service::vault::create_vault(&state, &user.id, "main")
            .await
            .unwrap();
        let lock = state.vault_push_lock(&vault.id);
        let guard = lock.lock().await;

        let state_for_task = state.clone();
        let cleanup = tokio::spawn(async move { run_scheduled_cleanup(&state_for_task).await });

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), async {
                loop {
                    if cleanup.is_finished() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .is_err(),
            "cleanup should wait for the in-flight push lock"
        );

        drop(guard);
        let report = cleanup.await.unwrap();
        assert_eq!(report.vaults_reconciled, 1);
        assert_eq!(report.vault_reconcile_failed, 0);
    }

    #[tokio::test]
    async fn cleanup_prunes_unreachable_git_commits() {
        let (state, _tmp) = state_for_cleanup().await;
        let user = state
            .users
            .create(NewUser {
                username: "u".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let vault = crate::service::vault::create_vault(&state, &user.id, "main")
            .await
            .unwrap();
        let git = state.git_store();
        let base = git
            .commit_changes(
                &vault.id,
                None,
                &[FileChange::Upsert {
                    path: "note.md".into(),
                    file: StoredFile::Text {
                        bytes: b"base".to_vec(),
                    },
                }],
                "base",
            )
            .await
            .unwrap();
        let orphan = git
            .commit_changes(
                &vault.id,
                Some(&base),
                &[FileChange::Upsert {
                    path: "note.md".into(),
                    file: StoredFile::Text {
                        bytes: b"orphaned".to_vec(),
                    },
                }],
                "orphan",
            )
            .await
            .unwrap();
        git.set_main_ref(&vault.id, &base, "test rollback")
            .await
            .unwrap();

        let repo_path = state.vault_root().join(&vault.id);
        let orphan_oid = Oid::from_str(&orphan).unwrap();
        {
            let repo = Repository::open_bare(&repo_path).unwrap();
            assert!(repo.find_commit(orphan_oid).is_ok());
        }

        let _report = run_scheduled_cleanup(&state).await;

        let repo = Repository::open_bare(&repo_path).unwrap();
        let err = repo
            .find_commit(orphan_oid)
            .expect_err("scheduled cleanup should prune orphan git commits");
        assert_eq!(err.code(), git2::ErrorCode::NotFound);
    }

    #[test]
    fn reconcile_all_vaults_uses_bounded_concurrency() {
        let source = include_str!("cleanup.rs");
        let start = source
            .find("async fn reconcile_all_vaults")
            .expect("reconcile_all_vaults exists");
        let end = source[start..]
            .find("\n#[cfg(test)]")
            .expect("test module follows reconcile_all_vaults");
        let function = &source[start..start + end];

        assert!(
            function.contains("MAX_CONCURRENT_VAULT_RECONCILES"),
            "reconcile_all_vaults should use a named concurrency limit"
        );
        assert!(
            function.contains("Semaphore") || function.contains("for_each_concurrent"),
            "reconcile_all_vaults should use a bounded concurrency primitive"
        );
        assert!(
            !function.contains("reconcile_vault_metadata(state, &vault_id).await"),
            "reconcile_all_vaults should not await vault reconciliation sequentially"
        );
    }

    #[tokio::test]
    async fn cleanup_prunes_old_sync_activity() {
        let (state, _tmp) = state_for_cleanup().await;
        let user = state
            .users
            .create(NewUser {
                username: "u".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();

        state
            .activities
            .insert(NewActivity {
                user_id: &user.id,
                vault_id: None,
                token_id: None,
                action: "push",
                commit_hash: None,
                client_ip: None,
                user_agent: None,
                details: None,
            })
            .await
            .unwrap();

        let report = run_scheduled_cleanup(&state).await;
        assert_eq!(report.activity_deleted, 0); // recent activity kept
    }
}
