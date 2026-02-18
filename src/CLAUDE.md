# src/ — Engine & Server Domains

This directory contains all Rust source code for primehunt, spanning two domains:

## Module Map

| Module | Lines | Domain | Responsibility |
|--------|-------|--------|----------------|
| `dashboard.rs` | ~2217 | Server | Axum web server, WebSocket, REST API, fleet coordination |
| `main.rs` | ~1311 | Server | CLI routing with clap (all 12 search subcommands + dashboard/verify/deploy) |
| `db.rs` | ~1297 | Server | PostgreSQL via `sqlx::PgPool` (Supabase): prime records, filtering, stats |
| `verify.rs` | ~1036 | Engine | Independent re-verification: 3-tier pipeline (deterministic → BPSW+MR → PFGW) |
| `kbn.rs` | ~1023 | Engine | k*b^n ± 1 search; Proth test, LLR test, Pocklington test, BSGS sieve |
| `agent.rs` | ~847 | Server | AI agent infrastructure for autonomous search management |
| `search_manager.rs` | ~749 | Server | Search job lifecycle, block generation, work distribution |
| `sieve.rs` | ~734 | Engine | Sieve of Eratosthenes, Montgomery multiplication, pow_mod, wheel factorization |
| `carol_kynea.rs` | ~641 | Engine | (2^n±1)²−2 search; LLR test, incremental modular sieve, PFGW acceleration |
| `events.rs` | ~605 | Server | Event bus for prime notifications and search status updates |
| `factorial.rs` | ~591 | Engine | n! ± 1 search; GMP factorial, modular sieve, PFGW -tp/-tm proofs |
| `cullen_woodall.rs` | ~579 | Engine | n·2^n ± 1 search; Proth/LLR tests, PFGW acceleration |
| `checkpoint.rs` | ~573 | Server | JSON checkpoint save/load (all 12 form variants), atomic writes |
| `pfgw.rs` | ~568 | Engine | PFGW subprocess integration; 50-100x acceleration for large candidates |
| `gwnum.rs` | ~567 | Engine | GWNUM FFI safe wrapper; Vrba-Reix test for Wagstaff (feature-gated) |
| `primorial.rs` | ~559 | Engine | p# ± 1 search; Pocklington/Morrison proofs, PFGW acceleration |
| `proof.rs` | ~558 | Engine | Pocklington (N−1), Morrison (N+1), BLS proofs for factorial/primorial/near-repdigit |
| `near_repdigit.rs` | ~539 | Engine | Near-repdigit palindromic search; BLS N+1 proofs, PFGW acceleration |
| `deploy.rs` | ~510 | Server | SSH deployment, service management, rolling updates |
| `gen_fermat.rs` | ~505 | Engine | b^(2^n)+1 search; Pépin/Proth test, PFGW acceleration |
| `wagstaff.rs` | ~495 | Engine | (2^p+1)/3 search; multiplicative order sieve, PFGW/GWNUM acceleration |
| `repunit.rs` | ~442 | Engine | R(b,n) = (b^n−1)/(b−1) search; PFGW acceleration |
| `sophie_germain.rs` | ~436 | Engine | Sophie Germain prime search; Proth+LLR intersection sieve |
| `twin.rs` | ~396 | Engine | Twin prime search; quad sieve, Proth+LLR intersection |
| `prst.rs` | ~365 | Engine | PRST subprocess integration for k*b^n±1 forms |
| `palindromic.rs` | ~624 | Engine | Palindromic prime search; digit array generation, deep sieve |
| `p1.rs` | ~285 | Engine | Pollard P−1 factoring for deep composite elimination |
| `fleet.rs` | ~277 | Server | In-memory worker registry, stale worker pruning (60s timeout) |
| `lib.rs` | ~268 | Engine | Module re-exports, small primes table, trial division, MR pre-screening, digit estimation |
| `worker_client.rs` | ~252 | Server | HTTP client for fleet coordination (register/heartbeat/report) |
| `pg_worker.rs` | ~218 | Server | PostgreSQL-based work claiming with `FOR UPDATE SKIP LOCKED` |
| `metrics.rs` | ~159 | Server | System metrics collection (CPU, memory, disk) |
| `flint.rs` | ~137 | Engine | FLINT integration for accelerated factorial/primorial (feature-gated) |
| `progress.rs` | ~137 | Server | Atomic counters (tested/found), background 30s reporter thread |

