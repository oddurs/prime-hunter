# Test Infrastructure Guide

How to set up, run, and maintain the darkreach test suite across all domains.

---

## 1. Rust Testing

### 1.1 Running Tests

```bash
# All unit tests (fast, no external deps)
cargo test --lib

# Specific module
cargo test --lib fleet::tests
cargo test --lib kbn::tests::llr_mersenne_primes

# With output (see eprintln! messages)
cargo test --lib -- --nocapture

# Sequential execution (for debugging race conditions)
cargo test --lib -- --test-threads=1

# Integration tests only (requires PostgreSQL)
cargo test --test '*'

# Specific integration test file
cargo test --test db_integration
cargo test --test cli_integration

# All tests (unit + integration)
cargo test

# Release mode (tests run faster, closer to production behavior)
cargo test --release
```

### 1.2 Test Configuration

Unit tests live inside each source file using `#[cfg(test)]`:

```rust
// src/fleet.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_adds_worker() {
        let mut fleet = Fleet::new();
        fleet.register("w1".into(), "host1".into(), 8, "kbn".into(), "{}".into());
        assert_eq!(fleet.get_all().len(), 1);
    }
}
```

Integration tests live in `tests/` directory and compile as separate crates:

```rust
// tests/db_integration.rs
use darkreach::db::Database;

#[tokio::test]
async fn insert_and_retrieve_prime() {
    let db = common::setup_test_db().await;
    // ...
}
```

### 1.3 Dev Dependencies

Current:
```toml
[dev-dependencies]
tempfile = "3.25.0"
```

Full (after all phases):
```toml
[dev-dependencies]
tempfile = "3.25.0"
assert_cmd = "2"              # CLI binary testing
predicates = "3"              # Output assertion matchers
proptest = "1"                # Property-based testing
tokio-test = "0.4"            # Async test utilities

[[bench]]
name = "sieve_bench"
harness = false

[[bench]]
name = "kbn_bench"
harness = false

[[bench]]
name = "proof_bench"
harness = false

[[bench]]
name = "core_bench"
harness = false
```

Plus criterion in dev-dependencies:
```toml
criterion = { version = "0.5", features = ["html_reports"] }
```

### 1.4 Test Database Setup

#### Option A: Docker (recommended for local dev)

```bash
# Start test PostgreSQL
docker run -d --name darkreach-test-pg \
  -e POSTGRES_PASSWORD=test \
  -e POSTGRES_DB=darkreach_test \
  -p 5433:5432 \
  postgres:16

# Apply migrations
export DATABASE_URL=postgres://postgres:test@localhost:5433/darkreach_test
for f in supabase/migrations/*.sql; do
  psql "$DATABASE_URL" -f "$f"
done

# Run integration tests
cargo test --test '*'

# Cleanup
docker rm -f darkreach-test-pg
```

#### Option B: Docker Compose

```yaml
# docker-compose.test.yml
services:
  postgres:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: test
      POSTGRES_DB: darkreach_test
    ports:
      - "5433:5432"
    volumes:
      - ./supabase/migrations:/docker-entrypoint-initdb.d
```

```bash
docker compose -f docker-compose.test.yml up -d
DATABASE_URL=postgres://postgres:test@localhost:5433/darkreach_test cargo test --test '*'
docker compose -f docker-compose.test.yml down
```

#### Option C: GitHub Actions Service Container

```yaml
services:
  postgres:
    image: postgres:16
    env:
      POSTGRES_PASSWORD: test
      POSTGRES_DB: darkreach_test
    ports: ['5432:5432']
    options: >-
      --health-cmd pg_isready
      --health-interval 10s
      --health-timeout 5s
      --health-retries 5
```

### 1.5 Test Helpers

Shared test utilities in `tests/common/mod.rs`:

