# Palindromic Primes

## Definition

A palindromic prime is a prime number that reads the same forwards and backwards in a given base. In base 10, the first palindromic primes are:

2, 3, 5, 7, 11, 101, 131, 151, 181, 191, 313, 353, 373, 383, 727, 757, 787, 797, 919, 929, ...

OEIS: [A002385](https://oeis.org/A002385)

## Even-digit palindromes are composite

**Theorem**: Except for 11 itself, no palindromic number with an even number of digits is prime.

**Proof**: The divisibility rule for 11 says a number is divisible by 11 if the alternating sum of its digits is divisible by 11. In a palindrome with $2k$ digits, the digits satisfy $d_i = d_{2k+1-i}$. In the alternating sum, each digit appears once with a positive sign and once with a negative sign, so all pairs cancel and the sum is 0. Therefore the number is divisible by 11.

Since the number has more than 2 digits and is divisible by 11, it is composite. Only 11 itself (the 2-digit case) escapes this.

**This generalizes to any base**: in base $b$, even-digit palindromes are always divisible by $b + 1$. So in base 2, even-digit binary palindromes are divisible by 3; in base 16, by 17; etc. `primehunt` exploits this by skipping all even digit counts.

## Distribution

| Range | Palindromic primes below |
|---|---|
| 10 | 4 |
| 100 | 5 |
| 1,000 | 20 |
| 10,000 | 20 |
| 100,000 | 113 |
| 1,000,000 | 113 |
| 10,000,000 | 781 |

The pattern (no growth at even digit boundaries) is explained by the even-digit theorem above.

Heuristically, the count of palindromic primes up to $N$ grows as roughly $\sqrt{N} / \log N$, because there are $\sim\!\sqrt{N}$ palindromes up to $N$ and each has $\sim\!1/\log N$ probability of being prime.

## Largest known palindromic prime

As of August 2024, the record is held by **Ryan Propper and Serge Batalov**:

> $10^{2{,}718{,}281} - 5 \cdot 10^{1{,}631{,}138} - 5 \cdot 10^{1{,}087{,}142} - 1$

This number has **2,718,281 digits**. The previous record (from 2014) had only 474,501 digits.

## Open problems

**Are there infinitely many palindromic primes?** This is a major open problem in number theory. It is widely conjectured that infinitely many exist in every base, but no proof exists for any base. The extreme sparsity of palindromic numbers makes standard analytic techniques (sieve methods, circle method) insufficient.

The best partial result is that there are infinitely many palindromic numbers with at most six prime factors.

## Related forms

- **Repunit primes**: $(10^n - 1)/9 = 111\ldots1$. All repunit primes are palindromic. Known for $n = 2, 19, 23, 317, 1031, 49081, 86453, 109297, 270343, \ldots$
- **Belphegor primes**: Palindromic primes of the form 1(0^n)666(0^n)1. The 31-digit Belphegor's number (n=13) is prime.
- **Emirps**: Primes whose reversal is a different prime (e.g., 13 and 31). Palindromic primes are excluded by definition since their reversal is the same number.
- **Binary palindromes**: All Mersenne primes (2^p - 1) are palindromes in base 2 (all 1s), though this is trivial.
