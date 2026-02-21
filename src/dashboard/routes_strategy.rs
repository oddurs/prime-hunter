//! Strategy engine API — status, decisions, scoring, configuration, and manual triggers.
//!
//! All routes require admin authentication via the `RequireAdmin` extractor.

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

use super::middleware_auth::RequireAdmin;
use super::AppState;
use crate::strategy;

/// GET /api/strategy/status — Engine status and last tick info.
pub(super) async fn handler_strategy_status(
    _auth: RequireAdmin,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config = match state.db.get_strategy_config().await {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let last_decision = state
        .db
        .get_strategy_decisions(1)
        .await
        .unwrap_or_default();
    let monthly_spend = state.db.get_monthly_strategy_spend().await.unwrap_or(0.0);

    Json(serde_json::json!({
        "enabled": config.enabled,
        "tick_interval_secs": config.tick_interval_secs,
        "last_tick": last_decision.first().map(|d| d.created_at),
        "monthly_spend_usd": monthly_spend,
        "monthly_budget_usd": config.max_monthly_budget_usd,
        "max_concurrent_projects": config.max_concurrent_projects,
    }))
    .into_response()
}

/// GET /api/strategy/decisions — Recent decisions with reasoning.
pub(super) async fn handler_strategy_decisions(
    _auth: RequireAdmin,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_strategy_decisions(100).await {
        Ok(decisions) => Json(serde_json::json!(decisions)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/strategy/scores — Current form scoring.
pub(super) async fn handler_strategy_scores(
    _auth: RequireAdmin,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match strategy::get_current_scores(&state.db).await {
        Ok(scores) => Json(serde_json::json!(scores)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/strategy/config — Read engine configuration.
pub(super) async fn handler_strategy_config_get(
    _auth: RequireAdmin,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_strategy_config().await {
        Ok(config) => Json(serde_json::json!(config)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub(super) struct UpdateConfigPayload {
    enabled: Option<bool>,
    max_concurrent_projects: Option<i32>,
    max_monthly_budget_usd: Option<f64>,
    max_per_project_budget_usd: Option<f64>,
    preferred_forms: Option<Vec<String>>,
    excluded_forms: Option<Vec<String>>,
    min_idle_workers_to_create: Option<i32>,
    record_proximity_threshold: Option<f64>,
    tick_interval_secs: Option<i32>,
}

/// PUT /api/strategy/config — Update engine configuration.
pub(super) async fn handler_strategy_config_put(
    _auth: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateConfigPayload>,
) -> impl IntoResponse {
    match state
        .db
        .update_strategy_config(
            payload.enabled,
            payload.max_concurrent_projects,
            payload.max_monthly_budget_usd,
            payload.max_per_project_budget_usd,
            payload.preferred_forms.as_deref(),
            payload.excluded_forms.as_deref(),
            payload.min_idle_workers_to_create,
            payload.record_proximity_threshold,
            payload.tick_interval_secs,
        )
        .await
    {
        Ok(config) => Json(serde_json::json!(config)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub(super) struct OverridePayload {
    action_taken: String,
    reason: String,
}

/// POST /api/strategy/decisions/{id}/override — Admin override of a decision.
pub(super) async fn handler_strategy_override(
    _auth: RequireAdmin,
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
    Json(payload): Json<OverridePayload>,
) -> impl IntoResponse {
    match state
        .db
        .override_strategy_decision(id, &payload.action_taken, &payload.reason)
        .await
    {
        Ok(()) => Json(serde_json::json!({"ok": true})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// POST /api/strategy/tick — Force an immediate AI engine tick.
pub(super) async fn handler_strategy_tick(
    _auth: RequireAdmin,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let mut engine = state.ai_engine.lock().await;
    match engine.tick(&state.db).await {
        Ok(outcome) => Json(serde_json::json!({
            "tick_id": outcome.tick_id,
            "decisions": outcome.decisions,
            "scores": outcome.analysis.scores,
            "drift": outcome.analysis.drift,
            "duration_ms": outcome.duration_ms,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/strategy/ai-engine — AI engine state (weights, cost model, tick count).
pub(super) async fn handler_ai_engine_status(
    _auth: RequireAdmin,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let engine = state.ai_engine.lock().await;
    Json(serde_json::json!({
        "enabled": engine.config.enabled,
        "tick_count": engine.tick_count,
        "scoring_weights": engine.scoring_weights,
        "cost_model_version": engine.cost_model.version,
        "cost_model_fitted_forms": engine.cost_model.fitted.keys().collect::<Vec<_>>(),
        "config": engine.config,
    }))
    .into_response()
}

/// GET /api/strategy/ai-decisions — Recent AI engine decisions.
pub(super) async fn handler_ai_engine_decisions(
    _auth: RequireAdmin,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_ai_engine_decisions(100).await {
        Ok(decisions) => Json(serde_json::json!(decisions)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
