<p align="center">
  <img src="frontend/public/favicon.svg" alt="darkreach" width="80" />
</p>

<h1 align="center">darkreach</h1>

<p align="center">
  <strong>Distributed prime number discovery platform</strong><br>
  12 search forms &middot; deterministic proofs &middot; fleet coordination &middot; real-time dashboard
</p>

<p align="center">
  <a href="https://github.com/oddurs/prime-hunter/actions/workflows/ci.yml"><img src="https://github.com/oddurs/prime-hunter/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/oddurs/prime-hunter/releases"><img src="https://img.shields.io/github/v/release/oddurs/prime-hunter?include_prereleases&label=release" alt="Release"></a>
  <a href="https://github.com/oddurs/prime-hunter/blob/master/LICENSE"><img src="https://img.shields.io/github/license/oddurs/prime-hunter" alt="License"></a>
  <a href="https://darkreach.ai"><img src="https://img.shields.io/badge/web-darkreach.ai-blue" alt="Website"></a>
</p>

---

darkreach hunts special-form prime numbers at scale. It combines GMP-powered arbitrary-precision arithmetic with Rayon parallelism and a distributed worker fleet to search for primes across 12 different mathematical forms — from factorial primes to generalized Fermats — with deterministic proof generation and a real-time monitoring dashboard.

## Prime Forms

| Form | Expression | Proof Type | Search Command |
|------|-----------|------------|----------------|
| **Factorial** | n! ± 1 | Pocklington / Morrison | `darkreach factorial` |
| **Palindromic** | Reads same forwards & backwards | PFGW PRP | `darkreach palindromic` |
| **Proth / Riesel** | k·b^n ± 1 | Proth / LLR (deterministic) | `darkreach kbn` |
| **Near-repdigit** | Palindromic near-repdigits | BLS N+1 | `darkreach near-repdigit` |
| **Primorial** | p# ± 1 | Pocklington / Morrison | `darkreach primorial` |
| **Cullen / Woodall** | n·2^n ± 1 | Proth / LLR | `darkreach cullen-woodall` |
| **Wagstaff** | (2^p + 1) / 3 | PRP (no proof exists) | `darkreach wagstaff` |
| **Carol / Kynea** | (2^n ± 1)² − 2 | LLR | `darkreach carol-kynea` |
| **Twin** | p and p + 2 | Proth + LLR intersection | `darkreach twin` |
| **Sophie Germain** | p and 2p + 1 | Proth + LLR intersection | `darkreach sophie-germain` |
| **Repunit** | (b^n − 1) / (b − 1) | PFGW PRP | `darkreach repunit` |
| **Generalized Fermat** | b^(2^n) + 1 | Pepin / Proth | `darkreach gen-fermat` |

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    darkreach platform                    │
├──────────────┬──────────────┬──────────────┬────────────┤
│   Engine     │   Server     │  Frontend    │   Fleet    │
│              │              │              │            │
│ 12 search    │ Axum REST    │ Next.js 16   │ N workers  │
│ forms        │ API + WS     │ React 19     │ heartbeat  │
│ GMP/rug      │ PostgreSQL   │ Tailwind 4   │ block      │
│ rayon        │ (Supabase)   │ shadcn/ui    │ claiming   │
│ PFGW/GWNUM   │ 15 route     │ Recharts     │ auto-      │
│ proofs       │ modules      │ Realtime     │ restart    │
└──────────────┴──────────────┴──────────────┴────────────┘
```

**Engine** — Rust algorithms: sieve → parallel primality test → deterministic proof → log. Montgomery multiplication, wheel factorization, BSGS sieving, Pollard P−1 filtering. External tool acceleration via PFGW (50-100x speedup), GWNUM FFI, PRST, and FLINT.

**Server** — Axum HTTP/WebSocket server with 15 route modules. PostgreSQL (Supabase) for prime records, fleet coordination, agent tasks, and project management. `FOR UPDATE SKIP LOCKED` work block claiming.

**Frontend** — Next.js static export with 14 pages, 50+ components, and 18 custom hooks. Supabase for data + auth + realtime notifications. WebSocket for fleet coordination.

**Fleet** — Distributed workers register via heartbeat, claim work blocks from PostgreSQL, and report results. Auto-restart on failure, stale worker pruning, release channel management.

## Quick Start

### Prerequisites

**Linux:**
```bash
sudo apt install build-essential libgmp-dev m4
```

**macOS:**
```bash
brew install gmp
```

### Build & Run

```bash
# Build
cargo build --release

# Search for factorial primes
darkreach factorial --start 1000 --end 50000

# Search for Proth primes (k·2^n + 1)
darkreach kbn --k 3 --base 2 --min-n 100000 --max-n 500000

# Search for palindromic primes
darkreach palindromic --base 10 --min-digits 50 --max-digits 200

# Search for twin primes
darkreach twin --k 3 --base 2 --min-n 1 --max-n 1000

