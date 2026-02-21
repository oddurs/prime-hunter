# Testing Roadmap

Phased plan to expand darkreach from 100 algorithm unit tests to a comprehensive multi-layered test system. Each phase is self-contained and delivers incremental value.

## Overview

| Phase | Focus | New Tests | Cumulative | Status |
|-------|-------|-----------|------------|--------|
| 0 | Quick wins — unit tests for untested modules | 77 | 177 | **Done** |
| 1 | Infrastructure unit tests + CI pipeline | 71 | 248 | **Done** |
| 2 | Integration tests (DB, API, CLI) | 53 | 301 | **Done** |
| 3 | Frontend test foundation | 86 | 387 | **Done** |
| 4 | Property-based + benchmark tests | 29 | 416 | **Done** |
| 5 | E2E tests + security tests | 39 | 455 | **Done** |

---

## Phase 0: Quick Wins (Unit Tests for Untested Modules)

**Goal**: Add unit tests to Rust modules that have no tests but are easy to test in isolation (no external dependencies).

### 0.1 `lib.rs` — Core Utilities (5 tests)

```
has_small_factor_returns_false_for_small_primes
has_small_factor_returns_true_for_composites
has_small_factor_equal_to_small_prime_is_not_composite
mr_screened_test_primes_pass
estimate_digits_within_one_of_exact
```

### 0.2 `network.rs` — Node Registry (7 tests)

```
new_network_is_empty
register_adds_node
register_duplicate_overwrites
heartbeat_known_node_returns_true
heartbeat_unknown_node_returns_false
send_command_delivered_on_heartbeat
deregister_removes_node
prune_stale_removes_old_nodes
```

### 0.3 `events.rs` — Event Bus (8 tests)

```
new_event_bus_has_no_events
emit_prime_found_recorded
emit_search_started_creates_notification
emit_search_completed_creates_notification
emit_milestone_creates_notification
emit_warning_no_notification
emit_error_creates_notification
flush_squashes_by_form
recent_events_capped_at_200
recent_notifications_capped_at_50
```

### 0.4 `search_manager.rs` — Search Params (5 tests)

```
search_params_serde_roundtrip_factorial
search_params_serde_roundtrip_kbn
search_params_serde_roundtrip_all_variants
search_type_name_matches_serde_tag
to_args_produces_valid_cli_args
```

### 0.5 `factorial.rs` — Factorial Computation (3 tests)

```
factorial_values_correct
factorial_sieve_eliminates_composites
factorial_known_primes_found
```

### 0.6 `palindromic.rs` — Palindrome Generation (4 tests)

```
generate_palindromes_are_valid
even_digit_palindromes_skipped
palindrome_divisible_by_base_plus_one
palindrome_batch_count_correct
```

### 0.7 `db.rs` — Filter Validation (3 tests)

```
safe_sort_column_whitelists_known_columns
safe_sort_column_defaults_to_id
safe_sort_dir_defaults_to_desc
```

**Phase 0 Total**: ~35 new tests | Cumulative: ~135

### Dev-dependencies needed
None beyond existing `tempfile`. All tests are pure logic, no I/O.

### Acceptance criteria
- `cargo test --lib` passes with ~135 tests
- No new dependencies added
- All tests run in < 1 second

---

## Phase 1: Infrastructure Unit Tests + CI Pipeline

**Goal**: Set up CI and add unit tests for remaining infrastructure that requires mocking (async, process spawning).

### 1.1 GitHub Actions CI Pipeline

Create `.github/workflows/test.yml`:
- **rust-unit**: `cargo test --lib` on Ubuntu with GMP
- **rust-clippy**: `cargo clippy -- -D warnings`
- **rust-fmt**: `cargo fmt --check`
- **frontend-lint**: `npm run lint` in `frontend/`
- Triggered on push and PR to master

### 1.2 `metrics.rs` — Hardware Metrics (2 tests)

```
collect_returns_valid_percentages
collect_returns_non_negative_gb
```

### 1.3 `progress.rs` — Atomic Counters (3 tests)

```
counter_starts_at_zero
increment_updates_value
concurrent_increments_are_accurate
```

### 1.4 `search_manager.rs` — Manager Logic (5 tests)

```
new_manager_has_no_searches
launch_adds_search_entry
max_concurrent_enforced
stop_terminates_process
list_returns_all_searches
```

Note: `SearchManager` spawns child processes. Tests should mock the process spawning or use small searches that complete instantly.

