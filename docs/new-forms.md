# New Prime Forms

Ranked by fit with the existing architecture (Rust + rug/GMP + rayon).

---

## Tier A: Trivial to Add

### Primorial primes (p# +/- 1)

Product of all primes up to p, plus or minus 1. Nearly identical to factorial search -- multiply by next prime instead of next integer. The `FactorialSieve` ports directly.

- **Records:** p#+1: 9,562,633# + 1 (~4.15M digits). p#-1: 6,533,299# - 1 (~2.84M digits).
- **Test:** Miller-Rabin. N-1 factorization not useful because p#-1 is not easily factorable.
- **Code estimate:** ~150-200 lines (adapt `factorial.rs`).
- **OEIS:** [A005234](https://oeis.org/A005234) (p#+1), [A014545](https://oeis.org/A014545) (p#-1).

### Cullen primes (n * 2^n + 1)

Special case of Proth numbers with k = n. Proth's theorem always applies since k < 2^n for n >= 1.

- **Records:** C(6,679,881) ~2.01M digits. Only 16 known.
- **Test:** Proth's theorem (already implemented).
- **Code estimate:** ~30 lines as kbn wrapper with k=n, or ~150 lines standalone.
- **OEIS:** [A005849](https://oeis.org/A005849).

## Tier B: Moderate Effort

### Woodall primes (n * 2^n - 1)

The -1 counterpart to Cullen. LLR test applies since k = n < 2^n.

- **Records:** W(8,508,301) ~2.56M digits. More common than Cullen.
- **Test:** Lucas-Lehmer-Riesel (already implemented).
- **Code estimate:** ~300 lines (shares LLR with kbn -1 form).
- **OEIS:** [A002234](https://oeis.org/A002234).

### Wagstaff primes ((2^p + 1) / 3)

No proven deterministic test (500 EUR bounty open). No active search project.

- **Records:** Proven: W(138,937). Probable: W(15,135,397) ~4.5M digits.
- **Test:** Fermat PRP. Modular reduction exploits 2^p structure.
- **Sieve:** Factors must have form 2kp + 1. Only prime exponents need testing.
- **Code estimate:** ~200-250 lines.
- **OEIS:** [A000979](https://oeis.org/A000979).

### Carol/Kynea primes

Carol: (2^n - 1)^2 - 2. Kynea: (2^n + 1)^2 - 2. Carol has k < 2^n so LLR applies. Kynea has k > 2^n, only MR.

- **Records:** Kynea: K(852,770) ~513K digits. Carol: C(695,631) ~418K digits.
- **Code estimate:** ~250-300 lines.
- **OEIS:** [A091515](https://oeis.org/A091515), [A093069](https://oeis.org/A093069).

### Repunit primes ((10^n - 1) / 9)

All 1s. n must be prime. Only 5 proven. No specialized deterministic test.

- **Records:** Proven: R(1,031). Probable: R(8,177,207) ~8.2M digits.
- **Code estimate:** ~200-250 lines.
- **OEIS:** [A004023](https://oeis.org/A004023).

## Tier C: Requires Additional Infrastructure

### Twin primes (k * 2^n +/- 1, both prime)

Both k*2^n+1 (Proth) and k*2^n-1 (LLR) must be prime. Quad sieve eliminates k values where any of 4 related forms has a small factor.

- **Records:** 2,996,863,034,895 * 2^1,290,000 +/- 1 (~388K digits, 2016).
- **Code estimate:** ~400-450 lines (quad sieve + dual Proth+LLR).
- **OEIS:** [A001359](https://oeis.org/A001359).

### Sophie Germain primes

p where 2p+1 is also prime. Shares infrastructure with twin primes.

- **Records:** 2,618,163,402,417 * 2^1,290,000 - 1 (~388K digits, 2016).
- **Code estimate:** ~350-400 lines.
- **OEIS:** [A005384](https://oeis.org/A005384).

### Generalized Fermat primes (b^(2^n) + 1)

Pepin-style deterministic test. GPU acceleration effectively required for n >= 20.

- **Records:** GFN-21 with 13.4M digits (PrimeGrid, Oct 2025).
- **Code estimate:** ~350-400 lines.
- **OEIS:** [A000215](https://oeis.org/A000215).

---

## Recommended Addition Order

```
1. Primorial primes       (trivial port of factorial.rs)
2. Cullen primes          (wrapper around kbn with k=n)
3. Woodall primes         (shares LLR from kbn)
4. Wagstaff primes        (unique niche, no competition)
5. Carol/Kynea primes     (shares LLR)
6. Twin primes            (Proth + LLR, quad sieve)
7. Sophie Germain primes  (shares twin infrastructure)
8. Repunit primes         (straightforward, no deterministic test)
9. Generalized Fermat     (GPU-preferred for frontier)
```
