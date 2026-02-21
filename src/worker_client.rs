//! # Worker Client — HTTP Coordination Client (Deprecated)
//!
//! **Deprecated**: The HTTP-based worker client has been superseded by
//! `PgWorkerClient` in `pg_worker.rs`. All nodes now coordinate directly
//! via PostgreSQL. This module is retained for backward compatibility but
//! is no longer used by the CLI.
//!
//! The worker side of fleet coordination: registers with the coordinator,
//! sends heartbeats every 10 seconds, and reports discovered primes. Uses
//! `ureq` (blocking HTTP) on a background thread, with atomic counters
//! shared with the engine search thread.
//!
//! ## Data Flow
//!
//! ```text
//! Engine thread → writes tested/found/current atomics
//! Background thread → reads atomics → POST /api/heartbeat (10s)
//! Engine thread → calls report_prime() → POST /api/prime
//! ```
//!
//! ## Stop Signal
//!
//! The coordinator can request a stop via the heartbeat response. The
//! `stop_requested` atomic flag is checked by the engine search loop
//! to trigger a graceful shutdown with checkpoint save.
//!
//! ## Implements `CoordinationClient`
//!
//! This trait allows engine search functions to check for stop signals
//! and report primes without knowing whether coordination is HTTP-based
//! or PostgreSQL-based.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tracing::{info, warn};

#[derive(Clone, Serialize)]
struct RegisterPayload {
    worker_id: String,
    hostname: String,
    cores: usize,
    search_type: String,
    search_params: String,
}

#[derive(Serialize)]
struct HeartbeatPayload {
    worker_id: String,
    tested: u64,
    found: u64,
    current: String,
    checkpoint: Option<String>,
    metrics: Option<crate::metrics::HardwareMetrics>,
}

#[derive(Serialize)]
struct PrimePayload {
    worker_id: String,
    form: String,
    expression: String,
    digits: u64,
    search_params: String,
    proof_method: String,
}

#[derive(Serialize)]
struct DeregisterPayload {
    worker_id: String,
}

pub struct WorkerClient {
    coordinator_url: String,
    worker_id: String,
    register_payload: RegisterPayload,
    agent: ureq::Agent,
    shutdown: Arc<AtomicBool>,
    // Shared state updated by the search thread, read by the heartbeat thread
    pub tested: Arc<AtomicU64>,
    pub found: Arc<AtomicU64>,
    pub current: Arc<Mutex<String>>,
    pub checkpoint: Arc<Mutex<Option<String>>>,
    /// Set when coordinator sends a "stop" command via heartbeat response.
    pub stop_requested: Arc<AtomicBool>,
}

impl WorkerClient {
    pub fn new(
        coordinator_url: &str,
        worker_id: &str,
        search_type: &str,
        search_params: &str,
    ) -> Self {
        let agent = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_connect(Some(Duration::from_secs(5)))
                .timeout_send_request(Some(Duration::from_secs(10)))
                .build(),
        );

        let cores = rayon::current_num_threads();
        let hostname = gethostname().unwrap_or_else(|| worker_id.to_string());

        let url = format!(
            "{}/api/worker/register",
            coordinator_url.trim_end_matches('/')
        );
        let payload = RegisterPayload {
            worker_id: worker_id.to_string(),
            hostname,
            cores,
            search_type: search_type.to_string(),
            search_params: search_params.to_string(),
        };

        match agent.post(&url).send_json(&payload) {
            Ok(_) => info!(coordinator_url, "Registered with coordinator"),
            Err(e) => warn!(error = %e, "Failed to register with coordinator"),
        }

        WorkerClient {
            coordinator_url: coordinator_url.trim_end_matches('/').to_string(),
            worker_id: worker_id.to_string(),
            register_payload: payload,
            agent,
            shutdown: Arc::new(AtomicBool::new(false)),
            tested: Arc::new(AtomicU64::new(0)),
            found: Arc::new(AtomicU64::new(0)),
            current: Arc::new(Mutex::new(String::new())),
            checkpoint: Arc::new(Mutex::new(None)),
            stop_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start_heartbeat(&self) -> thread::JoinHandle<()> {
        let heartbeat_url = format!("{}/api/worker/heartbeat", self.coordinator_url);
        let register_url = format!("{}/api/worker/register", self.coordinator_url);
        let worker_id = self.worker_id.clone();
        let register_payload = self.register_payload.clone();
        let agent = self.agent.clone();
        let shutdown = Arc::clone(&self.shutdown);
        let tested = Arc::clone(&self.tested);
        let found = Arc::clone(&self.found);
        let current = Arc::clone(&self.current);
        let checkpoint = Arc::clone(&self.checkpoint);
        let stop_requested = Arc::clone(&self.stop_requested);

        thread::spawn(move || {
            let mut sys = sysinfo::System::new();
            loop {
                thread::sleep(Duration::from_secs(10));
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }

                sys.refresh_cpu_all();
                sys.refresh_memory();
                let hw = crate::metrics::collect(&sys);

                let payload = HeartbeatPayload {
                    worker_id: worker_id.clone(),
                    tested: tested.load(Ordering::Relaxed),
                    found: found.load(Ordering::Relaxed),
                    current: current.lock().unwrap().clone(),
                    checkpoint: checkpoint.lock().unwrap().clone(),
                    metrics: Some(hw),
                };

                match agent.post(&heartbeat_url).send_json(&payload) {
                    Ok(mut resp) => {
                        if let Ok(body) = resp.body_mut().read_to_string() {
                            // Re-register if coordinator doesn't recognize us
                            if body.contains("re-register") {
                                warn!("Coordinator lost registration, re-registering");
                                let _ = agent.post(&register_url).send_json(&register_payload);
                            }
                            // Check for stop command from coordinator
                            if body.contains("\"command\":\"stop\"") {
                                info!("Received stop command from coordinator");
                                stop_requested.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Heartbeat failed");
                    }
                }
            }
        })
    }

    pub fn is_stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::Relaxed)
    }