### 1.5 `deploy.rs` — Deployment Logic (5 tests)

```
deployment_manager_new_empty
add_deployment_recorded
deployment_status_lifecycle
cli_args_for_deploy_search
deploy_script_bash_syntax_valid
```

### 1.6 Checkpoint Resume Logic (5 tests)

```
factorial_checkpoint_resumes_from_saved_n
kbn_checkpoint_resumes_from_saved_block
palindromic_checkpoint_resumes_from_saved_digits
checkpoint_cleared_after_completion
checkpoint_enum_covers_all_search_types
```

**Phase 1 Total**: ~20 new tests + CI pipeline | Cumulative: ~155

### Dev-dependencies needed
None new for tests. CI requires GMP package.

### Acceptance criteria
- CI pipeline runs on every push/PR
- `cargo test --lib` passes with ~155 tests
- `cargo clippy` and `cargo fmt` pass
- CI badge in README

---

## Phase 2: Integration Tests (DB, API, CLI)

**Goal**: Test real interactions with PostgreSQL, HTTP API, and CLI binary.

### 2.1 Test Infrastructure Setup

#### Docker Compose for Test Database

```yaml
# docker-compose.test.yml
services:
  postgres:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: test
      POSTGRES_DB: darkreach_test
    ports: ['5433:5432']
```

#### Test Helper Module (`tests/common/mod.rs`)

```rust
pub async fn setup_test_db() -> Database { ... }
pub async fn cleanup_test_db(db: &Database) { ... }
pub async fn spawn_test_server() -> (String, JoinHandle<()>) { ... }
```

### 2.2 Database Integration Tests (`tests/db_integration.rs`) — 16 tests

```
connect_to_test_db
insert_prime_and_retrieve
insert_duplicate_expression
filter_primes_by_form
filter_primes_by_digit_range
filter_primes_search_text
sort_primes_ascending_descending
paginate_primes
get_stats_with_data
get_stats_empty_db
get_timeline_buckets
get_digit_distribution
get_prime_detail
worker_heartbeat_insert_and_update
claim_work_block_atomic
search_job_full_lifecycle
```

### 2.3 HTTP API Integration Tests (`tests/api_integration.rs`) — 18 tests

```
get_stats_returns_200
get_primes_returns_paginated
get_primes_with_filters
get_prime_detail_returns_200
get_prime_detail_not_found_404
post_heartbeat_new_worker
post_heartbeat_existing_worker
post_command_queued
get_workers_list
get_search_jobs
post_search_job_creates_blocks
get_events
get_notifications
websocket_connects
websocket_receives_updates
cors_headers_present
large_body_rejected
request_timeout_enforced
```

### 2.4 CLI Integration Tests (`tests/cli_integration.rs`) — 14 tests

Uses `assert_cmd` crate for binary execution.

```
help_shows_subcommands
version_flag
factorial_finds_known_primes
palindromic_finds_known_primes
kbn_finds_mersenne_primes
primorial_finds_known_primes
cullen_woodall_finds_primes
wagstaff_finds_known_primes
carol_kynea_finds_primes
twin_finds_known_pairs
sophie_germain_finds_primes
repunit_finds_known_primes
gen_fermat_finds_primes
invalid_args_error_exit
```

### 2.5 Node Coordination Tests (`tests/coordination_integration.rs`) — 7 tests

> **Note:** Add operator API test categories for the new operator endpoints (`/api/operators/*`).

```
node_register_via_http
node_heartbeat_cycle
pg_node_heartbeat_rpc
claim_block_via_pg
complete_block_via_pg
stale_block_reclaimed
stop_command_delivered
```

### 2.6 CI Pipeline Update

Add integration test job with PostgreSQL service container:
```yaml
rust-integration:
  services:
    postgres:
      image: postgres:16
      env: { POSTGRES_PASSWORD: test, POSTGRES_DB: darkreach_test }
  steps:
    - run: cargo test --test '*'
```

**Phase 2 Total**: ~55 new tests | Cumulative: ~210

### Dev-dependencies needed (Cargo.toml)

```toml
assert_cmd = "2"
predicates = "3"
axum-test = "16"
tokio-test = "0.4"
```

### Acceptance criteria
- All integration tests pass with local PostgreSQL
- CI runs integration tests with service container
- `cargo test` (all tests) passes
- Database tests use transactions for isolation

---

