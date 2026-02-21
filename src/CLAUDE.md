# src/ — Engine & Server Domains

All Rust source code for darkreach, spanning algorithm modules (engine) and infrastructure (server).

## Directory Structure

```
src/
├── main.rs                    # Entry point, CLI argument parsing (clap)
├── cli.rs                     # CLI execution functions (search dispatch, work loop)
├── lib.rs                     # Module re-exports, small primes, trial division, MR pre-screening
│
├── [12 Search Forms]
├── factorial.rs               # n! ± 1
├── palindromic.rs             # Palindromic primes
├── kbn.rs                     # k·b^n ± 1 (Proth/LLR/Pocklington, BSGS)
├── near_repdigit.rs           # Near-repdigit palindromic
├── primorial.rs               # p# ± 1
├── cullen_woodall.rs          # n·2^n ± 1
├── wagstaff.rs                # (2^p+1)/3
├── carol_kynea.rs             # (2^n±1)²−2
├── twin.rs                    # Twin primes
├── sophie_germain.rs          # Sophie Germain primes
├── repunit.rs                 # R(b,n) = (b^n−1)/(b−1)
├── gen_fermat.rs              # b^(2^n)+1
│
├── [Core Primitives]
├── sieve.rs                   # Sieve, Montgomery mult, wheel factorization, BitSieve
├── proof.rs                   # Pocklington, Morrison, BLS proofs
├── verify.rs                  # 3-tier verification pipeline
├── certificate.rs             # PrimalityCertificate enum
├── p1.rs                      # Pollard P−1 factoring
│
├── [External Tool Integrations]
├── pfgw.rs                    # PFGW subprocess (50-100x speedup)
├── gwnum.rs                   # GWNUM FFI wrapper (feature-gated)
├── prst.rs                    # PRST subprocess for k·b^n±1
├── flint.rs                   # FLINT integration (feature-gated)
│
├── [Server Infrastructure]
├── dashboard/                 # Axum web server (15 route modules + WebSocket)
│   ├── mod.rs                 # Router setup, AppState, middleware, static file serving
│   ├── websocket.rs           # WebSocket handler (2s push interval)
│   ├── routes_agents.rs       # /api/agents/* — agent tasks, budgets, templates
│   ├── routes_docs.rs         # /api/docs/* — documentation serving
│   ├── routes_fleet.rs        # /api/fleet/* — fleet overview
│   ├── routes_health.rs       # /api/health — health check, readiness
│   ├── routes_jobs.rs         # /api/search_jobs/* — job CRUD, work blocks
│   ├── routes_notifications.rs # /api/notifications/* — push notifications
│   ├── routes_observability.rs # /api/observability/* — metrics, logs, charts
│   ├── routes_projects.rs     # /api/projects/* — project management
│   ├── routes_releases.rs     # /api/releases/* — worker release channels
│   ├── routes_searches.rs     # /api/searches/* — search management
│   ├── routes_status.rs       # /api/status — coordinator status
│   ├── routes_verify.rs       # /api/verify — prime re-verification
│   ├── routes_volunteer.rs    # /api/volunteer/* — volunteer worker management
│   └── routes_workers.rs      # /api/workers/* — worker heartbeat, registration
│
├── db/                        # PostgreSQL via sqlx (Supabase)
│   ├── mod.rs                 # Database struct, connection pool, PrimeRecord types
│   ├── primes.rs              # Prime record CRUD (insert, query, filter, verify)
│   ├── workers.rs             # Worker heartbeat, registration, pruning
│   ├── jobs.rs                # Search job lifecycle, work block coordination
│   ├── agents.rs              # Agent tasks, events, budgets, templates
│   ├── memory.rs              # Agent memory key-value store
│   ├── roles.rs               # Agent role configuration
│   ├── schedules.rs           # Agent schedule automation
│   ├── projects.rs            # Multi-phase project management
│   ├── calibrations.rs        # Cost model calibration coefficients
│   ├── records.rs             # World record tracking
│   ├── observability.rs       # Metrics, logs, worker rates
│   ├── releases.rs            # Worker release channels, adoption tracking
│   └── volunteers.rs          # Volunteer worker registration, capabilities
│
├── project/                   # Campaign-style discovery management
│   ├── mod.rs                 # Module re-exports
│   ├── types.rs               # Database row types (projects, phases, records, events)
│   ├── config.rs              # TOML configuration structs, parsing, validation
│   ├── cost.rs                # Power-law cost estimation model
│   ├── orchestration.rs       # Phase state machine, auto-strategy, 30s tick loop
│   ├── records.rs             # World record tracking via t5k.org scraping
│   └── tests.rs               # Unit tests for project module
│
├── [Other Server Modules]
├── checkpoint.rs              # JSON checkpoint save/load (all 12 form variants)
├── search_manager.rs          # Search job lifecycle, block generation
├── agent.rs                   # AI agent infrastructure
├── fleet.rs                   # In-memory worker registry (60s stale timeout)
├── pg_worker.rs               # PostgreSQL work claiming (FOR UPDATE SKIP LOCKED)
├── worker_client.rs           # HTTP client for workers → coordinator
├── deploy.rs                  # SSH deployment, service management, rolling updates
├── events.rs                  # Event bus (prime notifications, search status)
├── metrics.rs                 # System metrics (CPU, memory, disk)
├── prom_metrics.rs            # Prometheus metric export
├── volunteer.rs               # Volunteer worker management
└── progress.rs                # Atomic counters, background 30s reporter
```

