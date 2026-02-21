//! Search management API — create, list, stop, pause, resume searches via PostgreSQL.
//!
//! Searches are managed as `search_jobs` + `work_blocks` in the database.
//! Creating a search creates a job row and generates work blocks for the range.
//! Nodes claim blocks directly via `FOR UPDATE SKIP LOCKED`.

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;
use tracing::info;

use super::AppState;
use crate::search_params::SearchParams;

pub(super) async fn handler_api_searches_list(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_search_jobs().await {
        Ok(jobs) => Json(serde_json::json!({ "searches": jobs })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to list searches: {}", e)})),
        )
            .into_response(),
    }
}

pub(super) async fn handler_api_searches_create(
    State(state): State<Arc<AppState>>,
    Json(params): Json<SearchParams>,
) -> impl IntoResponse {
    let search_type = params.search_type_name().to_string();
    let params_json = match serde_json::to_value(&params) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid params: {}", e)})),
            )
                .into_response()
        }
    };

    let (range_start, range_end) = params.range();
    let block_size = params.default_block_size();

    match state
        .db
        .create_search_job(&search_type, &params_json, range_start, range_end, block_size)
        .await
    {
        Ok(job_id) => {
            info!(
                job_id,
                search_type,
                range_start,
                range_end,
                block_size,
                "search job created"
            );
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "id": job_id,
                    "search_type": search_type,
                    "status": "running",
                    "range_start": range_start,
                    "range_end": range_end,
                    "block_size": block_size,
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to create search: {}", e)})),
        )
            .into_response(),
    }
}

pub(super) async fn handler_api_searches_get(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> impl IntoResponse {
    match state.db.get_search_job(id).await {
        Ok(Some(job)) => Json(serde_json::json!(job)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Search not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Query failed: {}", e)})),
        )
            .into_response(),
    }
}

pub(super) async fn handler_api_searches_stop(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> impl IntoResponse {
    match state
        .db
        .update_search_job_status(id, "cancelled", Some("Cancelled via API"))
        .await
    {
        Ok(()) => {
            info!(id, "search job cancelled");
            Json(serde_json::json!({"ok": true, "id": id, "status": "cancelled"})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to cancel search: {}", e)})),
        )
            .into_response(),
    }
}

pub(super) async fn handler_api_searches_pause(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> impl IntoResponse {
    match state
        .db
        .update_search_job_status(id, "paused", Some("Paused via API"))
        .await
    {
        Ok(()) => {
            info!(id, "search job paused");
            Json(serde_json::json!({"ok": true, "id": id, "status": "paused"})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to pause search: {}", e)})),
        )
            .into_response(),
    }
}

pub(super) async fn handler_api_searches_resume(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> impl IntoResponse {
    match state
        .db
        .update_search_job_status(id, "running", Some("Resumed via API"))
        .await
    {
        Ok(()) => {
            info!(id, "search job resumed");
            Json(serde_json::json!({"ok": true, "id": id, "status": "running"})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to resume search: {}", e)})),
        )
            .into_response(),
    }
}