## Engine Domain

Algorithm files for prime-hunting searches (12 forms).

### Key patterns
- **rug/GMP**: All arbitrary-precision arithmetic via `rug::Integer`. Primality testing via `is_probably_prime(25)` (Miller-Rabin + BPSW).
- **rayon parallelism**: All search forms use `rayon::par_iter` over blocks/batches for full core utilization.
- **Modular sieve**: Each search form implements its own sieve to pre-filter candidates before expensive PRP tests.
- **PFGW acceleration**: Large candidates (configurable digit threshold) are routed to PFGW subprocess for 50-100x speedup via GWNUM internally.
- **Deterministic proofs**: Proth (N−1), LLR (N+1 Lucas), Pocklington, Morrison, BLS — form-specific.
- **No unsafe code** except gwnum-sys FFI crate (feature-gated).

### PFGW/GWNUM tool selection by form
| Form | Primary tool | Fallback |
|------|-------------|----------|
| kbn k*b^n±1 | PRST / GWNUM direct | GMP Proth/LLR |
| Factorial n!±1 | PFGW -tp/-tm | GMP MR + Pocklington/Morrison |
| Primorial p#±1 | PFGW -tp/-tm | GMP MR + Pocklington/Morrison |
| Wagstaff (2^p+1)/3 | GWNUM Vrba-Reix / PFGW | GMP MR |
| Palindromic | PFGW | GMP MR |
| Near-repdigit | PFGW PRP | GMP MR + BLS proof |
| Cullen/Woodall | PFGW PRP | GMP Proth/LLR |
| Carol/Kynea | PFGW PRP | GMP LLR |
| Gen Fermat | PFGW PRP | GMP Pépin/Proth |
| Repunit | PFGW PRP | GMP MR |

### Conventions
- Even-digit palindromes are skipped (always divisible by base+1).
- Primality results classified as "deterministic" (proven) or "probabilistic" (PRP).
- `has_small_factor()` in `lib.rs` uses 64 hardcoded primes for trial division.
- Proth test in `kbn.rs` uses Jacobi symbol to find quadratic non-residues.
- `kbn::test_prime` is `pub(crate)` for reuse by twin, sophie_germain, cullen_woodall, carol_kynea, gen_fermat.

### Testing commands
```bash
cargo run -- factorial --start 1 --end 100
cargo run -- kbn --k 3 --base 2 --min-n 1 --max-n 1000
cargo run -- palindromic --base 10 --min-digits 1 --max-digits 9
cargo run -- cullen-woodall --min-n 1 --max-n 200
cargo run -- repunit --base 10 --min-n 2 --max-n 100
```

## Server Domain

Web dashboard, database, fleet coordination, and infrastructure.

### Axum API
- Dashboard runs on configurable port (default 8080)
- Static file serving for Next.js frontend
- REST endpoints: `/api/stats`, `/api/primes`, `/api/workers`, `/api/search_jobs`, `/api/verify`
- WebSocket at `/ws` pushes updates every 2 seconds

### Fleet coordination
- Workers register via HTTP heartbeat to coordinator
- `fleet.rs` maintains in-memory worker registry
- `worker_client.rs` is the HTTP client workers use to talk to the coordinator
- `pg_worker.rs` provides PostgreSQL-based work claiming with `FOR UPDATE SKIP LOCKED`
- Workers pruned after 60s without heartbeat

### Database (PostgreSQL via sqlx)
- `primes` table: expression, form, digits, found_at, search_params (JSON), proof_method
- `work_blocks` table: block-based work distribution for cluster coordination
- Checkpoint types: all 12 search forms (serialized as JSON)

### Key dependencies
- `axum` 0.8 — HTTP server + WebSocket
- `tokio` 1 — async runtime
- `sqlx` 0.8 — PostgreSQL (async, Supabase)
- `ureq` 3 — blocking HTTP client for workers

## Roadmaps
- Engine: `docs/roadmaps/engine.md`
- Server: `docs/roadmaps/server.md`
