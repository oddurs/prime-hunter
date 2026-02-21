//! # PgWorkerClient — PostgreSQL-Based Coordination
//!
//! Drop-in alternative to [`WorkerClient`](crate::worker_client::WorkerClient)
//! that heartbeats directly to the `workers` PostgreSQL table instead of going
//! through an HTTP coordinator. This eliminates a network hop and lets workers
//! run independently as long as they have database connectivity.
//!
//! ## Data Flow
//!
//! ```text
//! Engine thread  → writes tested/found/current atomics
//! Background thread → reads atomics → SQL INSERT/UPDATE workers (10s)
//! Engine thread  → calls report_prime() → SQL INSERT primes
//! ```
//!
//! ## Shared State Pattern
//!
//! Identical to `WorkerClient`: `Arc<AtomicU64>` for counters, `Arc<Mutex<String>>`
//! for current candidate. The search thread writes, the heartbeat thread reads.
//! Stop signal comes from the `pending_command` column (set by the dashboard).
//!
//! ## Exponential Backoff
//!
//! On heartbeat failure, the interval doubles (10s → 20s → 40s → ... up to 300s).
//! On success, the interval resets to the base 10s. This prevents thundering herd
//! on temporary PG outages.
//!
//! ## Block Progress Reporting
//!
//! The `current_block_id` field tracks which block the worker is processing.
//! On each successful heartbeat, live progress (tested/found) is reported to
//! `update_block_progress`, enabling real-time block tracking and smarter reclaim.
//!
//! ## Auto-Selection
//!
//! `main.rs` chooses `PgWorkerClient` when no `--coordinator` URL is given,
//! falling back to `WorkerClient` when an HTTP coordinator is specified.

use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Base heartbeat interval in seconds.
const HEARTBEAT_BASE_SECS: u64 = 10;

/// Maximum heartbeat interval in seconds (after exponential backoff).
const HEARTBEAT_MAX_SECS: u64 = 300;

/// PostgreSQL-based worker client — heartbeats directly to the `workers` table.
/// Drop-in alternative to `WorkerClient` with the same shared-state pattern.
pub struct PgWorkerClient {
    pool: PgPool,
    rt_handle: tokio::runtime::Handle,
    worker_id: String,
    hostname: String,
    cores: i32,
    search_type: String,
    search_params: String,
    shutdown: Arc<AtomicBool>,
    pub tested: Arc<AtomicU64>,
    pub found: Arc<AtomicU64>,
    pub current: Arc<Mutex<String>>,
    pub checkpoint: Arc<Mutex<Option<String>>>,
    pub stop_requested: Arc<AtomicBool>,
    /// Currently processing block ID. Set by the work loop before processing
    /// each block, cleared after completion. Used by the heartbeat thread to
    /// report live block progress via `update_block_progress`.
    pub current_block_id: Arc<Mutex<Option<i64>>>,
}

