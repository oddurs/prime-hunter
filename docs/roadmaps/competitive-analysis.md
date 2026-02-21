# Competitive Analysis & Catch-Up Roadmap

Deep research on GIMPS, PrimeGrid, and the broader prime-hunting ecosystem.
Goal: identify the technical gaps between darkreach and the state of the art,
then chart a realistic path to close them.

---

## 1. The Landscape (Feb 2026)

### GIMPS — The Gold Standard

| Metric | Value |
|--------|-------|
| Active users | ~1,200 |
| Active CPUs/GPUs | 23,000+ |
| Sustained compute | ~7 PFLOPS |
| Search frontier | All exponents below 139.6M tested |
| Verified frontier | All exponents below 80M double-checked |
| Key software | Prime95 (GWNUM), GpuOwl/PRPLL, Mlucas, mfaktc |
| Years running | 30 (since 1996) |

**Key innovations (in order of adoption):**

1. **IBDWT** (Crandall, 1994) — Irrational Base Discrete Weighted Transform eliminates
   zero-padding in FFT-based squaring for Mersenne numbers, halving FFT size.
2. **PrimeNet** (Kurowski, 1997) — automated work assignment, result collection,
   reliability tracking, assignment reclamation (tiered 30-360 day deadlines).
3. **Gerbicz error checking** (2017) — ~0.1% overhead, catches hardware errors with
   99.999%+ reliability. Stores intermediate residues every L iterations, verifies
   `d(t+1) == u(0) * d(t)^(2^L) mod N` every L^2 steps.
4. **PRP testing** (2018) — replaced LL as default. Supports Gerbicz + proofs.
5. **Pietrzak PRP proofs** (2020) — cryptographic proof that PRP test ran correctly,
   verifiable at ~0.5% of original cost. Eliminated most double-checks, nearly
   doubling throughput.
6. **GPU revolution** (2024) — Luke Durant spent ~$2M on cloud GPUs across 24 datacenter
   regions in 17 countries. Found M52 (41M digits). First GIMPS discovery via PRP and
   first via GPU. Single A100 delivers 800-1000 GHz-days/day vs 10-30 for a fast CPU.

**Multi-stage factoring pipeline:**

```
Stage 1: Trial Factoring (TF)     → up to 2^80, eliminates ~60% of candidates
Stage 2: P-1 Factoring            → 2^80 to 2^120, uses Dickman function for probability
Stage 3: ECM (small exponents)    → larger factors, only for exponents < 1M
Stage 4: PRP test + Gerbicz + proof → full test on survivors only
Stage 5: Cert verification        → 0.5% of PRP cost, cryptographic certainty
Stage 6: LL confirmation          → only if PRP indicates probable prime
```

### PrimeGrid — Multi-Form Volunteer Computing

| Metric | Value |
|--------|-------|
| Active users | 3,126 |
| Active computers | 12,656 |
| Registered users | 357,258 |
| Sustained compute | 4,173 TFLOPS |
| Infrastructure | BOINC (feeder → scheduler → validator → assimilator) |
| Key software | LLR2/PRST, PFGW, GeneferOCL, mtsieve sieves |
| Years running | ~20 |

**Active subprojects (as of Feb 2026):**

| Subproject | Form | Software | Status |
|-----------|------|----------|--------|
| 321 Prime Search | 3×2^n ± 1 | LLR2 | Active |
| Cullen Prime Search | n×2^n + 1 | LLR2 | Active |
| Woodall Prime Search | n×2^n - 1 | LLR2 | Active |
| Gen. Cullen/Woodall | n×b^n ± 1 | LLR2 | Active |
| Extended Sierpinski Problem | k×2^n + 1 | LLR2 | Active |
| Seventeen or Bust | k×2^n + 1 (6 remaining k) | LLR2 | Active |
| The Riesel Problem | k×2^n - 1 (41 remaining k) | LLR2 | Active |
| Sierpinski/Riesel Base 5 | k×5^n ± 1 | LLR2 | Active |
| Proth Prime Search (PPS) | k×2^n + 1 | LLR2 | Active |
| Proth Prime Search Extended | k×2^n + 1 (large n) | LLR2 | Active |
| Generalized Fermat (GFN 15-22) | b^(2^n) + 1 | GeneferOCL (GPU) | Active |
| GFN-23 (Mega) | b^(2^23) + 1 | GeneferOCL | Active (new) |
| Factorial Prime Search | n! ± 1 | PRST | Rebooted Sep 2025 |
| Primorial Prime Search | p# ± 1 | PRST | Active |
| Sophie Germain Search | k×2^n - 1 pairs | LLR2 | **Suspended 2024** |
| AP27 Search | 27-prime arithmetic progressions | Custom | Completed (2019) |

**Key architecture:**

- **BOINC server pipeline**: feeder → shared-memory work cache → scheduler → dispatcher →
  validator (wingman quorum of 2) → assimilator → file deleter / DB purger.
- **Sieve-then-test**: mtsieve programs sieve to "optimal depth" (crossover where
  sieve_cost > primality_test_cost × P(factor)), then BOINC distributes primality tests.
- **GeneferOCL**: GPU-accelerated Pépin test for Generalized Fermat numbers. Found GFN-21
  prime (13.4M digits, Oct 2025) — largest PrimeGrid prime ever.
- **PRST**: Pavel Atnashev's successor to LLR2. Uses GWNUM. Supports k×b^n±1, n!±c,
  n#±c, multifactorials, Gerbicz checking, and PRP proof generation/verification.

**PrimeGrid's recent major discoveries:**

| Date | Prime | Digits | Form |
|------|-------|--------|------|
| Oct 2025 | GFN-21 mega prime | 13,426,224 | b^(2^21) + 1 |
| Apr 2025 | 4052186×69^4052186 + 1 | 7,451,366 | Gen. Cullen |
| Dec 2025 | 751882!/751879# + 1 | 3,765,621 | Compositorial |
| Aug 2024 | 107347×2^23427517 - 1 | 7,052,391 | Riesel (elimination) |

