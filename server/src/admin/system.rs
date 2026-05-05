use serde::Serialize;
use std::path::{Path, PathBuf};
use sysinfo::{Disks, System};

#[derive(Debug, Clone, Serialize)]
pub struct SystemMetrics {
    pub cpu_percent: f32,
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
    system.refresh_all();
    let cpu_percent = system.global_cpu_info().cpu_usage();
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
        memory_used_mb,
        memory_total_mb,
        disk_used_bytes,
        disk_total_bytes,
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
}
