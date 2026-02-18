# docs/ — Research Domain

Documentation on prime forms, competitive landscape, and project roadmaps.

## Structure

```
docs/
├── factorial-primes.md      # n! +/- 1: definition, known primes, records, testing strategies
├── palindromic-primes.md    # Palindromic primes: even-digit proof, distribution, records
├── kbn-primes.md            # k*b^n +/- 1: Proth/LLR/Mersenne theory, records, software
├── references.md            # OEIS, PrimePages, GIMPS, PrimeGrid, software links
└── roadmaps/
    ├── engine.md            # Algorithm improvements (sieving, primality tests, new forms)
    ├── server.md            # Backend infrastructure (pipeline, checkpoints, coordination)
    ├── frontend.md          # Dashboard features (filtering, visualization, search management)
    ├── ops.md               # Deployment & hardware optimization
    └── research.md          # Discovery strategy, publication pipeline, references
```

## Conventions

- **Prime form docs** (`factorial-primes.md`, etc.) are research summaries covering: definition, known primes/records, testing methods, and open questions.
- **Roadmap docs** contain actionable improvement plans organized by priority tier.
- Mathematical expressions use LaTeX notation with `$...$` delimiters (rendered via KaTeX in the frontend).
- OEIS sequence references use format: `A002981` with link to `https://oeis.org/A002981`.

## How to Update Roadmaps

1. Each domain has its own roadmap in `docs/roadmaps/`.
2. Items are organized by impact tier (Tier 1 = quick wins, higher tiers = more effort).
3. Include: current state, target, algorithm/pseudocode where relevant, rationale.
4. The root `ROADMAP.md` is a slim index linking to domain roadmaps — update it if adding a new domain.

## References Format

References go in `docs/roadmaps/research.md` (consolidated). Format:
```
- [Title - Source](URL)
```

Group by category: math resources, software projects, academic papers, community links.

## Strategic Context

Key targets for prime discovery are documented in `docs/roadmaps/research.md`:
- **Best ROI:** Non-base-2 Sierpinski/Riesel conjectures (1-10 core-years per discovery)
- **Highest visibility:** Palindromic prime record (provable, 1 competitor)
- **No competition:** Wagstaff primes (PRP only, no active project)
- **Auto-provable:** Factorial primes (Pocklington/Morrison proofs are free)
