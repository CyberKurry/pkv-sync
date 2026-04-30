use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub network: NetworkConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    /// Required. Use `pkvsyncd genkey` to generate.
    pub deployment_key: String,
    /// Optional. If set, used to construct shareable URLs.
    #[serde(default)]
    pub public_host: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    /// IPs/CIDRs whose `X-Forwarded-For` we trust.
    pub trusted_proxies: Vec<IpNet>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub format: LoggingFormat,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LoggingFormat {
    #[default]
    Json,
    Pretty,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: LoggingFormat::Json,
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

impl LoggingFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Pretty => "pretty",
        }
    }
}

impl<'de> Deserialize<'de> for LoggingFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "json" => Ok(Self::Json),
            "pretty" => Ok(Self::Pretty),
            _ => Err(serde::de::Error::custom(format!(
                "unknown logging format '{value}', expected 'json' or 'pretty'"
            ))),
        }
    }
}

impl Config {
    pub fn load(path: &std::path::Path) -> Result<Self, ConfigError> {
        let raw =
            std::fs::read_to_string(path).map_err(|e| ConfigError::Read(path.to_path_buf(), e))?;
        toml::from_str(&raw).map_err(ConfigError::Parse)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("reading config file {0}: {1}")]
    Read(PathBuf, std::io::Error),
    #[error("parsing config: {0}")]
    Parse(#[from] toml::de::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn loads_minimal_valid_config() {
        let f = write_temp(
            r#"
            [server]
            bind_addr = "127.0.0.1:6710"
            deployment_key = "k_test"

            [storage]
            data_dir = "/var/lib/pkv-sync"
            db_path = "/var/lib/pkv-sync/metadata.db"

            [network]
            trusted_proxies = ["127.0.0.1/32", "::1/128"]
            "#,
        );
        let cfg = Config::load(f.path()).expect("config loads");
        assert_eq!(cfg.server.bind_addr.port(), 6710);
        assert_eq!(cfg.server.deployment_key, "k_test");
        assert_eq!(cfg.network.trusted_proxies.len(), 2);
        assert_eq!(cfg.logging.level, "info");
        assert_eq!(cfg.logging.format.as_str(), "json");
    }

    #[test]
    fn loads_pretty_logging_format() {
        let f = write_temp(
            r#"
            [server]
            bind_addr = "127.0.0.1:6710"
            deployment_key = "k_test"

            [storage]
            data_dir = "/x"
            db_path = "/x/y"

            [network]
            trusted_proxies = []

            [logging]
            level = "debug"
            format = "pretty"
            "#,
        );
        let cfg = Config::load(f.path()).expect("config loads");
        assert_eq!(cfg.logging.level, "debug");
        assert_eq!(cfg.logging.format.as_str(), "pretty");
    }

    #[test]
    fn rejects_unknown_logging_format() {
        let f = write_temp(
            r#"
            [server]
            bind_addr = "127.0.0.1:6710"
            deployment_key = "k_test"

            [storage]
            data_dir = "/x"
            db_path = "/x/y"

            [network]
            trusted_proxies = []

            [logging]
            format = "xml"
            "#,
        );
        let err = Config::load(f.path()).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("logging format"),
            "expected logging format parse error, got: {err}"
        );
    }

    #[test]
    fn reports_missing_required_field() {
        let f = write_temp(
            r#"
            [server]
            bind_addr = "127.0.0.1:6710"

            [storage]
            data_dir = "/x"
            db_path = "/x/y"

            [network]
            trusted_proxies = []
            "#,
        );
        let err = Config::load(f.path()).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("deployment_key"),
            "expected 'deployment_key' missing error, got: {err}"
        );
    }

    #[test]
    fn reports_io_error_for_missing_file() {
        let err = Config::load(std::path::Path::new("/nonexistent/path/x.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::Read(_, _)));
    }

    #[test]
    fn loads_optional_public_host() {
        let f = write_temp(
            r#"
            [server]
            bind_addr = "127.0.0.1:6710"
            deployment_key = "k_test"
            public_host = "sync.example.com"

            [storage]
            data_dir = "/x"
            db_path = "/x/y"

            [network]
            trusted_proxies = []
            "#,
        );
        let cfg = Config::load(f.path()).unwrap();
        assert_eq!(cfg.server.public_host.as_deref(), Some("sync.example.com"));
    }
}
