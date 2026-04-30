use crate::admin::session;
use crate::db::repos::{IdempotencyRepo, SyncActivityRepo, TokenRepo};
use crate::service::AppState;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CleanupReport {
    pub sessions_deleted: u64,
    pub tokens_deleted: u64,
    pub activity_deleted: u64,
    pub idempotency_deleted: u64,
    pub vaults_reconciled: usize,
    pub vault_reconcile_failed: usize,
    pub blobs_deleted: usize,
}

pub async fn run_scheduled_cleanup(state: &AppState) -> CleanupReport {
    let now = chrono::Utc::now().timestamp();
    let thirty_days_ago = now - 30 * 24 * 60 * 60;

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
        .delete_older_than(now)
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

    CleanupReport {
        sessions_deleted,
        tokens_deleted,
        activity_deleted,
        idempotency_deleted,
        vaults_reconciled,
        vault_reconcile_failed,
        blobs_deleted,
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
    for (vault_id,) in rows {
        match crate::service::sync::reconcile_vault_metadata(state, &vault_id).await {
            Ok(_) => ok += 1,
            Err(e) => {
                failed += 1;
                tracing::warn!(
                    vault_id = %vault_id,
                    error = %e.message,
                    "vault metadata reconcile failed"
                );
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

    async fn state_for_cleanup() -> (AppState, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect(&tmp.path().join("metadata.db"))
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into())
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

        // Insert directly with a past timestamp so it's definitely older than now
        let past = chrono::Utc::now().timestamp() - 10;
        sqlx::query(
            "INSERT INTO idempotency_cache (key, user_id, response_json, created_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind("key1")
        .bind("user1")
        .bind("{}")
        .bind(past)
        .execute(&state.pool)
        .await
        .unwrap();

        let report = run_scheduled_cleanup(&state).await;
        assert_eq!(report.idempotency_deleted, 1);
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
