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
use redis::AsyncCommands;
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
/// Checks database connectivity (primary + read replica + Redis) with a
/// 2-second timeout. Returns 503 Service Unavailable if any critical
/// component is unreachable.
pub async fn handler_readyz(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let timeout = std::time::Duration::from_secs(2);

    // Check primary pool
    let primary_check = tokio::time::timeout(timeout, state.db.health_check()).await;
    match primary_check {
        Ok(Ok(())) => {}
        Ok(Err(_)) => return (StatusCode::SERVICE_UNAVAILABLE, "primary database unreachable"),
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, "primary database timeout"),
    }

    // Check read replica pool (may be same as primary if no replica configured)
    let read_check = tokio::time::timeout(timeout, async {
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(state.db.read_pool())
            .await
    })
    .await;
    match read_check {
        Ok(Ok(_)) => {}
        Ok(Err(_)) => return (StatusCode::SERVICE_UNAVAILABLE, "read replica unreachable"),
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, "read replica timeout"),
    }

    // Check Redis (non-critical — degrade gracefully)
    if let Some(redis) = state.db.redis() {
        let mut conn = redis.clone();
        let redis_check = tokio::time::timeout(timeout, async {
            redis::cmd("PING")
                .query_async::<String>(&mut conn)
                .await
        })
        .await;
        match redis_check {
            Ok(Ok(_)) => {}
            _ => {
                // Redis is optional — warn but don't fail readiness
                tracing::warn!("readyz: Redis health check failed (degraded mode)");
            }
        }
    }

    (StatusCode::OK, "ok")
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