### mtsieve — The Sieving Framework

Mark Rodenkirch's mtsieve provides 20+ specialized sieve programs:

| Program | Form | GPU? |
|---------|------|------|
| srsieve2/srsieve2cl | k×b^n ± c (Sierpinski/Riesel) | Yes (OpenCL) |
| gcwsieve/gcwsievecl | n×b^n ± 1 (Cullen/Woodall) | Yes |
| gfndsieve/gfndsievecl | b^(2^n) + 1 (Gen. Fermat) | Yes |
| cksieve/cksievecl | (b^n±1)²−2 (Carol/Kynea) | Yes |
| psieve/psievecl | p# ± 1 (Primorial) | Yes |
| mfsieve/mfsievecl | Multifactorial | Yes |
| twinsieve | k×b^n ± 1 (twin pairs) | No |
| sgsieve | Sophie Germain | No |
| fbncsieve | k×b^n + c (general) | No |

All use BSGS (Baby-Step Giant-Step) discrete logarithm for O(√p) per-prime sieving.
GPU variants use OpenCL. Framework handles threading, I/O, factor reporting.

### Independent Searchers

- **Ryan Propper & Serge Batalov** — Found largest palindromic prime (2,718,281 digits,
  Aug 2024) and multiple Riesel eliminations. Use custom infrastructure.
- **Luke Durant** — M52 via cloud GPUs (~$2M). Demonstrated that capital + GPU infrastructure
  can outperform decades of volunteer CPU time.
- **Conjectures 'R Us** — Community tracking hundreds of open Sierpinski/Riesel bases.
  Many bases have only 1-3 remaining k-values with low search limits.

---

## 2. The Gap: darkreach vs State of the Art

### What darkreach has

- 12 prime forms with sieve + parallel test + proof + PostgreSQL logging
- Proth, LLR, Pocklington, Morrison, BLS deterministic proofs
- PFGW subprocess for 50-100x acceleration (all forms)
- PRST subprocess for k×b^n±1 with Gerbicz checking
- GWNUM FFI wrapper (Vrba-Reix for Wagstaff, Proth/LLR with Gerbicz)
- BSGS sieve, Montgomery multiplication, wheel factorization, P-1 factoring
- Cluster coordination (heartbeat, block-based work claiming, FOR UPDATE SKIP LOCKED)
- Web dashboard, WebSocket updates, search job management
- AI agent system for autonomous search management
- Primality certificates (JSONB) and 3-tier verification pipeline
- ~443 tests passing

### What darkreach is missing (ordered by impact)

| Gap | GIMPS/PrimeGrid Has | darkreach Status | Impact |
|-----|---------------------|-----------------|--------|
| **PRP proof generation** | Pietrzak VDF proofs, verifiable at 0.5% cost | None | Critical — eliminates double-checks |
| **GPU primality testing** | GpuOwl/PRPLL (30-100x/device), GeneferOCL | CPU only | Critical — orders of magnitude more throughput |
| **GPU sieving** | srsieve2cl, gcwsievecl, gfndsievecl, cksievecl | CPU only | High — enables deeper sieve depths |
| **Native GWNUM FFT** | Prime95/PRST use GWNUM directly, no subprocess overhead | Subprocess wrapper only | High — subprocess has I/O overhead, no mid-computation checkpointing |
| **Auto-tuning** | Prime95 benchmarks FFT sizes at startup, stores optimal choices | Fixed parameters | Medium — up to 10% performance difference |
| **Formal verification pipeline** | GIMPS: PRP proof cert. PrimeGrid: wingman quorum of 2 | 3-tier (deterministic/BPSW+MR/PFGW) but no proof generation | High — t5k.org requires proofs |
| **Multi-stage factoring** | TF → P-1 → ECM → PRP (each stage cost-optimized) | Sieve → P-1 → PRP (no TF stage, no ECM) | Medium |
| **Volunteer computing** | BOINC/PrimeNet with 3K-23K active machines | Private cluster only | Low priority (different model) |
| **Credit/attribution** | GIMPS GHz-days, PrimeGrid cobblestones/badges | None | Low (matters for volunteers) |
| **Challenge events** | PrimeGrid runs 9 challenges/year driving engagement | None | Low (community feature) |
| **Sieve depth auto-tuning** | Optimal crossover calculation, stops when sieve_cost > test_cost/p | Fixed sieve limits | Medium |
| **ECM factoring** | GMP-ECM, ecm crate | Not integrated | Low-Medium |
| **NTT multiplication** | PRPLL-NTT (avoids floating-point precision issues) | None (GWNUM uses FP-FFT) | Low (only matters at extreme sizes) |

---

## 3. Catch-Up Roadmap

### Phase 1: Proof & Verification (highest impact, enables publication)

**Goal:** Generate verifiable primality proofs accepted by t5k.org.

#### 1.1 PRST proof pipeline integration

Currently darkreach shells out to PRST for k×b^n±1 forms. Extend this to:

- Pass `-proof save` flag to PRST invocations
- Collect proof files from PRST output directory
- Store proof file paths/hashes in `primes.certificate` JSONB
- Add `prst -proof build` step to construct verifiable certificate
- Add `prst -proof cert` step for independent verification
- Expose proof verification via `/api/primes/{id}/verify-proof` endpoint

**Applies to:** kbn, twin, sophie_germain, cullen_woodall, carol_kynea, gen_fermat
(all forms that route through PRST)

**Effort:** ~2-3 days. No new math — PRST already implements it.

#### 1.2 PFGW proof collection for factorial/primorial

PFGW already generates proof certificates with `-tp` (Pocklington N-1) and `-tm`
(Morrison N+1) flags. Currently darkreach captures the PRP result but discards the
proof output.

