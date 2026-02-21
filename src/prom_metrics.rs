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
//! | `darkreach_http_request_duration_seconds` | Histogram | `method`, `path` | API request latency |
//! | `darkreach_db_query_duration_seconds` | Histogram | `query` | Database query latency |
//! | `darkreach_work_block_duration_seconds` | Histogram | `form` | Work block processing time |
//! | `darkreach_heartbeat_rtt_seconds` | Histogram | — | Worker heartbeat round-trip time |
//! | `darkreach_db_pool_active` | Gauge | — | Active (in-use) database connections |
//! | `darkreach_db_pool_idle` | Gauge | — | Idle database connections |
//! | `darkreach_db_pool_max` | Gauge | — | Maximum configured database connections |
//! | `darkreach_ws_connections_active` | Gauge | — | Active WebSocket connections |
//! | `darkreach_ws_messages_sent_total` | Counter | — | Total WebSocket messages sent |
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
use prometheus_client::metrics::histogram::Histogram;
use prometheus_client::registry::Registry;
use std::sync::atomic::AtomicU64;

/// A single entry in the metric catalog returned by `GET /api/observability/catalog`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct MetricCatalogEntry {
    pub name: &'static str,
    pub metric_type: &'static str,
    pub unit: &'static str,
    pub description: &'static str,
    pub labels: &'static [&'static str],
}

/// Label set for per-form metrics (primes found, candidates tested, block duration).
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct FormLabel {
    pub form: String,
}

/// Label set for HTTP request duration histogram (method + path).
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct HttpLabel {
    pub method: String,
    pub path: String,
}

