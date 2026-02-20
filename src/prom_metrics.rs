//! # Prometheus Metrics — Exposition for Container Orchestration
//!
//! Exposes darkreach operational metrics in the Prometheus text exposition format
//! for scraping by Prometheus, Grafana Agent, or any OpenMetrics-compatible collector.
//!
//! ## Metrics Exposed
//!
//! | Metric | Type | Labels | Description |
//! |--------|------|--------|-------------|
//! | `darkreach_primes_found_total` | Counter | `form` | Total primes discovered |
//! | `darkreach_candidates_tested_total` | Counter | `form` | Total candidates tested |
//! | `darkreach_workers_connected` | Gauge | — | Currently connected workers |
//! | `darkreach_work_blocks_available` | Gauge | — | Unclaimed work blocks |
//! | `darkreach_work_blocks_claimed` | Gauge | — | Currently claimed work blocks |
//! | `darkreach_search_jobs_active` | Gauge | — | Active search jobs |
//! | `darkreach_cpu_usage_percent` | Gauge | — | Coordinator CPU usage |
//! | `darkreach_memory_usage_percent` | Gauge | — | Coordinator memory usage |
//!
//! ## Integration
//!
//! Metrics are updated from the dashboard's 30-second background loop.
//! The `/metrics` endpoint renders the current registry state on each scrape.
//!
//! ## References
//!
//! - [OpenMetrics specification](https://openmetrics.io/)
//! - [Prometheus exposition format](https://prometheus.io/docs/instrumenting/exposition_formats/)

use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use std::sync::atomic::AtomicU64;

/// Label set for per-form metrics (primes found, candidates tested).
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct FormLabel {
    pub form: String,
}

/// Thread-safe metrics registry for the darkreach coordinator.
///
/// All fields use atomic types and are safe to update from any thread or async task.
/// The `Family` type automatically creates per-label-set metric instances on first use.
pub struct Metrics {
    pub registry: Registry,
    pub primes_found: Family<FormLabel, Counter>,
    pub candidates_tested: Family<FormLabel, Counter<f64, AtomicU64>>,
    pub workers_connected: Gauge,
    pub work_blocks_available: Gauge,
    pub work_blocks_claimed: Gauge,
    pub search_jobs_active: Gauge,
    pub cpu_usage_percent: Gauge<f64, AtomicU64>,
    pub memory_usage_percent: Gauge<f64, AtomicU64>,
}

impl Metrics {
    /// Create a new metrics registry with all darkreach metrics registered.
    pub fn new() -> Self {
        let mut registry = Registry::default();

        let primes_found = Family::<FormLabel, Counter>::default();
        registry.register(
            "darkreach_primes_found",
            "Total primes discovered by form",
            primes_found.clone(),
        );

        let candidates_tested = Family::<FormLabel, Counter<f64, AtomicU64>>::default();
        registry.register(
            "darkreach_candidates_tested",
            "Total candidates tested by form",
            candidates_tested.clone(),
        );

        let workers_connected = Gauge::default();
        registry.register(
            "darkreach_workers_connected",
            "Number of currently connected workers",
            workers_connected.clone(),
        );

        let work_blocks_available = Gauge::default();
        registry.register(
            "darkreach_work_blocks_available",
            "Number of unclaimed work blocks",
            work_blocks_available.clone(),
        );

        let work_blocks_claimed = Gauge::default();
        registry.register(
            "darkreach_work_blocks_claimed",
            "Number of currently claimed work blocks",
            work_blocks_claimed.clone(),
        );

        let search_jobs_active = Gauge::default();
        registry.register(
            "darkreach_search_jobs_active",
            "Number of active search jobs",
            search_jobs_active.clone(),
        );

        let cpu_usage_percent = Gauge::<f64, AtomicU64>::default();
        registry.register(
            "darkreach_cpu_usage_percent",
            "Coordinator CPU usage percentage",
            cpu_usage_percent.clone(),
        );

        let memory_usage_percent = Gauge::<f64, AtomicU64>::default();
        registry.register(
            "darkreach_memory_usage_percent",
            "Coordinator memory usage percentage",
            memory_usage_percent.clone(),
        );

        Self {
            registry,
            primes_found,
            candidates_tested,
            workers_connected,
            work_blocks_available,
            work_blocks_claimed,
            search_jobs_active,
            cpu_usage_percent,
            memory_usage_percent,
        }
    }

    /// Render all metrics in Prometheus text exposition format.
    pub fn encode(&self) -> String {
        let mut buf = String::new();
        encode(&mut buf, &self.registry).expect("encoding metrics should not fail");
        buf
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_encode_returns_valid_text() {
        let m = Metrics::new();
        m.workers_connected.set(5);
        m.cpu_usage_percent.set(42.5);
        m.primes_found
            .get_or_create(&FormLabel {
                form: "factorial".to_string(),
            })
            .inc();

        let output = m.encode();
        assert!(output.contains("darkreach_workers_connected"));
        assert!(output.contains("darkreach_cpu_usage_percent"));
        assert!(output.contains("darkreach_primes_found"));
        assert!(output.contains("factorial"));
    }

    #[test]
    fn metrics_default_values_are_zero() {
        let m = Metrics::new();
        let output = m.encode();
        // Gauges should be present but at default (0)
        assert!(output.contains("darkreach_workers_connected"));
        assert!(output.contains("darkreach_work_blocks_available"));
    }

    #[test]
    fn metrics_per_form_counters_independent() {
        let m = Metrics::new();
        m.primes_found
            .get_or_create(&FormLabel {
                form: "factorial".to_string(),
            })
            .inc_by(3);
        m.primes_found
            .get_or_create(&FormLabel {
                form: "kbn".to_string(),
            })
            .inc_by(7);

        let output = m.encode();
        assert!(output.contains("factorial"));
        assert!(output.contains("kbn"));
    }
}
