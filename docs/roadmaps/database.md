# Database Infrastructure Roadmap

Scaling PostgreSQL from hosted Supabase to a production-grade, self-managed database tier capable of supporting hundreds of compute nodes, sub-second dashboard queries, and years of time-series retention.

**Key files:** `src/db/`, `supabase/migrations/`, `src/dashboard/`

**See also:** [architecture.md](architecture.md) for the master migration plan, [network.md](network.md) for distributed compute coordination, [ops.md](ops.md) for deployment infrastructure.

---

## Current State

**Platform:** Hosted Supabase (managed PostgreSQL 15, PgBouncer, PostgREST, Realtime)

**What works well:**
- 29 migrations, fully portable standard SQL
- Row-Level Security on all tables
- Supabase Realtime for live prime notifications to the frontend
- Supabase JS client for frontend data queries and auth
- `FOR UPDATE SKIP LOCKED` work claiming in `src/pg_worker.rs`
- sqlx compile-time checked queries in `src/db/`

**What's limited:**
- **Connection pool hardcoded to 2** (`src/db/mod.rs:437`) — no env var override
- **No TimescaleDB** — `metric_samples` and `system_logs` grow unbounded without hypertables
- **No WAL/vacuum tuning** — Supabase doesn't expose `postgresql.conf`
- **Heartbeat write storm** — every node heartbeats every 10s via `INSERT ... ON CONFLICT UPDATE`, the highest-frequency write in the system, polluting a transactional database with ephemeral data
- **Single region** — Supabase project is in one region, adding latency for geographically distributed nodes
- **No read replicas** — dashboard queries and node coordination share the same pool
- **Manual rollups** — `src/db/observability.rs` implements manual daily rollup functions that TimescaleDB continuous aggregates would handle automatically
- **Frontend coupled to Supabase** — the dashboard imports `@supabase/supabase-js` directly, making migration harder

---

## Phase 1: Optimize Supabase

Quick wins without any migration. Estimated effort: 1-2 days.

### 1.1 Configurable Connection Pool

**Problem:** Pool size is hardcoded to 2 in `src/db/mod.rs:437`.

**Solution:** Add `DB_MAX_CONNECTIONS` env var with sensible default.

```rust
let max_conn = std::env::var("DB_MAX_CONNECTIONS")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(5);

let pool = PgPoolOptions::new()
    .max_connections(max_conn)
    .connect_with(opts)
    .await?;
```

**Files:** `src/db/mod.rs`

### 1.2 Supavisor Connection Pooler

**Problem:** Direct connections to Supabase hit the 60-connection limit quickly with multiple nodes.

**Solution:** Switch `DATABASE_URL` to Supabase's Supavisor pooler endpoint (port 6543, transaction mode). This multiplexes many clients onto fewer server connections.

**Gotcha:** Transaction-mode pooling breaks prepared statements — sqlx must use `statement_cache_capacity(0)` (already set in `src/db/mod.rs:432`).

**Files:** `.env`, `deploy/` service configs

### 1.3 Materialized Views for Dashboard

**Problem:** Dashboard stats queries (`get_stats`, `form_leaderboard`, `get_digit_distribution`) scan full tables on every page load.

**Solution:** Create materialized views refreshed on a schedule (every 5 minutes via pg_cron or application-side `REFRESH MATERIALIZED VIEW CONCURRENTLY`).

**Candidates:**
- `dashboard_stats` — total primes, forms, max digits (replaces `get_stats()` RPC)
- `form_leaderboard_mv` — pre-computed form rankings (replaces `form_leaderboard()` RPC)
- `digit_distribution_mv` — digit histogram (replaces `get_digit_distribution()` RPC)

**Files:** New migration `supabase/migrations/030_materialized_views.sql`, `src/db/primes.rs`

### 1.4 Partition metric_samples by Time

**Problem:** `system_metrics` table grows linearly. Queries for "last hour" scan the entire table.

**Solution:** Convert to range-partitioned table on `recorded_at` (weekly partitions). Drop partitions older than retention window.

**Note:** Supabase supports declarative partitioning in standard PostgreSQL. No extensions needed.

**Files:** New migration, `src/db/observability.rs`

### Acceptance Criteria

- [ ] `DB_MAX_CONNECTIONS` env var works, defaults to 5
- [ ] Dashboard stats load from materialized views (< 50ms)
- [ ] `metric_samples` partitioned, old partitions auto-dropped