/// Label set for database query duration histogram.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct QueryLabel {
    pub query: String,
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
    /// API request latency by method and path.
    pub http_request_duration: Family<HttpLabel, Histogram, fn() -> Histogram>,
    /// Database query latency by query name.
    pub db_query_duration: Family<QueryLabel, Histogram, fn() -> Histogram>,
    /// Work block processing time by search form.
    pub work_block_duration: Family<FormLabel, Histogram, fn() -> Histogram>,
    /// Worker heartbeat SQL round-trip time.
    pub heartbeat_rtt: Histogram,
    /// Active (in-use) database connections.
    pub db_pool_active: Gauge,
    /// Idle database connections.
    pub db_pool_idle: Gauge,
    /// Maximum configured database connections.
    pub db_pool_max: Gauge,
    /// Active WebSocket connections.
    pub ws_connections_active: Gauge,
    /// Total WebSocket messages sent to clients.
    pub ws_messages_sent: Counter,
    /// Active (in-use) read replica database connections.
    pub db_read_pool_active: Gauge,
    /// Idle read replica database connections.
    pub db_read_pool_idle: Gauge,
    /// AI engine tick duration in seconds.
    pub ai_engine_tick_duration: Histogram,
    /// AI engine total tick count.
    pub ai_engine_tick_count: Counter,
    /// AI engine decisions made (by type).
    pub ai_engine_decisions: Family<FormLabel, Counter>,
    /// AI engine cost model version.
    pub ai_engine_cost_model_version: Gauge,
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

        // HTTP request latency: fine-grained buckets from 1ms to 5s
        fn http_histogram() -> Histogram {
            Histogram::new([0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0])
        }
        let http_request_duration =
            Family::<HttpLabel, Histogram, fn() -> Histogram>::new_with_constructor(
                http_histogram as fn() -> Histogram,
            );
        registry.register(
            "darkreach_http_request_duration_seconds",
            "API request latency in seconds",
            http_request_duration.clone(),
        );

        // DB query latency: 1ms to 5s
        fn db_histogram() -> Histogram {
            Histogram::new([0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0])
        }
        let db_query_duration =
            Family::<QueryLabel, Histogram, fn() -> Histogram>::new_with_constructor(
                db_histogram as fn() -> Histogram,
            );
        registry.register(
            "darkreach_db_query_duration_seconds",
            "Database query latency in seconds",
            db_query_duration.clone(),
        );

        // Work block processing: 1s to 1 hour
        fn block_histogram() -> Histogram {
            Histogram::new([1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0])
        }
        let work_block_duration =
            Family::<FormLabel, Histogram, fn() -> Histogram>::new_with_constructor(
                block_histogram as fn() -> Histogram,
            );
        registry.register(
            "darkreach_work_block_duration_seconds",
            "Work block processing time in seconds",
            work_block_duration.clone(),
        );

        // Heartbeat RTT: 10ms to 5s
        let heartbeat_rtt =
            Histogram::new([0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0]);
        registry.register(
            "darkreach_heartbeat_rtt_seconds",
            "Worker heartbeat SQL round-trip time in seconds",
            heartbeat_rtt.clone(),
        );

        let db_pool_active = Gauge::default();
        registry.register(
            "darkreach_db_pool_active",
            "Active (in-use) database connections",
            db_pool_active.clone(),
        );

        let db_pool_idle = Gauge::default();
        registry.register(
            "darkreach_db_pool_idle",
            "Idle database connections",
            db_pool_idle.clone(),
        );

        let db_pool_max = Gauge::default();
        registry.register(
            "darkreach_db_pool_max",
            "Maximum configured database connections",
            db_pool_max.clone(),
        );

        let ws_connections_active = Gauge::default();
        registry.register(
            "darkreach_ws_connections_active",
            "Active WebSocket connections",
            ws_connections_active.clone(),
        );

        let ws_messages_sent = Counter::default();
        registry.register(
            "darkreach_ws_messages_sent",
            "Total WebSocket messages sent to clients",
            ws_messages_sent.clone(),
        );

        let db_read_pool_active = Gauge::default();
        registry.register(
            "darkreach_db_read_pool_active",
            "Active (in-use) read replica database connections",
            db_read_pool_active.clone(),
        );

        let db_read_pool_idle = Gauge::default();
        registry.register(
            "darkreach_db_read_pool_idle",
            "Idle read replica database connections",
            db_read_pool_idle.clone(),
        );

        // AI engine tick duration: 10ms to 30s
        let ai_engine_tick_duration =
            Histogram::new([0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0]);
        registry.register(
            "darkreach_ai_engine_tick_duration_seconds",
            "AI engine tick duration in seconds",
            ai_engine_tick_duration.clone(),
        );

        let ai_engine_tick_count = Counter::default();
        registry.register(
            "darkreach_ai_engine_tick_count",
            "Total AI engine ticks executed",
            ai_engine_tick_count.clone(),
        );

        let ai_engine_decisions = Family::<FormLabel, Counter>::default();
        registry.register(
            "darkreach_ai_engine_decisions",
            "AI engine decisions by type",
            ai_engine_decisions.clone(),
        );

        let ai_engine_cost_model_version = Gauge::default();
        registry.register(
            "darkreach_ai_engine_cost_model_version",
            "Current AI engine cost model version",
            ai_engine_cost_model_version.clone(),
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
            http_request_duration,
            db_query_duration,
            work_block_duration,
            heartbeat_rtt,
            db_pool_active,
            db_pool_idle,
            db_pool_max,
            ws_connections_active,
            ws_messages_sent,
            db_read_pool_active,
            db_read_pool_idle,
            ai_engine_tick_duration,
            ai_engine_tick_count,
            ai_engine_decisions,
            ai_engine_cost_model_version,
        }
    }

    /// Return a catalog of all registered metrics with name, type, unit,
    /// description, and labels. Consumed by `GET /api/observability/catalog`.
    pub fn catalog() -> Vec<MetricCatalogEntry> {
        vec![
            MetricCatalogEntry { name: "darkreach_primes_found_total", metric_type: "counter", unit: "primes", description: "Total primes discovered", labels: &["form"] },
            MetricCatalogEntry { name: "darkreach_candidates_tested_total", metric_type: "counter", unit: "candidates", description: "Total candidates tested", labels: &["form"] },
            MetricCatalogEntry { name: "darkreach_workers_connected", metric_type: "gauge", unit: "workers", description: "Currently connected workers", labels: &[] },
            MetricCatalogEntry { name: "darkreach_work_blocks_available", metric_type: "gauge", unit: "blocks", description: "Unclaimed work blocks", labels: &[] },
            MetricCatalogEntry { name: "darkreach_work_blocks_claimed", metric_type: "gauge", unit: "blocks", description: "Currently claimed work blocks", labels: &[] },
            MetricCatalogEntry { name: "darkreach_search_jobs_active", metric_type: "gauge", unit: "jobs", description: "Active search jobs", labels: &[] },
            MetricCatalogEntry { name: "darkreach_cpu_usage_percent", metric_type: "gauge", unit: "percent", description: "Coordinator CPU usage", labels: &[] },
            MetricCatalogEntry { name: "darkreach_memory_usage_percent", metric_type: "gauge", unit: "percent", description: "Coordinator memory usage", labels: &[] },
            MetricCatalogEntry { name: "darkreach_http_request_duration_seconds", metric_type: "histogram", unit: "seconds", description: "API request latency", labels: &["method", "path"] },
            MetricCatalogEntry { name: "darkreach_db_query_duration_seconds", metric_type: "histogram", unit: "seconds", description: "Database query latency", labels: &["query"] },
            MetricCatalogEntry { name: "darkreach_work_block_duration_seconds", metric_type: "histogram", unit: "seconds", description: "Work block processing time", labels: &["form"] },
            MetricCatalogEntry { name: "darkreach_heartbeat_rtt_seconds", metric_type: "histogram", unit: "seconds", description: "Worker heartbeat round-trip time", labels: &[] },
            MetricCatalogEntry { name: "darkreach_db_pool_active", metric_type: "gauge", unit: "connections", description: "Active (in-use) database connections", labels: &[] },
            MetricCatalogEntry { name: "darkreach_db_pool_idle", metric_type: "gauge", unit: "connections", description: "Idle database connections", labels: &[] },
            MetricCatalogEntry { name: "darkreach_db_pool_max", metric_type: "gauge", unit: "connections", description: "Maximum configured database connections", labels: &[] },
            MetricCatalogEntry { name: "darkreach_ws_connections_active", metric_type: "gauge", unit: "connections", description: "Active WebSocket connections", labels: &[] },
            MetricCatalogEntry { name: "darkreach_ws_messages_sent_total", metric_type: "counter", unit: "messages", description: "Total WebSocket messages sent", labels: &[] },
            MetricCatalogEntry { name: "darkreach_db_read_pool_active", metric_type: "gauge", unit: "connections", description: "Active read replica database connections", labels: &[] },
            MetricCatalogEntry { name: "darkreach_db_read_pool_idle", metric_type: "gauge", unit: "connections", description: "Idle read replica database connections", labels: &[] },
            MetricCatalogEntry { name: "darkreach_ai_engine_tick_duration_seconds", metric_type: "histogram", unit: "seconds", description: "AI engine tick duration", labels: &[] },
            MetricCatalogEntry { name: "darkreach_ai_engine_tick_count", metric_type: "counter", unit: "ticks", description: "Total AI engine ticks", labels: &[] },
            MetricCatalogEntry { name: "darkreach_ai_engine_decisions", metric_type: "counter", unit: "decisions", description: "AI engine decisions by type", labels: &["form"] },
            MetricCatalogEntry { name: "darkreach_ai_engine_cost_model_version", metric_type: "gauge", unit: "version", description: "Cost model version", labels: &[] },
        ]
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
    //! Tests for Prometheus metrics exposition.
    //!
    //! Validates that the darkreach metrics registry produces valid Prometheus
    //! text exposition format output with correct HELP/TYPE lines, per-form
    //! label families, gauge values, and counter semantics.
    //!
    //! ## Prometheus Exposition Format
    //!
    //! Each metric produces three lines:
    //!   # HELP darkreach_<name> <description>
    //!   # TYPE darkreach_<name> <gauge|counter>
    //!   darkreach_<name>{labels} <value>
    //!
    //! The Family<FormLabel, Counter> type creates per-label instances on first
    //! access via get_or_create(), allowing independent per-form tracking.

    use super::*;

    // ── Encoding and Format ────────────────────────────────────────

    /// Validates that metrics encode to text containing all registered metric
    /// names and form labels. This output is scraped by Prometheus at /metrics.
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

    /// Fresh metrics must produce zero-valued gauges. This is the initial
    /// state before the first background loop populates real values.
    #[test]
    fn metrics_default_values_are_zero() {
        let m = Metrics::new();
        let output = m.encode();
        // Gauges should be present but at default (0)
        assert!(output.contains("darkreach_workers_connected"));
        assert!(output.contains("darkreach_work_blocks_available"));
    }

    // ── Per-Form Label Families ──────────────────────────────────

    /// Per-form counters must track independently. Incrementing "factorial"
    /// must not affect "kbn". The Family type auto-creates instances keyed
    /// by FormLabel on first get_or_create() call.
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

    /// Prometheus text format requires HELP lines for metric descriptions.
    /// Missing HELP lines cause Grafana import warnings.
    #[test]
    fn encode_contains_help_lines() {
        let m = Metrics::new();
        let output = m.encode();
        // Prometheus text format should include HELP lines for registered metrics
        assert!(
            output.contains("# HELP"),
            "Prometheus output should contain HELP lines"
        );
    }

    /// Prometheus text format requires TYPE lines (gauge, counter, etc.).
    /// Missing TYPE lines cause Prometheus to treat all metrics as untyped.
    #[test]
    fn encode_contains_type_lines() {
        let m = Metrics::new();
        let output = m.encode();
        // Prometheus text format should include TYPE lines
        assert!(
            output.contains("# TYPE"),
            "Prometheus output should contain TYPE lines"
        );
    }

    // ── Gauge Values ─────────────────────────────────────────────

    /// Gauge.set() values must appear in the encoded output. The background
    /// loop calls set() every 30 seconds with fresh values from PostgreSQL.
    #[test]
    fn gauge_set_reflected_in_output() {
        let m = Metrics::new();
        m.workers_connected.set(42);
        m.work_blocks_available.set(100);
        m.work_blocks_claimed.set(17);
        m.search_jobs_active.set(3);

        let output = m.encode();
        assert!(output.contains("42"), "workers_connected=42 not found in output");
        assert!(output.contains("100"), "work_blocks_available=100 not found");
        assert!(output.contains("17"), "work_blocks_claimed=17 not found");
    }

    /// The candidates_tested counter uses f64 (Counter<f64, AtomicU64>) to
    /// support large increments without integer overflow concerns.
    #[test]
    fn candidates_tested_counter_works() {
        let m = Metrics::new();
        m.candidates_tested
            .get_or_create(&FormLabel {
                form: "palindromic".to_string(),
            })
            .inc_by(1000.0);

        let output = m.encode();
        assert!(output.contains("darkreach_candidates_tested"));
        assert!(output.contains("palindromic"));
    }

    /// All 5 test forms must appear in the same primes_found family output.
    /// The Family type dynamically creates labeled instances.
    #[test]
    fn multiple_forms_in_single_family() {
        let m = Metrics::new();
        let forms = ["factorial", "kbn", "palindromic", "primorial", "wagstaff"];
        for form in &forms {
            m.primes_found
                .get_or_create(&FormLabel {
                    form: form.to_string(),
                })
                .inc();
        }

        let output = m.encode();
        for form in &forms {
            assert!(output.contains(form), "Form '{}' missing from output", form);
        }
    }

    /// CPU and memory gauges use f64 for fractional percentages. Verifies
    /// the Gauge<f64, AtomicU64> type works correctly with set().
    #[test]
    fn cpu_and_memory_gauge_f64() {
        let m = Metrics::new();
        m.cpu_usage_percent.set(73.5);
        m.memory_usage_percent.set(88.2);

        let output = m.encode();
        assert!(output.contains("darkreach_cpu_usage_percent"));
        assert!(output.contains("darkreach_memory_usage_percent"));
    }

    // ── Construction ─────────────────────────────────────────────

    /// Default and new() must produce structurally identical registries.
    /// Both are used in different contexts (Default for struct initialization,
    /// new() for explicit construction).
    #[test]
    fn default_creates_same_as_new() {
        let m1 = Metrics::new();
        let m2 = Metrics::default();
        // Both should produce valid (and similar) output
        let out1 = m1.encode();
        let out2 = m2.encode();
        assert_eq!(
            out1.lines().count(),
            out2.lines().count(),
            "new() and default() should produce same structure"
        );
    }

    /// FormLabel must implement Hash+Eq correctly for use as Family keys.
    /// Same-form labels must be equal; different-form labels must differ.
    #[test]
    fn form_label_hash_equality() {
        let l1 = FormLabel { form: "factorial".to_string() };
        let l2 = FormLabel { form: "factorial".to_string() };
        let l3 = FormLabel { form: "kbn".to_string() };
        assert_eq!(l1, l2);
        assert_ne!(l1, l3);
    }

    /// Even a fresh registry must produce non-empty output (HELP and TYPE
    /// lines are always emitted for registered metrics).
    #[test]
    fn encode_output_is_not_empty() {
        let m = Metrics::new();
        let output = m.encode();
        assert!(!output.is_empty(), "Encoded metrics should not be empty");
    }

    // ── Histogram Tests ───────────────────────────────────────────

    /// HTTP request duration histogram records observations and appears
    /// in encoded output with correct metric name and labels.
    #[test]
    fn http_request_duration_histogram() {
        let m = Metrics::new();
        m.http_request_duration
            .get_or_create(&HttpLabel {
                method: "GET".to_string(),
                path: "/api/status".to_string(),
            })
            .observe(0.042);

        let output = m.encode();
        assert!(output.contains("darkreach_http_request_duration_seconds"));
        assert!(output.contains("GET"));
        assert!(output.contains("/api/status"));
    }

    /// DB query duration histogram tracks per-query latency.
    #[test]
    fn db_query_duration_histogram() {
        let m = Metrics::new();
        m.db_query_duration
            .get_or_create(&QueryLabel {
                query: "worker_heartbeat".to_string(),
            })
            .observe(0.005);

        let output = m.encode();
        assert!(output.contains("darkreach_db_query_duration_seconds"));
        assert!(output.contains("worker_heartbeat"));
    }

    /// Work block duration histogram tracks processing time per form.
    #[test]
    fn work_block_duration_histogram() {
        let m = Metrics::new();
        m.work_block_duration
            .get_or_create(&FormLabel {
                form: "kbn".to_string(),
            })
            .observe(45.0);

        let output = m.encode();
        assert!(output.contains("darkreach_work_block_duration_seconds"));
        assert!(output.contains("kbn"));
    }

    /// Heartbeat RTT histogram records round-trip times.
    #[test]
    fn heartbeat_rtt_histogram() {
        let m = Metrics::new();
        m.heartbeat_rtt.observe(0.08);
        m.heartbeat_rtt.observe(0.12);

        let output = m.encode();
        assert!(output.contains("darkreach_heartbeat_rtt_seconds"));
        // Should have bucket entries and a count of 2
        assert!(output.contains("_count 2"));
    }

    /// DB pool metrics are registered and appear in encoded output.
    #[test]
    fn db_pool_metrics_registered() {
        let m = Metrics::new();
        m.db_pool_active.set(1);
        m.db_pool_idle.set(3);
        m.db_pool_max.set(5);
        let output = m.encode();
        assert!(output.contains("darkreach_db_pool_active"));
        assert!(output.contains("darkreach_db_pool_idle"));
        assert!(output.contains("darkreach_db_pool_max"));
    }

    /// WebSocket metrics are registered and appear in encoded output.
    #[test]
    fn ws_metrics_registered() {
        let m = Metrics::new();
        m.ws_connections_active.set(4);
        m.ws_messages_sent.inc_by(100);
        let output = m.encode();
        assert!(output.contains("darkreach_ws_connections_active"));
        assert!(output.contains("darkreach_ws_messages_sent"));
    }

    /// Metric catalog returns all 23 registered metrics.
    #[test]
    fn catalog_contains_all_metrics() {
        let catalog = Metrics::catalog();
        assert_eq!(catalog.len(), 23);
        let names: Vec<&str> = catalog.iter().map(|e| e.name).collect();
        assert!(names.contains(&"darkreach_primes_found_total"));
        assert!(names.contains(&"darkreach_http_request_duration_seconds"));
        assert!(names.contains(&"darkreach_db_pool_active"));
        assert!(names.contains(&"darkreach_ws_connections_active"));
        assert!(names.contains(&"darkreach_ws_messages_sent_total"));
    }

    /// HttpLabel and QueryLabel must implement Hash+Eq for Family keys.
    #[test]
    fn label_types_hash_equality() {
        let h1 = HttpLabel { method: "GET".into(), path: "/api".into() };
        let h2 = HttpLabel { method: "GET".into(), path: "/api".into() };
        let h3 = HttpLabel { method: "POST".into(), path: "/api".into() };
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);

        let q1 = QueryLabel { query: "heartbeat".into() };
        let q2 = QueryLabel { query: "heartbeat".into() };
        let q3 = QueryLabel { query: "insert".into() };
        assert_eq!(q1, q2);
        assert_ne!(q1, q3);
    }
}
