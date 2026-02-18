# Search Strategy

Where to hunt primes for maximum discovery potential, and platform-specific optimization.

---

## Tier 1: Highest-Value Targets

### Wagstaff primes ((2^p+1)/3) -- NO active searcher

- 9.3M exponent gap between p=4,031,399 and p=13,347,311 is NOT exhaustively searched
- Range above p=15,135,397 completely open
- No organized project (PrimeGrid does not cover Wagstaff)
- PRP test at p~5M (~1.5M digits): 2-6 hours per candidate
- A discovery gets published and recognized immediately

### k*b^n+/-1 non-base-2 conjectures -- HUNDREDS of open targets

- Conjectures 'R Us tracks bases 2-1030+, many with only 1-3 remaining k values
- Many bases have search limits below n=3,000
- Each prime found SOLVES A CONJECTURE -- publishable result
- No organized project for most bases

### Factorial primes (n!+1) -- lags n!-1 by 210K

- n!-1 record at n=632,760; n!+1 at n=422,429
- PrimeGrid active but +1 backlog may be partially covered

## Tier 2: Good Targets

- **Primorial primes (p#-1):** p#-1 record lags p#+1 by 3M primes
- **Twin primes:** Record stale since 2016 (388K digits). PrimeGrid's search ended. Zero competition.

## Tier 3: Niche

- **Palindromic primes (non-base-10):** Almost completely unexplored for bases 3-9+
- **Multi-factorial primes:** Low search limits, retesting needed

## What NOT to Target

Mersenne (GIMPS), Sierpinski/Riesel base 2 (PrimeGrid), Cullen/Woodall (PrimeGrid), Generalized Fermat (PrimeGrid, GPU-preferred), Wieferich (exhausted to 2^64).

---

## Record-Breaking Strategies

### Strategy 1: Non-Base-2 Sierpinski/Riesel (BEST ROI)

Pick bases where 1-3 k-values remain and search limits are low (< n=500K). Extending to n=1M-2M has ~40% chance per k-value. Tools: mtsieve/srsieve2 for sieving, LLR/PRST for testing. **Timeline: weeks to months.**

### Strategy 2: Palindromic Prime Record (HIGH VISIBILITY)

Current record: 2,718,281 digits (Propper & Batalov, Aug 2024). Near-repdigit construction with BLS proof. Target d=3,000,001+. Per candidate: ~2-3 weeks/core at 3M digits. **Timeline: 1-3 years.**

### Strategy 3: Wagstaff PRP (NO COMPETITION)

Vrba-Reix test with modular reduction nearly free (same trick as Mersenne). ~3 days/core at p=5M, ~4 weeks at p=15M. Limitation: PRPs cannot be proven. **Timeline: months.**

### Strategy 4: Factorial Primes (AUTO-PROVABLE)

No sieving possible. Proof is free (Pocklington for +1, Morrison for -1). ~12-14 months/core at n~700K. **Timeline: 3-5 years.**

### ROI Comparison

| Search Type | Core-years per discovery | Provable? | Competition |
|---|---|---|---|
| Sierpinski/Riesel (non-base-2) | **1-10** | Yes | Low |
| Palindromic record | **100-1,000** | Yes (BLS) | 1 team |
| Factorial | **~2,300** | Yes | PrimeGrid |
| Wagstaff PRP | **~3,000** | No | None |
| Primorial | **~140,000** | Yes | Not viable solo |

---

## The t5k.org Top 5000

- Current minimum: **~825K digits** (growing ~50K/year)
- Special forms have their own Top 20 lists with lower thresholds
- Any prime in a recognized archivable form gets listed regardless of size
- **Only PROVEN primes accepted. PRPs are rejected.**
- 51 archivable form categories including: Factorial (#9), Palindrome (#39), Wagstaff (#50), Twin (#48)

---

## Apple Silicon Optimization

### GMP Performance

Apple M1 ranks #6 on GMPbench at 3.2 GHz. `mpn_mul_1`: 1.0 cycles/limb, `mpn_addmul_1`: 1.25 c/l.

### Build Configuration

```toml
# .cargo/config.toml
[target.aarch64-apple-darwin]
rustflags = ["-C", "target-cpu=apple-m1"]
```

**Do NOT use `-Ctarget-cpu=native`** -- Rust bug #93889 resolves it to `cyclone` (2013 chip), producing worse code.

### Quick Wins

| Action | Impact |
|---|---|
| Use `apple-m1` target CPU | Correct codegen |
| Set Rayon thread QoS to `user-initiated` | P-core scheduling |
| Add `mimalloc` as global allocator | ~10x faster large allocations |
| Profile-Guided Optimization (PGO) | 8-12% improvement |

### Not Worth Pursuing

- **AMX coprocessor:** Wrong operand widths, undocumented
- **vDSP/Accelerate FFT:** Float-only, not exact arithmetic
- **Metal GPU sieving:** 64-bit multiply is 8 cycles, 8KB L1 cache
- **Prime95:** Not optimized for ARM

### FLINT on Apple Silicon

FLINT 3's small-prime FFT supports ARM NEON. For numbers above ~10K digits, 3-10x faster than GMP (single to 8-core). Most promising library upgrade path.
