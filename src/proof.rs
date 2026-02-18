//! # Proof — Deterministic Primality Proofs
//!
//! Provides deterministic (non-probabilistic) primality proofs for candidates
//! found by the search modules. These upgrade "probably prime" results from
//! Miller–Rabin into mathematically rigorous certificates.
//!
//! ## Proof Methods
//!
//! ### Pocklington N−1 Proof (for n!+1, p#+1)
//!
//! When N−1 has fully known factorization (as for factorial and primorial primes),
//! Pocklington's theorem provides a deterministic proof: for each prime factor q
//! of N−1, find witness a such that a^(N−1) ≡ 1 (mod N) and gcd(a^((N−1)/q) − 1, N) = 1.
//!
//! ### Morrison N+1 Proof (for n!−1, p#−1)
//!
//! When N+1 has fully known factorization, Morrison's theorem (a dual of
//! Pocklington) proves primality using Lucas V-sequences: V_{N+1}(P,1) ≡ 2 (mod N)
//! and gcd(V_{(N+1)/q}(P,1) − 2, N) = 1 for each prime factor q.
//!
//! ### BLS N+1 Proof (for near-repdigit palindromes)
//!
//! Brillhart–Lehmer–Selfridge theorem: if ≥ 1/3 of N+1's bits come from known
//! prime factors, the Morrison conditions prove primality. For near-repdigit
//! palindromes, N+1 contains a large power of 10 = 2·5, providing factored
//! bits for free. Trial division of the cofactor adds more when needed.
//!
//! ## Lucas V-Sequence
//!
//! Both Morrison and BLS proofs use the Lucas V binary chain:
//! V₀ = 2, V₁ = P, V_n = P·V_{n−1} − V_{n−2}. Computed in O(log k)
//! multiplications mod N using the doubling formulas:
//! V(2m) = V(m)² − 2, V(2m+1) = V(m)·V(m+1) − P.
//!
//! `lucas_v_big` accepts arbitrary-precision indices (needed for n!/q where
//! n!/q exceeds u64), while `kbn::lucas_v_k` handles u64 indices.
//!
//! ## References
//!
//! - H.C. Pocklington, "The Determination of the Prime or Composite Nature
//!   of Large Numbers by Fermat's Theorem", Proc. Cambridge Phil. Soc., 1914.
//! - M.A. Morrison, "A Note on Primality Testing Using Lucas Sequences",
//!   Mathematics of Computation, 29(129), 1975.
//! - J. Brillhart, D.H. Lehmer, J.L. Selfridge, "New Primality Criteria and
//!   Factorizations of 2^m ± 1", Mathematics of Computation, 29(130), 1975.
//! - OEIS: [A002981](https://oeis.org/A002981) — n! + 1 primes.
//! - OEIS: [A002982](https://oeis.org/A002982) — n! − 1 primes.

use rayon::prelude::*;
use rug::ops::{Pow, RemRounding};
use rug::Integer;

/// Compute V_k(P, 1) mod N using the Lucas V binary chain, with arbitrary-precision index k.
///
/// Same algorithm as `lucas_v_k` in kbn.rs but accepts `rug::Integer` index
/// (needed because n!/q is too large for u64).
pub(crate) fn lucas_v_big(k: &Integer, p_val: u32, n: &Integer) -> Integer {
    if *k == 0u32 {
        return Integer::from(2);
    }
    if *k == 1u32 {
        return Integer::from(p_val).rem_euc(n);
    }

    let p_int = Integer::from(p_val);
    let mut r = p_int.clone(); // V(1) = P
    let mut s = (Integer::from(&p_int * &p_int) - 2u32).rem_euc(n); // V(2) = P^2 - 2

    let bits = k.significant_bits(); // number of significant bits
    for i in (0..bits - 1).rev() {
        if !k.get_bit(i) {
            s = Integer::from(&r * &s - &p_int).rem_euc(n);
            r.square_mut();
            r -= 2u32;
            r = r.rem_euc(n);
        } else {
            r = Integer::from(&r * &s - &p_int).rem_euc(n);
            s.square_mut();
            s -= 2u32;
            s = s.rem_euc(n);
        }
    }
    r
}

