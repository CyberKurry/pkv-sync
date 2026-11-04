use async_trait::async_trait;
use rand::{rngs::OsRng, RngCore};
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize)]
pub struct Invite {
    pub code: String,
    pub created_by: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub used_at: Option<i64>,
    pub used_by: Option<String>,
}

#[async_trait]
pub trait InviteRepo: Send + Sync {
    async fn create(
        &self,
        created_by: &str,
        expires_at: Option<i64>,
    ) -> Result<Invite, sqlx::Error>;
    async fn find(&self, code: &str) -> Result<Option<Invite>, sqlx::Error>;
    async fn list_active(&self, now_ts: i64) -> Result<Vec<Invite>, sqlx::Error>;
    async fn count_used(&self) -> Result<usize, sqlx::Error>;
    async fn delete(&self, code: &str) -> Result<(), sqlx::Error>;
}

pub struct SqliteInviteRepo {
    pool: SqlitePool,
}

impl SqliteInviteRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn random_code() -> String {
    let mut buf = [0u8; 16];
    OsRng.fill_bytes(&mut buf);
    format!("inv_{}", hex::encode(buf))
}

#[async_trait]
impl InviteRepo for SqliteInviteRepo {
    async fn create(
        &self,
        created_by: &str,
        expires_at: Option<i64>,
    ) -> Result<Invite, sqlx::Error> {
        let code = random_code();
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO invites (code, created_by, created_at, expires_at) VALUES (?, ?, ?, ?)",
        )
        .bind(&code)
        .bind(created_by)
        .bind(now)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(Invite {
            code,
            created_by: created_by.into(),
            created_at: now,
            expires_at,
            used_at: None,
            used_by: None,
        })
    }

    async fn find(&self, code: &str) -> Result<Option<Invite>, sqlx::Error> {
        let r: Option<(
            String,
            String,
            i64,
            Option<i64>,
            Option<i64>,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT code, created_by, created_at, expires_at, used_at, used_by
                 FROM invites WHERE code = ?",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await?;
        Ok(r.map(|t| Invite {
            code: t.0,
            created_by: t.1,
            created_at: t.2,
            expires_at: t.3,
            used_at: t.4,
            used_by: t.5,
        }))
    }

    async fn list_active(&self, now_ts: i64) -> Result<Vec<Invite>, sqlx::Error> {
        let rows: Vec<(
            String,
            String,
            i64,
            Option<i64>,
            Option<i64>,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT code, created_by, created_at, expires_at, used_at, used_by
                 FROM invites
                 WHERE used_at IS NULL AND (expires_at IS NULL OR expires_at > ?)
                 ORDER BY created_at DESC, code DESC",
        )
        .bind(now_ts)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|t| Invite {
                code: t.0,
                created_by: t.1,
                created_at: t.2,
                expires_at: t.3,
                used_at: t.4,
                used_by: t.5,
            })
            .collect())
    }

    async fn count_used(&self) -> Result<usize, sqlx::Error> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM invites WHERE used_at IS NOT NULL")
                .fetch_one(&self.pool)
                .await?;
        Ok(count as usize)
    }

    async fn delete(&self, code: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM invites WHERE code = ?")
            .bind(code)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::{
        NewUser, RegistrationMode, RuntimeConfigRepo, SqliteUserRepo, UserRepo,
    };

    async fn setup() -> (
        SqliteInviteRepo,
        SqliteUserRepo,
        String,
        crate::service::AppState,
    ) {
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let users = SqliteUserRepo::new(p.clone());
        let admin = users
            .create(NewUser {
                username: "a".into(),
                password_hash: "h".into(),
                is_admin: true,
            })
            .await
            .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let state = crate::service::AppState::new(p, tmp.path().to_path_buf(), "t".into(), true)
            .await
            .unwrap();
        state
            .runtime_cfg_repo
            .set_registration_mode(RegistrationMode::InviteOnly, None)
            .await
            .unwrap();
        let cfg = state.runtime_cfg_repo.load().await.unwrap();
        state.runtime_cfg.replace(cfg).await;
        (
            SqliteInviteRepo::new(state.pool.clone()),
            users,
            admin.id,
            state,
        )
    }

    #[tokio::test]
    async fn create_then_find() {
        let (repo, _users, admin, _state) = setup().await;
        let inv = repo.create(&admin, Some(9999999999)).await.unwrap();
        assert!(inv.code.starts_with("inv_"));
        let found = repo.find(&inv.code).await.unwrap().unwrap();
        assert_eq!(found.code, inv.code);
    }

    #[tokio::test]
    async fn deleting_invite_creator_cascades_to_invite() {
        let (repo, users, admin, _state) = setup().await;
        let inv = repo.create(&admin, None).await.unwrap();

        users.delete(&admin).await.unwrap();

        assert!(repo.find(&inv.code).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn deleting_invite_user_nulls_used_by() {
        let (repo, users, admin, state) = setup().await;
        let inv = repo.create(&admin, None).await.unwrap();
        // Claim via the production register path (the only claim producer).
        let invitee = crate::service::auth::register(
            &state,
            crate::service::auth::RegisterReq {
                username: "invitee".into(),
                password: "Passw0rdStrong".into(),
                device_id: "d-inv".into(),
                device_name: "d".into(),
                invite_code: Some(inv.code.clone()),
            },
        )
        .await
        .unwrap();
        let claimed_at = repo
            .find(&inv.code)
            .await
            .unwrap()
            .unwrap()
            .used_at
            .expect("register claims the invite");

        users.delete(&invitee.user_id).await.unwrap();

        let found = repo.find(&inv.code).await.unwrap().unwrap();
        assert_eq!(found.used_at, Some(claimed_at));
        assert!(found.used_by.is_none());
    }

    #[tokio::test]
    async fn register_claims_each_invite_once() {
        // Was mark_used_first_succeeds_second_fails: used_at idempotency is
        // a DB guarantee the register transaction relies on — pinned here
        // through the production claim path.
        let (repo, _users, admin, state) = setup().await;
        let inv = repo.create(&admin, None).await.unwrap();
        let req = |name: String| crate::service::auth::RegisterReq {
            username: name,
            password: "Passw0rdStrong".into(),
            device_id: "d-claim".into(),
            device_name: "d".into(),
            invite_code: Some(inv.code.clone()),
        };
        crate::service::auth::register(&state, req("first".into()))
            .await
            .unwrap();
        let second = crate::service::auth::register(&state, req("second".into())).await;
        let err = second.unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::BAD_REQUEST);
        assert_eq!(err.code, "invalid_invite");
    }

    #[tokio::test]
    async fn register_rejects_expired_invites() {
        // Was cannot_use_expired: the claim's expiry predicate.
        let (repo, _users, admin, state) = setup().await;
        let inv = repo.create(&admin, Some(50)).await.unwrap();
        let err = crate::service::auth::register(
            &state,
            crate::service::auth::RegisterReq {
                username: "late".into(),
                password: "Passw0rdStrong".into(),
                device_id: "d-exp".into(),
                device_name: "d".into(),
                invite_code: Some(inv.code),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "invalid_invite");
    }

    #[tokio::test]
    async fn count_used_sees_only_claims() {
        // Was count_used_includes_used_invites_excluded_from_active_list.
        let (repo, _users, admin, state) = setup().await;
        let inv = repo.create(&admin, None).await.unwrap();
        crate::service::auth::register(
            &state,
            crate::service::auth::RegisterReq {
                username: "claimed".into(),
                password: "Passw0rdStrong".into(),
                device_id: "d-cnt".into(),
                device_name: "d".into(),
                invite_code: Some(inv.code),
            },
        )
        .await
        .unwrap();

        assert_eq!(
            repo.list_active(chrono::Utc::now().timestamp())
                .await
                .unwrap()
                .len(),
            0
        );
        assert_eq!(repo.count_used().await.unwrap(), 1);
    }
}
