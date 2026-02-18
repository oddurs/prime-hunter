# Primehunt Test Plan

Comprehensive testing strategy covering all domains: engine algorithms, server infrastructure, frontend dashboard, database, deployment, and end-to-end workflows.

## Current State

| Domain | Tests | Coverage | Status |
|--------|-------|----------|--------|
| Engine algorithms | 100 unit tests | ~90% of algorithm code | Solid |
| Server infrastructure | 0 tests | ~0% | Critical gap |
| Frontend dashboard | 0 tests | 0% | Not started |
| Database | 0 tests | 0% | Not started |
| CLI / integration | 0 tests | 0% | Not started |
| E2E workflows | 0 tests | 0% | Not started |

**Goal**: Expand from 100 algorithm-only unit tests to a multi-layered testing system covering every domain with appropriate test types.

---

## 1. Test Types & Strategy

### 1.1 Unit Tests (Rust `#[test]`)

Test individual functions in isolation. Already strong for engine modules; needs expansion to infrastructure.

**Existing coverage (100 tests):**

| Module | Tests | What's tested |
|--------|-------|---------------|
| `sieve.rs` | 7 | mod_inverse, factor_u64, multiplicative_order, BSGS discrete log, generate_primes, pow_mod |
| `proof.rs` | 9 | lucas_v_big, Pocklington proof, Morrison proof, BLS proof, composite rejection |
| `kbn.rs` | 13 | LLR (Mersenne, Riesel k=3/5), Proth, lucas_v_k, BSGS sieve, test_prime integration |
| `carol_kynea.rs` | 8 | Known primes/composites, LLR proof, sieve, decomposition |
| `cullen_woodall.rs` | 8 | Known primes/composites, Proth/LLR proof, sieve, decomposition |
| `twin.rs` | 5 | Known pairs, composite rejection, deterministic proof, sieve intersection |
| `sophie_germain.rs` | 6 | Known SG pairs, composite rejection, deterministic proof, sieve intersection |
| `primorial.rs` | 7 | Known primes/composites (±1), sieve, Pocklington/Morrison proofs |
| `gen_fermat.rs` | 10 | Known GF primes/composites, Pepin proof, sieve, formula verification |
| `repunit.rs` | 7 | Known primes/composites (base 2/3/10), sieve, formula verification |
| `wagstaff.rs` | 7 | Known primes/composites, odd exponent guard, sieve, multiplicative order condition |
| `near_repdigit.rs` | 8 | Candidate building, modular arithmetic, sieve, parameter validation, integration |
| `checkpoint.rs` | 5 | Save/load roundtrip, rotation, corruption fallback, legacy format, clear |

**New unit tests needed:**

| Module | Priority | Tests to add |
|--------|----------|--------------|
| `lib.rs` | High | `has_small_factor` (primes, composites, edge cases), `mr_screened_test` (known primes/composites), `estimate_digits` / `exact_digits` (cross-validation) |
| `fleet.rs` | High | `register` + `get_all`, `heartbeat` (known/unknown worker), `send_command` + delivery, `deregister`, `prune_stale` (before/after timeout) |
| `events.rs` | High | `emit` each event variant, `flush` squashing (group by form, max_details cap), `recent_events` / `recent_notifications` (capacity bounds), notification broadcast |
| `search_manager.rs` | Medium | `SearchParams` serde roundtrip (all 11 variants), `to_args` CLI argument generation, `search_type_name` mapping, concurrent search limit |
| `metrics.rs` | Low | `collect` returns valid ranges (0-100% for percentages, non-negative for GB) |
| `progress.rs` | Low | Atomic counter increment/read, concurrent increments |
| `factorial.rs` | Medium | Factorial computation correctness (n! values), sieve pre-filtering |
| `palindromic.rs` | Medium | Palindrome generation (digit arrays), even-digit skip, base+1 divisibility |
| `db.rs` | Medium | `PrimeFilter::safe_sort_column` (whitelist), `PrimeFilter::safe_sort_dir` (default DESC) |

### 1.2 Integration Tests (Rust `tests/` directory)

Test module interactions and real system behavior. Requires running services (database, HTTP server).

**Categories:**

#### A. Database Integration Tests (`tests/db_integration.rs`)
Require a test PostgreSQL instance (local Docker or test Supabase project).

