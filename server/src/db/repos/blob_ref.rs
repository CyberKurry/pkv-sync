use crate::db::SQLITE_SAFE_BIND_LIMIT;
use async_trait::async_trait;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use std::collections::HashSet;

const QUERY_REFS_SHARED_BINDS: usize = 1;

#[async_trait]
pub trait BlobRefRepo: Send + Sync {
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

    #[cfg(test)]
    pub async fn add_refs(
        &self,
        vault_id: &str,
        commit_hash: &str,
        hashes: &[String],
    ) -> Result<(), sqlx::Error> {
        if hashes.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for chunk in hashes.chunks(add_refs_chunk_size()) {
            let mut query = QueryBuilder::<Sqlite>::new(
                "INSERT OR IGNORE INTO blob_refs (blob_hash, vault_id, commit_hash) ",
            );
            query.push_values(chunk, |mut row, hash| {
                row.push_bind(hash)
                    .push_bind(vault_id)
                    .push_bind(commit_hash);
            });
            query.build().execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

#[async_trait]
impl BlobRefRepo for SqliteBlobRefRepo {
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

        let mut found = HashSet::new();
        for chunk in hashes.chunks(query_refs_chunk_size()) {
            let mut query = QueryBuilder::<Sqlite>::new(
                "SELECT DISTINCT blob_hash FROM blob_refs WHERE vault_id = ",
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
}

#[cfg(test)]
fn add_refs_chunk_size() -> usize {
    SQLITE_SAFE_BIND_LIMIT / 3
}

fn query_refs_chunk_size() -> usize {
    SQLITE_SAFE_BIND_LIMIT - QUERY_REFS_SHARED_BINDS
}

#[cfg(test)]
fn add_refs_chunk_lengths(hashes: &[String]) -> impl Iterator<Item = usize> + '_ {
    hashes.chunks(add_refs_chunk_size()).map(<[String]>::len)
}

#[cfg(test)]
fn query_refs_chunk_lengths(hashes: &[String]) -> impl Iterator<Item = usize> + '_ {
    hashes.chunks(query_refs_chunk_size()).map(<[String]>::len)
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
        assert_eq!(
            repo.referenced_hashes_for_vault(&v.id, &["sha:a".into(), "sha:b".into()])
                .await
                .unwrap()
                .len(),
            2
        );
        assert_eq!(repo.all_hashes().await.unwrap().len(), 2);
    }

    #[test]
    fn add_refs_chunks_stay_under_sqlite_bind_limit() {
        let hashes: Vec<String> = (0..1000).map(|n| format!("sha:{n}")).collect();
        let chunks: Vec<usize> = add_refs_chunk_lengths(&hashes).collect();

        assert_eq!(chunks.iter().sum::<usize>(), hashes.len());
        assert!(chunks.iter().all(|len| len * 3 <= SQLITE_SAFE_BIND_LIMIT));
        assert!(chunks.len() > 1);
    }

    #[test]
    fn batch_query_refs_chunks_stay_under_sqlite_bind_limit() {
        let hashes: Vec<String> = (0..10_000).map(|n| format!("sha:{n}")).collect();
        let chunks: Vec<usize> = query_refs_chunk_lengths(&hashes).collect();

        assert_eq!(chunks.iter().sum::<usize>(), hashes.len());
        assert!(chunks.iter().all(|len| *len < SQLITE_SAFE_BIND_LIMIT));
        assert!(chunks.len() > 1);
    }

    #[test]
    fn blob_ref_repo_does_not_expose_production_dead_apis() {
        let source = include_str!("blob_ref.rs");
        let trait_start = source
            .find("pub trait BlobRefRepo")
            .expect("BlobRefRepo trait exists");
        let trait_end = source[trait_start..]
            .find("\npub struct SqliteBlobRefRepo")
            .map(|idx| trait_start + idx)
            .expect("SqliteBlobRefRepo follows BlobRefRepo");
        let trait_source = &source[trait_start..trait_end];

        assert!(!trait_source.contains("async fn hashes_for_vault("));
        assert!(!trait_source.contains("async fn add_refs("));

        let inherent_start = source
            .find("impl SqliteBlobRefRepo")
            .expect("SqliteBlobRefRepo inherent impl exists");
        let inherent_end = source[inherent_start..]
            .find("\n#[async_trait]")
            .map(|idx| inherent_start + idx)
            .expect("trait impl follows inherent impl");
        let inherent_source = &source[inherent_start..inherent_end];

        assert!(
            inherent_source.contains("#[cfg(test)]\n    pub async fn add_refs("),
            "add_refs should be limited to crate-local test callers"
        );
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

    #[tokio::test]
    async fn batch_query_refs_handles_more_hashes_than_sqlite_bind_limit() {
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
        let hashes: Vec<String> = (0..50_000).map(|n| format!("sha:{n:05}")).collect();

        let got = repo
            .referenced_hashes_for_vault(&v.id, &hashes)
            .await
            .unwrap();

        assert!(got.is_empty());
    }
}
