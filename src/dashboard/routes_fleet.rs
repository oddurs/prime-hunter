//! Fleet API — fleet status, deployment management.

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::{lock_or_recover, AppState};
use crate::{deploy, fleet, search_manager};

#[derive(Serialize)]
pub(super) struct FleetData { workers: Vec<fleet::WorkerState>, total_workers: usize, total_cores: usize, total_tested: u64, total_found: u64 }

pub(super) fn build_fleet_data(workers: &[fleet::WorkerState]) -> FleetData {
    FleetData { total_workers: workers.len(), total_cores: workers.iter().map(|w| w.cores).sum(), total_tested: workers.iter().map(|w| w.tested).sum(), total_found: workers.iter().map(|w| w.found).sum(), workers: workers.to_vec() }
}

pub(super) async fn handler_api_fleet(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let workers = state.get_workers_from_pg().await;
    Json(build_fleet_data(&workers))
}

#[derive(Deserialize)]
pub(super) struct DeployRequest { hostname: String, ssh_user: String, ssh_key: Option<String>, coordinator_url: String, search_type: String, k: Option<u64>, base: Option<u32>, min_n: Option<u64>, max_n: Option<u64>, start: Option<u64>, end: Option<u64>, min_digits: Option<u64>, max_digits: Option<u64> }

impl DeployRequest {
    fn to_search_params(&self) -> Result<search_manager::SearchParams, String> {
        match self.search_type.as_str() {
            "kbn" => { let k = self.k.ok_or("k is required for kbn")?; let base = self.base.ok_or("base is required for kbn")?; let min_n = self.min_n.ok_or("min_n is required for kbn")?; let max_n = self.max_n.ok_or("max_n is required for kbn")?; Ok(search_manager::SearchParams::Kbn { k, base, min_n, max_n }) }
            "factorial" => { let start = self.start.ok_or("start is required for factorial")?; let end = self.end.ok_or("end is required for factorial")?; Ok(search_manager::SearchParams::Factorial { start, end }) }
            "palindromic" => { let base = self.base.ok_or("base is required for palindromic")?; let min_digits = self.min_digits.ok_or("min_digits is required for palindromic")?; let max_digits = self.max_digits.ok_or("max_digits is required for palindromic")?; Ok(search_manager::SearchParams::Palindromic { base, min_digits, max_digits }) }
            other => Err(format!("Unknown search type: {}", other)),
        }
    }
    fn search_params_summary(&self) -> String {
        match self.search_type.as_str() {
            "kbn" => format!("k={} base={} n=[{},{}]", self.k.unwrap_or(0), self.base.unwrap_or(0), self.min_n.unwrap_or(0), self.max_n.unwrap_or(0)),
            "factorial" => format!("n=[{},{}]", self.start.unwrap_or(0), self.end.unwrap_or(0)),
            "palindromic" => format!("base={} digits=[{},{}]", self.base.unwrap_or(0), self.min_digits.unwrap_or(0), self.max_digits.unwrap_or(0)),
            _ => String::new(),
        }
    }
}

pub(super) async fn handler_fleet_deploy(State(state): State<Arc<AppState>>, Json(req): Json<DeployRequest>) -> impl IntoResponse {
    let params = match req.to_search_params() { Ok(p) => p, Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response() };
    let deployment = lock_or_recover(&state.deployments).deploy(req.hostname.clone(), req.ssh_user.clone(), req.search_type.clone(), req.search_params_summary(), req.coordinator_url.clone(), state.database_url.clone(), req.ssh_key.clone(), Some(params.clone()));
    let id = deployment.id;
    eprintln!("Deploying worker deploy-{} to {}@{} ({})", id, req.ssh_user, req.hostname, req.search_type);
    let deploy_state = Arc::clone(&state);
    let hostname = req.hostname.clone(); let ssh_user = req.ssh_user.clone(); let ssh_key = req.ssh_key.clone();
    let coordinator_url = req.coordinator_url.clone(); let database_url = state.database_url.clone();
    tokio::spawn(async move {
        let result = deploy::ssh_deploy(&hostname, &ssh_user, ssh_key.as_deref(), &coordinator_url, &database_url, id, &params).await;
        match result { Ok(pid) => { eprintln!("Deployment {} running with remote PID {}", id, pid); lock_or_recover(&deploy_state.deployments).mark_running(id, pid); } Err(e) => { eprintln!("Deployment {} failed: {}", id, e); lock_or_recover(&deploy_state.deployments).mark_failed(id, e); } }
    });
    (StatusCode::CREATED, Json(serde_json::json!({"id": deployment.id, "status": "deploying", "worker_id": deployment.worker_id}))).into_response()
}