---

## Phase 2: Redis for Hot-Path Data

Decouple ephemeral state from the persistent database. Estimated effort: 2-3 days.

### 2.1 Worker Heartbeats → Redis

**Problem:** Node heartbeats (every 10s per node) are the highest-frequency write in the system. With 50 nodes, that's 5 writes/second to PostgreSQL — wasteful for data that expires in 60 seconds.

**Solution:** Store heartbeats in Redis with automatic TTL expiry.

```
SET worker:{worker_id} {json_payload} EX 60
```

**Benefits:**
- Removes ~90% of PostgreSQL write load
- No manual stale-worker pruning needed — TTL handles it
- Sub-millisecond reads for fleet status

**Fleet status query:** `SCAN worker:*` to list active nodes.

**Pending commands:** `SET worker:{id}:cmd {command}` with short TTL. Worker reads and deletes atomically with `GETDEL`.

### 2.2 Rate Limiting & Session Cache

Store API rate limit counters and session tokens in Redis. Removes need for per-request database queries on authenticated endpoints.

### 2.3 Infrastructure

**Option A:** Small Hetzner VPS (CX22, ~$4/mo) running Redis 7 with AOF persistence.
**Option B:** Redis sidecar on the coordinator (CX22 has enough RAM for a small Redis instance).
**Recommendation:** Option B initially — avoids extra server. Migrate to dedicated VPS when coordinator runs out of RAM.

**Rust crate:** `redis` (async, connection pooling, TLS support).

### File Impact

| File | Change |
|------|--------|
| `Cargo.toml` | Add `redis` dependency |
| `src/db/mod.rs` | Add `RedisPool` alongside `PgPool` |
| `src/db/workers.rs` | Heartbeat writes to Redis instead of PG |
| `src/dashboard/routes_workers.rs` | Read fleet status from Redis |
| `src/fleet.rs` | Query Redis for active nodes |
| `src/pg_worker.rs` | Work claiming stays in PG (transactional) |

### Acceptance Criteria

- [ ] Worker heartbeats stored in Redis with 60s TTL
- [ ] Fleet status page reads from Redis (< 5ms)
- [ ] PostgreSQL write rate drops by ~90%
- [ ] Work claiming (`FOR UPDATE SKIP LOCKED`) still uses PostgreSQL

---

## Phase 3: Self-Hosted PostgreSQL + TimescaleDB

Full control over configuration, extensions, and performance tuning. Estimated effort: 1-2 weeks.

### 3.1 Dedicated Database Server

**Hardware:** Hetzner CX32 (4 vCPU, 8 GB RAM, 80 GB NVMe, ~$15/mo) or CX42 (8 vCPU, 16 GB RAM, 160 GB NVMe, ~$30/mo).

**Why not the coordinator?** The CX22 coordinator (2 vCPU, 4 GB RAM) is too small to co-host PostgreSQL. Database needs dedicated CPU for vacuum, dedicated RAM for shared_buffers, and dedicated I/O for WAL writes.

**Software:** PostgreSQL 16 + TimescaleDB 2.x + pg_stat_statements.

### 3.2 TimescaleDB Hypertables

Convert time-series tables to TimescaleDB hypertables for automatic partitioning, compression, and continuous aggregates.

**Target tables:**

| Table | Chunk interval | Compression after | Retention |
|-------|---------------|-------------------|-----------|
| `system_metrics` | 1 day | 7 days | 90 days raw, aggregates forever |
| `system_logs` | 1 day | 3 days | 30 days |
| `metric_rollups_daily` | 1 month | never | forever |

### 3.3 Continuous Aggregates

Replace the manual rollup functions in `src/db/observability.rs` with TimescaleDB continuous aggregates.

**Current (manual):**
```sql
-- Called by application code every 24h
SELECT compute_daily_rollups();
```

**Target (automatic):**
```sql
CREATE MATERIALIZED VIEW metric_hourly
WITH (timescaledb.continuous) AS
SELECT time_bucket('1 hour', recorded_at) AS bucket,
       metric_name,
       avg(value) AS avg_val,
       max(value) AS max_val,
       min(value) AS min_val
FROM system_metrics
GROUP BY bucket, metric_name;

SELECT add_continuous_aggregate_policy('metric_hourly',
  start_offset => INTERVAL '3 hours',
  end_offset => INTERVAL '1 hour',
  schedule_interval => INTERVAL '1 hour');
```

