# Factorial Primes (n! +/- 1)

## Definition

A factorial prime is a prime of the form $n! + 1$ or $n! - 1$, where $n!$ is the product of all positive integers from 1 to $n$. Since $n!$ is divisible by every integer up to $n$, both $n! + 1$ and $n! - 1$ are guaranteed to have no prime factor $\leq n$, making them natural primality candidates.

## Known factorial primes

**$n! + 1$ is prime for** (OEIS [A002981](https://oeis.org/A002981)):

n = 1, 2, 3, 11, 27, 37, 41, 73, 77, 116, 154, 320, 340, 399, 427, 872, 1477, 6380, 26951, 110059, 150209, 288465, 308084, 422429

**$n! - 1$ is prime for** (OEIS [A002982](https://oeis.org/A002982)):

n = 3, 4, 6, 7, 12, 14, 30, 32, 33, 38, 94, 166, 324, 379, 469, 546, 974, 1963, 3507, 3610, 6917, 21480, 34790, 94550, 103040, 147855, 208003, 632760

As of late 2024, there are 52 known factorial primes across both forms.

## Records

| Expression | Digits | Discoverer | Date |
|---|---|---|---|
| 632760! - 1 | 3,395,992 | A43 (volunteer) | Oct 2024 |
| 422429! + 1 | 2,193,027 | PrimeGrid | Feb 2022 |
| 308084! + 1 | 1,557,176 | PrimeGrid | Jan 2022 |
| 288465! + 1 | 1,449,771 | PrimeGrid | Jan 2022 |
| 208003! - 1 | 1,015,843 | S. Fukui | Jul 2016 |

## Computational notes

### Why $n! + 1$ has an advantage

Primality testing for $n! + 1$ can use the **Pocklington primality test** because $N - 1 = n!$ has a fully known factorization (it's the product of all integers up to $n$). This allows deterministic proofs of primality, not just probable-prime results.

$n! - 1$ does not have this $N-1$ factoring advantage and requires other methods.

### Software

- **PFGW** (PrimeForm/GW) using George Woltman's gwnum library for fast modular arithmetic
- **PRST** by Pavel Atnashev, specifically designed for factorial and primorial primes with Gerbicz-Li error detection

### Sieving

Before full primality testing, candidates are sieved for divisibility by small primes. Since $n! + 1$ and $n! - 1$ have no factors $\leq n$, sieving starts from primes greater than $n$.

### Difficulty scaling

By Stirling's approximation, $n!$ has approximately $n \log_{10}(n) - n \log_{10}(e)$ digits. For $n = 632760$, that's ~3.4 million digits. Each primality test requires modular exponentiation with cost $O(d \cdot \log d)$ per squaring step, where $d$ is the digit count.

## Connection to Wilson's theorem

Wilson's theorem states that $p$ is prime if and only if $(p-1)! \equiv -1 \pmod{p}$. While theoretically beautiful, this is computationally useless for primality testing because computing $(N-1)! \bmod N$ is at least as expensive as trial division.

## Search projects

- **[PrimeGrid](https://www.primegrid.com/)** runs the Factorial Prime Search (FPS) as a BOINC distributed computing project and has been responsible for the majority of modern discoveries.
- Both forms have been tested up to at least n = 1,000,000.
