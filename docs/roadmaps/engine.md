# Engine Roadmap

Prime-hunting algorithms, sieving, and primality testing improvements.

**Key files:** `src/{factorial,palindromic,kbn,sieve,lib}.rs`

---

## Tier 1: Quick Wins (high impact, low effort)

### 1.1 Increase sieve limits

**Current:** `SIEVE_LIMIT = 1,000,000` (~78,498 primes) in `src/sieve.rs`.

**Target:** Configurable sieve limit, defaulting to 10,000,000 (~664,579 primes) or higher.

**Rationale:** Each additional sieve prime costs O(1) per step to maintain but can eliminate a candidate that would otherwise require an expensive primality test (hours or days for large numbers). Production projects sieve to 10^8 or beyond. The marginal cost of maintaining more sieve primes is negligible compared to even one avoided Miller-Rabin test on a million-digit number.

**Applies to:** factorial, kbn, palindromic searches.

### 1.2 Use GMP's built-in factorial for precomputation

**Current:** `src/factorial.rs` computes the initial factorial with a manual loop:
```rust
for i in 2..resume_from {
    factorial *= i;
}
```

**Target:** Use `Integer::factorial(n)` which internally uses binary splitting (balanced multiplies enabling Karatsuba/FFT) and a prime-sieve decomposition, both significantly faster than sequential Nx1 multiplies.

**Rationale:** GMP's factorial algorithm achieves O(n log^2 n M(n)) versus O(n M(n)) for the naive loop, where M(n) is multiplication cost. The difference is dramatic for large starting values.

### 1.3 Deepen palindromic sieve

**Current:** `has_small_factor()` in `src/lib.rs` trial-divides by 64 primes up to 311.

**Target:** Sieve palindromic candidates against thousands of primes (up to 10^6+) using modular arithmetic on the half-value representation, avoiding construction of the full big integer for candidates with small factors.

**Rationale:** ~88% of composite numbers have a prime factor below 100. With only 64 primes, many composites slip through to the expensive PRP test. Deeper sieving on the compact half-value representation is cheap.

### 1.4 Wilson's theorem pre-filter for factorial primes

**Current:** No special handling when n+1 is prime.

