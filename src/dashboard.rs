//! # Dashboard — Web Server and Fleet Coordination Hub
//!
//! Runs an Axum HTTP server that serves the Next.js frontend, provides REST API
//! endpoints for prime data, and coordinates the distributed worker fleet via
//! WebSocket and HTTP heartbeat.
//!
//! ## Architecture
//!
//! ```text
//! Browser ←── static files ──→ ServeDir (frontend/out/)
//!         ←── REST API ──────→ /api/stats, /api/primes, /api/workers, /api/docs
//!         ←── WebSocket ─────→ /ws (2s push: stats + fleet + searches)
//!
//! Workers ──── HTTP POST ────→ /api/register, /api/heartbeat, /api/prime
//! ```
//!
//! ## Key Endpoints
//!
//! - `GET /api/stats` — Prime counts by form, largest prime.
//! - `GET /api/primes?page=N&per_page=N` — Paginated primes with filtering.
//! - `GET /api/workers` — Connected workers with metrics.
//! - `POST /api/register` — Worker registration.
//! - `POST /api/heartbeat` — Worker status update (10s interval).
//! - `POST /api/prime` — Worker prime discovery report.
//! - `WS /ws` — Real-time push (stats, fleet, managed searches).
//! - `POST /api/search_jobs` — Create search job + work blocks (PostgreSQL).
//!
//! ## State Management
//!
//! `AppState` holds the database pool, in-memory fleet, search manager, event
//! bus, and deployment tracker. Shared via `Arc` across all handlers.

use crate::{agent, checkpoint, db, deploy, events, fleet, metrics, project, search_manager, verify};
use anyhow::Result;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::ServeDir;
use tower_http::timeout::TimeoutLayer;

use std::path::Path;
use std::sync::Arc;

/// Lock a mutex, recovering from poisoning. If a previous holder panicked,
/// we still get access to the data — the alternative is crashing the server.
fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct AppState {
    pub db: db::Database,
    pub database_url: String,
    pub checkpoint_path: PathBuf,
    pub fleet: Mutex<fleet::Fleet>,
    pub searches: Mutex<search_manager::SearchManager>,
    pub deployments: Mutex<deploy::DeploymentManager>,
    pub coordinator_metrics: Mutex<Option<metrics::HardwareMetrics>>,
    pub event_bus: events::EventBus,
    pub agents: Mutex<agent::AgentManager>,
}

impl AppState {
    /// Read workers from PostgreSQL, falling back to in-memory fleet.
    async fn get_workers_from_pg(&self) -> Vec<fleet::WorkerState> {
        match self.db.get_all_workers().await {
            Ok(rows) => rows
                .into_iter()
                .map(|r| {
                    let now = chrono::Utc::now();
                    let heartbeat_age = (now - r.last_heartbeat).num_seconds().max(0) as u64;
                    let uptime = (now - r.registered_at).num_seconds().max(0) as u64;
                    fleet::WorkerState {
                        worker_id: r.worker_id,
                        hostname: r.hostname,
                        cores: r.cores as usize,
                        search_type: r.search_type,
                        search_params: r.search_params,
                        tested: r.tested as u64,
                        found: r.found as u64,
                        current: r.current,
                        checkpoint: r.checkpoint,
                        metrics: r.metrics.and_then(|v| serde_json::from_value(v).ok()),
                        uptime_secs: uptime,
                        last_heartbeat_secs_ago: heartbeat_age,
                        last_heartbeat: std::time::Instant::now(),
                        registered_at: std::time::Instant::now(),
                    }
                })
                .collect(),
            Err(e) => {
                eprintln!(
                    "Warning: failed to read workers from PG: {}, using in-memory fleet",
                    e
                );
                lock_or_recover(&self.fleet).get_all()
            }
        }
    }

    /// Create a new AppState with an already-connected database.
    pub fn with_db(
        db: db::Database,
        database_url: &str,
        checkpoint_path: PathBuf,
        port: u16,
    ) -> Arc<Self> {
        Arc::new(AppState {
            db,
            database_url: database_url.to_string(),
            checkpoint_path,
            fleet: Mutex::new(fleet::Fleet::new()),
            searches: Mutex::new(search_manager::SearchManager::new(port, database_url)),
            deployments: Mutex::new(deploy::DeploymentManager::new()),
            coordinator_metrics: Mutex::new(None),
            event_bus: events::EventBus::new(),
            agents: Mutex::new(agent::AgentManager::new()),
        })
    }
}

fn gethostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Build the Axum router with all API routes and middleware layers.
pub fn build_router(state: Arc<AppState>, static_dir: Option<&Path>) -> Router {
    let mut app = Router::new()
        .route("/ws", get(handler_ws))
        .route("/api/status", get(handler_api_status))
        .route("/api/docs", get(handler_api_docs))
        .route("/api/docs/search", get(handler_api_docs_search))
        .route("/api/docs/roadmaps/{slug}", get(handler_api_doc_roadmap))
        .route("/api/docs/agent/{slug}", get(handler_api_doc_agent))
        .route("/api/docs/{slug}", get(handler_api_doc))
        .route("/api/export", get(handler_api_export))
        .route("/api/fleet", get(handler_api_fleet))
        .route("/api/fleet/deploy", post(handler_fleet_deploy))
        .route(
            "/api/fleet/deploy/{id}",
            axum::routing::delete(handler_fleet_deploy_stop),
        )
        .route(
            "/api/fleet/deploy/{id}/pause",
            post(handler_fleet_deploy_pause),
        )
        .route(
            "/api/fleet/deploy/{id}/resume",
            post(handler_fleet_deploy_resume),
        )
        .route("/api/fleet/deployments", get(handler_fleet_deployments))
        .route(
            "/api/searches",
            get(handler_api_searches_list).post(handler_api_searches_create),
        )
        .route(
            "/api/searches/{id}",
            get(handler_api_searches_get).delete(handler_api_searches_stop),
        )
        .route("/api/searches/{id}/pause", post(handler_api_searches_pause))
        .route(
            "/api/searches/{id}/resume",
            post(handler_api_searches_resume),
        )
        .route("/api/worker/register", post(handler_worker_register))
        .route("/api/worker/heartbeat", post(handler_worker_heartbeat))
        .route("/api/worker/prime", post(handler_worker_prime))
        .route("/api/worker/deregister", post(handler_worker_deregister))
        .route(
            "/api/fleet/workers/{worker_id}/stop",
            post(handler_fleet_worker_stop),
        )
        .route(
            "/api/search_jobs",
            get(handler_api_search_jobs_list).post(handler_api_search_jobs_create),
        )
        .route("/api/search_jobs/{id}", get(handler_api_search_job_get))
        .route(
            "/api/search_jobs/{id}/cancel",
            post(handler_api_search_job_cancel),
        )
        .route("/api/notifications", get(handler_api_notifications))
        .route("/api/events", get(handler_api_events))
        .route(
            "/api/agents/tasks",
            get(handler_api_agent_tasks).post(handler_api_agent_task_create),
        )
        .route("/api/agents/tasks/{id}", get(handler_api_agent_task_get))
        .route(
            "/api/agents/tasks/{id}/cancel",
            post(handler_api_agent_task_cancel),
        )
        .route("/api/agents/events", get(handler_api_agent_events))
        .route(
            "/api/agents/templates",
            get(handler_api_agent_templates),
        )
        .route(
            "/api/agents/templates/{name}/expand",
            post(handler_api_agent_template_expand),
        )
        .route(
            "/api/agents/tasks/{id}/children",
            get(handler_api_agent_task_children),
        )
        .route(
            "/api/agents/budgets",
            get(handler_api_agent_budgets).put(handler_api_agent_budget_update),
        )
        .route("/api/primes/{id}/verify", post(handler_api_prime_verify))
        .route(
            "/api/agents/memory",
            get(handler_api_agent_memory_list).post(handler_api_agent_memory_upsert),
        )
        .route(
            "/api/agents/memory/{key}",
            axum::routing::delete(handler_api_agent_memory_delete),
        )
        // Agent Roles
        .route("/api/agents/roles", get(handler_api_agent_roles))
        .route("/api/agents/roles/{name}", get(handler_api_agent_role_get))
        .route(
            "/api/agents/roles/{name}/templates",
            get(handler_api_agent_role_templates),
        )
        // Project Management
        .route(
            "/api/projects",
            get(handler_api_projects_list).post(handler_api_projects_create),
        )
        .route("/api/projects/import", post(handler_api_projects_import))
        .route("/api/projects/{slug}", get(handler_api_project_get))
        .route(
            "/api/projects/{slug}/activate",
            post(handler_api_project_activate),
        )
        .route(
            "/api/projects/{slug}/pause",
            post(handler_api_project_pause),
        )
        .route(
            "/api/projects/{slug}/cancel",
            post(handler_api_project_cancel),
        )
        .route(
            "/api/projects/{slug}/events",
            get(handler_api_project_events),
        )
        .route(
            "/api/projects/{slug}/cost",
            get(handler_api_project_cost),
        )
        // Records
        .route("/api/records", get(handler_api_records))
        .route("/api/records/refresh", post(handler_api_records_refresh));

    if let Some(dir) = static_dir {
        app = app.fallback_service(ServeDir::new(dir).append_index_html_on_directories(true));
    } else {
        app = app.route("/", get(handler_index));
    }

    app.layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any),
    )
    .layer(CatchPanicLayer::new())
    .layer(RequestBodyLimitLayer::new(1024 * 1024)) // 1MB
    .layer(TimeoutLayer::with_status_code(
        StatusCode::REQUEST_TIMEOUT,
        Duration::from_secs(30),
    ))
    .with_state(state)
}