### 3.4 PostgreSQL Tuning

```ini
# postgresql.conf — tuned for CX32 (8 GB RAM)
shared_buffers = 2GB              # 25% of RAM
effective_cache_size = 6GB        # 75% of RAM
work_mem = 64MB                   # per-sort/hash operation
maintenance_work_mem = 512MB      # for VACUUM, CREATE INDEX
wal_buffers = 64MB
max_connections = 100
checkpoint_completion_target = 0.9
random_page_cost = 1.1            # NVMe SSD

# Autovacuum tuning for high-write tables
autovacuum_vacuum_scale_factor = 0.05    # vacuum at 5% dead tuples (default 20%)
autovacuum_analyze_scale_factor = 0.02   # analyze at 2% changes (default 10%)
```

### 3.5 Migration from Supabase

1. `pg_dump` from Supabase (schema + data)
2. Create TimescaleDB hypertables on target
3. `pg_restore` data into new server
4. Convert time-series tables to hypertables
5. Update `DATABASE_URL` in all services
6. Verify all 29 migrations applied correctly
7. Run integration tests against new database
8. Cut over: update coordinator + all nodes simultaneously

**Rollback:** Keep Supabase project active for 30 days. If issues arise, revert `DATABASE_URL`.

### File Impact

| File | Change |
|------|--------|
| `src/db/observability.rs` | Remove manual rollup code, use continuous aggregates |
| `src/db/mod.rs` | Connection pool tuning for self-hosted |
| `deploy/` | Add PostgreSQL systemd unit, backup scripts |
| `supabase/migrations/` | TimescaleDB-specific migration (hypertables, policies) |

### Acceptance Criteria

- [ ] PostgreSQL 16 + TimescaleDB running on dedicated server
- [ ] `system_metrics` as hypertable with automatic compression and retention
- [ ] Continuous aggregates replace manual rollup functions
- [ ] All existing migrations apply cleanly
- [ ] Integration tests pass against new database
- [ ] Automated daily backups to object storage

---

## Phase 4: Read Replicas

Scale dashboard and analytics reads independently of the write path. Estimated effort: 3-5 days.

### 4.1 Streaming Replication

Set up one read replica using PostgreSQL streaming replication. The replica receives WAL records from the primary and applies them asynchronously (< 1s lag).

### 4.2 Dual Connection Pool

```rust
pub struct Database {
    write_pool: PgPool,   // Primary — all writes
    read_pool: PgPool,    // Replica — dashboard queries, analytics
}
```

**Routing rules:**
- All `INSERT`, `UPDATE`, `DELETE` → `write_pool`
- Dashboard stats, leaderboard, charts → `read_pool`
- Work claiming (`FOR UPDATE SKIP LOCKED`) → `write_pool`
- Agent task queries → `read_pool`

**Env vars:**
- `DATABASE_URL` — primary (read-write)
- `REPLICA_DATABASE_URL` — replica (read-only), optional

When `REPLICA_DATABASE_URL` is not set, both pools point to the primary (backward compatible).

### File Impact

| File | Change |
|------|--------|
| `src/db/mod.rs` | Dual pool initialization, `read_pool()` accessor |
| `src/db/primes.rs` | Stats/leaderboard queries use read pool |
| `src/db/observability.rs` | Metric queries use read pool |
| `src/db/agents.rs` | Agent listing uses read pool |
| `src/dashboard/routes_status.rs` | Status queries use read pool |

### Acceptance Criteria

- [ ] Streaming replication configured with < 1s lag
- [ ] Dashboard queries routed to read replica
- [ ] Write operations still go to primary
- [ ] Falls back to primary when no replica configured

---

## Phase 5: High Availability

Production-grade database infrastructure. Estimated effort: 1-2 weeks.

### 5.1 Automatic Failover

Use Patroni or repmgr for automatic primary failover. If the primary dies, the replica promotes itself within 30 seconds.

**Components:**
- etcd or Consul for leader election
- Patroni agent on each PostgreSQL node
- HAProxy or PgBouncer VIP for transparent failover

### 5.2 PgBouncer Connection Multiplexing

Replace direct pool connections with PgBouncer in transaction mode. This allows hundreds of application connections to share a small number of actual PostgreSQL connections.

