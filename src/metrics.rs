//! # Metrics — Hardware Telemetry Collection
//!
//! Collects system-level hardware metrics from each worker node using the
//! [`sysinfo`] crate. Metrics are attached to heartbeat payloads (both HTTP
//! and PostgreSQL) and displayed on the dashboard for fleet monitoring.
//!
//! ## Collected Metrics
//!
//! | Metric | Source | Unit |
//! |--------|--------|------|
//! | CPU usage | `System::global_cpu_usage()` | percent (0–100) |
//! | Memory used/total | `System::used_memory()` / `total_memory()` | GiB |
//! | Disk used/total | `Disks::new_with_refreshed_list()` | GiB |
//! | Load averages | `System::load_average()` | 1m, 5m, 15m |
//!
//! ## Usage
//!
//! Called every 10 seconds by the heartbeat thread in both `WorkerClient`
//! and `PgWorkerClient`. The `sysinfo::System` instance is reused across
//! calls (passed by `&mut` reference) to amortize initialization cost.

use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct HardwareMetrics {
    pub cpu_usage_percent: f32,
    pub memory_used_gb: f64,
    pub memory_total_gb: f64,
    pub memory_usage_percent: f32,
    pub disk_used_gb: f64,
    pub disk_total_gb: f64,
    pub disk_usage_percent: f32,
    pub load_avg_1m: f64,
    pub load_avg_5m: f64,
    pub load_avg_15m: f64,
}

pub fn collect(sys: &System) -> HardwareMetrics {
    let cpu_usage = sys.global_cpu_usage();

    let mem_total = sys.total_memory() as f64;
    let mem_used = sys.used_memory() as f64;
    let mem_total_gb = mem_total / 1_073_741_824.0;
    let mem_used_gb = mem_used / 1_073_741_824.0;
    let mem_pct = if mem_total > 0.0 {
        (mem_used / mem_total * 100.0) as f32
    } else {
        0.0
    };

    let mut disk_total: u64 = 0;
    let mut disk_used: u64 = 0;
    for disk in sysinfo::Disks::new_with_refreshed_list().iter() {
        disk_total += disk.total_space();
        disk_used += disk.total_space() - disk.available_space();
    }
    let disk_total_f = disk_total as f64;
    let disk_used_f = disk_used as f64;
    let disk_total_gb = disk_total_f / 1_073_741_824.0;
    let disk_used_gb = disk_used_f / 1_073_741_824.0;
    let disk_pct = if disk_total_f > 0.0 {
        (disk_used_f / disk_total_f * 100.0) as f32
    } else {
        0.0
    };

    let load = System::load_average();

    HardwareMetrics {
        cpu_usage_percent: cpu_usage,
        memory_used_gb: (mem_used_gb * 10.0).round() / 10.0,
        memory_total_gb: (mem_total_gb * 10.0).round() / 10.0,
        memory_usage_percent: (mem_pct * 10.0).round() / 10.0,
        disk_used_gb: (disk_used_gb * 10.0).round() / 10.0,
        disk_total_gb: (disk_total_gb * 10.0).round() / 10.0,
        disk_usage_percent: (disk_pct * 10.0).round() / 10.0,
        load_avg_1m: (load.one * 100.0).round() / 100.0,
        load_avg_5m: (load.five * 100.0).round() / 100.0,
        load_avg_15m: (load.fifteen * 100.0).round() / 100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_returns_valid_percentages() {
        let mut sys = System::new_all();
        sys.refresh_all();
        let m = collect(&sys);

        assert!(m.cpu_usage_percent >= 0.0, "CPU usage negative");
        assert!(
            m.cpu_usage_percent <= 100.0 * sys.cpus().len() as f32,
            "CPU usage unreasonable"
        );
        assert!(
            m.memory_usage_percent >= 0.0,
            "Memory usage percent negative"
        );
        assert!(
            m.memory_usage_percent <= 100.0,
            "Memory usage percent > 100"
        );
        assert!(m.disk_usage_percent >= 0.0, "Disk usage percent negative");
        assert!(m.disk_usage_percent <= 100.0, "Disk usage percent > 100");
    }

    #[test]
    fn collect_returns_non_negative_gb() {
        let mut sys = System::new_all();
        sys.refresh_all();
        let m = collect(&sys);

        assert!(m.memory_used_gb >= 0.0, "Memory used GB negative");
        assert!(m.memory_total_gb >= 0.0, "Memory total GB negative");
        assert!(m.memory_used_gb <= m.memory_total_gb, "Used > total memory");
        assert!(m.disk_used_gb >= 0.0, "Disk used GB negative");
        assert!(m.disk_total_gb >= 0.0, "Disk total GB negative");
        assert!(m.disk_used_gb <= m.disk_total_gb, "Used > total disk");
    }

    #[test]
    fn collect_returns_non_negative_load() {
        let mut sys = System::new_all();
        sys.refresh_all();
        let m = collect(&sys);

        assert!(m.load_avg_1m >= 0.0, "Load 1m negative");
        assert!(m.load_avg_5m >= 0.0, "Load 5m negative");
        assert!(m.load_avg_15m >= 0.0, "Load 15m negative");
    }

    #[test]
    fn hardware_metrics_default_is_zeroed() {
        let m = HardwareMetrics::default();
        assert_eq!(m.cpu_usage_percent, 0.0);
        assert_eq!(m.memory_used_gb, 0.0);
        assert_eq!(m.memory_total_gb, 0.0);
        assert_eq!(m.disk_used_gb, 0.0);
        assert_eq!(m.disk_total_gb, 0.0);
        assert_eq!(m.load_avg_1m, 0.0);
    }

    #[test]
    fn hardware_metrics_serde_roundtrip() {
        let m = HardwareMetrics {
            cpu_usage_percent: 45.5,
            memory_used_gb: 8.2,
            memory_total_gb: 16.0,
            memory_usage_percent: 51.3,
            disk_used_gb: 100.0,
            disk_total_gb: 500.0,
            disk_usage_percent: 20.0,
            load_avg_1m: 2.5,
            load_avg_5m: 1.8,
            load_avg_15m: 1.2,
        };
        let json = serde_json::to_string(&m).unwrap();
        let parsed: HardwareMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.cpu_usage_percent, m.cpu_usage_percent);
        assert_eq!(parsed.memory_used_gb, m.memory_used_gb);
        assert_eq!(parsed.disk_total_gb, m.disk_total_gb);
        assert_eq!(parsed.load_avg_15m, m.load_avg_15m);
    }
}
