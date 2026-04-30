use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub is_active: bool,
    pub created_at: i64,
    pub last_login_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub username: String,
    pub password_hash: String,
    pub is_admin: bool,
}

#[async_trait]
pub trait UserRepo: Send + Sync {
    async fn create(&self, new_user: NewUser) -> Result<User, sqlx::Error>;
    async fn find_by_id(&self, id: &str) -> Result<Option<User>, sqlx::Error>;
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, sqlx::Error>;
    async fn list(&self) -> Result<Vec<User>, sqlx::Error>;
    async fn update_password(&self, id: &str, new_hash: &str) -> Result<(), sqlx::Error>;
    async fn set_active(&self, id: &str, active: bool) -> Result<(), sqlx::Error>;
    async fn set_admin(&self, id: &str, admin: bool) -> Result<(), sqlx::Error>;
    async fn touch_last_login(&self, id: &str, ts: i64) -> Result<(), sqlx::Error>;
    async fn delete(&self, id: &str) -> Result<(), sqlx::Error>;
    async fn count_admins(&self) -> Result<i64, sqlx::Error>;
}

pub struct SqliteUserRepo {
    pool: SqlitePool,
}

impl SqliteUserRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepo for SqliteUserRepo {
    async fn create(&self, n: NewUser) -> Result<User, sqlx::Error> {
        let id = Uuid::new_v4().simple().to_string();
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, is_admin, is_active, created_at)
             VALUES (?, ?, ?, ?, 1, ?)",
        )
        .bind(&id)
        .bind(&n.username)
        .bind(&n.password_hash)
        .bind(n.is_admin)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(User {
            id,
            username: n.username,
            password_hash: n.password_hash,
            is_admin: n.is_admin,
            is_active: true,
            created_at: now,
            last_login_at: None,
        })
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as::<_, (String, String, String, bool, bool, i64, Option<i64>)>(
            "SELECT id, username, password_hash, is_admin, is_active, created_at, last_login_at
             FROM users WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map(|r| r.map(row_to_user))
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as::<_, (String, String, String, bool, bool, i64, Option<i64>)>(
            "SELECT id, username, password_hash, is_admin, is_active, created_at, last_login_at
             FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map(|r| r.map(row_to_user))
    }

    async fn list(&self) -> Result<Vec<User>, sqlx::Error> {
        sqlx::query_as::<_, (String, String, String, bool, bool, i64, Option<i64>)>(
            "SELECT id, username, password_hash, is_admin, is_active, created_at, last_login_at
             FROM users ORDER BY username",
        )
        .fetch_all(&self.pool)
        .await
        .map(|rs| rs.into_iter().map(row_to_user).collect())
    }

    async fn update_password(&self, id: &str, new_hash: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
            .bind(new_hash)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn set_active(&self, id: &str, active: bool) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET is_active = ? WHERE id = ?")
            .bind(active)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn set_admin(&self, id: &str, admin: bool) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
            .bind(admin)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn touch_last_login(&self, id: &str, ts: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET last_login_at = ? WHERE id = ?")
            .bind(ts)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn count_admins(&self) -> Result<i64, sqlx::Error> {
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_admin = 1")
            .fetch_one(&self.pool)
            .await?;
        Ok(n)
    }
}

fn row_to_user(t: (String, String, String, bool, bool, i64, Option<i64>)) -> User {
    User {
        id: t.0,
        username: t.1,
        password_hash: t.2,
        is_admin: t.3,
        is_active: t.4,
        created_at: t.5,
        last_login_at: t.6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;

    async fn fresh_repo() -> SqliteUserRepo {
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        SqliteUserRepo::new(p)
    }

    #[tokio::test]
    async fn create_then_find() {
        let repo = fresh_repo().await;
        let u = repo
            .create(NewUser {
                username: "alice".into(),
                password_hash: "h1".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        assert_eq!(u.username, "alice");
        let by_id = repo.find_by_id(&u.id).await.unwrap().unwrap();
        assert_eq!(by_id.id, u.id);
        let by_name = repo.find_by_username("alice").await.unwrap().unwrap();
        assert_eq!(by_name.id, u.id);
    }

    #[tokio::test]
    async fn duplicate_username_errors() {
        let repo = fresh_repo().await;
        repo.create(NewUser {
            username: "x".into(),
            password_hash: "h".into(),
            is_admin: false,
        })
        .await
        .unwrap();
        let err = repo
            .create(NewUser {
                username: "x".into(),
                password_hash: "h2".into(),
                is_admin: false,
            })
            .await
            .unwrap_err();
        assert!(err.to_string().to_lowercase().contains("unique"));
    }

    #[tokio::test]
    async fn update_password_persists() {
        let repo = fresh_repo().await;
        let u = repo
            .create(NewUser {
                username: "u".into(),
                password_hash: "old".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        repo.update_password(&u.id, "new").await.unwrap();
        let r = repo.find_by_id(&u.id).await.unwrap().unwrap();
        assert_eq!(r.password_hash, "new");
    }

    #[tokio::test]
    async fn touch_last_login_persists() {
        let repo = fresh_repo().await;
        let u = repo
            .create(NewUser {
                username: "u".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        repo.touch_last_login(&u.id, 1234567890).await.unwrap();
        let r = repo.find_by_id(&u.id).await.unwrap().unwrap();
        assert_eq!(r.last_login_at, Some(1234567890));
    }

    #[tokio::test]
    async fn count_admins() {
        let repo = fresh_repo().await;
        assert_eq!(repo.count_admins().await.unwrap(), 0);
        repo.create(NewUser {
            username: "admin".into(),
            password_hash: "h".into(),
            is_admin: true,
        })
        .await
        .unwrap();
        assert_eq!(repo.count_admins().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn delete_removes_user() {
        let repo = fresh_repo().await;
        let u = repo
            .create(NewUser {
                username: "x".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        repo.delete(&u.id).await.unwrap();
        assert!(repo.find_by_id(&u.id).await.unwrap().is_none());
    }
}
