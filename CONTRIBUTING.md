# Contributing to darkreach

## Prerequisites

- **Rust** (stable toolchain) with `rustfmt` and `clippy` components
- **GMP** library:
  - Linux: `sudo apt install build-essential libgmp-dev m4`
  - macOS: `brew install gmp`
- **Node.js 22+** and npm (for the frontend)
- **PostgreSQL** / Supabase (for database features)

## Setup

```bash
git clone https://github.com/<your-fork>/prime-hunter.git
cd prime-hunter

# Build the engine
cargo build

# Install frontend dependencies
cd frontend && npm install && cd ..

# Install the pre-commit hook
ln -sf ../../scripts/pre-commit .git/hooks/pre-commit
```

## Development commands

### Engine (Rust)

```bash
cargo test              # Run all tests
cargo fmt               # Format code
cargo fmt -- --check    # Check formatting without changes
cargo clippy            # Run linter
cargo build --release   # Optimized build
```

### Frontend (Next.js)

```bash
cd frontend
npm run dev             # Dev server at localhost:3000
npm run build           # Production build
npm run lint            # ESLint
npx tsc --noEmit        # Type check
```

## Pre-commit hook

The hook at `scripts/pre-commit` runs automatically on every commit:

1. `cargo fmt -- --check` — formatting must be clean
2. `cargo test --quiet` — all tests must pass
3. `npx tsc --noEmit` — type check (only when frontend files are staged)

Install it with:

```bash
ln -sf ../../scripts/pre-commit .git/hooks/pre-commit
```

## Code style

- Rust: `rustfmt.toml` enforces 100-char line width, 2021 edition
- TypeScript/JS: 2-space indent, Prettier defaults
- See `.editorconfig` for per-filetype settings

## Adding a new prime form

See [`docs/new-forms.md`](docs/new-forms.md) for the step-by-step guide to adding a new search module.

## Pull requests

- Keep PRs focused — one feature or fix per PR
- All CI checks must pass (`cargo fmt`, `cargo test`, frontend build)
- Fill out the PR template
- Reference related issues with `Fixes #N` or `Relates to #N`

## Project documentation

- [`CLAUDE.md`](CLAUDE.md) — Architecture overview and design decisions
- [`docs/`](docs/) — Research notes and per-module documentation
- [`docs/roadmaps/`](docs/roadmaps/) — Domain roadmaps (engine, server, frontend, ops, research)
