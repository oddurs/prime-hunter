//! Search job API — PG-based block coordination for distributed searches.

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

use super::AppState;

#[derive(Deserialize)]
pub(super) struct CreateSearchJobPayload {
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

pub(super) async fn handler_api_search_jobs_list(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_search_jobs().await {
        Ok(jobs) => Json(serde_json::json!({ "search_jobs": jobs })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub(super) async fn handler_api_search_jobs_create(
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
                Json(serde_json::json!({"id": job_id, "blocks": num_blocks})),
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

pub(super) async fn handler_api_search_job_get(
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
    Json(serde_json::json!({"job": job, "blocks": summary})).into_response()
}

pub(super) async fn handler_api_search_job_cancel(
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
