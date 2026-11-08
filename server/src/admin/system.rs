use serde::Serialize;
use std::path::{Path, PathBuf};
use sysinfo::{Disks, System};

#[derive(Debug, Clone, Serialize)]
pub struct SystemMetrics {
    pub cpu_percent: f32,
    pub cpu_cores: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
}

#[derive(Debug, Clone)]
struct DiskSample {
    mount_point: PathBuf,
    total_space: u64,
    available_space: u64,
}

pub fn collect(data_dir: &Path) -> SystemMetrics {
    let mut system = System::new_all();
    // R14-M6: sysinfo's cpu_usage() is the DELTA between two CPU
    // refreshes — new_all() + a back-to-back refresh_all() measured a
    // microsecond window and reported ~0% forever (every dashboard load
    // also built a fresh System, so it never converged). Sleep the
    // library's own minimum interval between the samples; the caller
    // runs this inside spawn_blocking, so the wait blocks a pool thread
    // only.
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_all();
    let cpu_percent = system.global_cpu_info().cpu_usage();
    let cpu_cores = effective_cpu_cores(system.cpus().len().max(1), detect_cgroup_cpu_quota());
    let memory_total_mb = system.total_memory() / 1024 / 1024;
    let memory_used_mb = system.used_memory() / 1024 / 1024;

    let disks = Disks::new_with_refreshed_list();
    let disk_samples: Vec<DiskSample> = disks
        .list()
        .iter()
        .map(|d| DiskSample {
            mount_point: d.mount_point().to_path_buf(),
            total_space: d.total_space(),
            available_space: d.available_space(),
        })
        .collect();
    let data_dir = data_dir
        .canonicalize()
        .unwrap_or_else(|_| data_dir.to_path_buf());
    let (disk_used_bytes, disk_total_bytes) =
        disk_usage_for_path(&data_dir, &disk_samples).unwrap_or((0, 0));

    SystemMetrics {
        cpu_percent,
        cpu_cores,
        memory_used_mb,
        memory_total_mb,
        disk_used_bytes,
        disk_total_bytes,
    }
}

fn detect_cgroup_cpu_quota() -> Option<f32> {
    std::fs::read_to_string("/sys/fs/cgroup/cpu.max")
        .ok()
        .and_then(|raw| parse_cgroup_v2_cpu_max(&raw))
        .or_else(|| {
            let quota = std::fs::read_to_string("/sys/fs/cgroup/cpu/cpu.cfs_quota_us").ok()?;
            let period = std::fs::read_to_string("/sys/fs/cgroup/cpu/cpu.cfs_period_us").ok()?;
            parse_cgroup_v1_cpu_quota(&quota, &period)
        })
}

fn parse_cgroup_v2_cpu_max(raw: &str) -> Option<f32> {
    let mut parts = raw.split_whitespace();
    let quota = parts.next()?;
    if quota == "max" {
        return None;
    }
    let period = parts.next()?;
    parse_cpu_quota_values(quota, period)
}

fn parse_cgroup_v1_cpu_quota(quota: &str, period: &str) -> Option<f32> {
    parse_cpu_quota_values(quota.trim(), period.trim())
}

fn parse_cpu_quota_values(quota: &str, period: &str) -> Option<f32> {
    let quota = quota.parse::<f32>().ok()?;
    let period = period.parse::<f32>().ok()?;
    if quota <= 0.0 || period <= 0.0 {
        return None;
    }
    Some(quota / period)
}

fn effective_cpu_cores(logical_cores: usize, quota_cores: Option<f32>) -> f32 {
    let logical = logical_cores.max(1) as f32;
    quota_cores
        .filter(|cores| cores.is_finite() && *cores > 0.0)
        .map(|cores| cores.min(logical))
        .unwrap_or(logical)
}

/// One-line summary of the last updater outcome for the dashboard:
/// "<phase word> <from> → <target> (<X ago>)[ — reason]". Phase words are
/// localized via the i18n table; the arrow form is language-neutral.
pub(crate) fn format_upgrade_last_result(
    st: &crate::service::upgrade_status::UpgradeStatus,
    lang: &'static str,
) -> String {
    use crate::service::upgrade_status::UpgradePhase;
    let t = match lang {
        "zh-CN" => crate::admin::i18n::AdminText::zh_cn(),
        "zh-Hant" => crate::admin::i18n::AdminText::zh_hant(),
        "ja" => crate::admin::i18n::AdminText::ja(),
        "ko" => crate::admin::i18n::AdminText::ko(),
        _ => crate::admin::i18n::AdminText::en(),
    };
    let phase = match st.phase {
        UpgradePhase::InProgress => t.upgrade_phase_in_progress,
        UpgradePhase::Success => t.upgrade_phase_success,
        UpgradePhase::Refused => t.upgrade_phase_refused,
        UpgradePhase::Failed => t.upgrade_phase_failed,
    };
    let mut line = if st.from.is_empty() {
        format!("{phase} → {}", st.target)
    } else {
        format!("{phase} {} → {}", st.from, st.target)
    };
    let now = chrono::Utc::now().timestamp().max(0) as u64;
    let age = now.saturating_sub(st.at_unix);
    let ago = crate::human::format_duration_seconds(age, lang);
    line.push_str(&format!(" ({ago})"));
    if !st.reason.is_empty() {
        line.push_str(&format!(" — {}", st.reason));
    }
    line
}

