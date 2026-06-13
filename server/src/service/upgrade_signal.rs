use crate::version::normalize_release_tag;
use serde::{Deserialize, Serialize};
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
}
