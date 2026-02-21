# Observability Roadmap — Metrics, Logging & Tracing

> **Goal:** Grafana-grade operational visibility across the entire darkreach stack — coordinator, workers, engine, and API — with structured logging, latency histograms, request tracing, and consistent instrumentation.

---

## Current State

### What's Working
- Prometheus metrics endpoint (`/metrics`, OpenMetrics 1.0 compliant)
- 8 Prometheus metrics: 2 per-form counters + 6 gauges
- 45+ internal time-series metrics stored in PostgreSQL (coordinator, fleet, search jobs, events, workers)
- Auto-rollups: hourly + daily aggregation with configurable retention
- Hardware telemetry: CPU, memory, disk, load averages via `sysinfo`
- Frontend observability page: 13 metric charts, top-workers, log viewer
- EventBus: 6 event types, batched prime notifications, WebSocket delivery
- Lock-free progress counters (AtomicU64) for search throughput

### What's Broken
- **75% of logging is `eprintln!`** — unstructured, no levels, not machine-parseable
- **`RUST_LOG` not wired** — `EnvFilter` feature is in Cargo.toml but never used
- **No request logging** — API calls are invisible (no method, path, status, latency)
- **No histograms** — can't track p50/p95/p99 for any operation
- **Silent failures** — `pg_worker.rs:153` swallows heartbeat errors via `.ok().flatten()`
- **No correlation IDs** — impossible to trace a request through coordinator → worker → DB
- **EventBus uses elapsed-from-start timestamps** — not UTC, useless for log aggregation

---

## Phase 1: Foundation — Structured Logging (Priority: Critical) ✅ COMPLETE

Wire `tracing` properly and migrate the most important `eprintln!` calls. This is the highest-ROI change — it unlocks `RUST_LOG` filtering, JSON output for log aggregation, and level-based routing.

### 1.1 Wire `RUST_LOG` via `EnvFilter`
- **File:** `src/main.rs` (lines 329–338)
- Add `EnvFilter::try_from_default_env()` with fallback to `"darkreach=info,tower_http=info"`
- Both human and JSON formatters get the filter
- Already have `tracing-subscriber = { features = ["json", "env-filter"] }` in Cargo.toml

### 1.2 Add request logging middleware
- **File:** `src/dashboard/mod.rs`
- Add `tower_http::trace::TraceLayer` to the Axum router
- Logs: method, path, status code, latency for every request
- Add `tower-http` feature `"trace"` to Cargo.toml

### 1.3 Migrate dashboard background loop
- **File:** `src/dashboard/mod.rs` (lines 487–520)
- Replace all `eprintln!("Warning: ...")` with `tracing::warn!(error = %e, "...")`
- Replace `eprintln!("Reclaimed/Rotated ...")` with `tracing::info!(...)`
- Structured fields: `count`, `error`, `duration_ms`

### 1.4 Migrate EventBus
- **File:** `src/events.rs`
- Replace `eprintln!("[{}] PRIME/WARN/ERROR ...")` with `tracing::info!/warn!/error!`
- Add structured fields: `form`, `digits`, `expression`, `proof_method`
- Keep the EventRecord + WebSocket broadcast path unchanged

### 1.5 Migrate progress reporter
- **File:** `src/progress.rs`
- Replace `eprintln!(...)` with `tracing::info!(tested, found, rate, current, "search progress")`

### 1.6 Fix silent heartbeat failure
- **File:** `src/pg_worker.rs` (line 153)
- Replace `.ok().flatten()` with match + `tracing::warn!` on error
- Log: `warn!(worker_id, error = %e, "heartbeat failed")`

---

## Phase 2: Prometheus Histograms ✅ COMPLETE

Add latency tracking for the operations that matter most. Uses `prometheus_client::metrics::histogram::Histogram`.