| Test | Description |
|------|-------------|
| `connect_and_migrate` | Connect to test DB, verify schema exists |
| `insert_and_retrieve_prime` | Insert prime record, query it back |
| `filter_by_form` | Insert primes of different forms, filter by form |
| `filter_by_digits_range` | Insert primes, filter by min/max digits |
| `search_expression` | Text search on expression field |
| `sort_ordering` | Verify sort by digits/form/found_at in ASC/DESC |
| `pagination` | Verify limit/offset pagination |
| `stats_aggregation` | Verify stats (total, by_form, largest) |
| `timeline_buckets` | Verify time-series bucketing |
| `digit_distribution` | Verify digit histogram buckets |
| `prime_detail_by_id` | Fetch single prime with full details |
| `worker_heartbeat_rpc` | Call worker_heartbeat, verify upsert + command return |
| `claim_work_block` | Create job + blocks, claim one, verify status change |
| `claim_block_skip_locked` | Concurrent claims don't double-assign |
| `reclaim_stale_blocks` | Mark block stale after timeout, verify reclaim |
| `search_job_lifecycle` | Create job → generate blocks → claim → complete → job done |

#### B. HTTP API Integration Tests (`tests/api_integration.rs`)
Spin up Axum server in-process with test database.

| Test | Description |
|------|-------------|
| `get_stats` | `GET /api/stats` returns valid Stats JSON |
| `get_primes_default` | `GET /api/primes` returns paginated list |
| `get_primes_filtered` | `GET /api/primes?form=factorial&min_digits=10` |
| `get_prime_detail` | `GET /api/primes/:id` returns PrimeDetail |
| `get_workers` | `GET /api/workers` returns worker list |
| `post_heartbeat` | `POST /api/heartbeat` registers/updates worker |
| `post_heartbeat_unknown` | Heartbeat from unregistered worker |
| `post_command` | `POST /api/command/:worker_id` queues command |
| `get_search_jobs` | `GET /api/search_jobs` returns job list |
| `post_search_job` | `POST /api/search_jobs` creates job + blocks |
| `get_events` | `GET /api/events` returns recent events |
| `get_notifications` | `GET /api/notifications` returns recent notifications |
| `websocket_connect` | Connect to `/ws`, receive initial state |
| `websocket_heartbeat_update` | Send heartbeat, verify WS broadcast |
| `cors_headers` | Verify CORS headers on API responses |
| `body_size_limit` | Verify large request bodies are rejected |
| `request_timeout` | Verify slow requests timeout |
| `static_file_serving` | Verify static files served from `--static-dir` |

#### C. CLI Integration Tests (`tests/cli_integration.rs`)
Use `assert_cmd` crate to test binary execution.

| Test | Description |
|------|-------------|
| `help_flag` | `primehunt --help` exits 0, shows subcommands |
| `factorial_small_range` | `primehunt factorial --start 1 --end 20` finds known primes |
| `palindromic_small_range` | `primehunt palindromic --base 10 --min-digits 1 --max-digits 5` |
| `kbn_small_range` | `primehunt kbn --k 1 --base 2 --min-n 2 --max-n 100` finds Mersenne primes |
| `primorial_small_range` | `primehunt primorial --start 2 --end 30` |
| `cullen_woodall_small` | `primehunt cullen_woodall --min-n 1 --max-n 30` |
| `wagstaff_small` | `primehunt wagstaff --min-exp 3 --max-exp 50` |
| `carol_kynea_small` | `primehunt carol_kynea --min-n 1 --max-n 30` |
| `twin_small` | `primehunt twin --k 3 --base 2 --min-n 1 --max-n 100` |
| `sophie_germain_small` | `primehunt sophie_germain --k 1 --base 2 --min-n 2 --max-n 100` |
| `repunit_small` | `primehunt repunit --base 10 --min-n 2 --max-n 50` |
| `gen_fermat_small` | `primehunt gen_fermat --fermat-exp 1 --min-base 2 --max-base 100` |
| `checkpoint_resume` | Start search, kill, resume from checkpoint |
| `invalid_args` | Missing required args → non-zero exit |
| `database_url_env` | `DATABASE_URL` env var picked up |

#### D. Worker Coordination Tests (`tests/coordination_integration.rs`)
Test the full worker lifecycle with real HTTP or PG connections.

| Test | Description |
|------|-------------|
| `worker_register_heartbeat_deregister` | Full worker lifecycle via HTTP |
| `pg_worker_heartbeat` | PgWorkerClient heartbeat via SQL |
| `pg_worker_claim_block` | Claim and complete work blocks via PG |
| `worker_stop_command` | Send stop, verify worker receives it |
| `stale_worker_pruning` | Worker times out, gets pruned |
| `block_reclamation` | Stale block reclaimed by another worker |
| `coordination_client_auto_select` | HTTP vs PG client selection |

