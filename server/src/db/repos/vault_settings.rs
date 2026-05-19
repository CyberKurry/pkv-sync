use async_trait::async_trait;
use sqlx::SqlitePool;
use std::collections::HashMap;

#[async_trait]
pub trait VaultSettingsRepo: Send + Sync {
    async fn get(&self, vault_id: &str, key: &str) -> Result<Option<String>, sqlx::Error>;
    async fn set(&self, vault_id: &str, key: &str, value: &str) -> Result<(), sqlx::Error>;
    async fn load_for_vault(&self, vault_id: &str) -> Result<HashMap<String, String>, sqlx::Error>;
}

pub struct SqliteVaultSettingsRepo {
    pool: SqlitePool,
}

impl SqliteVaultSettingsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl VaultSettingsRepo for SqliteVaultSettingsRepo {
    async fn get(&self, vault_id: &str, key: &str) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM vault_settings WHERE vault_id = ? AND key = ?")
                .bind(vault_id)
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|t| t.0))
    }

    async fn set(&self, vault_id: &str, key: &str, value: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO vault_settings (vault_id, key, value, updated_at) VALUES (?, ?, ?, ?)
             ON CONFLICT(vault_id, key) DO UPDATE SET value = excluded.value,
                                                       updated_at = excluded.updated_at",
        )
        .bind(vault_id)
        .bind(key)
        .bind(value)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn load_for_vault(&self, vault_id: &str) -> Result<HashMap<String, String>, sqlx::Error> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM vault_settings WHERE vault_id = ?")
                .bind(vault_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::{NewUser, SqliteUserRepo, SqliteVaultRepo, UserRepo, VaultRepo};

    async fn setup() -> (SqliteVaultSettingsRepo, SqliteVaultRepo, String) {
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let users = SqliteUserRepo::new(p.clone());
        let user = users
            .create(NewUser {
                username: "cyberkurry".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        (
            SqliteVaultSettingsRepo::new(p.clone()),
            SqliteVaultRepo::new(p),
            user.id,
        )
    }

    #[tokio::test]
    async fn set_get_and_load_for_vault_round_trip() {
        let (settings, vaults, user_id) = setup().await;
        let vault = vaults.create(&user_id, "main").await.unwrap();

        settings
            .set(&vault.id, "extra_sync_globs", r#"["notes/**"]"#)
            .await
            .unwrap();

        assert_eq!(
            settings.get(&vault.id, "extra_sync_globs").await.unwrap(),
            Some(r#"["notes/**"]"#.to_string())
        );
        assert_eq!(
            settings
                .load_for_vault(&vault.id)
                .await
                .unwrap()
                .get("extra_sync_globs"),
            Some(&r#"["notes/**"]"#.to_string())
        );
    }
}
