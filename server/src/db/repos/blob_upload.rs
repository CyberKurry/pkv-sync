use async_trait::async_trait;
use sqlx::SqlitePool;

#[async_trait]
pub trait BlobUploadRepo: Send + Sync {
    async fn record_upload(
        &self,
        vault_id: &str,
        hash: &str,
        uploaded_at: i64,
    ) -> Result<(), sqlx::Error>;
    async fn has_upload(&self, vault_id: &str, hash: &str) -> Result<bool, sqlx::Error>;
    async fn delete_uploads(&self, vault_id: &str, hashes: &[String]) -> Result<(), sqlx::Error>;
}

pub struct SqliteBlobUploadRepo {
    pool: SqlitePool,
}

impl SqliteBlobUploadRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BlobUploadRepo for SqliteBlobUploadRepo {
    async fn record_upload(
        &self,
        vault_id: &str,
        hash: &str,
        uploaded_at: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO blob_uploads (blob_hash, vault_id, uploaded_at)
             VALUES (?, ?, ?)",
        )
        .bind(hash)
        .bind(vault_id)
        .bind(uploaded_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn has_upload(&self, vault_id: &str, hash: &str) -> Result<bool, sqlx::Error> {
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM blob_uploads WHERE vault_id = ? AND blob_hash = ?",
        )
        .bind(vault_id)
        .bind(hash)
        .fetch_one(&self.pool)
        .await?;
        Ok(n > 0)
    }

    async fn delete_uploads(&self, vault_id: &str, hashes: &[String]) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        for hash in hashes {
            sqlx::query("DELETE FROM blob_uploads WHERE vault_id = ? AND blob_hash = ?")
                .bind(vault_id)
                .bind(hash)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::{NewUser, SqliteUserRepo, SqliteVaultRepo, UserRepo, VaultRepo};

    #[tokio::test]
    async fn tracks_uploads_per_vault() {
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let users = SqliteUserRepo::new(p.clone());
        let vaults = SqliteVaultRepo::new(p.clone());
        let user = users
            .create(NewUser {
                username: "u".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let first = vaults.create(&user.id, "first").await.unwrap();
        let second = vaults.create(&user.id, "second").await.unwrap();
        let repo = SqliteBlobUploadRepo::new(p);

        repo.record_upload(&first.id, "a", 1).await.unwrap();

        assert!(repo.has_upload(&first.id, "a").await.unwrap());
        assert!(!repo.has_upload(&second.id, "a").await.unwrap());
        repo.delete_uploads(&first.id, &["a".into()]).await.unwrap();
        assert!(!repo.has_upload(&first.id, "a").await.unwrap());
    }
}