### 1.3 Property-Based Tests (proptest / quickcheck)

Test invariants over random inputs. Catches edge cases that hand-picked test vectors miss.

| Module | Property | Generator |
|--------|----------|-----------|
| `sieve.rs` | `pow_mod(b, e, m) == b^e % m` for all b,e,m>0 | Random u64 triples |
| `sieve.rs` | `mod_inverse(a, p) * a ≡ 1 (mod p)` when inverse exists | Random a, prime p |
| `sieve.rs` | `gcd(a, b) == gcd(b, a)` and divides both | Random u32 pairs |
| `sieve.rs` | `generate_primes(n)` all pass Miller-Rabin | Random limits 10..10^6 |
| `lib.rs` | `has_small_factor(p)` is false for all primes p > 311 | Random primes |
| `lib.rs` | `estimate_digits(n)` is within 1 of `exact_digits(n)` | Random large integers |
| `kbn.rs` | Proth test agrees with GMP `is_probably_prime` for k*2^n+1 | Random k, n |
| `kbn.rs` | LLR test agrees with GMP for k*2^n-1 (k < 2^n) | Random k, n |
| `proof.rs` | Pocklington proof only succeeds for actual primes | Random n values |
| `checkpoint.rs` | `load(save(x)) == x` for all checkpoint variants | Random checkpoint data |
| `search_manager.rs` | `SearchParams` serde roundtrip: `deserialize(serialize(x)) == x` | Random params |
| `fleet.rs` | `register` then `heartbeat` always returns `(true, _)` | Random worker IDs |
| `events.rs` | `recent_events` never exceeds `RECENT_EVENTS_CAP` | Emit N random events |
| `db.rs` | `safe_sort_column` always returns whitelisted column | Random strings |
| `near_repdigit.rs` | `build_candidate(k,d,m)` is always a palindrome | Random valid k,d,m |

### 1.4 Benchmark Tests (criterion)

Measure performance of hot paths. Guard against regressions.

| Benchmark | What it measures |
|-----------|-----------------|
| `bench_sieve_generate` | `generate_primes(10^6)` throughput |
| `bench_pow_mod` | `pow_mod` for various exponent sizes |
| `bench_bsgs_sieve` | BSGS sieve for k*2^n-1 (1000-block) |
| `bench_llr_test` | LLR test for 2^1279-1 (Mersenne prime) |
| `bench_proth_test` | Proth test for known Proth prime |
| `bench_mr_screened` | `mr_screened_test` (2-round pre-screen vs full 25 rounds) |
| `bench_has_small_factor` | Trial division for random 1000-digit numbers |
| `bench_palindrome_generate` | Palindrome batch generation (10^6 range) |
| `bench_factorial_incremental` | Incremental factorial computation (n=1000..1100) |
| `bench_checkpoint_save_load` | Checkpoint serialization roundtrip |

### 1.5 Frontend Unit Tests (Vitest + Testing Library)

Test React components, hooks, and utilities in isolation.

#### Utility Tests

| File | Tests |
|------|-------|
| `lib/format.ts` | Number formatting, digit abbreviation, time formatting |
| `lib/utils.ts` | `cn()` class merger edge cases |

#### Hook Tests (with mocked Supabase)

| Hook | Tests |
|------|-------|
| `use-primes.ts` | Fetch primes, apply filters, handle errors, pagination |
| `use-stats.ts` | Fetch stats RPC, handle empty data |
| `use-timeline.ts` | Fetch timeline, bucket parsing |
| `use-distribution.ts` | Fetch distribution, bucket ranges |
| `use-prime-realtime.ts` | Subscribe to INSERT events, handle new prime |
| `use-websocket.ts` | Connect, receive messages, reconnect on close |
| `use-theme.ts` | Toggle theme, persist to localStorage |
| `use-mobile.ts` | Responsive breakpoint detection |

#### Component Tests

| Component | Tests |
|-----------|-------|
| `app-header.tsx` | Renders nav links, active page highlight, theme toggle |
| `prime-notifier.tsx` | Shows toast on new prime, auto-dismiss |
| `search-card.tsx` | Renders search params, status, progress |
| `host-node-card.tsx` | Renders worker info, metrics bars, status badge |
| `process-row.tsx` | Renders process details, action buttons |
| `metrics-bar.tsx` | Renders percentage bar, color thresholds |
| `new-search-dialog.tsx` | Form validation, submit, error display |
| `add-server-dialog.tsx` | Form inputs, hostname validation |
| `charts/discovery-timeline.tsx` | Renders Recharts with data, empty state |
| `charts/digit-distribution.tsx` | Renders histogram, axis labels |
| `charts/throughput-gauge.tsx` | Renders gauge, value display |

