//! # Dashboard — Web Server and Fleet Coordination Hub
//!
//! Runs an Axum HTTP server that serves the Next.js frontend, provides REST API
//! endpoints for prime data, and coordinates the distributed worker fleet via
//! WebSocket and HTTP heartbeat.

mod routes_agents;
mod routes_docs;
mod routes_fleet;
mod routes_health;
mod routes_jobs;
mod routes_notifications;
mod routes_observability;
mod routes_projects;
mod routes_searches;
mod routes_status;
mod routes_verify;
mod routes_volunteer;
mod routes_workers;
mod websocket;

use crate::{
    agent, db, deploy, events, fleet, metrics, project, prom_metrics, search_manager, verify,
};
use anyhow::Result;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::Router;
use chrono::{DateTime, Timelike, Utc};
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

/// Lock a mutex, recovering from poisoning.
pub(super) fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct AppState {
    pub db: db::Database,
    pub database_url: String,
    pub checkpoint_path: PathBuf,
    pub coordinator_hostname: String,
    pub fleet: Mutex<fleet::Fleet>,
    pub searches: Mutex<search_manager::SearchManager>,
    pub deployments: Mutex<deploy::DeploymentManager>,
    pub coordinator_metrics: Mutex<Option<metrics::HardwareMetrics>>,
    pub event_bus: events::EventBus,
    pub agents: Mutex<agent::AgentManager>,
    pub prom_metrics: prom_metrics::Metrics,
}

impl AppState {
    pub(super) async fn get_workers_from_pg(&self) -> Vec<fleet::WorkerState> {
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
            coordinator_hostname: gethostname(),
            fleet: Mutex::new(fleet::Fleet::new()),
            searches: Mutex::new(search_manager::SearchManager::new(port, database_url)),
            deployments: Mutex::new(deploy::DeploymentManager::new()),
            coordinator_metrics: Mutex::new(None),
            event_bus: events::EventBus::new(),
            agents: Mutex::new(agent::AgentManager::new()),
            prom_metrics: prom_metrics::Metrics::new(),
        })
    }
}

pub(super) fn gethostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "unknown".to_string())
}

