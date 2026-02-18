use crate::{checkpoint, db, deploy, events, fleet, metrics, search_manager, verify};
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

struct AppState {
    db: db::Database,
    database_url: String,
    checkpoint_path: PathBuf,
    fleet: Mutex<fleet::Fleet>,
    searches: Mutex<search_manager::SearchManager>,
    deployments: Mutex<deploy::DeploymentManager>,
    coordinator_metrics: Mutex<Option<metrics::HardwareMetrics>>,
    event_bus: events::EventBus,
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
}

pub async fn run(
    port: u16,
    database_url: &str,
    checkpoint_path: &Path,
    static_dir: Option<&Path>,
) -> Result<()> {
    let database = db::Database::connect(database_url).await?;
    let (ws_tx, _) = tokio::sync::broadcast::channel::<String>(256);
    let event_bus = events::EventBus::new();
    event_bus.set_ws_sender(ws_tx.clone());
    let state = Arc::new(AppState {
        db: database,
        database_url: database_url.to_string(),
        checkpoint_path: checkpoint_path.to_path_buf(),
        fleet: Mutex::new(fleet::Fleet::new()),
        searches: Mutex::new(search_manager::SearchManager::new(port, database_url)),
        deployments: Mutex::new(deploy::DeploymentManager::new()),
        coordinator_metrics: Mutex::new(None),
        event_bus,
    });

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
        .route("/api/events", get(handler_api_events));

    if let Some(dir) = static_dir {
        app = app.fallback_service(ServeDir::new(dir).append_index_html_on_directories(true));
    } else {
        app = app.route("/", get(handler_index));
    }

    let app = app
        .layer(
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
        .with_state(state.clone());

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
    serde_json::to_string(&serde_json::json!({
        "type": "update",
        "status": status,
        "fleet": fleet_data,
        "searches": searches,
        "search_jobs": search_jobs,
        "deployments": deployments,
        "coordinator": coord_metrics,
        "notifications": recent_notifications,
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