pub async fn run(
    port: u16,
    database_url: &str,
    checkpoint_path: &Path,
    static_dir: Option<&Path>,
) -> Result<()> {
    let database = db::Database::connect(database_url).await?;
    let (ws_tx, _) = tokio::sync::broadcast::channel::<String>(256);
    let state = AppState::with_db(database, database_url, checkpoint_path.to_path_buf(), port);
    state.event_bus.set_ws_sender(ws_tx.clone());

    let app = build_router(state.clone(), static_dir);

    // Background task: prune stale workers, reclaim stale blocks, poll searches, collect metrics
    let prune_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut sys = sysinfo::System::new();
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            // Prune in-memory fleet (backward compat for HTTP-only workers)
            lock_or_recover(&prune_state.fleet).prune_stale(60);
            // Prune stale workers from PG
            if let Err(e) = prune_state.db.prune_stale_workers(120).await {
                eprintln!("Warning: failed to prune stale PG workers: {}", e);
            }
            // Rotate expired budget periods
            match prune_state.db.rotate_agent_budget_periods().await {
                Ok(n) if n > 0 => eprintln!("Rotated {} budget periods", n),
                Err(e) => eprintln!("Warning: failed to rotate budget periods: {}", e),
                _ => {}
            }
            // Reclaim stale work blocks
            match prune_state.db.reclaim_stale_blocks(120).await {
                Ok(n) if n > 0 => eprintln!("Reclaimed {} stale work blocks", n),
                Err(e) => eprintln!("Warning: failed to reclaim stale blocks: {}", e),
                _ => {}
            }
            {
                let fleet_workers = prune_state.get_workers_from_pg().await;
                let worker_stats: Vec<(String, u64, u64)> = fleet_workers
                    .iter()
                    .map(|w| (w.worker_id.clone(), w.tested, w.found))
                    .collect();
                let mut mgr = lock_or_recover(&prune_state.searches);
                mgr.sync_worker_stats(&worker_stats);
                mgr.poll_completed();
            }
            // Orchestrate active projects (advance phases, check budgets)
            if let Err(e) = project::orchestrate_tick(&prune_state.db).await {
                eprintln!("Warning: project orchestration tick failed: {}", e);
            }
            // Flush pending prime events (safety net for squash window)
            prune_state.event_bus.flush();
            sys.refresh_cpu_all();
            sys.refresh_memory();
            let hw = metrics::collect(&sys);
            *lock_or_recover(&prune_state.coordinator_metrics) = Some(hw);
        }
    });

    // Background task: auto-verify newly discovered primes (60s interval)
    let verify_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.tick().await; // skip immediate first tick
        loop {
            interval.tick().await;
            let primes = match verify_state.db.get_unverified_primes(10).await {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Auto-verify: failed to fetch unverified primes: {}", e);
                    continue;
                }
            };
            if primes.is_empty() {
                continue;
            }
            eprintln!("Auto-verify: checking {} primes", primes.len());
            for prime in &primes {
                let prime_clone = prime.clone();
                let result =
                    tokio::task::spawn_blocking(move || verify::verify_prime(&prime_clone)).await;
                match result {
                    Ok(verify::VerifyResult::Verified { method, tier }) => {
                        eprintln!(
                            "  Auto-verified #{}: {} ({})",
                            prime.id, prime.expression, method
                        );
                        if let Err(e) = verify_state
                            .db
                            .mark_verified(prime.id, &method, tier as i16)
                            .await
                        {
                            eprintln!("  Failed to mark #{} verified: {}", prime.id, e);
                        }
                    }
                    Ok(verify::VerifyResult::Failed { reason }) => {
                        eprintln!("  Auto-verify #{} FAILED: {}", prime.id, reason);
                        if let Err(e) = verify_state
                            .db
                            .mark_verification_failed(prime.id, &reason)
                            .await
                        {
                            eprintln!("  Failed to mark #{} failed: {}", prime.id, e);
                        }
                    }
                    Ok(verify::VerifyResult::Skipped { reason }) => {
                        eprintln!("  Auto-verify #{} skipped: {}", prime.id, reason);
                    }
                    Err(e) => {
                        eprintln!("  Auto-verify #{} panicked: {}", prime.id, e);
                    }
                }
            }
        }
    });

    // Background task: agent execution engine (10s interval)
    let agent_state = Arc::clone(&state);
    tokio::spawn(async move {
        let agent_name = format!("coordinator@{}", gethostname());
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.tick().await; // skip immediate first tick
        loop {
            interval.tick().await;

            // 1. Poll completed agents
            let completed = lock_or_recover(&agent_state.agents).poll_completed();
            for c in completed {
                let status_str = match &c.status {
                    agent::AgentStatus::Completed => "completed",
                    agent::AgentStatus::Failed { .. } => "failed",
                    agent::AgentStatus::TimedOut => "failed",
                    agent::AgentStatus::Cancelled => "cancelled",
                    agent::AgentStatus::Running => "in_progress",
                };
                let reason = match &c.status {
                    agent::AgentStatus::Failed { reason } => Some(reason.clone()),
                    agent::AgentStatus::TimedOut => Some("Timed out".to_string()),
                    _ => None,
                };
                let (result_json, tokens, cost) = match c.result {
                    Some(ref r) => (
                        Some(serde_json::json!({
                            "text": r.result_text,
                        })),
                        r.tokens_used,
                        r.cost_usd,
                    ),
                    None => (reason.as_ref().map(|r| serde_json::json!({"error": r})), 0, 0.0),
                };
                if let Err(e) = agent_state
                    .db
                    .complete_agent_task(c.task_id, status_str, result_json.as_ref(), tokens, cost)
                    .await
                {
                    eprintln!("Agent: failed to complete task {}: {}", c.task_id, e);
                }
                let summary = match &c.status {
                    agent::AgentStatus::Completed => "Task completed".to_string(),
                    agent::AgentStatus::Failed { reason } => format!("Task failed: {}", reason),
                    agent::AgentStatus::TimedOut => "Task timed out".to_string(),
                    agent::AgentStatus::Cancelled => "Task cancelled".to_string(),
                    _ => "Task finished".to_string(),
                };
                let _ = agent_state
                    .db
                    .insert_agent_event(Some(c.task_id), status_str, Some("system"), &summary, None)
                    .await;
                // Update budget spending
                if tokens > 0 || cost > 0.0 {
                    let _ = agent_state
                        .db
                        .update_agent_budget_spending(tokens, cost)
                        .await;
                }
                eprintln!(
                    "Agent task {} finished: {} (tokens={}, cost=${:.4})",
                    c.task_id, status_str, tokens, cost
                );

                // Parent auto-completion: if this child's parent exists, check if all siblings done
                if let Ok(Some(completed_task)) = agent_state.db.get_agent_task(c.task_id).await {
                    if let Some(parent_id) = completed_task.parent_task_id {
                        // If this child failed and parent has on_child_failure='fail', cancel remaining
                        if status_str == "failed" {
                            if let Ok(Some(parent)) = agent_state.db.get_agent_task(parent_id).await {
                                if parent.on_child_failure == "fail" {
                                    let cancelled = agent_state.db.cancel_pending_siblings(parent_id).await.unwrap_or(0);
                                    if cancelled > 0 {
                                        eprintln!("Agent: cancelled {} pending siblings of parent {}", cancelled, parent_id);
                                    }
                                }
                            }
                        }
                        // Try to auto-complete the parent
                        if let Ok(Some(parent)) = agent_state.db.try_complete_parent(parent_id).await {
                            let event_type = if parent.status == "failed" { "parent_failed" } else { "parent_completed" };
                            let _ = agent_state.db.insert_agent_event(
                                Some(parent_id), event_type, None,
                                &format!("Parent task '{}' auto-{}", parent.title, parent.status),
                                None,
                            ).await;
                            eprintln!("Agent: parent task {} auto-{}", parent_id, parent.status);
                        }
                    }
                }
            }

            // 2. Global budget enforcement: if any period is over budget, kill ALL running agents
            let budget_ok = agent_state.db.check_agent_budget().await.unwrap_or(true);
            if !budget_ok {
                let killed = lock_or_recover(&agent_state.agents).kill_all();
                for task_id in &killed {
                    let _ = agent_state
                        .db
                        .complete_agent_task(
                            *task_id,
                            "failed",
                            Some(&serde_json::json!({"error": "Global budget exceeded"})),
                            0,
                            0.0,
                        )
                        .await;
                    let _ = agent_state
                        .db
                        .insert_agent_event(
                            Some(*task_id),
                            "budget_exceeded",
                            Some("system"),
                            "Killed: global budget exceeded",
                            None,
                        )
                        .await;
                }
                if !killed.is_empty() {
                    eprintln!(
                        "Agent: global budget exceeded, killed {} agents: {:?}",
                        killed.len(),
                        killed
                    );
                }
                continue;
            }

            // 3. Check capacity
            let active = lock_or_recover(&agent_state.agents).active_count();
            if active >= agent::MAX_AGENTS {
                continue;
            }

            // 4. Claim a pending task
            let task = match agent_state
                .db
                .claim_pending_agent_task(&agent_name)
                .await
            {
                Ok(Some(t)) => t,
                Ok(None) => continue,
                Err(e) => {
                    eprintln!("Agent: failed to claim task: {}", e);
                    continue;
                }
            };

            eprintln!(
                "Agent: claimed task {} — \"{}\" (priority={}, model={:?})",
                task.id, task.title, task.priority, task.agent_model
            );
            let _ = agent_state
                .db
                .insert_agent_event(
                    Some(task.id),
                    "claimed",
                    Some(&agent_name),
                    &format!("Task claimed by {}", agent_name),
                    None,
                )
                .await;

            // 5. Fetch role (if assigned) and assemble context prompts
            let role = if let Some(ref rn) = task.role_name {
                agent_state.db.get_role_by_name(rn).await.ok().flatten()
            } else {
                None
            };
            let context_prompts = agent::assemble_context(&task, &agent_state.db, role.as_ref()).await;

            // 6. Spawn the agent (scope the lock so it drops before any await)
            let db_clone = agent_state.db.clone();
            let spawn_result = {
                lock_or_recover(&agent_state.agents).spawn_agent(
                    &task,
                    db_clone,
                    task.max_cost_usd,
                    context_prompts,
                )
            };
            match spawn_result {
                Ok(_info) => {}
                Err(e) => {
                    eprintln!("Agent: failed to spawn for task {}: {}", task.id, e);
                    let _ = agent_state
                        .db
                        .complete_agent_task(
                            task.id,
                            "failed",
                            Some(&serde_json::json!({"error": e})),
                            0,
                            0.0,
                        )
                        .await;
                    let _ = agent_state
                        .db
                        .insert_agent_event(
                            Some(task.id),
                            "failed",
                            Some("system"),
                            &format!("Failed to spawn: {}", e),
                            None,
                        )
                        .await;
                }
            }
        }
    });

    // Background task: refresh world records from t5k.org (24h interval, 10s initial delay)
    let records_state = Arc::clone(&state);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        eprintln!("Records: initial refresh from t5k.org...");
        match project::refresh_all_records(&records_state.db).await {
            Ok(n) => eprintln!("Records: refreshed {} forms", n),
            Err(e) => eprintln!("Warning: records refresh failed: {}", e),
        }
        let mut interval = tokio::time::interval(Duration::from_secs(24 * 3600));
        interval.tick().await; // consume immediate tick
        loop {
            interval.tick().await;
            eprintln!("Records: 24h refresh from t5k.org...");
            match project::refresh_all_records(&records_state.db).await {
                Ok(n) => eprintln!("Records: refreshed {} forms", n),
                Err(e) => eprintln!("Warning: records refresh failed: {}", e),
            }
        }
    });

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    eprintln!("Dashboard running at http://localhost:{}", port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    eprintln!("Dashboard shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => eprintln!("\nReceived SIGINT, shutting down..."),
            _ = sigterm.recv() => eprintln!("\nReceived SIGTERM, shutting down..."),
        }
    }
    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
        eprintln!("\nReceived SIGINT, shutting down...");
    }
}

