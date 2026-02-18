# src/ — Engine & Server Domains

This directory contains all Rust source code for primehunt, spanning two domains:

## Module Map

| Module | Lines | Domain | Responsibility |
|--------|-------|--------|----------------|
| `dashboard.rs` | ~786 | Server | Axum web server, WebSocket, REST API, fleet coordination |
| `kbn.rs` | ~610 | Engine | k*b^n +/- 1 search; Proth test, LLR test, Pocklington test, block-based parallel iteration |
| `db.rs` | ~336 | Server | SQLite layer: prime records, filtering, stats, schema |
| `palindromic.rs` | ~337 | Engine | Palindromic prime search; digit array generation, batch processing |
| `factorial.rs` | ~225 | Engine | n! +/- 1 search; incremental factorial, modular sieve, rayon::join for +/- 1 |
| `main.rs` | ~244 | Server | CLI routing with clap (subcommands: factorial, palindromic, kbn, dashboard) |
| `worker_client.rs` | ~167 | Server | HTTP client for fleet coordination (register/heartbeat/report) |
| `fleet.rs` | ~107 | Server | In-memory worker registry, stale worker pruning (60s timeout) |
| `sieve.rs` | ~58 | Engine | Sieve of Eratosthenes, pow_mod, GCD utilities |
| `progress.rs` | ~58 | Server | Atomic counters (tested/found), background 30s reporter thread |
| `lib.rs` | ~57 | Engine | Module re-exports, small primes table (64 primes up to 311), trial division, MR pre-screening, digit estimation |
| `checkpoint.rs` | ~48 | Server | JSON checkpoint save/load (Factorial/Palindromic/Kbn variants), atomic writes |

## Engine Domain

Algorithm files for prime-hunting searches.

### Key patterns
- **rug/GMP**: All arbitrary-precision arithmetic via `rug::Integer`. Primality testing via `is_probably_prime(25)` (Miller-Rabin + BPSW).
- **rayon parallelism**: Factorial uses `rayon::join` for +/-1 testing. Palindromic and kbn use `rayon::par_iter` over batches/blocks.
- **Modular sieve**: Each search form implements its own sieve to pre-filter candidates before expensive GMP tests.
- **No unsafe code**.

### Conventions
- Even-digit palindromes are skipped (always divisible by base+1).
- Primality results classified as "deterministic" (GMP verified exactly) or "probabilistic".
- `has_small_factor()` in `lib.rs` uses 64 hardcoded primes for trial division.
- Proth test in `kbn.rs` uses Jacobi symbol to find quadratic non-residues.

### Testing commands
```bash
cargo run -- factorial --start 1 --end 100
cargo run -- kbn --k 3 --base 2 --min-n 1 --max-n 1000
cargo run -- palindromic --base 10 --min-digits 1 --max-digits 9
```

## Server Domain

Web dashboard, database, fleet coordination, and infrastructure.

### Axum API
- Dashboard runs on configurable port (default 8080)
- Static file serving for Next.js frontend
- REST endpoints: `/api/stats`, `/api/primes`, `/api/workers`, `/api/docs`
- WebSocket at `/ws` pushes updates every 2 seconds

### Fleet coordination
- Workers register via HTTP heartbeat to coordinator
- `fleet.rs` maintains in-memory worker registry
- `worker_client.rs` is the HTTP client workers use to talk to the coordinator
- Workers pruned after 60s without heartbeat

### Database schema (SQLite via rusqlite)
- `primes` table: expression, form, digits, found_at, search_params (JSON)
- Checkpoint types: Factorial, Palindromic, Kbn (serialized as JSON)

### Key dependencies
- `axum` 0.8 — HTTP server + WebSocket
- `tokio` 1 — async runtime
- `rusqlite` 0.31 — SQLite (bundled)
- `ureq` 3 — blocking HTTP client for workers

## Roadmaps
- Engine: `docs/roadmaps/engine.md`
- Server: `docs/roadmaps/server.md`
