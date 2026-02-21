# CLAUDE.md

## Project Overview

`darkreach` is a Rust CLI tool and distributed platform for hunting special-form prime numbers. It uses GMP (via `rug`) for arbitrary-precision arithmetic, `rayon` for parallel search, and coordinates fleets of workers via PostgreSQL (Supabase). The web dashboard is a Next.js 16 static export served by the Rust backend.

**Domain:** `darkreach.ai` | **Dashboard:** `app.darkreach.ai` | **API:** `api.darkreach.ai`

## Domain Map

| Domain | Directory | CLAUDE.md | Roadmap |
|--------|-----------|-----------|---------|
| **Engine** | `src/` (algorithm files) | `src/CLAUDE.md` | `docs/roadmaps/engine.md` |
| **Server** | `src/` (infra files) | `src/CLAUDE.md` | `docs/roadmaps/server.md` |
| **Frontend** | `frontend/` | `frontend/CLAUDE.md` | `docs/roadmaps/frontend.md` |
| **Ops** | `deploy/` | `deploy/CLAUDE.md` | `docs/roadmaps/ops.md` |
| **Research** | `docs/` | `docs/CLAUDE.md` | `docs/roadmaps/research.md` |
| **Database** | `supabase/`, `src/db/` | `supabase/CLAUDE.md` | `docs/roadmaps/database.md` |
| **Tests** | `tests/` + `benches/` | `tests/CLAUDE.md` | `docs/roadmaps/testing.md` |
| **AI Engine** | `src/ai_engine.rs`, `src/strategy.rs`, `src/agent.rs` | `src/CLAUDE.md` | `docs/roadmaps/ai-engine.md` |
| **Agents** | `src/agent.rs` + frontend | `src/CLAUDE.md` | `docs/roadmaps/agents.md` |
| **Projects** | `src/project/` + frontend | `src/CLAUDE.md` | `docs/roadmaps/projects.md` |
| **Network** | `src/fleet.rs`, `src/pg_worker.rs` | `src/CLAUDE.md` | `docs/roadmaps/network.md` |

## Slash Commands

| Command | Purpose |
|---------|---------|
| `/engine` | Work on prime-hunting algorithms |
| `/dashboard` | Work on the web frontend |
| `/deploy` | Handle deployment tasks |
| `/research` | Research prime forms and strategies |
| `/hunt` | Plan and execute a prime discovery campaign |

## Build

Requires GMP installed on the system:
- Linux: `apt install build-essential libgmp-dev m4`
- macOS: `brew install gmp`

```bash
cargo build --release
```

Frontend:
```bash
cd frontend && npm install && npm run build
```

## Architecture

### Engine (12 search forms + core primitives)

**Search forms** — each implements sieve → parallel test → proof → log pipeline:
- `src/factorial.rs` — n! ± 1 (GMP factorial, modular sieve, PFGW -tp/-tm proofs)
- `src/palindromic.rs` — Palindromic primes (batch generation, deep sieve, parallel testing)
- `src/kbn.rs` — k·b^n ± 1 (Proth/LLR/Pocklington, BSGS sieve) — **reused by 5 other forms**
- `src/near_repdigit.rs` — Near-repdigit palindromic (BLS N+1 proofs, PFGW)
- `src/primorial.rs` — p# ± 1 (Pocklington/Morrison proofs, PFGW)
- `src/cullen_woodall.rs` — n·2^n ± 1 (Proth/LLR, PFGW)
- `src/wagstaff.rs` — (2^p+1)/3 (multiplicative order sieve, PFGW/GWNUM)
- `src/carol_kynea.rs` — (2^n±1)²−2 (LLR, PFGW)
- `src/twin.rs` — Twin primes (quad sieve, Proth+LLR intersection)
- `src/sophie_germain.rs` — Sophie Germain (Proth+LLR intersection sieve)
- `src/repunit.rs` — R(b,n) = (b^n−1)/(b−1) (PFGW)
- `src/gen_fermat.rs` — b^(2^n)+1 (Pépin/Proth, PFGW)

