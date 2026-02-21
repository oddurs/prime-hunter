//! # Fleet — Worker State Types
//!
//! Defines the `WorkerState` struct representing a connected worker in the
//! distributed search network. Used by the dashboard to display node status.
//!
//! **Note:** The in-memory `Fleet` registry has been removed. All worker state
//! is now sourced from PostgreSQL via `Database::get_all_workers()`. This module
//! retains `WorkerState` as a shared type used by routes_fleet, websocket, and
//! the dashboard background task.

use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct WorkerState {
    pub worker_id: String,
    pub hostname: String,
    pub cores: usize,
    pub search_type: String,
    pub search_params: String,
    pub tested: u64,
    pub found: u64,
    pub current: String,
    pub checkpoint: Option<String>,
    pub metrics: Option<crate::metrics::HardwareMetrics>,
    pub uptime_secs: u64,
    pub last_heartbeat_secs_ago: u64,
    #[serde(skip)]
    pub last_heartbeat: std::time::Instant,
    #[serde(skip)]
    pub registered_at: std::time::Instant,
}

#[cfg(test)]
mod tests {
    //! Tests for the WorkerState type used in fleet monitoring.
    //!
    //! Validates construction, serialization (with `#[serde(skip)]` exclusions),
    //! hardware metrics attachment, checkpoint handling, clone correctness, counter
    //! updates, stale worker detection, and JSON round-trip behavior.
    //!
    //! ## Serialization Contract
    //!
    //! `WorkerState` is serialized to JSON for the dashboard fleet view. The
    //! `last_heartbeat` and `registered_at` fields use `std::time::Instant` which
    //! is not serializable, so they are marked `#[serde(skip)]`. The computed
    //! fields `uptime_secs` and `last_heartbeat_secs_ago` provide the same
    //! information in a serializable form.

    use super::*;
    use std::time::Instant;

    /// Helper: create a WorkerState with sensible defaults for testing.
    /// Uses a `.local` hostname suffix to simulate LAN worker nodes.
    fn make_worker(id: &str, cores: usize) -> WorkerState {
        WorkerState {
            worker_id: id.into(),
            hostname: format!("{}.local", id),
            cores,
            search_type: "factorial".into(),
            search_params: "start=1 end=1000".into(),
            tested: 0,
            found: 0,
            current: String::new(),
            checkpoint: None,
            metrics: None,
            uptime_secs: 0,
            last_heartbeat_secs_ago: 0,
            last_heartbeat: Instant::now(),
            registered_at: Instant::now(),
        }
    }

    // ── Construction and Field Access ────────────────────────────────

    /// Validates that WorkerState fields are correctly initialized by the
    /// constructor. All counters start at zero; the search begins in an
    /// idle state with no candidates tested yet.
    #[test]
    fn worker_state_construction() {
        let worker = make_worker("worker-1", 16);
        assert_eq!(worker.worker_id, "worker-1");
        assert_eq!(worker.hostname, "worker-1.local");
        assert_eq!(worker.cores, 16);
        assert_eq!(worker.search_type, "factorial");
        assert_eq!(worker.tested, 0);
        assert_eq!(worker.found, 0);
    }

    // ── Serialization ──────────────────────────────────────────────

    /// Validates that `#[serde(skip)]` correctly excludes `last_heartbeat` and
    /// `registered_at` (Instant types) from JSON output, while all other fields
    /// are present. This is critical for the dashboard API — Instant is not
    /// serializable and would cause a runtime panic if included.
    #[test]
    fn worker_state_serialization_excludes_instants() {
        let worker = make_worker("worker-2", 8);
        let json = serde_json::to_value(&worker).unwrap();

        // Fields that should be present
        assert_eq!(json["worker_id"], "worker-2");
        assert_eq!(json["hostname"], "worker-2.local");
        assert_eq!(json["cores"], 8);
        assert_eq!(json["search_type"], "factorial");
        assert_eq!(json["tested"], 0);
        assert_eq!(json["found"], 0);
        assert_eq!(json["uptime_secs"], 0);
        assert_eq!(json["last_heartbeat_secs_ago"], 0);

        // #[serde(skip)] fields should NOT be present
        assert!(json.get("last_heartbeat").is_none());
        assert!(json.get("registered_at").is_none());
    }

    // ── Hardware Metrics ──────────────────────────────────────────

