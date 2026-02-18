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
