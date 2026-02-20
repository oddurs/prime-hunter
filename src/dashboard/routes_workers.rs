//! Worker API — register, heartbeat, prime report, deregister.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

use super::{lock_or_recover, AppState};
use crate::metrics;

#[derive(Deserialize)]
pub(super) struct WorkerRegisterPayload { worker_id: String, hostname: String, cores: usize, search_type: String, search_params: String }

pub(super) async fn handler_worker_register(State(state): State<Arc<AppState>>, Json(payload): Json<WorkerRegisterPayload>) -> impl IntoResponse {
    eprintln!("Worker registered: {} ({}, {} cores, {})", payload.worker_id, payload.hostname, payload.cores, payload.search_type);
    if let Err(e) = state.db.upsert_worker(&payload.worker_id, &payload.hostname, payload.cores as i32, &payload.search_type, &payload.search_params).await { eprintln!("Warning: failed to upsert worker to PG: {}", e); }
    lock_or_recover(&state.fleet).register(payload.worker_id, payload.hostname, payload.cores, payload.search_type, payload.search_params);
    Json(serde_json::json!({"ok": true}))
}

#[derive(Deserialize)]
pub(super) struct WorkerHeartbeatPayload { worker_id: String, tested: u64, found: u64, current: String, checkpoint: Option<String>, #[serde(default)] metrics: Option<metrics::HardwareMetrics> }

pub(super) async fn handler_worker_heartbeat(State(state): State<Arc<AppState>>, Json(payload): Json<WorkerHeartbeatPayload>) -> impl IntoResponse {
    let metrics_json = payload.metrics.as_ref().and_then(|m| serde_json::to_value(m).ok());
    let pg_command = state.db.worker_heartbeat_rpc(&payload.worker_id, "", 0, "", "", payload.tested as i64, payload.found as i64, &payload.current, payload.checkpoint.as_deref(), metrics_json.as_ref()).await.ok().flatten();
    let (known, mem_command) = lock_or_recover(&state.fleet).heartbeat(&payload.worker_id, payload.tested, payload.found, payload.current, payload.checkpoint, payload.metrics);
    let command = pg_command.or(mem_command);
    if known || command.is_some() {
        let mut resp = serde_json::json!({"ok": true});
        if let Some(cmd) = command { resp["command"] = serde_json::Value::String(cmd); }
        Json(resp)
    } else {
        Json(serde_json::json!({"ok": false, "error": "unknown worker, re-register"}))
    }
}

#[derive(Deserialize)]
pub(super) struct WorkerPrimePayload { form: String, expression: String, digits: u64, search_params: String, #[serde(default = "default_proof_method")] proof_method: String }

fn default_proof_method() -> String { "probabilistic".to_string() }

pub(super) async fn handler_worker_prime(State(state): State<Arc<AppState>>, Json(payload): Json<WorkerPrimePayload>) -> impl IntoResponse {
    eprintln!("Prime received from worker: {} ({} digits, {})", payload.expression, payload.digits, payload.proof_method);
    match state.db.insert_prime_ignore(&payload.form, &payload.expression, payload.digits, &payload.search_params, &payload.proof_method).await {
        Ok(_) => Json(serde_json::json!({"ok": true})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

#[derive(Deserialize)]
pub(super) struct WorkerDeregisterPayload { worker_id: String }

pub(super) async fn handler_worker_deregister(State(state): State<Arc<AppState>>, Json(payload): Json<WorkerDeregisterPayload>) -> impl IntoResponse {
    eprintln!("Worker deregistered: {}", payload.worker_id);
    if let Err(e) = state.db.delete_worker(&payload.worker_id).await { eprintln!("Warning: failed to delete worker from PG: {}", e); }
    lock_or_recover(&state.fleet).deregister(&payload.worker_id);
    Json(serde_json::json!({"ok": true}))
}
