//! Outcome record for the opt-in upgrader (the marker's successor).
//!
//! The privileged updater (systemd script or Docker sidecar) consumes
//! `upgrade-request.json` and — instead of only deleting it — leaves a
//! small `upgrade-status.json` behind so the admin panel can distinguish
//! "request written" from "applied / refused / failed". One file, last
//! writer wins: it is a liveness record, not a log.

use crate::service::upgrade_signal::UpgradeRequest;
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
    /// Coarse stage the updater reported while in_progress: `pull` /
    /// `recreate` / `health`. Empty on terminal records and absent in
    /// legacy status files.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub step: String,
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

/// Whether a written request still awaits its updater pickup: the marker
/// exists and no in_progress status covers ITS target. A stale
/// in_progress for an older request must not block a new one — the
/// marker is the request's source of truth.
pub fn request_pending(data_dir: &Path) -> Option<UpgradeRequest> {
    let req = crate::service::upgrade_signal::read_request(data_dir)
        .ok()
        .flatten()?;
    match read_status(data_dir) {
        Some(st) if st.phase == UpgradePhase::InProgress && st.target == req.target_version => None,
        _ => Some(req),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::upgrade_signal::write_request;

    #[test]
    fn status_round_trips_and_omits_empty_optional_fields() {
        let dir = tempfile::tempdir().unwrap();
        let status = UpgradeStatus {
            phase: UpgradePhase::Success,
            from: "1.7.4".into(),
            target: "1.8.0".into(),
            at_unix: 1_794_000_000,
            reason: String::new(),
            step: String::new(),
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

    #[test]
    fn step_round_trips_and_defaults_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        // A legacy record without the step key deserializes with an empty step.
        let legacy = format!(
            "{{\"phase\": \"in_progress\", \"from\": \"1.8.0\", \"target\": \"1.8.1\", \"at_unix\": {}, \"reason\": \"\"}}",
            1_794_000_000
        );
        std::fs::write(status_path(dir.path()), legacy).unwrap();
        let read = read_status(dir.path()).unwrap();
        assert_eq!(read.phase, UpgradePhase::InProgress);
        assert_eq!(read.step, "");
        // New records round-trip the step.
        let status = UpgradeStatus {
            phase: UpgradePhase::InProgress,
            from: "1.8.0".into(),
            target: "1.8.1".into(),
            at_unix: 1_794_000_000,
            reason: String::new(),
            step: "pull".into(),
        };
        write_status(dir.path(), &status).unwrap();
        assert_eq!(read_status(dir.path()), Some(status.clone()));
        let raw = std::fs::read_to_string(status_path(dir.path())).unwrap();
        assert!(raw.contains("\"step\": \"pull\""), "step serialized: {raw}");
        // An empty step is omitted, matching from/reason.
        let done = UpgradeStatus {
            step: String::new(),
            ..status
        };
        write_status(dir.path(), &done).unwrap();
        let raw = std::fs::read_to_string(status_path(dir.path())).unwrap();
        assert!(!raw.contains("\"step\""), "empty step omitted: {raw}");
    }

    #[test]
    fn request_pending_tracks_marker_and_in_progress_target() {
        let dir = tempfile::tempdir().unwrap();
        // No marker at all.
        assert_eq!(request_pending(dir.path()), None);
        // Marker written, the updater has not reported anything yet.
        let req = UpgradeRequest::new("1.8.1", 1_794_000_000, "admin").unwrap();
        write_request(dir.path(), &req).unwrap();
        assert_eq!(request_pending(dir.path()), Some(req.clone()));
        // Same-target in_progress: the request is being applied.
        let st = UpgradeStatus {
            phase: UpgradePhase::InProgress,
            from: "1.8.0".into(),
            target: "1.8.1".into(),
            at_unix: 1_794_000_001,
            reason: String::new(),
            step: "pull".into(),
        };
        write_status(dir.path(), &st).unwrap();
        assert_eq!(request_pending(dir.path()), None);
        // A stale in_progress for a DIFFERENT target does not block the
        // new request — the marker is the request's source of truth.
        let req2 = UpgradeRequest::new("1.8.2", 1_794_000_002, "admin").unwrap();
        write_request(dir.path(), &req2).unwrap();
        assert_eq!(request_pending(dir.path()), Some(req2.clone()));
    }
}
