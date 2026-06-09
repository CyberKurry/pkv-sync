use crate::auth::token;
use async_trait::async_trait;
use serde::Serialize;
use sqlx::{Executor, Sqlite, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct TokenRow {
    pub id: String,
    pub user_id: String,
    pub device_id: String,
    pub device_name: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub last_used_at: Option<i64>,
    pub revoked_at: Option<i64>,
}

type TokenRowTuple = (
    String,
    String,
    String,
    String,
    i64,
    i64,
    Option<i64>,
    Option<i64>,
);

#[derive(Debug, Clone)]
pub struct NewToken<'a> {
    pub user_id: &'a str,
    pub token_hash: &'a str,
    pub device_id: &'a str,
    pub device_name: &'a str,
}

#[async_trait]
pub trait TokenRepo: Send + Sync {
    async fn create(&self, n: NewToken<'_>) -> Result<TokenRow, sqlx::Error>;
    /// Look up an unrevoked token by its hash.
    async fn find_by_hash(&self, hash: &str) -> Result<Option<(TokenRow, String)>, sqlx::Error>;
    async fn is_active_for_user(&self, id: &str, user_id: &str) -> Result<bool, sqlx::Error>;
    async fn find_by_id_for_user(
        &self,
        id: &str,
        user_id: &str,
    ) -> Result<Option<TokenRow>, sqlx::Error>;
    async fn list_for_user(&self, user_id: &str) -> Result<Vec<TokenRow>, sqlx::Error>;
    async fn list_active_for_user(&self, user_id: &str) -> Result<Vec<TokenRow>, sqlx::Error>;
    async fn touch_used(&self, id: &str, ts: i64) -> Result<(), sqlx::Error>;
    async fn revoke(&self, id: &str, ts: i64) -> Result<(), sqlx::Error>;
    async fn revoke_all_for_user(
        &self,
        user_id: &str,
        ts: i64,
        except: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn revoke_other_active_for_device(
        &self,
        user_id: &str,
        device_id: &str,
        ts: i64,
        except: &str,
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

    pub async fn create_replacing_device(&self, n: NewToken<'_>) -> Result<TokenRow, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let row = insert_token(&mut *tx, n).await?;
        sqlx::query(
            "UPDATE tokens SET revoked_at = ?
             WHERE user_id = ? AND device_id = ? AND id != ? AND revoked_at IS NULL",
        )
        .bind(row.created_at)
        .bind(&row.user_id)
        .bind(&row.device_id)
        .bind(&row.id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(row)
    }
}

async fn insert_token<'e, E>(executor: E, n: NewToken<'_>) -> Result<TokenRow, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let id = Uuid::new_v4().simple().to_string();
    let now = chrono::Utc::now().timestamp();
    let expires_at = now + token::TOKEN_TTL_SECONDS;
    sqlx::query(
        "INSERT INTO tokens (id, user_id, token_hash, device_id, device_name, created_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(n.user_id)
    .bind(n.token_hash)
    .bind(n.device_id)
    .bind(n.device_name)
    .bind(now)
    .bind(expires_at)
    .execute(executor)
    .await?;
    Ok(TokenRow {
        id,
        user_id: n.user_id.into(),
        device_id: n.device_id.into(),
        device_name: n.device_name.into(),
        created_at: now,
        expires_at,
        last_used_at: None,
        revoked_at: None,
    })
}

fn row_to_token_row(t: TokenRowTuple) -> TokenRow {
    TokenRow {
        id: t.0,
        user_id: t.1,
        device_id: t.2,
        device_name: t.3,
        created_at: t.4,
        expires_at: t.5,
        last_used_at: t.6,
        revoked_at: t.7,
    }
}

#[async_trait]
impl TokenRepo for SqliteTokenRepo {
    async fn create(&self, n: NewToken<'_>) -> Result<TokenRow, sqlx::Error> {
        insert_token(&self.pool, n).await
    }

    async fn find_by_hash(&self, hash: &str) -> Result<Option<(TokenRow, String)>, sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        let row: Option<(
            String,
            String,
            String,
            String,
            String,
            i64,
            i64,
            Option<i64>,
            Option<i64>,
        )> = sqlx::query_as(
            "SELECT id, user_id, token_hash, device_id, device_name, created_at, expires_at, last_used_at, revoked_at
                 FROM tokens
                 WHERE token_hash = ?
                   AND revoked_at IS NULL
                   AND expires_at > ?
                   AND created_at + ? > ?",
        )
        .bind(hash)
        .bind(now)
        .bind(token::TOKEN_ABSOLUTE_LIFETIME_SECONDS)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|t| {
            (
                TokenRow {
                    id: t.0,
                    user_id: t.1.clone(),
                    device_id: t.3,
                    device_name: t.4,
                    created_at: t.5,
                    expires_at: t.6,
                    last_used_at: t.7,
                    revoked_at: t.8,
                },
                t.1,
            )
        }))
    }

