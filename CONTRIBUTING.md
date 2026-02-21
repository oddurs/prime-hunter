# Contributing to darkreach

Thanks for your interest in contributing to darkreach! This project hunts special-form prime numbers at scale using Rust, GMP, and distributed computing.

## Prerequisites

- **Rust** (stable toolchain) with `rustfmt` and `clippy`
- **GMP** library:
  - Linux: `sudo apt install build-essential libgmp-dev m4`
  - macOS: `brew install gmp`
- **Node.js 22+** and npm (for the frontend)
- **PostgreSQL** (for integration tests — use `docker-compose.test.yml` or Supabase)

## Setup

```bash
git clone https://github.com/oddurs/prime-hunter.git
cd prime-hunter

# Build the engine
cargo build

# Install frontend dependencies
cd frontend && npm install && cd ..

# Install the pre-commit hook
ln -sf ../../scripts/pre-commit .git/hooks/pre-commit
```

## Development Commands

### Engine (Rust)

```bash
cargo test                    # All unit tests (449 passing)
cargo fmt                     # Format code
cargo fmt -- --check          # Check formatting
cargo clippy                  # Lint
cargo build --release         # Optimized build
cargo bench                   # Run benchmarks
```

### Integration Tests

```bash
# Start test database
docker compose -f docker-compose.test.yml up -d

# Run integration tests
TEST_DATABASE_URL=postgres://postgres:test@localhost:5432/darkreach_test \
  cargo test --test db_integration -- --test-threads=1

TEST_DATABASE_URL=postgres://postgres:test@localhost:5432/darkreach_test \
  cargo test --test api_integration -- --test-threads=1
```

### Frontend (Next.js)

```bash
cd frontend
npm run dev             # Dev server at localhost:3000
npm run build           # Static export to out/
npm test                # Vitest unit tests
npm run test:e2e        # Playwright E2E tests
npm run lint            # ESLint
npx tsc --noEmit        # Type check
```

### Local Dev Stack

```bash
./scripts/dev.sh                    # Backend (:7001) + frontend (:3001)
./scripts/dev.sh --remote <url>     # Local frontend proxying to remote API
./scripts/dev-status.sh             # Check running services
./scripts/dev-down.sh               # Stop everything
```

## Pre-commit Hook

The hook at `scripts/pre-commit` runs on every commit:

1. `cargo fmt -- --check` — formatting must be clean
2. `cargo test --quiet` — all tests must pass
3. `npx tsc --noEmit` — TypeScript check (when frontend files staged)

Install:
```bash
ln -sf ../../scripts/pre-commit .git/hooks/pre-commit
```

## Code Style

- **Rust**: `rustfmt.toml` enforces 100-char line width, 2021 edition
- **TypeScript**: 2-space indent, strict mode
- **Comments**: This codebase is a teaching tool — see `.claude/commenting-standards.md` for the full academic commenting standard

### Comment density targets

| Area | Target | Focus |
|------|--------|-------|
| Engine (algorithms) | ~30-40% | Math proofs, complexity, invariants, OEIS refs |
| Server (infrastructure) | ~20-30% | Data flow, API contracts, error handling |
| Frontend (UI) | ~15-25% | Data sources, user interactions, state |

## Project Structure

| Domain | Directory | Documentation |
|--------|-----------|---------------|
| Engine | `src/` (algorithm files) | `src/CLAUDE.md` |
| Server | `src/` (infra files) | `src/CLAUDE.md` |
| Frontend | `frontend/` | `frontend/CLAUDE.md` |
| Deployment | `deploy/` | `deploy/CLAUDE.md` |
| Database | `supabase/` | `supabase/CLAUDE.md` |
| Research | `docs/` | `docs/CLAUDE.md` |
| Tests | `tests/` | `tests/CLAUDE.md` |

Each domain has its own CLAUDE.md with module maps, conventions, and step-by-step guides.

## Common Tasks

### Adding a new search form

1. Create `src/<form>.rs` following the sieve → test → proof → log pipeline
2. Add checkpoint variant in `src/checkpoint.rs`
3. Add CLI subcommand in `src/main.rs` + dispatch in `src/cli.rs`
4. Add module in `src/lib.rs`
5. Update `src/search_manager.rs` and `src/deploy.rs`
6. Add unit tests with known values from OEIS
7. See `src/CLAUDE.md` for detailed guide

### Adding a new API route

1. Add handler in `src/dashboard/routes_*.rs`
2. Register in `src/dashboard/mod.rs`
3. Add DB query in `src/db/*.rs`
4. See `src/CLAUDE.md` for detailed guide

### Adding a new frontend page

1. Create `frontend/src/app/<page>/page.tsx`
2. Add nav link in `frontend/src/components/app-header.tsx`
3. Create hook in `frontend/src/hooks/use-<name>.ts`
4. See `frontend/CLAUDE.md` for detailed guide

### Adding a database migration

1. Create `supabase/migrations/NNN_<description>.sql`
2. Add Rust types/queries in `src/db/*.rs`
3. See `supabase/CLAUDE.md` for schema conventions

## Pull Requests

- Keep PRs focused — one feature or fix per PR
- All CI checks must pass (fmt, clippy, test, frontend build)
- Fill out the PR template
- Reference related issues with `Fixes #N` or `Relates to #N`

## Reporting Issues

- **Bugs**: Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.md)
- **Features**: Use the [feature request template](.github/ISSUE_TEMPLATE/feature_request.md)
- **Security**: Email security concerns directly (do not open public issues)

## Documentation

- [`CLAUDE.md`](CLAUDE.md) — Architecture overview and agent coding guide
- [`ROADMAP.md`](ROADMAP.md) — Roadmap index (14 domain roadmaps)
- [`docs/roadmaps/`](docs/roadmaps/) — Detailed domain roadmaps
- [`.claude/commenting-standards.md`](.claude/commenting-standards.md) — Academic commenting standard