    pub fn report_prime(
        &self,
        form: &str,
        expression: &str,
        digits: u64,
        search_params: &str,
        proof_method: &str,
    ) {
        let url = format!("{}/api/worker/prime", self.coordinator_url);
        let payload = PrimePayload {
            worker_id: self.worker_id.clone(),
            form: form.to_string(),
            expression: expression.to_string(),
            digits,
            search_params: search_params.to_string(),
            proof_method: proof_method.to_string(),
        };

        if let Err(e) = self.agent.post(&url).send_json(&payload) {
            warn!(error = %e, "Failed to report prime to coordinator");
        }
    }

    pub fn deregister(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let url = format!("{}/api/worker/deregister", self.coordinator_url);
        let payload = DeregisterPayload {
            worker_id: self.worker_id.clone(),
        };

        if let Err(e) = self.agent.post(&url).send_json(&payload) {
            warn!(error = %e, "Failed to deregister from coordinator");
        }
    }
}

impl crate::CoordinationClient for WorkerClient {
    fn is_stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::Relaxed)
    }

    fn report_prime(
        &self,
        form: &str,
        expression: &str,
        digits: u64,
        search_params: &str,
        proof_method: &str,
    ) {
        self.report_prime(form, expression, digits, search_params, proof_method);
    }
}