- Parse PFGW proof output (witness values, factor chains)
- Store structured proof data in certificate JSONB
- Cross-reference with our native Pocklington/Morrison proof output

**Applies to:** factorial, primorial, near_repdigit

**Effort:** ~1-2 days.

#### 1.3 Pietrzak PRP proof generation (long-term)

For forms where neither PRST nor PFGW generates proofs, implement Pietrzak's VDF
proof scheme directly in Rust:

- During PRP test, save intermediate residues at 2^power intervals
- After test completes, construct recursive halving proof
- Proof file verifiable at ~0.5% of original test cost
- Implement `proof verify` subcommand

This is the technique that transformed GIMPS. Non-trivial to implement from scratch
but extremely high value.

**Effort:** ~2-4 weeks for a correct implementation. Could use PRST as reference.

#### 1.4 t5k.org submission automation

- Create prover account and proof code
- Implement `darkreach submit` CLI command
- Auto-format expression for t5k.org (under 255 chars)
- Generate submission package: expression + proof + verification log
- Post to OEIS if applicable

**Effort:** ~1 day (mostly process, not code).

---

### Phase 2: GPU Acceleration (biggest throughput multiplier)

**Goal:** 30-100x throughput increase for primality testing; 10-50x for sieving.

#### 2.1 GPU primality testing via GpuOwl/PRPLL integration

Rather than writing GPU kernels from scratch, integrate with existing GPU programs:

- **GpuOwl/PRPLL** for Mersenne-form numbers (if we ever target Mersenne)
- **GeneferOCL** for generalized Fermat b^(2^n)+1 — already works, just needs
  subprocess integration similar to PFGW
- **PRST** already uses GWNUM which supports AVX-512 multi-threaded FFT on CPU

For non-Mersenne forms, the practical GPU path is:

1. GeneferOCL subprocess for gen_fermat (immediate, ~1 day)
2. cudarc-based GPU modular exponentiation kernel for PRP tests (weeks)
3. Metal compute shaders for Apple Silicon (long-term, limited 64-bit int perf)

**Note:** Apple Silicon GPUs have 8-cycle 64-bit multiply and 8KB L1 — not viable
for prime hunting. GPU acceleration means NVIDIA/AMD discrete GPUs on Linux servers.

#### 2.2 GPU sieving via mtsieve OpenCL

mtsieve already has OpenCL sieve programs for most of our forms:

| darkreach form | mtsieve sieve | GPU? |
|---------------|---------------|------|
| kbn | srsieve2cl | Yes |
| cullen_woodall | gcwsievecl | Yes |
| gen_fermat | gfndsievecl | Yes |
| carol_kynea | cksievecl | Yes |
| primorial | psievecl | Yes |
| factorial | mfsievecl | Yes |
| twin | twinsieve | No (CPU only) |
| sophie_germain | sgsieve | No (CPU only) |

Integration approach:

1. Add mtsieve subprocess integration (similar to PFGW/PRST pattern)
2. Generate sieve input files from darkreach search parameters
3. Run GPU sieve to optimal depth
4. Parse survivor list → feed to primality testing pipeline
5. Auto-detect GPU availability, fall back to CPU sieve

**Effort:** ~1 week per form for subprocess integration. No custom GPU code needed.

#### 2.3 Native GPU kernels (long-term)

For maximum performance, implement GPU-native modular arithmetic:

