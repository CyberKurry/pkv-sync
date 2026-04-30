use async_trait::async_trait;
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyEntry {
    pub vault_id: String,
    pub route: String,
    pub request_hash: String,
    pub response_json: String,
}

#[async_trait]
pub trait IdempotencyRepo: Send + Sync {
    async fn get(&self, key: &str, user_id: &str) -> Result<Option<IdempotencyEntry>, sqlx::Error>;
    async fn put(
        &self,
        key: &str,
        user_id: &str,
        vault_id: &str,
        route: &str,
        request_hash: &str,
        response_json: &str,
    ) -> Result<(), sqlx::Error>;
    async fn delete_older_than(&self, ts: i64) -> Result<u64, sqlx::Error>;
}

pub struct SqliteIdempotencyRepo {
    pool: SqlitePool,
}

impl SqliteIdempotencyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl IdempotencyRepo for SqliteIdempotencyRepo {
    async fn get(&self, key: &str, user_id: &str) -> Result<Option<IdempotencyEntry>, sqlx::Error> {
        let r: Option<(String, String, String, String)> = sqlx::query_as(
            "SELECT vault_id, route, request_hash, response_json
             FROM idempotency_cache WHERE key = ? AND user_id = ?",
        )
        .bind(key)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(r.map(|t| IdempotencyEntry {
            vault_id: t.0,
            route: t.1,
            request_hash: t.2,
            response_json: t.3,
        }))
    }

    async fn put(
        &self,
        key: &str,
        user_id: &str,
        vault_id: &str,
        route: &str,
        request_hash: &str,
        response_json: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO idempotency_cache
             (user_id, key, vault_id, route, request_hash, response_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(key)
        .bind(vault_id)
        .bind(route)
        .bind(request_hash)
        .bind(response_json)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_older_than(&self, ts: i64) -> Result<u64, sqlx::Error> {
        let r = sqlx::query("DELETE FROM idempotency_cache WHERE created_at < ?")
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

    #[tokio::test]
    async fn put_get_delete_old() {
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let repo = SqliteIdempotencyRepo::new(p);
        repo.put("k", "u", "v", "push", "hash1", "{\"ok\":true}")
            .await
            .unwrap();
        assert_eq!(
            repo.get("k", "u").await.unwrap().unwrap().response_json,
            "{\"ok\":true}"
        );
        let n = repo
            .delete_older_than(chrono::Utc::now().timestamp() + 1)
            .await
            .unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn same_key_is_scoped_per_user() {
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let repo = SqliteIdempotencyRepo::new(p);

        repo.put("k", "u1", "v1", "push", "hash1", "{\"user\":1}")
            .await
            .unwrap();
        repo.put("k", "u2", "v2", "push", "hash2", "{\"user\":2}")
            .await
            .unwrap();

        assert_eq!(
            repo.get("k", "u1").await.unwrap().unwrap().response_json,
            "{\"user\":1}"
        );
        assert_eq!(
            repo.get("k", "u2").await.unwrap().unwrap().response_json,
            "{\"user\":2}"
        );
    }
}