```rust
use darkreach::db::Database;
use anyhow::Result;

/// Connect to test database and apply migrations.
pub async fn setup_test_db() -> Database {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:test@localhost:5433/darkreach_test".into());
    Database::connect(&url).await.expect("failed to connect to test DB")
}

/// Spawn the dashboard server on a random port for API testing.
pub async fn spawn_test_server() -> (String, tokio::task::JoinHandle<()>) {
    let port = portpicker::pick_unused_port().expect("no free port");
    let url = format!("http://localhost:{}", port);
    let handle = tokio::spawn(async move {
        darkreach::dashboard::run(port, &db_url, &checkpoint_path, None)
            .await
            .unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    (url, handle)
}
```

### 1.6 Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate HTML coverage report
cargo tarpaulin --lib --out Html --output-dir coverage/

# With integration tests
cargo tarpaulin --out Html --output-dir coverage/

# Coverage for specific module
cargo tarpaulin --lib --out Html --output-dir coverage/ -- fleet::tests
```

### 1.7 Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench sieve_bench

# Generate HTML report (opens in browser)
cargo bench -- --output-format html

# Compare against baseline
cargo bench -- --save-baseline main
# ... make changes ...
cargo bench -- --baseline main
```

Benchmark files live in `benches/`:

```rust
// benches/sieve_bench.rs
use criterion::{criterion_group, criterion_main, Criterion};
use darkreach::sieve;

fn bench_generate_primes(c: &mut Criterion) {
    c.bench_function("generate_primes_1m", |b| {
        b.iter(|| sieve::generate_primes(1_000_000))
    });
}

criterion_group!(benches, bench_generate_primes);
criterion_main!(benches);
```

### 1.8 Property-Based Testing

```rust
// tests/property_tests.rs
use proptest::prelude::*;
use darkreach::sieve;

proptest! {
    #[test]
    fn pow_mod_matches_big_int(
        base in 1u64..1000,
        exp in 1u64..100,
        modulus in 2u64..10000
    ) {
        let result = sieve::pow_mod(base, exp, modulus);
        let expected = {
            use rug::Integer;
            let b = Integer::from(base);
            let e = Integer::from(exp);
            let m = Integer::from(modulus);
            b.pow_mod(&e, &m).unwrap().to_u64().unwrap()
        };
        prop_assert_eq!(result, expected);
    }
}
```

---

## 2. Frontend Testing

### 2.1 Setup

```bash
cd frontend

# Install test dependencies
npm install -D vitest @testing-library/react @testing-library/jest-dom \
  @testing-library/user-event msw jsdom

# For E2E
npm install -D @playwright/test
npx playwright install
```

### 2.2 Vitest Configuration

```typescript
// frontend/vitest.config.ts
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    setupFiles: ['./test-setup.ts'],
    globals: true,
    css: false,
    include: ['__tests__/**/*.test.{ts,tsx}'],
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
})
```

```typescript
// frontend/test-setup.ts
import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach } from 'vitest'
import { server } from './__mocks__/server'

beforeAll(() => server.listen())
afterEach(() => {
  cleanup()
  server.resetHandlers()
})
afterAll(() => server.close())
```

### 2.3 Running Frontend Tests

```bash
cd frontend

# Run all tests
npm test

# Watch mode (re-run on file changes)
npm run test:watch

# With coverage
npm run test:coverage

# Specific test file
npx vitest run __tests__/hooks/use-stats.test.ts

# E2E tests
npx playwright test

# E2E with browser visible
npx playwright test --headed

# E2E specific test
npx playwright test e2e/login.spec.ts
```

### 2.4 MSW Mock Setup

```typescript
// frontend/__mocks__/handlers.ts
import { http, HttpResponse } from 'msw'

const SUPABASE_URL = 'https://test.supabase.co'

export const handlers = [
  // Mock Supabase REST API
  http.get(`${SUPABASE_URL}/rest/v1/primes`, () => {
    return HttpResponse.json([
      { id: 1, form: 'factorial', expression: '3!+1', digits: 1, proof_method: 'deterministic' },
      { id: 2, form: 'kbn', expression: '2^31-1', digits: 10, proof_method: 'LLR' },
    ])
  }),

  // Mock Supabase RPC
  http.post(`${SUPABASE_URL}/rest/v1/rpc/get_stats`, () => {
    return HttpResponse.json({ total: 42, largest_digits: 1000 })
  }),
]

// frontend/__mocks__/server.ts
import { setupServer } from 'msw/node'
import { handlers } from './handlers'

export const server = setupServer(...handlers)
```

