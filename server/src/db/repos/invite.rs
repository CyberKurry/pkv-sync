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
    async fn mark_used(&self, code: &str, used_by: &str, ts: i64) -> Result<bool, sqlx::Error>;
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
    let h: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    format!("inv_{h}")
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

    /// Atomic claim. Returns true if claim succeeded; false if already used or expired.
    async fn mark_used(&self, code: &str, used_by: &str, ts: i64) -> Result<bool, sqlx::Error> {
        let r = sqlx::query(
            "UPDATE invites SET used_at = ?, used_by = ?
             WHERE code = ? AND used_at IS NULL AND (expires_at IS NULL OR expires_at > ?)",
        )
        .bind(ts)
        .bind(used_by)
        .bind(code)
        .bind(ts)
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected() == 1)
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
    use crate::db::repos::{NewUser, SqliteUserRepo, UserRepo};

    async fn setup() -> (SqliteInviteRepo, String) {
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
        (SqliteInviteRepo::new(p), admin.id)
    }

    #[tokio::test]
    async fn create_then_find() {
        let (repo, admin) = setup().await;
        let inv = repo.create(&admin, Some(9999999999)).await.unwrap();
        assert!(inv.code.starts_with("inv_"));
        let found = repo.find(&inv.code).await.unwrap().unwrap();
        assert_eq!(found.code, inv.code);
    }

    #[tokio::test]
    async fn mark_used_first_succeeds_second_fails() {
        let (repo, admin) = setup().await;
        let inv = repo.create(&admin, None).await.unwrap();
        assert!(repo.mark_used(&inv.code, &admin, 100).await.unwrap());
        assert!(!repo.mark_used(&inv.code, &admin, 200).await.unwrap());
    }

    #[tokio::test]
    async fn list_active_excludes_used() {
        let (repo, admin) = setup().await;
        let i1 = repo.create(&admin, None).await.unwrap();
        let _i2 = repo.create(&admin, None).await.unwrap();
        repo.mark_used(&i1.code, &admin, 1).await.unwrap();
        let active = repo.list_active(0).await.unwrap();
        assert_eq!(active.len(), 1);
    }

    #[tokio::test]
    async fn list_active_excludes_expired() {
        let (repo, admin) = setup().await;
        let _i = repo.create(&admin, Some(50)).await.unwrap();
        let active = repo.list_active(100).await.unwrap();
        assert_eq!(active.len(), 0);
    }

    #[tokio::test]
    async fn cannot_use_expired() {
        let (repo, admin) = setup().await;
        let inv = repo.create(&admin, Some(50)).await.unwrap();
        assert!(!repo.mark_used(&inv.code, &admin, 100).await.unwrap());
    }
}