- Barrett reduction kernels (like mfaktc's 73-bit Barrett for trial factoring)
- NTT-based large integer multiplication (like PRPLL-NTT)
- Use `cudarc` crate for CUDA from Rust, or `wgpu` for cross-platform compute

**Effort:** Months. Only worthwhile after Phase 1-2 subprocess integrations are working.

---

### Phase 3: Deep Sieving Pipeline (reduce wasted primality tests)

**Goal:** Eliminate more composites before expensive PRP tests.

#### 3.1 Multi-stage sieve pipeline

Implement GIMPS-style ordered elimination:

```
Stage 1: Algebraic filters          → Free (e.g., Wilson's theorem, even-digit skip)
Stage 2: Small prime trial division → p < 10^6, O(1) per prime per candidate
Stage 3: BSGS sieve (existing)     → p < 10^9-10^12, O(√range) per prime
Stage 4: GPU sieve (Phase 2.2)     → p < 10^15, massively parallel
Stage 5: P-1 factoring (existing)  → finds smooth factors up to ~2^80-2^120
Stage 6: ECM factoring (new)       → finds larger factors for high-value candidates
Stage 7: PRP test                  → full test on survivors only
```

#### 3.2 ECM integration

The `ecm` Rust crate (v1.0.1) provides Lenstra ECM with rug backend:

```rust
use ecm::ecm;
let factor = ecm(&candidate, /* curves */ 100, /* B1 */ 1_000_000);
```

Apply selectively to high-value candidates (e.g., close to records) where the cost
of ECM is justified by the value of eliminating a composite before a multi-day PRP test.

**Effort:** ~2 days integration + tuning.

#### 3.3 Sieve depth auto-tuning

Implement the crossover heuristic used by GIMPS and PrimeGrid:

```
Continue sieving while:
  time_to_remove_one_candidate_via_sieve < primality_test_time × P(factor_in_next_range)

P(factor in range [p, 2p]) ≈ 1/ln(p)  (Mertens' theorem)
```

Benchmark sieve rate and primality test time at startup, compute optimal depth
dynamically. Store calibration in `cost_calibrations` table.

**Effort:** ~1-2 days.

---

### Phase 4: Native GWNUM Integration (eliminate subprocess overhead)

**Goal:** Use GWNUM as a library, not via subprocess.

#### 4.1 Complete GWNUM FFI

The `gwnum-sys` crate exists but is x86-64 only and feature-gated. Extend to:

- Full Proth/LLR/Pépin test APIs (not just Vrba-Reix)
- PRP test with Gerbicz checking via GWNUM
- Proof generation during GWNUM-based PRP tests
- Multi-threaded FFT (GWNUM's two-pass architecture)
- Sin/cos table sharing for multiple concurrent tests
- Error monitoring (roundoff checking)

This eliminates PRST/PFGW subprocess overhead and enables mid-computation checkpointing,
which is critical for tests taking days or weeks.

#### 4.2 FFT auto-benchmarking

Like Prime95's startup benchmarking:

1. At startup, benchmark all FFT sizes needed for current search range
2. Test all available instruction set variants (SSE2, AVX, AVX-512)
3. Store results in local cache file
4. Select optimal FFT implementation per size
5. Re-benchmark periodically (every 21 hours, like Prime95)

**Effort:** ~1-2 weeks for full GWNUM FFI. ~2 days for benchmarking.

---

### Phase 5: Distribution & Scale (grow beyond single cluster)

**Goal:** Support untrusted volunteers and larger fleets.

#### 5.1 PRP proof-based verification (enables trustless distribution)

With Phase 1.3 complete, results come with cryptographic proofs. This means:

- No need for wingman/double-check for PRP results with valid proofs
- Can accept results from untrusted machines
- Verification at 0.5% cost (a 100-hour test verified in 30 minutes)

This is the single most important feature for scaling beyond a trusted cluster.

#### 5.2 Work type routing

Assign work based on hardware capability:

| Hardware | Best work type |
|----------|---------------|
| Fast CPU (high single-core) | Factorial/primorial (sequential) |
| Many-core CPU | k×b^n sieve + test (parallel) |
| NVIDIA GPU | GeneferOCL, GPU sieve, GPU PRP |
| Small/slow machine | Proof verification (cert) |
| Unreliable machine | Trial factoring (cheap to re-do) |

#### 5.3 Public participation API

Extend the existing coordinator with:

- User registration and attribution
- Work preferences (form, difficulty, hardware type)
- Result submission with proof upload
- Leaderboard / credit system
- Challenge events (time-limited searches for specific forms)

This is the PrimeGrid/GIMPS model, adapted for a non-BOINC architecture.

**Effort:** Phase 5 is a project-level effort, ~months.

---

### Phase 6: Form-Specific Optimizations (competitive on specific targets)

**Goal:** Match or exceed the state of the art for our target forms.

#### 6.1 Wagstaff — Exploit the gap

**Opportunity:** No active organized project. 9.3M exponent gap not exhaustively
searched. No competition.

Optimizations needed:
- GWNUM Vrba-Reix test (already implemented in gwnum.rs)
- Multiplicative order sieve (already implemented)
- PFGW cross-verification (already implemented)
- Missing: systematic gap search orchestration, checkpoint resumption for multi-day tests

#### 6.2 Non-base-2 Sierpinski/Riesel — Best ROI

**Opportunity:** Conjectures 'R Us tracks hundreds of bases with low search limits.
Each prime found SOLVES A CONJECTURE.

Current capability: darkreach's kbn module handles arbitrary bases. Need:
- Import remaining k-value lists from CRUS
- Auto-select bases with fewest remaining k-values and lowest search limits
- Pipeline: mtsieve sieve → PRST/LLR test → proof → submit

#### 6.3 Palindromic — High visibility

**Opportunity:** Current record is 2,718,281 digits (Propper & Batalov, Aug 2024).
Only one team competing.

darkreach's near_repdigit module implements the right approach. Need:
- GWNUM acceleration for 3M+ digit candidates (currently PFGW subprocess)
- BLS proof generation (partially implemented in proof.rs)
- Target d = 3,000,001 or d = 3,141,593

#### 6.4 Twin primes — No active project, stale record

**Opportunity:** Record from Sept 2016, nearly a decade stale. PrimeGrid's Twin
Prime Search ended. Zero competition.

darkreach's twin module exists. Need:
- Deeper quad sieve (currently limited)
- PRST integration for Proth+LLR testing
- GPU sieve via twinsieve (CPU only in mtsieve, may need custom GPU kernel)

---

### Public Compute Release Parity (new track)

To directly match competitor public-compute operations, darkreach now tracks a dedicated roadmap:

- `docs/roadmaps/public-compute.md`

This track covers:

- Volunteer worker packaging and signed distribution
- Auto-update channels (`stable`/`beta`) with rollback
- Assignment expiry/reclaim policies
- Quorum validation + host reputation
- Release canary/ramp controls
- Volunteer credits, teams, and challenge cadence

This is intentionally separated from engine optimization because release lifecycle and trust
controls are now the main bottleneck for scaling beyond private clusters.

---

## 4. Priority Matrix

Items ordered by (impact × feasibility / effort):

| # | Item | Impact | Effort | Phase |
|---|------|--------|--------|-------|
| 1 | PRST proof pipeline (`-proof save/build/cert`) | Critical | 2-3 days | 1.1 |
| 2 | PFGW proof collection | High | 1-2 days | 1.2 |
| 3 | GeneferOCL subprocess integration | High | 1 day | 2.1 |
| 4 | mtsieve subprocess integration (srsieve2cl first) | High | 1 week | 2.2 |
| 5 | ECM integration (ecm crate) | Medium | 2 days | 3.2 |
| 6 | Sieve depth auto-tuning | Medium | 1-2 days | 3.3 |
| 7 | t5k.org submission automation | Medium | 1 day | 1.4 |
| 8 | Multi-stage sieve pipeline | Medium | 1 week | 3.1 |
| 9 | CRUS k-value import + auto-strategy | Medium | 3-5 days | 6.2 |
| 10 | Native GWNUM FFT completion | High | 2 weeks | 4.1 |
| 11 | Pietrzak PRP proof generation | Critical | 2-4 weeks | 1.3 |
| 12 | FFT auto-benchmarking | Medium | 2 days | 4.2 |
| 13 | GPU PRP testing (cudarc) | Very High | Weeks-months | 2.3 |
| 14 | Public participation API | Medium | Months | 5.3 |

**Recommended execution order:** 1 → 2 → 7 → 3 → 4 → 5 → 6 → 8 → 9 → 10 → 11 → 12 → 13 → 14

---

## 5. World Records by Form (Feb 2026)

Reference table showing current records and darkreach's competitive position.

| Form | Record | Digits | Date | Discoverer | darkreach Competitive? |
|------|--------|--------|------|-----------|----------------------|
| Mersenne 2^p−1 | M136279841 | 41,024,320 | Oct 2024 | Durant/GIMPS | No (GIMPS domain) |
| Gen. Fermat b^(2^n)+1 | GFN-21 | 13,426,224 | Oct 2025 | PrimeGrid | No (GPU-dominated) |
| Gen. Cullen n×b^n+1 | 4052186×69^4052186+1 | 7,451,366 | Apr 2025 | PrimeGrid | Possible long-term |
| Riesel k×2^n−1 | 107347×2^23427517−1 | 7,052,391 | Aug 2024 | Propper/PG | Possible (non-base-2) |
| Factorial n!±1 | 632760!−1 | 3,395,992 | Oct 2024 | PrimeGrid | Possible with GWNUM |
| Primorial p#±1 | 9562633#+1 | 4,150,000 | Jun 2025 | PrimeGrid | Not viable solo |
| Palindromic | 10^2718281−5×10^1631138−... | 2,718,281 | Aug 2024 | Propper/Batalov | **Target** |
| Twin | 2996863034895×2^1290000±1 | 388,342 | Sep 2016 | PrimeGrid | **Target** (stale) |
| Sophie Germain | 2618163402417×2^1290000−1 | 388,342 | Feb 2016 | PrimeGrid | Possible |
| Wagstaff (2^p+1)/3 | (2^15135397+1)/3 | ~4,556,000 | 2024 PRP | — | **Target** (no competition) |
| Cullen n×2^n+1 | 6679881×2^6679881+1 | 2,010,852 | Jul 2009 | PrimeGrid | Possible |
| Woodall n×2^n−1 | 17016602×2^17016602−1 | 5,122,515 | Mar 2018 | PrimeGrid | Possible |
| Repunit (b^n−1)/(b−1) | R(10,8177207) PRP | 8,177,207 | 2024 | — | PRP only |
| Carol/Kynea (2^n±1)²−2 | Various at 300K+ digits | — | — | — | Possible |

**Best targets for darkreach** (marked above):
1. **Wagstaff** — No competition, large unexplored gap, darkreach already has Vrba-Reix
2. **Twin primes** — Record nearly a decade stale, no active search
3. **Palindromic** — Only one competing team, BLS proofs available
4. **Non-base-2 Sierpinski/Riesel** — Best ROI at 1-10 core-years per discovery

---

## 6. Key Technical Lessons

### From GIMPS

1. **Proofs > double-checks.** Pietrzak proofs at 0.5% cost saved GIMPS "tens of thousands
   of future double-checks." This is the single highest-value feature for any distributed
   prime search.

2. **The factoring pipeline matters.** GIMPS eliminates 60%+ of candidates via trial
   factoring before touching PRP. Every avoided primality test saves days of compute.

3. **Error checking is non-negotiable.** Pre-Gerbicz, GIMPS had ~1.5% error rate on LL
   tests. For multi-day computations, undetected errors waste enormous resources.

4. **GPUs change the economics.** A single A100 delivers 30-100x more throughput than a
   high-end CPU core for modular exponentiation. Cloud GPUs are available on-demand.

5. **Assignment management is hard.** PrimeNet's tiered deadline system (30-360 days),
   reliability requirements, and assignment reclamation represent 30 years of operational
   wisdom about unreliable volunteers.

### From PrimeGrid

1. **Separate sieving from testing.** PrimeGrid sieves to "optimal depth" centrally,
   then distributes only survivors for primality testing. Sieve work is cheap and
   parallelizes differently than primality testing.

2. **mtsieve is the reference implementation.** 20+ specialized sieve programs, all using
   BSGS, all with OpenCL GPU variants. Rather than reinventing, integrate.

3. **PRST replaces LLR2.** Pavel Atnashev's PRST is the current state of the art for
   k×b^n±1, n!±c, n#±c forms. It includes Gerbicz checking, proof generation, and
   certificate verification. darkreach already uses PRST as a subprocess — the proof
   pipeline integration (Phase 1.1) is the next logical step.

4. **Challenge events drive engagement.** PrimeGrid's 9 challenges per year maintain
   volunteer interest. Even a small project can benefit from focused search sprints.

5. **GPU forms win big.** GeneferOCL found the largest PrimeGrid prime ever (13.4M digits).
   GPU-accelerated forms dominate the leaderboard.

### From the ecosystem

1. **t5k.org requires PROVEN primes.** PRPs are rejected. darkreach must generate proofs
   for any discovery to be recognized. This is the most urgent gap.

2. **CRUS is a goldmine.** Hundreds of open Sierpinski/Riesel conjectures with low search
   limits. Each prime found solves a conjecture — a publishable result. Many bases have
   only 1-3 remaining k-values.

3. **The "no competition" niches are real.** Wagstaff, twin primes, and non-base-2
   Sierpinski/Riesel all have minimal or zero organized searching. A focused effort
   on these has realistic odds of discovery.

---

## References

### GIMPS
- [GIMPS Main](https://www.mersenne.org/)
- [GIMPS The Math](https://www.mersenne.org/various/math.php)
- [GIMPS Work Types](https://www.mersenne.org/worktypes/)
- [GIMPS Assignment Rules](https://www.mersenne.org/thresholds/)
- [GIMPS Milestones](https://www.mersenne.org/report_milestones/)
- [M52 Press Release](https://www.mersenne.org/primes/press/M136279841.html)
- [Pietrzak VDF Paper](https://eprint.iacr.org/2018/627.pdf)

### PrimeGrid
- [PrimeGrid Main](https://www.primegrid.com/)
- [PrimeGrid Server Status](https://www.primegrid.com/server_status.php)
- [PrimeGrid Wiki](https://primegrid.fandom.com/wiki/PrimeGrid_Wiki)
- [PrimeGrid Forums (News)](https://www.primegrid.com/forum_forum.php?id=1)

### BOINC / Release Infrastructure
- [BOINC Overview](https://boinc.berkeley.edu/trac/wiki/BoincOverview)
- [BOINC App Versioning](https://github.com/BOINC/boinc/wiki/App-Versioning)
- [BOINC Sign Executable](https://github.com/BOINC/boinc/wiki/Sign_executable)
- [BOINC Releases](https://github.com/BOINC/boinc/releases)

### Software
- [PRST (GitHub)](https://github.com/patnashev/prst)
- [mtsieve (GitHub)](https://github.com/primesearch/mtsieve)
- [GpuOwl/PRPLL (GitHub)](https://github.com/preda/gpuowl)
- [mfaktc (GitHub)](https://github.com/primesearch/mfaktc)
- [Mlucas (GitHub)](https://github.com/primesearch/Mlucas)
- [ecm Rust crate](https://crates.io/crates/ecm)

### Records & Community
- [t5k.org Top 5000 Primes](https://t5k.org/)
- [Conjectures 'R Us — Sierpinski](http://www.noprimeleftbehind.net/crus/Sierp-conjectures.htm)
- [Conjectures 'R Us — Riesel](http://www.noprimeleftbehind.net/crus/Riesel-conjectures.htm)
- [Palindromic Prime Records](https://www.worldofnumbers.com/palprim2.htm)
- [Prime-Wiki PrimeGrid](https://www.rieselprime.de/ziki/PrimeGrid)

---

## 7. Landing Page Competitive Analysis (Feb 2026)

Deep analysis of competitor and reference landing pages to inform darkreach.ai's
homepage strategy. Every major prime-hunting project has a dated web presence —
this is a significant opportunity for differentiation.

### 7.1 Competitor Landing Pages

#### GIMPS (mersenne.org)

**Design era:** Mid-2000s. jQuery dropdown menus, dense text blocks, no responsive framework.

**Page structure:**
1. Header with login/register bar
2. Hero: project name + featured discovery (M52, 41M digits)
3. Live statistics dashboard (real-time computing metrics)
4. News/updates feed (chronological announcements)
5. "Make Math History" recruitment pitch
6. Educational content for newcomers
7. Footer with merchandise links

**Lead messaging:** "Great Internet Mersenne Prime Search — Finding World Record Primes Since 1996."
Leads with the world record achievement, then explains the project. Academic tone throughout.

**Key stats displayed (live):**

| Metric | Value |
|--------|-------|
| Computing power | 5,695,331 GFLOP/s |
| Active devices | 3,099,191 CPUs & GPUs |
| Mersenne primes known | 52 (18 by GIMPS) |
| Tested below | 139.6M exponents |
| Largest prime digits | 41,024,320 |

**CTAs:** "Join GIMPS", "Download Software", $3,000 discovery award. Functional rather than
persuasive — no urgency language, no onboarding funnel.

**Strengths:**
- Live stats dashboard is the single most compelling element — millions of active devices
  creates visceral sense of scale
- World record framing communicates significance to non-mathematicians
- 28 years of history establishes unmatched credibility
- Low barrier pitch: "All you need is a personal computer, patience, and a lot of luck"

**Weaknesses:**
- No visual hierarchy for scanning — dense text walls
- No progress visualizations despite excellent data (no charts, no progress bars)
- Buried onboarding behind dropdown menus
- No social proof beyond stats (no testimonials, no contributor spotlights)
- No hero image or illustration

#### PrimeGrid (primegrid.com)

**Design era:** Mid-2000s BOINC project template. Monospace fonts, no images beyond logo.

**Page structure:**
1. Logo + donation banner
2. "Join PrimeGrid" 4-step instructions (first content block)
3. Returning participant links
4. Community resources
5. Leaderboards (6 ranking pages)
6. Active subprojects table with task counts
7. Challenge information (Tour de Primes 2026)
8. Statistics dashboard
9. Recent significant primes with hardware specs
10. News feed + newly reported primes
11. Top crunchers leaderboard

**Lead messaging:** No hero, no mission statement above the fold. Leads with procedural
onboarding: "1. Download BOINC, 2. Enter URL, 3. Select subprojects, 4. Start computing."
Purpose communicated indirectly through data, not copywriting.

**Key stats displayed:**

| Metric | Value |
|--------|-------|
| Total users | 357,266 |
| Total hosts | 885,651 |
| Tasks in progress | 321,149 |
| Primes discovered | 100,684 |
| Mega primes | 3,394 |
| Computing power | 4,329 TFLOPS |

**CTAs:** All plain-text hyperlinks. No styled buttons, no contrasting CTA colors. "Download",
"Make a donation", "Create or Join a Team" — all equally weighted.

**Strengths:**
- Data density communicates legitimacy — 357K users and 100K primes need no marketing copy
- Full transparency (server load, generation timestamps, per-subproject task counts)
- Community challenges (9/year) create recurring engagement
- Detailed discovery announcements celebrate contributors by name

**Weaknesses:**
- No hero section or emotional hook — newcomers who don't know prime hunting will bounce
- No visual design to speak of — monospace text on white
- High onboarding friction (download BOINC → enter URL → configure subprojects)
- No storytelling about what prime hunting means or why it matters
- No social proof beyond raw numbers

#### Folding@home (foldingathome.org)

**Design era:** Modern but traditional. Functional design with clear conversion funnel.

**Page structure:**
1. Navigation (8 items with dropdowns)
2. Hero with platform-specific download buttons
3. Latest blog posts (3 entries)
4. "JOIN THE COMMUNITY" social links
5. Three feature cards ("1 in a million" / "together" / "what's FOLDING?")
6. Team/leadership section
7. Partner logos (NVIDIA, AMD, Microsoft, AWS, Oracle, VMware, ARM)

**Lead messaging:** "START FOLDING NOW" — imperative, action-first hero. Subheadline frames
the value: "become a citizen scientist and contribute your compute power to help fight global
health threats like COVID19, Alzheimer's Disease, and cancer." Immediately followed by trust
line: "Our software is completely free, easy to install, and safe to use."

**Messaging structure:** Action → Purpose → Reassurance.

**CTAs:** Platform-specific download buttons (Linux, Windows, Mac) as the single dominant
funnel. "JOIN THE COMMUNITY" as secondary. Donate as tertiary.

**Strengths:**
- Action-first hero — no preamble, immediately told what to do and why
- Disease names (COVID-19, Alzheimer's, cancer) create emotional urgency
- Trust reassurance ("free, easy, safe") addresses top three hesitations
- Partner logos provide massive institutional credibility
- Multiple community channels (Discord, forums) signal active participation

**Weaknesses:**
- No live stats on the homepage despite rich statistics backend
- Blog posts are low-impact for conversion
- Feature cards use vague inspirational copy rather than concrete metrics
- No contributor stories or testimonials

#### SETI@home (seti.berkeley.edu)

**Design era:** Early 2010s placeholder. Minimal to the point of sparse.

**Page structure:**
1. Header
2. Hero/description (one paragraph)
3. Two CTA cards (SETI@home + Breakthrough Listen)
4. Social media links
5. Footer

**Lead messaging:** "The Search for ExtraTerrestrial Intelligence at UC Berkeley."
Identity-first, then description, then two project cards.

**Strengths:** Mission is inherently fascinating; UC Berkeley backing.

**Weaknesses:** Essentially a hub page. No statistics, no social proof, no visualizations,
no onboarding flow, no explanation of technology or discoveries. Feels abandoned
(SETI@home has been in hibernation since March 2020).

#### BOINC (boinc.berkeley.edu)

**Design era:** 2000s institutional/academic.

**Page structure:**
1. Logo/branding
2. Value proposition ("Compute for Science")
3. Dual-path CTA (Join Science United vs Download BOINC)
4. Project news feed
5. Learning resources
6. Community links
7. Support/contribution pathways

**Lead messaging:** "Compute for Science" — clear, purpose-driven headline. Supporting copy
emphasizes ease ("It's easy and safe") and breadth (~30 active science projects).

**Strengths:**
- Dual onboarding paths (casual via Science United vs power user via BOINC download)
- Mission-driven messaging works for volunteer projects
- UC Berkeley and NSF institutional credibility

**Weaknesses:** Text-heavy, 2000s-era design. Minimal visual design. White/light background,
no animation, no gradient effects.

#### t5k.org (The Prime Pages)

**Design era:** Early 2000s academic web.

**Page structure:**
1. Navigation header
2. Hero with "31 MPH" speed limit sign image (playful prime reference)
3. Dictionary-style prime number definition
4. Database features
5. Research/theory section (Riemann Hypothesis, proving methods)
6. Collapsible FAQ
7. Four-column footer

**Lead messaging:** "The PrimePages: prime number research & records" — positions as
the "Guinness Book of prime number records." Updated hourly.

**Key stats:** 5,000 largest primes tracked, 50,000,000 primes listed, 2,000+ years of history.

**Strengths:**
- "Updated hourly" builds trust — this is a living resource
- Layered depth: starts accessible (definitions), progresses to advanced (Riemann Hypothesis)
- Verification status transparency
- "Prime Curios" adds personality alongside academic rigor
- Primality checker as interactive tool

**Weaknesses:** Classic academic design, text-heavy, minimal visual hierarchy.

---

### 7.2 Modern Reference Landing Pages

These are not competitors but represent the state of the art in landing page design.

#### Render (render.com)

**Pattern: Persuasion funnel.** Hero (attention) → Logo carousel (credibility) → 3-step
walkthrough (simplicity) → Feature deep-dives → Testimonial → Final CTA.

**Key techniques:**
- Rotating hero headline showing different use cases — demonstrates breadth
- "Click, click, done" 3-step onboarding — makes complex feel easy
- Hard credibility numbers: "4.5 million builders", $100M funding at $1.5B valuation
- Dark mode with purple-to-orange gradient accents
- Real testimonial with name, title, company
- CTA repeated at multiple scroll positions

#### Vercel (vercel.com)

**Pattern: Outcome-driven.** Hero → Quantified customer outcomes → Solution categories →
Product features → Infrastructure → Templates.

**Key techniques:**
- Quantified outcomes instead of feature lists: "build times from 7 min to 40 sec"
- Solution-first organization (by what you're building, not by product feature)
- "Start Deploying" instead of "Sign Up" — action-oriented CTA language
- Coined concept ("Framework-Defined Infrastructure") to position as category creator
- Named case studies with real performance data

---

### 7.3 Cross-Cutting Patterns

What the most effective landing pages all share:

| Pattern | Who Does It | Impact |
|---------|-------------|--------|
| **Live/real-time data** | GIMPS, PrimeGrid | Creates "something is happening now" energy |
| **Lead with biggest achievement** | GIMPS (M52), Folding@home (COVID-19) | Communicates significance instantly |
| **3-step simplicity** | Render, Folding@home, BOINC | Reduces perceived friction |
| **Quantified outcomes** | Vercel, Render, GIMPS | Concrete > vague |
| **Partner/institutional logos** | Folding@home, BOINC, Render | Borrowed credibility |
| **Action-oriented CTAs** | Vercel ("Start Deploying"), F@h ("START FOLDING NOW") | Creates momentum |
| **Rotating/dynamic hero** | Render | Shows breadth without clutter |
| **Contributor celebration** | PrimeGrid (discoverer names + hardware) | Humanizes the project |
| **Challenge/event cadence** | PrimeGrid (9/year) | Recurring engagement hooks |

---

### 7.4 darkreach Landing Page Audit

Current homepage sections vs what competitors and best practices suggest:

| Section | darkreach Status | Gap | Priority |
|---------|-----------------|-----|----------|
| **Hero** | AI narrative, static headline | Add rotating headline cycling prime forms/achievements | Medium |
| **Stats bar** | 5 metrics, hardcoded mock data | Connect to `api.darkreach.ai/api/stats` for live numbers | **High** |
| **Feature grid** | 3 AI capability cards | Good — unique differentiator vs competitors | Keep |
| **Pipeline** | 4-step technical visualization | Good depth | Keep |
| **12 Prime Forms** | Form cards with formulas | Strong breadth display (PrimeGrid-style taxonomy) | Keep |
| **Discoveries** | 10-row mock table | Connect to Supabase for real discoveries | **High** |
| **Comparison** | 3-column matrix vs GIMPS/PrimeGrid | Add rows: live stats, challenge events, in-browser | Low |
| **CTA** | Two code-block paths (worker/self-host) | Add simpler 1-click path; reduce friction | **High** |
| **Missing: Live activity feed** | — | Real-time prime discovery stream (unique vs all competitors) | **High** |
| **Missing: Progress visualization** | — | Search range coverage, "X% explored" progress bars | Medium |
| **Missing: Contributor spotlight** | — | Featured discoverer with hardware and story | Medium |
| **Missing: Goal/mission statement** | — | Clear "why" above fold for non-technical visitors | Medium |
| **Missing: Testimonials/logos** | — | Even self-sourced ("Built with Rust + GMP + Rayon") | Low |

---

### 7.5 Recommendations (ordered by impact)

#### Tier 1: Live data (the biggest differentiator)

No prime-hunting project shows live data on their landing page. darkreach already
has the infrastructure (Supabase Realtime, WebSocket, REST API).

1. **Live stats bar** — Replace hardcoded numbers with `api.darkreach.ai/api/stats`.
   Fallback to static values when API is unreachable. Show pulsing green dots
   (already implemented in UI, just needs data source).

2. **Real-time discovery feed** — A scrolling ticker or card stream showing primes
   as they're found. Use Supabase Realtime `INSERT` subscription (same pattern as
   dashboard's `use-prime-realtime.ts`). This would be genuinely unique — no
   competitor has anything like it.

3. **Live fleet stats** — "X workers across Y servers testing Z candidates/sec right now."
   Pull from `/api/fleet` endpoint.

#### Tier 2: Reduce onboarding friction

Every competitor requires downloading software. darkreach's CTA currently shows
multi-line terminal commands — high friction.

4. **1-click worker download** — Pre-built binary download button with OS detection
   (already have `os-detector.tsx` component). "Download for macOS" primary CTA,
   with code blocks as secondary.

5. **3-step visual walkthrough** — Render-style "Download → Configure → Hunt" with
   icons and minimal text. Replace the current code-heavy CTA section.

6. **In-browser demo** — Long-term: a WebAssembly sieve demo that runs in the
   browser. Immediate differentiation vs BOINC-dependent projects.

#### Tier 3: Social proof and storytelling

7. **Contributor spotlight** — "Recent Discovery" featured card with discoverer
   pseudonym, hardware specs, and form name. PrimeGrid does this in their news;
   darkreach can do it more visually.

8. **Achievement badges** — "First 18,000-digit prime", "392K primes found",
   "12 search forms". Visual badges that communicate scale.

9. **"Why prime hunting matters"** — A concise block explaining the mathematical
   significance for non-technical visitors. Folding@home's disease names create
   urgency; darkreach needs an equivalent emotional hook ("Every prime discovered
   is a permanent contribution to mathematics").

#### Tier 4: Engagement loops

10. **Challenge events** — PrimeGrid runs 9/year. darkreach could run monthly
    focused searches ("February: Twin Prime Sprint"). Announce on landing page.

11. **Goal progress bar** — "Searching k·b^n±1 for n = 60,000..100,000" with
    a visual progress indicator. GIMPS shows exponent frontier progress;
    darkreach should too.

12. **Leaderboard preview** — Top 3-5 contributors shown on the homepage with
    primes found count. Links to full leaderboard page.

---

### 7.6 Competitive Positioning Summary

```
                    Modern Design
                         ↑
                         |
              darkreach ●|
                         |
         ────────────────┼────────────────→ Live Data / Scale
                         |
               Folding@h ●
                         |
           BOINC ●       |        ● GIMPS
                         |
          SETI ●   t5k ● |  ● PrimeGrid
                         |
                    Dated Design
```

darkreach is already the only project with modern design. Adding live data to the
landing page would move it into a quadrant no competitor occupies — modern design
AND live data. This is the primary strategic opportunity.

### References

- [GIMPS](https://www.mersenne.org/)
- [PrimeGrid](https://www.primegrid.com/)
- [Folding@home](https://foldingathome.org/)
- [SETI@home](https://seti.berkeley.edu/)
- [BOINC](https://boinc.berkeley.edu/)
- [t5k.org](https://t5k.org/)
- [Render](https://render.com/) (modern landing page reference)
- [Vercel](https://vercel.com/) (modern landing page reference)