    /// Workers with hardware metrics attached must serialize the nested
    /// metrics object. The dashboard fleet view displays CPU, memory, and
    /// disk usage per worker node.
    #[test]
    fn worker_state_with_metrics() {
        let mut worker = make_worker("worker-3", 4);
        worker.metrics = Some(crate::metrics::HardwareMetrics {
            cpu_usage_percent: 85.0,
            memory_used_gb: 12.5,
            memory_total_gb: 32.0,
            memory_usage_percent: 39.1,
            disk_used_gb: 100.0,
            disk_total_gb: 500.0,
            disk_usage_percent: 20.0,
            load_avg_1m: 3.5,
            load_avg_5m: 3.2,
            load_avg_15m: 3.0,
        });
        let json = serde_json::to_value(&worker).unwrap();
        assert!(json["metrics"].is_object());
        assert_eq!(json["metrics"]["cpu_usage_percent"], 85.0);
        assert_eq!(json["metrics"]["memory_total_gb"], 32.0);
    }

    /// Before the first heartbeat with hardware metrics, the metrics field
    /// is None and serializes as JSON null. The dashboard must handle this
    /// by showing placeholder values (e.g., "N/A").
    #[test]
    fn worker_state_without_metrics() {
        let worker = make_worker("worker-4", 4);
        let json = serde_json::to_value(&worker).unwrap();
        assert!(json["metrics"].is_null());
    }

    // ── Checkpoint and Clone ────────────────────────────────────────

    /// Workers with active checkpoints store the file path for resumption.
    /// The coordinator can use this to restart a search from the last saved
    /// state if the worker disconnects.
    #[test]
    fn worker_state_with_checkpoint() {
        let mut worker = make_worker("worker-5", 8);
        worker.checkpoint = Some("/tmp/darkreach.checkpoint".into());
        assert_eq!(worker.checkpoint.as_deref(), Some("/tmp/darkreach.checkpoint"));
    }

    /// Clone must produce an independent copy. WorkerState is cloned when
    /// building the fleet overview response (dashboard reads a snapshot while
    /// the background task continues updating the original).
    #[test]
    fn worker_state_clone() {
        let worker = make_worker("worker-6", 16);
        let cloned = worker.clone();
        assert_eq!(cloned.worker_id, worker.worker_id);
        assert_eq!(cloned.cores, worker.cores);
    }

    // ── Counter Updates ────────────────────────────────────────────

    /// Simulates a heartbeat update: the coordinator writes tested/found
    /// counters, current candidate description, uptime, and heartbeat
    /// recency from the worker's heartbeat payload into the WorkerState.
    #[test]
    fn worker_state_update_counters() {
        let mut worker = make_worker("worker-7", 8);
        worker.tested = 50000;
        worker.found = 3;
        worker.current = "4999! (~16324 digits)".into();
        worker.uptime_secs = 3600;
        worker.last_heartbeat_secs_ago = 5;

        assert_eq!(worker.tested, 50000);
        assert_eq!(worker.found, 3);
        assert_eq!(worker.current, "4999! (~16324 digits)");
        assert_eq!(worker.uptime_secs, 3600);
        assert_eq!(worker.last_heartbeat_secs_ago, 5);
    }

    // ── Stale Worker Detection ─────────────────────────────────────

    /// A worker is considered stale when `last_heartbeat_secs_ago` exceeds
    /// the 60-second threshold. The dashboard marks stale workers with a
    /// warning indicator; the pruning loop removes them after extended absence.
    #[test]
    fn worker_state_stale_detection() {
        let mut worker = make_worker("worker-8", 4);
        // Simulate a stale worker: last_heartbeat_secs_ago > 60
        worker.last_heartbeat_secs_ago = 120;
        assert!(worker.last_heartbeat_secs_ago > 60, "Worker should be considered stale");
    }

    // ── JSON Round-Trip ────────────────────────────────────────────

    /// JSON round-trip via Value (not back to WorkerState, since Instant
    /// fields are skipped and cannot be deserialized). This mirrors how the
    /// frontend consumes the fleet API response.
    #[test]
    fn worker_state_json_roundtrip() {
        let worker = make_worker("worker-9", 32);
        let json_str = serde_json::to_string(&worker).unwrap();
        // We can deserialize back to a Value (not WorkerState, since Instant fields are skipped)
        let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(value["worker_id"], "worker-9");
        assert_eq!(value["cores"], 32);
    }

    /// Validates that search_type and search_params are preserved in JSON
    /// output. Different forms use different parameter formats (e.g., kbn
    /// uses "k=3 b=2 min_n=100000 max_n=200000").
    #[test]
    fn worker_state_search_params_formatting() {
        let mut worker = make_worker("worker-10", 8);
        worker.search_type = "kbn".into();
        worker.search_params = "k=3 b=2 min_n=100000 max_n=200000".into();

        let json = serde_json::to_value(&worker).unwrap();
        assert_eq!(json["search_type"], "kbn");
        assert!(json["search_params"].as_str().unwrap().contains("k=3"));
    }
}
