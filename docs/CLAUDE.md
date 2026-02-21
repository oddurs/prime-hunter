# docs/ — Research Domain

Research documentation, strategic analysis, and project roadmaps for darkreach.

## Structure

```
docs/
├── factorial-primes.md           # n! ± 1: definition, known primes, records, testing
├── palindromic-primes.md         # Palindromic: even-digit proof, distribution, records
├── kbn-primes.md                 # k·b^n ± 1: Proth/LLR/Mersenne theory, records
├── landscape.md                  # Distributed compute landscape (BOINC, GIMPS, PrimeGrid)
├── references.md                 # OEIS, PrimePages, GIMPS, PrimeGrid, software links
│
└── roadmaps/                     # Strategic roadmaps (14 files)
    ├── [Core Domain Roadmaps]
    ├── engine.md                 # Algorithm improvements: sieving, tests, new forms (18 KB)
    ├── server.md                 # Backend: pipeline, checkpoints, coordination (7 KB)
    ├── frontend.md               # Dashboard: filtering, visualization, search mgmt (7 KB)
    ├── ops.md                    # Deployment: optimization, GPU, PGO, automation (9 KB)
    ├── research.md               # Discovery strategy, publication, references (17 KB)
    │
    ├── [Infrastructure Roadmaps]
    ├── agents.md                 # AI agent infrastructure (20 KB)
    ├── fleet.md                  # Cluster coordination, distributed workers (15 KB)
    ├── projects.md               # Project lifecycle, cost tracking (17 KB)
    ├── testing.md                # Testing strategy, benchmarks (16 KB)
    │
    ├── [Integration Roadmaps]
    ├── gwnum-flint.md            # GWNUM/FLINT external library integration (14 KB)
    ├── cluster.md                # Multi-node cluster management (3 KB)
    │
    ├── [Platform Roadmaps]
    ├── website.md                # Public website features (8 KB)
    ├── public-compute.md         # Volunteer compute platform (10 KB)
    │
    └── competitive-analysis.md   # Market analysis and landscape (27 KB)
```

## Conventions

- **Prime form docs** (`factorial-primes.md`, etc.): research summaries with definition, known primes/records, testing methods, open questions.
- **Roadmaps**: actionable plans organized by priority tier (Tier 1 = quick wins, higher = more effort).
- **Math notation**: LaTeX with `$...$` delimiters (rendered via KaTeX in frontend).
- **OEIS references**: `A002981` with link to `https://oeis.org/A002981`.
- **References**: consolidated in `docs/roadmaps/research.md`, grouped by category.

## How to Update Roadmaps

1. Each domain has its own roadmap in `docs/roadmaps/`
2. Items organized by impact tier with: current state, target, algorithm/pseudocode, rationale
3. Root `ROADMAP.md` is a slim index linking to domain roadmaps — update if adding a new domain
4. Mark completed items with checkmarks, add completion dates

## Strategic Context

Key targets for prime discovery (from `docs/roadmaps/research.md`):

| Target | Core-years/discovery | Provable? | Competition |
|--------|---------------------|-----------|-------------|
| Sierpinski/Riesel (non-base-2) | 1-10 | Yes | Low |
| Palindromic record | 100-1,000 | Yes (BLS) | 1 team |
| Factorial | ~2,300 | Yes | PrimeGrid |
| Wagstaff PRP | ~3,000 | No | None |

## Publication Pipeline

```
Discovery → Cross-verify (different software/hardware)
  → t5k.org submission → OEIS update
  → arXiv preprint → Journal paper
```

## How to Evaluate New Forms

1. Check competitive landscape (who else is searching?)
2. Estimate core-years per discovery
3. Can results be proven? (t5k.org requires proofs)
4. Does it fit darkreach architecture? (Rust + rug/GMP + rayon)
5. What existing code can be reused? (`kbn::test_prime` covers many forms)

## Agent Coding Guide

### Adding research for a new prime form

1. Create `docs/<form>-primes.md` with: definition, known records, testing methods, OEIS refs
2. Add strategic analysis to `docs/roadmaps/research.md` (ROI, competition, provability)
3. Update `docs/roadmaps/engine.md` if implementation is planned

### Updating a roadmap

1. Read current state in the roadmap file
2. Move completed items to a "Completed" section with dates
3. Add new items at appropriate tier level
4. Update `ROADMAP.md` index if adding new roadmap files
