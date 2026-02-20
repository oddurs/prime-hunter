//! Observability API — metrics, logs, and reports.

use super::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
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
    label_key: Option<String>,
    label_value: Option<String>,
    from: Option<String>,
    to: Option<String>,
    rollup: Option<String>,
    format: Option<String>,
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
    format: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct ReportQuery {
    from: Option<String>,
    to: Option<String>,
    format: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct TopWorkersQuery {
    limit: Option<i64>,
    window_minutes: Option<i64>,
}

fn parse_ts(value: Option<&str>, default: DateTime<Utc>) -> DateTime<Utc> {
    value
        .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or(default)
}

fn select_rollup(rollup: Option<&str>, from: DateTime<Utc>, to: DateTime<Utc>) -> &'static str {
    match rollup.unwrap_or("auto") {
        "day" | "daily" => "day",
        "hour" | "hourly" => "hour",
        "raw" => "raw",
        _ => {
            let span = to - from;
            if span > Duration::days(90) {
                "day"
            } else if span > Duration::days(7) {
                "hour"
            } else {
                "raw"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_rollup_auto_picks_raw_for_short_ranges() {
        let now = Utc::now();
        let from = now - Duration::hours(6);
        assert_eq!(select_rollup(None, from, now), "raw");
    }

    #[test]
    fn select_rollup_auto_picks_hour_for_medium_ranges() {
        let now = Utc::now();
        let from = now - Duration::days(30);
        assert_eq!(select_rollup(None, from, now), "hour");
    }

    #[test]
    fn select_rollup_auto_picks_day_for_long_ranges() {
        let now = Utc::now();
        let from = now - Duration::days(120);
        assert_eq!(select_rollup(None, from, now), "day");
    }

    #[test]
    fn select_rollup_respects_explicit_setting() {
        let now = Utc::now();
        let from = now - Duration::days(1);
        assert_eq!(select_rollup(Some("hour"), from, now), "hour");
        assert_eq!(select_rollup(Some("day"), from, now), "day");
        assert_eq!(select_rollup(Some("raw"), from, now), "raw");
    }
}
pub(super) async fn handler_metrics(
    State(state): State<Arc<AppState>>,
    Query(q): Query<MetricsQuery>,
) -> Response {
    let now = Utc::now();
    let from = parse_ts(q.from.as_deref(), now - Duration::hours(6));
    let to = parse_ts(q.to.as_deref(), now);
    let rollup = select_rollup(q.rollup.as_deref(), from, to);

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
            "rollup": rollup
        }))
        .into_response();
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
                q.label_key.as_deref(),
                q.label_value.as_deref(),
                rollup,
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

    if matches!(q.format.as_deref(), Some("csv")) {
        let mut csv = String::new();
        csv.push_str("metric,ts,value,scope,worker_id,label_key,label_value\n");
        for s in &series {
            let metric = s["metric"].as_str().unwrap_or_default();
            let scope = s["scope"].as_str().unwrap_or_default();
            let worker_id = s["worker_id"].as_str().unwrap_or_default();
            if let Some(points) = s["points"].as_array() {
                for p in points {
                    let ts = p["ts"].as_str().unwrap_or_default();
                    let value = p["value"].as_f64().unwrap_or(0.0);
                    csv.push_str(&format!(
                        "\"{}\",\"{}\",{},\"{}\",\"{}\",\"{}\",\"{}\"\n",
                        metric,
                        ts,
                        value,
                        scope,
                        worker_id,
                        q.label_key.clone().unwrap_or_default(),
                        q.label_value.clone().unwrap_or_default(),
                    ));
                }
            }
        }
        return (
            StatusCode::OK,
            [("content-type", "text/csv; charset=utf-8")],
            csv,
        )
            .into_response();
    }

    Json(serde_json::json!({
        "series": series,
        "rollup": rollup
    }))
    .into_response()
}