### 2.5 Example Test Files

**Hook test:**
```typescript
// frontend/__tests__/hooks/use-stats.test.ts
import { renderHook, waitFor } from '@testing-library/react'
import { useStats } from '@/hooks/use-stats'

describe('useStats', () => {
  it('returns stats after successful fetch', async () => {
    const { result } = renderHook(() => useStats())
    await waitFor(() => {
      expect(result.current.data).toBeDefined()
      expect(result.current.data.total).toBe(42)
    })
  })

  it('handles error gracefully', async () => {
    server.use(
      http.post('*/rpc/get_stats', () => HttpResponse.error())
    )
    const { result } = renderHook(() => useStats())
    await waitFor(() => {
      expect(result.current.error).toBeDefined()
    })
  })
})
```

**Component test:**
```typescript
// frontend/__tests__/components/metrics-bar.test.tsx
import { render, screen } from '@testing-library/react'
import { MetricsBar } from '@/components/metrics-bar'

describe('MetricsBar', () => {
  it('renders percentage value', () => {
    render(<MetricsBar label="CPU" value={75} />)
    expect(screen.getByText('75%')).toBeInTheDocument()
  })

  it('applies warning color above threshold', () => {
    const { container } = render(<MetricsBar label="CPU" value={90} />)
    expect(container.querySelector('.bg-red-500')).toBeTruthy()
  })
})
```

### 2.6 Playwright E2E Configuration

```typescript
// frontend/playwright.config.ts
import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    baseURL: 'http://localhost:3000',
    trace: 'on-first-retry',
  },
  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:3000',
    reuseExistingServer: !process.env.CI,
  },
})
```

**Example E2E test:**
```typescript
// frontend/e2e/dashboard.spec.ts
import { test, expect } from '@playwright/test'

test('dashboard loads and shows stats', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByText('Total Primes')).toBeVisible()
  await expect(page.getByRole('table')).toBeVisible()
})

test('theme toggle switches to dark mode', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: /theme/i }).click()
  await expect(page.locator('html')).toHaveClass(/dark/)
})
```

---

## 3. CI/CD Pipeline

### 3.1 GitHub Actions Workflow

```yaml
# .github/workflows/test.yml
name: Tests

on:
  push:
    branches: [master]
  pull_request:
    branches: [master]

env:
  CARGO_TERM_COLOR: always

jobs:
  rust-unit:
    name: Rust Unit Tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: sudo apt-get update && sudo apt-get install -y libgmp-dev
      - run: cargo test --lib

  rust-integration:
    name: Rust Integration Tests
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:16
        env:
          POSTGRES_PASSWORD: test
          POSTGRES_DB: darkreach_test
        ports: ['5432:5432']
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: sudo apt-get update && sudo apt-get install -y libgmp-dev
      - name: Apply migrations
        run: |
          for f in supabase/migrations/*.sql; do
            PGPASSWORD=test psql -h localhost -U postgres -d darkreach_test -f "$f"
          done
      - run: cargo test --test '*'
        env:
          DATABASE_URL: postgres://postgres:test@localhost/darkreach_test

  rust-lint:
    name: Rust Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
      - run: sudo apt-get update && sudo apt-get install -y libgmp-dev
      - run: cargo clippy -- -D warnings
      - run: cargo fmt --check

  rust-bench-check:
    name: Benchmark Compilation
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: sudo apt-get update && sudo apt-get install -y libgmp-dev
      - run: cargo bench --no-run

  frontend-lint:
    name: Frontend Lint
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: frontend
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '22'
          cache: 'npm'
          cache-dependency-path: frontend/package-lock.json
      - run: npm ci
      - run: npm run lint

  frontend-unit:
    name: Frontend Unit Tests
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: frontend
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '22'
          cache: 'npm'
          cache-dependency-path: frontend/package-lock.json
      - run: npm ci
      - run: npm test

  frontend-e2e:
    name: Frontend E2E Tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '22'
          cache: 'npm'
          cache-dependency-path: frontend/package-lock.json
      - run: cd frontend && npm ci
      - run: cd frontend && npx playwright install --with-deps
      - run: cd frontend && npx playwright test
```

