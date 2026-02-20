//! # Fleet — In-Memory Worker Registry
//!
//! Tracks connected workers in the distributed search fleet. Workers register
//! via HTTP and send heartbeats every 10 seconds. Workers missing 6 consecutive
//! heartbeats (60s) are pruned as stale.
//!
//! ## Data Flow
//!
//! ```text
//! Worker → POST /api/register → Fleet::register()
//! Worker → POST /api/heartbeat (10s) → Fleet::heartbeat()
//! Dashboard → Fleet::get_workers() → WebSocket push to browser
//! Background → Fleet::prune_stale(60s) → remove unresponsive workers
//! ```
//!
//! ## Pending Commands
//!
//! The coordinator can queue commands (e.g., "stop") for workers, delivered
//! in the next heartbeat response. This enables graceful shutdown without
//! requiring workers to poll a separate endpoint.

use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;

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
    pub last_heartbeat: Instant,
    #[serde(skip)]
    pub registered_at: Instant,
}

#[derive(Default)]
pub struct Fleet {
    workers: HashMap<String, WorkerState>,
    pending_commands: HashMap<String, String>,
}

impl Fleet {
    pub fn new() -> Self {
        Fleet {
            workers: HashMap::new(),
            pending_commands: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        worker_id: String,
        hostname: String,
        cores: usize,
        search_type: String,
        search_params: String,
    ) {
        let now = Instant::now();
        self.workers.insert(
            worker_id.clone(),
            WorkerState {
                worker_id,
                hostname,
                cores,
                search_type,
                search_params,
                tested: 0,
                found: 0,
                current: String::new(),
                checkpoint: None,
                metrics: None,
                uptime_secs: 0,
                last_heartbeat_secs_ago: 0,
                last_heartbeat: now,
                registered_at: now,
            },
        );
    }

    /// Process a heartbeat. Returns (known, pending_command).
    pub fn heartbeat(
        &mut self,
        worker_id: &str,
        tested: u64,
        found: u64,
        current: String,
        checkpoint: Option<String>,
        metrics: Option<crate::metrics::HardwareMetrics>,
    ) -> (bool, Option<String>) {
        if let Some(w) = self.workers.get_mut(worker_id) {
            w.tested = tested;
            w.found = found;
            w.current = current;
            w.checkpoint = checkpoint;
            w.metrics = metrics;
            w.last_heartbeat = Instant::now();
            w.uptime_secs = w.registered_at.elapsed().as_secs();
            let cmd = self.pending_commands.remove(worker_id);
            (true, cmd)
        } else {
            (false, None)
        }
    }

    /// Queue a command for a worker, delivered on next heartbeat.
    pub fn send_command(&mut self, worker_id: &str, command: String) {
        self.pending_commands.insert(worker_id.to_string(), command);
    }

    pub fn deregister(&mut self, worker_id: &str) {
        self.workers.remove(worker_id);
    }

    pub fn get_all(&self) -> Vec<WorkerState> {
        let now = Instant::now();
        self.workers
            .values()
            .map(|w| {
                let mut w = w.clone();
                w.uptime_secs = w.registered_at.elapsed().as_secs();
                w.last_heartbeat_secs_ago = now.duration_since(w.last_heartbeat).as_secs();
                w
            })
            .collect()
    }

    pub fn prune_stale(&mut self, timeout_secs: u64) {
        let now = Instant::now();
        self.workers
            .retain(|_, w| now.duration_since(w.last_heartbeat).as_secs() < timeout_secs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fleet_with_worker(id: &str) -> Fleet {
        let mut f = Fleet::new();
        f.register(id.into(), "host1".into(), 8, "kbn".into(), "{}".into());
        f
    }

    #[test]
    fn new_fleet_is_empty() {
        let f = Fleet::new();
        assert!(f.get_all().is_empty());
    }

    #[test]
    fn register_adds_worker() {
        let f = make_fleet_with_worker("w1");
        let workers = f.get_all();
        assert_eq!(workers.len(), 1);
        assert_eq!(workers[0].worker_id, "w1");
        assert_eq!(workers[0].hostname, "host1");
        assert_eq!(workers[0].cores, 8);
        assert_eq!(workers[0].search_type, "kbn");
    }

    #[test]
    fn register_duplicate_overwrites() {
        let mut f = make_fleet_with_worker("w1");
        f.register(
            "w1".into(),
            "host2".into(),
            16,
            "factorial".into(),
            "{}".into(),
        );
        let workers = f.get_all();
        assert_eq!(workers.len(), 1);
        assert_eq!(workers[0].hostname, "host2");
        assert_eq!(workers[0].cores, 16);
        assert_eq!(workers[0].search_type, "factorial");
    }

    #[test]
    fn heartbeat_known_worker_returns_true() {
        let mut f = make_fleet_with_worker("w1");
        let (known, cmd) = f.heartbeat("w1", 100, 5, "n=42".into(), None, None);
        assert!(known);
        assert!(cmd.is_none());
        let w = &f.get_all()[0];
        assert_eq!(w.tested, 100);
        assert_eq!(w.found, 5);
        assert_eq!(w.current, "n=42");
    }

    #[test]
    fn heartbeat_unknown_worker_returns_false() {
        let mut f = Fleet::new();
        let (known, cmd) = f.heartbeat("ghost", 0, 0, "".into(), None, None);
        assert!(!known);
        assert!(cmd.is_none());
    }

    #[test]
    fn send_command_delivered_on_heartbeat() {
        let mut f = make_fleet_with_worker("w1");
        f.send_command("w1", "stop".into());
        let (known, cmd) = f.heartbeat("w1", 0, 0, "".into(), None, None);
        assert!(known);
        assert_eq!(cmd, Some("stop".into()));

        // Command consumed — next heartbeat has no command
        let (_, cmd2) = f.heartbeat("w1", 0, 0, "".into(), None, None);
        assert!(cmd2.is_none());
    }

    #[test]
    fn deregister_removes_worker() {
        let mut f = make_fleet_with_worker("w1");
        f.deregister("w1");
        assert!(f.get_all().is_empty());
    }

    #[test]
    fn deregister_nonexistent_is_noop() {
        let mut f = make_fleet_with_worker("w1");
        f.deregister("w999");
        assert_eq!(f.get_all().len(), 1);
    }

    #[test]
    fn prune_stale_with_zero_timeout_removes_all() {
        let mut f = make_fleet_with_worker("w1");
        // timeout_secs=0 means elapsed_secs < 0 is always false → all pruned
        f.prune_stale(0);
        assert!(f.get_all().is_empty());
    }

    #[test]
    fn prune_stale_with_large_timeout_keeps_recent() {
        let mut f = make_fleet_with_worker("w1");
        f.prune_stale(1000);
        assert_eq!(f.get_all().len(), 1);
    }

    #[test]
    fn multiple_workers_independent() {
        let mut f = Fleet::new();
        f.register("w1".into(), "host1".into(), 4, "kbn".into(), "{}".into());
        f.register(
            "w2".into(),
            "host2".into(),
            8,
            "factorial".into(),
            "{}".into(),
        );
        f.register(
            "w3".into(),
            "host3".into(),
            16,
            "palindromic".into(),
            "{}".into(),
        );
        assert_eq!(f.get_all().len(), 3);

        f.deregister("w2");
        assert_eq!(f.get_all().len(), 2);

        let (known, _) = f.heartbeat("w1", 50, 2, "n=10".into(), None, None);
        assert!(known);
        let (known, _) = f.heartbeat("w2", 0, 0, "".into(), None, None);
        assert!(!known);
    }

    #[test]
    fn heartbeat_updates_checkpoint() {
        let mut f = make_fleet_with_worker("w1");
        let (known, _) = f.heartbeat("w1", 0, 0, "".into(), Some("cp-data".into()), None);
        assert!(known);
        let w = &f.get_all()[0];
        assert_eq!(w.checkpoint, Some("cp-data".into()));
    }

    #[test]
    fn get_all_computes_uptime() {
        let f = make_fleet_with_worker("w1");
        let workers = f.get_all();
        // Worker just registered — uptime should be 0 or very small
        assert!(workers[0].uptime_secs <= 1);
        assert!(workers[0].last_heartbeat_secs_ago <= 1);
    }
}
