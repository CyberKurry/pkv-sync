use std::cmp::Ordering;

pub fn normalize_release_tag(tag: &str) -> Option<String> {
    let version = tag.trim().trim_start_matches('v');
    if version.is_empty() || version.contains('-') {
        return None;
    }
    parse_version(version)?;
    Some(version.to_string())
}

pub fn compare_versions(left: &str, right: &str) -> Ordering {
    let left = parse_version(left).unwrap_or([0, 0, 0]);
    let right = parse_version(right).unwrap_or([0, 0, 0]);
    left.cmp(&right)
}

fn parse_version(value: &str) -> Option<[u32; 3]> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some([major, minor, patch])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_stable_release_tags() {
        assert_eq!(normalize_release_tag(" v1.2.3 "), Some("1.2.3".to_string()));
        assert_eq!(normalize_release_tag("1.2"), Some("1.2".to_string()));
        assert_eq!(normalize_release_tag("v1.2.3-beta.1"), None);
        assert_eq!(normalize_release_tag("v1.2.3.4"), None);
        assert_eq!(normalize_release_tag("v1.x.3"), None);
    }

    #[test]
    fn compares_release_versions_numerically() {
        assert_eq!(compare_versions("1.10.0", "1.2.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("bad", "0.0.1"), Ordering::Less);
    }
}
