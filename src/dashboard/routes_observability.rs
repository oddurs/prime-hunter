//! Observability API — metrics, logs, and reports.

use super::AppState;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub(super) struct MetricsQuery {
    metric: Option<String>,
    metrics: Option<String>,
    scope: Option<String>,
    worker_id: Option<String>,
    from: Option<String>,
    to: Option<String>,
    rollup: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct LogsQuery {
    from: Option<String>,
    to: Option<String>,
    level: Option<String>,
    source: Option<String>,
    component: Option<String>,
    worker_id: Option<String>,
    limit: Option<i64>,
}

#[derive(Deserialize)]
pub(super) struct ReportQuery {
    from: Option<String>,
    to: Option<String>,
}

fn parse_ts(value: Option<&str>, default: DateTime<Utc>) -> DateTime<Utc> {
    value
        .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or(default)
}

fn should_use_rollup(rollup: Option<&str>, from: DateTime<Utc>, to: DateTime<Utc>) -> bool {
    match rollup.unwrap_or("auto") {
        "hour" | "hourly" => true,
        "raw" => false,
        _ => (to - from) > Duration::days(7),
    }
}

pub(super) async fn handler_metrics(
    State(state): State<Arc<AppState>>,
    Query(q): Query<MetricsQuery>,
) -> impl IntoResponse {
    let now = Utc::now();
    let from = parse_ts(q.from.as_deref(), now - Duration::hours(6));
    let to = parse_ts(q.to.as_deref(), now);
    let use_rollup = should_use_rollup(q.rollup.as_deref(), from, to);

    let mut metrics = Vec::new();
    if let Some(list) = q.metrics.as_ref().or(q.metric.as_ref()) {
        for item in list.split(',') {
            let trimmed = item.trim();
            if !trimmed.is_empty() {
                metrics.push(trimmed.to_string());
            }
        }
    }

    if metrics.is_empty() {
        return Json(serde_json::json!({
            "series": [],
            "rollup": if use_rollup { "hour" } else { "raw" }
        }));
    }

    let mut series = Vec::new();
    for metric in metrics {
        let points = match state
            .db
            .get_metric_points(
                from,
                to,
                &metric,
                q.scope.as_deref(),
                q.worker_id.as_deref(),
                use_rollup,
            )
            .await
        {
            Ok(p) => p,
            Err(_) => Vec::new(),
        };
        series.push(serde_json::json!({
            "metric": metric,
            "scope": q.scope.clone(),
            "worker_id": q.worker_id.clone(),
            "points": points,
        }));
    }

    Json(serde_json::json!({
        "series": series,
        "rollup": if use_rollup { "hour" } else { "raw" }
    }))
}

pub(super) async fn handler_logs(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LogsQuery>,
) -> impl IntoResponse {
    let now = Utc::now();
    let from = parse_ts(q.from.as_deref(), now - Duration::hours(6));
    let to = parse_ts(q.to.as_deref(), now);
    let limit = q.limit.unwrap_or(200).clamp(1, 2000);

    match state
        .db
        .get_system_logs(
            from,
            to,
            q.level.as_deref(),
            q.source.as_deref(),
            q.component.as_deref(),
            q.worker_id.as_deref(),
            limit,
        )
        .await
    {
        Ok(rows) => Json(serde_json::json!({"logs": rows})),
        Err(e) => Json(serde_json::json!({"error": e.to_string(), "logs": []})),
    }
}

pub(super) async fn handler_report(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ReportQuery>,
) -> impl IntoResponse {
    let now = Utc::now();
    let from = parse_ts(q.from.as_deref(), now - Duration::days(7));
    let to = parse_ts(q.to.as_deref(), now);

    let primes_total = state.db.count_primes_in_range(from, to).await.unwrap_or(0);
    let primes_by_form = state
        .db
        .count_primes_by_form_in_range(from, to)
        .await
        .unwrap_or_default();

    let errors_by_level = state
        .db
        .count_system_logs_by_level(from, to)
        .await
        .unwrap_or_default();

    let workers_peak = state
        .db
        .max_metric_in_range(from, to, "fleet.workers_connected", Some("fleet"))
        .await
        .unwrap_or(None);

    let avg_coord_cpu = state
        .db
        .avg_metric_in_range(
            from,
            to,
            "coordinator.cpu_usage_percent",
            Some("coordinator"),
        )
        .await
        .unwrap_or(None);

    let tested_delta = state
        .db
        .delta_metric_in_range(from, to, "fleet.total_tested", Some("fleet"))
        .await
        .unwrap_or(None);

    let found_delta = state
        .db
        .delta_metric_in_range(from, to, "fleet.total_found", Some("fleet"))
        .await
        .unwrap_or(None);

    Json(serde_json::json!({
        "from": from,
        "to": to,
        "primes": {
            "total": primes_total,
            "by_form": primes_by_form,
        },
        "logs": {
            "by_level": errors_by_level,
        },
        "fleet": {
            "workers_peak": workers_peak,
            "tested_delta": tested_delta,
            "found_delta": found_delta,
        },
        "coordinator": {
            "avg_cpu_usage_percent": avg_coord_cpu,
        }
    }))
}
