use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationMode {
    Disabled,
    InviteOnly,
    Open,
}

impl RegistrationMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::InviteOnly => "invite_only",
            Self::Open => "open",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "disabled" => Some(Self::Disabled),
            "invite_only" => Some(Self::InviteOnly),
            "open" => Some(Self::Open),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub registration_mode: RegistrationMode,
    pub server_name: String,
    pub timezone: String,
    pub login_failure_threshold: u32,
    pub login_window_seconds: u64,
    pub login_lock_seconds: u64,
    pub max_file_size: u64,
    pub text_extensions: Vec<String>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            registration_mode: RegistrationMode::Disabled,
            server_name: "PKV Sync".into(),
            timezone: crate::time::DEFAULT_TIMEZONE.into(),
            login_failure_threshold: 10,
            login_window_seconds: 15 * 60,
            login_lock_seconds: 15 * 60,
            max_file_size: 100 * 1024 * 1024,
            text_extensions: vec![
                "md".into(),
                "canvas".into(),
                "base".into(),
                "json".into(),
                "txt".into(),
                "css".into(),
            ],
        }
    }
}

#[async_trait]
pub trait RuntimeConfigRepo: Send + Sync {
    async fn load(&self) -> Result<RuntimeConfig, sqlx::Error>;
    async fn set_registration_mode(
        &self,
        mode: RegistrationMode,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn set_server_name(&self, name: &str, by: Option<&str>) -> Result<(), sqlx::Error>;
    async fn set_timezone(&self, timezone: &str, by: Option<&str>) -> Result<(), sqlx::Error>;
    async fn set_login_rate_limit(
        &self,
        threshold: u32,
        window_seconds: u64,
        lock_seconds: u64,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn set_max_file_size(
        &self,
        max_file_size: u64,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn set_text_extensions(
        &self,
        extensions: Vec<String>,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error>;
}

pub struct SqliteRuntimeConfigRepo {
    pool: SqlitePool,
}

impl SqliteRuntimeConfigRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

async fn read_kv(pool: &SqlitePool, key: &str) -> Result<Option<String>, sqlx::Error> {
    let r: Option<(String,)> = sqlx::query_as("SELECT value FROM runtime_config WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(r.map(|t| t.0))
}

async fn write_kv(
    pool: &SqlitePool,
    key: &str,
    value: &str,
    by: Option<&str>,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO runtime_config (key, value, updated_at, updated_by) VALUES (?, ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value,
                                       updated_at = excluded.updated_at,
                                       updated_by = excluded.updated_by",
    )
    .bind(key)
    .bind(value)
    .bind(now)
    .bind(by)
    .execute(pool)
    .await?;
    Ok(())
}

#[async_trait]
impl RuntimeConfigRepo for SqliteRuntimeConfigRepo {
    async fn load(&self) -> Result<RuntimeConfig, sqlx::Error> {
        let mut cfg = RuntimeConfig::default();
        if let Some(v) = read_kv(&self.pool, "registration_mode").await? {
            if let Ok(s) = serde_json::from_str::<String>(&v) {
                if let Some(m) = RegistrationMode::parse(&s) {
                    cfg.registration_mode = m;
                }
            }
        }
        if let Some(v) = read_kv(&self.pool, "server_name").await? {
            if let Ok(s) = serde_json::from_str::<String>(&v) {
                cfg.server_name = s;
            }
        }
        if let Some(v) = read_kv(&self.pool, "timezone").await? {
            if let Ok(s) = serde_json::from_str::<String>(&v) {
                if let Some(timezone) = crate::time::normalize_timezone(&s) {
                    cfg.timezone = timezone;
                }
            }
        }
        if let Some(v) = read_kv(&self.pool, "login_failure_threshold").await? {
            if let Ok(n) = serde_json::from_str::<u32>(&v) {
                cfg.login_failure_threshold = n.max(1);
            }
        }
        if let Some(v) = read_kv(&self.pool, "login_window_seconds").await? {
            if let Ok(n) = serde_json::from_str::<u64>(&v) {
                cfg.login_window_seconds = n.max(1);
            }
        }
        if let Some(v) = read_kv(&self.pool, "login_lock_seconds").await? {
            if let Ok(n) = serde_json::from_str::<u64>(&v) {
                cfg.login_lock_seconds = n.max(1);
            }
        }
        if let Some(v) = read_kv(&self.pool, "max_file_size").await? {
            if let Ok(n) = serde_json::from_str::<u64>(&v) {
                cfg.max_file_size = n.max(1024);
            }
        }
        if let Some(v) = read_kv(&self.pool, "text_extensions").await? {
            if let Ok(exts) = serde_json::from_str::<Vec<String>>(&v) {
                cfg.text_extensions = exts;
            }
        }
        Ok(cfg)
    }

    async fn set_registration_mode(
        &self,
        mode: RegistrationMode,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let v = serde_json::to_string(mode.as_str()).expect("string serializes");
        write_kv(&self.pool, "registration_mode", &v, by).await
    }

    async fn set_server_name(&self, name: &str, by: Option<&str>) -> Result<(), sqlx::Error> {
        let v = serde_json::to_string(name).expect("string serializes");
        write_kv(&self.pool, "server_name", &v, by).await
    }

    async fn set_timezone(&self, timezone: &str, by: Option<&str>) -> Result<(), sqlx::Error> {
        let v = serde_json::to_string(timezone).expect("string serializes");
        write_kv(&self.pool, "timezone", &v, by).await
    }

    async fn set_login_rate_limit(
        &self,
        threshold: u32,
        window_seconds: u64,
        lock_seconds: u64,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        write_kv(
            &self.pool,
            "login_failure_threshold",
            &serde_json::to_string(&threshold.max(1)).unwrap(),
            by,
        )
        .await?;
        write_kv(
            &self.pool,
            "login_window_seconds",
            &serde_json::to_string(&window_seconds.max(1)).unwrap(),
            by,
        )
        .await?;
        write_kv(
            &self.pool,
            "login_lock_seconds",
            &serde_json::to_string(&lock_seconds.max(1)).unwrap(),
            by,
        )
        .await?;
        Ok(())
    }

    async fn set_max_file_size(
        &self,
        max_file_size: u64,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        write_kv(
            &self.pool,
            "max_file_size",
            &serde_json::to_string(&max_file_size.max(1024)).unwrap(),
            by,
        )
        .await
    }

    async fn set_text_extensions(
        &self,
        extensions: Vec<String>,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        write_kv(
            &self.pool,
            "text_extensions",
            &serde_json::to_string(&extensions).unwrap(),
            by,
        )
        .await
    }
}

/// Hot-reloadable cache shared by handlers.
#[derive(Clone)]
pub struct RuntimeConfigCache(pub Arc<RwLock<RuntimeConfig>>);

impl RuntimeConfigCache {
    pub fn new(cfg: RuntimeConfig) -> Self {
        Self(Arc::new(RwLock::new(cfg)))
    }

    pub async fn snapshot(&self) -> RuntimeConfig {
        self.0.read().await.clone()
    }

    pub async fn replace(&self, cfg: RuntimeConfig) {
        *self.0.write().await = cfg;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;

    async fn setup() -> SqliteRuntimeConfigRepo {
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        SqliteRuntimeConfigRepo::new(p)
    }

    #[tokio::test]
    async fn defaults_on_empty_db() {
        let r = setup().await;
        let cfg = r.load().await.unwrap();
        assert_eq!(cfg.registration_mode, RegistrationMode::Disabled);
        assert_eq!(cfg.server_name, "PKV Sync");
    }

    #[tokio::test]
    async fn set_and_reload_registration_mode() {
        let r = setup().await;
        r.set_registration_mode(RegistrationMode::InviteOnly, None)
            .await
            .unwrap();
        assert_eq!(
            r.load().await.unwrap().registration_mode,
            RegistrationMode::InviteOnly
        );
        r.set_registration_mode(RegistrationMode::Open, None)
            .await
            .unwrap();
        assert_eq!(
            r.load().await.unwrap().registration_mode,
            RegistrationMode::Open
        );
    }

    #[tokio::test]
    async fn set_and_reload_server_name() {
        let r = setup().await;
        r.set_server_name("Alice's Vault Hub", None).await.unwrap();
        assert_eq!(r.load().await.unwrap().server_name, "Alice's Vault Hub");
    }

    #[tokio::test]
    async fn cache_snapshot_and_replace() {
        let cache = RuntimeConfigCache::new(RuntimeConfig::default());
        let snap1 = cache.snapshot().await;
        assert_eq!(snap1.registration_mode, RegistrationMode::Disabled);
        cache
            .replace(RuntimeConfig {
                registration_mode: RegistrationMode::Open,
                server_name: "X".into(),
                timezone: crate::time::DEFAULT_TIMEZONE.into(),
                login_failure_threshold: 10,
                login_window_seconds: 900,
                login_lock_seconds: 900,
                max_file_size: 100 * 1024 * 1024,
                text_extensions: RuntimeConfig::default().text_extensions.clone(),
            })
            .await;
        let snap2 = cache.snapshot().await;
        assert_eq!(snap2.registration_mode, RegistrationMode::Open);
    }

    #[tokio::test]
    async fn set_and_reload_max_file_size() {
        let r = setup().await;
        r.set_max_file_size(50 * 1024 * 1024, None).await.unwrap();
        let cfg = r.load().await.unwrap();
        assert_eq!(cfg.max_file_size, 50 * 1024 * 1024);
    }

    #[tokio::test]
    async fn set_and_reload_timezone() {
        let r = setup().await;
        r.set_timezone("Asia/Shanghai", None).await.unwrap();
        assert_eq!(r.load().await.unwrap().timezone, "Asia/Shanghai");
    }

    #[tokio::test]
    async fn set_and_reload_text_extensions() {
        let r = setup().await;
        r.set_text_extensions(vec!["md".into(), "txt".into()], None)
            .await
            .unwrap();
        let cfg = r.load().await.unwrap();
        assert_eq!(cfg.text_extensions, vec!["md", "txt"]);
    }

    #[tokio::test]
    async fn defaults_include_max_file_size_and_extensions() {
        let cfg = RuntimeConfig::default();
        assert_eq!(cfg.max_file_size, 100 * 1024 * 1024);
        assert!(cfg.text_extensions.contains(&"md".to_string()));
    }
}
