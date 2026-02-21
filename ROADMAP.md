# darkreach Roadmap

**Goal:** Build the world's most advanced prime number discovery platform. Make real discoveries.

---

## Domain Roadmaps

| Domain | Scope | Roadmap |
|--------|-------|---------|
| **Engine** | Prime-hunting algorithms, sieving, primality testing, proofs | [docs/roadmaps/engine.md](docs/roadmaps/engine.md) |
| **Server** | Dashboard API, WebSocket, fleet coordination, DB, checkpoints | [docs/roadmaps/server.md](docs/roadmaps/server.md) |
| **Frontend** | Next.js dashboard, charts, real-time monitoring | [docs/roadmaps/frontend.md](docs/roadmaps/frontend.md) |
| **Ops** | Deployment, build optimization, fleet management | [docs/roadmaps/ops.md](docs/roadmaps/ops.md) |
| **Research** | Discovery strategy, publication pipeline, competitive analysis | [docs/roadmaps/research.md](docs/roadmaps/research.md) |
| **Agents** | AI agent execution, orchestration, budgets, automation | [docs/roadmaps/agents.md](docs/roadmaps/agents.md) |
| **Projects** | Campaign management, orchestration, cost tracking, records | [docs/roadmaps/projects.md](docs/roadmaps/projects.md) |
| **Fleet** | Cluster coordination, distributed work claiming | [docs/roadmaps/fleet.md](docs/roadmaps/fleet.md) |
| **Testing** | Test strategy, integration tests, benchmarks, CI | [docs/roadmaps/testing.md](docs/roadmaps/testing.md) |
| **GWNUM/FLINT** | External library integration for 50-100x acceleration | [docs/roadmaps/gwnum-flint.md](docs/roadmaps/gwnum-flint.md) |
| **Public Compute** | Volunteer onboarding, release channels, validation | [docs/roadmaps/public-compute.md](docs/roadmaps/public-compute.md) |
| **Website** | Public-facing website at darkreach.ai | [docs/roadmaps/website.md](docs/roadmaps/website.md) |
| **Cluster** | Multi-node cluster management | [docs/roadmaps/cluster.md](docs/roadmaps/cluster.md) |
| **Competitive Analysis** | Market landscape and competitor research | [docs/roadmaps/competitive-analysis.md](docs/roadmaps/competitive-analysis.md) |

---

## Current Status

### Completed

- **12 search forms**: factorial, palindromic, kbn, near-repdigit, primorial, Cullen/Woodall, Wagstaff, Carol/Kynea, twin, Sophie Germain, repunit, generalized Fermat
- **Deterministic proofs**: Proth, LLR, Pocklington, Morrison, BLS, Pepin
- **Core primitives**: Montgomery multiplication, wheel factorization, BitSieve, Frobenius test, Pollard P-1
- **External tools**: PFGW subprocess, GWNUM FFI (feature-gated), PRST subprocess, FLINT (feature-gated)
- **3-tier verification**: deterministic → BPSW+MR → PFGW cross-check
- **Distributed fleet**: PostgreSQL-based work claiming with `FOR UPDATE SKIP LOCKED`
- **Dashboard**: 14 pages, 50+ components, real-time via Supabase + WebSocket
- **Project campaigns**: TOML-defined, multi-phase, with cost tracking and record comparison
- **AI agent infrastructure**: tasks, budgets, memory, roles, schedules
- **CI/CD**: 10 CI jobs, Docker builds, signed releases (x86_64 + aarch64)
- **449 unit tests passing**, integration tests, property tests, security tests, E2E tests

### In Progress

- Multi-stage sieving pipeline
- FLINT deep integration for accelerated factorial/primorial
- Public volunteer compute platform
- GPU-accelerated sieving

---

## Deployment Phases

Hardware scaling is gated on software milestones. **GWNUM integration is worth more than $10K/mo in hardware.** Scale software before hardware.

### Phase 1: Foundation ($53/mo — 1x Hetzner AX42, 8 cores)

Ship software optimizations on a single machine. Run non-base-2 Sierpinski/Riesel searches — best ROI (1-10 core-years per discovery).

### Phase 2: Scale Up ($215-430/mo — add 1-2x AX162-S)

GWNUM unlocks 50-100x speedup. Hardware investment pays off. Target Wagstaff PRP gap and palindromic record.

### Phase 3: Full Fleet ($550-760/mo — add AX102 + more AX162-S)

Distributed campaigns across multiple workers. Factorial frontier extension, sustained palindromic record attempt.

### Phase 4: Infrastructure & Public Compute

GPU-accelerated sieving, volunteer compute platform, full dashboard control plane.

---

## Strategic Targets

| Target | ROI (core-years) | Provable? | Competition | Phase |
|--------|------------------|-----------|-------------|-------|
| Sierpinski/Riesel (non-base-2) | **1-10** | Yes | Low | **Phase 1** |
| Palindromic record | 100-1,000 | Yes (BLS) | 1 team | Phase 2 |
| Wagstaff PRP | ~3,000 | No | None | Phase 2 |
| Factorial | ~2,300 | Yes | PrimeGrid | Phase 3 |

See [docs/roadmaps/research.md](docs/roadmaps/research.md) for full analysis.

---

## Public Compute Track

Competing with GIMPS/PrimeGrid on contributor experience requires a dedicated release and trust pipeline:

- Public worker packaging + auto-update channels
- Quorum validation + host reputation for untrusted compute
- Release canary/ramp/rollback controls
- Volunteer engagement layer (credits, teams, challenges)

See [docs/roadmaps/public-compute.md](docs/roadmaps/public-compute.md).