#### Page Tests

| Page | Tests |
|------|-------|
| `app/page.tsx` | Dashboard renders stat cards, primes table, charts |
| `app/login/page.tsx` | Login form, submit, error handling, redirect |
| `app/browse/page.tsx` | Prime browser: filters, table, pagination, detail dialog |
| `app/searches/page.tsx` | Search list, new search dialog, status updates |
| `app/fleet/page.tsx` | Fleet view: worker cards, metrics, commands |
| `app/docs/page.tsx` | Doc viewer: search, markdown render, KaTeX |

#### Context Tests

| Context | Tests |
|---------|-------|
| `auth-context.tsx` | AuthProvider wraps children, AuthGuard redirects unauthenticated, useAuth returns session |
| `websocket-context.tsx` | WebSocketProvider connects, provides fleet/search data, handles disconnect |

### 1.6 Frontend E2E Tests (Playwright)

Test complete user workflows in a real browser.

| Test | Workflow |
|------|----------|
| `login_flow` | Navigate to login → enter credentials → redirect to dashboard |
| `dashboard_loads` | Stats cards render, primes table has rows, charts visible |
| `browse_primes` | Navigate to browse → filter by form → sort by digits → click row → detail dialog |
| `start_search` | Navigate to searches → click new → fill form → submit → card appears |
| `fleet_monitoring` | Navigate to fleet → worker cards visible → metrics updating |
| `live_prime_notification` | When new prime inserted → toast appears |
| `theme_toggle` | Click theme toggle → dark mode → refresh → persists |
| `responsive_layout` | Dashboard renders correctly at mobile viewport |
| `docs_search` | Navigate to docs → search for keyword → result highlights |
| `pagination` | Browse page → next page → previous page → page numbers correct |

### 1.7 Database Schema Tests

Verify migrations and data integrity.

| Test | Description |
|------|-------------|
| `migrations_apply_cleanly` | Run all 4 migrations on fresh DB |
| `migrations_idempotent` | Running migrations twice doesn't error |
| `primes_table_constraints` | NOT NULL, type checks on primes table |
| `workers_table_pk` | worker_id is unique primary key |
| `search_jobs_status_enum` | Status values constrained to valid set |
| `work_blocks_fk` | search_job_id references search_jobs |
| `rls_policies_active` | RLS blocks unauthenticated reads |
| `rpc_functions_exist` | worker_heartbeat, claim_work_block, reclaim_stale_blocks callable |
| `realtime_enabled` | Primes table has realtime enabled |
| `indices_exist` | Performance indices on form, digits, found_at |

### 1.8 Deployment Tests

Test deployment scripts and systemd units.

| Test | Description |
|------|-------------|
| `deploy_script_syntax` | `bash -n deploy.sh` — no syntax errors |
| `systemd_unit_valid` | `systemd-analyze verify` on both unit files |
| `deploy_script_env_vars` | Required env vars documented and checked |
| `binary_builds_release` | `cargo build --release` succeeds |
| `frontend_builds` | `npm run build` in frontend/ succeeds |
| `static_export_exists` | `frontend/out/` directory created after build |

### 1.9 Security Tests

| Test | Description |
|------|-------------|
| `sql_injection_sort_column` | PrimeFilter rejects arbitrary SQL in sort_by |
| `sql_injection_search_field` | Search parameter is properly escaped |
| `body_size_limit_enforced` | >1MB request body rejected |
| `cors_restricted` | Verify CORS doesn't allow arbitrary origins in production |
| `no_secret_in_static` | Static frontend build doesn't contain API keys |
| `supabase_rls_unauthenticated` | Unauthenticated Supabase requests blocked by RLS |

---

## 2. Test Matrix by Module

Every source module mapped to the test types it needs:

