//! Operator public API — v1 REST endpoints for the operator computing platform.
//!
//! Implements the public work API that operators interact with:
//! registration, node management, work claiming, result submission,
//! personal stats, and the leaderboard.
//!
//! All endpoints except `/api/v1/operators/register` and `/api/v1/operators/leaderboard`
//! require `Authorization: Bearer <api_key>` authentication.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::middleware_auth::RequireAuth;
use super::AppState;
use crate::db::operators::{OperatorRow, WorkerCapabilities};

// ── GET /api/volunteer/worker/latest ─────────────────────────────

#[derive(Deserialize)]
pub(super) struct LatestWorkerQuery {
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    worker_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WorkerReleaseManifest {
    channels: std::collections::HashMap<String, WorkerReleaseChannel>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WorkerReleaseChannel {
    version: String,
    published_at: String,
    artifacts: Vec<WorkerReleaseArtifact>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WorkerReleaseArtifact {
    os: String,
    arch: String,
    url: String,
    sha256: String,
}

fn worker_manifest_path() -> std::path::PathBuf {
    std::env::var("DARKREACH_WORKER_RELEASE_MANIFEST")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("deploy/releases/worker-manifest.json"))
}

fn load_worker_manifest() -> anyhow::Result<WorkerReleaseManifest> {
    let path = worker_manifest_path();
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path.display(), e))?;
    let parsed: WorkerReleaseManifest = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {}", path.display(), e))?;
    Ok(parsed)
}

pub(super) async fn handler_worker_latest(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LatestWorkerQuery>,
) -> impl IntoResponse {
    let channel = query.channel.unwrap_or_else(|| "stable".to_string());
    if let Ok(Some(row)) = state
        .db
        .resolve_worker_release_for_channel(&channel, query.worker_id.as_deref())
        .await
    {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "channel": channel,
                "version": row.version,
                "published_at": row.published_at,
                "notes": row.notes,
                "artifacts": row.artifacts,
            })),
        );
    }

    let manifest = match load_worker_manifest() {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "Worker release manifest unavailable",
                    "detail": e.to_string(),
                })),
            );
        }
    };

    let Some(release) = manifest.channels.get(&channel) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!("Unknown release channel: {}", channel),
            })),
        );
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "channel": channel,
            "version": release.version,
            "published_at": release.published_at,
            "notes": release.notes,
            "artifacts": release.artifacts,
        })),
    )
}

// ── Authentication ────────────────────────────────────────────────

/// Extract and validate the API key from the Authorization header.
/// Returns the volunteer record if valid, or an error response.
async fn authenticate(
    state: &Arc<AppState>,
    headers: &HeaderMap,
) -> Result<OperatorRow, (StatusCode, Json<serde_json::Value>)> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let api_key = auth.strip_prefix("Bearer ").unwrap_or("");
    if api_key.is_empty() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Missing or invalid Authorization header"})),
        ));
    }

    match state.db.get_operator_by_api_key(api_key).await {
        Ok(Some(vol)) => {
            // Update last_seen
            let _ = state.db.touch_operator(vol.id).await;
            Ok(vol)
        }
        Ok(None) => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Invalid API key"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Database error: {}", e)})),
        )),
    }
}

// ── POST /api/v1/register ─────────────────────────────────────────

#[derive(Deserialize)]
pub(super) struct RegisterPayload {
    username: String,
    email: String,
}

pub(super) async fn handler_v1_register(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterPayload>,
) -> impl IntoResponse {
    // Basic validation
    if payload.username.len() < 3 || payload.username.len() > 32 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Username must be 3-32 characters"})),
        );
    }
    if !payload.email.contains('@') {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid email address"})),
        );
    }

    match state
        .db
        .register_operator(&payload.username, &payload.email)
        .await
    {
        Ok(vol) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "api_key": vol.api_key,
                "username": vol.username,
            })),
        ),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("duplicate key") || msg.contains("unique constraint") {
                (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({"error": "Username or email already registered"})),
                )
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("Registration failed: {}", e)})),
                )
            }
        }
    }
}

// ── POST /api/v1/worker/register ──────────────────────────────────

#[derive(Deserialize)]
pub(super) struct WorkerRegisterPayload {
    worker_id: String,
    hostname: String,
    cores: i32,
    #[serde(default)]
    cpu_model: String,
    #[serde(default)]
    os: Option<String>,
    #[serde(default)]
    arch: Option<String>,
    #[serde(default)]
    ram_gb: Option<i32>,
    #[serde(default)]
    has_gpu: Option<bool>,
    #[serde(default)]
    gpu_model: Option<String>,
    #[serde(default)]
    gpu_vram_gb: Option<i32>,
    #[serde(default)]
    worker_version: Option<String>,
    #[serde(default)]
    update_channel: Option<String>,
}