### 2.1 API request duration histogram
- **File:** `src/prom_metrics.rs`
- New metric: `darkreach_http_request_duration_seconds` (Family with `method` + `path` labels)
- Buckets: `[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]`
- Recorded via Axum middleware layer

### 2.2 Database query duration histogram
- **File:** `src/prom_metrics.rs`
- New metric: `darkreach_db_query_duration_seconds` (Family with `query` label)
- Buckets: `[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]`
- Instrument key queries: `insert_prime`, `claim_work_block`, `worker_heartbeat`, `search_job_status`

### 2.3 Work block completion duration histogram
- **File:** `src/prom_metrics.rs`
- New metric: `darkreach_work_block_duration_seconds` (Family with `form` label)
- Buckets: `[1, 5, 10, 30, 60, 120, 300, 600, 1800, 3600]`
- Recorded when a worker submits block results

### 2.4 Heartbeat round-trip histogram
- **File:** `src/prom_metrics.rs`
- New metric: `darkreach_heartbeat_rtt_seconds` (no labels)
- Buckets: `[0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0]`
- Instrument the SQL heartbeat call in `pg_worker.rs`

---

## Phase 3: Request Tracing & Correlation ✅ COMPLETE

### 3.1 Request ID middleware
- **File:** `src/dashboard/mod.rs`
- Generate UUID per request, inject into `tracing::Span`
- Add `x-request-id` response header
- All log lines within a request automatically include the ID

### 3.2 Worker correlation
- **File:** `src/pg_worker.rs`, `src/worker_client.rs`
- Add `worker_id` to all tracing spans in heartbeat/claim/report operations
- Include `worker_id` in `x-worker-id` header on API calls

### 3.3 Search job context
- Engine modules get a tracing span with `job_id`, `form`, `block_id`
- All log lines during a search block carry this context automatically

---

## Phase 4: Engine Instrumentation ✅ COMPLETE

### 4.1 Migrate engine `eprintln!` to tracing
- **Files:** `src/factorial.rs`, `src/kbn.rs`, `src/palindromic.rs`, etc. (all 12 forms)
- Search start → `info!(form, start, end, cores, "search starting")`
- Checkpoint save → `info!(n, sieved_out, "checkpoint saved")`
- Prime found → `info!(form, expression, digits, proof_method, "prime discovered")`
- Stop signal → `info!(n, "stop requested, checkpoint saved")`

### 4.2 Operation timing
- Add timing around sieve, primality test, and proof phases
- Log at debug level: `debug!(sieve_ms, candidates_after_sieve, "sieve complete")`
- Feed durations into Prometheus histograms (phase 2)

### 4.3 Sieve efficiency metrics
- Track: candidates_before_sieve, candidates_after_sieve, sieve_ratio
- New Prometheus gauge: `darkreach_sieve_efficiency_ratio` (Family with `form` label)
- Helps identify when sieve parameters need tuning

---

## Phase 5: Operational Polish ✅ COMPLETE

### 5.1 Metric catalog endpoint
- **Route:** `GET /api/observability/catalog`
- Returns all 17 metrics with name, type, unit, description, labels
- `MetricCatalogEntry` struct in `prom_metrics.rs`, `Metrics::catalog()` static method
- Frontend can consume dynamically instead of hardcoding

### 5.2 Alert threshold definitions
- SLO thresholds via env vars: `OBS_ERROR_BUDGET_ERRORS_PER_HOUR` (default 10), `OBS_ERROR_BUDGET_WARNINGS_PER_HOUR` (default 50)
- `/api/observability/report` evaluates against thresholds, returns `budget.status`: healthy/risk/breached
- Already implemented in Phase 4 work

### 5.3 Connection pool metrics
- Three new Prometheus gauges: `darkreach_db_pool_active`, `_idle`, `_max`
- Updated every 30s from sqlx `PgPool::size()` and `PgPool::num_idle()`
- Active computed as `pool_size - pool_idle`