```
[darkreach]
host = 127.0.0.1
port = 5432
pool_mode = transaction
default_pool_size = 20
max_client_conn = 200
```

### 5.3 Monitoring

- `pg_stat_statements` for slow query identification
- `pg_stat_user_tables` for table bloat and vacuum lag
- Prometheus exporter (`postgres_exporter`) feeding existing Grafana dashboards
- Alerting on replication lag > 5s, connection pool exhaustion, long-running queries

### Acceptance Criteria

- [ ] Automatic failover completes within 30 seconds
- [ ] Zero-downtime primary maintenance via switchover
- [ ] PgBouncer handles connection multiplexing
- [ ] pg_stat_statements enabled, slow queries visible in Grafana

---

## Phase 6: Frontend Independence

Remove the frontend's direct dependency on Supabase, routing all data through the REST API. Estimated effort: 1-2 weeks.

### 6.1 Migrate Frontend Data Fetching

**Current:** Frontend uses `@supabase/supabase-js` to query `primes`, `agent_tasks`, `projects` tables directly.

**Target:** All data flows through the Axum REST API (`/api/*` routes). The frontend becomes a pure consumer of the darkreach API.

**Migration path per hook:**
1. Identify Supabase query in `frontend/src/hooks/use-*.ts`
2. Ensure equivalent API endpoint exists in `src/dashboard/routes_*.rs`
3. Replace `supabase.from('table').select(...)` with `fetch('/api/...')`
4. Remove `@supabase/supabase-js` from frontend dependencies when complete

### 6.2 Auth Decision

**Option A:** Keep Supabase Auth — it handles OAuth, password resets, session management. The frontend talks to Supabase only for auth, everything else through the API.

**Option B:** Self-hosted auth (e.g., `axum-login` + `argon2` + JWT). Full independence from Supabase but significant implementation effort.

**Recommendation:** Option A initially. Auth is the hardest part to self-host correctly.

### 6.3 Live Notifications

**Current:** Supabase Realtime for `primes` table changes → live prime notifications.

**Target:** WebSocket push from the Axum server (already implemented in `src/dashboard/websocket.rs`). Extend the existing 2s push interval to include prime notifications alongside fleet status.

### File Impact

| File | Change |
|------|--------|
| `frontend/src/hooks/use-*.ts` | Replace Supabase queries with API fetch calls |
| `frontend/src/lib/supabase.ts` | Reduce to auth-only (or remove entirely) |
| `frontend/package.json` | Remove `@supabase/supabase-js` (if fully migrated) |
| `src/dashboard/websocket.rs` | Add prime notification events |
| `src/dashboard/routes_*.rs` | Ensure all frontend queries have API equivalents |

### Acceptance Criteria

- [ ] Frontend fetches all data from `/api/*` endpoints
- [ ] No direct Supabase table queries from frontend code
- [ ] Live prime notifications via WebSocket (not Supabase Realtime)
- [ ] Auth continues to work (Supabase Auth or self-hosted)

---

## Hardware Recommendations

| Phase | Server | Specs | Cost/mo | Purpose |
|-------|--------|-------|---------|---------|
| 1 | — | Hosted Supabase | $0-25 | Current state, optimize in place |
| 2 | Coordinator sidecar | Redis on CX22 | $0 | Heartbeat offload |
| 3 | Hetzner CX32 | 4 vCPU, 8 GB, 80 GB NVMe | ~$15 | Primary PG + TimescaleDB |
| 4 | Hetzner CX22 | 2 vCPU, 4 GB, 40 GB NVMe | ~$4 | Read replica |
| 5 | 2x CX32 + CX22 | Primary + standby + PgBouncer | ~$34 | HA cluster |

---

## Architecture Diagram (Target State)

