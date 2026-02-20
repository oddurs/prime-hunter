//! Search management API — start, stop, pause, resume managed searches.

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;

use super::{lock_or_recover, AppState};
use crate::search_manager;

pub(super) async fn handler_api_searches_list(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let searches = lock_or_recover(&state.searches).get_all();
    Json(serde_json::json!({ "searches": searches }))
}

pub(super) async fn handler_api_searches_create(
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

pub(super) async fn handler_api_searches_get(
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

pub(super) async fn handler_api_searches_stop(
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

pub(super) async fn handler_api_searches_pause(
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

pub(super) async fn handler_api_searches_resume(
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