/// Pocklington N-1 proof for n!+1.
///
/// N-1 = n! has fully known factorization (all primes <= n).
/// For each prime factor q <= n, find witness a such that:
///   1. a^(N-1) ≡ 1 (mod N)
///   2. gcd(a^((N-1)/q) - 1, N) = 1
pub fn pocklington_factorial_proof(n: u64, candidate: &Integer, sieve_primes: &[u64]) -> bool {
    // Collect prime factors of n! (all primes <= n)
    let factors: Vec<u64> = sieve_primes.iter().copied().filter(|&p| p <= n).collect();

    // If n > largest sieve prime, we can't factor n! completely
    if n > *sieve_primes.last().unwrap_or(&0) {
        eprintln!("  Pocklington: n={} exceeds sieve limit, skipping proof", n);
        return false;
    }

    // N-1 = n! has no prime factors when n <= 1 (trivial case)
    if factors.is_empty() {
        return *candidate == 2u32;
    }

    let n_minus_1 = Integer::from(candidate - 1u32); // = n!

    eprintln!(
        "  Pocklington proof: verifying {} prime factors of {}!",
        factors.len(),
        n
    );

    // Parallelize across factors
    let all_pass = factors.par_iter().enumerate().all(|(i, &q)| {
        if i > 0 && i % 100 == 0 {
            eprintln!("  Pocklington: {}/{} factors verified", i, factors.len());
        }

        let exp_q = Integer::from(&n_minus_1 / q);

        // Try witnesses a = 2..=200 until one works for this factor.
        // For prime N and factor q, fraction 1-1/q of bases work,
        // so even q=2 succeeds within ~2 attempts on average.
        for a in 2u32..=200 {
            let a_int = Integer::from(a);

            // Check a^(N-1) ≡ 1 (mod N)
            let fermat = match a_int.clone().pow_mod(&n_minus_1, candidate) {
                Ok(r) => r,
                Err(_) => return false,
            };
            if fermat != 1u32 {
                return false; // N is composite (should not happen after MR)
            }

            // Check gcd(a^((N-1)/q) - 1, N) = 1
            let r = match a_int.pow_mod(&exp_q, candidate) {
                Ok(r) => r,
                Err(_) => return false,
            };
            let g = (r - 1u32).gcd(candidate);
            if g == 1u32 {
                return true; // This factor passes with this witness
            }
        }
        false // No witness worked for this factor
    });

    all_pass
}

/// Morrison N+1 proof for n!-1 using Lucas V-sequences.
///
/// N+1 = n! has fully known factorization (all primes <= n).
/// For each suitable P (Jacobi(P^2-4, N) = -1):
///   1. Verify V_{N+1}(P,1) ≡ 2 (mod N)
///   2. For each prime factor q: gcd(V_{(N+1)/q}(P,1) - 2, N) = 1
///
/// Different factors may require different P values. We try multiple P values
/// and accumulate satisfied factors across all of them.
pub fn morrison_factorial_proof(n: u64, candidate: &Integer, sieve_primes: &[u64]) -> bool {
    let factors: Vec<u64> = sieve_primes.iter().copied().filter(|&p| p <= n).collect();

    if n > *sieve_primes.last().unwrap_or(&0) {
        eprintln!("  Morrison: n={} exceeds sieve limit, skipping proof", n);
        return false;
    }

    if factors.is_empty() {
        return false;
    }

    let n_plus_1 = Integer::from(candidate + 1u32); // = n!

    eprintln!(
        "  Morrison proof: verifying {} prime factors of {}!",
        factors.len(),
        n
    );

    // Track which factors have been satisfied
    let mut factor_satisfied = vec![false; factors.len()];

    // Try multiple P values until all factors are satisfied
    for p_candidate in 3..1003u32 {
        let disc = Integer::from(p_candidate * p_candidate) - 4u32;
        if disc.jacobi(candidate) != -1 {
            continue;
        }

        // Global check: V_{N+1}(P,1) ≡ 2 (mod N)
        let v_full = lucas_v_big(&n_plus_1, p_candidate, candidate);
        if v_full != 2u32 {
            return false; // N is composite
        }

        // Check unsatisfied factors with this P
        let newly_satisfied: Vec<(usize, bool)> = factors
            .par_iter()
            .enumerate()
            .filter(|(i, _)| !factor_satisfied[*i])
            .map(|(i, &q)| {
                let exp_q = Integer::from(&n_plus_1 / q);
                let v = lucas_v_big(&exp_q, p_candidate, candidate);
                let g = (v - 2u32).gcd(candidate);
                (i, g == 1u32)
            })
            .collect();

        for (i, passed) in newly_satisfied {
            if passed {
                factor_satisfied[i] = true;
            }
        }

        if factor_satisfied.iter().all(|&s| s) {
            return true;
        }
    }

    let remaining = factor_satisfied.iter().filter(|&&s| !s).count();
    eprintln!(
        "  Morrison: {} factors unsatisfied after exhausting P candidates",
        remaining
    );
    false
}

