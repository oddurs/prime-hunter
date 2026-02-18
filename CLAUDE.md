# CLAUDE.md

## Project overview

`primehunt` is a Rust CLI tool for hunting special-form prime numbers on dedicated servers. It uses GMP (via `rug`) for arbitrary-precision arithmetic and `rayon` for parallel search.

## Domain Map

| Domain | Directory | CLAUDE.md | Roadmap |
|--------|-----------|-----------|---------|
| **Engine** | `src/` (algorithm files) | `src/CLAUDE.md` | `docs/roadmaps/engine.md` |
| **Server** | `src/` (infra files) | `src/CLAUDE.md` | `docs/roadmaps/server.md` |
| **Frontend** | `frontend/` | `frontend/CLAUDE.md` | `docs/roadmaps/frontend.md` |
| **Ops** | `deploy/` | `deploy/CLAUDE.md` | `docs/roadmaps/ops.md` |
| **Research** | `docs/` | `docs/CLAUDE.md` | `docs/roadmaps/research.md` |
| **Agents** | `src/` (agent infra) | `src/CLAUDE.md` | `docs/roadmaps/agents.md` |
| **Projects** | `src/project.rs` + frontend | `src/CLAUDE.md` | `docs/roadmaps/projects.md` |

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

### Engine (algorithm modules)
- `src/factorial.rs` — n! ± 1 search (GMP factorial, modular sieve, PFGW -tp/-tm proofs)
- `src/palindromic.rs` — Palindromic prime search (batch generation, deep sieve, parallel testing)
- `src/kbn.rs` — k*b^n ± 1 search (Proth/LLR/Pocklington tests, BSGS sieve)
- `src/near_repdigit.rs` — Near-repdigit palindromic search (BLS N+1 proofs, PFGW)
- `src/primorial.rs` — p# ± 1 search (Pocklington/Morrison proofs, PFGW)
- `src/cullen_woodall.rs` — n·2^n ± 1 search (Proth/LLR, PFGW acceleration)
- `src/wagstaff.rs` — (2^p+1)/3 search (multiplicative order sieve, PFGW/GWNUM)
- `src/carol_kynea.rs` — (2^n±1)²−2 search (LLR test, PFGW acceleration)
- `src/twin.rs` — Twin prime search (quad sieve, Proth+LLR intersection)
- `src/sophie_germain.rs` — Sophie Germain search (Proth+LLR intersection sieve)
- `src/repunit.rs` — Repunit search R(b,n) = (b^n−1)/(b−1) (PFGW acceleration)
- `src/gen_fermat.rs` — Generalized Fermat b^(2^n)+1 search (Pépin/Proth, PFGW)
- `src/sieve.rs` — Sieve of Eratosthenes, Montgomery multiplication, wheel factorization
- `src/lib.rs` — Module re-exports, small primes table, trial division, MR pre-screening
- `src/proof.rs` — Pocklington (N−1), Morrison (N+1), BLS deterministic proofs
- `src/verify.rs` — 3-tier independent re-verification pipeline (deterministic → BPSW+MR → PFGW)
- `src/pfgw.rs` — PFGW subprocess integration (50-100x acceleration for large candidates)
- `src/gwnum.rs` — GWNUM FFI safe wrapper (Vrba-Reix test, feature-gated)
- `src/flint.rs` — FLINT integration (accelerated factorial/primorial, feature-gated)
- `src/prst.rs` — PRST subprocess integration for k*b^n±1 forms
- `src/p1.rs` — Pollard P−1 factoring for deep composite elimination

### Server (infrastructure modules)
- `src/main.rs` — CLI routing with `clap` (12 search subcommands + dashboard/verify/deploy)
- `src/dashboard.rs` — Axum web server, WebSocket (coordination only), REST API, fleet coordination
- `src/db.rs` — PostgreSQL via `sqlx::PgPool` (connects to Supabase)
- `src/checkpoint.rs` — JSON checkpoint save/load (all 12 form variants), atomic writes
- `src/progress.rs` — Atomic counters + background status reporter thread (30s interval)
- `src/fleet.rs` — In-memory worker registry
- `src/worker_client.rs` — HTTP client for worker-to-coordinator communication
- `src/pg_worker.rs` — PostgreSQL-based work claiming (`FOR UPDATE SKIP LOCKED`)
- `src/search_manager.rs` — Search job lifecycle, block generation, work distribution
- `src/agent.rs` — AI agent infrastructure for autonomous search management
- `src/deploy.rs` — SSH deployment, service management, rolling updates
- `src/events.rs` — Event bus for prime notifications and search status updates
- `src/metrics.rs` — System metrics collection (CPU, memory, disk)

### Frontend
- Next.js 16 + React 19 + Tailwind 4 + shadcn/ui + Recharts + Supabase JS
- Static export served by Rust backend
- **Supabase** for prime data (queries, stats, charts) and auth
- **Supabase Realtime** for live prime notifications
- **WebSocket** for coordination data only (fleet, searches, deployments, status)

### Deployment
- `deploy/deploy.sh` — SSH deployment script
- `deploy/primehunt-coordinator.service` — Dashboard systemd unit
- `deploy/primehunt-worker@.service` — Worker template unit

## Key Design Decisions

- Checkpoints save every 60 seconds as JSON with atomic rename. Cleared on search completion.
- Factorial computation is sequential (n! = n * (n-1)!) but primality tests of n!+1 and n!-1 run in parallel via `rayon::join`.
- Palindromic and kbn searches use `rayon::par_iter` over batches/blocks for full core utilization.
- Even-digit palindromes are skipped — they're always divisible by (base+1).
- Primality testing uses `rug::Integer::is_probably_prime(25)` (Miller-Rabin). Results classified as "deterministic" (GMP verified exactly) or "probabilistic".

## Testing

Run with small ranges to verify:
```bash
cargo run -- factorial --start 1 --end 100
cargo run -- kbn --k 3 --base 2 --min-n 1 --max-n 1000
cargo run -- palindromic --base 10 --min-digits 1 --max-digits 9
```

## Commenting Standards

**This codebase is a teaching tool for computational number theory.** All code must be documented at an academic level. See `.claude/commenting-standards.md` for the full standard. Summary:

- Every file needs a module-level doc comment (purpose, algorithm, references)
- Every public item needs a doc comment (algorithm steps, math basis, complexity)
- Cite theorems by name, link OEIS sequences, reference papers
- Document invariants, preconditions, and non-obvious logic
- Engine files: ~30-40% comments. Server: ~20-30%. Frontend: ~15-25%.

## Conventions

- All output goes to stderr (eprintln!). Results are logged to Supabase PostgreSQL via `sqlx`.
- Global flag `--database-url` (or `DATABASE_URL` env) and `--checkpoint` go before the subcommand.
- Engine modules use `db.insert_prime_sync(rt, ...)` wrapper for rayon threads (can't `.await`).
- No `unsafe` code in main crate (except macOS QoS syscall in main.rs). The `gwnum-sys` FFI crate (feature-gated) contains unsafe bindings. Dependencies kept minimal.
