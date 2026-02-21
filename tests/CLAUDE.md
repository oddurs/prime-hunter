# tests/ + benches/ — Testing Domain

Integration tests and benchmarks for darkreach. Unit tests live alongside source files in `src/`.

## Directory Structure

```
tests/
├── common/
│   └── mod.rs                 # Shared helpers: test DB setup, migration runner, table truncation
├── db_integration.rs          # Database integration tests (all db module operations)
├── api_integration.rs         # API endpoint tests (Axum test client, all routes)
├── cli_tests.rs               # CLI smoke tests (subcommand parsing, help text)
├── property_tests.rs          # Property-based tests (sieve correctness, prime form invariants)
└── security_tests.rs          # Security tests (no secret leaks, input validation)

benches/
├── core_bench.rs              # Core algorithm benchmarks (trial division, MR, sieve)
├── sieve_bench.rs             # Sieve benchmarks (Eratosthenes, Montgomery, wheel)
├── kbn_bench.rs               # k·b^n benchmark (Proth test, LLR, BSGS sieve)
├── proof_bench.rs             # Proof benchmarks (Pocklington, Morrison, BLS)
└── flint_bench.rs             # FLINT integration benchmarks (feature-gated)
```

## Running Tests

```bash
# Unit tests (449 passing, 7 ignored PFGW-dependent)
cargo test

# Specific test file
cargo test --test db_integration
cargo test --test api_integration
cargo test --test cli_tests
cargo test --test property_tests
cargo test --test security_tests

# Integration tests require a test database
TEST_DATABASE_URL=postgres://... cargo test --test db_integration
TEST_DATABASE_URL=postgres://... cargo test --test api_integration

# Benchmarks
cargo bench
cargo bench --bench core_bench
cargo bench --bench sieve_bench

# Frontend tests
cd frontend && npm test                # Vitest unit tests
cd frontend && npm run test:e2e        # Playwright E2E tests
```

## Test Infrastructure

### common/mod.rs — Shared Helpers

| Function | Purpose |
|----------|---------|
| `test_db_url()` | Read `TEST_DATABASE_URL` env var (panics if missing) |
| `has_test_db()` | Check if test database is configured |
| `ensure_schema()` | Run migrations once per test suite (via `Once`) |
| `setup_test_db()` | Connect + ensure schema + truncate all tables |
| `build_test_app()` | Build Axum test router with test DB |
| `truncate_all_tables()` | Reset all tables + re-seed roles/templates/budgets |
| `clean_migration_sql()` | Strip Supabase-specific SQL (RLS, Realtime, policies) |

### Test Database Setup

Integration tests need a PostgreSQL database. The test harness:
1. Reads `TEST_DATABASE_URL` from environment
2. Runs migrations (filtered subset, Supabase-specific SQL stripped)
3. Truncates all tables before each test
4. Re-seeds reference data (agent roles, templates, budgets)

Use `docker-compose.test.yml` for local test database.

## Test Patterns

### Unit tests (in `src/*.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_primes() {
        // Verify against known values from OEIS
        assert!(is_prime(&Integer::from(7)));
    }
}
```

### Integration tests (in `tests/`)

```rust
use common::*;

#[tokio::test]
async fn test_insert_and_query_prime() {
    if !has_test_db() { return; }
    let db = setup_test_db().await;
    // ... test database operations
}
```

### API tests (in `tests/api_integration.rs`)

```rust
use axum_test::TestServer;

#[tokio::test]
async fn test_health_endpoint() {
    if !has_test_db() { return; }
    let app = build_test_app().await;
    let server = TestServer::new(app).unwrap();
    let resp = server.get("/api/health").await;
    resp.assert_status_ok();
}
```

### Property tests (in `tests/property_tests.rs`)

Tests mathematical invariants:
- Sieve correctness (all flagged composites are actually composite)
- Prime form expression parsing
- Checkpoint serialization round-trips

## Agent Coding Guide

### Adding a new integration test

1. Add test function in appropriate `tests/*.rs` file
2. Guard with `if !has_test_db() { return; }` for DB-dependent tests
3. Use `setup_test_db().await` for database access
4. Use `build_test_app().await` for API route testing
5. Truncation is automatic — each test starts clean

### Adding a new test file

1. Create `tests/<name>.rs`
2. Add `mod common;` at top if database access needed
3. Add `[[test]]` entry in `Cargo.toml` if needed
4. Use `#[tokio::test]` for async tests

### Adding a benchmark

1. Add to existing `benches/<domain>_bench.rs` or create new file
2. Use `criterion` for benchmarks
3. Add `[[bench]]` entry in `Cargo.toml` if new file

### When to update tests

- Adding a new search form: add known-values test in the form's `#[cfg(test)]` module
- Adding a new API route: add test in `tests/api_integration.rs`
- Adding a new DB operation: add test in `tests/db_integration.rs`
- Changing checkpoint format: update round-trip test in `tests/property_tests.rs`

## Roadmap

See `docs/roadmaps/testing.md` for testing strategy and planned improvements.
