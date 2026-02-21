//! Fleet API — network status overview and worker management.
//!
//! Provides fleet-level views of the worker network. Deployment management
//! has been removed — all coordination is now PostgreSQL-backed.

use axum::extract::{Path as AxumPath, State};
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use std::sync::Arc;
use tracing::{info, warn};

use super::AppState;
use crate::{fleet, metrics};

/// Per-host server summary, grouping workers by hostname and classifying
/// hosts as either "service" (coordinator) or "compute" (worker nodes).
#[derive(Serialize)]
pub(super) struct ServerInfo {
    hostname: String,
    role: String,
    metrics: Option<metrics::HardwareMetrics>,
    worker_count: usize,
    cores: usize,
    worker_ids: Vec<String>,
    total_tested: u64,
    total_found: u64,
    uptime_secs: u64,
}

#[derive(Serialize)]
pub(super) struct FleetData {
    workers: Vec<fleet::WorkerState>,
    servers: Vec<ServerInfo>,
    total_workers: usize,
    total_cores: usize,
    total_tested: u64,
    total_found: u64,
}

pub(super) fn build_fleet_data(
    workers: &[fleet::WorkerState],
    coordinator_hostname: &str,
    coordinator_metrics: &Option<metrics::HardwareMetrics>,
) -> FleetData {
    // Build compute servers by grouping workers by hostname
    let mut host_map: std::collections::HashMap<String, Vec<&fleet::WorkerState>> =
        std::collections::HashMap::new();
    for w in workers {
        host_map.entry(w.hostname.clone()).or_default().push(w);
    }

    let mut servers = Vec::new();

    // Coordinator is always the first server (role: "service")
    servers.push(ServerInfo {
        hostname: coordinator_hostname.to_string(),
        role: "service".to_string(),
        metrics: coordinator_metrics.clone(),
        worker_count: 0,
        cores: 0,
        worker_ids: Vec::new(),
        total_tested: 0,
        total_found: 0,
        uptime_secs: 0,
    });

    // Each unique worker hostname becomes a "compute" server
    let mut hostnames: Vec<&String> = host_map.keys().collect();
    hostnames.sort();
    for hostname in hostnames {
        let host_workers = &host_map[hostname];
        servers.push(ServerInfo {
            hostname: hostname.clone(),
            role: "compute".to_string(),
            // Use the first worker's metrics (all workers on same host report the same hardware)
            metrics: host_workers.first().and_then(|w| w.metrics.clone()),
            worker_count: host_workers.len(),
            cores: host_workers.iter().map(|w| w.cores).sum(),
            worker_ids: host_workers.iter().map(|w| w.worker_id.clone()).collect(),
            total_tested: host_workers.iter().map(|w| w.tested).sum(),
            total_found: host_workers.iter().map(|w| w.found).sum(),
            uptime_secs: host_workers
                .iter()
                .map(|w| w.uptime_secs)
                .max()
                .unwrap_or(0),
        });
    }

    FleetData {
        total_workers: workers.len(),
        total_cores: workers.iter().map(|w| w.cores).sum(),
        total_tested: workers.iter().map(|w| w.tested).sum(),
        total_found: workers.iter().map(|w| w.found).sum(),
        workers: workers.to_vec(),
        servers,
    }
}

pub(super) async fn handler_api_fleet(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let workers = state.get_workers_from_pg().await;
    let coord_metrics = lock_or_recover(&state.coordinator_metrics).clone();
    Json(build_fleet_data(
        &workers,
        &state.coordinator_hostname,
        &coord_metrics,
    ))
}

pub(super) async fn handler_fleet_worker_stop(
    State(state): State<Arc<AppState>>,
    AxumPath(worker_id): AxumPath<String>,
) -> impl IntoResponse {
    info!(worker_id, "queueing stop command for worker");
    if let Err(e) = state.db.set_worker_command(&worker_id, "stop").await {
        warn!(worker_id, error = %e, "failed to set PG stop command");
    }
    Json(serde_json::json!({"ok": true, "worker_id": worker_id}))
}

fn lock_or_recover<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