pub(super) async fn handler_logs(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LogsQuery>,
) -> Response {
    let now = Utc::now();
    let from = parse_ts(q.from.as_deref(), now - Duration::hours(6));
    let to = parse_ts(q.to.as_deref(), now);
    let limit = q.limit.unwrap_or(200).clamp(1, 2000);

    let result = state
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
        .await;

    match result {
        Ok(rows) => {
            if matches!(q.format.as_deref(), Some("csv")) {
                let mut csv = String::new();
                csv.push_str("ts,level,source,component,message,worker_id\n");
                for row in rows {
                    let msg = row.message.replace('\"', "\"\"");
                    let component = row.component.replace('\"', "\"\"");
                    let source = row.source.replace('\"', "\"\"");
                    let worker_id = row.worker_id.unwrap_or_default();
                    csv.push_str(&format!(
                        "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                        row.ts.to_rfc3339(),
                        row.level,
                        source,
                        component,
                        msg,
                        worker_id
                    ));
                }
                (
                    StatusCode::OK,
                    [("content-type", "text/csv; charset=utf-8")],
                    csv,
                )
                    .into_response()
            } else {
                Json(serde_json::json!({ "logs": rows })).into_response()
            }
        }
        Err(e) => Json(serde_json::json!({ "error": e.to_string(), "logs": [] })).into_response(),
    }
}

pub(super) async fn handler_report(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ReportQuery>,
) -> Response {
    let now = Utc::now();
    let from = parse_ts(q.from.as_deref(), now - Duration::days(7));
    let to = parse_ts(q.to.as_deref(), now);
    let duration_hours = ((to - from).num_minutes().max(1) as f64) / 60.0;
    let error_budget_errors_per_hour: f64 = std::env::var("OBS_ERROR_BUDGET_ERRORS_PER_HOUR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10.0);
    let error_budget_warnings_per_hour: f64 = std::env::var("OBS_ERROR_BUDGET_WARNINGS_PER_HOUR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50.0);

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
    let error_count = errors_by_level
        .iter()
        .find(|(level, _)| level == "error")
        .map(|(_, count)| *count)
        .unwrap_or(0) as f64;
    let warning_count = errors_by_level
        .iter()
        .find(|(level, _)| level == "warning" || level == "warn")
        .map(|(_, count)| *count)
        .unwrap_or(0) as f64;
    let errors_per_hour = error_count / duration_hours;
    let warnings_per_hour = warning_count / duration_hours;
    let budget_status = if errors_per_hour > error_budget_errors_per_hour {
        "breached"
    } else if warnings_per_hour > error_budget_warnings_per_hour {
        "risk"
    } else {
        "healthy"
    };

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

    let payload = serde_json::json!({
        "from": from,
        "to": to,
        "duration_hours": duration_hours,
        "primes": {
            "total": primes_total,
            "by_form": primes_by_form,
        },
        "logs": {
            "by_level": errors_by_level,
        },
        "budget": {
            "errors_per_hour": errors_per_hour,
            "warnings_per_hour": warnings_per_hour,
            "status": budget_status,
        },
        "fleet": {
            "workers_peak": workers_peak,
            "tested_delta": tested_delta,
            "found_delta": found_delta,
        },
        "coordinator": {
            "avg_cpu_usage_percent": avg_coord_cpu,
        }
    });

    if matches!(q.format.as_deref(), Some("csv")) {
        let mut csv = String::new();
        csv.push_str("from,to,primes_total,workers_peak,tested_delta,found_delta,avg_cpu,error_count,warning_count,errors_per_hour,warnings_per_hour,budget_status\n");
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            from.to_rfc3339(),
            to.to_rfc3339(),
            primes_total,
            workers_peak.unwrap_or(0.0),
            tested_delta.unwrap_or(0.0),
            found_delta.unwrap_or(0.0),
            avg_coord_cpu.unwrap_or(0.0),
            error_count,
            warning_count,
            errors_per_hour,
            warnings_per_hour,
            budget_status
        ));
        csv.push_str("\nform,primes_count\n");
        for (form, count) in primes_by_form {
            csv.push_str(&format!("{},{}\n", form, count));
        }
        return (
            StatusCode::OK,
            [("content-type", "text/csv; charset=utf-8")],
            csv,
        )
            .into_response();
    }

    Json(payload).into_response()
}

pub(super) async fn handler_top_workers(
    State(state): State<Arc<AppState>>,
    Query(q): Query<TopWorkersQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(10).clamp(1, 50);
    let window = q.window_minutes.unwrap_or(30).clamp(5, 240);
    match state.db.get_top_workers_by_rate(window, limit).await {
        Ok(rows) => Json(serde_json::json!({ "workers": rows })),
        Err(e) => Json(serde_json::json!({ "workers": [], "error": e.to_string() })),
    }
}
