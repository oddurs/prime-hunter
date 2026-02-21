//! # Dashboard — Web Server and Fleet Coordination Hub
//!
//! Runs an Axum HTTP server that serves the Next.js frontend, provides REST API
//! endpoints for prime data, and coordinates the distributed worker fleet via
//! WebSocket and HTTP heartbeat.

pub(crate) mod middleware_auth;
mod routes_agents;
mod routes_auth;
mod routes_docs;
mod routes_fleet;
mod routes_health;
mod routes_jobs;
mod routes_notifications;
mod routes_observability;
mod routes_projects;
mod routes_releases;
mod routes_searches;
mod routes_status;
mod routes_strategy;
mod routes_verify;
mod routes_operator;
mod websocket;

use crate::{agent, db, events, fleet, metrics, project, prom_metrics, strategy, verify};
use tracing::{info, warn, Instrument};
use anyhow::Result;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
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
use tower_http::trace::TraceLayer;

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
                warn!(error = %e, "failed to read workers from PG");
                Vec::new()
            }
        }
    }

    pub fn with_db(
        db: db::Database,
        database_url: &str,
        checkpoint_path: PathBuf,
    ) -> Arc<Self> {
        Arc::new(AppState {
            db,
            database_url: database_url.to_string(),
            checkpoint_path,
            coordinator_hostname: gethostname(),
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
        .or_else(|_| sysinfo::System::host_name().ok_or(std::env::VarError::NotPresent))
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Middleware that records HTTP request duration into the Prometheus histogram,
/// generates (or propagates) a request ID for correlation, and wraps the
/// request in a tracing span using `.instrument()` for proper async propagation.
async fn metrics_middleware(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> axum::response::Response {
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let method = req.method().to_string();
    let raw_path = req.uri().path().to_string();
    let norm_path = normalize_path(&raw_path);
    let start = std::time::Instant::now();

    let span = tracing::info_span!(
        "request",
        request_id = %request_id,
        method = %method,
        path = %raw_path,
    );
    let response = next.run(req).instrument(span).await;

    let duration = start.elapsed().as_secs_f64();
    state
        .prom_metrics
        .http_request_duration
        .get_or_create(&prom_metrics::HttpLabel {
            method,
            path: norm_path,
        })
        .observe(duration);

    let mut response = response;
    response.headers_mut().insert(
        "x-request-id",
        request_id.parse().unwrap(),
    );
    response
}

/// Normalize URL path to collapse high-cardinality segments (UUIDs, numeric IDs)
/// into placeholders, preventing histogram label explosion.
fn normalize_path(path: &str) -> String {
    path.split('/')
        .map(|seg| {
            if seg.is_empty() {
                seg.to_string()
            } else if seg.chars().all(|c| c.is_ascii_digit()) {
                ":id".to_string()
            } else if seg.len() == 36 && seg.chars().filter(|c| *c == '-').count() == 4 {
                ":uuid".to_string()
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
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
            "/api/observability/workers/top",
            get(routes_observability::handler_top_workers),
        )
        .route(
            "/api/observability/catalog",
            get(routes_observability::handler_catalog),
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
        .route(
            "/api/releases/worker",
            get(routes_releases::handler_releases_list)
                .post(routes_releases::handler_releases_upsert),
        )
        .route(
            "/api/releases/events",
            get(routes_releases::handler_releases_events),
        )
        .route(
            "/api/releases/health",
            get(routes_releases::handler_releases_health),
        )
        .route(
            "/api/releases/rollout",
            post(routes_releases::handler_releases_rollout),
        )
        .route(
            "/api/releases/rollback",
            post(routes_releases::handler_releases_rollback),
        )
        // Strategy engine
        .route(
            "/api/strategy/status",
            get(routes_strategy::handler_strategy_status),
        )
        .route(
            "/api/strategy/decisions",
            get(routes_strategy::handler_strategy_decisions),
        )
        .route(
            "/api/strategy/scores",
            get(routes_strategy::handler_strategy_scores),
        )
        .route(
            "/api/strategy/config",
            get(routes_strategy::handler_strategy_config_get)
                .put(routes_strategy::handler_strategy_config_put),
        )
        .route(
            "/api/strategy/decisions/{id}/override",
            post(routes_strategy::handler_strategy_override),
        )
        .route(
            "/api/strategy/tick",
            post(routes_strategy::handler_strategy_tick),
        )
        .route("/api/records", get(routes_projects::handler_api_records))
        .route(
            "/api/records/refresh",
            post(routes_projects::handler_api_records_refresh),
        )
        .route(
            "/api/auth/profile",
            get(routes_auth::handler_api_profile),
        )
        .route("/api/auth/me", get(routes_auth::handler_api_me))
        .route("/healthz", get(routes_health::handler_healthz))
        .route("/readyz", get(routes_health::handler_readyz))
        .route("/metrics", get(routes_health::handler_metrics))
        // Operator public API (v1) — new canonical routes
        .route(
            "/api/v1/operators/register",
            post(routes_operator::handler_v1_register),
        )
        .route(
            "/api/v1/nodes/register",
            post(routes_operator::handler_v1_worker_register),
        )
        .route(
            "/api/v1/nodes/heartbeat",
            post(routes_operator::handler_v1_worker_heartbeat),
        )
        .route(
            "/api/v1/nodes/latest",
            get(routes_operator::handler_worker_latest),
        )
        .route("/api/v1/nodes/work", get(routes_operator::handler_v1_work))
        .route(
            "/api/v1/nodes/result",
            post(routes_operator::handler_v1_result),
        )
        .route(
            "/api/v1/operators/stats",
            get(routes_operator::handler_v1_stats),
        )
        .route(
            "/api/v1/operators/leaderboard",
            get(routes_operator::handler_v1_leaderboard),
        )
        .route(
            "/api/v1/operators/me/nodes",
            get(routes_operator::handler_v1_operator_nodes),
        )
        .route(
            "/api/v1/operators/rotate-key",
            post(routes_operator::handler_v1_rotate_key),
        )
        // Legacy routes (kept for backward compatibility, 2 release cycles)
        .route(
            "/api/v1/register",
            post(routes_operator::handler_v1_register),
        )
        .route(
            "/api/v1/worker/register",
            post(routes_operator::handler_v1_worker_register),
        )
        .route(
            "/api/v1/worker/heartbeat",
            post(routes_operator::handler_v1_worker_heartbeat),
        )
        .route(
            "/api/v1/worker/latest",
            get(routes_operator::handler_worker_latest),
        )
        .route("/api/v1/work", get(routes_operator::handler_v1_work))
        .route("/api/v1/result", post(routes_operator::handler_v1_result))
        .route("/api/v1/stats", get(routes_operator::handler_v1_stats))
        .route(
            "/api/v1/leaderboard",
            get(routes_operator::handler_v1_leaderboard),
        )
        .route(
            "/api/volunteer/worker/latest",
            get(routes_operator::handler_worker_latest),
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
    .layer(axum::middleware::from_fn_with_state(
        state.clone(),
        metrics_middleware,
    ))
    .layer(TraceLayer::new_for_http())
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
    let state = AppState::with_db(database, database_url, checkpoint_path.to_path_buf());
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
        let mut last_strategy_tick = std::time::Instant::now() - Duration::from_secs(300);
        let mut last_reliability_refresh = std::time::Instant::now() - Duration::from_secs(300);
        let mut last_event_id: u64 = 0;
        let mut last_tick = std::time::Instant::now();
        let mut event_counts = std::collections::HashMap::<String, i64>::new();
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
        let daily_rollup_retention_days: i64 = std::env::var("OBS_DAILY_ROLLUP_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1825);
        loop {
            interval.tick().await;
            let tick_now = std::time::Instant::now();
            let tick_interval_ms = tick_now
                .duration_since(last_tick)
                .as_millis()
                .min(i64::MAX as u128) as i64;
            let tick_drift_ms = tick_interval_ms - 30_000;
            last_tick = tick_now;
            if let Err(e) = prune_state.db.prune_stale_workers(120).await {
                warn!(error = %e, "failed to prune stale workers");
            }
            match prune_state.db.rotate_agent_budget_periods().await {
                Ok(n) if n > 0 => info!(count = n, "rotated budget periods"),
                Err(e) => warn!(error = %e, "failed to rotate budget periods"),
                _ => {}
            }
            match prune_state.db.reclaim_stale_blocks(120).await {
                Ok(n) if n > 0 => info!(count = n, "reclaimed stale work blocks"),
                Err(e) => warn!(error = %e, "failed to reclaim stale blocks"),
                _ => {}
            }
            // Operator blocks get a 24-hour timeout (86400s) vs 2-min for internal workers
            match prune_state.db.reclaim_stale_operator_blocks(86400).await {
                Ok(n) if n > 0 => info!(count = n, "reclaimed stale operator blocks"),
                Err(e) => warn!(error = %e, "failed to reclaim stale operator blocks"),
                _ => {}
            }

            // ── Verification pipeline: queue unverified operator blocks ──
            match prune_state.db.get_unverified_operator_blocks(20).await {
                Ok(blocks) => {
                    for block in blocks {
                        // Look up operator trust level
                        let trust_level = if let Some(vol_id) = block.volunteer_id {
                            prune_state
                                .db
                                .get_operator_trust(vol_id)
                                .await
                                .ok()
                                .flatten()
                                .map(|t| t.trust_level)
                                .unwrap_or(1)
                        } else {
                            1
                        };

                        let quorum = verify::required_quorum(trust_level, &block.search_type);

                        if quorum >= 2 {
                            // Check if already queued for verification
                            let already_queued = prune_state
                                .db
                                .has_pending_verification(block.block_id as i64)
                                .await
                                .unwrap_or(false);
                            if !already_queued {
                                // Fetch block details for verification queue
                                if let Ok(Some(wb)) = prune_state
                                    .db
                                    .get_work_block_details(block.block_id as i64)
                                    .await
                                {
                                    if let Err(e) = prune_state
                                        .db
                                        .queue_verification(
                                            block.block_id as i64,
                                            block.search_job_id,
                                            wb.block_start,
                                            wb.block_end,
                                            wb.tested,
                                            wb.found,
                                            &wb.claimed_by,
                                            block.volunteer_id,
                                        )
                                        .await
                                    {
                                        warn!(
                                            block_id = block.block_id,
                                            error = %e,
                                            "failed to queue verification"
                                        );
                                    }
                                }
                            }
                        } else {
                            // Trusted or provable form: mark verified directly
                            if let Err(e) = prune_state
                                .db
                                .mark_block_verified(block.block_id)
                                .await
                            {
                                warn!(
                                    block_id = block.block_id,
                                    error = %e,
                                    "failed to mark block verified"
                                );
                            }
                            // Record valid result for trust advancement
                            if let Some(vol_id) = block.volunteer_id {
                                let _ = prune_state.db.record_valid_result(vol_id).await;
                            }
                        }
                    }
                }
                Err(e) => warn!(error = %e, "failed to fetch unverified operator blocks"),
            }

            // Refresh node reliability scores every 5 minutes
            if last_reliability_refresh.elapsed() >= Duration::from_secs(300) {
                last_reliability_refresh = std::time::Instant::now();
                // Node reliability is computed on-the-fly via SQL function,
                // so no explicit refresh needed. This is a placeholder for
                // future batch refresh of materialized reliability data.
            }

            let fleet_workers = prune_state.get_workers_from_pg().await;
            if let Err(e) = project::orchestrate_tick(&prune_state.db).await {
                warn!(error = %e, "project orchestration tick failed");
            }
            // Strategy engine tick (default every 300s / 5 minutes)
            if last_strategy_tick.elapsed() >= Duration::from_secs(300) {
                last_strategy_tick = std::time::Instant::now();
                match strategy::strategy_tick(&prune_state.db).await {
                    Ok(result) => {
                        let count = result.decisions.len();
                        if count > 0 {
                            info!(count, "strategy decisions applied");
                        }
                    }
                    Err(e) => warn!(error = %e, "strategy tick failed"),
                }
            }
            prune_state.event_bus.flush();
            {
                let events = prune_state
                    .event_bus
                    .recent_events_since(last_event_id, 200);
                if let Some(last) = events.last() {
                    last_event_id = last.id;
                }
                if !events.is_empty() {
                    for e in &events {
                        *event_counts.entry(e.kind.clone()).or_insert(0) += 1;
                    }
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
                        warn!(error = %e, "failed to persist event logs");
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

            // Connection pool stats
            let pool_size = prune_state.db.pool().size();
            let pool_idle = prune_state.db.pool().num_idle();
            prune_state.prom_metrics.db_pool_active.set((pool_size as i64) - (pool_idle as i64));
            prune_state.prom_metrics.db_pool_idle.set(pool_idle as i64);
            prune_state.prom_metrics.db_pool_max.set(2); // matches PgPoolOptions::max_connections(2)

            if let Ok(jobs) = prune_state.db.get_search_jobs().await {
                let active = jobs.iter().filter(|j| j.status == "running").count();
                prune_state
                    .prom_metrics
                    .search_jobs_active
                    .set(active as i64);
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
                    metric: "coordinator.tick_interval_ms".to_string(),
                    value: tick_interval_ms as f64,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "coordinator".to_string(),
                    metric: "coordinator.tick_drift_ms".to_string(),
                    value: tick_drift_ms as f64,
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
                let max_heartbeat_age: i64 = fleet_workers
                    .iter()
                    .map(|w| w.last_heartbeat_secs_ago as i64)
                    .max()
                    .unwrap_or(0);
                let avg_heartbeat_age: f64 = if fleet_workers.is_empty() {
                    0.0
                } else {
                    fleet_workers
                        .iter()
                        .map(|w| w.last_heartbeat_secs_ago as f64)
                        .sum::<f64>()
                        / fleet_workers.len() as f64
                };

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
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "fleet".to_string(),
                    metric: "fleet.max_heartbeat_age_secs".to_string(),
                    value: max_heartbeat_age as f64,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "fleet".to_string(),
                    metric: "fleet.avg_heartbeat_age_secs".to_string(),
                    value: avg_heartbeat_age,
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
                    samples.push(db::MetricSample {
                        ts: now,
                        scope: "fleet".to_string(),
                        metric: "fleet.work_blocks_completed".to_string(),
                        value: summary.completed as f64,
                        labels: None,
                    });
                    samples.push(db::MetricSample {
                        ts: now,
                        scope: "fleet".to_string(),
                        metric: "fleet.work_blocks_failed".to_string(),
                        value: summary.failed as f64,
                        labels: None,
                    });
                    samples.push(db::MetricSample {
                        ts: now,
                        scope: "fleet".to_string(),
                        metric: "fleet.block_total_tested".to_string(),
                        value: summary.total_tested as f64,
                        labels: None,
                    });
                    samples.push(db::MetricSample {
                        ts: now,
                        scope: "fleet".to_string(),
                        metric: "fleet.block_total_found".to_string(),
                        value: summary.total_found as f64,
                        labels: None,
                    });
                }

                if let Ok(jobs) = prune_state.db.get_search_jobs().await {
                    let active = jobs.iter().filter(|j| j.status == "running").count();
                    samples.push(db::MetricSample {
                        ts: now,
                        scope: "fleet".to_string(),
                        metric: "fleet.search_jobs_active".to_string(),
                        value: active as f64,
                        labels: None,
                    });
                }

                if let Ok(jobs) = prune_state.db.get_recent_search_jobs(24, 50).await {
                    for job in jobs {
                        let summary = match prune_state.db.get_job_block_summary(job.id).await {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let total_blocks = summary.available
                            + summary.claimed
                            + summary.completed
                            + summary.failed;
                        let completion_pct = if total_blocks > 0 {
                            (summary.completed as f64 / total_blocks as f64) * 100.0
                        } else {
                            0.0
                        };
                        let labels = serde_json::json!({
                            "job_id": job.id.to_string(),
                            "search_type": job.search_type,
                            "status": job.status,
                        });

                        samples.push(db::MetricSample {
                            ts: now,
                            scope: "search_job".to_string(),
                            metric: "search_job.blocks_available".to_string(),
                            value: summary.available as f64,
                            labels: Some(labels.clone()),
                        });
                        samples.push(db::MetricSample {
                            ts: now,
                            scope: "search_job".to_string(),
                            metric: "search_job.blocks_claimed".to_string(),
                            value: summary.claimed as f64,
                            labels: Some(labels.clone()),
                        });
                        samples.push(db::MetricSample {
                            ts: now,
                            scope: "search_job".to_string(),
                            metric: "search_job.blocks_completed".to_string(),
                            value: summary.completed as f64,
                            labels: Some(labels.clone()),
                        });
                        samples.push(db::MetricSample {
                            ts: now,
                            scope: "search_job".to_string(),
                            metric: "search_job.blocks_failed".to_string(),
                            value: summary.failed as f64,
                            labels: Some(labels.clone()),
                        });
                        samples.push(db::MetricSample {
                            ts: now,
                            scope: "search_job".to_string(),
                            metric: "search_job.completion_pct".to_string(),
                            value: completion_pct,
                            labels: Some(labels.clone()),
                        });
                        samples.push(db::MetricSample {
                            ts: now,
                            scope: "search_job".to_string(),
                            metric: "search_job.total_tested".to_string(),
                            value: summary.total_tested as f64,
                            labels: Some(labels.clone()),
                        });
                        samples.push(db::MetricSample {
                            ts: now,
                            scope: "search_job".to_string(),
                            metric: "search_job.total_found".to_string(),
                            value: summary.total_found as f64,
                            labels: Some(labels.clone()),
                        });
                    }
                }

                let error_count = *event_counts.get("error").unwrap_or(&0) as f64;
                let warning_count = *event_counts.get("warning").unwrap_or(&0) as f64;
                let prime_count = *event_counts.get("prime").unwrap_or(&0) as f64;
                let milestone_count = *event_counts.get("milestone").unwrap_or(&0) as f64;
                let search_start_count = *event_counts.get("search_start").unwrap_or(&0) as f64;
                let search_done_count = *event_counts.get("search_done").unwrap_or(&0) as f64;
                let total_events: f64 = event_counts.values().copied().sum::<i64>() as f64;
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "events".to_string(),
                    metric: "events.total_count".to_string(),
                    value: total_events,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "events".to_string(),
                    metric: "events.error_count".to_string(),
                    value: error_count,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "events".to_string(),
                    metric: "events.warning_count".to_string(),
                    value: warning_count,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "events".to_string(),
                    metric: "events.prime_count".to_string(),
                    value: prime_count,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "events".to_string(),
                    metric: "events.milestone_count".to_string(),
                    value: milestone_count,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "events".to_string(),
                    metric: "events.search_start_count".to_string(),
                    value: search_start_count,
                    labels: None,
                });
                samples.push(db::MetricSample {
                    ts: now,
                    scope: "events".to_string(),
                    metric: "events.search_done_count".to_string(),
                    value: search_done_count,
                    labels: None,
                });
                event_counts.clear();

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
                    warn!(error = %e, count = samples.len(), "failed to persist metric samples");
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
                    warn!(error = %e, "failed to roll up hourly metrics");
                }
                let day_start = now
                    .with_hour(0)
                    .and_then(|t| t.with_minute(0))
                    .and_then(|t| t.with_second(0))
                    .and_then(|t| t.with_nanosecond(0))
                    .unwrap_or(now);
                let prev_day = day_start - chrono::Duration::days(1);
                if let Err(e) = prune_state.db.rollup_metrics_day(prev_day).await {
                    warn!(error = %e, "failed to roll up daily metrics");
                }
                if let Err(e) = prune_state
                    .db
                    .prune_metric_samples(metric_retention_days)
                    .await
                {
                    warn!(error = %e, "failed to prune metric samples");
                }
                if let Err(e) = prune_state
                    .db
                    .prune_metric_rollups(rollup_retention_days)
                    .await
                {
                    warn!(error = %e, "failed to prune metric rollups");
                }
                if let Err(e) = prune_state
                    .db
                    .prune_metric_rollups_daily(daily_rollup_retention_days)
                    .await
                {
                    warn!(error = %e, "failed to prune daily rollups");
                }
                if let Err(e) = prune_state.db.prune_system_logs(log_retention_days).await {
                    warn!(error = %e, "failed to prune system logs");
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
                    warn!(error = %e, "auto-verify: failed to fetch unverified primes");
                    continue;
                }
            };
            if primes.is_empty() {
                continue;
            }
            info!(count = primes.len(), "auto-verify: checking primes");
            for prime in &primes {
                let prime_clone = prime.clone();
                let result =
                    tokio::task::spawn_blocking(move || verify::verify_prime(&prime_clone)).await;
                match result {
                    Ok(verify::VerifyResult::Verified { method, tier }) => {
                        info!(prime_id = prime.id, expression = %prime.expression, method = %method, tier, "auto-verified prime");
                        if let Err(e) = verify_state
                            .db
                            .mark_verified(prime.id, &method, tier as i16)
                            .await
                        {
                            warn!(prime_id = prime.id, error = %e, "failed to mark prime verified");
                        }
                    }
                    Ok(verify::VerifyResult::Failed { reason }) => {
                        warn!(prime_id = prime.id, reason = %reason, "auto-verify failed");
                        if let Err(e) = verify_state
                            .db
                            .mark_verification_failed(prime.id, &reason)
                            .await
                        {
                            warn!(prime_id = prime.id, error = %e, "failed to mark prime verification failed");
                        }
                    }
                    Ok(verify::VerifyResult::Skipped { reason }) => {
                        tracing::debug!(prime_id = prime.id, reason = %reason, "auto-verify skipped");
                    }
                    Err(e) => {
                        warn!(prime_id = prime.id, error = %e, "auto-verify task panicked");
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
                    warn!(task_id = c.task_id, error = %e, "agent: failed to complete task");
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
                info!(task_id = c.task_id, status = status_str, tokens, cost, "agent task finished");
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
                                        info!(parent_id, cancelled, "agent: cancelled pending siblings");
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
                            info!(parent_id, status = %parent.status, "agent: parent task auto-completed");
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
                    warn!(count = killed.len(), task_ids = ?killed, "agent: global budget exceeded, killed agents");
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
                    warn!(error = %e, "agent: failed to claim task");
                    continue;
                }
            };
            info!(task_id = task.id, title = %task.title, priority = task.priority, model = ?task.agent_model, "agent: claimed task");
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
                    warn!(task_id = task.id, error = %e, "agent: failed to spawn");
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
        info!("records: initial refresh from t5k.org");
        match project::refresh_all_records(&records_state.db).await {
            Ok(n) => info!(count = n, "records: refreshed forms"),
            Err(e) => warn!(error = %e, "records: refresh failed"),
        }
        let mut interval = tokio::time::interval(Duration::from_secs(24 * 3600));
        interval.tick().await;
        loop {
            interval.tick().await;
            info!("records: 24h refresh from t5k.org");
            match project::refresh_all_records(&records_state.db).await {
                Ok(n) => info!(count = n, "records: refreshed forms"),
                Err(e) => warn!(error = %e, "records: refresh failed"),
            }
        }
    });

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    info!(port, "dashboard running");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    info!("dashboard shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        tokio::select! { _ = ctrl_c => info!("received SIGINT, shutting down"), _ = sigterm.recv() => info!("received SIGTERM, shutting down") }
    }
    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
        info!("received SIGINT, shutting down");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path_preserves_api_routes() {
        assert_eq!(normalize_path("/api/status"), "/api/status");
        assert_eq!(normalize_path("/api/fleet"), "/api/fleet");
        assert_eq!(normalize_path("/metrics"), "/metrics");
    }

    #[test]
    fn normalize_path_collapses_numeric_ids() {
        assert_eq!(normalize_path("/api/search_jobs/42"), "/api/search_jobs/:id");
        assert_eq!(normalize_path("/api/primes/12345/verify"), "/api/primes/:id/verify");
    }

    #[test]
    fn normalize_path_collapses_uuids() {
        assert_eq!(
            normalize_path("/api/agents/tasks/550e8400-e29b-41d4-a716-446655440000"),
            "/api/agents/tasks/:uuid"
        );
    }

    #[test]
    fn normalize_path_handles_empty_and_root() {
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path(""), "");
    }
}
