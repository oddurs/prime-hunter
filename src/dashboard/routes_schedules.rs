//! # Agent Schedule REST API
//!
//! CRUD endpoints for agent schedules, replacing direct Supabase table access.
//! Part of Phase 6: Frontend Independence.
//!
//! | Endpoint | Replaces |
//! |----------|----------|
//! | `GET /api/schedules` | `supabase.from("agent_schedules").select()` |
//! | `POST /api/schedules` | `supabase.from("agent_schedules").insert()` |
//! | `PUT /api/schedules/{id}` | `supabase.from("agent_schedules").update()` |
//! | `PUT /api/schedules/{id}/toggle` | `supabase.from("agent_schedules").update({enabled})` |
//! | `DELETE /api/schedules/{id}` | `supabase.from("agent_schedules").delete()` |

use super::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

/// `GET /api/schedules` — List all agent schedules.
///
/// Replaces `supabase.from("agent_schedules").select("*").order("name")`.
pub(super) async fn handler_api_schedules_list(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_agent_schedules().await {
        Ok(schedules) => Json(serde_json::json!({ "schedules": schedules })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub(super) struct CreateSchedulePayload {
    name: String,
    #[serde(default)]
    description: String,
    trigger_type: String,
    cron_expr: Option<String>,
    event_filter: Option<String>,
    #[serde(default = "default_action_type")]
    action_type: String,
    template_name: Option<String>,
    role_name: Option<String>,
    task_title: String,
    #[serde(default)]
    task_description: String,
    #[serde(default = "default_priority")]
    priority: String,
    max_cost_usd: Option<f64>,
    #[serde(default = "default_permission_level")]
    permission_level: i32,
}

fn default_action_type() -> String {
    "task".to_string()
}

fn default_priority() -> String {
    "normal".to_string()
}

fn default_permission_level() -> i32 {
    1
}

/// `POST /api/schedules` — Create a new agent schedule.
///
/// Replaces `supabase.from("agent_schedules").insert({...}).select().single()`.
pub(super) async fn handler_api_schedules_create(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateSchedulePayload>,
) -> impl IntoResponse {
    match state
        .db
        .create_agent_schedule(
            &payload.name,
            &payload.description,
            &payload.trigger_type,
            payload.cron_expr.as_deref(),
            payload.event_filter.as_deref(),
            &payload.action_type,
            payload.template_name.as_deref(),
            payload.role_name.as_deref(),
            &payload.task_title,
            &payload.task_description,
            &payload.priority,
            payload.max_cost_usd,
            payload.permission_level,
        )
        .await
    {
        Ok(schedule) => (StatusCode::CREATED, Json(serde_json::json!(schedule))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// `PUT /api/schedules/{id}` — Update an existing schedule.
///
/// Replaces `supabase.from("agent_schedules").update({...}).eq("id", id).select().single()`.
/// Accepts a partial JSON object with only the fields to update.
pub(super) async fn handler_api_schedules_update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(updates): Json<serde_json::Value>,
) -> impl IntoResponse {
    match state.db.update_agent_schedule(id, &updates).await {
        Ok(Some(schedule)) => Json(serde_json::json!(schedule)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Schedule not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub(super) struct TogglePayload {
    enabled: bool,
}

/// `PUT /api/schedules/{id}/toggle` — Toggle schedule enabled/disabled.
///
/// Replaces `supabase.from("agent_schedules").update({enabled}).eq("id", id)`.
pub(super) async fn handler_api_schedules_toggle(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<TogglePayload>,
) -> impl IntoResponse {
    match state.db.update_schedule_enabled(id, payload.enabled).await {
        Ok(()) => Json(serde_json::json!({"ok": true})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// `DELETE /api/schedules/{id}` — Delete a schedule.
///
/// Replaces `supabase.from("agent_schedules").delete().eq("id", id)`.
pub(super) async fn handler_api_schedules_delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.delete_agent_schedule(id).await {
        Ok(true) => Json(serde_json::json!({"ok": true})).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Schedule not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