pub(crate) fn format_cpu_cores(cores: f32, t: &crate::admin::i18n::AdminText) -> String {
    let rounded = cores.round();
    if (cores - rounded).abs() < 0.05 {
        let cores = rounded.max(1.0) as u32;
        format!("{cores} {}", cores_unit(t, cores == 1))
    } else {
        format!("{cores:.1} {}", cores_unit(t, false))
    }
}

/// `AdminText::cores` carries `singular|plural` for languages with plural
/// agreement and a single form for languages without it.
fn cores_unit(t: &crate::admin::i18n::AdminText, singular: bool) -> &'static str {
    match t.cores.split_once('|') {
        Some((one, _)) if singular => one,
        Some((_, many)) => many,
        None => t.cores,
    }
}

fn disk_usage_for_path(path: &Path, disks: &[DiskSample]) -> Option<(u64, u64)> {
    let disk = disks
        .iter()
        .filter(|disk| path_is_on_mount(path, &disk.mount_point))
        .max_by_key(|disk| normalized_path(&disk.mount_point).len())?;
    Some((
        disk.total_space.saturating_sub(disk.available_space),
        disk.total_space,
    ))
}

fn path_is_on_mount(path: &Path, mount_point: &Path) -> bool {
    if path.starts_with(mount_point) {
        return true;
    }
    let path = normalized_path(path);
    let mount = normalized_path(mount_point);
    if mount == "/" {
        return path.starts_with('/');
    }
    path == mount
        || path.starts_with(&format!("{mount}/"))
        || (mount.ends_with('/') && path.starts_with(&mount))
}

fn normalized_path(path: &Path) -> String {
    let mut value = path.to_string_lossy().replace('\\', "/").to_lowercase();
    if let Some(stripped) = value.strip_prefix("//?/") {
        value = stripped.to_string();
    }
    while value.len() > 1 && value.ends_with('/') && !value.ends_with(":/") {
        value.pop();
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_returns_nonzero_memory() {
        let metrics = collect(Path::new("."));
        assert!(metrics.memory_total_mb > 0);
        assert!(metrics.disk_total_bytes > 0);
    }

    #[test]
    fn cpu_sampling_waits_the_minimum_interval_between_refreshes() {
        // R14-M6: sysinfo's cpu_usage() is the DELTA between two CPU
        // refreshes; new_all() + back-to-back refresh_all() left the
        // dashboard at ~0% forever (every page load also built a fresh
        // System, so it never converged). The source contract: build →
        // sleep(MINIMUM_CPU_UPDATE_INTERVAL) → refresh → read. A
        // behavioral CPU assertion would be load-dependent and flake on
        // idle runners, so this pins the structure instead — searching
        // ONLY inside collect's own body (an unbounded search matches
        // this test's literals and passes either way, which is no
        // evidence at all).
        let source = include_str!("system.rs");
        let collect_start = source.find("pub fn collect").expect("collect fn");
        let collect_end = source[collect_start..]
            .find("\nfn ")
            .or_else(|| source[collect_start..].find("\nmod tests"))
            .expect("end of collect")
            + collect_start;
        let body = &source[collect_start..collect_end];
        let first = body.find("new_all()").expect("first refresh (new_all)");
        let sleep = body
            .find("MINIMUM_CPU_UPDATE_INTERVAL")
            .expect("sampling interval");
        // The second refresh is the first refresh_all() AFTER the sleep
        // (the comment between them names both calls).
        let second = body[sleep..].find("refresh_all()").expect("second refresh") + sleep;
        assert!(
            first < sleep && sleep < second,
            "collect must sample, sleep the sysinfo minimum interval, refresh again"
        );
    }

    #[test]
    fn disk_usage_uses_data_directory_mount_instead_of_summing_all_disks() {
        let disks = vec![
            DiskSample {
                mount_point: PathBuf::from("/"),
                total_space: 25,
                available_space: 10,
            },
            DiskSample {
                mount_point: PathBuf::from("/etc/hosts"),
                total_space: 25,
                available_space: 20,
            },
            DiskSample {
                mount_point: PathBuf::from("/data"),
                total_space: 25,
                available_space: 12,
            },
            DiskSample {
                mount_point: PathBuf::from("/mnt/big"),
                total_space: 147,
                available_space: 103,
            },
        ];

        assert_eq!(
            disk_usage_for_path(Path::new("/data/pkv-sync"), &disks),
            Some((13, 25))
        );
    }

    #[test]
    fn cgroup_v2_cpu_quota_limits_reported_cores() {
        let quota = parse_cgroup_v2_cpu_max("100000 100000\n");
        assert_eq!(effective_cpu_cores(4, quota), 1.0);
        assert_eq!(
            format_cpu_cores(1.0, &crate::admin::i18n::AdminText::en()),
            "1 core"
        );
    }

    #[test]
    fn cgroup_v1_cpu_quota_limits_reported_cores() {
        let quota = parse_cgroup_v1_cpu_quota("150000", "100000");
        assert_eq!(effective_cpu_cores(4, quota), 1.5);
        assert_eq!(
            format_cpu_cores(1.5, &crate::admin::i18n::AdminText::en()),
            "1.5 cores"
        );
    }

    #[test]
    fn cpu_cores_use_the_active_language_unit() {
        assert_eq!(
            format_cpu_cores(4.0, &crate::admin::i18n::AdminText::zh_cn()),
            "4 核"
        );
        assert_eq!(
            format_cpu_cores(2.0, &crate::admin::i18n::AdminText::ja()),
            "2 コア"
        );
    }
}