pub(super) async fn handler_v1_worker_register(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<WorkerRegisterPayload>,
) -> impl IntoResponse {
    let vol = match authenticate(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };

    match state
        .db
        .register_operator_node(
            vol.id,
            &payload.worker_id,
            &payload.hostname,
            payload.cores,
            &payload.cpu_model,
            payload.os.as_deref(),
            payload.arch.as_deref(),
            payload.ram_gb,
            payload.has_gpu,
            payload.gpu_model.as_deref(),
            payload.gpu_vram_gb,
            payload.worker_version.as_deref(),
            payload.update_channel.as_deref(),
        )
        .await
    {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Worker registration failed: {}", e)})),
        ),
    }
}

// ── POST /api/v1/worker/heartbeat ─────────────────────────────────

#[derive(Deserialize)]
pub(super) struct WorkerHeartbeatPayload {
    worker_id: String,
}

pub(super) async fn handler_v1_worker_heartbeat(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<WorkerHeartbeatPayload>,
) -> impl IntoResponse {
    let _vol = match authenticate(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };

    let start = std::time::Instant::now();
    let result = state
        .db
        .operator_node_heartbeat(&payload.worker_id)
        .await;
    let rtt = start.elapsed().as_secs_f64();
    state.prom_metrics.heartbeat_rtt.observe(rtt);
    state
        .prom_metrics
        .db_query_duration
        .get_or_create(&crate::prom_metrics::QueryLabel {
            query: "worker_heartbeat".to_string(),
        })
        .observe(rtt);

    match result {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Heartbeat failed: {}", e)})),
        ),
    }
}

// ── GET /api/v1/work ──────────────────────────────────────────────

#[derive(Deserialize)]
pub(super) struct WorkQuery {
    #[serde(default)]
    cores: Option<usize>,
    #[serde(default)]
    ram_gb: Option<u64>,
    #[serde(default)]
    has_gpu: Option<bool>,
    #[serde(default)]
    os: Option<String>,
    #[serde(default)]
    arch: Option<String>,
}

pub(super) async fn handler_v1_work(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<WorkQuery>,
) -> impl IntoResponse {
    let vol = match authenticate(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };

    let caps = WorkerCapabilities {
        cores: query.cores.unwrap_or(1).clamp(1, i32::MAX as usize) as i32,
        ram_gb: query.ram_gb.unwrap_or(0).min(i32::MAX as u64) as i32,
        has_gpu: query.has_gpu.unwrap_or(false),
        os: query.os.filter(|v| !v.trim().is_empty()),
        arch: query.arch.filter(|v| !v.trim().is_empty()),
    };

    let claim_start = std::time::Instant::now();
    let claim_result = state.db.claim_operator_block(vol.id, &caps).await;
    state
        .prom_metrics
        .db_query_duration
        .get_or_create(&crate::prom_metrics::QueryLabel {
            query: "claim_work_block".to_string(),
        })
        .observe(claim_start.elapsed().as_secs_f64());
    match claim_result {
        Ok(Some(block)) => {
            // Set quorum based on volunteer trust level and search form
            if let Some(ref search_type) = block.search_type {
                let trust = state.db.get_operator_trust(vol.id).await.ok().flatten();
                let trust_level = trust.map(|t| t.trust_level).unwrap_or(1);
                let quorum = crate::verify::required_quorum(trust_level, search_type);
                let _ = state.db.set_block_quorum(block.block_id, quorum).await;
            }

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "block_id": block.block_id,
                    "search_job_id": block.search_job_id,
                    "search_type": block.search_type,
                    "params": block.params,
                    "block_start": block.block_start,
                    "block_end": block.block_end,
                })),
            )
        }
        Ok(None) => (StatusCode::NO_CONTENT, Json(serde_json::json!(null))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Work claim failed: {}", e)})),
        ),
    }
}

// ── POST /api/v1/result ───────────────────────────────────────────

#[derive(Deserialize)]
pub(super) struct ResultPayload {
    block_id: i32,
    tested: i64,
    found: i64,
    #[serde(default)]
    primes: Vec<PrimeReportPayload>,
}

#[derive(Deserialize)]
pub(super) struct PrimeReportPayload {
    expression: String,
    form: String,
    digits: u64,
    proof_method: String,
    #[serde(default)]
    #[allow(dead_code)]
    certificate: Option<String>,
}

