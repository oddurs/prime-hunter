# Technical Reference

Implementation details, architecture patterns, sieving techniques, and mathematical frontiers.

---

## Rust Ecosystem

### Arithmetic Libraries

| Library | vs GMP speed | Primality testing | License |
|---|---|---|---|
| **rug** (GMP) | 1.0x (reference) | Baillie-PSW + MR | LGPL |
| **malachite** | 0.55-0.75x | None built-in | LGPL |
| **num-bigint** | 0.004-0.15x | Via num-primes | MIT |

Benchmark: 10M digits of e -- rug: 2.8s, malachite: 3.7s, num-bigint: 482s. No Rust crate approaches GMP for million-digit arithmetic.

### Useful Crates

| Crate | Use |
|---|---|
| `rug` 1.28 | All big-integer arithmetic |
| `machine-prime` | Deterministic u64/u128 primality |
| `primesieve-sys` | Fast prime generation (Kim Walisch) |
| `ecm` 1.0.1 | ECM factoring with rug backend |
| `bitvec` | Efficient candidate tracking |

No Rust equivalents exist for LLR, PFGW, srsieve/mtsieve, GWNUM, or GpuOwl.

---

## Architecture Patterns

### FFT Memory Management (Prime95/gwnum)

- **Pooled allocator:** Cache up to 10 freed gwnums for instant reuse
- **128-byte SIMD alignment:** All data aligned for AVX-512
- **Sin/cos table sharing:** Multiple handles share FFT twiddle tables (hundreds of MB)
- **Large page support:** 2MB pages reduce TLB pressure

### Multi-Threaded FFT (Prime95)

Two-pass 2D FFT (R x C matrix):
1. Pass 1: C-point FFTs on each row (fits L2/L3 cache)
2. Twiddle multiply (folded into IDBWT weights)
3. Pass 2: R-point FFTs on each column

Thread sync via atomic work-stealing counter -- no locks except carry propagation.

### Error Detection

| Method | Overhead | Detection Rate |
|---|---|---|
| Gerbicz-Li checking | ~0.1% | 99.999%+ |
| Jacobi symbol check | ~0.07% | ~50% per check |
| FFT round-off monitoring | 0% | Catches precision loss |
| Pietrzak PRP proofs | ~1-5% | Cryptographic certainty |

### Memory Patterns for rug

```rust
// BAD: allocates every iteration
for n in start..end {
    let candidate = Integer::from(k) * Integer::from(base).pow(exp);
}

// GOOD: pre-allocate and reuse
let mut candidate = Integer::with_capacity(bit_size);
for n in start..end {
    candidate.assign(k);
    candidate <<= exp;
    candidate += 1;
}
```

Pre-allocate all `Integer` objects before hot loops. Use `Integer::with_capacity(bits)`.

---

## Advanced Sieving

### mtsieve Framework

The [mtsieve framework](https://github.com/primesearch/mtsieve) is the gold standard: 16+ sieve implementations, dynamic chunk sizing (1-5s per chunk), OpenCL/Metal GPU backends, near-linear scaling.

### BSGS Sieving

Replace per-n iteration with per-prime discrete log:

```
For each sieve prime p:
  target = (-k^{-1}) mod p
  M = ceil(sqrt(ord_p(b)))
  baby_table = { b^j mod p : 0 <= j < M }
  Walk giant steps to find all n_hits
  Mark composite: n_hit + k*ord_p(b) for all k
```

### Montgomery Multiplication for Sieve Primes

Replace u128 division (~35-90 cycles) with Montgomery multiply (~4-6 cycles):

```rust
fn mont_mul(a: u64, b: u64, n: u64, n_prime: u64) -> u64 {
    let t: u128 = (a as u128) * (b as u128);
    let m: u64 = (t as u64).wrapping_mul(n_prime);
    let u: u128 = t + (m as u128) * (n as u128);
    let result = (u >> 64) as u64;
    if result >= n { result - n } else { result }
}
```

6-20x speedup for primes above 2^32.

### Candidate Survival Rates (Mertens' theorem: ~0.5615/ln(P))

| Sieve depth | Survival rate |
|---|---|
| 10^6 | ~4.1% |
| 10^7 | ~3.5% |
| 10^9 | ~2.7% |
| 10^12 | ~2.0% |

### Multi-Form Simultaneous Sieving

Computing k*b^n mod p serves both +1 and -1 for free (darkreach already does this). For twin primes, quad sieve gives 4/(q-1) elimination rate per prime. srsieve2 sieves multiple k values simultaneously -- b^n mod p is shared.

### Algebraic Factorizations

- **Even-digit palindromes:** Always divisible by b+1 (already handled)
- **Aurifeuillean factorizations:** For specific (base, exponent) pairs, cyclotomic polynomials factor further
- **Sophie Germain identity:** a^4+4b^4 = (a^2-2ab+2b^2)(a^2+2ab+2b^2)

---

## Mathematical Frontiers (2023-2026)

### Strengthened BPSW (Baillie-Fiori-Wagstaff, 2021)

Extra strong Lucas test: composites pass for at most 1/8 of bases (vs 1/4 for MR). No BPSW pseudoprime found. Verified to 2^64. GMP 6.2.0+ already uses BPSW in `mpz_probab_prime_p`.

### SuperBFPSW (Hamburg, Nov 2025)

Stronger than Baillie-Fiori-Wagstaff but faster than original BPSW. Montgomery-like ladder for V_n(P,1). Worth monitoring. [IACR ePrint 2025/2083](https://eprint.iacr.org/2025/2083)

### Novel Primality Tests

- **Pell's Cubic Test (Nov 2024):** Deterministic for n < 2^36. O(log n). [arXiv:2411.01638](https://arxiv.org/abs/2411.01638)
- **Circulant Matrix Test (Apr 2025):** Deterministic O~(log^6 n). [arXiv:2505.00730](https://arxiv.org/abs/2505.00730)
- **Grantham's RQFT:** False positive < 1/7710 per round (vs MR's 1/4). Costs ~3x one MR round.

### Edwards Curve ECM

Twisted Edwards curves achieve 8M point addition (vs 9M+1S). Torsion group Z/12Z forces orders divisible by 12, improving smooth-order probability. GMP-ECM 7.0.6 supports CUDA.

### FastECPP Records

86,453-digit proof: ~103 days, 383 CPU-years. 109,297-digit proof: May 2025. Software: [CM by Enge](https://www.multiprecision.org/cm/). Relevant for general-form numbers up to ~100K digits.

### Batch GCD (Bernstein)

Given C candidates and primorial P, compute P-smooth part of each simultaneously in O(C * (log C)^(2+o(1))). Replaces sequential trial division with tree-based parallel approach.
