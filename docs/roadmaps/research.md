# Research Roadmap

Discovery strategy, competitive analysis, publication pipeline, and references.

**Key files:** `docs/`, `docs/roadmaps/competitive-analysis.md`, `docs/roadmaps/public-compute.md`

---

## Strategic Targets: Where to Actually Find Primes

### Tier 1: Highest-value targets (realistic discovery with 8-16 cores, 1-2 years)

**Wagstaff primes ((2^p+1)/3) — NO ACTIVE SEARCHER**
- 9.3 million exponent gap between p=4,031,399 and p=13,347,311 is NOT exhaustively searched
- Range above p=15,135,397 is completely open
- No organized project (PrimeGrid does not cover Wagstaff)
- PRP test at p~5M (~1.5M digits): 2-6 hours per candidate
- ~620,000 prime exponents in the gap — systematic search feasible
- A discovery here gets published and recognized immediately

**k*b^n+/-1 for non-base-2 Sierpinski/Riesel conjectures — HUNDREDS OF OPEN TARGETS**
- Conjectures 'R Us tracks bases 2-1030+, many with only 1-3 remaining k values
- Many bases have search limits below n=3,000 — barely scratched
- Example: Riesel base 6 has 3 remaining k values
- Each prime found SOLVES A CONJECTURE — publishable result
- Testing at n~1M produces ~500K-1M digit numbers, testable in 1-6 hours
- No organized project for most bases (PrimeGrid only covers base 2 and 5)

### Tier 2: Good targets (months to years, more compute needed)