async fn handler_index() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("dashboard.html"),
    )
}

async fn handler_ws(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Each WS client gets its own broadcast receiver
    let notif_rx = state.event_bus.subscribe_ws();
    ws.on_upgrade(|socket| ws_loop(socket, state, notif_rx))
}

/// WebSocket loop — pushes coordination-only data (status, fleet, searches, deployments)
/// and event bus notifications.
async fn ws_loop(
    mut socket: WebSocket,
    state: Arc<AppState>,
    mut notif_rx: tokio::sync::broadcast::Receiver<String>,
) {
    // Send initial data immediately
    if let Some(msg) = build_update(&state).await {
        if socket.send(Message::Text(msg.into())).await.is_err() {
            return;
        }
    }

    let mut interval = tokio::time::interval(Duration::from_secs(2));
    interval.tick().await; // consume the immediate first tick

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
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Slow consumer — skip missed notifications
                    }
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

async fn build_update(state: &Arc<AppState>) -> Option<String> {
    let cp = checkpoint::load(&state.checkpoint_path);
    let status = StatusResponse {
        active: cp.is_some(),
        checkpoint: cp.and_then(|c| serde_json::to_value(&c).ok()),
    };
    let workers = state.get_workers_from_pg().await;
    let fleet_data = build_fleet_data(&workers);
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
    // Include search jobs from PG
    let search_jobs = state.db.get_search_jobs().await.unwrap_or_default();
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

// --- REST endpoints ---

#[derive(Deserialize)]
struct ExportQuery {
    format: Option<String>,
    form: Option<String>,
    search: Option<String>,
    min_digits: Option<i64>,
    max_digits: Option<i64>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
}

async fn handler_api_export(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ExportQuery>,
) -> impl IntoResponse {
    let filter = db::PrimeFilter {
        form: params.form,
        search: params.search,
        min_digits: params.min_digits,
        max_digits: params.max_digits,
        sort_by: params.sort_by,
        sort_dir: params.sort_dir,
    };
    let format = params.format.unwrap_or_else(|| "csv".to_string());
    let primes = match state.db.get_primes_filtered(100_000, 0, &filter).await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    if format == "json" {
        let body = serde_json::to_string_pretty(&primes).unwrap_or_default();
        (
            [
                (header::CONTENT_TYPE, "application/json"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"primes.json\"",
                ),
            ],
            body,
        )
            .into_response()
    } else {
        let mut csv = String::from("id,form,expression,digits,found_at,proof_method\n");
        for p in &primes {
            csv.push_str(&format!(
                "{},\"{}\",\"{}\",{},{},\"{}\"\n",
                p.id,
                p.form.replace('"', "\"\""),
                p.expression.replace('"', "\"\""),
                p.digits,
                p.found_at.to_rfc3339(),
                p.proof_method.replace('"', "\"\""),
            ));
        }
        (
            [
                (header::CONTENT_TYPE, "text/csv"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"primes.csv\"",
                ),
            ],
            csv,
        )
            .into_response()
    }
}

#[derive(Serialize)]
struct StatusResponse {
    active: bool,
    checkpoint: Option<serde_json::Value>,
}

async fn handler_api_status(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
    let cp = checkpoint::load(&state.checkpoint_path);
    match cp {
        Some(c) => Json(StatusResponse {
            active: true,
            checkpoint: serde_json::to_value(&c).ok(),
        }),
        None => Json(StatusResponse {
            active: false,
            checkpoint: None,
        }),
    }
}

// --- Docs API ---

#[derive(Deserialize)]
struct DocSearchQuery {
    q: String,
}

#[derive(Serialize)]
struct SearchSnippet {
    text: String,
    line: usize,
}

#[derive(Serialize)]
struct DocSearchResult {
    slug: String,
    title: String,
    snippets: Vec<SearchSnippet>,
    category: Option<String>,
}

async fn handler_api_docs_search(Query(params): Query<DocSearchQuery>) -> impl IntoResponse {
    let query = params.q.to_lowercase();
    if query.is_empty() {
        return Json(serde_json::json!({ "results": [] })).into_response();
    }
    let docs_dir = std::path::Path::new("docs");
    if !docs_dir.exists() {
        return Json(serde_json::json!({ "results": [] })).into_response();
    }
    let mut results = Vec::new();

    let mut check_file = |path: &std::path::Path, slug: String, category: Option<String>| {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let title = extract_title(&content, &slug);
        let mut snippets = search_file_for_snippets(&content, &query);
        if title.to_lowercase().contains(&query) || !snippets.is_empty() {
            if snippets.is_empty() {
                snippets.push(SearchSnippet {
                    text: title.clone(),
                    line: 1,
                });
            }
            results.push(DocSearchResult {
                slug,
                title,
                snippets,
                category,
            });
        }
    };

    if let Ok(entries) = std::fs::read_dir(docs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                let slug = path.file_stem().unwrap().to_string_lossy().to_string();
                check_file(&path, slug, None);
            }
        }
    }

    let roadmaps_dir = docs_dir.join("roadmaps");
    if let Ok(entries) = std::fs::read_dir(&roadmaps_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                let slug = format!("roadmaps/{}", stem);
                check_file(&path, slug, Some("roadmaps".into()));
            }
        }
    }

    let root_roadmap = std::path::Path::new("ROADMAP.md");
    if root_roadmap.exists() {
        check_file(
            root_roadmap,
            "roadmaps/index".into(),
            Some("roadmaps".into()),
        );
    }

    // Search agent files (CLAUDE.md)
    for &(name, path_str, _label) in AGENT_FILES {
        let path = std::path::Path::new(path_str);
        if path.exists() {
            check_file(path, format!("agent/{}", name), Some("agent".into()));
        }
    }

    results.sort_by(|a, b| a.slug.cmp(&b.slug));
    Json(serde_json::json!({ "results": results })).into_response()
}

