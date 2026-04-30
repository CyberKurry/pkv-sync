use async_trait::async_trait;
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize)]
pub struct NewActivity<'a> {
    pub user_id: &'a str,
    pub vault_id: Option<&'a str>,
    pub token_id: Option<&'a str>,
    pub action: &'a str,
    pub commit_hash: Option<&'a str>,
    pub client_ip: Option<&'a str>,
    pub user_agent: Option<&'a str>,
    pub details: Option<&'a str>,
}

#[async_trait]
pub trait SyncActivityRepo: Send + Sync {
    async fn insert(&self, a: NewActivity<'_>) -> Result<(), sqlx::Error>;
    async fn delete_older_than(&self, ts: i64) -> Result<u64, sqlx::Error>;
}

pub struct SqliteSyncActivityRepo {
    pool: SqlitePool,
}

impl SqliteSyncActivityRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SyncActivityRepo for SqliteSyncActivityRepo {
    async fn insert(&self, a: NewActivity<'_>) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO sync_activity
             (user_id, vault_id, token_id, action, commit_hash, client_ip, user_agent, timestamp, details)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(a.user_id)
        .bind(a.vault_id)
        .bind(a.token_id)
        .bind(a.action)
        .bind(a.commit_hash)
        .bind(a.client_ip)
        .bind(a.user_agent)
        .bind(chrono::Utc::now().timestamp())
        .bind(a.details)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_older_than(&self, ts: i64) -> Result<u64, sqlx::Error> {
        let r = sqlx::query("DELETE FROM sync_activity WHERE timestamp < ?")
            .bind(ts)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::{NewUser, SqliteUserRepo, UserRepo};

    #[tokio::test]
    async fn insert_activity() {
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let users = SqliteUserRepo::new(p.clone());
        let u = users
            .create(NewUser {
                username: "u".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let repo = SqliteSyncActivityRepo::new(p.clone());
        repo.insert(NewActivity {
            user_id: &u.id,
            vault_id: None,
            token_id: None,
            action: "login",
            commit_hash: None,
            client_ip: Some("127.0.0.1"),
            user_agent: Some("PKVSync-Plugin/0.1.0"),
            details: None,
        })
        .await
        .unwrap();
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sync_activity")
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(n, 1);
        assert_eq!(
            repo.delete_older_than(chrono::Utc::now().timestamp() + 1)
                .await
                .unwrap(),
            1
        );
    }
}