**Target:** Skip testing n!+1 when n+1 is prime (Wilson's theorem guarantees (n+1) | (n!+1) when n+1 is prime and n > 2).

**Rationale:** Free elimination of ~n/ln(n) candidates. Trivial to implement.

### 1.5 Reduce Miller-Rabin rounds for screening

**Current:** 25 rounds of Miller-Rabin for every candidate (`mr_rounds = 25`).

**Target:** Use 2 rounds for initial screening (false positive rate < 6.25 * 10^-4), then 25 rounds only for probable primes. Or use Baillie-PSW (Miller-Rabin base 2 + strong Lucas), which has no known pseudoprimes.

**Rationale:** Each Miller-Rabin round requires a full modular exponentiation. For large numbers, reducing from 25 to 2 rounds for composite rejection gives ~12x speedup on the testing phase for composites (which are the vast majority of candidates).

---

## Tier 2: Algorithmic Upgrades (high impact, medium effort)

### 2.1 Proth's theorem for k*2^n + 1

**Current:** Generic Miller-Rabin via `is_probably_prime(25)`.

**Target:** For base=2, k odd, k < 2^n: implement Proth's test. Find a quadratic non-residue `a` (check Jacobi symbol), compute `a^((N-1)/2) mod N`. If result is N-1, the number is **proven prime** with a single exponentiation.

**Algorithm:**
```
1. Verify k < 2^n (Proth number condition)
2. For a in [3, 5, 7, 11, 13, ...]:
     if jacobi(a, N) == -1:
       if pow(a, (N-1)/2, N) == N-1: return PRIME
       else: return COMPOSITE
```

**Rationale:** Replaces 25 probabilistic exponentiations with 1 deterministic one. Single biggest algorithmic win for base-2 +1 searches. Used by PrimeGrid's Proth Prime Search.

### 2.2 Lucas-Lehmer-Riesel test for k*2^n - 1

**Current:** Generic Miller-Rabin via `is_probably_prime(25)`.

**Target:** For base=2, k odd, k < 2^n: implement the LLR test. Find initial value u_0 via Lucas sequences, then iterate `u_i = u_{i-1}^2 - 2 (mod N)` for n-2 steps. Prime iff u_{n-2} = 0.

**Algorithm:**
```
1. Verify k < 2^n (Riesel number condition)
2. Find starting value u_0:
     For P in [5, 8, 9, 11, ...]:
       D = P^2 - 4
       if jacobi(D, N) == -1:
         u_0 = lucas_V(P, k) mod N
         break
3. For i in 1..=(n-2):
     u_i = u_{i-1}^2 - 2 (mod N)
4. Return PRIME if u_{n-2} == 0, else COMPOSITE
```

**Rationale:** Deterministic primality proof with n-2 squarings (vs 25 full exponentiations). Used by Jean Penne's LLR program and PrimeGrid. Includes the classic Lucas-Lehmer test for Mersenne numbers (k=1) as a special case.

### 2.3 Baby-Step Giant-Step sieving for kbn

**Current:** `sieve_block()` in `src/kbn.rs` iterates all sieve primes for every n in the block — O(num_primes * block_size).

**Target:** Implement BSGS discrete logarithm sieving. For each prime p, solve `b^n = target (mod p)` to find exactly which n-values are eliminated. Total cost: O(num_primes * sqrt(p)).

**Algorithm:**
```
For each sieve prime p:
  target_plus  = (-k^{-1}) mod p    // for k*b^n + 1
  target_minus = (k^{-1}) mod p     // for k*b^n - 1

  // Baby-step giant-step to solve b^n = target (mod p)
  M = ceil(sqrt(ord_p(b)))
  baby_table = { b^j mod p : 0 <= j < M }
  giant_step = b^{-M} mod p

  For i = 0, 1, 2, ...:
    if target * giant_step^i in baby_table:
      n_hit = i*M + j
      // All n = n_hit + k*ord_p(b) are eliminated
      mark_composite(n_hit, ord_p(b), n_range)
```

**Rationale:** This is the algorithm used by srsieve, sr2sieve, and the mtsieve framework. It scales much better than the current approach, especially with deeper sieve limits. For p ~ 10^6 and block_size ~ 10^4, BSGS is ~100x more efficient.

### 2.4 Near-repdigit palindrome search mode

**Current:** Exhaustive search over all half-values of palindromes.

**Target:** Add a dedicated search mode for near-repdigit palindromic primes of the form:
```
10^(2k+1) - d * 10^(k+m) - d * 10^(k-m) - 1
```
where the candidate is parameterized by (digit_count, offset_position, modification_value).

**Rationale:** This is how all record palindromic primes are found (Propper & Batalov, 2.7M digits, August 2024). The search space collapses from ~10^(n/2) half-values to a polynomial number of (offset, modification) pairs. Additionally:
- N+1 involves powers of 10 with trivially known factorizations
- Enables BLS deterministic primality proofs
- Modular sieving reduces to solving quadratic equations over Z/pZ

### 2.5 Pocklington proof for n! + 1

**Current:** Probabilistic Miller-Rabin result classified as "probabilistic".

**Target:** When `is_probably_prime` returns true for n!+1, run a Pocklington N-1 proof. Since N-1 = n! has a completely known factorization (product of all primes up to n), the proof is straightforward.

**Algorithm:**
```
For each prime q <= n:
  Find witness a such that:
    1. a^(N-1) = 1 (mod N)
    2. gcd(a^((N-1)/q) - 1, N) = 1
  If no witness found for some q: N is composite
If all primes verified: N is PROVEN prime
```

**Rationale:** Upgrades results from "probabilistic" to "deterministic" at moderate computational cost. The factorization of n! is free. Used by PRST and PFGW for factorial prime verification.

### 2.6 Morrison proof for n! - 1

**Current:** Same probabilistic approach as n!+1.

**Target:** When PRP found for n!-1, use Morrison's N+1 theorem with Lucas sequences, since N+1 = n! has known factorization.

**Rationale:** Complements the Pocklington proof for the -1 form. Implemented by PRST.

---

## GWNUM/FLINT Integration (very high impact, high effort)

> **CRITICAL PATH ITEM.** See [`gwnum-flint.md`](gwnum-flint.md) for the full integration guide, architecture decisions, API references, and deployment checklist.

**Status:**
- **Phase 0 (PFGW subprocess):** Complete — `src/pfgw.rs` provides 50-100x acceleration for all 10 forms via PFGW subprocess. CLI flags: `--pfgw-min-digits`, `--pfgw-path`. Atomic temp file counter prevents rayon thread collisions.
- **Phase 1 (FLINT):** Complete — `src/flint.rs` with `--features flint` for 3-10x faster factorial/primorial computation via SIMD NTTs. Works on Apple Silicon (NEON).
- **Phase 2 (Extended PRST):** Complete — `src/prst.rs` routes k*b^n±1 forms through PRST subprocess. Gerbicz error checking integrated.
- **Phase 3 (GWNUM FFI):** Complete — `gwnum-sys/` workspace crate + `src/gwnum.rs` safe wrapper. Vrba-Reix test for Wagstaff, accelerated Proth/LLR with Gerbicz checking. x86-64 only.
- **Phase 4 (Hardening):** Complete — 3-tier cross-verification pipeline (tier 1: deterministic proof, tier 2: BPSW+MR10, tier 3: PFGW independent tool). PFGW acceleration added to cullen_woodall, carol_kynea, gen_fermat, repunit forms.

**Tool selection by form:**
| Form | Primary tool | Fallback |
|------|-------------|----------|
| kbn k*b^n±1 | GWNUM direct / PRST | GMP Proth/LLR |
| Factorial n!±1 | PFGW -tp/-tm | GMP MR + Pocklington/Morrison |
| Primorial p#±1 | PFGW -tp/-tm | GMP MR + Pocklington/Morrison |
| Wagstaff (2^p+1)/3 | GWNUM Vrba-Reix / PFGW | GMP MR |
| Palindromic | PFGW | GMP MR |
| Near-repdigit | PFGW PRP | GMP MR + BLS proof |
| Cullen/Woodall n·2^n±1 | PFGW PRP | GMP Proth/LLR |
| Carol/Kynea (2^n±1)²−2 | PFGW PRP | GMP LLR |
| Gen Fermat b^(2^n)+1 | PFGW PRP | GMP Pépin/Proth |
| Repunit (b^n−1)/(b−1) | PFGW PRP | GMP MR |

---

## Advanced Sieving Techniques

### Montgomery Multiplication for Sieve Primes

The current sieve uses u128 division for modular arithmetic, which is **35-90 cycles** per operation. Montgomery multiplication replaces this with **4-6 cycles**:

```rust
/// Montgomery multiplication: a * b * R^(-1) mod n
/// where R = 2^64, n' = -n^(-1) mod R (precomputed)
fn mont_mul(a: u64, b: u64, n: u64, n_prime: u64) -> u64 {
    let t: u128 = (a as u128) * (b as u128);
    let m: u64 = (t as u64).wrapping_mul(n_prime);
    let u: u128 = t + (m as u128) * (n as u128);
    let result = (u >> 64) as u64;
    if result >= n { result - n } else { result }
}
```

**6-20x speedup** over division-based approach for primes above 2^32.

### Multi-Stage Sieving Pipeline

```
Stage 1: Trial division (p up to 10^6-10^9)
  Cost: O(1) per prime per candidate
  Eliminates: ~85-95% of candidates

Stage 2: BSGS sieving (p up to 10^9-10^15)
  Cost: O(sqrt(N_range)) per prime (all candidates at once)
  Eliminates: Additional ~50-80% of remaining

Stage 3: P-1 factoring (factors up to ~2^80-2^120)
  Cost: One modular exponentiation per candidate
  Eliminates: ~1-3% of remaining (GIMPS data)

Stage 4: ECM (factors up to ~50-60 digits)
  Cost: Multiple curves per candidate
  Eliminates: Additional ~1-5% (selectively applied)

Stage 5: PRP/primality testing
  Cost: Full modular exponentiation
  Applied to: Survivors only (~0.1-2% of original candidates)
```

### Algebraic Factorizations

Many special forms have guaranteed algebraic factors:

- **Even-digit palindromes (base b)**: Always divisible by b+1 (already handled in primehunt)
- **Aurifeuillean factorizations**: For specific (base, exponent) pairs, cyclotomic polynomials factor further. Example base 2: 2^(4k-2)+1 = (2^(2k-1)-2^k+1)(2^(2k-1)+2^k+1)
- **Sophie Germain identity**: a^4+4b^4 = (a^2-2ab+2b^2)(a^2+2ab+2b^2)
- **General condition**: For b = s^2*t (t square-free), Aurifeuillean factorizations exist when t=1 (mod 4) and n=t (mod 2t)

### Wheel Factorization

| Wheel | Modulus | Fraction of candidates kept |
|-------|---------|---------------------------|
| W3 | 30 (2*3*5) | 26.7% |
| W4 | 210 (2*3*5*7) | 22.9% |
| W5 | 2310 (2*3*5*7*11) | 20.8% |

### Sieve Depth Optimization

**Fundamental crossover condition**: Continue sieving while
```
T_sieve(p) < T_test / p
```

For BSGS: T_bsgs(p) costs O(sqrt(N_range)) per prime.

**Candidate survival rates by sieve depth** (Mertens' theorem: ~0.5615/ln(P)):

| Sieve depth | Survival rate | Per 1M n-values |
|---|---|---|
| 10^6 (current) | ~4.1% | 41,000 |
| 10^7 | ~3.5% | 35,000 |
| 10^8 | ~3.1% | 31,000 |
| 10^9 | ~2.7% | 27,000 |
| 10^12 | ~2.0% | 20,000 |

### Candidate Data Structures at Scale

| Structure | When to use | Memory for 10B candidates |
|-----------|------------|--------------------------|
| Bitmap | During sieve phase | ~1.2 GB |
| Sorted Vec<u64> | After sieve (sparse) | 8 bytes x survivors |
| Roaring bitmap | Adaptive hybrid | Best of both |

Transition from bitmap to sorted list when survival rate drops below ~1/512.

---

## Cutting-Edge Mathematical Techniques (2023-2026)

### Strengthened BPSW (Baillie-Fiori-Wagstaff, 2021)

Adds **Lucas-V pseudoprime** check — an additional congruence on the V-sequence at essentially zero cost since V values are already computed. Only 5 Lucas-V pseudoprimes below 10^15.

**Extra strong Lucas test** (Q=1 variant): composites pass for at most **1/8** of bases (vs 4/15 for strong Lucas, 1/4 for Miller-Rabin). Runs in ~2/3 the time of strong Lucas-Selfridge.

**Note:** GMP 6.2.0+ already uses BPSW in `mpz_probab_prime_p`. primehunt's `is_probably_prime(25)` gets BPSW + 1 extra MR round automatically.

### SuperBFPSW (November 2025)

Uses Montgomery-like ladder for V_n(P,1) computation. **Stronger than Baillie-Fiori-Wagstaff but faster than original BPSW.** Worth monitoring and implementing once peer-reviewed.

### Novel Primality Tests

- **Pell's Cubic Test (Nov 2024, published 2025):** Works over Pell's cubic C_p, using third-order linear recurrences. Deterministic for n < 2^36. O(log n) complexity. Published in Mediterranean Journal of Mathematics (2025).
- **Circulant Matrix Eigenvalue Test (Apr 2025):** Deterministic O~(log^6 n).

### Frobenius Tests

**Grantham's RQFT**: False positive probability < 1/7710 per round (vs Miller-Rabin's 1/4). Costs ~3x one MR round.

### Edwards Curve ECM

Edwards curves (x^2+y^2=1+dx^2y^2) with a=-1 twisted coordinates achieve:
- **Point addition**: 8M (vs 9M+1S for prior formulas)
- **Point doubling**: 4M + 4S
- **Stage 1**: 7.6-8.8 M/bit vs GMP-ECM's ~9 M/bit

### Batch GCD and Product Trees

**Bernstein's algorithm** for batch smoothness detection: given C candidates and a primorial P, compute the P-smooth part of each candidate simultaneously in O(C * (log C)^(2+o(1))) bit operations.

**Application:** Replace sequential trial division with tree-based parallel approach for the sieve phase.

---

## New Prime Forms

Ranked by fit with the existing primehunt architecture (Rust + rug/GMP + rayon).

### Tier A: Trivial to add (reuse existing infrastructure)

**Primorial primes (p# +/- 1)** — Nearly identical to factorial search. ~150-200 lines.

**Cullen primes (n * 2^n + 1)** — Special case of Proth numbers. ~30 lines as kbn wrapper.

### Tier B: Moderate effort (requires LLR or new algorithms)

**Woodall primes (n * 2^n - 1)** — Requires LLR. ~300-350 lines.

**Wagstaff primes ((2^p + 1) / 3)** — No active project, unique niche. ~200-250 lines.

**Carol/Kynea primes** — Carol: LLR deterministic. Kynea: Miller-Rabin only. ~250-300 lines.

**Repunit primes ((10^n - 1) / 9)** — No specialized deterministic test. ~200-250 lines.

### Tier C: Requires LLR or specialized infrastructure

**Twin primes** — Needs Proth + LLR + quad sieve. ~400-450 lines.

**Sophie Germain primes** — Shares twin prime infrastructure. ~350-400 lines.

**Generalized Fermat primes (b^(2^n) + 1)** — GPU-preferred for frontier. ~350-400 lines.

### Recommended addition order

```
1. Primorial primes       (trivial port of factorial.rs)
2. Cullen primes          (wrapper around kbn with k=n)
3. Woodall primes         (needs LLR — shared with kbn -1 improvement)
4. Wagstaff primes        (unique niche, no active project)
5. Carol/Kynea primes     (shares LLR from Woodall)
6. Twin primes            (needs Proth + LLR, quad sieve)
7. Sophie Germain primes  (shares twin prime infrastructure)
8. Repunit primes         (straightforward, no deterministic test)
9. Generalized Fermat     (GPU-preferred for frontier)
```

---

## Implementation Pseudocode

### Proth test with rug

```rust
fn proth_test(n: &Integer) -> Option<bool> {
    let n_minus_1 = Integer::from(n - 1u32);
    let exp = Integer::from(&n_minus_1 >> 1u32);  // (N-1)/2

    for &a in &[3u32, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41] {
        let a_int = Integer::from(a);
        let j = a_int.jacobi(n);

        if j == 0 { return Some(false); }
        if j == 1 { continue; }

        // j == -1: quadratic non-residue, test is now deterministic
        match a_int.pow_mod(&exp, n) {
            Ok(result) if result == n_minus_1 => return Some(true),
            Ok(_) => return Some(false),
            Err(_) => return Some(false),
        }
    }
    None  // no QNR found (probability ~2^-12)
}
```

### LLR test with rug

```rust
fn llr_test(k: u64, n: u64) -> bool {
    let mut nn = Integer::from(k);
    nn <<= n as u32;
    nn -= 1;  // N = k * 2^n - 1

    let v1: u32 = if k % 3 != 0 { 4 } else { find_rodseth_v1(&nn) };
    let mut u = lucas_v_k(k, v1, &nn);

    for _ in 0..(n - 2) {
        u.square_mut();
        u -= 2u32;
        u.rem_euc_assign(&nn);
    }

    u == 0u32
}
```

### BSGS sieve speedup estimates

| Range (n-values) | Current O(primes*range) | BSGS O(primes*sqrt(range)) | Speedup |
|---|---|---|---|
| 1,000 | 78M ops | 2.5M ops | ~31x |
| 10,000 | 785M ops | 7.8M ops | ~100x |
| 100,000 | 7.8B ops | 25M ops | ~315x |
| 1,000,000 | 78B ops | 78M ops | ~1,000x |

---

## Performance Comparison: Current vs Optimized

| Metric | Current primehunt | Optimized (Tier 1+2) | Optimized (All tiers) |
|--------|-------------------|----------------------|-----------------------|
| Sieve depth | 78K primes | 664K+ primes | Auto-tuned, 10^8+ |
| Sieve algorithm (kbn) | O(primes * block) | O(primes * sqrt(p)) via BSGS | GPU-accelerated BSGS |
| Primality test (base-2) | 25x Miller-Rabin | 1x Proth/LLR (deterministic) | GWNUM-accelerated Proth/LLR |
| Primality test (general) | 25x Miller-Rabin | 2x screen + 25x confirm | FLINT/GWNUM-accelerated |
| Factorial precompute | Sequential Nx1 multiply | GMP binary-split factorial | Same |
| Palindrome search | Exhaustive half-values | Near-repdigit parameterized | + algebraic sieving |
| Error detection | None | None | Gerbicz checking |
| Result quality | Probabilistic | Deterministic (base-2) | + verifiable certificates |
| Arithmetic library | GMP (~1x) | GMP (~1x) | GWNUM (~10-100x for large n) |