#[derive(Serialize)]
struct DocEntry {
    slug: String,
    title: String,
    form: Option<String>,
    category: Option<String>,
}

fn doc_form(slug: &str) -> Option<String> {
    match slug {
        "factorial" => Some("Factorial".into()),
        "palindromic" => Some("Palindromic".into()),
        "kbn" => Some("Kbn".into()),
        _ => None,
    }
}

fn extract_title(content: &str, fallback: &str) -> String {
    content
        .lines()
        .next()
        .unwrap_or(fallback)
        .trim_start_matches('#')
        .trim()
        .to_string()
}

fn search_file_for_snippets(content: &str, query: &str) -> Vec<SearchSnippet> {
    let mut snippets = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if line.to_lowercase().contains(query) {
            let text = if line.len() > 120 {
                let lower = line.to_lowercase();
                let pos = lower.find(query).unwrap_or(0);
                let start = pos.saturating_sub(40);
                let end = (pos + query.len() + 40).min(line.len());
                let mut snippet = String::new();
                if start > 0 {
                    snippet.push_str("...");
                }
                snippet.push_str(&line[start..end]);
                if end < line.len() {
                    snippet.push_str("...");
                }
                snippet
            } else {
                line.to_string()
            };
            snippets.push(SearchSnippet { text, line: i + 1 });
            if snippets.len() >= 3 {
                break;
            }
        }
    }
    snippets
}

async fn handler_api_doc_roadmap(AxumPath(slug): AxumPath<String>) -> impl IntoResponse {
    if slug.contains('/') || slug.contains('\\') || slug.contains("..") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid slug"})),
        )
            .into_response();
    }
    let path = if slug == "index" {
        std::path::PathBuf::from("ROADMAP.md")
    } else {
        std::path::Path::new("docs/roadmaps").join(format!("{}.md", slug))
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let title = extract_title(&content, &slug);
            Json(serde_json::json!({
                "slug": format!("roadmaps/{}", slug),
                "title": title,
                "content": content,
                "category": "roadmaps",
            }))
            .into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Doc not found"})),
        )
            .into_response(),
    }
}

async fn handler_api_docs() -> impl IntoResponse {
    let docs_dir = std::path::Path::new("docs");
    if !docs_dir.exists() {
        return Json(serde_json::json!({ "docs": [] })).into_response();
    }
    let mut docs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(docs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                let slug = path.file_stem().unwrap().to_string_lossy().to_string();
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                let title = extract_title(&content, &slug);
                let form = doc_form(&slug);
                docs.push(DocEntry {
                    slug,
                    title,
                    form,
                    category: None,
                });
            }
        }
    }
    let roadmaps_dir = docs_dir.join("roadmaps");
    if let Ok(entries) = std::fs::read_dir(&roadmaps_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                let title = extract_title(&content, &stem);
                docs.push(DocEntry {
                    slug: format!("roadmaps/{}", stem),
                    title,
                    form: None,
                    category: Some("roadmaps".into()),
                });
            }
        }
    }
    let root_roadmap = std::path::Path::new("ROADMAP.md");
    if root_roadmap.exists() {
        let content = std::fs::read_to_string(root_roadmap).unwrap_or_default();
        let title = extract_title(&content, "Roadmap");
        docs.push(DocEntry {
            slug: "roadmaps/index".into(),
            title,
            form: None,
            category: Some("roadmaps".into()),
        });
    }
    // Agent files (CLAUDE.md from various directories)
    for &(name, path_str, label) in AGENT_FILES {
        let path = std::path::Path::new(path_str);
        if path.exists() {
            let content = std::fs::read_to_string(path).unwrap_or_default();
            let title = extract_title(&content, &format!("CLAUDE.md ({})", label));
            docs.push(DocEntry {
                slug: format!("agent/{}", name),
                title,
                form: None,
                category: Some("agent".into()),
            });
        }
    }

    docs.sort_by(|a, b| a.slug.cmp(&b.slug));
    Json(serde_json::json!({ "docs": docs })).into_response()
}

/// Known CLAUDE.md agent configuration files.
const AGENT_FILES: &[(&str, &str, &str)] = &[
    ("root", "CLAUDE.md", "Project"),
    ("engine", "src/CLAUDE.md", "Engine"),
    ("frontend", "frontend/CLAUDE.md", "Frontend"),
    ("docs", "docs/CLAUDE.md", "Research"),
    ("deploy", "deploy/CLAUDE.md", "Deployment"),
];

async fn handler_api_doc_agent(AxumPath(slug): AxumPath<String>) -> impl IntoResponse {
    if slug.contains('/') || slug.contains('\\') || slug.contains("..") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid slug"})),
        )
            .into_response();
    }
    for &(name, path_str, _label) in AGENT_FILES {
        if name == slug {
            let path = std::path::Path::new(path_str);
            return match std::fs::read_to_string(path) {
                Ok(content) => {
                    let title = extract_title(&content, &format!("CLAUDE.md ({})", name));
                    Json(serde_json::json!({
                        "slug": format!("agent/{}", name),
                        "title": title,
                        "content": content,
                        "category": "agent",
                    }))
                    .into_response()
                }
                Err(_) => (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "Agent file not found"})),
                )
                    .into_response(),
            };
        }
    }
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error": "Unknown agent file"})),
    )
        .into_response()
}

async fn handler_api_doc(AxumPath(slug): AxumPath<String>) -> impl IntoResponse {
    if slug.contains('/') || slug.contains('\\') || slug.contains("..") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid slug"})),
        )
            .into_response();
    }
    let path = std::path::Path::new("docs").join(format!("{}.md", slug));
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let title = extract_title(&content, &slug);
            Json(serde_json::json!({
                "slug": slug,
                "title": title,
                "content": content,
            }))
            .into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Doc not found"})),
        )
            .into_response(),
    }
}

// --- Search management API ---

async fn handler_api_searches_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let searches = lock_or_recover(&state.searches).get_all();
    Json(serde_json::json!({ "searches": searches }))
}

