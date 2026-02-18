# Optimization Roadmap

Tiers ranked by impact-to-effort ratio. Implementation order at the bottom.

---

## Tier 1: Quick Wins (all done)

### 1.1 Increase sieve limits

Default sieve bumped from 1M to 10M (~664K primes). Configurable via `--sieve-limit`.

### 1.2 GMP built-in factorial

`Integer::factorial()` uses binary splitting, O(n log^2 n M(n)) vs O(n M(n)) for sequential multiply.

### 1.3 Deepen palindromic sieve

Replaced 15 hardcoded filter primes (3-59) with full sieve set (~664K primes). Per-prime safety bounds avoid false positives on small candidates.

### 1.4 Wilson's theorem filter

When n+1 is prime and n > 2, skip n!+1 test since (n+1) | (n!+1). Free ~n/ln(n) eliminations.

### 1.5 Two-round MR pre-screening

2 fast MR rounds reject composites before full test. ~7x faster composite rejection.

---

## Tier 2: Algorithmic Upgrades

### 2.1 Proth's theorem (k*2^n + 1) -- done

Single modular exponentiation gives deterministic proof when k < 2^n.

### 2.2 Lucas-Lehmer-Riesel (k*2^n - 1) -- done

n-2 squarings for deterministic proof. Includes Rodseth starting value for k divisible by 3.

### 2.3 BSGS sieving for kbn

For each sieve prime p, solve b^n = target (mod p) via baby-step giant-step to find exactly which n-values are eliminated. Cost: O(primes * sqrt(p)) vs current O(primes * block_size).

| Range (n-values) | Current | BSGS | Speedup |
|---|---|---|---|
| 10,000 | 785M ops | 7.8M ops | ~100x |
| 1,000,000 | 78B ops | 78M ops | ~1,000x |

### 2.4 Near-repdigit palindrome search

Parameterize palindromes as 10^d - a*10^e - b*10^f - 1. Collapses search from ~10^(n/2) half-values to polynomial (offset, modification) pairs. N+1 has trivially known factorization enabling BLS proofs. This is how the 2.7M-digit record was found.

### 2.5 Pocklington proof (n! + 1)

N-1 = n! has fully known factorization. For each prime q <= n, find witness a with a^(N-1) = 1 (mod N) and gcd(a^((N-1)/q) - 1, N) = 1. Upgrades "probabilistic" to "deterministic."

### 2.6 Morrison proof (n! - 1)

N+1 = n! has known factorization. Lucas sequence N+1 test complements Pocklington for the -1 form.

---

## Tier 3: Performance Engineering

### 3.1 GWNUM integration

IBDWT folds modular reduction into FFT convolution, halving FFT size vs GMP. Hand-tuned x86 assembly.

| Number size | GWNUM advantage over GMP |
|---|---|
| ~3,000 digits | ~2x |
| ~30,000 digits | ~5-10x |
| ~300,000 digits | ~50-100x |

Alternative: shell out to PRST/PFGW as subprocess for the primality test step.

### 3.2 FLINT integration

FLINT 3's small-prime FFT with SIMD-vectorized NTTs: 7-10x faster than GMP above ~10K digits. BSD-licensed. Multithreaded. Use for forms where GWNUM's IBDWT doesn't apply.

### 3.3 Gerbicz error checking

Checksum sequence verified every L^2 iterations (~4M). Catches bit-flip errors at ~0.1% overhead. Essential for multi-day tests.

### 3.4 GPU-accelerated sieving

BSGS is embarrassingly parallel. Precedent: mtsieve's `srsieve2cl` (OpenCL). Pushes sieve depth to 10^12+.

### 3.5 Sieve depth auto-tuning

Stop sieving when `sieve_cost_per_candidate_removed > primality_test_cost * P(factor in next range)`. Compute at runtime based on candidate size and hardware speed.

---

## Tier 4: Architectural

### 4.1 Multi-stage pipeline

Deep sieve (~95% eliminated) -> 2-round MR screen -> full PRP -> deterministic proof.

### 4.2 Checkpoint hardening

Multiple backup generations, integrity checksums, per-search-type files.

### 4.3 P-1 factoring for kbn

Finds factors up to ~2^80-2^120 missed by trial division. GIMPS eliminates ~1-3% this way.

### 4.4 Distributed search coordination

Work-unit server extending the Axum dashboard. Assigns ranges, collects results, avoids duplicates.

### 4.5 Primality certificates

Proth/LLR certificates, Pocklington/Morrison witnesses, Pietrzak PRP proofs (verifiable in 0.5% of test time). Required for t5k.org.

---

## Current vs Optimized

| Metric | Current | Tier 1+2 | All tiers |
|---|---|---|---|
| Sieve depth | 664K primes | 664K+ | Auto-tuned, 10^8+ |
| Sieve algorithm (kbn) | O(primes * block) | O(primes * sqrt(p)) | GPU BSGS |
| Primality (base-2) | MR screen + full | Proth/LLR (deterministic) | GWNUM-accelerated |
| Palindrome search | Exhaustive half-values | Near-repdigit | + algebraic sieving |
| Error detection | None | None | Gerbicz checking |
| Result quality | Probabilistic | Deterministic (base-2) | + certificates |
| Arithmetic | GMP (~1x) | GMP (~1x) | GWNUM (~10-100x) |

## Implementation Order

```
Phase 1 (done):   1.1-1.5 Quick wins
Phase 2 (done):   2.1-2.2 Proth + LLR
Phase 3 (next):   2.3 BSGS sieving, 2.5-2.6 Pocklington/Morrison proofs
Phase 4:          2.4 Near-repdigit palindromes, 4.1 Pipeline
Phase 5:          3.1 GWNUM or PRST subprocess, 3.2 FLINT, 3.5 Auto-tuning
Phase 6:          3.3 Gerbicz, 4.2 Checkpoints, 4.3 P-1, 4.5 Certificates
Phase 7:          3.4 GPU sieving, 4.4 Distributed coordination
```
