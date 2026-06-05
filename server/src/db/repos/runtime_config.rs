use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

use crate::storage::text_kind::TextClassifier;

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
    pub text_classifier: Arc<TextClassifier>,
    pub enable_history_ui: bool,
    pub enable_diff_endpoint: bool,
    pub extra_exclude_globs: Vec<String>,
    pub inline_content_max_bytes: u32,
    pub sse_heartbeat_seconds: u64,
    pub push_debounce_ms: u32,
    pub enable_git_smart_http: bool,
    pub enable_metrics: bool,
    pub enable_auto_merge: bool,
    pub update_check_enabled: bool,
    pub update_check_interval_seconds: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        let text_extensions = vec![
            "md".into(),
            "canvas".into(),
            "base".into(),
            "json".into(),
            "txt".into(),
            "css".into(),
        ];
        Self {
            registration_mode: RegistrationMode::Disabled,
            server_name: "PKV Sync".into(),
            timezone: crate::time::DEFAULT_TIMEZONE.into(),
            login_failure_threshold: 10,
            login_window_seconds: 15 * 60,
            login_lock_seconds: 15 * 60,
            max_file_size: 100 * 1024 * 1024,
            text_classifier: Arc::new(TextClassifier::new(
                text_extensions.iter().map(String::as_str),
            )),
            text_extensions,
            enable_history_ui: true,
            enable_diff_endpoint: true,
            extra_exclude_globs: vec![],
            inline_content_max_bytes: 8192,
            sse_heartbeat_seconds: 30,
            push_debounce_ms: 250,
            enable_git_smart_http: false,
            enable_metrics: false,
            enable_auto_merge: true,
            update_check_enabled: true,
            update_check_interval_seconds: 86_400,
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
    async fn set_history_flags(
        &self,
        enable_history_ui: bool,
        enable_diff_endpoint: bool,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn set_extra_exclude_globs(
        &self,
        globs: Vec<String>,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn set_sse_heartbeat_seconds(
        &self,
        value: u64,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn set_push_debounce_ms(&self, value: u32, by: Option<&str>) -> Result<(), sqlx::Error>;
    async fn set_enable_git_smart_http(
        &self,
        value: bool,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn set_enable_metrics(&self, value: bool, by: Option<&str>) -> Result<(), sqlx::Error>;
    async fn set_enable_auto_merge(&self, value: bool, by: Option<&str>)
        -> Result<(), sqlx::Error>;
    async fn set_update_check_enabled(
        &self,
        value: bool,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn set_update_check_interval_seconds(
        &self,
        value: u64,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn seed_update_check_from_static_config(
        &self,
        enabled: bool,
        interval_seconds: u64,
    ) -> Result<(), sqlx::Error>;
    async fn set_inline_content_max_bytes(
        &self,
        value: u32,
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

fn runtime_config_from_rows(rows: Vec<(String, String)>) -> RuntimeConfig {
    let values: HashMap<String, String> = rows.into_iter().collect();
    let mut cfg = RuntimeConfig::default();
    if let Some(s) = read_json_value::<String>(&values, "registration_mode") {
        if let Some(m) = RegistrationMode::parse(&s) {
            cfg.registration_mode = m;
        }
    }
    if let Some(s) = read_json_value::<String>(&values, "server_name") {
        cfg.server_name = s;
    }
    if let Some(s) = read_json_value::<String>(&values, "timezone") {
        if let Some(timezone) = crate::time::normalize_timezone(&s) {
            cfg.timezone = timezone;
        }
    }
    if let Some(n) = read_json_value::<u32>(&values, "login_failure_threshold") {
        cfg.login_failure_threshold = n.max(1);
    }
    if let Some(n) = read_json_value::<u64>(&values, "login_window_seconds") {
        cfg.login_window_seconds = n.max(1);
    }
    if let Some(n) = read_json_value::<u64>(&values, "login_lock_seconds") {
        cfg.login_lock_seconds = n.max(1);
    }
    if let Some(n) = read_json_value::<u64>(&values, "max_file_size") {
        cfg.max_file_size = n.max(1024);
    }
    if let Some(exts) = read_json_value::<Vec<String>>(&values, "text_extensions") {
        cfg.text_extensions = exts;
        cfg.rebuild_text_classifier();
    }
    if let Some(enabled) = read_json_value::<bool>(&values, "enable_history_ui") {
        cfg.enable_history_ui = enabled;
    }
    if let Some(enabled) = read_json_value::<bool>(&values, "enable_diff_endpoint") {
        cfg.enable_diff_endpoint = enabled;
    }
    if let Some(globs) = read_json_value::<Vec<String>>(&values, "extra_exclude_globs") {
        cfg.extra_exclude_globs = globs;
    }
    if let Some(n) = read_json_value::<u32>(&values, "inline_content_max_bytes") {
        cfg.inline_content_max_bytes = n.max(1);
    }
    if let Some(n) = read_json_value::<u64>(&values, "sse_heartbeat_seconds") {
        cfg.sse_heartbeat_seconds = n.max(10);
    }
    if let Some(n) = read_json_value::<u32>(&values, "push_debounce_ms") {
        cfg.push_debounce_ms = n.max(1);
    }
    if let Some(enabled) = read_json_value::<bool>(&values, "enable_git_smart_http") {
        cfg.enable_git_smart_http = enabled;
    }
    if let Some(enabled) = read_json_value::<bool>(&values, "enable_metrics") {
        cfg.enable_metrics = enabled;
    }
    if let Some(enabled) = read_json_value::<bool>(&values, "enable_auto_merge") {
        cfg.enable_auto_merge = enabled;
    }
    if let Some(enabled) = read_json_value::<bool>(&values, "update_check.enabled") {
        cfg.update_check_enabled = enabled;
    }
    if let Some(n) = read_json_value::<u64>(&values, "update_check.interval_seconds") {
        cfg.update_check_interval_seconds = n.max(60);
    }
    cfg
}

impl RuntimeConfig {
    pub fn rebuild_text_classifier(&mut self) {
        self.text_classifier = Arc::new(TextClassifier::new(
            self.text_extensions.iter().map(String::as_str),
        ));
    }
}

fn read_json_value<T: DeserializeOwned>(values: &HashMap<String, String>, key: &str) -> Option<T> {
    values
        .get(key)
        .and_then(|value| serde_json::from_str::<T>(value).ok())
}

#[async_trait]
impl RuntimeConfigRepo for SqliteRuntimeConfigRepo {
    async fn load(&self) -> Result<RuntimeConfig, sqlx::Error> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM runtime_config")
            .fetch_all(&self.pool)
            .await?;
        Ok(runtime_config_from_rows(rows))
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
        let values = [
            (
                "login_failure_threshold",
                serde_json::to_string(&threshold.max(1)).unwrap(),
            ),
            (
                "login_window_seconds",
                serde_json::to_string(&window_seconds.max(1)).unwrap(),
            ),
            (
                "login_lock_seconds",
                serde_json::to_string(&lock_seconds.max(1)).unwrap(),
            ),
        ];
        let now = chrono::Utc::now().timestamp();
        let mut tx = self.pool.begin().await?;
        for (key, value) in values {
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
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
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

    async fn set_history_flags(
        &self,
        enable_history_ui: bool,
        enable_diff_endpoint: bool,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        let mut tx = self.pool.begin().await?;
        for (key, value) in [
            (
                "enable_history_ui",
                serde_json::to_string(&enable_history_ui).unwrap(),
            ),
            (
                "enable_diff_endpoint",
                serde_json::to_string(&enable_diff_endpoint).unwrap(),
            ),
        ] {
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
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn set_extra_exclude_globs(
        &self,
        globs: Vec<String>,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let json = serde_json::to_string(&globs).unwrap_or_else(|_| "[]".into());
        write_kv(&self.pool, "extra_exclude_globs", &json, by).await
    }

    async fn set_sse_heartbeat_seconds(
        &self,
        value: u64,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        write_kv(
            &self.pool,
            "sse_heartbeat_seconds",
            &serde_json::to_string(&value.max(10)).unwrap(),
            by,
        )
        .await
    }

    async fn set_push_debounce_ms(&self, value: u32, by: Option<&str>) -> Result<(), sqlx::Error> {
        write_kv(
            &self.pool,
            "push_debounce_ms",
            &serde_json::to_string(&value.max(1)).unwrap(),
            by,
        )
        .await
    }

    async fn set_enable_git_smart_http(
        &self,
        value: bool,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        write_kv(
            &self.pool,
            "enable_git_smart_http",
            &serde_json::to_string(&value).unwrap(),
            by,
        )
        .await
    }

    async fn set_enable_metrics(&self, value: bool, by: Option<&str>) -> Result<(), sqlx::Error> {
        write_kv(
            &self.pool,
            "enable_metrics",
            &serde_json::to_string(&value).unwrap(),
            by,
        )
        .await
    }

    async fn set_enable_auto_merge(
        &self,
        value: bool,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        write_kv(
            &self.pool,
            "enable_auto_merge",
            &serde_json::to_string(&value).unwrap(),
            by,
        )
        .await
    }

    async fn set_update_check_enabled(
        &self,
        value: bool,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        write_kv(
            &self.pool,
            "update_check.enabled",
            &serde_json::to_string(&value).unwrap(),
            by,
        )
        .await
    }

    async fn set_update_check_interval_seconds(
        &self,
        value: u64,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        write_kv(
            &self.pool,
            "update_check.interval_seconds",
            &serde_json::to_string(&value.max(60)).unwrap(),
            by,
        )
        .await
    }

    async fn seed_update_check_from_static_config(
        &self,
        enabled: bool,
        interval_seconds: u64,
    ) -> Result<(), sqlx::Error> {
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT key, value, updated_by FROM runtime_config
             WHERE key IN ('update_check.enabled', 'update_check.interval_seconds')",
        )
        .fetch_all(&self.pool)
        .await?;
        let enabled_row = rows
            .iter()
            .find(|(key, _, _)| key == "update_check.enabled");
        let interval_row = rows
            .iter()
            .find(|(key, _, _)| key == "update_check.interval_seconds");
        let should_seed = matches!(enabled_row, Some((_, value, None)) if value == "true")
            && matches!(interval_row, Some((_, value, None)) if value == "86400");
        if !should_seed {
            return Ok(());
        }

        let now = chrono::Utc::now().timestamp();
        let values = [
            (
                "update_check.enabled",
                serde_json::to_string(&enabled).unwrap(),
            ),
            (
                "update_check.interval_seconds",
                serde_json::to_string(&interval_seconds.max(60)).unwrap(),
            ),
        ];
        let mut tx = self.pool.begin().await?;
        for (key, value) in values {
            sqlx::query(
                "UPDATE runtime_config
                 SET value = ?, updated_at = ?, updated_by = NULL
                 WHERE key = ?",
            )
            .bind(value)
            .bind(now)
            .bind(key)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn set_inline_content_max_bytes(
        &self,
        value: u32,
        by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        write_kv(
            &self.pool,
            "inline_content_max_bytes",
            &serde_json::to_string(&value.max(1)).unwrap(),
            by,
        )
        .await
    }
}

/// Hot-reloadable cache shared by handlers.
#[derive(Clone)]
pub struct RuntimeConfigCache(pub Arc<RwLock<RuntimeConfig>>);

impl RuntimeConfigCache {
    pub fn new(mut cfg: RuntimeConfig) -> Self {
        cfg.rebuild_text_classifier();
        Self(Arc::new(RwLock::new(cfg)))
    }

    pub async fn snapshot(&self) -> RuntimeConfig {
        self.0.read().await.clone()
    }

    pub async fn replace(&self, mut cfg: RuntimeConfig) {
        cfg.rebuild_text_classifier();
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

    #[test]
    fn config_from_rows_applies_known_values_and_ignores_invalid_rows() {
        let cfg = runtime_config_from_rows(vec![
            ("registration_mode".into(), "\"open\"".into()),
            ("server_name".into(), "\"Team PKV\"".into()),
            ("timezone".into(), "\"UTC\"".into()),
            ("login_failure_threshold".into(), "0".into()),
            ("max_file_size".into(), "1".into()),
            ("enable_metrics".into(), "true".into()),
            ("enable_auto_merge".into(), "false".into()),
            ("update_check.enabled".into(), "false".into()),
            ("update_check.interval_seconds".into(), "3600".into()),
            ("unknown_key".into(), "\"ignored\"".into()),
            ("push_debounce_ms".into(), "\"not a number\"".into()),
        ]);

        assert_eq!(cfg.registration_mode, RegistrationMode::Open);
        assert_eq!(cfg.server_name, "Team PKV");
        assert_eq!(cfg.timezone, "UTC");
        assert_eq!(cfg.login_failure_threshold, 1);
        assert_eq!(cfg.max_file_size, 1024);
        assert!(cfg.enable_metrics);
        assert!(!cfg.enable_auto_merge);
        assert_eq!(
            cfg.push_debounce_ms,
            RuntimeConfig::default().push_debounce_ms
        );
        assert!(!cfg.update_check_enabled);
        assert_eq!(cfg.update_check_interval_seconds, 3600);
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
                text_classifier: RuntimeConfig::default().text_classifier.clone(),
                enable_history_ui: true,
                enable_diff_endpoint: true,
                extra_exclude_globs: vec![],
                inline_content_max_bytes: 8192,
                sse_heartbeat_seconds: 30,
                push_debounce_ms: 250,
                enable_git_smart_http: false,
                enable_metrics: false,
                enable_auto_merge: true,
                update_check_enabled: true,
                update_check_interval_seconds: 86_400,
            })
            .await;
        let snap2 = cache.snapshot().await;
        assert_eq!(snap2.registration_mode, RegistrationMode::Open);
    }

    #[tokio::test]
    async fn cache_replace_rebuilds_text_classifier_from_extensions() {
        let cache = RuntimeConfigCache::new(RuntimeConfig::default());
        let mut cfg = RuntimeConfig {
            text_extensions: vec!["foo".into()],
            ..RuntimeConfig::default()
        };
        cfg.text_classifier = RuntimeConfig::default().text_classifier.clone();

        cache.replace(cfg).await;
        let snap = cache.snapshot().await;

        assert!(snap.text_classifier.is_text_path("note.foo"));
        assert!(!snap.text_classifier.is_text_path("note.md"));
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
        assert!(cfg.text_classifier.is_text_path("note.md"));
        assert!(!cfg.text_classifier.is_text_path("note.foo"));
    }

    #[tokio::test]
    async fn set_and_reload_history_flags() {
        let r = setup().await;
        r.set_history_flags(false, false, None).await.unwrap();
        let cfg = r.load().await.unwrap();
        assert!(!cfg.enable_history_ui);
        assert!(!cfg.enable_diff_endpoint);
    }

    #[tokio::test]
    async fn defaults_include_max_file_size_and_extensions() {
        let cfg = RuntimeConfig::default();
        assert_eq!(cfg.max_file_size, 100 * 1024 * 1024);
        assert!(cfg.text_extensions.contains(&"md".to_string()));
        assert!(cfg.enable_history_ui);
        assert!(cfg.enable_diff_endpoint);
        assert!(cfg.enable_auto_merge);
    }

    #[tokio::test]
    async fn set_and_reload_enable_auto_merge() {
        let r = setup().await;
        assert!(r.load().await.unwrap().enable_auto_merge);
        r.set_enable_auto_merge(false, None).await.unwrap();
        assert!(!r.load().await.unwrap().enable_auto_merge);
    }

    #[tokio::test]
    async fn set_and_reload_update_check_enabled() {
        let r = setup().await;
        assert!(r.load().await.unwrap().update_check_enabled);
        r.set_update_check_enabled(false, None).await.unwrap();
        assert!(!r.load().await.unwrap().update_check_enabled);
    }

    #[tokio::test]
    async fn set_and_reload_update_check_interval_seconds() {
        let r = setup().await;
        assert_eq!(
            r.load().await.unwrap().update_check_interval_seconds,
            86_400
        );
        r.set_update_check_interval_seconds(3600, None)
            .await
            .unwrap();
        assert_eq!(r.load().await.unwrap().update_check_interval_seconds, 3600);
    }

    #[tokio::test]
    async fn seed_update_check_from_static_config_applies_only_to_unmodified_defaults() {
        let r = setup().await;
        r.seed_update_check_from_static_config(false, 7200)
            .await
            .unwrap();
        let cfg = r.load().await.unwrap();
        assert!(!cfg.update_check_enabled);
        assert_eq!(cfg.update_check_interval_seconds, 7200);
    }

    #[tokio::test]
    async fn seed_update_check_from_static_config_preserves_admin_edits() {
        let r = setup().await;
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, is_admin, is_active, created_at)
             VALUES ('admin', 'admin', 'hash', 1, 1, 1)",
        )
        .execute(&r.pool)
        .await
        .unwrap();
        r.set_update_check_enabled(false, Some("admin"))
            .await
            .unwrap();
        r.set_update_check_interval_seconds(3600, Some("admin"))
            .await
            .unwrap();

        r.seed_update_check_from_static_config(true, 86_400)
            .await
            .unwrap();
        let cfg = r.load().await.unwrap();
        assert!(!cfg.update_check_enabled);
        assert_eq!(cfg.update_check_interval_seconds, 3600);
    }
}