impl PgWorkerClient {
    pub fn new(
        pool: PgPool,
        rt_handle: tokio::runtime::Handle,
        worker_id: &str,
        search_type: &str,
        search_params: &str,
    ) -> Self {
        let hostname = gethostname().unwrap_or_else(|| worker_id.to_string());
        let cores = rayon::current_num_threads() as i32;

        // Register immediately
        let wid = worker_id.to_string();
        let hn = hostname.clone();
        let st = search_type.to_string();
        let sp = search_params.to_string();
        let p = pool.clone();
        rt_handle
            .block_on(async {
                sqlx::query(
                    "INSERT INTO workers (worker_id, hostname, cores, search_type, search_params, last_heartbeat)
                 VALUES ($1, $2, $3, $4, $5, NOW())
                 ON CONFLICT (worker_id) DO UPDATE SET
                   hostname = EXCLUDED.hostname, cores = EXCLUDED.cores,
                   search_type = EXCLUDED.search_type, search_params = EXCLUDED.search_params,
                   last_heartbeat = NOW(), pending_command = NULL",
                )
                .bind(&wid)
                .bind(&hn)
                .bind(cores)
                .bind(&st)
                .bind(&sp)
                .execute(&p)
                .await
            })
            .expect("Failed to register worker in database");
        info!(worker_id = %worker_id, hostname = %hostname, cores, "registered with PostgreSQL");

        PgWorkerClient {
            pool,
            rt_handle,
            worker_id: worker_id.to_string(),
            hostname,
            cores,
            search_type: search_type.to_string(),
            search_params: search_params.to_string(),
            shutdown: Arc::new(AtomicBool::new(false)),
            tested: Arc::new(AtomicU64::new(0)),
            found: Arc::new(AtomicU64::new(0)),
            current: Arc::new(Mutex::new(String::new())),
            checkpoint: Arc::new(Mutex::new(None)),
            stop_requested: Arc::new(AtomicBool::new(false)),
            current_block_id: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start_heartbeat(&self) -> thread::JoinHandle<()> {
        let pool = self.pool.clone();
        let rt_handle = self.rt_handle.clone();
        let worker_id = self.worker_id.clone();
        let hostname = self.hostname.clone();
        let cores = self.cores;
        let search_type = self.search_type.clone();
        let search_params = self.search_params.clone();
        let shutdown = Arc::clone(&self.shutdown);
        let tested = Arc::clone(&self.tested);
        let found = Arc::clone(&self.found);
        let current = Arc::clone(&self.current);
        let checkpoint = Arc::clone(&self.checkpoint);
        let stop_requested = Arc::clone(&self.stop_requested);
        let current_block_id = Arc::clone(&self.current_block_id);

        thread::spawn(move || {
            let mut sys = sysinfo::System::new();
            let mut consecutive_failures: u32 = 0;
            loop {
                let interval_secs = if consecutive_failures == 0 {
                    HEARTBEAT_BASE_SECS
                } else {
                    (HEARTBEAT_BASE_SECS * 2u64.saturating_pow(consecutive_failures))
                        .min(HEARTBEAT_MAX_SECS)
                };
                thread::sleep(Duration::from_secs(interval_secs));
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }

                sys.refresh_cpu_all();
                sys.refresh_memory();
                let hw = crate::metrics::collect(&sys);
                let metrics_json = serde_json::to_value(&hw).ok();

                let t = tested.load(Ordering::Relaxed) as i64;
                let f = found.load(Ordering::Relaxed) as i64;
                let cur = current.lock().unwrap().clone();
                let cp = checkpoint.lock().unwrap().clone();

                let hb_start = std::time::Instant::now();
                let heartbeat_result: Result<Option<String>, _> = rt_handle.block_on(async {
                    sqlx::query_scalar("SELECT worker_heartbeat($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
                        .bind(&worker_id)
                        .bind(&hostname)
                        .bind(cores)
                        .bind(&search_type)
                        .bind(&search_params)
                        .bind(t)
                        .bind(f)
                        .bind(&cur)
                        .bind(cp.as_deref())
                        .bind(&metrics_json)
                        .fetch_one(&pool)
                        .await
                });
                let hb_rtt_ms = hb_start.elapsed().as_millis();

                match heartbeat_result {
                    Ok(command) => {
                        debug!(
                            worker_id = %worker_id,
                            rtt_ms = hb_rtt_ms,
                            "heartbeat ok"
                        );
                        if consecutive_failures > 0 {
                            info!(
                                worker_id = %worker_id,
                                failures = consecutive_failures,
                                interval_secs,
                                "heartbeat recovered"
                            );
                        }
                        consecutive_failures = 0;

                        if command.as_deref() == Some("stop") {
                            info!(worker_id = %worker_id, "received stop command from PostgreSQL");
                            stop_requested.store(true, Ordering::Relaxed);
                        }

                        // Report block progress if we're working on a block
                        let block_id = current_block_id.lock().unwrap().clone();
                        if let Some(bid) = block_id {
                            let _ = rt_handle.block_on(async {
                                sqlx::query("SELECT update_block_progress($1, $2, $3, NULL)")
                                    .bind(bid)
                                    .bind(t)
                                    .bind(f)
                                    .execute(&pool)
                                    .await
                            });
                        }
                    }
                    Err(e) => {
                        consecutive_failures = consecutive_failures.saturating_add(1);
                        let next_interval = (HEARTBEAT_BASE_SECS
                            * 2u64.saturating_pow(consecutive_failures))
                        .min(HEARTBEAT_MAX_SECS);
                        warn!(
                            worker_id = %worker_id,
                            attempt = consecutive_failures,
                            error = %e,
                            retry_secs = next_interval,
                            "heartbeat failed"
                        );
                    }
                }
            }
        })
    }

    pub fn deregister(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let pool = self.pool.clone();
        let worker_id = self.worker_id.clone();
        let _ = self.rt_handle.block_on(async {
            sqlx::query("DELETE FROM workers WHERE worker_id = $1")
                .bind(&worker_id)
                .execute(&pool)
                .await
        });
    }
}

impl crate::CoordinationClient for PgWorkerClient {
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
        let f = form.to_string();
        let e = expression.to_string();
        let sp = search_params.to_string();
        let pm = proof_method.to_string();
        let pool = self.pool.clone();
        let _ = self.rt_handle.block_on(async {
            sqlx::query(
                "INSERT INTO primes (form, expression, digits, found_at, search_params, proof_method)
                 VALUES ($1, $2, $3, NOW(), $4, $5)
                 ON CONFLICT (form, expression) DO NOTHING",
            )
            .bind(&f)
            .bind(&e)
            .bind(digits as i64)
            .bind(&sp)
            .bind(&pm)
            .execute(&pool)
            .await
        });
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
