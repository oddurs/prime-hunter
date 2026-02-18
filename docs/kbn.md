# Primes of the Form k * b^n +/- 1

## Definition

This family of prime forms is parameterized by a multiplier k, a base b, and an exponent n. Different parameter choices yield well-known prime families:

| Form | Name | Condition |
|---|---|---|
| $k \cdot 2^n + 1$ | **Proth prime** | $k$ odd, $k < 2^n$ |
| $k \cdot 2^n - 1$ | **Riesel prime** | $k$ odd, $k < 2^n$ |
| $1 \cdot 2^n - 1$ | **Mersenne prime** | $n$ must be prime |
| $1 \cdot 2^{2^m} + 1$ | **Fermat prime** | only 5 known: $F_0 \ldots F_4$ |
| $b^{2^m} + 1$ | **Generalized Fermat** | $b > 2$, $b$ even |

## Why these primes are computationally tractable

The vast majority of the largest known primes are of this form. The reason is that specialized primality tests exist that are dramatically faster than general-purpose methods.

### Proth's theorem (for $k \cdot 2^n + 1$)

**Statement**: Let $N = k \cdot 2^n + 1$ where $k$ is odd and $2^n > k$. If there exists an integer $a$ such that:

$$a^{(N-1)/2} \equiv -1 \pmod{N}$$

then $N$ is **proven** prime. This is deterministic, not probabilistic.

The test requires a single modular exponentiation: $O(n)$ squarings mod $N$. Each squaring of a $d$-digit number costs $O(d \cdot \log d)$ with FFT multiplication. Compare this to general-purpose primality proving (ECPP) at $O(d^4)$ or worse.

In practice, small witnesses $a = 2, 3, 5, \ldots$ are tried and one almost always works quickly.

### Lucas-Lehmer-Riesel test (for $k \cdot 2^n - 1$)

For $N = k \cdot 2^n - 1$ ($k$ odd, $k < 2^n$), define a Lucas sequence:

$$u_0 = f(k), \quad u_{i+1} = u_i^2 - 2 \pmod{N}$$

$N$ is prime if and only if $u_{n-2} = 0 \pmod{N}$. This requires $n - 2$ squarings mod $N$, same complexity class as Proth's test.

The standard **Lucas-Lehmer test** for Mersenne primes ($k = 1$) is the special case with $u_0 = 4$.

### Software

- **Prime95 / mprime** (George Woltman): Used by GIMPS for Mersenne primes. Implements the world's fastest large-integer FFT multiplication.
- **LLR** (Jean Penne): Implements the Lucas-Lehmer-Riesel test. Widely used by PrimeGrid.
- **PFGW** (OpenPFGW): General-purpose tool for Proth and other forms.

## Current record: 2^136,279,841 - 1

Discovered October 12, 2024 by **Luke Durant** through GIMPS. This Mersenne prime has **41,024,320 digits** and is the largest known prime of any kind. It is the 52nd known Mersenne prime and the first ever discovered using GPUs (Durant used thousands of NVIDIA GPUs across 24 datacenter regions in 17 countries).

## Active search projects

### GIMPS (Great Internet Mersenne Prime Search)

- Active since 1996
- Found the 17 largest known Mersenne primes
- All exponents below 136,279,841 have been tested at least once
- Uses Prime95/mprime software
- [mersenne.org](https://www.mersenne.org/)

### PrimeGrid

- BOINC distributed computing project
- Runs multiple sub-projects: Proth Prime Search, Sierpinski Problem, Riesel Problem, Generalized Fermat Search, Factorial Prime Search
- [primegrid.com](https://www.primegrid.com/)

### Seventeen or Bust (now part of PrimeGrid)

Attempting to prove 78,557 is the smallest **Sierpinski number** (a k where k * 2^n + 1 is composite for ALL n >= 1). Must find at least one prime k * 2^n + 1 for every odd k < 78,557.

**Five candidates remain**: k = 21181, 22699, 24737, 55459, 67607.

The most recent elimination was k = 10223 via 10223 * 2^31,172,165 + 1 (9.4 million digits, November 2016).

### Riesel Problem

Attempting to prove 509,203 is the smallest **Riesel number** (a k where k * 2^n - 1 is composite for all n >= 1).

**41 candidates remain** out of the original 101. The most recent elimination was 107347 * 2^23,427,517 - 1 (August 2024, Ryan Propper).

## Notable discoveries

| Prime | Digits | Form | Date |
|---|---|---|---|
| 2^136,279,841 - 1 | 41,024,320 | Mersenne | Oct 2024 |
| 2^82,589,933 - 1 | 24,862,048 | Mersenne | Dec 2018 |
| 10223 * 2^31,172,165 + 1 | 9,383,761 | Proth / Sierpinski | Nov 2016 |
| 107347 * 2^23,427,517 - 1 | ~7,050,000 | Riesel | Aug 2024 |

## What primehunt does differently

`primehunt kbn` uses GMP's `is_probably_prime(25)` (Miller-Rabin) rather than Proth's theorem or LLR. This means:

- Results are **probabilistic** for large numbers (probability of false positive: $< 4^{-25}$)
- It works for **any** k, b, n combination, not just b = 2
- It does **not** provide the speed advantages of specialized tests

For serious large-scale searching of k * 2^n +/- 1 forms, specialized tools like LLR, Prime95, or PFGW are orders of magnitude faster. `primehunt` is useful for exploration across arbitrary bases and multipliers.
