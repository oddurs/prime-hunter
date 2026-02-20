//! Worker release management endpoints (operator control plane).

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

use super::AppState;

#[derive(Deserialize)]
pub(super) struct ListQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    50
}

pub(super) async fn handler_releases_list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let limit = query.limit.clamp(1, 200);
    let releases = match state.db.list_worker_releases(limit).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("failed to list releases: {}", e)})),
            );
        }
    };
    let channels = match state.db.list_worker_release_channels().await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("failed to list channels: {}", e)})),
            );
        }
    };
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "releases": releases,
            "channels": channels,
        })),
    )
}

#[derive(Deserialize)]
pub(super) struct EventsQuery {
    #[serde(default)]
    channel: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
}

pub(super) async fn handler_releases_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<EventsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.clamp(1, 500);
    match state
        .db
        .list_worker_release_events(query.channel.as_deref(), limit)
        .await
    {
        Ok(events) => (
            StatusCode::OK,
            Json(serde_json::json!({ "events": events })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("failed to list events: {}", e)})),
        ),
    }
}

#[derive(Deserialize)]
pub(super) struct HealthQuery {
    #[serde(default = "default_active_hours")]
    active_hours: i64,
}

fn default_active_hours() -> i64 {
    24
}

pub(super) async fn handler_releases_health(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HealthQuery>,
) -> impl IntoResponse {
    let active_hours = query.active_hours.clamp(1, 24 * 30);
    let adoption = match state.db.worker_release_adoption(active_hours).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("failed to fetch adoption: {}", e)})),
            );
        }
    };
    let channels = match state.db.list_worker_release_channels().await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("failed to list channels: {}", e)})),
            );
        }
    };
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "active_hours": active_hours,
            "adoption": adoption,
            "channels": channels,
        })),
    )
}

#[derive(Deserialize)]
pub(super) struct UpsertReleasePayload {
    version: String,
    artifacts: serde_json::Value,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    published_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub(super) async fn handler_releases_upsert(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpsertReleasePayload>,
) -> impl IntoResponse {
    if payload.version.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "version is required"})),
        );
    }
    if !payload.artifacts.is_array() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "artifacts must be a JSON array"})),
        );
    }
    match state
        .db
        .upsert_worker_release(
            &payload.version,
            &payload.artifacts,
            payload.notes.as_deref(),
            payload.published_at,
        )
        .await
    {
        Ok(row) => (StatusCode::OK, Json(serde_json::json!({ "release": row }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("failed to upsert release: {}", e)})),
        ),
    }
}

#[derive(Deserialize)]
pub(super) struct RolloutPayload {
    channel: String,
    version: String,
    #[serde(default = "default_rollout")]
    rollout_percent: i32,
    #[serde(default)]
    changed_by: Option<String>,
}

fn default_rollout() -> i32 {
    100
}

pub(super) async fn handler_releases_rollout(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RolloutPayload>,
) -> impl IntoResponse {
    if payload.channel.trim().is_empty() || payload.version.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "channel and version are required"})),
        );
    }
    match state
        .db
        .set_worker_release_channel(
            &payload.channel,
            &payload.version,
            payload.rollout_percent,
            payload.changed_by.as_deref(),
        )
        .await
    {
        Ok(row) => (StatusCode::OK, Json(serde_json::json!({ "channel": row }))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

#[derive(Deserialize)]
pub(super) struct RollbackPayload {
    channel: String,
    #[serde(default)]
    changed_by: Option<String>,
}

pub(super) async fn handler_releases_rollback(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RollbackPayload>,
) -> impl IntoResponse {
    if payload.channel.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "channel is required"})),
        );
    }
    match state
        .db
        .rollback_worker_release_channel(&payload.channel, payload.changed_by.as_deref())
        .await
    {
        Ok(row) => (StatusCode::OK, Json(serde_json::json!({ "channel": row }))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}
