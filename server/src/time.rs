use chrono::{TimeZone, Utc};
use chrono_tz::Tz;

pub const DEFAULT_TIMEZONE: &str = "UTC";

pub fn normalize_timezone(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.eq_ignore_ascii_case("utc") {
        return Some(DEFAULT_TIMEZONE.into());
    }
    value.parse::<Tz>().ok()?;
    Some(value.into())
}

pub fn format_unix_seconds(timestamp: i64, timezone: &str) -> String {
    let tz = timezone.parse::<Tz>().unwrap_or(Tz::UTC);
    match Utc.timestamp_opt(timestamp, 0).single() {
        Some(dt) => {
            let local = dt.with_timezone(&tz);
            format!("{} {}", local.format("%Y-%m-%d %H:%M:%S %:z"), timezone)
        }
        None => timestamp.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_timezone_names() {
        assert_eq!(normalize_timezone("utc").as_deref(), Some("UTC"));
        assert_eq!(
            normalize_timezone("Asia/Shanghai").as_deref(),
            Some("Asia/Shanghai")
        );
        assert!(normalize_timezone("No/SuchZone").is_none());
    }

    #[test]
    fn formats_unix_seconds_in_timezone() {
        assert_eq!(
            format_unix_seconds(0, "Asia/Shanghai"),
            "1970-01-01 08:00:00 +08:00 Asia/Shanghai"
        );
    }
}