## Engine Domain

### Search form pipeline

Every search form follows the same pattern:

```
sieve candidates → parallel primality test (rayon) → deterministic proof → log to PostgreSQL
```

Each form's `search()` function:
1. Loads checkpoint (if resuming)
2. Generates candidates for the range
3. Sieves out composites (form-specific sieve)
4. Tests survivors in parallel via `rayon::par_iter`
5. Attempts deterministic proof (form-specific)
6. Logs primes via `db.insert_prime_sync(rt, ...)`
7. Saves checkpoint every 60 seconds
8. Checks `worker_client.is_stop_requested()` each block

### Key patterns

- **rug/GMP**: All arbitrary-precision arithmetic via `rug::Integer`. Primality via `is_probably_prime(25)`.
- **rayon**: `par_iter` for batches/blocks, `rayon::join` for dual tests (e.g., n!+1 and n!-1).
- **PFGW acceleration**: Large candidates routed to PFGW subprocess for 50-100x speedup.
- **kbn::test_prime**: `pub(crate)`, returns `(IsPrime, &str, Option<PrimalityCertificate>)`. Reused by twin, sophie_germain, cullen_woodall, carol_kynea, gen_fermat.
- **Proofs**: Proth (N−1), LLR (N+1 Lucas), Pocklington, Morrison, BLS — form-specific.
- **No unsafe** except gwnum-sys FFI (feature-gated).

### PFGW/GWNUM tool selection

| Form | Primary | Fallback |
|------|---------|----------|
| kbn k·b^n±1 | PRST / GWNUM | GMP Proth/LLR |
| Factorial n!±1 | PFGW -tp/-tm | GMP MR + Pocklington/Morrison |
| Primorial p#±1 | PFGW -tp/-tm | GMP MR + Pocklington/Morrison |
| Wagstaff (2^p+1)/3 | GWNUM Vrba-Reix / PFGW | GMP MR (PRP only) |
| Palindromic | PFGW | GMP MR |
| Near-repdigit | PFGW PRP | GMP MR + BLS |
| Cullen/Woodall | PFGW PRP | GMP Proth/LLR |
| Carol/Kynea | PFGW PRP | GMP LLR |
| Gen Fermat | PFGW PRP | GMP Pépin/Proth |
| Repunit | PFGW PRP | GMP MR |

### Conventions

- Even-digit palindromes skipped (always divisible by base+1).
- Results classified as "deterministic" (proven) or "probabilistic" (PRP).
- `has_small_factor()` uses 64 hardcoded primes, compare via `*n != p` (avoids heap alloc).
- `checked_u32()` in `lib.rs`: always use instead of `n as u32` for `.pow()` / `<<`.
- Wagstaff: no deterministic proof exists — results always PRP.

## Server Domain

### Axum API (dashboard/)

Dashboard runs on configurable port (default 7001). Route modules are organized by domain:

| Route module | Base path | Key endpoints |
|-------------|-----------|---------------|
| `routes_health` | `/api/health` | Health check, readiness |
| `routes_status` | `/api/status` | Coordinator status summary |
| `routes_workers` | `/api/workers` | Worker CRUD, heartbeat, list |
| `routes_fleet` | `/api/fleet` | Fleet overview (workers + searches) |
| `routes_jobs` | `/api/search_jobs` | Job CRUD, work blocks, status |
| `routes_searches` | `/api/searches` | Search management |
| `routes_verify` | `/api/verify` | Prime re-verification |
| `routes_agents` | `/api/agents` | Agent tasks, budgets, memory, roles |
| `routes_projects` | `/api/projects` | Project CRUD, phases, events |
| `routes_docs` | `/api/docs` | Documentation list + content |
| `routes_notifications` | `/api/notifications` | Push notification management |
| `routes_observability` | `/api/observability` | Metrics, logs, charts |
| `routes_releases` | `/api/releases` | Worker release channels |
| `routes_volunteer` | `/api/volunteer` | Volunteer worker management |
| `websocket` | `/ws` | Real-time push (2s interval) |

