//! WebSocket handler — pushes coordination-only data every 2 seconds.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use std::sync::Arc;
use std::time::Duration;

use super::routes_fleet::build_fleet_data;
use super::routes_status::StatusResponse;
use super::{lock_or_recover, AppState};
use crate::checkpoint;

pub(super) async fn handler_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let notif_rx = state.event_bus.subscribe_ws();
    ws.on_upgrade(|socket| ws_loop(socket, state, notif_rx))
}

async fn ws_loop(
    mut socket: WebSocket,
    state: Arc<AppState>,
    mut notif_rx: tokio::sync::broadcast::Receiver<String>,
) {
    if let Some(msg) = build_update(&state).await {
        if socket.send(Message::Text(msg.into())).await.is_err() {
            return;
        }
    }

    let mut interval = tokio::time::interval(Duration::from_secs(2));
    interval.tick().await;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Some(msg) = build_update(&state).await {
                    if socket.send(Message::Text(msg.into())).await.is_err() {
                        break;
                    }
                }
            }
            result = notif_rx.recv() => {
                match result {
                    Ok(msg) => {
                        if socket.send(Message::Text(msg.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

pub(super) async fn build_update(state: &Arc<AppState>) -> Option<String> {
    let cp = checkpoint::load(&state.checkpoint_path);
    let workers = state.get_workers_from_pg().await;
    let coord_metrics = lock_or_recover(&state.coordinator_metrics).clone();
    let fleet_data = build_fleet_data(
        &workers,
        &state.coordinator_hostname,
        &coord_metrics,
    );
    let search_jobs = state.db.get_search_jobs().await.unwrap_or_default();
    let has_running_jobs = search_jobs.iter().any(|j| j.status == "running");
    let status = StatusResponse {
        active: cp.is_some() || has_running_jobs || !workers.is_empty(),
        checkpoint: cp.and_then(|c| serde_json::to_value(&c).ok()),
    };
    {
        let worker_stats: Vec<(String, u64, u64)> = workers
            .iter()
            .map(|w| (w.worker_id.clone(), w.tested, w.found))
            .collect();
        lock_or_recover(&state.searches).sync_worker_stats(&worker_stats);
    }
    let searches = lock_or_recover(&state.searches).get_all();
    let deployments = lock_or_recover(&state.deployments).get_all();
    let coord_metrics = lock_or_recover(&state.coordinator_metrics).clone();
    let recent_notifications = state.event_bus.recent_notifications(20);
    let agent_tasks = state
        .db
        .get_agent_tasks(Some("in_progress"), 100)
        .await
        .unwrap_or_default();
    let agent_budgets = state.db.get_agent_budgets().await.unwrap_or_default();
    let running_agents = lock_or_recover(&state.agents).get_all();
    let projects = state.db.get_projects(None).await.unwrap_or_default();
    let records = state.db.get_records().await.unwrap_or_default();
    serde_json::to_string(&serde_json::json!({
        "type": "update",
        "status": status,
        "fleet": fleet_data,
        "searches": searches,
        "search_jobs": search_jobs,
        "deployments": deployments,
        "coordinator": coord_metrics,
        "notifications": recent_notifications,
        "agent_tasks": agent_tasks,
        "agent_budgets": agent_budgets,
        "running_agents": running_agents,
        "projects": projects,
        "records": records,
    }))
    .ok()
}
