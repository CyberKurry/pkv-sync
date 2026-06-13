use crate::version::{compare_versions, normalize_release_tag};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const MARKER_FILE: &str = "upgrade-request.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpgradeRequest {
    pub target_version: String,
    pub requested_at_unix: u64,
    pub requested_by: String,
}

impl UpgradeRequest {
    /// Returns `None` if `target` is not a stable `X.Y.Z` tag.
    pub fn new(target: &str, requested_at_unix: u64, requested_by: &str) -> Option<Self> {
        let version = normalize_release_tag(target)?;
        Some(Self {
            target_version: version,
            requested_at_unix,
            requested_by: requested_by.to_string(),
        })
    }
}

pub fn marker_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MARKER_FILE)
}

/// Writes the marker atomically (temp file + rename) so a crash never leaves a
/// half-written request for the privileged updater to read.
pub fn write_request(data_dir: &Path, req: &UpgradeRequest) -> std::io::Result<()> {
    let path = marker_path(data_dir);
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(req).map_err(std::io::Error::other)?;
    let mut file = fs::File::create(&tmp)?;
    file.write_all(&bytes)?;
    file.flush()?;
    fs::rename(&tmp, &path)
}

pub fn read_request(data_dir: &Path) -> std::io::Result<Option<UpgradeRequest>> {
    match fs::read(marker_path(data_dir)) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn clear_request(data_dir: &Path) -> std::io::Result<()> {
    match fs::remove_file(marker_path(data_dir)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Given the running version and the latest stable tag discovered by
/// update-check, returns the normalized target version when an upgrade is
/// available (strictly newer), or `None` when up to date / ahead / unparseable.
pub fn resolve_target(current: &str, latest_tag: &str) -> Option<String> {
    let latest = normalize_release_tag(latest_tag)?;
    match compare_versions(&latest, current) {
        Ordering::Greater => Some(latest),
        _ => None,
    }
}

/// One-click entry point used by the admin endpoint: if `latest_tag` is strictly
/// newer than `current`, write an upgrade-request marker into `data_dir` and
/// return the target version; otherwise leave the data dir untouched and return
/// `None`. The privileged updater watches for that marker.
pub fn request_upgrade(
    data_dir: &Path,
    current: &str,
    latest_tag: &str,
    now_unix: u64,
) -> std::io::Result<Option<String>> {
    let Some(target) = resolve_target(current, latest_tag) else {
        return Ok(None);
    };
    let req = UpgradeRequest::new(&target, now_unix, "admin")
        .expect("resolve_target only returns stable versions");
    write_request(data_dir, &req)?;
    Ok(Some(target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn marker_path_is_under_data_dir() {
        let p = marker_path(&PathBuf::from("/var/lib/pkv-sync"));
        assert_eq!(p, PathBuf::from("/var/lib/pkv-sync/upgrade-request.json"));
    }

    #[test]
    fn request_round_trips_through_json() {
        let req = UpgradeRequest {
            target_version: "1.4.0".into(),
            requested_at_unix: 1_750_000_000,
            requested_by: "admin".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: UpgradeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn rejects_non_stable_target_version() {
        assert!(UpgradeRequest::new("v1.4.0-beta.1", 1, "admin").is_none());
        assert!(UpgradeRequest::new("1.4.0", 1, "admin").is_some());
    }

    #[test]
    fn write_then_read_round_trips_and_clear_removes() {
        let dir = tempfile::tempdir().unwrap();
        let req = UpgradeRequest::new("1.4.0", 1_750_000_000, "admin").unwrap();
        write_request(dir.path(), &req).unwrap();
        assert_eq!(read_request(dir.path()).unwrap(), Some(req));
        clear_request(dir.path()).unwrap();
        assert_eq!(read_request(dir.path()).unwrap(), None);
    }

    #[test]
    fn read_missing_marker_is_none_not_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_request(dir.path()).unwrap(), None);
    }

    #[test]
    fn resolve_target_returns_newer_only() {
        assert_eq!(resolve_target("1.3.2", "1.4.0"), Some("1.4.0".to_string()));
        assert_eq!(resolve_target("1.4.0", "1.4.0"), None);
        assert_eq!(resolve_target("1.4.1", "1.4.0"), None);
        assert_eq!(resolve_target("1.3.2", "v1.4.0"), Some("1.4.0".to_string()));
        assert_eq!(resolve_target("1.3.2", "garbage"), None);
    }

    #[test]
    fn request_upgrade_writes_marker_when_newer() {
        let dir = tempfile::tempdir().unwrap();
        let written = request_upgrade(dir.path(), "1.3.2", "1.4.0", 1_750_000_000).unwrap();
        assert_eq!(written, Some("1.4.0".to_string()));
        let marker = read_request(dir.path()).unwrap().unwrap();
        assert_eq!(marker.target_version, "1.4.0");
        assert_eq!(marker.requested_by, "admin");
    }

    #[test]
    fn request_upgrade_noop_when_up_to_date() {
        let dir = tempfile::tempdir().unwrap();
        let written = request_upgrade(dir.path(), "1.4.0", "1.4.0", 1).unwrap();
        assert_eq!(written, None);
        assert!(read_request(dir.path()).unwrap().is_none());
    }
}
