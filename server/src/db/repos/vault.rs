use async_trait::async_trait;
use serde::Serialize;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct Vault {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub created_at: i64,
    pub last_sync_at: Option<i64>,
    pub size_bytes: i64,
    pub file_count: i64,
}

type VaultRow = (String, String, String, i64, Option<i64>, i64, i64);

#[async_trait]
pub trait VaultRepo: Send + Sync {
    async fn create(&self, user_id: &str, name: &str) -> Result<Vault, sqlx::Error>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Vault>, sqlx::Error>;
    async fn find_for_user(&self, user_id: &str, id: &str) -> Result<Option<Vault>, sqlx::Error>;
    async fn list_for_user(&self, user_id: &str) -> Result<Vec<Vault>, sqlx::Error>;
    async fn delete_for_user(&self, user_id: &str, id: &str) -> Result<bool, sqlx::Error>;
    async fn update_stats(
        &self,
        id: &str,
        size_bytes: i64,
        file_count: i64,
        ts: i64,
    ) -> Result<(), sqlx::Error>;
}

pub struct SqliteVaultRepo {
    pool: SqlitePool,
}

impl SqliteVaultRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl VaultRepo for SqliteVaultRepo {
    async fn create(&self, user_id: &str, name: &str) -> Result<Vault, sqlx::Error> {
        let id = Uuid::new_v4().simple().to_string();
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO vaults (id, user_id, name, created_at) VALUES (?, ?, ?, ?)")
            .bind(&id)
            .bind(user_id)
            .bind(name)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(Vault {
            id,
            user_id: user_id.into(),
            name: name.into(),
            created_at: now,
            last_sync_at: None,
            size_bytes: 0,
            file_count: 0,
        })
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Vault>, sqlx::Error> {
        query_one(
            &self.pool,
            "SELECT id, user_id, name, created_at, last_sync_at, size_bytes, file_count
             FROM vaults WHERE id = ?",
            id,
        )
        .await
    }

    async fn find_for_user(&self, user_id: &str, id: &str) -> Result<Option<Vault>, sqlx::Error> {
        let row: Option<VaultRow> = sqlx::query_as(
            "SELECT id, user_id, name, created_at, last_sync_at, size_bytes, file_count
             FROM vaults WHERE user_id = ? AND id = ?",
        )
        .bind(user_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(row_to_vault))
    }

    async fn list_for_user(&self, user_id: &str) -> Result<Vec<Vault>, sqlx::Error> {
        let rows: Vec<VaultRow> = sqlx::query_as(
            "SELECT id, user_id, name, created_at, last_sync_at, size_bytes, file_count
             FROM vaults WHERE user_id = ? ORDER BY name",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(row_to_vault).collect())
    }

    async fn delete_for_user(&self, user_id: &str, id: &str) -> Result<bool, sqlx::Error> {
        let r = sqlx::query("DELETE FROM vaults WHERE user_id = ? AND id = ?")
            .bind(user_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected() == 1)
    }

    async fn update_stats(
        &self,
        id: &str,
        size_bytes: i64,
        file_count: i64,
        ts: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE vaults SET size_bytes = ?, file_count = ?, last_sync_at = ? WHERE id = ?",
        )
        .bind(size_bytes)
        .bind(file_count)
        .bind(ts)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

async fn query_one(pool: &SqlitePool, sql: &str, id: &str) -> Result<Option<Vault>, sqlx::Error> {
    let row: Option<VaultRow> = sqlx::query_as(sql).bind(id).fetch_optional(pool).await?;
    Ok(row.map(row_to_vault))
}

fn row_to_vault(t: VaultRow) -> Vault {
    Vault {
        id: t.0,
        user_id: t.1,
        name: t.2,
        created_at: t.3,
        last_sync_at: t.4,
        size_bytes: t.5,
        file_count: t.6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::{NewUser, SqliteUserRepo, UserRepo};

    async fn setup() -> (SqliteVaultRepo, String) {
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
        (SqliteVaultRepo::new(p), u.id)
    }

    #[tokio::test]
    async fn create_and_list_for_user() {
        let (repo, uid) = setup().await;
        let v = repo.create(&uid, "main").await.unwrap();
        assert_eq!(v.name, "main");
        let list = repo.list_for_user(&uid).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, v.id);
    }

    #[tokio::test]
    async fn duplicate_name_per_user_errors() {
        let (repo, uid) = setup().await;
        repo.create(&uid, "main").await.unwrap();
        let err = repo.create(&uid, "main").await.unwrap_err();
        assert!(err.to_string().to_lowercase().contains("unique"));
    }

    #[tokio::test]
    async fn update_stats() {
        let (repo, uid) = setup().await;
        let v = repo.create(&uid, "main").await.unwrap();
        repo.update_stats(&v.id, 123, 4, 99).await.unwrap();
        let got = repo.find_by_id(&v.id).await.unwrap().unwrap();
        assert_eq!(got.size_bytes, 123);
        assert_eq!(got.file_count, 4);
        assert_eq!(got.last_sync_at, Some(99));
    }

    #[tokio::test]
    async fn delete_is_scoped_to_user() {
        let (repo, uid) = setup().await;
        let v = repo.create(&uid, "main").await.unwrap();
        assert!(!repo.delete_for_user("other", &v.id).await.unwrap());
        assert!(repo.delete_for_user(&uid, &v.id).await.unwrap());
    }
}
