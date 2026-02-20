//! Event bus and notification endpoints.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;

use super::AppState;

pub(super) async fn handler_api_notifications(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let notifications = state.event_bus.recent_notifications(50);
    Json(serde_json::json!({ "notifications": notifications }))
}

pub(super) async fn handler_api_events(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let events = state.event_bus.recent_events(200);
    Json(serde_json::json!({ "events": events }))
}
