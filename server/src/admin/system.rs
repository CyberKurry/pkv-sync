use serde::Serialize;
use sysinfo::{Disks, System};

#[derive(Debug, Clone, Serialize)]
pub struct SystemMetrics {
    pub cpu_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub disk_used_gb: u64,
    pub disk_total_gb: u64,
}

pub fn collect() -> SystemMetrics {
    let mut system = System::new_all();
    system.refresh_all();
    let cpu_percent = system.global_cpu_info().cpu_usage();
    let memory_total_mb = system.total_memory() / 1024 / 1024;
    let memory_used_mb = system.used_memory() / 1024 / 1024;

    let disks = Disks::new_with_refreshed_list();
    let total: u64 = disks.list().iter().map(|d| d.total_space()).sum();
    let available: u64 = disks.list().iter().map(|d| d.available_space()).sum();
    let used = total.saturating_sub(available);

    SystemMetrics {
        cpu_percent,
        memory_used_mb,
        memory_total_mb,
        disk_used_gb: used / 1024 / 1024 / 1024,
        disk_total_gb: total / 1024 / 1024 / 1024,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_returns_nonzero_memory() {
        let metrics = collect();
        assert!(metrics.memory_total_mb > 0);
    }
}
