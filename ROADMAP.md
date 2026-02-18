# primehunt Roadmap

Goal: Build the world's most advanced prime number finder. Make real discoveries.

Based on research into GIMPS, PrimeGrid, PFGW, LLR, mtsieve, and the current competitive landscape.

---

## Domain Map

| Domain | Scope | Roadmap |
|--------|-------|---------|
| **Engine** | Prime-hunting algorithms, sieving, primality testing | [docs/roadmaps/engine.md](docs/roadmaps/engine.md) |
| **Server** | Dashboard API, WebSocket, fleet coordination, DB, checkpoints | [docs/roadmaps/server.md](docs/roadmaps/server.md) |
| **Frontend** | Next.js dashboard, charts, real-time monitoring | [docs/roadmaps/frontend.md](docs/roadmaps/frontend.md) |
| **Ops** | Deployment, build optimization, Apple Silicon, fleet management | [docs/roadmaps/ops.md](docs/roadmaps/ops.md) |
| **Research** | Discovery strategy, publication pipeline, competitive analysis | [docs/roadmaps/research.md](docs/roadmaps/research.md) |
| **Agents** | AI agent execution, orchestration, tools, budgets, automation | [docs/roadmaps/agents.md](docs/roadmaps/agents.md) |
| **Projects** | Campaign management, orchestration, cost tracking, records | [docs/roadmaps/projects.md](docs/roadmaps/projects.md) |

---

## Deployment Phases

Hardware scaling is gated on software milestones. **The single most impactful investment is GWNUM integration — worth more than $10K/mo in hardware.** Scale software before hardware.

Details: [ops.md Phased Deployment](docs/roadmaps/ops.md#phased-deployment-plan), [server-setup.md](docs/server-setup.md)

### Phase 1: Foundation ($53/mo — 1x Hetzner AX42, 8 cores)

Ship software optimizations on a single machine. Run non-base-2 Sierpinski/Riesel searches immediately — these work today with current code and have the best ROI (1-10 core-years per discovery).

**Software milestones:**
- Increase sieve limits (10M+ primes)
- BSGS sieving for kbn (100-1000x speedup)
- Proth's theorem + LLR test (deterministic proofs for base-2)
- Wilson's theorem pre-filter, 2-round MR pre-screen
- GMP built-in factorial for precomputation
- **GWNUM integration** (or PRST/PFGW subprocess) — critical path

**Search targets:** Non-base-2 Sierpinski/Riesel conjectures (Conjectures 'R Us). Each discovery solves a conjecture.

Details: [engine.md Tier 1](docs/roadmaps/engine.md#tier-1-quick-wins-high-impact-low-effort), [engine.md Tier 2](docs/roadmaps/engine.md#tier-2-algorithmic-upgrades-high-impact-medium-effort)

### Phase 2: Scale Up (after GWNUM works — $215-430/mo, add 1-2x AX162-S)

GWNUM unlocks 50-100x speedup for large candidates. Now hardware investment pays off.

**Software milestones:**
- Gerbicz error checking for long PRP tests
- P-1 factoring pre-filter
- Near-repdigit palindrome search mode
- Multi-stage pipeline: sieve -> screen -> test -> prove
- Pocklington proof (n!+1), Morrison proof (n!-1)

**Search targets:**
- Wagstaff PRP gap (p=4M-15M) — no competition, GWNUM makes this feasible
- Palindromic prime record attempt (near-repdigit, BLS proofs)
- Continue Sierpinski/Riesel on more cores

Details: [engine.md GWNUM/FLINT](docs/roadmaps/engine.md#gwnumflint-integration-very-high-impact-high-effort), [server.md](docs/roadmaps/server.md)

### Phase 3: Full Fleet (after GWNUM + Gerbicz are solid — $550-760/mo, add AX102 + more AX162-S)

**Software milestones:**
- Checkpoint hardening (multiple generations, checksums)
- Distributed search coordination
- Result verification and proof generation
- FLINT integration for general arithmetic

**Search targets:**
- Factorial primes (frontier extension, auto-provable)
- Parallel Sierpinski/Riesel campaigns across workers
- Palindromic record (sustained multi-month campaign)

Details: [server.md](docs/roadmaps/server.md#distributed-search-coordination), [ops.md](docs/roadmaps/ops.md#phased-deployment-plan)

### Phase 4: Infrastructure & Frontend

GPU acceleration, full dashboard control plane.

- GPU-accelerated sieving
- Search management from the web
- Multi-node coordination UI
- Fleet deployment automation

Details: [ops.md](docs/roadmaps/ops.md#gpu-accelerated-sieving), [frontend.md](docs/roadmaps/frontend.md)

---

## New Prime Forms (Priority Order)

| Form | Effort | Notes |
|------|--------|-------|
| Primorial (p# +/- 1) | Trivial | Port of factorial.rs |
| Cullen (n*2^n + 1) | Trivial | Wrapper around kbn |
| Woodall (n*2^n - 1) | Moderate | Needs LLR |
| Wagstaff ((2^p+1)/3) | Moderate | No competition, unique niche |
| Carol/Kynea | Moderate | Shares LLR from Woodall |
| Twin primes | High | Needs Proth + LLR + quad sieve |
| Sophie Germain | High | Shares twin infrastructure |
| Repunit ((10^n-1)/9) | Moderate | No deterministic test |
| Generalized Fermat | High | GPU-preferred for frontier |

Details: [engine.md New Prime Forms](docs/roadmaps/engine.md#new-prime-forms)

---

## Strategic Targets

| Target | ROI (core-years) | Provable? | Competition | When |
|--------|------------------|-----------|-------------|------|
| Sierpinski/Riesel (non-base-2) | **1-10** | Yes | Low | **Phase 1 (now)** |
| Palindromic record | 100-1,000 | Yes (BLS) | 1 team | Phase 2 (needs GWNUM) |
| Wagstaff PRP | ~3,000 | No | None | Phase 2 (needs GWNUM) |
| Factorial | ~2,300 | Yes | PrimeGrid | Phase 3 (needs GWNUM + fleet) |

Details: [research.md](docs/roadmaps/research.md#strategic-targets-where-to-actually-find-primes)