async fn handler_api_searches_create(
    State(state): State<Arc<AppState>>,
    Json(params): Json<search_manager::SearchParams>,
) -> impl IntoResponse {
    let mut mgr = lock_or_recover(&state.searches);
    match mgr.start_search(params) {
        Ok(info) => (StatusCode::CREATED, Json(serde_json::json!(info))).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

async fn handler_api_searches_get(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<u64>,
) -> impl IntoResponse {
    let mgr = lock_or_recover(&state.searches);
    match mgr.get(id) {
        Some(info) => Json(serde_json::json!(info)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Search not found"})),
        )
            .into_response(),
    }
}

async fn handler_api_searches_stop(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<u64>,
) -> impl IntoResponse {
    let mut mgr = lock_or_recover(&state.searches);
    match mgr.stop_search(id) {
        Ok(info) => Json(serde_json::json!(info)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

async fn handler_api_searches_pause(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<u64>,
) -> impl IntoResponse {
    let mut mgr = lock_or_recover(&state.searches);
    match mgr.pause_search(id) {
        Ok(info) => Json(serde_json::json!(info)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

async fn handler_api_searches_resume(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<u64>,
) -> impl IntoResponse {
    let mut mgr = lock_or_recover(&state.searches);
    match mgr.resume_search(id) {
        Ok(info) => Json(serde_json::json!(info)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

// --- Fleet API ---

#[derive(Serialize)]
struct FleetData {
    workers: Vec<fleet::WorkerState>,
    total_workers: usize,
    total_cores: usize,
    total_tested: u64,
    total_found: u64,
}

fn build_fleet_data(workers: &[fleet::WorkerState]) -> FleetData {
    FleetData {
        total_workers: workers.len(),
        total_cores: workers.iter().map(|w| w.cores).sum(),
        total_tested: workers.iter().map(|w| w.tested).sum(),
        total_found: workers.iter().map(|w| w.found).sum(),
        workers: workers.to_vec(),
    }
}

async fn handler_api_fleet(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let workers = state.get_workers_from_pg().await;
    Json(build_fleet_data(&workers))
}

// --- Deployment API ---

#[derive(Deserialize)]
struct DeployRequest {
    hostname: String,
    ssh_user: String,
    ssh_key: Option<String>,
    coordinator_url: String,
    search_type: String,
    k: Option<u64>,
    base: Option<u32>,
    min_n: Option<u64>,
    max_n: Option<u64>,
    start: Option<u64>,
    end: Option<u64>,
    min_digits: Option<u64>,
    max_digits: Option<u64>,
}

impl DeployRequest {
    fn to_search_params(&self) -> Result<search_manager::SearchParams, String> {
        match self.search_type.as_str() {
            "kbn" => {
                let k = self.k.ok_or("k is required for kbn")?;
                let base = self.base.ok_or("base is required for kbn")?;
                let min_n = self.min_n.ok_or("min_n is required for kbn")?;
                let max_n = self.max_n.ok_or("max_n is required for kbn")?;
                Ok(search_manager::SearchParams::Kbn {
                    k,
                    base,
                    min_n,
                    max_n,
                })
            }
            "factorial" => {
                let start = self.start.ok_or("start is required for factorial")?;
                let end = self.end.ok_or("end is required for factorial")?;
                Ok(search_manager::SearchParams::Factorial { start, end })
            }
            "palindromic" => {
                let base = self.base.ok_or("base is required for palindromic")?;
                let min_digits = self
                    .min_digits
                    .ok_or("min_digits is required for palindromic")?;
                let max_digits = self
                    .max_digits
                    .ok_or("max_digits is required for palindromic")?;
                Ok(search_manager::SearchParams::Palindromic {
                    base,
                    min_digits,
                    max_digits,
                })
            }
            other => Err(format!("Unknown search type: {}", other)),
        }
    }

    fn search_params_summary(&self) -> String {
        match self.search_type.as_str() {
            "kbn" => format!(
                "k={} base={} n=[{},{}]",
                self.k.unwrap_or(0),
                self.base.unwrap_or(0),
                self.min_n.unwrap_or(0),
                self.max_n.unwrap_or(0),
            ),
            "factorial" => format!("n=[{},{}]", self.start.unwrap_or(0), self.end.unwrap_or(0),),
            "palindromic" => format!(
                "base={} digits=[{},{}]",
                self.base.unwrap_or(0),
                self.min_digits.unwrap_or(0),
                self.max_digits.unwrap_or(0),
            ),
            _ => String::new(),
        }
    }
}

async fn handler_fleet_deploy(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DeployRequest>,
) -> impl IntoResponse {
    let params = match req.to_search_params() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e})),
            )
                .into_response();
        }
    };

    let deployment = lock_or_recover(&state.deployments).deploy(
        req.hostname.clone(),
        req.ssh_user.clone(),
        req.search_type.clone(),
        req.search_params_summary(),
        req.coordinator_url.clone(),
        state.database_url.clone(),
        req.ssh_key.clone(),
        Some(params.clone()),
    );

    let id = deployment.id;
    eprintln!(
        "Deploying worker deploy-{} to {}@{} ({})",
        id, req.ssh_user, req.hostname, req.search_type
    );

    let deploy_state = Arc::clone(&state);
    let hostname = req.hostname.clone();
    let ssh_user = req.ssh_user.clone();
    let ssh_key = req.ssh_key.clone();
    let coordinator_url = req.coordinator_url.clone();
    let database_url = state.database_url.clone();

    tokio::spawn(async move {
        let result = deploy::ssh_deploy(
            &hostname,
            &ssh_user,
            ssh_key.as_deref(),
            &coordinator_url,
            &database_url,
            id,
            &params,
        )
        .await;

        match result {
            Ok(pid) => {
                eprintln!("Deployment {} running with remote PID {}", id, pid);
                lock_or_recover(&deploy_state.deployments).mark_running(id, pid);
            }
            Err(e) => {
                eprintln!("Deployment {} failed: {}", id, e);
                lock_or_recover(&deploy_state.deployments).mark_failed(id, e);
            }
        }
    });

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": deployment.id,
            "status": "deploying",
            "worker_id": deployment.worker_id,
        })),
    )
        .into_response()
}

async fn handler_fleet_deploy_stop(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<u64>,
) -> impl IntoResponse {
    let deployment = {
        let mgr = lock_or_recover(&state.deployments);
        mgr.get(id).cloned()
    };

    let deployment = match deployment {
        Some(d) => d,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Deployment not found"})),
            )
                .into_response();
        }
    };

    if deployment.status != deploy::DeploymentStatus::Running {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Deployment is not running"})),
        )
            .into_response();
    }

    let pid = match deployment.remote_pid {
        Some(pid) => pid,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "No remote PID available"})),
            )
                .into_response();
        }
    };

    match deploy::ssh_stop(
        &deployment.hostname,
        &deployment.ssh_user,
        deployment.ssh_key.as_deref(),
        pid,
    )
    .await
    {
        Ok(()) => {
            lock_or_recover(&state.deployments).mark_stopped(id);
            eprintln!("Deployment {} stopped", id);
            Json(serde_json::json!({"ok": true, "id": id})).into_response()
        }
        Err(e) => {
            eprintln!("Failed to stop deployment {}: {}", id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e})),
            )
                .into_response()
        }
    }
}

async fn handler_fleet_deploy_pause(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<u64>,
) -> impl IntoResponse {
    let deployment = {
        let mgr = lock_or_recover(&state.deployments);
        mgr.get(id).cloned()
    };

    let deployment = match deployment {
        Some(d) => d,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Deployment not found"})),
            )
                .into_response();
        }
    };

    if deployment.status != deploy::DeploymentStatus::Running {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Deployment is not running"})),
        )
            .into_response();
    }

    let pid = match deployment.remote_pid {
        Some(pid) => pid,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "No remote PID available"})),
            )
                .into_response();
        }
    };

    match deploy::ssh_stop(
        &deployment.hostname,
        &deployment.ssh_user,
        deployment.ssh_key.as_deref(),
        pid,
    )
    .await
    {
        Ok(()) => {
            lock_or_recover(&state.deployments).mark_paused(id);
            eprintln!("Deployment {} paused", id);
            Json(serde_json::json!({"ok": true, "id": id})).into_response()
        }
        Err(e) => {
            eprintln!("Failed to pause deployment {}: {}", id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e})),
            )
                .into_response()
        }
    }
}