## Phase 3: Frontend Test Foundation

**Goal**: Establish frontend testing infrastructure and cover hooks, utilities, and key components.

### 3.1 Testing Stack Setup

Install test dependencies:
```bash
cd frontend
npm install -D vitest @testing-library/react @testing-library/jest-dom \
  @testing-library/user-event msw jsdom
```

Configure Vitest:
```typescript
// vitest.config.ts
import { defineConfig } from 'vitest/config'
export default defineConfig({
  test: {
    environment: 'jsdom',
    setupFiles: ['./test-setup.ts'],
    globals: true,
  },
})
```

Add test scripts to `package.json`:
```json
{
  "scripts": {
    "test": "vitest run",
    "test:watch": "vitest",
    "test:coverage": "vitest run --coverage"
  }
}
```

### 3.2 MSW Mock Setup

Create mock handlers for Supabase API:
```typescript
// __mocks__/supabase.ts — Mock Supabase client
// __mocks__/handlers.ts — MSW request handlers
```

### 3.3 Utility Tests (4 tests)

```
format_ts — formatNumber, formatDigits, formatTime, formatDate
utils_ts — cn merges classes, handles undefined
```

### 3.4 Hook Tests (16 tests)

```
use_primes — fetches data, handles filters, handles error
use_stats — fetches stats, handles empty, handles error
use_timeline — fetches buckets, parses dates
use_distribution — fetches histogram, handles empty
use_prime_realtime — subscribes, receives new prime, unsubscribes
use_websocket — connects, receives network data, reconnects
use_theme — defaults to system, toggles, persists
use_mobile — detects mobile breakpoint, handles resize
```

### 3.5 Component Tests (12 tests)

```
app_header — renders nav, highlights active, toggles theme
metrics_bar — renders percentage, correct color threshold
search_card — renders params, status badge, progress
host_node_card — renders node info, metrics
prime_notifier — shows toast on new prime
new_search_dialog — form validation, submit
charts/discovery_timeline — renders with data, empty state
charts/digit_distribution — renders histogram
charts/throughput_gauge — renders gauge value
```

### 3.6 Page Tests (6 tests)

```
dashboard_page — renders stat cards, table, charts
login_page — renders form, handles submit
browse_page — renders filters, table, pagination
searches_page — renders search list
network_page — renders node cards
docs_page — renders markdown, search
```

### 3.7 CI Pipeline Update

Add frontend test job:
```yaml
frontend-unit:
  steps:
    - run: cd frontend && npm ci && npm test
```

**Phase 3 Total**: ~38 new tests | Cumulative: ~248

### Acceptance criteria
- `npm test` passes in `frontend/`
- Vitest coverage report generated
- CI runs frontend tests
- MSW mocks cover all Supabase API calls

---

## Phase 4: Property-Based + Benchmark Tests

**Goal**: Add property-based testing for mathematical invariants and criterion benchmarks for performance regression detection.

### 4.1 Property-Based Tests (proptest)

Add `proptest = "1"` to dev-dependencies.

Create `tests/property_tests.rs`:

```
prop_pow_mod_matches_big_int          # pow_mod(b,e,m) == (b^e) % m
prop_mod_inverse_roundtrip            # mod_inverse(a,p) * a ≡ 1 (mod p)
prop_gcd_symmetric_and_divides        # gcd(a,b) == gcd(b,a), divides both
prop_generate_primes_all_prime        # all elements pass MR
prop_has_small_factor_false_for_primes # known primes pass
prop_estimate_digits_within_one       # |estimate - exact| <= 1
prop_checkpoint_roundtrip             # load(save(x)) == x
prop_search_params_serde_roundtrip    # deserialize(serialize(x)) == x
prop_build_candidate_is_palindrome    # near_repdigit palindrome invariant
prop_safe_sort_column_whitelisted     # never returns arbitrary input
```

~10 property tests

### 4.2 Benchmark Suite (criterion)

Add `criterion = { version = "0.5", features = ["html_reports"] }` to dev-dependencies.

Create `benches/` directory:

**`benches/sieve_bench.rs`**:
```
bench_generate_primes_1m
bench_pow_mod_large
bench_multiplicative_order
bench_bsgs_discrete_log
```

**`benches/kbn_bench.rs`**:
```
bench_llr_mersenne_127
bench_llr_mersenne_521
bench_proth_test
bench_bsgs_sieve_1000_block
```