### 5.4 WebSocket observability
- `darkreach_ws_connections_active` gauge: inc on connect, dec on disconnect
- `darkreach_ws_messages_sent_total` counter: inc on every successful message send
- `tracing::info!` on connect/disconnect with `active_connections` field
- All 3 send paths instrumented (initial, interval tick, notification relay)

---

## Phase 6: Log Infrastructure ✅ COMPLETE

### 6.1 Log levels in EventBus
- Already implemented: PrimeFound→`info!`, SearchStarted→`info!`, SearchCompleted→`info!`, Milestone→`info!`, Warning→`warn!`, Error→`error!`
- All structured with form, expression, digits, proof_method fields

### 6.2 Sensitive data protection
- Expression truncation: expressions > 1000 chars truncated in tracing output with `...(truncated)` suffix
- Full expression preserved in event records and WebSocket delivery
- `redact_database_url()` utility in `lib.rs`: replaces password with `***`
- Verified: no code path currently logs DATABASE_URL directly

### 6.3 Log sampling for high-volume paths
- Heartbeat success: `debug!` level (filtered by default `darkreach=info` env filter)
- Candidate test: never logged individually (only via atomic progress counters)
- Block claim: `info!` level (low volume, important for audit trail)

---

## File Impact Map

| File | Phase | Changes |
|------|-------|---------|
| `src/main.rs` | 1.1 | Wire EnvFilter |
| `src/dashboard/mod.rs` | 1.2, 1.3, 3.1, 5.3 | TraceLayer, structured logging, request IDs, pool metrics |
| `src/dashboard/websocket.rs` | 5.4 | WS connect/disconnect logging, message counting |
| `src/dashboard/routes_observability.rs` | 5.1 | Catalog endpoint |
| `src/dashboard/routes_operator.rs` | 2.2–2.4 | Heartbeat RTT, block duration, query timing |
| `src/events.rs` | 1.4, 6.2 | tracing macros, expression truncation |
| `src/progress.rs` | 1.5 | tracing::info |
| `src/pg_worker.rs` | 1.6, 2.4, 3.2, 6.3 | Fix silent error, heartbeat histogram, worker spans, debug sampling |
| `src/prom_metrics.rs` | 2.1–2.4, 5.1, 5.3–5.4 | Histograms, catalog, pool metrics, WS metrics |
| `src/lib.rs` | 6.2 | `redact_database_url()` utility |
| `src/factorial.rs` (×12 forms) | 4.1–4.2 | tracing macros, timing |
| `src/cli.rs` | 4.1 | tracing macros for operational logging |
| `src/worker_client.rs` | 4.1 | tracing macros |
| `src/proof.rs` | 4.1 | tracing macros |
| `src/checkpoint.rs` | 4.1 | tracing macros |
| `src/agent.rs` | 4.1 | tracing macros |
| `Cargo.toml` | 1.2, 3.1 | tower-http "trace" feature, uuid "v4" feature |

---

## Success Criteria ✅ ALL MET

- ✅ `RUST_LOG=darkreach=debug` shows structured key=value output on stderr
- ✅ `RUST_LOG=darkreach=warn` silences all info-level noise
- ✅ `LOG_FORMAT=json` produces parseable JSON lines (for CloudWatch/ELK)
- ✅ Every API request logged with method, path, status, latency_ms (via metrics_middleware)
- ✅ Grafana can display p50/p95/p99 request latency from `/metrics` (4 histogram metrics)
- ✅ Heartbeat failures are visible in logs with worker_id context
- ✅ A single request can be traced from API entry → DB query → response via request_id
- ✅ 200+ `eprintln!` calls migrated to structured `tracing` across 25+ files
- ✅ 17 Prometheus metrics registered (8 original + 4 histograms + 5 new gauges/counters)
- ✅ WebSocket connect/disconnect logged with active connection count
- ✅ Sensitive data protection: expression truncation + DATABASE_URL redaction utility
- ✅ 1,069 tests passing, 0 failures