| Module | Unit | Integration | Property | Benchmark | E2E |
|--------|------|-------------|----------|-----------|-----|
| `sieve.rs` | 7 existing | - | pow_mod, gcd | sieve_generate, pow_mod | - |
| `proof.rs` | 9 existing | - | pocklington | - | - |
| `kbn.rs` | 13 existing | CLI | proth, llr | llr_test, bsgs | - |
| `carol_kynea.rs` | 8 existing | CLI | - | - | - |
| `cullen_woodall.rs` | 8 existing | CLI | - | - | - |
| `twin.rs` | 5 existing | CLI | - | - | - |
| `sophie_germain.rs` | 6 existing | CLI | - | - | - |
| `primorial.rs` | 7 existing | CLI | - | - | - |
| `gen_fermat.rs` | 10 existing | CLI | - | - | - |
| `repunit.rs` | 7 existing | CLI | - | - | - |
| `wagstaff.rs` | 7 existing | CLI | - | - | - |
| `near_repdigit.rs` | 8 existing | CLI | palindrome | - | - |
| `checkpoint.rs` | 5 existing | resume | roundtrip | save_load | - |
| `lib.rs` | **+5 new** | - | small_factor, digits | has_small_factor, mr | - |
| `fleet.rs` | **+7 new** | - | register/heartbeat | - | - |
| `events.rs` | **+8 new** | WebSocket | cap bounds | - | - |
| `search_manager.rs` | **+5 new** | launch/stop | serde roundtrip | - | - |
| `metrics.rs` | **+2 new** | - | valid ranges | - | - |
| `progress.rs` | **+3 new** | - | concurrent incr | - | - |
| `db.rs` | **+3 new** | **+16 new** | safe_sort | - | - |
| `dashboard.rs` | - | **+18 new** | - | - | - |
| `worker_client.rs` | - | **+3 new** | - | - | - |
| `pg_worker.rs` | - | **+3 new** | - | - | - |
| `main.rs` | - | **+14 new** | - | - | - |
| `deploy.rs` | - | **+3 new** | - | - | - |
| `factorial.rs` | **+3 new** | CLI | - | factorial_incr | - |
| `palindromic.rs` | **+4 new** | CLI | - | palindrome_gen | - |
| Frontend hooks | **+16 new** | - | - | - | - |
| Frontend components | **+18 new** | - | - | - | - |
| Frontend pages | **+6 new** | - | - | - | **+10 new** |
| Frontend contexts | **+2 new** | - | - | - | - |
| Database schema | - | **+10 new** | - | - | - |

---

## 3. Test Data Strategy

### 3.1 Known Prime Vectors

Each engine module already uses OEIS-verified prime/composite test vectors. These should be centralized into a shared test fixture module.

```rust
// tests/fixtures/known_primes.rs
pub const MERSENNE_PRIME_EXPONENTS: &[u32] = &[2, 3, 5, 7, 13, 17, 19, 31];
pub const FACTORIAL_PRIMES_PLUS: &[u32] = &[1, 2, 3, 11, 27, 37, 41, 73, 77, 116, 154];
pub const FACTORIAL_PRIMES_MINUS: &[u32] = &[3, 4, 6, 7, 12, 14, 30, 32, 33, 38, 94, 166];
pub const WAGSTAFF_EXPONENTS: &[u32] = &[3, 5, 7, 11, 13, 17, 19, 23, 31, 43, 61, 79, 101, 127];
// ... etc for all forms
```

### 3.2 Test Database

- **Local dev**: Docker PostgreSQL container with test migrations applied
- **CI**: GitHub Actions service container (postgres:16)
- **Isolation**: Each test gets its own schema or uses transactions that roll back

### 3.3 Frontend Mock Data

- **Supabase mocks**: MSW (Mock Service Worker) intercepts Supabase REST/Realtime calls
- **WebSocket mocks**: Custom mock WebSocket server for coordination data
- **Fixture files**: JSON fixtures for primes, stats, timeline, distribution

---

## 4. Test Organization

```
primehunt/
├── src/                          # Source + unit tests (existing pattern)
│   ├── sieve.rs                  # #[cfg(test)] mod tests { ... }
│   ├── kbn.rs                    # #[cfg(test)] mod tests { ... }
│   └── ...
├── tests/                        # Integration tests (new)
│   ├── fixtures/
│   │   ├── known_primes.rs       # Shared prime test vectors
│   │   └── mod.rs
│   ├── cli_integration.rs        # Binary execution tests
│   ├── db_integration.rs         # Database tests (requires PG)
│   ├── api_integration.rs        # HTTP API tests
│   ├── coordination_integration.rs # Worker coordination tests
│   └── common/
│       └── mod.rs                # Test helpers (DB setup, server spawn)
├── benches/                      # Benchmark tests (new)
│   ├── sieve_bench.rs
│   ├── kbn_bench.rs
│   ├── proof_bench.rs
│   └── checkpoint_bench.rs
├── frontend/
│   ├── __tests__/                # Frontend unit tests (new)
│   │   ├── hooks/
│   │   ├── components/
│   │   ├── pages/
│   │   └── lib/
│   ├── e2e/                      # Playwright E2E tests (new)
│   │   ├── dashboard.spec.ts
│   │   ├── browse.spec.ts
│   │   ├── login.spec.ts
│   │   └── fleet.spec.ts
│   └── __mocks__/                # MSW handlers (new)
│       ├── supabase.ts
│       └── websocket.ts
└── supabase/
    └── tests/                    # Schema tests (new)
        └── schema_test.sql
```