async fn handler_fleet_deploy_resume(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<u64>,
) -> impl IntoResponse {
    let deployment = {
        let mgr = lock_or_recover(&state.deployments);
        mgr.get(id).cloned()
    };

    let deployment = match deployment {
        Some(d) => d,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Deployment not found"})),
            )
                .into_response();
        }
    };

    if deployment.status != deploy::DeploymentStatus::Paused {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Deployment is not paused"})),
        )
            .into_response();
    }

    let params = match &deployment.search_params_typed {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "No search params stored for this deployment"})),
            )
                .into_response();
        }
    };

    lock_or_recover(&state.deployments).mark_resuming(id);
    eprintln!(
        "Resuming deployment {} on {}@{}",
        id, deployment.ssh_user, deployment.hostname
    );

    let deploy_state = Arc::clone(&state);
    let hostname = deployment.hostname.clone();
    let ssh_user = deployment.ssh_user.clone();
    let ssh_key = deployment.ssh_key.clone();
    let coordinator_url = deployment.coordinator_url.clone();
    let database_url = deployment.database_url.clone();

    tokio::spawn(async move {
        let result = deploy::ssh_deploy(
            &hostname,
            &ssh_user,
            ssh_key.as_deref(),
            &coordinator_url,
            &database_url,
            id,
            &params,
        )
        .await;

        match result {
            Ok(pid) => {
                eprintln!("Deployment {} resumed with remote PID {}", id, pid);
                lock_or_recover(&deploy_state.deployments).mark_running(id, pid);
            }
            Err(e) => {
                eprintln!("Deployment {} resume failed: {}", id, e);
                lock_or_recover(&deploy_state.deployments).mark_failed(id, e);
            }
        }
    });

    Json(serde_json::json!({"ok": true, "id": id, "status": "resuming"})).into_response()
}

async fn handler_fleet_deployments(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let deployments = lock_or_recover(&state.deployments).get_all();
    Json(serde_json::json!({ "deployments": deployments }))
}

async fn handler_fleet_worker_stop(
    State(state): State<Arc<AppState>>,
    AxumPath(worker_id): AxumPath<String>,
) -> impl IntoResponse {
    eprintln!("Queueing stop command for worker {}", worker_id);
    // Set pending_command in PG (for PG-based workers)
    if let Err(e) = state.db.set_worker_command(&worker_id, "stop").await {
        eprintln!("Warning: failed to set PG stop command: {}", e);
    }
    // Also set in-memory command (for HTTP-based workers)
    lock_or_recover(&state.fleet).send_command(&worker_id, "stop".to_string());
    Json(serde_json::json!({"ok": true, "worker_id": worker_id}))
}

// --- Worker API ---

#[derive(Deserialize)]
struct WorkerRegisterPayload {
    worker_id: String,
    hostname: String,
    cores: usize,
    search_type: String,
    search_params: String,
}

async fn handler_worker_register(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WorkerRegisterPayload>,
) -> impl IntoResponse {
    eprintln!(
        "Worker registered: {} ({}, {} cores, {})",
        payload.worker_id, payload.hostname, payload.cores, payload.search_type
    );
    // Write to PG
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
    // Also keep in-memory fleet for backward compat
    lock_or_recover(&state.fleet).register(
        payload.worker_id,
        payload.hostname,
        payload.cores,
        payload.search_type,
        payload.search_params,
    );
    Json(serde_json::json!({"ok": true}))
}

#[derive(Deserialize)]
struct WorkerHeartbeatPayload {
    worker_id: String,
    tested: u64,
    found: u64,
    current: String,
    checkpoint: Option<String>,
    #[serde(default)]
    metrics: Option<metrics::HardwareMetrics>,
}

async fn handler_worker_heartbeat(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WorkerHeartbeatPayload>,
) -> impl IntoResponse {
    // Write heartbeat to PG via the worker_heartbeat() RPC
    let metrics_json = payload
        .metrics
        .as_ref()
        .and_then(|m| serde_json::to_value(m).ok());
    let pg_command = state
        .db
        .worker_heartbeat_rpc(
            &payload.worker_id,
            "", // hostname not in heartbeat payload, RPC will use existing value via UPSERT
            0,  // cores not in heartbeat payload
            "", // search_type not in heartbeat payload
            "", // search_params not in heartbeat payload
            payload.tested as i64,
            payload.found as i64,
            &payload.current,
            payload.checkpoint.as_deref(),
            metrics_json.as_ref(),
        )
        .await
        .ok()
        .flatten();

    // Also update in-memory fleet for backward compat
    let (known, mem_command) = lock_or_recover(&state.fleet).heartbeat(
        &payload.worker_id,
        payload.tested,
        payload.found,
        payload.current,
        payload.checkpoint,
        payload.metrics,
    );

    // Prefer PG command over in-memory command
    let command = pg_command.or(mem_command);

    if known || command.is_some() {
        let mut resp = serde_json::json!({"ok": true});
        if let Some(cmd) = command {
            resp["command"] = serde_json::Value::String(cmd);
        }
        Json(resp)
    } else {
        Json(serde_json::json!({"ok": false, "error": "unknown worker, re-register"}))
    }
}

