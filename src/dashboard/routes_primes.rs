//! # Prime Data REST API
//!
//! REST endpoints that replace direct Supabase RPC and table queries,
//! enabling the frontend to work against any PostgreSQL backend (not
//! just Supabase). Part of Phase 6: Frontend Independence.
//!
//! | Endpoint | Replaces |
//! |----------|----------|
//! | `GET /api/stats` | `supabase.rpc("get_stats")` |
//! | `GET /api/stats/timeline` | `supabase.rpc("get_discovery_timeline")` |
//! | `GET /api/stats/distribution` | `supabase.rpc("get_digit_distribution")` |
//! | `GET /api/stats/leaderboard` | `supabase.rpc("get_form_leaderboard")` |
//! | `GET /api/primes` | `supabase.from("primes").select()` |
//! | `GET /api/primes/{id}` | `supabase.from("primes").eq("id",id)` |

use super::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

/// `GET /api/stats` — Dashboard summary statistics.
///
/// Replaces `supabase.rpc("get_stats")`. Returns total prime count,
/// per-form counts, and largest prime info. Reads from the
/// `mv_dashboard_stats` materialized view when available.
pub(super) async fn handler_api_stats(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let result: Result<serde_json::Value, _> =
        sqlx::query_scalar("SELECT get_stats()")
            .fetch_one(state.db.read_pool())
            .await;
    match result {
        Ok(json) => Json(json).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub(super) struct TimelineQuery {
    bucket_type: Option<String>,
}

/// `GET /api/stats/timeline?bucket_type=day` — Discovery timeline.
///
/// Replaces `supabase.rpc("get_discovery_timeline", { bucket_type })`.
/// Returns primes bucketed by time period and form.
pub(super) async fn handler_api_timeline(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TimelineQuery>,
) -> impl IntoResponse {
    let bucket_type = params.bucket_type.unwrap_or_else(|| "day".to_string());
    let result: Result<serde_json::Value, _> =
        sqlx::query_scalar("SELECT get_discovery_timeline($1)")
            .bind(&bucket_type)
            .fetch_one(state.db.read_pool())
            .await;
    match result {
        Ok(json) => Json(json).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub(super) struct DistributionQuery {
    bucket_size: Option<i64>,
}

/// `GET /api/stats/distribution?bucket_size=100` — Digit distribution.
///
/// Replaces `supabase.rpc("get_digit_distribution", { bucket_size_param })`.
/// Returns primes bucketed by digit count ranges.
pub(super) async fn handler_api_distribution(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DistributionQuery>,
) -> impl IntoResponse {
    let bucket_size = params.bucket_size.unwrap_or(10);
    let result: Result<serde_json::Value, _> =
        sqlx::query_scalar("SELECT get_digit_distribution($1)")
            .bind(bucket_size)
            .fetch_one(state.db.read_pool())
            .await;
    match result {
        Ok(json) => Json(json).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// `GET /api/stats/leaderboard` — Form leaderboard.
///
/// Replaces `supabase.rpc("get_form_leaderboard")`. Returns per-form
/// aggregate statistics ordered by prime count.
pub(super) async fn handler_api_leaderboard(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let result: Result<serde_json::Value, _> =
        sqlx::query_scalar("SELECT get_form_leaderboard()")
            .fetch_one(state.db.read_pool())
            .await;
    match result {
        Ok(json) => Json(json).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub(super) struct PrimesQuery {
    limit: Option<i64>,
    offset: Option<i64>,
    form: Option<String>,
    search: Option<String>,
    min_digits: Option<i64>,
    max_digits: Option<i64>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
}

/// `GET /api/primes?limit=50&offset=0&form=factorial` — Filtered prime listing.
///
/// Replaces `supabase.from("primes").select()` with filters and pagination.
/// Returns both the primes array and total count for pagination UI.
pub(super) async fn handler_api_primes_list(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PrimesQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(1000);
    let offset = params.offset.unwrap_or(0);
    let filter = crate::db::PrimeFilter {
        form: params.form,
        search: params.search,
        min_digits: params.min_digits,
        max_digits: params.max_digits,
        sort_by: params.sort_by,
        sort_dir: params.sort_dir,
    };

    let (primes, total) = tokio::join!(
        state.db.get_primes_filtered(limit, offset, &filter),
        state.db.get_filtered_count(&filter),
    );

    match (primes, total) {
        (Ok(primes), Ok(total)) => Json(serde_json::json!({
            "primes": primes,
            "total": total,
            "limit": limit,
            "offset": offset,
        }))
        .into_response(),
        (Err(e), _) | (_, Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// `GET /api/primes/{id}` — Single prime detail.
///
/// Replaces `supabase.from("primes").eq("id", id).single()`.
pub(super) async fn handler_api_prime_get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.get_prime_by_id(id).await {
        Ok(Some(prime)) => Json(serde_json::json!(prime)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Prime not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
