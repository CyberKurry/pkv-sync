use crate::version::normalize_release_tag;
use serde::{Deserialize, Serialize};
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
}
