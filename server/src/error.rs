use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("config: {0}")]
    Config(#[from] crate::config::ConfigError),

    #[error("database: {0}")]
    Db(#[from] sqlx::Error),

    #[error("migration: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("io error at {0}: {1}")]
    Io(PathBuf, #[source] std::io::Error),

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("internal error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_error_converts() {
        let toml_err: toml::de::Error =
            toml::from_str::<toml::Value>("not = valid = toml").unwrap_err();
        let cfg_err = crate::config::ConfigError::Parse(toml_err);
        let err: Error = cfg_err.into();
        assert!(err.to_string().starts_with("config:"));
    }

    #[test]
    fn invalid_config_renders() {
        let e = Error::InvalidConfig("bind_addr empty".into());
        assert_eq!(e.to_string(), "invalid configuration: bind_addr empty");
    }
}
