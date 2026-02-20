//! Worker API — register, heartbeat, prime report, deregister.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use std::sync::Arc;

use super::{lock_or_recover, AppState};
use crate::metrics;

#[derive(Deserialize)]
pub(super) struct WorkerRegisterPayload {
    worker_id: String,
    hostname: String,
    cores: usize,
    search_type: String,
    search_params: String,
}

pub(super) async fn handler_worker_register(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WorkerRegisterPayload>,
) -> impl IntoResponse {
    let worker_id = payload.worker_id.clone();
    let search_type_log = payload.search_type.clone();
    eprintln!(
        "Worker registered: {} ({}, {} cores, {})",
        payload.worker_id, payload.hostname, payload.cores, payload.search_type
    );
    if let Err(e) = state
        .db
        .upsert_worker(
            &payload.worker_id,
            &payload.hostname,
            payload.cores as i32,
            &payload.search_type,
            &payload.search_params,
        )
        .await
    {
        eprintln!("Warning: failed to upsert worker to PG: {}", e);
    }
    lock_or_recover(&state.fleet).register(
        payload.worker_id,
        payload.hostname,
        payload.cores,
        payload.search_type,
        payload.search_params,
    );
    let log = crate::db::SystemLogEntry {
        ts: Utc::now(),
        level: "info".to_string(),
        source: "coordinator".to_string(),
        component: "worker_register".to_string(),
        message: format!(
            "Worker registered: {} ({} cores, {})",
            worker_id, payload.cores, search_type_log
        ),
        worker_id: Some(worker_id),
        search_job_id: None,
        search_id: None,
        context: None,
    };
    if let Err(e) = state.db.insert_system_log(&log).await {
        eprintln!("Warning: failed to log worker register: {}", e);
    }
    Json(serde_json::json!({"ok": true}))
}

#[derive(Deserialize)]
pub(super) struct WorkerHeartbeatPayload {
    worker_id: String,
    tested: u64,
    found: u64,
    current: String,
    checkpoint: Option<String>,
    #[serde(default)]
    metrics: Option<metrics::HardwareMetrics>,
}

pub(super) async fn handler_worker_heartbeat(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WorkerHeartbeatPayload>,
) -> impl IntoResponse {
    let metrics_json = payload
        .metrics
        .as_ref()
        .and_then(|m| serde_json::to_value(m).ok());
    let pg_command = state
        .db
        .worker_heartbeat_rpc(
            &payload.worker_id,
            "",
            0,
            "",
            "",
            payload.tested as i64,
            payload.found as i64,
            &payload.current,
            payload.checkpoint.as_deref(),
            metrics_json.as_ref(),
        )
        .await
        .ok()
        .flatten();
    let (known, mem_command) = lock_or_recover(&state.fleet).heartbeat(
        &payload.worker_id,
        payload.tested,
        payload.found,
        payload.current,
        payload.checkpoint,
        payload.metrics,
    );
    let command = pg_command.or(mem_command);
    if known || command.is_some() {
        let mut resp = serde_json::json!({"ok": true});
        if let Some(cmd) = command {
            resp["command"] = serde_json::Value::String(cmd);
        }
        Json(resp)
    } else {
        let log = crate::db::SystemLogEntry {
            ts: Utc::now(),
            level: "warn".to_string(),
            source: "coordinator".to_string(),
            component: "worker_heartbeat".to_string(),
            message: format!("Heartbeat from unknown worker: {}", payload.worker_id),
            worker_id: Some(payload.worker_id),
            search_job_id: None,
            search_id: None,
            context: None,
        };
        if let Err(e) = state.db.insert_system_log(&log).await {
            eprintln!("Warning: failed to log heartbeat warning: {}", e);
        }
        Json(serde_json::json!({"ok": false, "error": "unknown worker, re-register"}))
    }
}

#[derive(Deserialize)]
pub(super) struct WorkerPrimePayload {
    form: String,
    expression: String,
    digits: u64,
    search_params: String,
    #[serde(default = "default_proof_method")]
    proof_method: String,
}

fn default_proof_method() -> String {
    "probabilistic".to_string()
}

pub(super) async fn handler_worker_prime(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WorkerPrimePayload>,
) -> impl IntoResponse {
    eprintln!(
        "Prime received from worker: {} ({} digits, {})",
        payload.expression, payload.digits, payload.proof_method
    );
    match state
        .db
        .insert_prime_ignore(
            &payload.form,
            &payload.expression,
            payload.digits,
            &payload.search_params,
            &payload.proof_method,
        )
        .await
    {
        Ok(_) => {
            let log = crate::db::SystemLogEntry {
                ts: Utc::now(),
                level: "info".to_string(),
                source: "coordinator".to_string(),
                component: "worker_prime".to_string(),
                message: format!(
                    "Prime received: {} ({} digits, {})",
                    payload.expression, payload.digits, payload.proof_method
                ),
                worker_id: None,
                search_job_id: None,
                search_id: None,
                context: Some(serde_json::json!({"form": payload.form, "search_params": payload.search_params})),
            };
            if let Err(e) = state.db.insert_system_log(&log).await {
                eprintln!("Warning: failed to log prime receipt: {}", e);
            }
            Json(serde_json::json!({"ok": true}))
        }
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

#[derive(Deserialize)]
pub(super) struct WorkerDeregisterPayload {
    worker_id: String,
}

pub(super) async fn handler_worker_deregister(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WorkerDeregisterPayload>,
) -> impl IntoResponse {
    eprintln!("Worker deregistered: {}", payload.worker_id);
    if let Err(e) = state.db.delete_worker(&payload.worker_id).await {
        eprintln!("Warning: failed to delete worker from PG: {}", e);
    }
    lock_or_recover(&state.fleet).deregister(&payload.worker_id);
    let log = crate::db::SystemLogEntry {
        ts: Utc::now(),
        level: "info".to_string(),
        source: "coordinator".to_string(),
        component: "worker_deregister".to_string(),
        message: format!("Worker deregistered: {}", payload.worker_id),
        worker_id: Some(payload.worker_id),
        search_job_id: None,
        search_id: None,
        context: None,
    };
    if let Err(e) = state.db.insert_system_log(&log).await {
        eprintln!("Warning: failed to log worker deregister: {}", e);
    }
    Json(serde_json::json!({"ok": true}))
}
