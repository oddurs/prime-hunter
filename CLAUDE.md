# CLAUDE.md

## Project overview

`primehunt` is a Rust CLI tool for hunting special-form prime numbers on dedicated servers. It uses GMP (via `rug`) for arbitrary-precision arithmetic and `rayon` for parallel search.

## Build

Requires GMP installed on the system:
- Linux: `apt install build-essential libgmp-dev m4`
- macOS: `brew install gmp`

```bash
cargo build --release
```

## Architecture

- `src/main.rs` — CLI routing with `clap` (subcommands: factorial, palindromic, kbn)
- `src/lib.rs` — Module re-exports
- `src/db.rs` — SQLite logging via `rusqlite`
- `src/checkpoint.rs` — JSON checkpoint save/load with atomic file writes
- `src/progress.rs` — Atomic counters + background status reporter thread (30s interval)
- `src/factorial.rs` — n! ± 1 search (incremental factorial, `rayon::join` for ±1 testing)
- `src/palindromic.rs` — Palindromic prime search (batch generation, parallel testing)
- `src/kbn.rs` — k·b^n ± 1 search (block-based parallel iteration)

## Key design decisions

- Checkpoints save every 60 seconds as JSON with atomic rename. Cleared on search completion.
- Factorial computation is sequential (n! = n × (n-1)!) but primality tests of n!+1 and n!-1 run in parallel via `rayon::join`.
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

- All output goes to stderr (eprintln!). Results are logged to SQLite.
- Global flags `--db` and `--checkpoint` go before the subcommand.
- No `unsafe` code. Dependencies kept minimal.