#[derive(Deserialize)]
struct WorkerPrimePayload {
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

async fn handler_worker_prime(
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
        Ok(_) => Json(serde_json::json!({"ok": true})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

#[derive(Deserialize)]
struct WorkerDeregisterPayload {
    worker_id: String,
}

async fn handler_worker_deregister(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WorkerDeregisterPayload>,
) -> impl IntoResponse {
    eprintln!("Worker deregistered: {}", payload.worker_id);
    if let Err(e) = state.db.delete_worker(&payload.worker_id).await {
        eprintln!("Warning: failed to delete worker from PG: {}", e);
    }
    lock_or_recover(&state.fleet).deregister(&payload.worker_id);
    Json(serde_json::json!({"ok": true}))
}

// --- Search Job API (PG-based block coordination) ---

#[derive(Deserialize)]
struct CreateSearchJobPayload {
    search_type: String,
    params: serde_json::Value,
    range_start: i64,
    range_end: i64,
    #[serde(default = "default_block_size")]
    block_size: i64,
}

fn default_block_size() -> i64 {
    10_000
}

async fn handler_api_search_jobs_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_search_jobs().await {
        Ok(jobs) => Json(serde_json::json!({ "search_jobs": jobs })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn handler_api_search_jobs_create(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateSearchJobPayload>,
) -> impl IntoResponse {
    if payload.range_start >= payload.range_end {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "range_start must be less than range_end"})),
        )
            .into_response();
    }
    if payload.block_size <= 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "block_size must be positive"})),
        )
            .into_response();
    }

    match state
        .db
        .create_search_job(
            &payload.search_type,
            &payload.params,
            payload.range_start,
            payload.range_end,
            payload.block_size,
        )
        .await
    {
        Ok(job_id) => {
            let num_blocks = ((payload.range_end - payload.range_start) + payload.block_size - 1)
                / payload.block_size;
            eprintln!(
                "Created search job {} ({}, range {}..{}, {} blocks)",
                job_id, payload.search_type, payload.range_start, payload.range_end, num_blocks
            );
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "id": job_id,
                    "blocks": num_blocks,
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn handler_api_search_job_get(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> impl IntoResponse {
    let job = match state.db.get_search_job(id).await {
        Ok(Some(j)) => j,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Search job not found"})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let summary = state.db.get_job_block_summary(id).await.ok();
    Json(serde_json::json!({
        "job": job,
        "blocks": summary,
    }))
    .into_response()
}

async fn handler_api_search_job_cancel(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> impl IntoResponse {
    match state
        .db
        .update_search_job_status(id, "cancelled", None)
        .await
    {
        Ok(()) => {
            eprintln!("Search job {} cancelled", id);
            Json(serde_json::json!({"ok": true, "id": id})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// --- Event Bus API ---

async fn handler_api_notifications(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let notifications = state.event_bus.recent_notifications(50);
    Json(serde_json::json!({ "notifications": notifications }))
}

async fn handler_api_events(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let events = state.event_bus.recent_events(200);
    Json(serde_json::json!({ "events": events }))
}

// --- Agent Management API ---

#[derive(Deserialize)]
struct AgentTasksQuery {
    status: Option<String>,
    limit: Option<i64>,
}

async fn handler_api_agent_tasks(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AgentTasksQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100);
    match state
        .db
        .get_agent_tasks(params.status.as_deref(), limit)
        .await
    {
        Ok(tasks) => Json(serde_json::json!({ "tasks": tasks })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct CreateAgentTaskPayload {
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_priority")]
    priority: String,
    agent_model: Option<String>,
    #[serde(default = "default_source")]
    source: String,
    max_cost_usd: Option<f64>,
    #[serde(default = "default_permission_level")]
    permission_level: i32,
    role_name: Option<String>,
}

fn default_permission_level() -> i32 {
    1
}

fn default_priority() -> String {
    "normal".to_string()
}

fn default_source() -> String {
    "manual".to_string()
}

async fn handler_api_agent_task_create(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateAgentTaskPayload>,
) -> impl IntoResponse {
    // If a role is specified, look it up and apply defaults for unset fields
    let mut agent_model = payload.agent_model.clone();
    let mut max_cost_usd = payload.max_cost_usd;
    let mut permission_level = payload.permission_level;

    if let Some(ref role_name) = payload.role_name {
        if let Ok(Some(role)) = state.db.get_role_by_name(role_name).await {
            // Apply role defaults only when the payload uses the default values
            if agent_model.is_none() {
                agent_model = Some(role.default_model.clone());
            }
            if max_cost_usd.is_none() {
                max_cost_usd = role.default_max_cost_usd;
            }
            if permission_level == 1 {
                // 1 is the default — override with role default
                permission_level = role.default_permission_level;
            }
        }
    }

    match state
        .db
        .create_agent_task(
            &payload.title,
            &payload.description,
            &payload.priority,
            agent_model.as_deref(),
            &payload.source,
            max_cost_usd,
            permission_level,
            payload.role_name.as_deref(),
        )
        .await
    {
        Ok(task) => {
            // Insert a "created" event
            let _ = state
                .db
                .insert_agent_event(
                    Some(task.id),
                    "created",
                    None,
                    &format!("Task created: {}", task.title),
                    None,
                )
                .await;
            (StatusCode::CREATED, Json(serde_json::json!(task))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn handler_api_agent_task_get(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> impl IntoResponse {
    match state.db.get_agent_task(id).await {
        Ok(Some(task)) => Json(serde_json::json!(task)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Task not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn handler_api_agent_task_cancel(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> impl IntoResponse {
    match state.db.cancel_agent_task(id).await {
        Ok(()) => {
            lock_or_recover(&state.agents).cancel_agent(id);
            let _ = state
                .db
                .insert_agent_event(Some(id), "cancelled", None, "Task cancelled", None)
                .await;
            Json(serde_json::json!({"ok": true, "id": id})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct AgentEventsQuery {
    task_id: Option<i64>,
    limit: Option<i64>,
}

async fn handler_api_agent_events(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AgentEventsQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100);
    match state.db.get_agent_events(params.task_id, limit).await {
        Ok(events) => Json(serde_json::json!({ "events": events })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn handler_api_agent_budgets(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_agent_budgets().await {
        Ok(budgets) => Json(serde_json::json!({ "budgets": budgets })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct UpdateBudgetPayload {
    id: i64,
    budget_usd: f64,
}

async fn handler_api_agent_budget_update(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateBudgetPayload>,
) -> impl IntoResponse {
    match state
        .db
        .update_agent_budget(payload.id, payload.budget_usd)
        .await
    {
        Ok(()) => Json(serde_json::json!({"ok": true, "id": payload.id})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Trigger manual re-verification of a prime.
async fn handler_api_prime_verify(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> impl IntoResponse {
    // Fetch the prime
    let prime = match state.db.get_prime_by_id(id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Prime not found"})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    // Run verification in a blocking task (uses GMP)
    let prime_clone = prime.clone();
    let result = match tokio::task::spawn_blocking(move || verify::verify_prime(&prime_clone)).await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Verification panicked: {}", e)})),
            )
                .into_response()
        }
    };

    match result {
        verify::VerifyResult::Verified { method, tier } => {
            if let Err(e) = state
                .db
                .mark_verified(id, &method, tier as i16)
                .await
            {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
                    .into_response();
            }
            Json(serde_json::json!({
                "ok": true,
                "result": "verified",
                "method": method,
                "tier": tier
            }))
            .into_response()
        }
        verify::VerifyResult::Failed { reason } => {
            let _ = state
                .db
                .mark_verification_failed(id, &reason)
                .await;
            Json(serde_json::json!({
                "ok": true,
                "result": "failed",
                "reason": reason
            }))
            .into_response()
        }
        verify::VerifyResult::Skipped { reason } => {
            Json(serde_json::json!({
                "ok": true,
                "result": "skipped",
                "reason": reason
            }))
            .into_response()
        }
    }
}

// --- Agent Memory API ---

async fn handler_api_agent_memory_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_all_agent_memory().await {
        Ok(entries) => Json(serde_json::json!({ "memories": entries })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct UpsertMemoryPayload {
    key: String,
    value: String,
    #[serde(default = "default_memory_category")]
    category: String,
}

fn default_memory_category() -> String {
    "general".to_string()
}

async fn handler_api_agent_memory_upsert(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpsertMemoryPayload>,
) -> impl IntoResponse {
    if payload.key.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "key must not be empty"})),
        )
            .into_response();
    }
    match state
        .db
        .upsert_agent_memory(&payload.key, &payload.value, &payload.category, None)
        .await
    {
        Ok(entry) => (StatusCode::OK, Json(serde_json::json!(entry))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn handler_api_agent_memory_delete(
    State(state): State<Arc<AppState>>,
    AxumPath(key): AxumPath<String>,
) -> impl IntoResponse {
    match state.db.delete_agent_memory(&key).await {
        Ok(true) => Json(serde_json::json!({"ok": true, "key": key})).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Memory entry not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// --- Agent role endpoints ---

/// GET /api/agents/roles — List all agent roles.
async fn handler_api_agent_roles(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_all_roles().await {
        Ok(roles) => Json(serde_json::json!(roles)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/agents/roles/{name} — Get a single role by name.
async fn handler_api_agent_role_get(
    State(state): State<Arc<AppState>>,
    AxumPath(name): AxumPath<String>,
) -> impl IntoResponse {
    match state.db.get_role_by_name(&name).await {
        Ok(Some(role)) => Json(serde_json::json!(role)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Role '{}' not found", name)})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/agents/roles/{name}/templates — Get templates associated with a role.
async fn handler_api_agent_role_templates(
    State(state): State<Arc<AppState>>,
    AxumPath(name): AxumPath<String>,
) -> impl IntoResponse {
    match state.db.get_role_templates(&name).await {
        Ok(templates) => Json(serde_json::json!(templates)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// --- Agent template & decomposition endpoints ---

/// GET /api/agents/templates — List all workflow templates.
async fn handler_api_agent_templates(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_all_templates().await {
        Ok(templates) => Json(serde_json::json!(templates)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// POST /api/agents/templates/{name}/expand — Expand a template into parent + child tasks.
async fn handler_api_agent_template_expand(
    State(state): State<Arc<AppState>>,
    AxumPath(name): AxumPath<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let title = body
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("Untitled");
    let description = body
        .get("description")
        .and_then(|d| d.as_str())
        .unwrap_or("");
    let priority = body
        .get("priority")
        .and_then(|p| p.as_str())
        .unwrap_or("normal");
    let max_cost_usd = body
        .get("max_cost_usd")
        .and_then(|c| c.as_f64());
    let permission_level = body
        .get("permission_level")
        .and_then(|l| l.as_i64())
        .unwrap_or(1) as i32;
    let role_name = body
        .get("role_name")
        .and_then(|r| r.as_str());

    match state
        .db
        .expand_template(&name, title, description, priority, max_cost_usd, permission_level, role_name)
        .await
    {
        Ok(parent_id) => {
            let _ = state
                .db
                .insert_agent_event(
                    Some(parent_id),
                    "created",
                    None,
                    &format!("Template '{}' expanded into task tree", name),
                    None,
                )
                .await;
            Json(serde_json::json!({"ok": true, "parent_task_id": parent_id})).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/agents/tasks/{id}/children — Get child tasks of a parent task.
async fn handler_api_agent_task_children(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> impl IntoResponse {
    match state.db.get_child_tasks(id).await {
        Ok(children) => Json(serde_json::json!(children)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ── Project Management Endpoints ────────────────────────────────

#[derive(Deserialize)]
struct ProjectListQuery {
    status: Option<String>,
}

/// GET /api/projects — List all projects, optionally filtered by status.
async fn handler_api_projects_list(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ProjectListQuery>,
) -> impl IntoResponse {
    match state.db.get_projects(params.status.as_deref()).await {
        Ok(projects) => Json(serde_json::json!(projects)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct CreateProjectPayload {
    name: String,
    description: Option<String>,
    objective: String,
    form: String,
    #[serde(default)]
    target: serde_json::Value,
    #[serde(default)]
    competitive: serde_json::Value,
    #[serde(default)]
    strategy: serde_json::Value,
    #[serde(default)]
    infrastructure: serde_json::Value,
    #[serde(default)]
    budget: serde_json::Value,
    #[serde(default)]
    workers: serde_json::Value,
}

/// POST /api/projects — Create a project from JSON.
async fn handler_api_projects_create(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateProjectPayload>,
) -> impl IntoResponse {
    // Build a TOML-like config from the JSON payload
    let toml_str = format!(
        "[project]\nname = {:?}\ndescription = {:?}\nobjective = {:?}\nform = {:?}\n",
        payload.name,
        payload.description.as_deref().unwrap_or(""),
        payload.objective,
        payload.form,
    );

    // Build ProjectConfig directly from JSON fields
    let obj = match payload.objective.as_str() {
        "record" => project::Objective::Record,
        "survey" => project::Objective::Survey,
        "verification" => project::Objective::Verification,
        "custom" => project::Objective::Custom,
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid objective: {}", other)})),
            )
                .into_response();
        }
    };

    let target: project::TargetConfig = serde_json::from_value(payload.target.clone())
        .unwrap_or_default();
    let strategy: project::StrategyConfig = serde_json::from_value(payload.strategy.clone())
        .unwrap_or_default();

    let config = project::ProjectConfig {
        project: project::ProjectMeta {
            name: payload.name.clone(),
            description: payload.description.unwrap_or_default(),
            objective: obj,
            form: payload.form.clone(),
            author: String::new(),
            tags: vec![],
        },
        target,
        competitive: serde_json::from_value(payload.competitive).ok(),
        strategy,
        infrastructure: serde_json::from_value(payload.infrastructure).ok(),
        budget: serde_json::from_value(payload.budget).ok(),
        workers: serde_json::from_value(payload.workers).ok(),
    };

    match state.db.create_project(&config, Some(&toml_str)).await {
        Ok(id) => {
            let slug = project::slugify(&payload.name);
            eprintln!("Project '{}' created (id={}, slug={})", payload.name, id, slug);
            (
                StatusCode::CREATED,
                Json(serde_json::json!({"id": id, "slug": slug})),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct ImportTomlPayload {
    toml: String,
}

/// POST /api/projects/import — Import a project from TOML content.
async fn handler_api_projects_import(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ImportTomlPayload>,
) -> impl IntoResponse {
    let config = match project::parse_toml(&payload.toml) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("TOML parse error: {}", e)})),
            )
                .into_response();
        }
    };

    match state.db.create_project(&config, Some(&payload.toml)).await {
        Ok(id) => {
            let slug = project::slugify(&config.project.name);
            eprintln!(
                "Project '{}' imported (id={}, slug={})",
                config.project.name, id, slug
            );
            (
                StatusCode::CREATED,
                Json(serde_json::json!({"id": id, "slug": slug})),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/projects/{slug} — Get project details with phases and recent events.
async fn handler_api_project_get(
    State(state): State<Arc<AppState>>,
    AxumPath(slug): AxumPath<String>,
) -> impl IntoResponse {
    let proj = match state.db.get_project_by_slug(&slug).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let phases = state
        .db
        .get_project_phases(proj.id)
        .await
        .unwrap_or_default();
    let events = state
        .db
        .get_project_events(proj.id, 50)
        .await
        .unwrap_or_default();

    Json(serde_json::json!({
        "project": proj,
        "phases": phases,
        "events": events,
    }))
    .into_response()
}

/// POST /api/projects/{slug}/activate — Start project orchestration.
async fn handler_api_project_activate(
    State(state): State<Arc<AppState>>,
    AxumPath(slug): AxumPath<String>,
) -> impl IntoResponse {
    let proj = match state.db.get_project_by_slug(&slug).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    if proj.status != "draft" && proj.status != "paused" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Cannot activate project with status '{}'", proj.status)
            })),
        )
            .into_response();
    }

    if let Err(e) = state.db.update_project_status(proj.id, "active").await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    state
        .db
        .insert_project_event(
            proj.id,
            "activated",
            &format!("Project '{}' activated via API", proj.name),
            None,
        )
        .await
        .ok();

    eprintln!("Project '{}' activated via API", slug);
    Json(serde_json::json!({"ok": true, "status": "active"})).into_response()
}

/// POST /api/projects/{slug}/pause — Pause project orchestration.
async fn handler_api_project_pause(
    State(state): State<Arc<AppState>>,
    AxumPath(slug): AxumPath<String>,
) -> impl IntoResponse {
    let proj = match state.db.get_project_by_slug(&slug).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    if proj.status != "active" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Cannot pause project with status '{}'", proj.status)
            })),
        )
            .into_response();
    }

    if let Err(e) = state.db.update_project_status(proj.id, "paused").await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    state
        .db
        .insert_project_event(
            proj.id,
            "paused",
            &format!("Project '{}' paused via API", proj.name),
            None,
        )
        .await
        .ok();

    Json(serde_json::json!({"ok": true, "status": "paused"})).into_response()
}

/// POST /api/projects/{slug}/cancel — Cancel a project.
async fn handler_api_project_cancel(
    State(state): State<Arc<AppState>>,
    AxumPath(slug): AxumPath<String>,
) -> impl IntoResponse {
    let proj = match state.db.get_project_by_slug(&slug).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    if let Err(e) = state.db.update_project_status(proj.id, "cancelled").await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    state
        .db
        .insert_project_event(
            proj.id,
            "cancelled",
            &format!("Project '{}' cancelled via API", proj.name),
            None,
        )
        .await
        .ok();

    Json(serde_json::json!({"ok": true, "status": "cancelled"})).into_response()
}

/// GET /api/projects/{slug}/events — Get project activity log.
async fn handler_api_project_events(
    State(state): State<Arc<AppState>>,
    AxumPath(slug): AxumPath<String>,
) -> impl IntoResponse {
    let proj = match state.db.get_project_by_slug(&slug).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    match state.db.get_project_events(proj.id, 100).await {
        Ok(events) => Json(serde_json::json!(events)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/projects/{slug}/cost — Get cost estimate for a project.
async fn handler_api_project_cost(
    State(state): State<Arc<AppState>>,
    AxumPath(slug): AxumPath<String>,
) -> impl IntoResponse {
    let proj = match state.db.get_project_by_slug(&slug).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    // Parse the stored TOML to estimate cost, or build a minimal config from DB fields
    let config = if let Some(toml_src) = &proj.toml_source {
        match project::parse_toml(toml_src) {
            Ok(c) => c,
            Err(_) => return Json(serde_json::json!({"error": "Invalid stored TOML"})).into_response(),
        }
    } else {
        // Build minimal config from DB fields
        project::ProjectConfig {
            project: project::ProjectMeta {
                name: proj.name.clone(),
                description: proj.description.clone(),
                objective: match proj.objective.as_str() {
                    "record" => project::Objective::Record,
                    "survey" => project::Objective::Survey,
                    "verification" => project::Objective::Verification,
                    _ => project::Objective::Custom,
                },
                form: proj.form.clone(),
                author: String::new(),
                tags: vec![],
            },
            target: serde_json::from_value(proj.target.clone()).unwrap_or_default(),
            competitive: serde_json::from_value(proj.competitive.clone()).ok(),
            strategy: serde_json::from_value(proj.strategy.clone()).unwrap_or_default(),
            infrastructure: serde_json::from_value(proj.infrastructure.clone()).ok(),
            budget: serde_json::from_value(proj.budget.clone()).ok(),
            workers: None,
        }
    };

    let estimate = project::estimate_project_cost(&config);
    Json(serde_json::json!(estimate)).into_response()
}

// ── Records Endpoints ───────────────────────────────────────────

/// GET /api/records — Get all world records with our-best comparison.
async fn handler_api_records(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_records().await {
        Ok(records) => Json(serde_json::json!(records)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// POST /api/records/refresh — Trigger manual records refresh from t5k.org.
async fn handler_api_records_refresh(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match project::refresh_all_records(&state.db).await {
        Ok(n) => {
            eprintln!("Records manually refreshed: {} forms updated", n);
            Json(serde_json::json!({"ok": true, "updated": n})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
