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
