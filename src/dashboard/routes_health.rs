//! # Health & Observability Endpoints
//!
//! Kubernetes-compatible health, readiness, and Prometheus metrics endpoints.
//!
//! | Endpoint | Purpose | K8s Probe |
//! |----------|---------|-----------|
//! | `GET /healthz` | Liveness — process is alive | `livenessProbe` |
//! | `GET /readyz` | Readiness — database connected, accepting traffic | `readinessProbe` |
//! | `GET /metrics` | Prometheus scraping endpoint | `ServiceMonitor` |
//!
//! The readiness probe performs a `SELECT 1` with a 2-second timeout. If the database
//! is unreachable, the coordinator returns 503 so the load balancer stops routing
//! traffic to it until connectivity is restored.

use super::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use std::sync::Arc;

/// Liveness probe: returns 200 if the process is running.
///
/// K8s uses this to determine if the container needs to be restarted.
/// No dependencies checked — if the binary is serving HTTP, it's alive.
pub async fn handler_healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// Readiness probe: returns 200 if the coordinator can serve requests.
///
/// Checks database connectivity with `SELECT 1` and a 2-second timeout.
/// Returns 503 Service Unavailable if the database is unreachable, which
/// tells K8s to stop routing traffic until the probe passes again.
pub async fn handler_readyz(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let check =
        tokio::time::timeout(std::time::Duration::from_secs(2), state.db.health_check()).await;

    match check {
        Ok(Ok(())) => (StatusCode::OK, "ok"),
        Ok(Err(_)) => (StatusCode::SERVICE_UNAVAILABLE, "database unreachable"),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "database timeout"),
    }
}

/// Prometheus metrics endpoint: returns all metrics in text exposition format.
///
/// Scraped by Prometheus every 15-30 seconds (configurable via ServiceMonitor).
/// Metrics are updated in the dashboard's 30-second background loop.
pub async fn handler_metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let body = state.prom_metrics.encode();
    (
        StatusCode::OK,
        [(
            "content-type",
            "application/openmetrics-text; version=1.0.0; charset=utf-8",
        )],
        body,
    )
}