---

## 5. Coverage Targets

| Domain | Current | Phase 1 Target | Phase 2 Target | Phase 3 Target |
|--------|---------|----------------|----------------|----------------|
| Engine algorithms | ~90% | 90% (maintain) | 95% (+property) | 95% |
| Server infrastructure | ~0% | 60% (unit) | 80% (+integration) | 90% |
| Frontend | 0% | 40% (hooks/utils) | 70% (+components) | 85% (+E2E) |
| Database | 0% | 50% (migrations) | 80% (+CRUD) | 90% |
| CLI | 0% | 30% (basic args) | 70% (+all forms) | 90% (+resume) |
| **Overall** | **~25%** | **~50%** | **~75%** | **~85%** |

---

## 6. Test Naming Conventions

### Rust

```rust
#[test]
fn <module>_<function>_<scenario>_<expected>() { ... }

// Examples:
fn fleet_heartbeat_known_worker_returns_true() { ... }
fn db_filter_by_form_returns_matching_records() { ... }
fn sieve_pow_mod_large_exponent_correct() { ... }
```

### Frontend

```typescript
describe('<ComponentName>', () => {
  it('renders <what> when <condition>', () => { ... });
  it('calls <handler> when <action>', () => { ... });
});

// Examples:
describe('useStats', () => {
  it('returns stats data after successful fetch', () => { ... });
  it('returns error when RPC fails', () => { ... });
});
```

---

## 7. Test Dependencies (New)

### Rust (`Cargo.toml` dev-dependencies)

```toml
[dev-dependencies]
tempfile = "3.25.0"        # existing
assert_cmd = "2"           # CLI integration tests
predicates = "3"           # CLI output assertions
proptest = "1"             # Property-based testing
criterion = "0.5"          # Benchmarks
tokio-test = "0.4"         # Async test utilities
axum-test = "16"           # HTTP API testing
mockito = "1"              # HTTP mock server
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "chrono"] }
```

### Frontend (`package.json` devDependencies)

```json
{
  "devDependencies": {
    "vitest": "^3",
    "@testing-library/react": "^16",
    "@testing-library/jest-dom": "^6",
    "@testing-library/user-event": "^14",
    "msw": "^2",
    "@playwright/test": "^1",
    "jsdom": "^25"
  }
}
```

---

## 8. CI/CD Pipeline

### GitHub Actions Workflow

```yaml
# .github/workflows/test.yml
name: Tests
on: [push, pull_request]

jobs:
  rust-unit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: sudo apt-get install -y libgmp-dev
      - run: cargo test --lib

  rust-integration:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:16
        env:
          POSTGRES_PASSWORD: test
          POSTGRES_DB: primehunt_test
        ports: ['5432:5432']
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: sudo apt-get install -y libgmp-dev
      - run: cargo test --test '*'
        env:
          DATABASE_URL: postgres://postgres:test@localhost/primehunt_test

  rust-benchmarks:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: sudo apt-get install -y libgmp-dev
      - run: cargo bench --no-run  # compile only, don't run in CI

  frontend-unit:
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: frontend
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: '22' }
      - run: npm ci
      - run: npm test

  frontend-e2e:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: '22' }
      - run: cd frontend && npm ci && npx playwright install
      - run: cd frontend && npx playwright test

  frontend-lint:
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: frontend
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: '22' }
      - run: npm ci
      - run: npm run lint
```

---

## 9. Cross-References

- **Test Roadmap**: `docs/roadmaps/testing.md` — phased implementation plan
- **Test Infrastructure**: `docs/testing/infrastructure.md` — tooling setup and running tests
- **Engine Roadmap**: `docs/roadmaps/engine.md` — algorithm development priorities
- **Server Roadmap**: `docs/roadmaps/server.md` — infrastructure priorities
- **Frontend Roadmap**: `docs/roadmaps/frontend.md` — dashboard priorities
