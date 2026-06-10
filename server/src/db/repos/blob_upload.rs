use crate::db::SQLITE_SAFE_BIND_LIMIT;
use async_trait::async_trait;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use std::collections::HashSet;

const QUERY_UPLOADS_SHARED_BINDS: usize = 1;

#[async_trait]
pub trait BlobUploadRepo: Send + Sync {
    async fn record_upload(
        &self,
        vault_id: &str,
        hash: &str,
        uploaded_at: i64,
    ) -> Result<(), sqlx::Error>;
    async fn uploaded_hashes_for_vault(
        &self,
        vault_id: &str,
        hashes: &[String],
    ) -> Result<HashSet<String>, sqlx::Error>;
    async fn all_hashes(&self) -> Result<HashSet<String>, sqlx::Error>;
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

    async fn uploaded_hashes_for_vault(
        &self,
        vault_id: &str,
        hashes: &[String],
    ) -> Result<HashSet<String>, sqlx::Error> {
        if hashes.is_empty() {
            return Ok(HashSet::new());
        }

        let mut found = HashSet::new();
        for chunk in hashes.chunks(query_uploads_chunk_size()) {
            let mut query = QueryBuilder::<Sqlite>::new(
                "SELECT DISTINCT blob_hash FROM blob_uploads WHERE vault_id = ",
            );
            query.push_bind(vault_id);
            query.push(" AND blob_hash IN (");
            let mut separated = query.separated(", ");
            for hash in chunk {
                separated.push_bind(hash);
            }
            separated.push_unseparated(")");

            let rows: Vec<(String,)> = query.build_query_as().fetch_all(&self.pool).await?;
            found.extend(rows.into_iter().map(|t| t.0));
        }
        Ok(found)
    }

    async fn all_hashes(&self) -> Result<HashSet<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT blob_hash FROM blob_uploads")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|t| t.0).collect())
    }
}

fn query_uploads_chunk_size() -> usize {
    SQLITE_SAFE_BIND_LIMIT - QUERY_UPLOADS_SHARED_BINDS
}

#[cfg(test)]
fn query_uploads_chunk_lengths(hashes: &[String]) -> impl Iterator<Item = usize> + '_ {
    hashes
        .chunks(query_uploads_chunk_size())
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

        assert_eq!(
            repo.uploaded_hashes_for_vault(&first.id, &["a".into()])
                .await
                .unwrap(),
            HashSet::from(["a".to_string()])
        );
        assert!(repo
            .uploaded_hashes_for_vault(&second.id, &["a".into()])
            .await
            .unwrap()
            .is_empty());
    }

    #[test]
    fn batch_query_uploads_chunks_stay_under_sqlite_bind_limit() {
        let hashes: Vec<String> = (0..10_000).map(|n| format!("sha:{n}")).collect();
        let chunks: Vec<usize> = query_uploads_chunk_lengths(&hashes).collect();

        assert_eq!(chunks.iter().sum::<usize>(), hashes.len());
        assert!(chunks
            .iter()
            .all(|len| *len < crate::db::SQLITE_SAFE_BIND_LIMIT));
        assert!(chunks.len() > 1);
    }

    #[test]
    fn blob_upload_repo_trait_does_not_expose_test_only_dead_apis() {
        let source = include_str!("blob_upload.rs");
        let trait_start = source
            .find("pub trait BlobUploadRepo")
            .expect("BlobUploadRepo trait exists");
        let trait_end = source[trait_start..]
            .find("\npub struct SqliteBlobUploadRepo")
            .map(|idx| trait_start + idx)
            .expect("SqliteBlobUploadRepo follows BlobUploadRepo");
        let trait_source = &source[trait_start..trait_end];

        assert!(!trait_source.contains("has_upload("));
        assert!(!trait_source.contains("delete_uploads("));
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

    #[tokio::test]
    async fn batch_query_uploads_handles_more_hashes_than_sqlite_bind_limit() {
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
        let vault = vaults.create(&user.id, "main").await.unwrap();
        let repo = SqliteBlobUploadRepo::new(p);
        let hashes: Vec<String> = (0..50_000).map(|n| format!("sha:{n:05}")).collect();

        let got = repo
            .uploaded_hashes_for_vault(&vault.id, &hashes)
            .await
            .unwrap();

        assert!(got.is_empty());
    }
}