**Core primitives:**
- `src/sieve.rs` — Sieve of Eratosthenes, Montgomery multiplication, wheel factorization, BitSieve
- `src/lib.rs` — Module re-exports, small primes table, trial division, MR pre-screening, Frobenius test
- `src/proof.rs` — Pocklington (N−1), Morrison (N+1), BLS deterministic proofs
- `src/verify.rs` — 3-tier verification pipeline (deterministic → BPSW+MR → PFGW)
- `src/certificate.rs` — PrimalityCertificate enum
- `src/p1.rs` — Pollard P−1 factoring for deep composite elimination

**External tool integrations:**
- `src/pfgw.rs` — PFGW subprocess (50-100x acceleration for large candidates)
- `src/gwnum.rs` — GWNUM FFI safe wrapper (feature-gated)
- `src/prst.rs` — PRST subprocess for k·b^n±1 forms
- `src/flint.rs` — FLINT integration (feature-gated)

### AI Engine (`src/ai_engine.rs`)
- Unified OODA decision loop: Observe → Orient → Decide → Act → Learn
- Replaces independent `strategy_tick()` + `orchestrate_tick()` with single `AiEngine::tick()`
- `WorldSnapshot`: single consistent view assembled via parallel DB queries (~50ms)
- 7-component scoring model: record_gap, yield_rate, cost_efficiency, opportunity_density, fleet_fit, momentum, competition
- `CostModel`: data-fitted power-law coefficients via OLS on log-log work block data, falls back to hardcoded defaults
- `ScoringWeights`: learned via online gradient descent, persisted in `ai_engine_state`
- Drift detection: compares consecutive snapshots for worker changes, discoveries, stalls, budget velocity
- Safety checks: budget gates, concurrency limits, stall penalties
- Decision audit trail: `ai_engine_decisions` table with reasoning, confidence, outcome tracking

### Server (modular directories)
- `src/main.rs` + `src/cli.rs` — CLI routing with clap (12 search + dashboard/verify/deploy/work/project)
- `src/dashboard/` — Axum web server (15 route modules + WebSocket), REST API, fleet coordination
- `src/db/` — PostgreSQL via sqlx (15 submodules: primes, jobs, workers, agents, projects, records, ai_engine, etc.)
- `src/project/` — Campaign management (config, cost, orchestration, records, types)
- `src/checkpoint.rs` — JSON checkpoint save/load (all 12 form variants), atomic writes
- `src/search_manager.rs` — Search job lifecycle, block generation, work distribution
- `src/agent.rs` — AI agent infrastructure for autonomous search management
- `src/fleet.rs` — In-memory worker registry (60s stale timeout)
- `src/pg_worker.rs` — PostgreSQL work claiming (`FOR UPDATE SKIP LOCKED`)
- `src/worker_client.rs` — HTTP client for worker-to-coordinator communication
- `src/deploy.rs` — SSH deployment, service management, rolling updates
- `src/events.rs` — Event bus for prime notifications and search status
- `src/metrics.rs` + `src/prom_metrics.rs` — System metrics + Prometheus export
- `src/operator.rs` — Operator node management
- `src/progress.rs` — Atomic counters + background 30s status reporter

### Frontend (`frontend/`)
- Next.js 16 + React 19 + Tailwind 4 + shadcn/ui + Recharts + Supabase JS
- Static export served by Rust backend
- **Supabase** for prime data (queries, stats, charts) and auth
- **Supabase Realtime** for live prime notifications
- **WebSocket** for coordination data only (fleet, searches, deployments, status)
- 14 pages, 50+ components, 17+ custom hooks

### Database (`supabase/`)
- 24 PostgreSQL migrations (Supabase)
- Tables: primes, search_jobs, work_blocks, workers, agent_tasks, projects, etc.
- See `supabase/CLAUDE.md` for schema details

### Deployment (`deploy/`)
- SSH scripts, systemd units, Nginx, Grafana, Helm charts, Terraform
- Fleet: CX22 coordinator + CCX23 workers
- See `deploy/CLAUDE.md` for details

## Key Design Decisions