pub fn build_router(state: Arc<AppState>, static_dir: Option<&Path>) -> Router {
    let mut app = Router::new()
        .route("/ws", get(websocket::handler_ws))
        .route("/api/status", get(routes_status::handler_api_status))
        .route("/api/docs", get(routes_docs::handler_api_docs))
        .route(
            "/api/docs/search",
            get(routes_docs::handler_api_docs_search),
        )
        .route(
            "/api/docs/roadmaps/{slug}",
            get(routes_docs::handler_api_doc_roadmap),
        )
        .route(
            "/api/docs/agent/{slug}",
            get(routes_docs::handler_api_doc_agent),
        )
        .route("/api/docs/{slug}", get(routes_docs::handler_api_doc))
        .route("/api/export", get(routes_status::handler_api_export))
        .route(
            "/api/ws-snapshot",
            get(routes_status::handler_api_ws_snapshot),
        )
        .route("/api/fleet", get(routes_fleet::handler_api_fleet))
        .route(
            "/api/fleet/deploy",
            post(routes_fleet::handler_fleet_deploy),
        )
        .route(
            "/api/fleet/deploy/{id}",
            axum::routing::delete(routes_fleet::handler_fleet_deploy_stop),
        )
        .route(
            "/api/fleet/deploy/{id}/pause",
            post(routes_fleet::handler_fleet_deploy_pause),
        )
        .route(
            "/api/fleet/deploy/{id}/resume",
            post(routes_fleet::handler_fleet_deploy_resume),
        )
        .route(
            "/api/fleet/deployments",
            get(routes_fleet::handler_fleet_deployments),
        )
        .route(
            "/api/searches",
            get(routes_searches::handler_api_searches_list)
                .post(routes_searches::handler_api_searches_create),
        )
        .route(
            "/api/searches/{id}",
            get(routes_searches::handler_api_searches_get)
                .delete(routes_searches::handler_api_searches_stop),
        )
        .route(
            "/api/searches/{id}/pause",
            post(routes_searches::handler_api_searches_pause),
        )
        .route(
            "/api/searches/{id}/resume",
            post(routes_searches::handler_api_searches_resume),
        )
        .route(
            "/api/worker/register",
            post(routes_workers::handler_worker_register),
        )
        .route(
            "/api/worker/heartbeat",
            post(routes_workers::handler_worker_heartbeat),
        )
        .route(
            "/api/worker/prime",
            post(routes_workers::handler_worker_prime),
        )
        .route(
            "/api/worker/deregister",
            post(routes_workers::handler_worker_deregister),
        )
        .route(
            "/api/fleet/workers/{worker_id}/stop",
            post(routes_fleet::handler_fleet_worker_stop),
        )
        .route(
            "/api/search_jobs",
            get(routes_jobs::handler_api_search_jobs_list)
                .post(routes_jobs::handler_api_search_jobs_create),
        )
        .route(
            "/api/search_jobs/{id}",
            get(routes_jobs::handler_api_search_job_get),
        )
        .route(
            "/api/search_jobs/{id}/cancel",
            post(routes_jobs::handler_api_search_job_cancel),
        )
        .route(
            "/api/notifications",
            get(routes_notifications::handler_api_notifications),
        )
        .route("/api/events", get(routes_notifications::handler_api_events))
        .route(
            "/api/observability/metrics",
            get(routes_observability::handler_metrics),
        )
        .route(
            "/api/observability/logs",
            get(routes_observability::handler_logs),
        )
        .route(
            "/api/observability/report",
            get(routes_observability::handler_report),
        )
        .route(
            "/api/agents/tasks",
            get(routes_agents::handler_api_agent_tasks)
                .post(routes_agents::handler_api_agent_task_create),
        )
        .route(
            "/api/agents/tasks/{id}",
            get(routes_agents::handler_api_agent_task_get),
        )
        .route(
            "/api/agents/tasks/{id}/cancel",
            post(routes_agents::handler_api_agent_task_cancel),
        )
        .route(
            "/api/agents/events",
            get(routes_agents::handler_api_agent_events),
        )
        .route(
            "/api/agents/templates",
            get(routes_agents::handler_api_agent_templates),
        )
        .route(
            "/api/agents/templates/{name}/expand",
            post(routes_agents::handler_api_agent_template_expand),
        )
        .route(
            "/api/agents/tasks/{id}/children",
            get(routes_agents::handler_api_agent_task_children),
        )
        .route(
            "/api/agents/budgets",
            get(routes_agents::handler_api_agent_budgets)
                .put(routes_agents::handler_api_agent_budget_update),
        )
        .route(
            "/api/primes/{id}/verify",
            post(routes_verify::handler_api_prime_verify),
        )
        .route(
            "/api/agents/memory",
            get(routes_agents::handler_api_agent_memory_list)
                .post(routes_agents::handler_api_agent_memory_upsert),
        )
        .route(
            "/api/agents/memory/{key}",
            axum::routing::delete(routes_agents::handler_api_agent_memory_delete),
        )
        .route(
            "/api/agents/roles",
            get(routes_agents::handler_api_agent_roles),
        )
        .route(
            "/api/agents/roles/{name}",
            get(routes_agents::handler_api_agent_role_get),
        )
        .route(
            "/api/agents/roles/{name}/templates",
            get(routes_agents::handler_api_agent_role_templates),
        )
        .route(
            "/api/projects",
            get(routes_projects::handler_api_projects_list)
                .post(routes_projects::handler_api_projects_create),
        )
        .route(
            "/api/projects/import",
            post(routes_projects::handler_api_projects_import),
        )
        .route(
            "/api/projects/{slug}",
            get(routes_projects::handler_api_project_get),
        )
        .route(
            "/api/projects/{slug}/activate",
            post(routes_projects::handler_api_project_activate),
        )
        .route(
            "/api/projects/{slug}/pause",
            post(routes_projects::handler_api_project_pause),
        )
        .route(
            "/api/projects/{slug}/cancel",
            post(routes_projects::handler_api_project_cancel),
        )
        .route(
            "/api/projects/{slug}/events",
            get(routes_projects::handler_api_project_events),
        )
        .route(
            "/api/projects/{slug}/cost",
            get(routes_projects::handler_api_project_cost),
        )
        .route("/api/records", get(routes_projects::handler_api_records))
        .route(
            "/api/records/refresh",
            post(routes_projects::handler_api_records_refresh),
        )
        .route("/healthz", get(routes_health::handler_healthz))
        .route("/readyz", get(routes_health::handler_readyz))
        .route("/metrics", get(routes_health::handler_metrics))
        // Volunteer public API (v1)
        .route(
            "/api/v1/register",
            post(routes_volunteer::handler_v1_register),
        )
        .route(
            "/api/v1/worker/register",
            post(routes_volunteer::handler_v1_worker_register),
        )
        .route(
            "/api/v1/worker/heartbeat",
            post(routes_volunteer::handler_v1_worker_heartbeat),
        )
        .route(
            "/api/v1/worker/latest",
            get(routes_volunteer::handler_worker_latest),
        )
        .route("/api/v1/work", get(routes_volunteer::handler_v1_work))
        .route("/api/v1/result", post(routes_volunteer::handler_v1_result))
        .route("/api/v1/stats", get(routes_volunteer::handler_v1_stats))
        .route(
            "/api/v1/leaderboard",
            get(routes_volunteer::handler_v1_leaderboard),
        )
        .route(
            "/api/volunteer/worker/latest",
            get(routes_volunteer::handler_worker_latest),
        );

    if let Some(dir) = static_dir {
        app = app.fallback_service(ServeDir::new(dir).append_index_html_on_directories(true));
    } else {
        app = app.route("/", get(routes_status::handler_index));
    }

    app.layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any),
    )
    .layer(CatchPanicLayer::new())
    .layer(RequestBodyLimitLayer::new(1024 * 1024))
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
        let mut last_metrics_sample = std::time::Instant::now() - Duration::from_secs(60);
        let mut last_worker_sample = std::time::Instant::now() - Duration::from_secs(120);
        let mut last_housekeeping = std::time::Instant::now() - Duration::from_secs(3600);
        let mut last_event_id: u64 = 0;
        let log_retention_days: i64 = std::env::var("OBS_LOG_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        let metric_retention_days: i64 = std::env::var("OBS_METRIC_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(7);
        let rollup_retention_days: i64 = std::env::var("OBS_ROLLUP_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(365);
        loop {
            interval.tick().await;
            lock_or_recover(&prune_state.fleet).prune_stale(60);
            if let Err(e) = prune_state.db.prune_stale_workers(120).await {
                eprintln!("Warning: failed to prune stale PG workers: {}", e);
            }
            match prune_state.db.rotate_agent_budget_periods().await {
                Ok(n) if n > 0 => eprintln!("Rotated {} budget periods", n),
                Err(e) => eprintln!("Warning: failed to rotate budget periods: {}", e),
                _ => {}
            }
            match prune_state.db.reclaim_stale_blocks(120).await {
                Ok(n) if n > 0 => eprintln!("Reclaimed {} stale work blocks", n),
                Err(e) => eprintln!("Warning: failed to reclaim stale blocks: {}", e),
                _ => {}
            }
            // Volunteer blocks get a 24-hour timeout (86400s) vs 2-min for internal workers
            match prune_state.db.reclaim_stale_volunteer_blocks(86400).await {
                Ok(n) if n > 0 => eprintln!("Reclaimed {} stale volunteer blocks", n),
                Err(e) => eprintln!("Warning: failed to reclaim stale volunteer blocks: {}", e),
                _ => {}
            }
            let fleet_workers = prune_state.get_workers_from_pg().await;
            {
                let worker_stats: Vec<(String, u64, u64)> = fleet_workers
                    .iter()
                    .map(|w| (w.worker_id.clone(), w.tested, w.found))
                    .collect();
                let mut mgr = lock_or_recover(&prune_state.searches);
                mgr.sync_worker_stats(&worker_stats);
                mgr.poll_completed();
            }
            if let Err(e) = project::orchestrate_tick(&prune_state.db).await {
                eprintln!("Warning: project orchestration tick failed: {}", e);
            }
            prune_state.event_bus.flush();
            {
                let events = prune_state.event_bus.recent_events_since(last_event_id, 200);
                if let Some(last) = events.last() {
                    last_event_id = last.id;
                }
                if !events.is_empty() {
                    let logs: Vec<db::SystemLogEntry> = events
                        .into_iter()
                        .map(|e| {
                            let level = match e.kind.as_str() {
                                "error" => "error",
                                "warning" => "warn",
                                _ => "info",
                            };
                            let ts = std::time::SystemTime::UNIX_EPOCH
                                + std::time::Duration::from_millis(e.timestamp_ms);
                            db::SystemLogEntry {
                                ts: DateTime::<Utc>::from(ts),
                                level: level.to_string(),
                                source: "coordinator".to_string(),
                                component: "event_bus".to_string(),
                                message: e.message,
                                worker_id: None,
                                search_job_id: None,
                                search_id: None,
                                context: Some(serde_json::json!({"kind": e.kind, "elapsed_secs": e.elapsed_secs})),
                            }
                        })
                        .collect();
                    if let Err(e) = prune_state.db.insert_system_logs(&logs).await {
                        eprintln!("Warning: failed to persist event logs: {}", e);
                    }
                }
            }
            sys.refresh_cpu_all();
            sys.refresh_memory();
            let hw = metrics::collect(&sys);

            // Update Prometheus gauges from hardware metrics and fleet state
            prune_state
                .prom_metrics
                .cpu_usage_percent
                .set(hw.cpu_usage_percent as f64);
            prune_state
                .prom_metrics
                .memory_usage_percent
                .set(hw.memory_usage_percent as f64);
            prune_state
                .prom_metrics
                .workers_connected
                .set(fleet_workers.len() as i64);
            {
                let mgr = lock_or_recover(&prune_state.searches);
                prune_state
                    .prom_metrics
                    .search_jobs_active
                    .set(mgr.active_count() as i64);
            }
            let mut block_summary = None;
            if let Ok(summary) = prune_state.db.get_all_block_summary().await {
                prune_state
                    .prom_metrics
                    .work_blocks_available
                    .set(summary.available);
                prune_state
                    .prom_metrics
                    .work_blocks_claimed
                    .set(summary.claimed);
                block_summary = Some(summary);
            }

            *lock_or_recover(&prune_state.coordinator_metrics) = Some(hw.clone());

            if last_metrics_sample.elapsed() >= Duration::from_secs(60) {
                last_metrics_sample = std::time::Instant::now();
                let now = Utc::now();
                let mut samples: Vec<db::MetricSample> = Vec::new();

                samples.push(db::MetricSample {
                    ts: now,
                    scope: "coordinator".to_string(),
                    metric: "coordinator.cpu_usage_percent".to_string(),
                    value: hw.cpu_usage_percent as f64,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "coordinator".to_string(),
                    metric: "coordinator.memory_usage_percent".to_string(),
                    value: hw.memory_usage_percent as f64,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "coordinator".to_string(),
                    metric: "coordinator.load_avg_1m".to_string(),
                    value: hw.load_avg_1m,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "coordinator".to_string(),
                    metric: "coordinator.load_avg_5m".to_string(),
                    value: hw.load_avg_5m,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "coordinator".to_string(),
                    metric: "coordinator.load_avg_15m".to_string(),
                    value: hw.load_avg_15m,
                    labels: None,
                });

                let total_cores: i64 = fleet_workers.iter().map(|w| w.cores as i64).sum();
                let total_tested: i64 = fleet_workers.iter().map(|w| w.tested as i64).sum();
                let total_found: i64 = fleet_workers.iter().map(|w| w.found as i64).sum();

                samples.push(db::MetricSample {
                    ts: now,
                    scope: "fleet".to_string(),
                    metric: "fleet.workers_connected".to_string(),
                    value: fleet_workers.len() as f64,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "fleet".to_string(),
                    metric: "fleet.total_cores".to_string(),
                    value: total_cores as f64,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "fleet".to_string(),
                    metric: "fleet.total_tested".to_string(),
                    value: total_tested as f64,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "fleet".to_string(),
                    metric: "fleet.total_found".to_string(),
                    value: total_found as f64,
                    labels: None,
                });

                if let Some(summary) = &block_summary {
                    samples.push(db::MetricSample {
                        ts: now,
                        scope: "fleet".to_string(),
                        metric: "fleet.work_blocks_available".to_string(),
                        value: summary.available as f64,
                        labels: None,
                    });
                    samples.push(db::MetricSample {
                        ts: now,
                        scope: "fleet".to_string(),
                        metric: "fleet.work_blocks_claimed".to_string(),
                        value: summary.claimed as f64,
                        labels: None,
                    });
                }

                {
                    let mgr = lock_or_recover(&prune_state.searches);
                    samples.push(db::MetricSample {
                        ts: now,
                        scope: "fleet".to_string(),
                        metric: "fleet.search_jobs_active".to_string(),
                        value: mgr.active_count() as f64,
                        labels: None,
                    });
                }

                if last_worker_sample.elapsed() >= Duration::from_secs(120) {
                    last_worker_sample = std::time::Instant::now();
                    for w in &fleet_workers {
                        if let Some(m) = &w.metrics {
                            let labels = serde_json::json!({
                                "worker_id": w.worker_id,
                                "hostname": w.hostname,
                                "search_type": w.search_type,
                            });
                            samples.push(db::MetricSample {
                                ts: now,
                                scope: "worker".to_string(),
                                metric: "worker.cpu_usage_percent".to_string(),
                                value: m.cpu_usage_percent as f64,
                                labels: Some(labels.clone()),
                            });
                            samples.push(db::MetricSample {
                                ts: now,
                                scope: "worker".to_string(),
                                metric: "worker.memory_usage_percent".to_string(),
                                value: m.memory_usage_percent as f64,
                                labels: Some(labels.clone()),
                            });
                            samples.push(db::MetricSample {
                                ts: now,
                                scope: "worker".to_string(),
                                metric: "worker.disk_usage_percent".to_string(),
                                value: m.disk_usage_percent as f64,
                                labels: Some(labels.clone()),
                            });
                        }
                        let labels = serde_json::json!({
                            "worker_id": w.worker_id,
                            "hostname": w.hostname,
                            "search_type": w.search_type,
                        });
                        samples.push(db::MetricSample {
                            ts: now,
                            scope: "worker".to_string(),
                            metric: "worker.tested".to_string(),
                            value: w.tested as f64,
                            labels: Some(labels.clone()),
                        });
                        samples.push(db::MetricSample {
                            ts: now,
                            scope: "worker".to_string(),
                            metric: "worker.found".to_string(),
                            value: w.found as f64,
                            labels: Some(labels.clone()),
                        });
                    }
                }

                if let Err(e) = prune_state.db.insert_metric_samples(&samples).await {
                    eprintln!("Warning: failed to persist metric samples: {}", e);
                }
            }

            if last_housekeeping.elapsed() >= Duration::from_secs(3600) {
                last_housekeeping = std::time::Instant::now();
                let now = Utc::now();
                let hour_start = now
                    .with_minute(0)
                    .and_then(|t| t.with_second(0))
                    .and_then(|t| t.with_nanosecond(0))
                    .unwrap_or(now);
                let prev_hour = hour_start - chrono::Duration::hours(1);
                if let Err(e) = prune_state.db.rollup_metrics_hour(prev_hour).await {
                    eprintln!("Warning: failed to roll up metrics: {}", e);
                }
                if let Err(e) = prune_state.db.prune_metric_samples(metric_retention_days).await {
                    eprintln!("Warning: failed to prune metric samples: {}", e);
                }
                if let Err(e) = prune_state.db.prune_metric_rollups(rollup_retention_days).await {
                    eprintln!("Warning: failed to prune metric rollups: {}", e);
                }
                if let Err(e) = prune_state.db.prune_system_logs(log_retention_days).await {
                    eprintln!("Warning: failed to prune system logs: {}", e);
                }
            }
        }
    });

    // Background task: auto-verify newly discovered primes (60s interval)
    let verify_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.tick().await;
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
        interval.tick().await;
        loop {
            interval.tick().await;
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
                        Some(serde_json::json!({"text": r.result_text})),
                        r.tokens_used,
                        r.cost_usd,
                    ),
                    None => (
                        reason.as_ref().map(|r| serde_json::json!({"error": r})),
                        0,
                        0.0,
                    ),
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
                if let Ok(Some(completed_task)) = agent_state.db.get_agent_task(c.task_id).await {
                    if let Some(parent_id) = completed_task.parent_task_id {
                        if status_str == "failed" {
                            if let Ok(Some(parent)) = agent_state.db.get_agent_task(parent_id).await
                            {
                                if parent.on_child_failure == "fail" {
                                    let cancelled = agent_state
                                        .db
                                        .cancel_pending_siblings(parent_id)
                                        .await
                                        .unwrap_or(0);
                                    if cancelled > 0 {
                                        eprintln!(
                                            "Agent: cancelled {} pending siblings of parent {}",
                                            cancelled, parent_id
                                        );
                                    }
                                }
                            }
                        }
                        if let Ok(Some(parent)) =
                            agent_state.db.try_complete_parent(parent_id).await
                        {
                            let event_type = if parent.status == "failed" {
                                "parent_failed"
                            } else {
                                "parent_completed"
                            };
                            let _ = agent_state
                                .db
                                .insert_agent_event(
                                    Some(parent_id),
                                    event_type,
                                    None,
                                    &format!(
                                        "Parent task '{}' auto-{}",
                                        parent.title, parent.status
                                    ),
                                    None,
                                )
                                .await;
                            eprintln!("Agent: parent task {} auto-{}", parent_id, parent.status);
                        }
                    }
                }
            }
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
            let active = lock_or_recover(&agent_state.agents).active_count();
            if active >= agent::MAX_AGENTS {
                continue;
            }
            let task = match agent_state.db.claim_pending_agent_task(&agent_name).await {
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
            let role = if let Some(ref rn) = task.role_name {
                agent_state.db.get_role_by_name(rn).await.ok().flatten()
            } else {
                None
            };
            let context_prompts =
                agent::assemble_context(&task, &agent_state.db, role.as_ref()).await;
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

    // Background task: refresh world records from t5k.org (24h interval)
    let records_state = Arc::clone(&state);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        eprintln!("Records: initial refresh from t5k.org...");
        match project::refresh_all_records(&records_state.db).await {
            Ok(n) => eprintln!("Records: refreshed {} forms", n),
            Err(e) => eprintln!("Warning: records refresh failed: {}", e),
        }
        let mut interval = tokio::time::interval(Duration::from_secs(24 * 3600));
        interval.tick().await;
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
        tokio::select! { _ = ctrl_c => eprintln!("\nReceived SIGINT, shutting down..."), _ = sigterm.recv() => eprintln!("\nReceived SIGTERM, shutting down...") }
    }
    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
        eprintln!("\nReceived SIGINT, shutting down...");
    }
}
