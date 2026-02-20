//! Status, export, and index handlers.

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::websocket;
use super::AppState;
use crate::{checkpoint, db};

pub(super) async fn handler_index() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../dashboard.html"),
    )
}

#[derive(Serialize)]
pub(super) struct StatusResponse {
    pub active: bool,
    pub checkpoint: Option<serde_json::Value>,
}

pub(super) async fn handler_api_status(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
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

#[derive(Deserialize)]
pub(super) struct ExportQuery {
    format: Option<String>,
    form: Option<String>,
    search: Option<String>,
    min_digits: Option<i64>,
    max_digits: Option<i64>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
}

pub(super) async fn handler_api_export(
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
                p.proof_method.replace('"', "\"\"")
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

/// Returns the same JSON snapshot that the WebSocket pushes every 2 seconds.
/// Used by the Vercel frontend (polling mode) since Vercel rewrites cannot proxy WebSocket.
pub(super) async fn handler_api_ws_snapshot(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match websocket::build_update(&state).await {
        Some(json) => ([(header::CONTENT_TYPE, "application/json")], json).into_response(),
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to build snapshot",
        )
            .into_response(),
    }
}
