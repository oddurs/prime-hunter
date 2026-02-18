use crate::{checkpoint, db};
use anyhow::Result;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower_http::services::ServeDir;

struct AppState {
    db: Arc<Mutex<db::Database>>,
    checkpoint_path: PathBuf,
}

pub fn run(
    port: u16,
    db_path: &Path,
    checkpoint_path: &Path,
    static_dir: Option<&Path>,
) -> Result<()> {
    let database = db::Database::open(db_path)?;
    let state = Arc::new(AppState {
        db: Arc::new(Mutex::new(database)),
        checkpoint_path: checkpoint_path.to_path_buf(),
    });

    let mut app = Router::new()
        .route("/ws", get(handler_ws))
        .route("/api/stats", get(handler_api_stats))
        .route("/api/primes", get(handler_api_primes))
        .route("/api/status", get(handler_api_status));

    if let Some(dir) = static_dir {
        app = app.fallback_service(ServeDir::new(dir).append_index_html_on_directories(true));
    } else {
        app = app.route("/", get(handler_index));
    }

    let app = app.with_state(state);

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        eprintln!("Dashboard running at http://localhost:{}", port);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    })
}

async fn handler_index() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("dashboard.html"),
    )
}

async fn handler_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| ws_loop(socket, state))
}

async fn ws_loop(mut socket: WebSocket, state: Arc<AppState>) {
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
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(req) = serde_json::from_str::<WsRequest>(&text) {
                            if req.action == "get_primes" {
                                if let Some(resp) = build_primes_response(
                                    &state, req.offset, req.limit
                                ).await {
                                    if socket.send(Message::Text(resp.into())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct WsRequest {
    action: String,
    #[serde(default)]
    offset: Option<i64>,
    #[serde(default)]
    limit: Option<i64>,
}

async fn build_update(state: &Arc<AppState>) -> Option<String> {
    let state = state.clone();
    tokio::task::spawn_blocking(move || {
        let db = state.db.lock().unwrap();
        let stats = db.get_stats().ok()?;
        let total = db.get_total_count().unwrap_or(0);
        let primes = db.get_primes(50, 0).ok()?;
        drop(db);
        let cp = checkpoint::load(&state.checkpoint_path);
        let status = StatusResponse {
            active: cp.is_some(),
            checkpoint: cp.and_then(|c| serde_json::to_value(&c).ok()),
        };
        serde_json::to_string(&serde_json::json!({
            "type": "update",
            "stats": stats,
            "primes": {
                "primes": primes,
                "total": total,
                "limit": 50,
                "offset": 0,
            },
            "status": status,
        }))
        .ok()
    })
    .await
    .ok()?
}

async fn build_primes_response(
    state: &Arc<AppState>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Option<String> {
    let state = state.clone();
    tokio::task::spawn_blocking(move || {
        let limit = limit.unwrap_or(50).min(500);
        let offset = offset.unwrap_or(0);
        let db = state.db.lock().unwrap();
        let total = db.get_total_count().unwrap_or(0);
        let primes = db.get_primes(limit, offset).ok()?;
        serde_json::to_string(&serde_json::json!({
            "type": "primes",
            "primes": primes,
            "total": total,
            "limit": limit,
            "offset": offset,
        }))
        .ok()
    })
    .await
    .ok()?
}

// --- REST endpoints (kept for compatibility) ---

async fn handler_api_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().unwrap();
    match db.get_stats() {
        Ok(stats) => Json(serde_json::json!(stats)).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct PrimesQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn handler_api_primes(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PrimesQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(500);
    let offset = params.offset.unwrap_or(0);
    let db = state.db.lock().unwrap();
    let total = db.get_total_count().unwrap_or(0);
    match db.get_primes(limit, offset) {
        Ok(primes) => Json(serde_json::json!({
            "primes": primes,
            "total": total,
            "limit": limit,
            "offset": offset,
        }))
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
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