### Database (db/)

PostgreSQL via `sqlx::PgPool` connecting to Supabase. Operations split by domain:
- `insert_prime_sync(rt, ...)`: Bridge for rayon threads (7 args including certificate)
- Each submodule maps to a set of tables (see `supabase/CLAUDE.md`)
- Public re-exports from `mod.rs`: `MetricPoint`, `MetricSeries`, `WorkerRelease*` types

### Fleet coordination

- Workers register via HTTP heartbeat (10s) or PostgreSQL polling
- `fleet.rs`: in-memory registry, stale pruning (60s)
- `pg_worker.rs`: `FOR UPDATE SKIP LOCKED` for work block claiming
- `worker_client.rs`: HTTP client (register/heartbeat/report/stop-check)
- Two coordination modes: `--coordinator <url>` (HTTP) or `DATABASE_URL` (PG direct)

### Key dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `axum` | 0.8 | HTTP server + WebSocket |
| `tokio` | 1 | Async runtime |
| `sqlx` | 0.8 | PostgreSQL (async, compile-time checked) |
| `ureq` | 3 | Blocking HTTP client for workers |
| `rug` | 1 | GMP bindings (arbitrary precision) |
| `rayon` | 1 | Data parallelism |
| `clap` | 4 | CLI argument parsing |
| `serde` / `serde_json` | 1 | Serialization |

## Agent Coding Guide

### Adding a new search form

1. Create `src/<form>.rs` with the standard pipeline:
   - `pub fn search(...)` taking db, runtime, progress, checkpoint, worker_client args
   - Form-specific sieve function
   - Primality test (reuse `kbn::test_prime` if applicable)
   - Proof attempt (form-specific)
   - Log via `db.insert_prime_sync(rt, expression, form, digits, proof_method, params_json, certificate)`
2. Add checkpoint variant to `CheckpointData` enum in `checkpoint.rs`
3. Add CLI subcommand in `main.rs` (`Commands` enum) + dispatch in `cli.rs` (`run_search`)
4. Add `pub mod <form>;` in `lib.rs`
5. Update `search_manager.rs` with new form's block generation
6. Update `deploy.rs` with new form's deployment support
7. Add tests (unit in file, integration in `tests/`)

### Adding a new API route

1. Create handler function in appropriate `src/dashboard/routes_*.rs`
2. If new domain, create `src/dashboard/routes_<domain>.rs` and add `mod` in `dashboard/mod.rs`
3. Register route in router builder in `dashboard/mod.rs`
4. Add DB query method in appropriate `src/db/<domain>.rs`
5. If new DB domain, create submodule and add `mod` in `db/mod.rs`

### Adding a new DB module

1. Create `src/db/<domain>.rs` with `impl Database { ... }` methods
2. Add `mod <domain>;` in `src/db/mod.rs`
3. Add `pub use` if types need to be visible outside db module
4. Create migration in `supabase/migrations/NNN_<description>.sql`

### Key gotchas

- `rug` lazy types (`&Integer ± u32` = `SubU32Incomplete`): wrap in `Integer::from()` before calling methods
- `proth_test` must skip bases where `a % p == 0` (Jacobi symbol undefined)
- Sieve only safe when candidate > sieve_limit (check `sieve_min_n` / `sieve_min_prime` guards)
- All 12 forms must check `worker_client.is_stop_requested()` in their block loop
- Carol prime indices (OEIS A091515): 2,3,4,6,7,10,12,15,18,19,21,25,27 (NOT 17,22,26!)

## Testing

```bash
cargo test                              # All unit tests
cargo test --test db_integration        # Database integration
cargo test --test api_integration       # API integration
cargo test --test cli_tests             # CLI smoke tests
cargo test --test property_tests        # Property-based tests
cargo test --test security_tests        # Security tests
cargo bench                             # All benchmarks
```

## Roadmaps

- Engine: `docs/roadmaps/engine.md`
- Server: `docs/roadmaps/server.md`
- Agents: `docs/roadmaps/agents.md`
- Fleet: `docs/roadmaps/fleet.md`
- Projects: `docs/roadmaps/projects.md`
- GWNUM/FLINT: `docs/roadmaps/gwnum-flint.md`
- Testing: `docs/roadmaps/testing.md`