pub(super) async fn handler_v1_result(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ResultPayload>,
) -> impl IntoResponse {
    let vol = match authenticate(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };

    // Complete the work block and record duration histogram
    let block_timing = match state
        .db
        .submit_operator_result(payload.block_id, payload.tested, payload.found)
        .await
    {
        Ok(timing) => timing,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Result submission failed: {}", e)})),
            );
        }
    };
    if let Some((duration_secs, search_type)) = block_timing {
        state
            .prom_metrics
            .work_block_duration
            .get_or_create(&crate::prom_metrics::FormLabel {
                form: search_type,
            })
            .observe(duration_secs);
    }

    // Record any discovered primes
    for prime in &payload.primes {
        let insert_start = std::time::Instant::now();
        let insert_result = state
            .db
            .insert_prime_ignore(
                &prime.form,
                &prime.expression,
                prime.digits,
                "",
                &prime.proof_method,
            )
            .await;
        state
            .prom_metrics
            .db_query_duration
            .get_or_create(&crate::prom_metrics::QueryLabel {
                query: "insert_prime".to_string(),
            })
            .observe(insert_start.elapsed().as_secs_f64());
        match insert_result {
            Ok(_) => {
                let _ = state.db.increment_operator_primes(vol.id).await;
                // Bonus credit for discoveries (10x block credit)
                let _ = state
                    .db
                    .grant_credit(vol.id, payload.block_id, 1000, "prime_discovered")
                    .await;
            }
            Err(e) => {
                tracing::warn!(
                    expression = %prime.expression,
                    error = %e,
                    "failed to insert operator prime"
                );
            }
        }
    }

    // Grant base credit for block completion (wall_seconds approximation)
    let credit = payload.tested.max(1);
    let _ = state
        .db
        .grant_credit(vol.id, payload.block_id, credit, "block_completed")
        .await;

    // Record valid result for trust scoring
    let _ = state.db.record_valid_result(vol.id).await;

    (StatusCode::OK, Json(serde_json::json!({"ok": true})))
}

// ── GET /api/v1/stats ─────────────────────────────────────────────

pub(super) async fn handler_v1_stats(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let vol = match authenticate(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };

    match state.db.get_operator_stats(vol.id).await {
        Ok(Some(stats)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "username": stats.username,
                "credit": stats.credit,
                "primes_found": stats.primes_found,
                "trust_level": stats.trust_level,
                "rank": stats.rank,
            })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Stats not found"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Stats query failed: {}", e)})),
        ),
    }
}

// ── GET /api/v1/leaderboard ───────────────────────────────────────

pub(super) async fn handler_v1_leaderboard(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_operator_leaderboard(100).await {
        Ok(entries) => {
            let result: Vec<serde_json::Value> = entries
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    serde_json::json!({
                        "rank": i + 1,
                        "username": e.username,
                        "team": e.team,
                        "credit": e.credit,
                        "primes_found": e.primes_found,
                        "worker_count": e.worker_count,
                    })
                })
                .collect();
            (StatusCode::OK, Json(serde_json::json!(result)))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Leaderboard query failed: {}", e)})),
        ),
    }
}

// ── GET /api/v1/operators/me/nodes ─────────────────────────────────

/// Extract the operator UUID from the authenticated user's profile.
async fn get_operator_uuid(
    state: &Arc<AppState>,
    user_id: &str,
) -> Result<uuid::Uuid, (StatusCode, Json<serde_json::Value>)> {
    match state.db.get_user_profile(user_id).await {
        Ok(Some(profile)) => profile.operator_id.ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "No operator account linked"})),
            )
        }),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "User profile not found"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Database error: {}", e)})),
        )),
    }
}

/// Get nodes belonging to the authenticated operator (JWT auth).
pub(super) async fn handler_v1_operator_nodes(
    State(state): State<Arc<AppState>>,
    RequireAuth(auth_user): RequireAuth,
) -> impl IntoResponse {
    let operator_id = match get_operator_uuid(&state, &auth_user.user_id).await {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match state.db.get_operator_nodes(operator_id).await {
        Ok(nodes) => (StatusCode::OK, Json(serde_json::json!(nodes))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to fetch nodes: {}", e)})),
        ),
    }
}

// ── POST /api/v1/operators/rotate-key ──────────────────────────────

/// Rotate the operator's API key (requires JWT auth).
pub(super) async fn handler_v1_rotate_key(
    State(state): State<Arc<AppState>>,
    RequireAuth(auth_user): RequireAuth,
) -> impl IntoResponse {
    let operator_id = match get_operator_uuid(&state, &auth_user.user_id).await {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match state.db.rotate_operator_api_key(operator_id).await {
        Ok(new_key) => (
            StatusCode::OK,
            Json(serde_json::json!({"api_key": new_key})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to rotate key: {}", e)})),
        ),
    }
}
