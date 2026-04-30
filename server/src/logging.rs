use crate::config::{LoggingConfig, LoggingFormat};
use tracing_subscriber::{fmt, EnvFilter};

/// Initialize structured JSON logging to stdout.
///
/// Reads log level from `RUST_LOG` env var, defaulting to `info`.
pub fn init() {
    init_with_config(&LoggingConfig::default());
}

pub fn init_with_config(config: &LoggingConfig) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(config.level.clone()));

    let result = match config.format {
        LoggingFormat::Json => fmt()
            .json()
            .with_env_filter(filter)
            .with_current_span(false)
            .with_span_list(false)
            .try_init(),
        LoggingFormat::Pretty => fmt().pretty().with_env_filter(filter).try_init(),
    };
    let _ = result;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_does_not_panic() {
        init();
    }

    #[test]
    fn init_accepts_pretty_config() {
        init_with_config(&LoggingConfig {
            level: "debug".into(),
            format: LoggingFormat::Pretty,
        });
    }
}