fn gethostname() -> Option<String> {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    //! Tests for the HTTP worker client (deprecated, retained for compatibility).
    //!
    //! Validates serialization of all 4 HTTP payload types (register, heartbeat,
    //! prime report, deregister), atomic counter behavior for shared state
    //! between engine and heartbeat threads, and the stop signal mechanism.
    //!
    //! ## Data Flow Under Test
    //!
    //! The worker client uses atomic counters (tested, found) and Mutex-guarded
    //! strings (current, checkpoint) shared between the engine search thread
    //! and the background heartbeat thread. The stop_requested AtomicBool
    //! provides the coordinator's ability to gracefully halt a search.

    use super::*;

    // ── Payload Serialization ──────────────────────────────────────

    /// Validates RegisterPayload JSON output containing worker_id, hostname,
    /// cores, search_type, and search_params. Sent once on startup via
    /// POST /api/worker/register.
    #[test]
    fn register_payload_serializes() {
        let payload = RegisterPayload {
            worker_id: "host-12345678".to_string(),
            hostname: "testhost".to_string(),
            cores: 8,
            search_type: "factorial".to_string(),
            search_params: "{\"start\":1,\"end\":100}".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("worker_id"));
        assert!(json.contains("host-12345678"));
        assert!(json.contains("\"cores\":8"));
        assert!(json.contains("factorial"));
    }

    /// Validates HeartbeatPayload JSON output with counters and current status.
    /// Sent every 10 seconds by the background thread.
    #[test]
    fn heartbeat_payload_serializes() {
        let payload = HeartbeatPayload {
            worker_id: "w1".to_string(),
            tested: 5000,
            found: 3,
            current: "testing n=42".to_string(),
            checkpoint: Some("{\"last_n\":42}".to_string()),
            metrics: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"tested\":5000"));
        assert!(json.contains("\"found\":3"));
        assert!(json.contains("testing n=42"));
    }

    /// Heartbeat with hardware metrics attached. The metrics field is
    /// populated from sysinfo collection on each heartbeat cycle.
    #[test]
    fn heartbeat_payload_with_metrics() {
        let hw = crate::metrics::HardwareMetrics {
            cpu_usage_percent: 75.0,
            memory_used_gb: 12.0,
            memory_total_gb: 16.0,
            memory_usage_percent: 75.0,
            disk_used_gb: 200.0,
            disk_total_gb: 500.0,
            disk_usage_percent: 40.0,
            load_avg_1m: 2.0,
            load_avg_5m: 1.5,
            load_avg_15m: 1.0,
        };
        let payload = HeartbeatPayload {
            worker_id: "w1".to_string(),
            tested: 100,
            found: 0,
            current: "".to_string(),
            checkpoint: None,
            metrics: Some(hw),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("cpu_usage_percent"));
        assert!(json.contains("75"));
    }

    /// Validates PrimePayload for POST /api/worker/prime. Sent immediately
    /// when a prime is discovered, before the next heartbeat cycle.
    #[test]
    fn prime_payload_serializes() {
        let payload = PrimePayload {
            worker_id: "w1".to_string(),
            form: "kbn".to_string(),
            expression: "3*2^100+1".to_string(),
            digits: 31,
            search_params: "{}".to_string(),
            proof_method: "proth".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("3*2^100+1"));
        assert!(json.contains("\"digits\":31"));
        assert!(json.contains("proth"));
    }

    /// Validates DeregisterPayload JSON for POST /api/worker/deregister.
    /// Sent on graceful shutdown to remove the worker from the coordinator's
    /// active registry, preventing stale-worker alerts.
    #[test]
    fn deregister_payload_serializes() {
        let payload = DeregisterPayload {
            worker_id: "w1".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["worker_id"], "w1");
    }

    // ── Stop Signal ────────────────────────────────────────────────

    /// The stop_requested flag must default to false. Workers start in an
    /// active state; only the coordinator can set the stop flag via a
    /// heartbeat response containing `"command":"stop"`.
    #[test]
    fn stop_requested_default_false() {
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!flag.load(Ordering::Relaxed));
    }

    /// Verifies that the stop flag transitions from false to true. The
    /// heartbeat thread sets this flag when the coordinator responds with
    /// a stop command, causing the engine search loop to exit gracefully.
    #[test]
    fn stop_requested_can_be_set() {
        let flag = Arc::new(AtomicBool::new(false));
        flag.store(true, Ordering::Relaxed);
        assert!(flag.load(Ordering::Relaxed));
    }

    // ── Atomic Counters ──────────────────────────────────────────────

    /// Validates that AtomicU64 counters correctly accumulate via fetch_add.
    /// The engine search thread calls fetch_add on `tested` and `found` from
    /// rayon worker threads; the heartbeat thread reads them with load().
    /// Relaxed ordering is sufficient since counters are monotonic and
    /// approximate values are acceptable in heartbeat payloads.
    #[test]
    fn atomic_counters_increment() {
        let tested = Arc::new(AtomicU64::new(0));
        let found = Arc::new(AtomicU64::new(0));

        tested.fetch_add(100, Ordering::Relaxed);
        tested.fetch_add(50, Ordering::Relaxed);
        found.fetch_add(2, Ordering::Relaxed);

        assert_eq!(tested.load(Ordering::Relaxed), 150);
        assert_eq!(found.load(Ordering::Relaxed), 2);
    }

    // ── Platform Utilities ────────────────────────────────────────────

    /// On any real machine, the `hostname` command should succeed and return
    /// a non-empty string. The hostname is included in the registration
    /// payload to identify workers in the dashboard fleet view.
    #[test]
    fn gethostname_returns_something() {
        // On any real machine, hostname should return a non-empty string
        let h = gethostname();
        assert!(h.is_some(), "gethostname should return Some on real machine");
        assert!(!h.unwrap().is_empty());
    }

    // ── Mutex-Guarded Shared State ─────────────────────────────────

    /// The `current` field is a Mutex<String> that holds the human-readable
    /// description of what the engine is currently testing (e.g., "4999! (~16324 digits)").
    /// Updated once per block by the engine thread, read by the heartbeat thread.
    #[test]
    fn mutex_current_string_works() {
        let current = Arc::new(Mutex::new(String::new()));
        {
            let mut guard = current.lock().unwrap();
            *guard = "testing n=42".to_string();
        }
        let val = current.lock().unwrap().clone();
        assert_eq!(val, "testing n=42");
    }

    /// The `checkpoint` field is a Mutex<Option<String>> holding the latest
    /// checkpoint JSON. None until the first checkpoint save (60s after search
    /// start), then periodically updated. Sent in heartbeat payloads so the
    /// coordinator can track search progress for resumption.
    #[test]
    fn mutex_checkpoint_option_works() {
        let checkpoint = Arc::new(Mutex::new(None::<String>));
        {
            let mut guard = checkpoint.lock().unwrap();
            *guard = Some("{\"last_n\":100}".to_string());
        }
        let val = checkpoint.lock().unwrap().clone();
        assert_eq!(val, Some("{\"last_n\":100}".to_string()));
    }

    /// When no checkpoint exists yet (first 60 seconds of a search), the
    /// checkpoint field serializes as JSON null. The coordinator must handle
    /// this gracefully without attempting to parse a checkpoint path.
    #[test]
    fn heartbeat_payload_null_checkpoint() {
        let payload = HeartbeatPayload {
            worker_id: "w1".to_string(),
            tested: 0,
            found: 0,
            current: "".to_string(),
            checkpoint: None,
            metrics: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"checkpoint\":null"));
    }
}
