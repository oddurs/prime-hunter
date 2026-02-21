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
    //! Tests for hardware telemetry collection.
    //!
    //! Validates that collect() produces sane metric values from the sysinfo
    //! crate, that HardwareMetrics serializes/deserializes correctly, and that
    //! rounding precision matches the expected 1-decimal (GB) and 2-decimal
    //! (load averages) precision targets.
    //!
    //! ## Testing Strategy
    //!
    //! - **Range validation**: percentages in [0, 100], GB values non-negative,
    //!   used <= total for both memory and disk
    //! - **Rounding**: GB values rounded to 1 decimal, load averages to 2 decimals
    //! - **Serialization**: JSON round-trip preserves all 10 fields
    //! - **Real hardware**: on any machine, total memory and disk must be > 0

    use super::*;

    // ── Range Validation ───────────────────────────────────────────

    /// Validates that all percentage metrics are in valid ranges. CPU usage
    /// can exceed 100% on multi-core systems (per-core reporting), but memory
    /// and disk percentages must be clamped to [0, 100].
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

    /// GB values must be non-negative and used must not exceed total.
    /// Negative values would indicate integer underflow in the conversion.
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

    /// Load averages must be non-negative. On Linux/macOS these come from
    /// the kernel's exponential moving average over 1, 5, and 15 minutes.
    #[test]
    fn collect_returns_non_negative_load() {
        let mut sys = System::new_all();
        sys.refresh_all();
        let m = collect(&sys);

        assert!(m.load_avg_1m >= 0.0, "Load 1m negative");
        assert!(m.load_avg_5m >= 0.0, "Load 5m negative");
        assert!(m.load_avg_15m >= 0.0, "Load 15m negative");
    }

    // ── Default and Serialization ─────────────────────────────────

    /// Default HardwareMetrics must be all zeros. This is used as the initial
    /// state before the first heartbeat collection.
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

    /// JSON round-trip must preserve all field values. HardwareMetrics is
    /// serialized in heartbeat payloads and stored in PostgreSQL.
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

    /// Clone must produce an identical copy. Metrics are cloned when attached
    /// to both the heartbeat payload and the WebSocket push message.
    #[test]
    fn hardware_metrics_clone() {
        let m = HardwareMetrics {
            cpu_usage_percent: 99.9,
            memory_used_gb: 15.5,
            memory_total_gb: 16.0,
            memory_usage_percent: 96.9,
            disk_used_gb: 450.0,
            disk_total_gb: 500.0,
            disk_usage_percent: 90.0,
            load_avg_1m: 4.0,
            load_avg_5m: 3.5,
            load_avg_15m: 3.0,
        };
        let cloned = m.clone();
        assert_eq!(cloned.cpu_usage_percent, m.cpu_usage_percent);
        assert_eq!(cloned.memory_used_gb, m.memory_used_gb);
        assert_eq!(cloned.disk_total_gb, m.disk_total_gb);
        assert_eq!(cloned.load_avg_15m, m.load_avg_15m);
    }

    /// JSON representation must contain exactly 10 fields. Adding or removing
    /// fields requires frontend dashboard updates, so this guards against
    /// accidental schema changes.
    #[test]
    fn hardware_metrics_json_has_all_fields() {
        let m = HardwareMetrics::default();
        let json = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 10, "HardwareMetrics should have 10 fields");
        assert!(obj.contains_key("cpu_usage_percent"));
        assert!(obj.contains_key("memory_used_gb"));
        assert!(obj.contains_key("memory_total_gb"));
        assert!(obj.contains_key("memory_usage_percent"));
        assert!(obj.contains_key("disk_used_gb"));
        assert!(obj.contains_key("disk_total_gb"));
        assert!(obj.contains_key("disk_usage_percent"));
        assert!(obj.contains_key("load_avg_1m"));
        assert!(obj.contains_key("load_avg_5m"));
        assert!(obj.contains_key("load_avg_15m"));
    }

    // ── Rounding Precision ───────────────────────────────────────

    /// Validates the rounding strategy: GB values are rounded to 1 decimal
    /// place (multiply by 10, round, divide by 10) and load averages to
    /// 2 decimal places. This prevents noisy fractional digits in the
    /// dashboard display.
    #[test]
    fn collect_rounding_precision() {
        // Verify that collect rounds to one decimal place for GB values
        // and two decimal places for load averages (based on the * 10.0 / 10.0 pattern)
        let mut sys = System::new_all();
        sys.refresh_all();
        let m = collect(&sys);

        // Check one-decimal rounding: multiply by 10, should have no fractional part
        let mem_used_x10 = m.memory_used_gb * 10.0;
        assert!(
            (mem_used_x10 - mem_used_x10.round()).abs() < 0.001,
            "memory_used_gb should be rounded to 1 decimal"
        );

        let mem_total_x10 = m.memory_total_gb * 10.0;
        assert!(
            (mem_total_x10 - mem_total_x10.round()).abs() < 0.001,
            "memory_total_gb should be rounded to 1 decimal"
        );

        // Check two-decimal rounding for load averages
        let load_1m_x100 = m.load_avg_1m * 100.0;
        assert!(
            (load_1m_x100 - load_1m_x100.round()).abs() < 0.001,
            "load_avg_1m should be rounded to 2 decimals"
        );
    }

    // ── Forward Compatibility ───────────────────────────────────

    /// Extra JSON fields must be silently ignored during deserialization.
    /// This ensures older workers can receive heartbeats from newer
    /// coordinators that may add additional metric fields.
    #[test]
    fn hardware_metrics_deserialize_with_extra_fields() {
        // serde should ignore unknown fields by default
        let json = r#"{
            "cpu_usage_percent": 50.0,
            "memory_used_gb": 8.0,
            "memory_total_gb": 16.0,
            "memory_usage_percent": 50.0,
            "disk_used_gb": 100.0,
            "disk_total_gb": 500.0,
            "disk_usage_percent": 20.0,
            "load_avg_1m": 1.0,
            "load_avg_5m": 0.5,
            "load_avg_15m": 0.3,
            "extra_field": "should_be_ignored"
        }"#;
        let m: HardwareMetrics = serde_json::from_str(json).unwrap();
        assert_eq!(m.cpu_usage_percent, 50.0);
    }

    // ── Real Hardware ────────────────────────────────────────────

    /// On any real machine, total memory and disk must be positive. This
    /// guards against sysinfo returning zero on unsupported platforms.
    #[test]
    fn collect_returns_real_system_data() {
        // Verify that on a real machine, we get non-zero total memory and disk
        let mut sys = System::new_all();
        sys.refresh_all();
        let m = collect(&sys);
        assert!(m.memory_total_gb > 0.0, "Total memory should be > 0 on real hardware");
        assert!(m.disk_total_gb > 0.0, "Total disk should be > 0 on real hardware");
    }
}
