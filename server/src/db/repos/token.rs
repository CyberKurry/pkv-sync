use async_trait::async_trait;
use serde::Serialize;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct TokenRow {
    pub id: String,
    pub user_id: String,
    pub device_name: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub revoked_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewToken<'a> {
    pub user_id: &'a str,
    pub token_hash: &'a str,
    pub device_name: &'a str,
}

#[async_trait]
pub trait TokenRepo: Send + Sync {
    async fn create(&self, n: NewToken<'_>) -> Result<TokenRow, sqlx::Error>;
    /// Look up an unrevoked token by its hash.
    async fn find_by_hash(&self, hash: &str) -> Result<Option<(TokenRow, String)>, sqlx::Error>;
    async fn list_for_user(&self, user_id: &str) -> Result<Vec<TokenRow>, sqlx::Error>;
    async fn touch_used(&self, id: &str, ts: i64) -> Result<(), sqlx::Error>;
    async fn revoke(&self, id: &str, ts: i64) -> Result<(), sqlx::Error>;
    async fn revoke_all_for_user(
        &self,
        user_id: &str,
        ts: i64,
        except: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn delete_revoked_older_than(&self, before_ts: i64) -> Result<u64, sqlx::Error>;
}

pub struct SqliteTokenRepo {
    pool: SqlitePool,
}

impl SqliteTokenRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TokenRepo for SqliteTokenRepo {
    async fn create(&self, n: NewToken<'_>) -> Result<TokenRow, sqlx::Error> {
        let id = Uuid::new_v4().simple().to_string();
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO tokens (id, user_id, token_hash, device_name, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(n.user_id)
        .bind(n.token_hash)
        .bind(n.device_name)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(TokenRow {
            id,
            user_id: n.user_id.into(),
            device_name: n.device_name.into(),
            created_at: now,
            last_used_at: None,
            revoked_at: None,
        })
    }

    async fn find_by_hash(&self, hash: &str) -> Result<Option<(TokenRow, String)>, sqlx::Error> {
        let row: Option<(
            String,
            String,
            String,
            String,
            i64,
            Option<i64>,
            Option<i64>,
        )> = sqlx::query_as(
            "SELECT id, user_id, token_hash, device_name, created_at, last_used_at, revoked_at
                 FROM tokens WHERE token_hash = ? AND revoked_at IS NULL",
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|t| {
            (
                TokenRow {
                    id: t.0,
                    user_id: t.1.clone(),
                    device_name: t.3,
                    created_at: t.4,
                    last_used_at: t.5,
                    revoked_at: t.6,
                },
                t.1,
            )
        }))
    }

    async fn list_for_user(&self, user_id: &str) -> Result<Vec<TokenRow>, sqlx::Error> {
        let rows: Vec<(String, String, String, i64, Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT id, user_id, device_name, created_at, last_used_at, revoked_at
             FROM tokens WHERE user_id = ? ORDER BY created_at DESC, id DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|t| TokenRow {
                id: t.0,
                user_id: t.1,
                device_name: t.2,
                created_at: t.3,
                last_used_at: t.4,
                revoked_at: t.5,
            })
            .collect())
    }

    async fn touch_used(&self, id: &str, ts: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE tokens SET last_used_at = ? WHERE id = ?")
            .bind(ts)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn revoke(&self, id: &str, ts: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE tokens SET revoked_at = ? WHERE id = ? AND revoked_at IS NULL")
            .bind(ts)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn revoke_all_for_user(
        &self,
        user_id: &str,
        ts: i64,
        except: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        match except {
            Some(skip) => {
                sqlx::query(
                    "UPDATE tokens SET revoked_at = ?
                     WHERE user_id = ? AND id != ? AND revoked_at IS NULL",
                )
                .bind(ts)
                .bind(user_id)
                .bind(skip)
                .execute(&self.pool)
                .await?;
            }
            None => {
                sqlx::query(
                    "UPDATE tokens SET revoked_at = ?
                     WHERE user_id = ? AND revoked_at IS NULL",
                )
                .bind(ts)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(())
    }

    async fn delete_revoked_older_than(&self, before_ts: i64) -> Result<u64, sqlx::Error> {
        let r = sqlx::query("DELETE FROM tokens WHERE revoked_at IS NOT NULL AND revoked_at < ?")
            .bind(before_ts)
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

    async fn setup() -> (SqliteUserRepo, SqliteTokenRepo, String) {
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let users = SqliteUserRepo::new(p.clone());
        let tokens = SqliteTokenRepo::new(p);
        let u = users
            .create(NewUser {
                username: "u".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        (users, tokens, u.id)
    }

    #[tokio::test]
    async fn create_and_find_by_hash() {
        let (_users, tokens, uid) = setup().await;
        tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "abc",
                device_name: "iphone",
            })
            .await
            .unwrap();
        let found = tokens.find_by_hash("abc").await.unwrap();
        let (row, user_id) = found.unwrap();
        assert_eq!(user_id, uid);
        assert_eq!(row.device_name, "iphone");
    }

    #[tokio::test]
    async fn revoked_token_not_returned() {
        let (_users, tokens, uid) = setup().await;
        let row = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "k",
                device_name: "d",
            })
            .await
            .unwrap();
        tokens.revoke(&row.id, 1).await.unwrap();
        assert!(tokens.find_by_hash("k").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn revoke_all_except_keeps_one() {
        let (_users, tokens, uid) = setup().await;
        let a = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "a",
                device_name: "1",
            })
            .await
            .unwrap();
        let _b = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "b",
                device_name: "2",
            })
            .await
            .unwrap();
        let _c = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "c",
                device_name: "3",
            })
            .await
            .unwrap();
        tokens
            .revoke_all_for_user(&uid, 999, Some(&a.id))
            .await
            .unwrap();
        let live: Vec<_> = tokens
            .list_for_user(&uid)
            .await
            .unwrap()
            .into_iter()
            .filter(|t| t.revoked_at.is_none())
            .collect();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].id, a.id);
    }

    #[tokio::test]
    async fn touch_used_updates_timestamp() {
        let (_users, tokens, uid) = setup().await;
        let row = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "z",
                device_name: "d",
            })
            .await
            .unwrap();
        tokens.touch_used(&row.id, 42).await.unwrap();
        let listed = tokens.list_for_user(&uid).await.unwrap();
        assert_eq!(listed[0].last_used_at, Some(42));
    }

    #[tokio::test]
    async fn delete_revoked_older() {
        let (_users, tokens, uid) = setup().await;
        let row = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "old",
                device_name: "d",
            })
            .await
            .unwrap();
        tokens.revoke(&row.id, 100).await.unwrap();
        let n = tokens.delete_revoked_older_than(200).await.unwrap();
        assert_eq!(n, 1);
    }
}
