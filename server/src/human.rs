pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 || value.fract() == 0.0 {
        format!("{} {}", value as u64, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn format_i64_bytes(bytes: i64) -> String {
    format_bytes(bytes.max(0) as u64)
}

pub fn format_duration_seconds(seconds: u64, _lang: &str) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let secs = seconds % 60;
    let zh = _lang.eq_ignore_ascii_case("zh-CN") || _lang.eq_ignore_ascii_case("zh");
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(if zh {
            format!("{days}天")
        } else {
            format!("{days}d")
        });
    }
    if hours > 0 {
        parts.push(if zh {
            format!("{hours}小时")
        } else {
            format!("{hours}h")
        });
    }
    if minutes > 0 {
        parts.push(if zh {
            format!("{minutes}分")
        } else {
            format!("{minutes}m")
        });
    }
    if secs > 0 || parts.is_empty() {
        parts.push(if zh {
            format!("{secs}秒")
        } else {
            format!("{secs}s")
        });
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes_with_scaled_units() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_i64_bytes(-10), "0 B");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5 GB");
    }

    #[test]
    fn formats_duration_for_admin_language() {
        assert_eq!(format_duration_seconds(83, "en"), "1m 23s");
        assert_eq!(
            format_duration_seconds(105_802, "zh-CN"),
            "1天 5小时 23分 22秒"
        );
    }
}