**`benches/proof_bench.rs`**:
```
bench_pocklington_proof
bench_morrison_proof
bench_lucas_v_big
```

**`benches/core_bench.rs`**:
```
bench_has_small_factor_prime
bench_has_small_factor_composite
bench_mr_screened_prime
bench_mr_screened_composite
bench_checkpoint_save_load
```

~15 benchmarks

### 4.3 CI Pipeline Update

Add benchmark compilation check (don't run, just verify they compile):
```yaml
rust-bench-check:
  steps:
    - run: cargo bench --no-run
```

**Phase 4 Total**: ~25 new tests/benchmarks | Cumulative: ~273

### Acceptance criteria
- `cargo test --test property_tests` passes
- `cargo bench` runs all benchmarks and generates HTML report
- No benchmark regressions > 20% (manual check initially)
- CI verifies benchmarks compile

---

## Phase 5: E2E Tests + Security Tests

**Goal**: Browser-based end-to-end tests and security-focused tests.

### 5.1 Playwright E2E Setup

```bash
cd frontend
npm install -D @playwright/test
npx playwright install
```

Configure:
```typescript
// playwright.config.ts
export default defineConfig({
  testDir: './e2e',
  webServer: {
    command: 'npm run dev',
    port: 3000,
  },
})
```

### 5.2 E2E Tests (`frontend/e2e/`) — 10 tests

```
login_and_redirect.spec.ts
dashboard_loads_data.spec.ts
browse_filter_and_sort.spec.ts
browse_pagination.spec.ts
browse_detail_dialog.spec.ts
start_new_search.spec.ts
fleet_monitoring.spec.ts
theme_toggle_persists.spec.ts
docs_search_and_render.spec.ts
responsive_mobile_layout.spec.ts
```

### 5.3 Security Tests — 6 tests

Rust integration tests (`tests/security_tests.rs`):
```
sql_injection_sort_column_sanitized
sql_injection_search_param_escaped
body_size_limit_enforced
request_timeout_prevents_slowloris
cors_configuration_correct
```

Frontend:
```
no_secrets_in_static_build
```

### 5.4 CI Pipeline Update

Add E2E job (runs after frontend-unit):
```yaml
frontend-e2e:
  steps:
    - run: cd frontend && npm ci && npx playwright install
    - run: cd frontend && npx playwright test
```

**Phase 5 Total**: ~16 new tests | Cumulative: ~289

### Acceptance criteria
- Playwright tests pass in headed and headless modes
- Security tests verify no injection vectors
- CI runs E2E tests
- All tests green across all CI jobs

---

## Summary

### Final Test Inventory (~289 tests)

| Category | Count |
|----------|-------|
| Rust unit tests (existing) | 100 |
| Rust unit tests (new, Phase 0-1) | ~55 |
| Rust integration tests (Phase 2) | ~55 |
| Rust property tests (Phase 4) | ~10 |
| Rust security tests (Phase 5) | ~5 |
| Criterion benchmarks (Phase 4) | ~15 |
| Frontend unit tests (Phase 3) | ~38 |
| Frontend E2E tests (Phase 5) | ~10 |
| **Total** | **~289** |

### Dependencies Added

| Phase | Rust | Frontend |
|-------|------|----------|
| 0 | None | - |
| 1 | None | - |
| 2 | assert_cmd, predicates, axum-test, tokio-test | - |
| 3 | - | vitest, @testing-library/*, msw, jsdom |
| 4 | proptest, criterion | - |
| 5 | - | @playwright/test |

### CI Pipeline (Final State)

```
test.yml
├── rust-unit          # cargo test --lib (~155 tests, <2s)
├── rust-integration   # cargo test --test '*' (~55 tests, postgres service)
├── rust-clippy        # cargo clippy -- -D warnings
├── rust-fmt           # cargo fmt --check
├── rust-bench-check   # cargo bench --no-run
├── frontend-lint      # npm run lint
├── frontend-unit      # npm test (~38 tests)
└── frontend-e2e       # npx playwright test (~10 tests)
```

### Cross-References

- **Test Plan**: `docs/testing/plan.md` — detailed test specifications
- **Test Infrastructure**: `docs/testing/infrastructure.md` — tooling and setup
- **Engine Roadmap**: `docs/roadmaps/engine.md`
- **Server Roadmap**: `docs/roadmaps/server.md`
- **Frontend Roadmap**: `docs/roadmaps/frontend.md`
