//! Outcome record for the opt-in upgrader (the marker's successor).
//!
//! The privileged updater (systemd script or Docker sidecar) consumes
//! `upgrade-request.json` and — instead of only deleting it — leaves a
//! small `upgrade-status.json` behind so the admin panel can distinguish
//! "request written" from "applied / refused / failed". One file, last
//! writer wins: it is a liveness record, not a log.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const STATUS_FILE: &str = "upgrade-status.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradePhase {
    /// The updater picked the request up and is working on it.
    InProgress,
    /// Applied: the service now runs the target version.
    Success,
    /// Legitimately not applied (stale request, non-upgrade target,
    /// malformed marker). Nothing was changed.
    Refused,
    /// Applied then rolled back (failed health check), or the apply
    /// step itself failed mid-way and was rolled back.
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpgradeStatus {
    pub phase: UpgradePhase,
    /// Version the upgrade started from (empty when unknown).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub from: String,
    pub target: String,
    pub at_unix: u64,
    /// Machine-readable refusal/failure reason (absent on success).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
}

pub fn status_path(data_dir: &Path) -> PathBuf {
    data_dir.join(STATUS_FILE)
}

/// Atomically replace the status file (temp + rename, same discipline as
/// the request marker: a crash never leaves a half-written record).
pub fn write_status(data_dir: &Path, status: &UpgradeStatus) -> std::io::Result<()> {
    let path = status_path(data_dir);
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(status).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, &path)
}

/// `None` when no updater has reported yet (fresh install, or the opt-in
/// updater has never run). Unparseable records read as absent — a corrupt
/// status must not break the dashboard, it is informational only.
pub fn read_status(data_dir: &Path) -> Option<UpgradeStatus> {
    let bytes = std::fs::read(status_path(data_dir)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips_and_omits_empty_optional_fields() {
        let dir = tempfile::tempdir().unwrap();
        let status = UpgradeStatus {
            phase: UpgradePhase::Success,
            from: "1.7.4".into(),
            target: "1.8.0".into(),
            at_unix: 1_794_000_000,
            reason: String::new(),
        };
        write_status(dir.path(), &status).unwrap();
        assert_eq!(read_status(dir.path()), Some(status));
        let raw = std::fs::read_to_string(status_path(dir.path())).unwrap();
        assert!(!raw.contains("\"reason\""), "empty reason omitted: {raw}");
        assert!(!raw.contains("\"from\"") || raw.contains("\"from\": \"1.7.4\""));
    }

    #[test]
    fn missing_or_corrupt_status_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_status(dir.path()), None);
        std::fs::write(status_path(dir.path()), b"{not json").unwrap();
        assert_eq!(read_status(dir.path()), None);
    }

    #[test]
    fn phase_serializes_snake_case() {
        let json = serde_json::to_string(&UpgradePhase::InProgress).unwrap();
        assert_eq!(json, "\"in_progress\"");
        let failed: UpgradePhase = serde_json::from_str("\"failed\"").unwrap();
        assert_eq!(failed, UpgradePhase::Failed);
    }
}