/// BLS N+1 proof for near-repdigit palindromes.
///
/// For N = 10^(2k+1) - 1 - d*(10^(k+m) + 10^(k-m)):
///   N+1 = 10^(k-m) * cofactor
///   where cofactor = 10^(k+m+1) - d*(10^(2m) + 1)
///
/// 10^(k-m) = 2^(k-m) * 5^(k-m) is trivially factored, providing
/// (k-m)*log2(10) bits. BLS requires >= 1/3 of N+1's bits factored.
///
/// Succeeds from 10-power alone when m < (k-1)/3. For larger m,
/// trial-divides the cofactor by sieve primes for extra factorization.
pub fn bls_near_repdigit_proof(
    k: u64,
    d: u32,
    m: u64,
    candidate: &Integer,
    sieve_primes: &[u64],
) -> bool {
    let n_plus_1 = Integer::from(candidate + 1u32);
    let power_of_10_exp = k - m;

    // 10^(k-m) = 2^(k-m) * 5^(k-m), contributing (k-m)*log2(10) bits
    let mut factored_bits = power_of_10_exp as f64 * 10f64.log2();
    let total_bits = candidate.significant_bits() as f64;

    // Compute cofactor: N+1 / 10^(k-m)
    let cofactor = if m == 0 {
        Integer::from(10u32).pow(crate::checked_u32(k + 1)) - Integer::from(2 * d)
    } else {
        Integer::from(10u32).pow(crate::checked_u32(k + m + 1))
            - Integer::from(d) * (Integer::from(10u32).pow(crate::checked_u32(2 * m)) + 1u32)
    };

    // Verify factorization: 10^(k-m) * cofactor == N+1
    debug_assert_eq!(
        Integer::from(10u32).pow(crate::checked_u32(power_of_10_exp)) * &cofactor,
        n_plus_1,
        "BLS factorization check failed"
    );

    // Trial-divide cofactor by sieve primes for extra factorization
    let mut cofactor_factors: Vec<(u64, u32)> = Vec::new();
    let mut remaining = cofactor;

    for &p in sieve_primes {
        if remaining == 1u32 {
            break;
        }
        if p <= u32::MAX as u64 && remaining.is_divisible_u(p as u32) {
            let mut exp = 0u32;
            let p_int = Integer::from(p as u32);
            while remaining.is_divisible(&p_int) {
                remaining /= &p_int;
                exp += 1;
            }
            cofactor_factors.push((p, exp));
            factored_bits += exp as f64 * (p as f64).log2();
        }
    }

    // BLS threshold: factored portion must exceed N^(1/3)
    if factored_bits < total_bits / 3.0 {
        return false;
    }

    // Collect all distinct prime factors for BLS verification
    let mut all_factors: Vec<u64> = Vec::new();
    if power_of_10_exp > 0 {
        all_factors.push(2);
        all_factors.push(5);
    }
    for &(p, _) in &cofactor_factors {
        if !all_factors.contains(&p) {
            all_factors.push(p);
        }
    }

    if all_factors.is_empty() {
        return false;
    }

    eprintln!(
        "  BLS N+1 proof: {} prime factors, {:.0}/{:.0} bits factored ({:.1}%)",
        all_factors.len(),
        factored_bits,
        total_bits,
        factored_bits / total_bits * 100.0
    );

    // BLS verification using Lucas V sequences
    // For each prime factor q, find P with Jacobi(P²-4, N) = -1 such that:
    //   1. V_{N+1}(P,1) ≡ 2 (mod N)
    //   2. gcd(V_{(N+1)/q}(P,1) - 2, N) = 1
    let n_plus_1 = Integer::from(candidate + 1u32);
    let mut factor_satisfied = vec![false; all_factors.len()];

    for p_candidate in 3..1003u32 {
        let disc = Integer::from(p_candidate * p_candidate) - 4u32;
        if disc.jacobi(candidate) != -1 {
            continue;
        }

        // Global check: V_{N+1}(P,1) ≡ 2 (mod N)
        let v_full = lucas_v_big(&n_plus_1, p_candidate, candidate);
        if v_full != 2u32 {
            return false; // Composite
        }

        // Check unsatisfied factors with this P
        for (i, &q) in all_factors.iter().enumerate() {
            if factor_satisfied[i] {
                continue;
            }
            let exp_q = Integer::from(&n_plus_1 / q);
            let v = lucas_v_big(&exp_q, p_candidate, candidate);
            let g = (v - 2u32).gcd(candidate);
            if g == 1u32 {
                factor_satisfied[i] = true;
            }
        }

        if factor_satisfied.iter().all(|&s| s) {
            return true;
        }
    }

    let remaining_count = factor_satisfied.iter().filter(|&&s| !s).count();
    eprintln!(
        "  BLS: {} factors unsatisfied after exhausting P candidates",
        remaining_count
    );
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sieve;

    fn factorial(n: u64) -> Integer {
        Integer::from(Integer::factorial(n as u32))
    }

    // ---- lucas_v_big cross-validation against kbn's lucas_v_k ----

    #[test]
    fn lucas_v_big_matches_small_indices() {
        let modulus = Integer::from(1000003u32); // a prime
        for k in 0..20u64 {
            let k_big = Integer::from(k);
            let result_big = lucas_v_big(&k_big, 4, &modulus);
            // Compute reference via the recurrence directly
            let result_ref = lucas_v_ref(k, 4, &modulus);
            assert_eq!(
                result_big, result_ref,
                "lucas_v_big({}, 4, {}) mismatch",
                k, modulus
            );
        }
    }

    /// Reference Lucas V computation for small k
    fn lucas_v_ref(k: u64, p: u32, n: &Integer) -> Integer {
        if k == 0 {
            return Integer::from(2);
        }
        if k == 1 {
            return Integer::from(p).rem_euc(n);
        }
        let mut prev2 = Integer::from(2); // V(0)
        let mut prev1 = Integer::from(p); // V(1)
        for _ in 2..=k {
            // V(i) = P * V(i-1) - V(i-2)
            let next = (Integer::from(p) * &prev1 - &prev2).rem_euc(n);
            prev2 = prev1;
            prev1 = next;
        }
        prev1
    }

    // ---- Pocklington positives: known factorial primes n!+1 ----

    #[test]
    fn pocklington_proves_small_factorial_primes() {
        let sieve_primes = sieve::generate_primes(1000);
        // n!+1 is prime for n = 1, 2, 3, 11, 27, 37, 41, 73, 77, 116, 154
        for &n in &[1u64, 2, 3, 11, 27, 37, 41] {
            let f = factorial(n);
            let candidate = Integer::from(&f + 1u32);
            // Verify it's actually prime first
            assert_ne!(
                candidate.is_probably_prime(25),
                rug::integer::IsPrime::No,
                "{}!+1 should be prime",
                n
            );
            assert!(
                pocklington_factorial_proof(n, &candidate, &sieve_primes),
                "Pocklington should prove {}!+1 prime",
                n
            );
        }
    }

    // ---- Pocklington negatives: n!+1 composite ----

    #[test]
    fn pocklington_rejects_composite() {
        let sieve_primes = sieve::generate_primes(1000);
        // 4!+1 = 25 = 5*5 — composite
        let candidate = Integer::from(25u32);
        assert!(
            !pocklington_factorial_proof(4, &candidate, &sieve_primes),
            "Pocklington should reject 4!+1 = 25"
        );
    }

    // ---- Morrison positives: known factorial primes n!-1 ----

    #[test]
    fn morrison_proves_small_factorial_primes() {
        let sieve_primes = sieve::generate_primes(1000);
        // n!-1 is prime for n = 3, 4, 6, 7, 12, 14, 30, 32, 33, 38, ...
        // Skip n=3 (N=5 too small — group order causes V_{(N+1)/q} ≡ 2 for q=2)
        for &n in &[4u64, 6, 7, 12, 14] {
            let f = factorial(n);
            let candidate = Integer::from(&f - 1u32);
            assert_ne!(
                candidate.is_probably_prime(25),
                rug::integer::IsPrime::No,
                "{}!-1 should be prime",
                n
            );
            assert!(
                morrison_factorial_proof(n, &candidate, &sieve_primes),
                "Morrison should prove {}!-1 prime",
                n
            );
        }
    }

    // ---- Morrison negatives: n!-1 composite ----

    #[test]
    fn morrison_rejects_composite() {
        let sieve_primes = sieve::generate_primes(1000);
        // 5!-1 = 119 = 7*17 — composite
        let candidate = Integer::from(119u32);
        assert!(
            !morrison_factorial_proof(5, &candidate, &sieve_primes),
            "Morrison should reject 5!-1 = 119"
        );
    }

    // ---- find suitable Morrison P ----

    #[test]
    fn find_morrison_p_succeeds() {
        // For a large-ish prime, a suitable P (Jacobi(P^2-4, N) = -1) should exist
        let n = Integer::from(Integer::factorial(12u32)) - 1u32; // 479001599 (prime)
        let mut found = false;
        for p in 3..100u32 {
            let disc = Integer::from(p * p) - 4u32;
            if disc.jacobi(&n) == -1 {
                found = true;
                break;
            }
        }
        assert!(found, "Should find P with Jacobi(P^2-4, N) = -1 for 12!-1");
    }

    // ---- BLS near-repdigit proofs ----

    #[test]
    fn bls_proves_small_near_repdigit_primes() {
        use crate::near_repdigit;
        let sieve_primes = sieve::generate_primes(10_000);

        // Known near-repdigit palindromic primes
        let cases = [
            (1u64, 4u32, 0u64, 919u64), // N+1=920=2^3*5*23
            (1, 2, 1, 797),             // N+1=798=2*3*7*19
            (1, 8, 1, 191),             // N+1=192=2^6*3
            (2, 4, 1, 95959),           // N+1=95960=2^3*5*2399
            (2, 8, 2, 19991),           // N+1=19992=2^3*3*7^2*17
        ];

        for &(k, d, m, expected_val) in &cases {
            let candidate = near_repdigit::build_candidate(k, d, m);
            assert_eq!(candidate, Integer::from(expected_val));
            assert!(
                bls_near_repdigit_proof(k, d, m, &candidate, &sieve_primes),
                "BLS should prove {} prime (k={}, d={}, m={})",
                expected_val,
                k,
                d,
                m
            );
        }
    }

    #[test]
    fn bls_rejects_composite_near_repdigit() {
        let sieve_primes = sieve::generate_primes(10_000);
        // k=1, d=1, m=0: 999 - 2*10 = 979 = 11*89 (composite)
        let candidate = Integer::from(979u32);
        assert!(
            !bls_near_repdigit_proof(1, 1, 0, &candidate, &sieve_primes),
            "BLS should reject 979 (composite)"
        );
    }

    #[test]
    fn bls_cofactor_factoring_needed() {
        use crate::near_repdigit;
        let sieve_primes = sieve::generate_primes(10_000);

        // k=1, d=8, m=1: N=191, N+1=192=2^6*3
        // 10^(k-m) = 10^0 = 1, so 10-power contributes 0 bits.
        // Cofactor factoring provides all factored bits.
        let candidate = near_repdigit::build_candidate(1, 8, 1);
        assert_eq!(candidate, Integer::from(191u32));
        assert!(
            bls_near_repdigit_proof(1, 8, 1, &candidate, &sieve_primes),
            "BLS should prove 191 with cofactor factoring"
        );
    }
}
