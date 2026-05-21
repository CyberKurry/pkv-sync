use async_trait::async_trait;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use std::collections::HashSet;

#[async_trait]
pub trait BlobRefRepo: Send + Sync {
    async fn add_refs(
        &self,
        vault_id: &str,
        commit_hash: &str,
        hashes: &[String],
    ) -> Result<(), sqlx::Error>;
    async fn hashes_for_vault(&self, vault_id: &str) -> Result<HashSet<String>, sqlx::Error>;
    async fn all_hashes(&self) -> Result<HashSet<String>, sqlx::Error>;
    async fn is_referenced_by_vault(&self, vault_id: &str, hash: &str)
        -> Result<bool, sqlx::Error>;
    async fn referenced_hashes_for_vault(
        &self,
        vault_id: &str,
        hashes: &[String],
    ) -> Result<HashSet<String>, sqlx::Error>;
}

pub struct SqliteBlobRefRepo {
    pool: SqlitePool,
}

impl SqliteBlobRefRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BlobRefRepo for SqliteBlobRefRepo {
    async fn add_refs(
        &self,
        vault_id: &str,
        commit_hash: &str,
        hashes: &[String],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        for h in hashes {
            sqlx::query(
                "INSERT OR IGNORE INTO blob_refs (blob_hash, vault_id, commit_hash)
                 VALUES (?, ?, ?)",
            )
            .bind(h)
            .bind(vault_id)
            .bind(commit_hash)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn hashes_for_vault(&self, vault_id: &str) -> Result<HashSet<String>, sqlx::Error> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT DISTINCT blob_hash FROM blob_refs WHERE vault_id = ?")
                .bind(vault_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|t| t.0).collect())
    }

    async fn all_hashes(&self) -> Result<HashSet<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT blob_hash FROM blob_refs")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|t| t.0).collect())
    }

    async fn is_referenced_by_vault(
        &self,
        vault_id: &str,
        hash: &str,
    ) -> Result<bool, sqlx::Error> {
        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM blob_refs WHERE vault_id = ? AND blob_hash = ?")
                .bind(vault_id)
                .bind(hash)
                .fetch_one(&self.pool)
                .await?;
        Ok(n > 0)
    }

    async fn referenced_hashes_for_vault(
        &self,
        vault_id: &str,
        hashes: &[String],
    ) -> Result<HashSet<String>, sqlx::Error> {
        if hashes.is_empty() {
            return Ok(HashSet::new());
        }

        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT DISTINCT blob_hash FROM blob_refs WHERE vault_id = ",
        );
        query.push_bind(vault_id);
        query.push(" AND blob_hash IN (");
        let mut separated = query.separated(", ");
        for hash in hashes {
            separated.push_bind(hash);
        }
        separated.push_unseparated(")");

        let rows: Vec<(String,)> = query.build_query_as().fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|t| t.0).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::{NewUser, SqliteUserRepo, SqliteVaultRepo, UserRepo, VaultRepo};

    #[tokio::test]
    async fn add_and_query_refs() {
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let users = SqliteUserRepo::new(p.clone());
        let vaults = SqliteVaultRepo::new(p.clone());
        let u = users
            .create(NewUser {
                username: "u".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let v = vaults.create(&u.id, "main").await.unwrap();
        let repo = SqliteBlobRefRepo::new(p);
        repo.add_refs(&v.id, "c1", &["sha:a".into(), "sha:b".into()])
            .await
            .unwrap();
        assert!(repo.is_referenced_by_vault(&v.id, "sha:a").await.unwrap());
        assert_eq!(repo.hashes_for_vault(&v.id).await.unwrap().len(), 2);
        assert_eq!(repo.all_hashes().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn batch_query_refs_stays_scoped_to_vault() {
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let users = SqliteUserRepo::new(p.clone());
        let vaults = SqliteVaultRepo::new(p.clone());
        let u = users
            .create(NewUser {
                username: "u".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let first = vaults.create(&u.id, "first").await.unwrap();
        let second = vaults.create(&u.id, "second").await.unwrap();
        let repo = SqliteBlobRefRepo::new(p);
        repo.add_refs(&first.id, "c1", &["sha:first".into(), "sha:shared".into()])
            .await
            .unwrap();
        repo.add_refs(&second.id, "c2", &["sha:second".into()])
            .await
            .unwrap();

        let got = repo
            .referenced_hashes_for_vault(
                &first.id,
                &[
                    "sha:first".into(),
                    "sha:second".into(),
                    "sha:missing".into(),
                    "sha:shared".into(),
                ],
            )
            .await
            .unwrap();

        assert_eq!(
            got,
            HashSet::from(["sha:first".to_string(), "sha:shared".to_string()])
        );
        assert!(repo
            .referenced_hashes_for_vault(&first.id, &[])
            .await
            .unwrap()
            .is_empty());
    }
}