**Primorial primes (p#-1) — LAGS BEHIND p#+1**
- p#+1 record: 9,562,633# + 1 (4.15M digits, June 2025)
- p#-1 record: 6,533,299# - 1 (2.84M digits, Aug 2024)
- 3M-prime gap between the two forms

**Twin primes — NO ACTIVE PROJECT, RECORD STALE SINCE 2016**
- Record: 2996863034895 * 2^1290000 +/- 1 (388K digits, Sept 2016) — nearly a decade stale
- PrimeGrid's Twin Prime Search ended. Zero competition.
- New activity below record: #3 entry (144K digits) found Jan 2026; multiple new entries in 2024-2025

**Factorial primes (n!+1) — LAGS n!-1 BY 210K IN n-VALUE**
- n!-1 record at n=632,760 (3.4M digits, Oct 2024); n!+1 record at n=422,429 (2.2M digits, Feb 2022)
- PrimeGrid rebooted Factorial Prime Search in September 2025
- No new factorial primes found since 632,760!-1

**Compositorial primes — NEW RECORDS**
- 751,882!/751,879# + 1 (3,765,621 digits, Dec 2025) — PrimeGrid discovery

### Tier 3: Niche targets

**Palindromic primes (non-base-10)** — Almost completely unexplored for bases 3-9+

**Multi-factorial primes** — Retesting needed due to past software bugs, low search limits

### What NOT to target (dominated by PrimeGrid/GIMPS)

- Mersenne primes (GIMPS with thousands of GPUs; M136279841 found Oct 2024 using NVIDIA GPUs, first GPU discovery)
- Sierpinski/Riesel base 2 (PrimeGrid massive compute; 5 k-values remain for Sierpinski, 41 for Riesel)
- Cullen/Woodall (PrimeGrid active)
- Generalized Fermat (PrimeGrid found GFN-21 prime with 13.4M digits in Oct 2025 — largest PrimeGrid prime ever)
- Wieferich (exhausted to 2^64)

---

## The t5k.org Top 5000

- Current minimum: **825,084 digits** (growing ~50K/year, as of Feb 2026)
- Special forms have their own Top 20 lists with lower thresholds
- Any prime in a recognized archivable form gets listed regardless of size
- **CRITICAL: Only PROVEN primes accepted. PRPs are rejected.**
- darkreach currently only does Miller-Rabin (probabilistic) — must add proof capability

**51 archivable form categories** including: Factorial (#9), Palindrome (#39), Primorial (#41), Wagstaff (#50), Twin (#48), Sophie Germain (#46), Generalized Fermat (#16), Repunit (#44), Woodall (#51), Cullen (#3).

### Path from discovery to recognition

```
1. Find PRP candidate (darkreach sieve + PRP test)
2. Prove primality (BLS/Proth/LLR for special forms, ECPP for general)
3. Cross-verify with PFGW/PRST on different hardware
4. Generate primality certificate
5. Submit to t5k.org (prover account + proof code)
6. Update OEIS sequences
7. Announce on mersenneforum.org
8. Write paper if algorithmic innovation involved
```

---

## Record-Breaking Strategies

Detailed search strategies with time estimates for an Apple Silicon workstation (16 cores).

### Strategy 1: Non-Base-2 Sierpinski/Riesel (BEST ROI) — Phase 1

**Deployment: Runs now on a single AX42 ($53/mo). No GWNUM needed for n<500K.**

**Source:** Conjectures 'R Us

Pick bases where only 1-3 k-values remain unresolved and search limits are low (< n=500K). Extending to n=1M-2M has ~40% chance of finding a prime per k-value.

**Tools:** mtsieve/srsieve2 for sieving, LLR/PRST for testing. darkreach's Proth + LLR implementations handle base-2; other bases use Miller-Rabin.

**Timeline:** Weeks to months for first discovery.

### Strategy 2: Palindromic Prime Record (HIGH VISIBILITY) — Phase 2

**Deployment: Requires GWNUM (100x speedup at 3M digits). Start after Phase 2 hardware ($215-430/mo).**

**Current record:** 2,718,281 digits (Propper & Batalov, Aug 2024)

**Technique:** Near-repdigit construction. N+1 has 10^(d/2) as known factor, enabling BLS proof. Discoveries are provable primes at sizes where ECPP is infeasible.

**Target:** d = 3,000,001 or d = 3,141,593.

**Timeline:** 1-3 years for a new record (after GWNUM works).

### Strategy 3: Wagstaff PRP (HIGH PRESTIGE, NO COMPETITION) — Phase 2

**Deployment: Requires GWNUM (50x speedup at 1.5M digits). Start after Phase 2 hardware ($215-430/mo).**

**Form:** (2^p+1)/3, p prime.

**Vrba-Reix test:** S(0)=3/2 mod N, iterate S(i+1)=S(i)^2-2 mod N. Modular reduction nearly free (same trick as Mersenne).

Times below assume GWNUM:

| Exponent p | Digits | Single-core time |
|------------|--------|-----------------|
| 5M | 1.5M | ~3 days |
| 10M | 3M | ~2 weeks |
| 15M | 4.5M | ~4 weeks |
| 20M | 6M | ~3.5 months |

Without GWNUM (GMP only), multiply these times by ~50-100x. A single p=5M test would take months instead of days.

**Limitation:** PRPs cannot be proven — no deterministic test exists. The largest proven Wagstaff prime is p=141,079 (42,469 digits, Oct 2025, via ECPP). Large PRPs at p>138,937 remain unproven and may never be — ECPP cannot scale to millions of digits.

### Strategy 4: Factorial Primes (MODERATE ROI, AUTO-PROVABLE) — Phase 3

**Deployment: Requires GWNUM (100x+ speedup at 3.4M digits) + Gerbicz error checking + AX102 for high single-core clocks. Start after Phase 3 hardware ($550-760/mo).**

**Frontier:** n!-1 at n=632,760; n!+1 at n=422,429.

**Proof is free:** Pocklington for n!+1, Morrison for n!-1.

**Per candidate at n~700K with GWNUM:** ~2 weeks per core. With 16 cores: ~400+ candidates/year.
**Per candidate at n~700K with GMP (current):** ~12-14 months per core — not competitive.

**Timeline:** 1-2 years for a discovery (after GWNUM + fleet).

### Resource ROI Comparison

| Search Type | Core-years per discovery | Provable? | Competition | Deployment Phase |
|------------|------------------------|-----------|-------------|-----------------|
| Sierpinski/Riesel (non-base-2) | **1-10** | Yes (LLR/Proth) | Low | **Phase 1 (now, $53/mo)** |
| Palindromic record | **100-1,000** | Yes (BLS) | 1 team | Phase 2 (GWNUM, $215-430/mo) |
| Wagstaff PRP | **~3,000** | No (PRP only) | None | Phase 2 (GWNUM, $215-430/mo) |
| Factorial | **~2,300** | Yes (Pocklington/Morrison) | PrimeGrid | Phase 3 (GWNUM+fleet, $550-760/mo) |
| Primorial | **~140,000** | Yes | PrimeGrid (not viable solo) | Not recommended |

---

## Publication Pipeline

### t5k.org Submission (Step-by-Step)

1. **Create prover account** at t5k.org/bios/submission.php — one-time setup
2. **Establish proof code** — short alphanumeric string documenting software/people/project
3. **Submit the prime** — formula (under 255 chars) or full decimal expansion
4. **Verification queue** — system performs trial division + PRP check
5. **Only PROVEN primes accepted.** PRPs are rejected.

### OEIS Contribution

1. Register at oeis.org (new accounts limited to 3 pending submissions)
2. Submit via oeis.org/Submit.html — minimum 4 terms, clear definition
3. Four-stage review: Proposal -> Review -> Approval -> Live
4. Software citation: `(Other) # Using darkreach (Rust/GMP), https://github.com/...`

**Key sequences:**
- A002981: n where n!+1 is prime (last term: 422,429)
- A002982: n where n!-1 is prime (last term: 208,003 in OEIS, actual frontier at 632,760)
- A002385: Palindromic primes (base 10)

### Mersenne Forum Announcement Protocol

1. **Verify first, announce second.** Independent confirmation before public announcement.
2. Post in correct subforum
3. Include: expression, digit count, software used, verification status, t5k.org link
4. Subject format: "[New record] 632760!-1 is prime (3,395,992 digits)"

### Academic Publication Venues

| Journal | Best for | Notes |
|---------|----------|-------|
| **Mathematics of Computation** (AMS) | Algorithmic innovation + computation | Gold standard |
| **Journal of Integer Sequences** (JIS) | New sequence terms, discovery announcements | Open access |
| **INTEGERS** | Combinatorial number theory | **Bans AI-generated content** |
| **Experimental Mathematics** (T&F) | Computational experiments | |
| **arXiv math.NT** | Preprints, discovery announcements | Primary for discoveries |
| **arXiv cs.MS** | Software papers | If paper focuses on darkreach itself |

### Verification Standards

Independent verification requires:
- **Different software** from discovery program
- **Different hardware architecture** (ideally spanning Intel/AMD/ARM/GPU)
- **Different algorithm** where possible

**Cross-verification tools by form:**

| Form | Verification Software |
|------|----------------------|
| k*b^n+1 (Proth) | LLR, PFGW, PRST |
| k*b^n-1 (Riesel) | LLR, PFGW, PRST |
| n!+/-1 | PFGW, Primo (small), PRST |
| Palindromic | PFGW, Primo (up to ~40K digits) |
| Wagstaff PRP | PFGW, custom Vrba-Reix |

### Recommended Discovery Pipeline

```
Day 0:    Discovery — record exact expression, digit count, timestamp, hardware, version
          DO NOT ANNOUNCE PUBLICLY
Day 0-7:  Cross-verify with different tool (PRST/PFGW) on different hardware
Day 7-14: Submit to t5k.org, seek community verification on Mersenne Forum
Month 1:  Update OEIS sequences, post arXiv preprint if novel
Month 3-6: Submit journal paper (JIS for discovery, Math.Comp for methods)
```

---

## Rust Ecosystem Notes

### Arithmetic library comparison

| Library | vs GMP speed | Primality testing | Dependencies |
|---|---|---|---|
| **rug** (GMP bindings) | 1.0x (reference) | Baillie-PSW + Miller-Rabin | System GMP, LGPL |
| **malachite** | 0.55-0.75x | None built-in | Pure Rust, LGPL |
| **num-bigint** | 0.004-0.15x | Via num-primes | Pure Rust, MIT |
| **dashu** | 0.14-0.5x | None built-in | Pure Rust |

**Verdict:** rug/GMP is unambiguously the right choice. No Rust crate approaches GMP for million-digit arithmetic.

### Useful crates

| Crate | Version | Use |
|---|---|---|
| `rug` | 1.28.1 (Jan 2026) | All big-integer arithmetic (current) |
| `malachite` | 0.9.1 (Feb 2026) | Pure-Rust bigint alternative (0.55-0.75x GMP speed) |
| `machine-prime` | | Deterministic u64/u128 primality |
| `primesieve-sys` | | Fast prime generation |
| `ecm` | 1.0.1 | ECM factoring with rug backend |
| `flint3-sys` | 3.3.1 | FLINT FFI bindings (exists, not yet updated to FLINT 3.4.0) |
| `discrete-logarithm` | | BSGS implementation (reference) |
| `cudarc` | 0.19.2 | CUDA GPU compute from Rust |
| `axum` | 0.8.8 | HTTP/WebSocket server (used by dashboard) |
| `bitvec` | | Efficient bitwise candidate tracking |

### No Rust equivalents exist for

- GWNUM (IBDWT modular multiplication) — no Rust bindings, FFI wrapper needed
- LLR/LLR2 (Lucas-Lehmer-Riesel tester) — LLR2 deprecated, PRST is successor
- PFGW (PrimeForm/GW general tester)
- srsieve/sr2sieve/mtsieve (BSGS sieving framework)
- GpuOwl/PRPLL (GPU primality testing)

---

## External Tool Versions (as of Feb 2026)

| Tool | Version | Notes |
|------|---------|-------|
| GMP | 6.3.0 | Stable, no new release |
| FLINT | 3.4.0 (Nov 2025) | New `mpn_mod`, `nfloat`, `fft_small` requires AVX2/NEON |
| PRST | v13.3 (Jan 2025) | Uses GWNum 31.03. Successor to LLR2 (deprecated) |
| mtsieve | 2.6.9 (Jan 2026) | BSGS sieving framework |
| PFGW | 4.0.4 | General-purpose primality tester |
| Prime95/mprime | 30.19 | GIMPS client, GWNUM reference implementation |

---

## Competitive Analysis

For a deep-dive into GIMPS, PrimeGrid, and the broader prime-hunting ecosystem —
including the technical gaps between darkreach and state of the art, and a phased
catch-up roadmap — see **[competitive-analysis.md](competitive-analysis.md)**.

---

## References

- [GIMPS - The Math](https://www.mersenne.org/various/math.php)
- [GIMPS - PRP Proofs](https://www.mersenne.org/various/works.php)
- [PrimeGrid Wiki](https://primegrid.fandom.com/wiki/PrimeGrid_Wiki)
- [mtsieve Framework](https://www.mersenneforum.org/rogue/mtsieve.html)
- [PFGW - Prime-Wiki](https://www.rieselprime.de/ziki/PFGW)
- [LLR - Prime-Wiki](https://www.rieselprime.de/ziki/LLR)
- [PRST (GitHub)](https://github.com/patnashev/prst)
- [GpuOwl / PRPLL (GitHub)](https://github.com/preda/gpuowl)
- [FLINT](https://flintlib.org/)
- [IBDWT - Wikipedia](https://en.wikipedia.org/wiki/Irrational_base_discrete_weighted_transform)
- [Gerbicz Error Checking - Prime-Wiki](https://www.rieselprime.de/ziki/Gerbicz_error_checking)
- [Caldwell & Gallot - On the primality of n! +/- 1](https://www.ams.org/journals/mcom/2002-71-237/S0025-5718-01-01315-1/)
- [GMP Algorithms](https://gmplib.org/manual/Algorithms)
- [y-cruncher Multiplication Internals](https://www.numberworld.org/y-cruncher/internals/multiplication.html)
- [Proth's Theorem - Wikipedia](https://en.wikipedia.org/wiki/Proth%27s_theorem)
- [Lucas-Lehmer-Riesel Test - Wikipedia](https://en.wikipedia.org/wiki/Lucas%E2%80%93Lehmer%E2%80%93Riesel_test)
- [Pocklington Primality Test - Wikipedia](https://en.wikipedia.org/wiki/Pocklington_primality_test)
- [gmprime - GMP LLR Reference (GitHub)](https://github.com/arcetri/gmprime)
- [RPT - Zig LLR Tester (GitHub)](https://github.com/dkull/rpt)
- [srsieve Source (GitHub)](https://github.com/xayahrainie4793/prime-programs-cached-copy)
- [mtsieve (GitHub)](https://github.com/primesearch/mtsieve)
- [GMP-ECM Documentation](https://members.loria.fr/PZimmermann/records/ecm/params.html)
- [ecm Rust Crate (GitHub)](https://github.com/skyf0l/ecm-rs)
- [discrete-logarithm Crate](https://crates.io/crates/discrete-logarithm)
- [primesieve Library](https://github.com/kimwalisch/primesieve)
- [Deterministic Primality Proving on Proth Numbers](https://arxiv.org/pdf/0812.2596)
- [Bigint Benchmark Rust (GitHub)](https://github.com/tczajka/bigint-benchmark-rs)
- [Malachite Performance](https://www.malachite.rs/performance/)
- [Baillie-Fiori-Wagstaff - Strengthening BPSW](https://arxiv.org/abs/2006.14425)
- [SuperBFPSW - Hamburg](https://eprint.iacr.org/2025/2083)
- [Pell's Cubic Primality Test](https://arxiv.org/abs/2411.01638)
- [Circulant Matrix Primality Test](https://arxiv.org/abs/2505.00730)
- [Frobenius Test GMP Implementation (GitHub)](https://github.com/yzhs/frobenius-test)
- [Edwards Curve ECM](https://eecm.cr.yp.to/)
- [FastECPP over MPI - Enge](https://arxiv.org/html/2404.05506)
- [CM Software (Enge)](https://www.multiprecision.org/cm/)
- [Batch GCD - Bernstein](https://facthacks.cr.yp.to/batchgcd.html)
- [Certifying Giant Nonprimes](https://eprint.iacr.org/2023/238)
- [Montgomery Multiplication](https://cp-algorithms.com/algebra/montgomery_multiplication.html)
- [Aurifeuillean Factorization](https://mathworld.wolfram.com/AurifeuilleanFactorization.html)
- [Dickman Function](https://encyclopediaofmath.org/wiki/Dickman_function)
- [Roaring Bitmaps](https://roaringbitmap.org/about/)
- [primesieve Algorithms (GitHub)](https://github.com/kimwalisch/primesieve/blob/master/doc/ALGORITHMS.md)
- [t5k.org Submission Page](https://t5k.org/bios/submission.php)
- [t5k.org Top 20 Index](https://t5k.org/top20/)
- [OEIS Contribution Overview](https://oeis.org/wiki/Overview_of_the_contribution_process)
- [Conjectures 'R Us - Sierpinski](http://www.noprimeleftbehind.net/crus/Sierp-conjectures.htm)
- [Conjectures 'R Us - Riesel](http://www.noprimeleftbehind.net/crus/Riesel-conjectures.htm)
- [Cullen Number - Wikipedia](https://en.wikipedia.org/wiki/Cullen_number)
- [Wagstaff Prime - Wikipedia](https://en.wikipedia.org/wiki/Wagstaff_prime)
- [Primorial Prime - Wikipedia](https://en.wikipedia.org/wiki/Primorial_prime)
- [Repunit - Wikipedia](https://en.wikipedia.org/wiki/Repunit)
- [Carol-Kynea Prime - Prime-Wiki](https://www.rieselprime.de/ziki/Carol-Kynea_prime)
- [Twin Prime Search - Wikipedia](https://en.wikipedia.org/wiki/Twin_Prime_Search)
- [Sophie Germain Primes - Wikipedia](https://en.wikipedia.org/wiki/Safe_and_Sophie_Germain_primes)
- [Pollard P-1 Algorithm - Wikipedia](https://en.wikipedia.org/wiki/Pollard%27s_p_%E2%88%921_algorithm)
- [Lenstra ECM - Wikipedia](https://en.wikipedia.org/wiki/Lenstra_elliptic-curve_factorization)
