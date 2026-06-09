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

pub fn format_duration_seconds(seconds: u64, lang: &str) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let secs = seconds % 60;
    let lang = lang.to_ascii_lowercase();
    let (day_unit, hour_unit, minute_unit, second_unit) =
        if lang.starts_with("zh-hant") || lang.starts_with("zh-tw") || lang.starts_with("zh-hk") {
            (
                "\u{5929}",
                "\u{5c0f}\u{6642}",
                "\u{5206}",
                "\u{79d2}",
            )
        } else if lang.starts_with("zh") {
            (
                "\u{5929}",
                "\u{5c0f}\u{65f6}",
                "\u{5206}",
                "\u{79d2}",
            )
        } else if lang.starts_with("ja") {
            (
                "\u{65e5}",
                "\u{6642}\u{9593}",
                "\u{5206}",
                "\u{79d2}",
            )
        } else if lang.starts_with("ko") {
            (
                "\u{c77c}",
                "\u{c2dc}\u{ac04}",
                "\u{bd84}",
                "\u{cd08}",
            )
        } else {
            ("d", "h", "m", "s")
        };
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}{day_unit}"));
    }
    if hours > 0 {
        parts.push(format!("{hours}{hour_unit}"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}{minute_unit}"));
    }
    if secs > 0 || parts.is_empty() {
        parts.push(format!("{secs}{second_unit}"));
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
        assert_eq!(
            format_duration_seconds(105_802, "zh-Hant"),
            concat!("1\u{5929}", " 5\u{5c0f}\u{6642}", " 23\u{5206}", " 22\u{79d2}")
        );
        assert_eq!(
            format_duration_seconds(105_802, "ja"),
            concat!("1\u{65e5}", " 5\u{6642}\u{9593}", " 23\u{5206}", " 22\u{79d2}")
        );
        assert_eq!(
            format_duration_seconds(105_802, "ko"),
            concat!("1\u{c77c}", " 5\u{c2dc}\u{ac04}", " 23\u{bd84}", " 22\u{cd08}")
        );
    }
}
