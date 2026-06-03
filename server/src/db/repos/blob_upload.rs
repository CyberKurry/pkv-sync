use async_trait::async_trait;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use std::collections::HashSet;

const SQLITE_SAFE_BIND_LIMIT: usize = 900;
const DELETE_UPLOADS_SHARED_BINDS: usize = 1;

#[async_trait]
pub trait BlobUploadRepo: Send + Sync {
    async fn record_upload(
        &self,
        vault_id: &str,
        hash: &str,
        uploaded_at: i64,
    ) -> Result<(), sqlx::Error>;
    async fn has_upload(&self, vault_id: &str, hash: &str) -> Result<bool, sqlx::Error>;
    async fn uploaded_hashes_for_vault(
        &self,
        vault_id: &str,
        hashes: &[String],
    ) -> Result<HashSet<String>, sqlx::Error>;
    async fn all_hashes(&self) -> Result<HashSet<String>, sqlx::Error>;
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

    async fn uploaded_hashes_for_vault(
        &self,
        vault_id: &str,
        hashes: &[String],
    ) -> Result<HashSet<String>, sqlx::Error> {
        if hashes.is_empty() {
            return Ok(HashSet::new());
        }

        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT DISTINCT blob_hash FROM blob_uploads WHERE vault_id = ",
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

    async fn all_hashes(&self) -> Result<HashSet<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT blob_hash FROM blob_uploads")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|t| t.0).collect())
    }

    async fn delete_uploads(&self, vault_id: &str, hashes: &[String]) -> Result<(), sqlx::Error> {
        if hashes.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for chunk in hashes.chunks(delete_uploads_chunk_size()) {
            let mut query =
                QueryBuilder::<Sqlite>::new("DELETE FROM blob_uploads WHERE vault_id = ");
            query.push_bind(vault_id);
            query.push(" AND blob_hash IN (");
            let mut separated = query.separated(", ");
            for hash in chunk {
                separated.push_bind(hash);
            }
            separated.push_unseparated(")");
            query.build().execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

fn delete_uploads_chunk_size() -> usize {
    SQLITE_SAFE_BIND_LIMIT - DELETE_UPLOADS_SHARED_BINDS
}

#[cfg(test)]
fn delete_uploads_chunk_lengths(hashes: &[String]) -> impl Iterator<Item = usize> + '_ {
    hashes
        .chunks(delete_uploads_chunk_size())
        .map(<[String]>::len)
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

    #[test]
    fn delete_uploads_chunks_stay_under_sqlite_bind_limit() {
        let hashes: Vec<String> = (0..1000).map(|n| format!("sha:{n}")).collect();
        let chunks: Vec<usize> = delete_uploads_chunk_lengths(&hashes).collect();

        assert_eq!(chunks.iter().sum::<usize>(), hashes.len());
        assert!(chunks.iter().all(|len| *len < SQLITE_SAFE_BIND_LIMIT));
        assert!(chunks.len() > 1);
    }

    #[tokio::test]
    async fn batch_query_uploads_stays_scoped_to_vault() {
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
        repo.record_upload(&first.id, "b", 1).await.unwrap();
        repo.record_upload(&second.id, "c", 1).await.unwrap();
        repo.record_upload(&second.id, "a", 1).await.unwrap();

        let got = repo
            .uploaded_hashes_for_vault(
                &first.id,
                &["a".into(), "b".into(), "c".into(), "missing".into()],
            )
            .await
            .unwrap();

        assert_eq!(got, HashSet::from(["a".to_string(), "b".to_string()]));
        assert_eq!(
            repo.all_hashes().await.unwrap(),
            HashSet::from(["a".to_string(), "b".to_string(), "c".to_string()])
        );
        assert!(repo
            .uploaded_hashes_for_vault(&first.id, &[])
            .await
            .unwrap()
            .is_empty());
    }
}