# Run all 12 forms — see `darkreach --help` for all subcommands
```

### With Database

```bash
# Set database connection (Supabase PostgreSQL)
export DATABASE_URL=postgres://...

# Run with result logging
darkreach --database-url $DATABASE_URL factorial --start 1 --end 10000
```

### Dashboard

```bash
# Build frontend
cd frontend && npm install && npm run build && cd ..

# Start coordinator with dashboard
darkreach --database-url $DATABASE_URL dashboard --port 7001 --static-dir frontend/out
```

### Local Dev Stack

```bash
./scripts/dev.sh                    # Backend (:7001) + frontend (:3001)
./scripts/dev.sh --remote <url>     # Local UI proxying to remote API
```

## Features

- **12 search forms** — factorial, palindromic, kbn, primorial, Cullen/Woodall, Wagstaff, Carol/Kynea, twin, Sophie Germain, repunit, generalized Fermat, near-repdigit
- **Deterministic proofs** — Proth, LLR, Pocklington, Morrison, BLS, Pepin — not just probable primes
- **PFGW/GWNUM acceleration** — 50-100x speedup for large candidates via external tool integration
- **Distributed fleet** — coordinator + N workers with PostgreSQL-based work block claiming
- **Resumable** — checkpoint every 60s with atomic writes; restart picks up where you left off
- **Parallel** — full CPU utilization via Rayon (par_iter over blocks/batches)
- **Real-time dashboard** — live discovery notifications, fleet monitoring, search management
- **3-tier verification** — deterministic proof → BPSW + Miller-Rabin → PFGW cross-check
- **Project campaigns** — multi-phase discovery campaigns with cost tracking and record comparison
- **AI agent infrastructure** — autonomous search management with budgets and scheduling
- **Primality certificates** — machine-verifiable proof artifacts for each discovery
- **Volunteer compute** — public worker registration with release channels and auto-update

## Testing

```bash
# Unit tests (449 passing)
cargo test

# Integration tests (requires PostgreSQL)
TEST_DATABASE_URL=postgres://... cargo test --test db_integration
TEST_DATABASE_URL=postgres://... cargo test --test api_integration

# Benchmarks
cargo bench

# Frontend
cd frontend && npm test              # Vitest
cd frontend && npm run test:e2e      # Playwright
```

## Project Structure

```
src/                    Rust engine + server (79 modules)
├── 12 search forms     factorial, palindromic, kbn, twin, ...
├── core primitives     sieve, proof, verify, certificate, p1
├── external tools      pfgw, gwnum, prst, flint
├── dashboard/          Axum web server (15 route modules)
├── db/                 PostgreSQL layer (14 submodules)
└── project/            Campaign management (6 submodules)

frontend/               Next.js 16 dashboard
├── src/app/            14 pages
├── src/components/     50+ React components
├── src/hooks/          18 custom hooks
└── src/lib/            Supabase client, utilities

deploy/                 Ops infrastructure
├── systemd units       Coordinator + worker templates
├── helm/               Kubernetes Helm chart
├── terraform/          Infrastructure-as-Code
└── grafana/            Monitoring dashboard

supabase/               Database schema
└── migrations/         24 PostgreSQL migrations

tests/                  Integration tests + benchmarks
docs/                   Research + 14 domain roadmaps
```

## CI / CD

| Job | What it checks |
|-----|---------------|
| **Format** | `cargo fmt` |
| **Clippy** | `cargo clippy` |
| **Test** | `cargo test --lib` |
| **Test (FLINT)** | `cargo test --lib --features flint` |
| **Bench Compile** | `cargo bench --no-run` |
| **Integration** | DB + API + CLI + security tests (PostgreSQL service) |
| **Frontend** | TypeScript + lint + Vitest + build |
| **Frontend E2E** | Playwright (Chromium) |
| **Docker** | Build + push to GHCR (master only) |
| **Release** | Linux x86_64 + aarch64 binaries, signed, GitHub Release |

## Deployment

```bash
# SSH deploy
./deploy/deploy.sh <host>

# Full production setup
./deploy/production-deploy.sh <host>

# Kubernetes
helm install darkreach deploy/helm/darkreach/ -f deploy/helm/darkreach/values-production.yaml

# PGO optimized build (5-15% speedup)
./deploy/pgo-build.sh
```

## Discovery Strategy

| Target | Core-years/discovery | Provable? | Competition |
|--------|---------------------|-----------|-------------|
| Sierpinski/Riesel (non-base-2) | **1-10** | Yes | Low |
| Palindromic record | 100-1,000 | Yes (BLS) | 1 team |
| Factorial frontier | ~2,300 | Yes | PrimeGrid |
| Wagstaff gap | ~3,000 | No (PRP) | None |

See [docs/roadmaps/research.md](docs/roadmaps/research.md) for full strategic analysis.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup instructions, code style, and PR guidelines.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the full roadmap index linking to 14 domain-specific roadmaps covering engine algorithms, server infrastructure, frontend features, deployment optimization, research strategy, and more.

## License

[MIT](LICENSE)