pub(super) async fn handler_fleet_deploy_stop(State(state): State<Arc<AppState>>, AxumPath(id): AxumPath<u64>) -> impl IntoResponse {
    let deployment = { let mgr = lock_or_recover(&state.deployments); mgr.get(id).cloned() };
    let deployment = match deployment { Some(d) => d, None => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Deployment not found"}))).into_response() };
    if deployment.status != deploy::DeploymentStatus::Running { return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Deployment is not running"}))).into_response(); }
    let pid = match deployment.remote_pid { Some(pid) => pid, None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "No remote PID available"}))).into_response() };
    match deploy::ssh_stop(&deployment.hostname, &deployment.ssh_user, deployment.ssh_key.as_deref(), pid).await {
        Ok(()) => { lock_or_recover(&state.deployments).mark_stopped(id); eprintln!("Deployment {} stopped", id); Json(serde_json::json!({"ok": true, "id": id})).into_response() }
        Err(e) => { eprintln!("Failed to stop deployment {}: {}", id, e); (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))).into_response() }
    }
}

pub(super) async fn handler_fleet_deploy_pause(State(state): State<Arc<AppState>>, AxumPath(id): AxumPath<u64>) -> impl IntoResponse {
    let deployment = { let mgr = lock_or_recover(&state.deployments); mgr.get(id).cloned() };
    let deployment = match deployment { Some(d) => d, None => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Deployment not found"}))).into_response() };
    if deployment.status != deploy::DeploymentStatus::Running { return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Deployment is not running"}))).into_response(); }
    let pid = match deployment.remote_pid { Some(pid) => pid, None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "No remote PID available"}))).into_response() };
    match deploy::ssh_stop(&deployment.hostname, &deployment.ssh_user, deployment.ssh_key.as_deref(), pid).await {
        Ok(()) => { lock_or_recover(&state.deployments).mark_paused(id); eprintln!("Deployment {} paused", id); Json(serde_json::json!({"ok": true, "id": id})).into_response() }
        Err(e) => { eprintln!("Failed to pause deployment {}: {}", id, e); (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))).into_response() }
    }
}

pub(super) async fn handler_fleet_deploy_resume(State(state): State<Arc<AppState>>, AxumPath(id): AxumPath<u64>) -> impl IntoResponse {
    let deployment = { let mgr = lock_or_recover(&state.deployments); mgr.get(id).cloned() };
    let deployment = match deployment { Some(d) => d, None => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Deployment not found"}))).into_response() };
    if deployment.status != deploy::DeploymentStatus::Paused { return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Deployment is not paused"}))).into_response(); }
    let params = match &deployment.search_params_typed { Some(p) => p.clone(), None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "No search params stored for this deployment"}))).into_response() };
    lock_or_recover(&state.deployments).mark_resuming(id);
    eprintln!("Resuming deployment {} on {}@{}", id, deployment.ssh_user, deployment.hostname);
    let deploy_state = Arc::clone(&state);
    let hostname = deployment.hostname.clone(); let ssh_user = deployment.ssh_user.clone(); let ssh_key = deployment.ssh_key.clone();
    let coordinator_url = deployment.coordinator_url.clone(); let database_url = deployment.database_url.clone();
    tokio::spawn(async move {
        let result = deploy::ssh_deploy(&hostname, &ssh_user, ssh_key.as_deref(), &coordinator_url, &database_url, id, &params).await;
        match result { Ok(pid) => { eprintln!("Deployment {} resumed with remote PID {}", id, pid); lock_or_recover(&deploy_state.deployments).mark_running(id, pid); } Err(e) => { eprintln!("Deployment {} resume failed: {}", id, e); lock_or_recover(&deploy_state.deployments).mark_failed(id, e); } }
    });
    Json(serde_json::json!({"ok": true, "id": id, "status": "resuming"})).into_response()
}

pub(super) async fn handler_fleet_deployments(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let deployments = lock_or_recover(&state.deployments).get_all();
    Json(serde_json::json!({ "deployments": deployments }))
}

pub(super) async fn handler_fleet_worker_stop(State(state): State<Arc<AppState>>, AxumPath(worker_id): AxumPath<String>) -> impl IntoResponse {
    eprintln!("Queueing stop command for worker {}", worker_id);
    if let Err(e) = state.db.set_worker_command(&worker_id, "stop").await { eprintln!("Warning: failed to set PG stop command: {}", e); }
    lock_or_recover(&state.fleet).send_command(&worker_id, "stop".to_string());
    Json(serde_json::json!({"ok": true, "worker_id": worker_id}))
}
