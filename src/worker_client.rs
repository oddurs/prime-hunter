use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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
            Ok(_) => eprintln!("Registered with coordinator at {}", coordinator_url),
            Err(e) => eprintln!("Warning: failed to register with coordinator: {}", e),
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
                                eprintln!("Coordinator lost registration, re-registering...");
                                let _ = agent.post(&register_url).send_json(&register_payload);
                            }
                            // Check for stop command from coordinator
                            if body.contains("\"command\":\"stop\"") {
                                eprintln!("Received stop command from coordinator");
                                stop_requested.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: heartbeat failed: {}", e);
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
            eprintln!("Warning: failed to report prime to coordinator: {}", e);
        }
    }

    pub fn deregister(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let url = format!("{}/api/worker/deregister", self.coordinator_url);
        let payload = DeregisterPayload {
            worker_id: self.worker_id.clone(),
        };

        if let Err(e) = self.agent.post(&url).send_json(&payload) {
            eprintln!("Warning: failed to deregister from coordinator: {}", e);
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