    async fn is_active_for_user(&self, id: &str, user_id: &str) -> Result<bool, sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM tokens
             WHERE id = ?
               AND user_id = ?
               AND revoked_at IS NULL
               AND expires_at > ?
               AND created_at + ? > ?",
        )
        .bind(id)
        .bind(user_id)
        .bind(now)
        .bind(token::TOKEN_ABSOLUTE_LIFETIME_SECONDS)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    async fn find_by_id_for_user(
        &self,
        id: &str,
        user_id: &str,
    ) -> Result<Option<TokenRow>, sqlx::Error> {
        let row: Option<TokenRowTuple> = sqlx::query_as(
            "SELECT id, user_id, device_id, device_name, created_at, expires_at, last_used_at, revoked_at
             FROM tokens
             WHERE id = ? AND user_id = ?",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(row_to_token_row))
    }

    async fn list_for_user(&self, user_id: &str) -> Result<Vec<TokenRow>, sqlx::Error> {
        let rows: Vec<TokenRowTuple> = sqlx::query_as(
            "SELECT id, user_id, device_id, device_name, created_at, expires_at, last_used_at, revoked_at
             FROM tokens WHERE user_id = ? ORDER BY created_at DESC, id DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(row_to_token_row).collect())
    }

    async fn list_active_for_user(&self, user_id: &str) -> Result<Vec<TokenRow>, sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        let rows: Vec<TokenRowTuple> = sqlx::query_as(
            "SELECT id, user_id, device_id, device_name, created_at, expires_at, last_used_at, revoked_at
             FROM tokens
             WHERE user_id = ?
               AND revoked_at IS NULL
               AND expires_at > ?
               AND created_at + ? > ?
             ORDER BY created_at DESC, id DESC",
        )
        .bind(user_id)
        .bind(now)
        .bind(token::TOKEN_ABSOLUTE_LIFETIME_SECONDS)
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(row_to_token_row).collect())
    }

    async fn touch_used(&self, id: &str, ts: i64) -> Result<(), sqlx::Error> {
        let expires_at = ts + token::TOKEN_TTL_SECONDS;
        sqlx::query(
            "UPDATE tokens
             SET last_used_at = ?,
                 expires_at = MIN(MAX(expires_at, ?), created_at + ?)
             WHERE id = ?",
        )
        .bind(ts)
        .bind(expires_at)
        .bind(token::TOKEN_ABSOLUTE_LIFETIME_SECONDS)
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

    async fn revoke_other_active_for_device(
        &self,
        user_id: &str,
        device_id: &str,
        ts: i64,
        except: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE tokens SET revoked_at = ?
             WHERE user_id = ? AND device_id = ? AND id != ? AND revoked_at IS NULL",
        )
        .bind(ts)
        .bind(user_id)
        .bind(device_id)
        .bind(except)
        .execute(&self.pool)
        .await?;
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

    fn assert_new_token_row(row: &TokenRow, user_id: &str, device_id: &str, device_name: &str) {
        assert_eq!(row.user_id, user_id);
        assert_eq!(row.device_id, device_id);
        assert_eq!(row.device_name, device_name);
        assert_eq!(row.expires_at, row.created_at + token::TOKEN_TTL_SECONDS);
        assert!(row.last_used_at.is_none());
        assert!(row.revoked_at.is_none());
    }

    #[tokio::test]
    async fn create_paths_return_equivalent_new_rows_and_replacing_revokes_old_device_token() {
        let (_users, tokens, uid) = setup().await;
        let plain = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "plain-equivalent",
                device_id: "device-plain-equivalent",
                device_name: "Plain Device",
            })
            .await
            .unwrap();
        let old_same_device = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "old-replaced",
                device_id: "device-replaced",
                device_name: "Old Device",
            })
            .await
            .unwrap();

        let replacing = tokens
            .create_replacing_device(NewToken {
                user_id: &uid,
                token_hash: "new-replacing",
                device_id: "device-replaced",
                device_name: "Replacement Device",
            })
            .await
            .unwrap();

        assert_new_token_row(&plain, &uid, "device-plain-equivalent", "Plain Device");
        assert_new_token_row(&replacing, &uid, "device-replaced", "Replacement Device");

        let rows = tokens.list_for_user(&uid).await.unwrap();
        let old_same_device = rows.iter().find(|t| t.id == old_same_device.id).unwrap();
        let replacing = rows.iter().find(|t| t.id == replacing.id).unwrap();
        assert!(old_same_device.revoked_at.is_some());
        assert!(replacing.revoked_at.is_none());
    }

    #[tokio::test]
    async fn create_and_find_by_hash() {
        let (_users, tokens, uid) = setup().await;
        tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "abc",
                device_id: "device-abc",
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
                device_id: "device-k",
                device_name: "d",
            })
            .await
            .unwrap();
        tokens.revoke(&row.id, 1).await.unwrap();
        assert!(tokens.find_by_hash("k").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn expired_token_not_returned() {
        let (_users, tokens, uid) = setup().await;
        let row = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "expired",
                device_id: "device-expired",
                device_name: "d",
            })
            .await
            .unwrap();
        sqlx::query("UPDATE tokens SET expires_at = ? WHERE id = ?")
            .bind(chrono::Utc::now().timestamp() - 1)
            .bind(&row.id)
            .execute(&tokens.pool)
            .await
            .unwrap();
        assert!(tokens.find_by_hash("expired").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn absolute_lifetime_expired_token_is_not_active() {
        let (_users, tokens, uid) = setup().await;
        let row = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "absolute-expired",
                device_id: "device-absolute-expired",
                device_name: "d",
            })
            .await
            .unwrap();
        let now = chrono::Utc::now().timestamp();
        sqlx::query("UPDATE tokens SET created_at = ?, expires_at = ? WHERE id = ?")
            .bind(now - token::TOKEN_TTL_SECONDS * 5)
            .bind(now + token::TOKEN_TTL_SECONDS)
            .bind(&row.id)
            .execute(&tokens.pool)
            .await
            .unwrap();

        assert!(tokens
            .find_by_hash("absolute-expired")
            .await
            .unwrap()
            .is_none());
        assert!(!tokens.is_active_for_user(&row.id, &uid).await.unwrap());
        assert!(tokens.list_active_for_user(&uid).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_active_for_user_filters_revoked_and_expired_tokens() {
        let (_users, tokens, uid) = setup().await;
        let active = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "active",
                device_id: "device-active",
                device_name: "active",
            })
            .await
            .unwrap();
        let revoked = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "revoked",
                device_id: "device-revoked",
                device_name: "revoked",
            })
            .await
            .unwrap();
        let expired = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "expired-list",
                device_id: "device-expired",
                device_name: "expired",
            })
            .await
            .unwrap();
        tokens.revoke(&revoked.id, 10).await.unwrap();
        sqlx::query("UPDATE tokens SET expires_at = ? WHERE id = ?")
            .bind(chrono::Utc::now().timestamp() - 1)
            .bind(&expired.id)
            .execute(&tokens.pool)
            .await
            .unwrap();

        let rows = tokens.list_active_for_user(&uid).await.unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, active.id);
    }

    #[tokio::test]
    async fn is_active_for_user_rejects_revoked_expired_and_wrong_user_tokens() {
        let (users, tokens, uid) = setup().await;
        let other = users
            .create(NewUser {
                username: "other".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let active = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "active-check",
                device_id: "device-active-check",
                device_name: "active",
            })
            .await
            .unwrap();
        let revoked = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "revoked-check",
                device_id: "device-revoked-check",
                device_name: "revoked",
            })
            .await
            .unwrap();
        tokens.revoke(&revoked.id, 10).await.unwrap();
        let expired = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "expired-check",
                device_id: "device-expired-check",
                device_name: "expired",
            })
            .await
            .unwrap();
        sqlx::query("UPDATE tokens SET expires_at = ? WHERE id = ?")
            .bind(chrono::Utc::now().timestamp() - 1)
            .bind(&expired.id)
            .execute(&tokens.pool)
            .await
            .unwrap();

        assert!(tokens.is_active_for_user(&active.id, &uid).await.unwrap());
        assert!(!tokens
            .is_active_for_user(&active.id, &other.id)
            .await
            .unwrap());
        assert!(!tokens.is_active_for_user(&revoked.id, &uid).await.unwrap());
        assert!(!tokens.is_active_for_user(&expired.id, &uid).await.unwrap());
    }

    #[tokio::test]
    async fn find_by_id_for_user_checks_token_ownership_without_active_filter() {
        let (users, tokens, uid) = setup().await;
        let other = users
            .create(NewUser {
                username: "other".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let owned = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "owned-lookup",
                device_id: "device-owned-lookup",
                device_name: "owned",
            })
            .await
            .unwrap();
        tokens.revoke(&owned.id, 10).await.unwrap();

        let found = tokens
            .find_by_id_for_user(&owned.id, &uid)
            .await
            .unwrap()
            .expect("owned revoked token should still be found for idempotent revoke");
        assert_eq!(found.id, owned.id);
        assert!(found.revoked_at.is_some());
        assert!(tokens
            .find_by_id_for_user(&owned.id, &other.id)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn revoke_all_except_keeps_one() {
        let (_users, tokens, uid) = setup().await;
        let a = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "a",
                device_id: "device-a",
                device_name: "1",
            })
            .await
            .unwrap();
        let _b = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "b",
                device_id: "device-b",
                device_name: "2",
            })
            .await
            .unwrap();
        let _c = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "c",
                device_id: "device-c",
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
    async fn revoke_other_active_for_device_keeps_only_current_device_token() {
        let (_users, tokens, uid) = setup().await;
        let old = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "old",
                device_id: "device-a",
                device_name: "Laptop",
            })
            .await
            .unwrap();
        let current = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "current",
                device_id: "device-a",
                device_name: "Laptop",
            })
            .await
            .unwrap();
        let other = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "other",
                device_id: "device-b",
                device_name: "Phone",
            })
            .await
            .unwrap();

        tokens
            .revoke_other_active_for_device(&uid, "device-a", 999, &current.id)
            .await
            .unwrap();

        let rows = tokens.list_for_user(&uid).await.unwrap();
        let old = rows.iter().find(|t| t.id == old.id).unwrap();
        let current = rows.iter().find(|t| t.id == current.id).unwrap();
        let other = rows.iter().find(|t| t.id == other.id).unwrap();
        assert_eq!(old.revoked_at, Some(999));
        assert!(current.revoked_at.is_none());
        assert!(other.revoked_at.is_none());
    }

    #[tokio::test]
    async fn touch_used_updates_timestamp() {
        let (_users, tokens, uid) = setup().await;
        let row = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "z",
                device_id: "device-z",
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
                device_id: "device-old",
                device_name: "d",
            })
            .await
            .unwrap();
        tokens.revoke(&row.id, 100).await.unwrap();
        let n = tokens.delete_revoked_older_than(200).await.unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn delete_revoked_older_keeps_activity_and_nulls_token_reference() {
        let (_users, tokens, uid) = setup().await;
        let row = tokens
            .create(NewToken {
                user_id: &uid,
                token_hash: "activity-token",
                device_id: "device-activity",
                device_name: "d",
            })
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sync_activity (user_id, token_id, action, timestamp)
             VALUES (?, ?, 'push', ?)",
        )
        .bind(&uid)
        .bind(&row.id)
        .bind(chrono::Utc::now().timestamp())
        .execute(&tokens.pool)
        .await
        .unwrap();
        tokens.revoke(&row.id, 100).await.unwrap();

        let n = tokens.delete_revoked_older_than(200).await.unwrap();

        assert_eq!(n, 1);
        let token_id: Option<String> =
            sqlx::query_scalar("SELECT token_id FROM sync_activity WHERE user_id = ?")
                .bind(&uid)
                .fetch_one(&tokens.pool)
                .await
                .unwrap();
        assert!(token_id.is_none());
    }
}