- Checkpoints save every 60 seconds as JSON with atomic rename. Cleared on search completion.
- All search forms follow: sieve → parallel primality test → proof → log to PostgreSQL.
- `kbn::test_prime` is `pub(crate)` — reused by twin, sophie_germain, cullen_woodall, carol_kynea, gen_fermat.
- Even-digit palindromes are skipped — always divisible by (base+1).
- Primality testing uses `rug::Integer::is_probably_prime(25)` (Miller-Rabin). Results classified as "deterministic" or "probabilistic".
- Engine modules use `db.insert_prime_sync(rt, ...)` wrapper for rayon threads (can't `.await`).
- No `unsafe` code in main crate (except macOS QoS syscall). `gwnum-sys` FFI crate is feature-gated.

## Testing

```bash
# Unit tests (449 passing, 7 ignored PFGW)
cargo test

# Integration tests
cargo test --test db_integration
cargo test --test api_integration
cargo test --test cli_tests
cargo test --test property_tests
cargo test --test security_tests

# Benchmarks
cargo bench

# Quick search verification
cargo run -- factorial --start 1 --end 100
cargo run -- kbn --k 3 --base 2 --min-n 1 --max-n 1000
cargo run -- palindromic --base 10 --min-digits 1 --max-digits 9

# Frontend
cd frontend && npm test          # Vitest unit tests
cd frontend && npm run test:e2e  # Playwright E2E tests
```

## Commenting Standards

**This codebase is a teaching tool for computational number theory.** See `.claude/commenting-standards.md` for the full standard. Summary:
- Every file needs a module-level doc comment (purpose, algorithm, references)
- Every public item needs a doc comment (algorithm steps, math basis, complexity)
- Cite theorems by name, link OEIS sequences, reference papers
- Engine: ~30-40% comments. Server: ~20-30%. Frontend: ~15-25%.

## Conventions

- All output goes to stderr (`eprintln!`). Results are logged to Supabase PostgreSQL via `sqlx`.
- Global flags `--database-url` (or `DATABASE_URL` env) and `--checkpoint` go before the subcommand.
- `insert_prime_sync` takes 7 args (including `certificate: Option<&str>`).
- **Naming migration**: `volunteer` → `operator`, `worker` → `node`, `fleet` → `network`. Old names available as backward-compat re-exports.
- All 12 search forms must check `worker_client.is_stop_requested()` in their block loop.
- `checked_u32()` in `lib.rs`: always use instead of `n as u32` for `.pow()` / `<<` with u64 exponents.
- `has_small_factor()`: compare via `*n != p` (PartialEq<u32>) to avoid heap-allocating Integer.
- Default checkpoint file: `darkreach.checkpoint`.

## Agent Coding Guide

When working autonomously on this codebase, read the domain-specific CLAUDE.md before making changes:

### Adding a new search form
1. Read `src/CLAUDE.md` for the engine pattern
2. Create `src/<form>.rs` following the sieve → test → proof → log pipeline
3. Add checkpoint variant in `src/checkpoint.rs`
4. Add CLI subcommand in `src/main.rs` + dispatch in `src/cli.rs`
5. Add module in `src/lib.rs`, update `search_manager.rs` and `deploy.rs`
6. Add migration if new tables needed (`supabase/migrations/`)
7. Update this file's Architecture section

### Adding a new API route
1. Read `src/CLAUDE.md` for the dashboard pattern
2. Create handler in appropriate `src/dashboard/routes_*.rs` file (or new file)
3. Register route in `src/dashboard/mod.rs`
4. Add DB method in appropriate `src/db/*.rs` submodule

### Adding a new frontend page
1. Read `frontend/CLAUDE.md` for conventions
2. Create `frontend/src/app/<page>/page.tsx`
3. Add navigation link in `frontend/src/components/app-header.tsx`
4. Create hooks in `frontend/src/hooks/` for data fetching
5. Use shadcn/ui components from `frontend/src/components/ui/`

### Adding a database migration
1. Read `supabase/CLAUDE.md` for schema and conventions
2. Create `supabase/migrations/NNN_<description>.sql` (next sequence number)
3. Add corresponding Rust types/queries in `src/db/*.rs`
4. Run migration via Supabase CLI or direct SQL