### 3.2 Local Pre-commit Hook

Optional pre-commit hook to run fast tests before pushing:

```bash
#!/bin/bash
# .git/hooks/pre-push
set -e
echo "Running unit tests..."
cargo test --lib --quiet
echo "Running frontend lint..."
(cd frontend && npm run lint --quiet)
echo "All checks passed."
```

---

## 4. Test Organization Summary

```
darkreach/
├── src/                              # Unit tests inline (existing pattern)
│   ├── sieve.rs                      # mod tests { ... }
│   ├── kbn.rs                        # mod tests { ... }
│   ├── fleet.rs                      # mod tests { ... } (new)
│   ├── events.rs                     # mod tests { ... } (new)
│   ├── lib.rs                        # mod tests { ... } (new)
│   └── ...
├── tests/                            # Integration tests
│   ├── common/
│   │   └── mod.rs                    # setup_test_db, spawn_test_server
│   ├── db_integration.rs             # Database CRUD tests
│   ├── api_integration.rs            # HTTP endpoint tests
│   ├── cli_integration.rs            # Binary execution tests
│   ├── coordination_integration.rs   # Worker lifecycle tests
│   ├── security_tests.rs             # Injection, limits, CORS
│   └── property_tests.rs             # proptest invariants
├── benches/                          # Criterion benchmarks
│   ├── sieve_bench.rs
│   ├── kbn_bench.rs
│   ├── proof_bench.rs
│   └── core_bench.rs
├── frontend/
│   ├── vitest.config.ts              # Vitest configuration
│   ├── test-setup.ts                 # Test setup (cleanup, MSW)
│   ├── playwright.config.ts          # Playwright configuration
│   ├── __tests__/                    # Unit tests
│   │   ├── hooks/
│   │   │   ├── use-primes.test.ts
│   │   │   ├── use-stats.test.ts
│   │   │   └── ...
│   │   ├── components/
│   │   │   ├── metrics-bar.test.tsx
│   │   │   ├── search-card.test.tsx
│   │   │   └── ...
│   │   ├── pages/
│   │   │   ├── dashboard.test.tsx
│   │   │   └── ...
│   │   └── lib/
│   │       ├── format.test.ts
│   │       └── utils.test.ts
│   ├── __mocks__/                    # MSW mock handlers
│   │   ├── handlers.ts
│   │   └── server.ts
│   └── e2e/                          # Playwright E2E tests
│       ├── dashboard.spec.ts
│       ├── browse.spec.ts
│       ├── login.spec.ts
│       └── fleet.spec.ts
├── docker-compose.test.yml           # Test database
└── .github/
    └── workflows/
        └── test.yml                  # CI pipeline
```

---

## 5. Quick Reference

| Task | Command |
|------|---------|
| Run all Rust unit tests | `cargo test --lib` |
| Run specific module tests | `cargo test --lib fleet::tests` |
| Run integration tests | `cargo test --test '*'` |
| Run benchmarks | `cargo bench` |
| Run property tests | `cargo test --test property_tests` |
| Generate Rust coverage | `cargo tarpaulin --lib --out Html` |
| Run frontend unit tests | `cd frontend && npm test` |
| Run frontend tests (watch) | `cd frontend && npm run test:watch` |
| Run frontend E2E | `cd frontend && npx playwright test` |
| Run E2E (headed) | `cd frontend && npx playwright test --headed` |
| Start test database | `docker compose -f docker-compose.test.yml up -d` |
| Stop test database | `docker compose -f docker-compose.test.yml down` |
| Check all (fast) | `cargo test --lib && cd frontend && npm run lint` |