```
                          ┌──────────────┐
                          │   Frontend   │
                          │  (Next.js)   │
                          └──────┬───────┘
                                 │ REST API + WebSocket
                          ┌──────▼───────┐
                          │ Coordinator  │
                          │  (Axum)      │
                          │  CX22        │
                          └──┬───┬───┬───┘
                             │   │   │
              ┌──────────────┘   │   └──────────────┐
              │                  │                  │
       ┌──────▼──────┐   ┌──────▼──────┐   ┌───────▼──────┐
       │   Redis     │   │ PgBouncer   │   │   Nodes      │
       │  (sidecar)  │   │  CX22       │   │  (N workers) │
       │             │   └──────┬──────┘   └──────────────┘
       │ Heartbeats  │          │
       │ Sessions    │   ┌──────▼──────┐
       │ Rate limits │   │ PostgreSQL  │
       └─────────────┘   │ + TimescaleDB│
                         │   Primary   │
                         │   CX32      │
                         └──────┬──────┘
                                │ Streaming replication
                         ┌──────▼──────┐
                         │ PostgreSQL  │
                         │   Replica   │
                         │   CX22      │
                         └─────────────┘
```

---

## Data Classification

Which store is optimal for each data type:

| Data | Current Store | Optimal Store | Rationale |
|------|--------------|---------------|-----------|
| Prime records | PostgreSQL | PostgreSQL | Permanent, relational, needs transactions |
| Search jobs / work blocks | PostgreSQL | PostgreSQL | Transactional (`FOR UPDATE SKIP LOCKED`) |
| Worker heartbeats | PostgreSQL | **Redis** | Ephemeral (60s TTL), highest write frequency |
| Worker pending commands | PostgreSQL | **Redis** | Ephemeral, read-once-delete |
| System metrics | PostgreSQL | **TimescaleDB** | Time-series, needs compression + retention |
| System logs | PostgreSQL | **TimescaleDB** | Time-series, needs retention policies |
| Metric rollups | PostgreSQL | **TimescaleDB** | Continuous aggregates replace manual rollups |
| Agent tasks | PostgreSQL | PostgreSQL | Relational, needs transactions |
| Agent memory | PostgreSQL | PostgreSQL | Persistent key-value (low volume) |
| Projects / phases | PostgreSQL | PostgreSQL | Relational, needs referential integrity |
| API rate limits | None | **Redis** | Counter with TTL, high frequency |
| Session tokens | Supabase Auth | Supabase Auth / **Redis** | Depends on auth decision |
| Dashboard stats cache | None | **Redis** | Computed values, refresh every 5 min |

---

## Success Metrics

| Metric | Current | Phase 1 | Phase 2 | Phase 3 | Phase 4 |
|--------|---------|---------|---------|---------|---------|
| Dashboard page load (p95) | ~500ms | < 200ms | < 200ms | < 100ms | < 50ms |
| PG writes/sec (50 nodes) | ~5/s | ~5/s | < 0.5/s | < 0.5/s | < 0.5/s |
| Connection pool utilization | 100% (2 conns) | ~30% | ~20% | ~15% | ~10% |
| Metric query (30 days) | ~2s | ~500ms | ~500ms | < 100ms | < 50ms |
| Max supported nodes | ~20 | ~50 | ~200 | ~500 | ~1000 |
| Database cost/mo | $0-25 | $0-25 | $0-25 | ~$15 | ~$19 |

---

## File Impact Map

Summary of all files affected across all phases:

| File | Phase | Change |
|------|-------|--------|
| `src/db/mod.rs` | 1, 3, 4 | Configurable pool size, dual pool, TimescaleDB compat |
| `src/db/workers.rs` | 2 | Heartbeat writes to Redis |
| `src/db/observability.rs` | 1, 3 | Materialized views, then continuous aggregates |
| `src/db/primes.rs` | 1, 4 | Materialized views for stats, read replica routing |
| `src/fleet.rs` | 2 | Query Redis for active nodes |
| `src/pg_worker.rs` | — | No change (transactional work claiming stays in PG) |
| `src/dashboard/routes_workers.rs` | 2 | Read fleet status from Redis |
| `src/dashboard/routes_status.rs` | 4 | Route stats queries to read replica |
| `src/dashboard/websocket.rs` | 6 | Add prime notification events |
| `Cargo.toml` | 2 | Add `redis` crate |
| `deploy/` | 3, 5 | PostgreSQL systemd units, backup scripts, PgBouncer config |
| `supabase/migrations/` | 1, 3 | Materialized views, TimescaleDB hypertables |
| `frontend/src/hooks/` | 6 | Replace Supabase queries with API fetches |
| `frontend/src/lib/supabase.ts` | 6 | Reduce to auth-only or remove |
| `.env` / service configs | 1, 2, 4 | `DB_MAX_CONNECTIONS`, `REDIS_URL`, `REPLICA_DATABASE_URL` |
