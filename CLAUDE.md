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
- `src/factorial.rs` — n! +/- 1 search (incremental factorial, `rayon::join` for +/-1 testing)
- `src/palindromic.rs` — Palindromic prime search (batch generation, parallel testing)
- `src/kbn.rs` — k*b^n +/- 1 search (Proth/LLR/Pocklington tests, block-based parallel iteration)
- `src/sieve.rs` — Sieve of Eratosthenes, modular arithmetic utilities
- `src/lib.rs` — Module re-exports, small primes table, trial division, MR pre-screening

### Server (infrastructure modules)
- `src/main.rs` — CLI routing with `clap` (subcommands: factorial, palindromic, kbn, dashboard)
- `src/dashboard.rs` — Axum web server, WebSocket (coordination only), REST API, fleet coordination
- `src/db.rs` — PostgreSQL via `sqlx::PgPool` (connects to Supabase)
- `src/checkpoint.rs` — JSON checkpoint save/load with atomic file writes
- `src/progress.rs` — Atomic counters + background status reporter thread (30s interval)
- `src/fleet.rs` — In-memory worker registry
- `src/worker_client.rs` — HTTP client for worker-to-coordinator communication

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

## Conventions

- All output goes to stderr (eprintln!). Results are logged to Supabase PostgreSQL via `sqlx`.
- Global flag `--database-url` (or `DATABASE_URL` env) and `--checkpoint` go before the subcommand.
- Engine modules use `db.insert_prime_sync(rt, ...)` wrapper for rayon threads (can't `.await`).
- No `unsafe` code. Dependencies kept minimal.
