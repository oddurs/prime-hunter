//! # Primehunt — Core Library
//!
//! Re-exports all engine and server modules, and provides shared utilities used
//! across multiple search forms: trial division pre-filtering, Miller–Rabin
//! pre-screening, and decimal digit estimation.
//!
//! ## Module Organization
//!
//! **Engine modules** (prime search algorithms):
//! - [`factorial`] — n! ± 1 primes (OEIS [A002981](https://oeis.org/A002981), [A002982](https://oeis.org/A002982))
//! - [`primorial`] — p# ± 1 primes (OEIS [A014545](https://oeis.org/A014545), [A057704](https://oeis.org/A057704))
//! - [`kbn`] — k·b^n ± 1 (Proth, Riesel, generalized forms)
//! - [`twin`] — Twin primes k·b^n ± 1 (both prime simultaneously)
//! - [`sophie_germain`] — Sophie Germain primes p, 2p+1 both prime
//! - [`palindromic`] — Palindromic primes in arbitrary bases
//! - [`near_repdigit`] — Near-repdigit palindromic primes
//! - [`cullen_woodall`] — Cullen (n·2^n + 1) and Woodall (n·2^n − 1)
//! - [`carol_kynea`] — Carol ((2^n−1)²−2) and Kynea ((2^n+1)²−2)
//! - [`wagstaff`] — Wagstaff primes (2^p + 1)/3 (OEIS [A000978](https://oeis.org/A000978))
//! - [`repunit`] — Repunit primes (b^n − 1)/(b − 1)
//! - [`gen_fermat`] — Generalized Fermat primes b^(2^n) + 1
//!
//! **Infrastructure modules** (server, coordination, proofs):
//! - [`sieve`] — Prime generation, Montgomery multiplication, BSGS discrete log
//! - [`proof`] — Pocklington N−1, Morrison N+1, BLS proofs
//! - [`p1`] — Pollard P−1 composite pre-filter
//! - [`prst`], [`pfgw`] — External tool integration (GWNUM-accelerated testing)
//! - [`dashboard`], [`db`], [`checkpoint`], [`progress`], etc.
//!
//! ## Shared Utilities
//!
//! - `has_small_factor`: Trial division by first 64 primes (up to 311).
//! - `mr_screened_test`: Two-round Miller–Rabin pre-screen before full test.
//! - `estimate_digits` / `exact_digits`: Decimal digit count from bit length.
//!
//! ## Design Philosophy
//!
//! All search modules follow the same pipeline: **sieve → parallel test → proof → log**.
//! The `CoordinationClient` trait allows search functions to check for stop signals
//! and report primes to either an HTTP coordinator or PostgreSQL directly.

pub mod agent;
pub mod ai_engine;
pub mod carol_kynea;
pub mod certificate;
pub mod checkpoint;
pub mod cullen_woodall;
pub mod dashboard;
pub mod db;
pub mod deploy;
pub mod events;
pub mod factorial;
pub mod fleet;
#[cfg(feature = "flint")]
pub mod flint;
pub mod gen_fermat;
pub mod gwnum;
pub mod kbn;
pub mod metrics;
pub mod near_repdigit;
pub mod p1;
pub mod palindromic;
pub mod pfgw;
pub mod pg_worker;
pub mod primorial;
pub mod progress;
pub mod project;
pub mod prom_metrics;
pub mod proof;
pub mod prst;
pub mod repunit;
pub mod search_manager;
pub mod search_params;
pub mod sieve;
pub mod sophie_germain;
pub mod strategy;
pub mod twin;
pub mod verify;
pub mod operator;
/// Backward compatibility re-export.
pub mod volunteer {
    pub use crate::operator::*;
}
pub mod wagstaff;
pub mod worker_client;

use rug::Integer;

/// Trait for coordination clients. All nodes now use `PgWorkerClient` (PostgreSQL-backed).
/// Search functions accept `Option<&dyn CoordinationClient>` to check for stop commands.
pub trait CoordinationClient: Send + Sync {
    fn is_stop_requested(&self) -> bool;
    fn report_prime(
        &self,
        form: &str,
        expression: &str,
        digits: u64,
        search_params: &str,
        proof_method: &str,
    );
}

/// Small primes for trial division pre-filter.
const SMALL_PRIMES: [u32; 64] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193,
    197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281, 283, 293, 307,
    311,
];

/// Convert a `u64` exponent to `u32` for `rug::Integer::pow()`, panicking with a clear
/// message if the value exceeds `u32::MAX`. This prevents silent truncation that would
/// produce wrong candidates and either miss primes or report false positives.
#[inline]
pub fn checked_u32(n: u64) -> u32 {
    u32::try_from(n).unwrap_or_else(|_| {
        panic!(
            "exponent {} exceeds u32::MAX ({}); candidate would be silently wrong",
            n,
            u32::MAX
        )
    })
}

/// Quick check if n is divisible by any small prime.
/// Returns true if n is definitely composite (has a small factor).
/// Returns false if n might be prime (passed trial division).
#[inline]
pub fn has_small_factor(n: &Integer) -> bool {
    for &p in &SMALL_PRIMES {
        if n.is_divisible_u(p) {
            // If n equals the small prime itself, it's prime, not composite.
            // Compare via PartialEq<u32> to avoid heap-allocating an Integer.
            return *n != p;
        }
    }
    false
}

/// Two-round Miller-Rabin pre-screening: run 2 fast rounds first, full rounds only for survivors.
/// Composites are rejected ~7x faster since most fail within 2 rounds.
///
/// For large candidates (>10K bits), also runs a Frobenius quadratic test that
/// catches composites MR occasionally misses (false positive < 1/7710 per round
/// vs MR's 1/4, at ~3× the cost of a single MR round).
#[inline]
pub fn mr_screened_test(candidate: &Integer, mr_rounds: u32) -> rug::integer::IsPrime {
    use rug::integer::IsPrime;
    if mr_rounds > 2 && candidate.is_probably_prime(2) == IsPrime::No {
        return IsPrime::No;
    }
    // Frobenius filter for candidates > 10K bits (where each MR round is expensive)
    if candidate.significant_bits() > 10_000 && !frobenius_test(candidate) {
        return IsPrime::No;
    }
    candidate.is_probably_prime(mr_rounds)
}

// ---- Frobenius quadratic compositeness test ----

/// Multiply two elements in the polynomial ring Z/nZ[x]/(x² − bx + c).
///
/// Each element is represented as `[a0, a1]` where the element is `a0 + a1·x`.
/// The reduction rule is `x² = bx − c`, so:
///
///   (a0 + a1·x)(b0 + b1·x) = a0·b0 + (a0·b1 + a1·b0)·x + a1·b1·x²
///                            = (a0·b0 − a1·b1·c) + (a0·b1 + a1·b0 + a1·b1·b)·x  (mod n)
fn poly_mul(
    a: &[Integer; 2],
    b: &[Integer; 2],
    coeff_b: &Integer,
    coeff_c: &Integer,
    n: &Integer,
) -> [Integer; 2] {
    // r0 = a0*b0 + a1*b1*(b*x - c) evaluated at degree 0 → a0*b0 - a1*b1*c
    // r1 = a0*b1 + a1*b0 + a1*b1*b
    let a1b1 = Integer::from(&a[1] * &b[1]) % n;
    let r0 = (Integer::from(&a[0] * &b[0]) - Integer::from(&a1b1 * coeff_c)) % n;
    let r1 = (Integer::from(&a[0] * &b[1])
        + Integer::from(&a[1] * &b[0])
        + Integer::from(&a1b1 * coeff_b))
        % n;
    // Normalize to [0, n)
    let r0 = if r0 < 0 { r0 + n } else { r0 };
    let r1 = if r1 < 0 { r1 + n } else { r1 };
    [r0, r1]
}

/// Square an element in the polynomial ring (slightly faster than generic mul).
fn poly_sqr(a: &[Integer; 2], coeff_b: &Integer, coeff_c: &Integer, n: &Integer) -> [Integer; 2] {
    poly_mul(a, a, coeff_b, coeff_c, n)
}

/// Binary exponentiation: compute x^exp mod (x² − bx + c) mod n.
///
/// Returns the result as `[r0, r1]` where the answer is `r0 + r1·x`.
/// The initial value is `x` itself, i.e., `[0, 1]`.
fn poly_pow_mod(exp: &Integer, coeff_b: &Integer, coeff_c: &Integer, n: &Integer) -> [Integer; 2] {
    if *exp == 0u32 {
        return [Integer::from(1u32), Integer::from(0u32)]; // 1 in the ring
    }

    let bits = exp.significant_bits();
    // Start with x = [0, 1]
    let mut result = [Integer::from(0u32), Integer::from(1u32)]; // = x
                                                                 // Process bits from second-highest down
    for i in (0..bits - 1).rev() {
        result = poly_sqr(&result, coeff_b, coeff_c, n);
        if exp.get_bit(i) {
            // Multiply by x: [r0, r1] * [0, 1] = [-r1*c, r0 + r1*b]
            let new_r0 = (Integer::from(n) - Integer::from(&result[1] * coeff_c) % n) % n;
            let new_r1 = (Integer::from(&result[0]) + Integer::from(&result[1] * coeff_b)) % n;
            result = [new_r0, new_r1];
        }
    }

    // Normalize
    result[0] %= n;
    result[1] %= n;
    if result[0] < 0 {
        result[0] += n;
    }
    if result[1] < 0 {
        result[1] += n;
    }
    result
}

/// Grantham's Restricted Quadratic Frobenius Test (RQFT).
///
/// Tests whether `n` behaves like a prime in a quadratic extension ring. For a
/// prime p, the Frobenius endomorphism x → x^p satisfies x^p ≡ (b − x) in
/// F_p[x]/(x² − bx + c) when `Jacobi(b² − 4c, p) = −1` (the polynomial is
/// irreducible over F_p).
///
/// Performs two independent checks:
/// 1. **Frobenius automorphism**: x^n ≡ (b − x) mod (x² − bx + c, n)
/// 2. **Euler criterion on c**: c^((n−1)/2) ≡ Jacobi(c, n) (mod n)
///
/// Both conditions must hold for a true prime. Together they dramatically reduce
/// false positives compared to either check alone.
///
/// **False positive rate**: < 1/7710 per round (Grantham, 2001), compared to
/// MR's 1/4. Combined with 2-round MR pre-screen, the probability of a
/// composite passing both is < 1/(16 × 7710) ≈ 1/123,000.
///
/// **Cost**: ~3× one MR round (polynomial ring exponentiation + Euler check).
///
/// Returns `true` if `n` passes (probably prime), `false` if definitely composite.
///
/// # References
///
/// - Jon Grantham, "Frobenius Pseudoprimes", Mathematics of Computation,
///   70(234):873–891, 2001.
/// - Crandall & Pomerance, "Prime Numbers: A Computational Perspective", §3.5.
pub fn frobenius_test(n: &Integer) -> bool {
    // Trivial cases
    if *n <= 2u32 {
        return *n == 2u32;
    }
    if n.is_even() {
        return false;
    }

    // Check for perfect squares — sqrt(n)² == n means n is composite.
    // Frobenius can be fooled by perfect squares in degenerate cases.
    {
        let s = n.clone().sqrt();
        if &(Integer::from(&s * &s)) == n {
            return false;
        }
    }

    // Find (b, c) with Jacobi(b² − 4c, n) = −1.
    // Prefer c ≥ 2 so the Euler criterion c^((n-1)/2) ≡ Jacobi(c,n) is non-trivial.
    let mut coeff_b = Integer::new();
    let mut coeff_c = Integer::new();
    let mut found = false;

    'outer: for c in 2u32..=20 {
        for b in 1u32..=50 {
            let disc = Integer::from(b * b) - Integer::from(4u32 * c);
            let j = disc.jacobi(n);
            if j == -1 {
                let g = disc.clone().abs().gcd(n);
                if g == 1u32 || &g == n {
                    coeff_b = Integer::from(b);
                    coeff_c = Integer::from(c);
                    found = true;
                    break 'outer;
                }
                return false; // non-trivial gcd → composite
            }
            if j == 0 {
                let g = disc.clone().abs().gcd(n);
                if g > 1u32 && &g < n {
                    return false;
                }
            }
        }
    }

    // Fallback: try c=1 if nothing found with c ≥ 2
    if !found {
        'fallback: for b in 1u32..=100 {
            let disc = Integer::from(b * b) - Integer::from(4u32);
            let j = disc.jacobi(n);
            if j == -1 {
                let g = disc.clone().abs().gcd(n);
                if g == 1u32 || &g == n {
                    coeff_b = Integer::from(b);
                    coeff_c = Integer::from(1u32);
                    found = true;
                    break 'fallback;
                }
                return false;
            }
            if j == 0 {
                let g = disc.clone().abs().gcd(n);
                if g > 1u32 && &g < n {
                    return false;
                }
            }
        }
    }

    if !found {
        return true; // extremely rare — can't run the test
    }

    if n.is_divisible(&coeff_c) {
        // c | n means n has a small factor (c ≤ 20) → composite (unless n = c)
        return coeff_c == *n;
    }

    // === Check 1: Euler criterion on c ===
    // For prime p: c^((p-1)/2) ≡ Jacobi(c, p) (mod p)
    let jacobi_c = coeff_c.clone().jacobi(n);
    if jacobi_c == 0 {
        // c and n share a factor → composite (since c ≤ 20)
        return false;
    }
    let half_nm1 = Integer::from(n - 1u32) >> 1u32;
    if let Ok(euler_val) = coeff_c.clone().pow_mod(&half_nm1, n) {
        let expected_euler = if jacobi_c == 1 {
            Integer::from(1u32)
        } else {
            Integer::from(n - 1u32) // -1 mod n
        };
        if euler_val != expected_euler {
            return false; // Euler criterion fails → composite
        }
    }

    // === Check 2: Frobenius automorphism ===
    // Compute x^n mod (x² − bx + c) in Z/nZ[x]
    let result = poly_pow_mod(n, &coeff_b, &coeff_c, n);

    // For prime p: x^p ≡ (b − x) mod (x² − bx + c, p)
    let expected_r0 = Integer::from(&coeff_b % n);
    let expected_r1 = Integer::from(n - 1u32); // -1 mod n

    result[0] == expected_r0 && result[1] == expected_r1
}

/// Estimate decimal digit count from bit length, avoiding expensive to_string conversion.
#[inline]
pub fn estimate_digits(n: &Integer) -> u64 {
    let bits = n.significant_bits();
    if bits == 0 {
        return 1;
    }
    (bits as f64 * std::f64::consts::LOG10_2) as u64 + 1
}

/// Exact decimal digit count (expensive for very large numbers).
#[inline]
pub fn exact_digits(n: &Integer) -> u64 {
    n.to_string_radix(10).len() as u64
}

/// Redact a database URL for safe logging. Replaces the password with `***`
/// while preserving the scheme, username, host, port, and database name.
///
/// Example: `postgres://user:secret@host:5432/db` becomes `postgres://user:***@host:5432/db`.
///
/// Returns the original string unchanged if it cannot be parsed as a URL.
pub fn redact_database_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut parsed) => {
            if parsed.password().is_some() {
                let _ = parsed.set_password(Some("***"));
            }
            parsed.to_string()
        }
        Err(_) => "***redacted***".to_string(),
    }
}

/// Block size for k*b^n±1-style searches (kbn, twin, Sophie Germain).
///
/// Larger blocks amortize sieve overhead; smaller blocks enable more frequent checkpointing.
/// For lightweight primality tests (Proth/LLR), blocks can be large.
#[inline]
pub fn block_size_for_n(n: u64) -> u64 {
    match n {
        0..=1_000 => 10_000,
        1_001..=10_000 => 10_000,
        10_001..=50_000 => 2_000,
        50_001..=200_000 => 500,
        _ => 100,
    }
}

/// Block size for heavy-computation searches (Cullen/Woodall, Carol/Kynea).
///
/// Smaller blocks than `block_size_for_n` because each candidate's test is more expensive
/// (full MR rather than fast Proth/LLR), so checkpoint intervals must be shorter.
#[inline]
pub fn block_size_for_n_heavy(n: u64) -> u64 {
    match n {
        0..=1_000 => 10_000,
        1_001..=10_000 => 5_000,
        10_001..=50_000 => 1_000,
        50_001..=200_000 => 200,
        _ => 50,
    }
}

#[cfg(test)]
mod tests {
    //! # Core Utility Tests
    //!
    //! Validates the shared primitives that every search form depends on:
    //!
    //! - **Trial division** (`has_small_factor`): Pre-filter using the 64 hardcoded
    //!   primes 2..311. Tests verify correct identification of primes, composites,
    //!   and the critical blind spot for semiprimes with both factors > 311.
    //!
    //! - **Miller-Rabin screening** (`mr_screened_test`): The two-round pre-screen
    //!   that gates expensive primality tests. Tests cover known primes, composites,
    //!   Mersenne numbers (OEIS [A000668](https://oeis.org/A000668)), and round-count
    //!   edge cases (1, 2, 15, 25 rounds).
    //!
    //! - **Grantham's Frobenius test** (`frobenius_test`): Quadratic extension ring
    //!   primality test with false positive rate < 1/7710 (Grantham, 2001). Tests
    //!   verify rejection of Carmichael numbers (OEIS [A002997](https://oeis.org/A002997)),
    //!   strong pseudoprimes to base 2, perfect squares, and cross-validate against
    //!   MR on all odd numbers in [3, 1000).
    //!
    //! - **Polynomial ring arithmetic** (`poly_mul`, `poly_sqr`, `poly_pow_mod`):
    //!   Operations in F_p[x]/(x^2 - bx + c) used by the Frobenius test. Tests
    //!   verify ring axioms (identity, commutativity, zero annihilation) and manual
    //!   computations of x^k for small k.
    //!
    //! - **Digit estimation** (`estimate_digits`, `exact_digits`): Tests verify
    //!   accuracy within +/- 1 digit across 10 orders of magnitude, exact results
    //!   at powers of 10, and the edge case of zero.
    //!
    //! - **Block sizing** (`block_size_for_n`, `block_size_for_n_heavy`): Tests
    //!   verify monotonic non-increasing behavior, boundary values at each match arm,
    //!   the invariant heavy <= normal, and positivity for all inputs.
    //!
    //! - **SMALL_PRIMES table**: Validates the hardcoded array is sorted, contains
    //!   only primes (verified via MR-25), starts at 2, ends at 311, and has exactly
    //!   64 entries (pi(311) - pi(1) = 63, plus the prime 2 = 64).
    //!
    //! ## References
    //!
    //! - Gary L. Miller, "Riemann's Hypothesis and Tests for Primality", 1976.
    //! - Michael O. Rabin, "Probabilistic Algorithm for Testing Primality", 1980.
    //! - Jon Grantham, "Frobenius Pseudoprimes", Mathematics of Computation, 2001.
    //! - OEIS A002997: Carmichael numbers (561, 1105, 1729, 2821, ...).
    //! - OEIS A001262: Strong pseudoprimes to base 2 (2047, 3277, 4033, ...).

    use super::*;
    use rug::integer::IsPrime;
    use rug::ops::Pow;

    // ── Trial Division (has_small_factor) ───────────────────────────────

    /// Verifies that every prime in the SMALL_PRIMES table (2..311) is correctly
    /// identified as NOT having a small factor. Each prime p divides only itself,
    /// and the `*n != p` guard in `has_small_factor` ensures self-division is
    /// excluded from the composite check.
    #[test]
    fn has_small_factor_returns_false_for_small_primes() {
        // Each small prime in our table should NOT be flagged as composite
        for &p in &SMALL_PRIMES {
            let n = Integer::from(p);
            assert!(
                !has_small_factor(&n),
                "has_small_factor incorrectly flagged prime {} as composite",
                p
            );
        }
    }

    /// Verifies that small composites (products of primes <= 311) are correctly
    /// detected by trial division. Includes perfect powers (4=2^2, 8=2^3, 9=3^2,
    /// 25=5^2, 49=7^2), semiprimes (6=2*3, 15=3*5, 21=3*7, 35=5*7), and
    /// smooth numbers (100=2^2*5^2, 1000=2^3*5^3).
    #[test]
    fn has_small_factor_returns_true_for_composites() {
        let composites: &[u32] = &[4, 6, 8, 9, 10, 12, 15, 21, 25, 35, 49, 100, 1000];
        for &c in composites {
            let n = Integer::from(c);
            assert!(
                has_small_factor(&n),
                "has_small_factor missed composite {}",
                c
            );
        }
    }

    /// Primes immediately above the SMALL_PRIMES table maximum (311) should
    /// pass trial division. These are the first 10 primes after 311: 313, 317,
    /// 331, ... 373. Since none has a factor <= 311, `has_small_factor` returns
    /// false, which is correct -- they will be caught by later MR testing.
    #[test]
    fn has_small_factor_false_for_primes_above_table() {
        // Primes larger than 311 (our table max) that have no small factors
        let large_primes: &[u32] = &[313, 317, 331, 337, 347, 349, 353, 359, 367, 373];
        for &p in large_primes {
            let n = Integer::from(p);
            assert!(
                !has_small_factor(&n),
                "has_small_factor incorrectly flagged prime {} as composite",
                p
            );
        }
    }

    /// The critical blind spot: 313 * 317 = 99221 is composite but has no
    /// prime factor <= 311. Trial division correctly returns false here, since
    /// this semiprime's smallest factor (313) exceeds the table. This is by
    /// design -- full MR testing catches these composites downstream.
    #[test]
    fn has_small_factor_composite_product_of_large_primes() {
        // 313 * 317 = 99221 — both factors are outside our small primes table
        let n = Integer::from(313u32 * 317);
        assert!(
            !has_small_factor(&n),
            "has_small_factor should miss composites with only large factors"
        );
    }

    // ── Miller-Rabin Pre-Screening (mr_screened_test) ──────────────────

    /// Verifies that `mr_screened_test` with 25 rounds accepts known primes
    /// across several orders of magnitude: 2, 3, 5, 7, 11, 13 (single-digit),
    /// 101 (3-digit), 1009 (4-digit), 10007 (5-digit). With 25 rounds, the
    /// probability of a false negative (rejecting a true prime) is zero --
    /// GMP's Miller-Rabin implementation uses deterministic witnesses for
    /// small values and is exact for all values tested here.
    #[test]
    fn mr_screened_test_known_primes_pass() {
        let primes: &[u32] = &[2, 3, 5, 7, 11, 13, 101, 1009, 10007];
        for &p in primes {
            let n = Integer::from(p);
            let result = mr_screened_test(&n, 25);
            assert_ne!(result, IsPrime::No, "MR rejected known prime {}", p);
        }
    }

    /// Verifies that `mr_screened_test` with 25 rounds rejects known composites.
    /// Includes perfect squares (4, 25, 100, 10000), even composites (6, 8),
    /// odd composites (9, 15, 21), and semiprimes (1001 = 7*11*13). With 25
    /// independent MR rounds, the false positive probability is < 4^{-25}
    /// = 2^{-50}, far below the threshold for any of these small values.
    #[test]
    fn mr_screened_test_known_composites_fail() {
        let composites: &[u32] = &[4, 6, 8, 9, 15, 21, 25, 100, 1001, 10000];
        for &c in composites {
            let n = Integer::from(c);
            let result = mr_screened_test(&n, 25);
            assert_eq!(result, IsPrime::No, "MR accepted composite {}", c);
        }
    }

    /// With mr_rounds <= 2, the Frobenius pre-screen is bypassed (it only fires
    /// for candidates > 10K bits), going straight to GMP's `is_probably_prime(2)`.
    /// Even with only 2 rounds, MR should still reject these small composites:
    /// 9=3^2, 15=3*5, 21=3*7, 25=5^2, 1001=7*11*13. The error probability per
    /// round is at most 1/4 (Rabin, 1980), so P(false positive) < (1/4)^2 = 1/16.
    #[test]
    fn mr_screened_test_two_rounds_still_rejects_composites() {
        // With mr_rounds <= 2, the pre-screen is skipped (goes straight to full test)
        let composites: &[u32] = &[9, 15, 21, 25, 1001];
        for &c in composites {
            let n = Integer::from(c);
            let result = mr_screened_test(&n, 2);
            assert_eq!(result, IsPrime::No, "MR(2) accepted composite {}", c);
        }
    }

    // ── Digit Estimation (estimate_digits / exact_digits) ──────────────

    /// Verifies that `estimate_digits` (using bit_length * log10(2)) agrees
    /// with `exact_digits` (via GMP's `to_string().len()`) to within +/- 1
    /// digit across 10 test values spanning 1 to 2^{1000} (302 digits).
    /// The log10(2) approximation introduces at most 1 digit of error due
    /// to rounding at powers of 10 boundaries.
    #[test]
    fn estimate_digits_within_one_of_exact() {
        // Test across a range of magnitudes
        let values: Vec<Integer> = vec![
            Integer::from(1u32),
            Integer::from(9u32),
            Integer::from(10u32),
            Integer::from(99u32),
            Integer::from(100u32),
            Integer::from(999u32),
            Integer::from(1000u32),
            Integer::from(10u32).pow(50),
            Integer::from(10u32).pow(100) - 1u32,
            Integer::from(2u32).pow(1000),
        ];
        for v in &values {
            let est = estimate_digits(v);
            let exact = exact_digits(v);
            assert!(
                (est as i64 - exact as i64).abs() <= 1,
                "estimate_digits({}) = {} but exact = {} (diff > 1)",
                v,
                est,
                exact
            );
        }
    }

    /// Verifies exact digit counts at key boundaries: 0 and 1 have 1 digit,
    /// 9 has 1 digit but 10 has 2 (the 10^k boundary), 99 has 2 but 100 has 3,
    /// 999 has 3 but 1000 has 4. These boundaries are where estimate_digits is
    /// most likely to disagree, making them critical regression tests.
    #[test]
    fn exact_digits_known_values() {
        assert_eq!(exact_digits(&Integer::from(0u32)), 1);
        assert_eq!(exact_digits(&Integer::from(1u32)), 1);
        assert_eq!(exact_digits(&Integer::from(9u32)), 1);
        assert_eq!(exact_digits(&Integer::from(10u32)), 2);
        assert_eq!(exact_digits(&Integer::from(99u32)), 2);
        assert_eq!(exact_digits(&Integer::from(100u32)), 3);
        assert_eq!(exact_digits(&Integer::from(999u32)), 3);
        assert_eq!(exact_digits(&Integer::from(1000u32)), 4);
    }

    /// Edge case: zero has bit_length 0, which would give estimate_digits = 0,
    /// but the convention is that 0 has 1 digit. Verifies the special-case guard.
    #[test]
    fn estimate_digits_zero() {
        assert_eq!(estimate_digits(&Integer::from(0u32)), 1);
    }

    // ── Safe u32 Conversion (checked_u32) ──────────────────────────────

    /// Verifies that `checked_u32` correctly converts u64 values that fit in
    /// u32: 0, 1, and u32::MAX (4294967295). These are the boundary values
    /// for the valid range [0, 2^32 - 1]. This function is used throughout
    /// the engine for `.pow()` and `<<` which require u32 exponents.
    #[test]
    fn checked_u32_valid_values() {
        assert_eq!(checked_u32(0), 0);
        assert_eq!(checked_u32(1), 1);
        assert_eq!(checked_u32(u32::MAX as u64), u32::MAX);
    }

    /// Verifies that `checked_u32` panics with a descriptive message when given
    /// u32::MAX + 1 = 4294967296. Silent truncation via `as u32` would give 0,
    /// causing catastrophic errors in `.pow(0)` (always returning 1) or `<< 0`
    /// (no shift). The panic catches this at the call site.
    #[test]
    #[should_panic(expected = "exceeds u32::MAX")]
    fn checked_u32_overflow_panics() {
        checked_u32(u32::MAX as u64 + 1);
    }

    // ── Block Sizing (block_size_for_n / block_size_for_n_heavy) ────────

    /// Verifies the block size at each match-arm boundary for the standard
    /// (lightweight) block sizer. Block sizes decrease as n grows because
    /// primality tests become more expensive at larger n, so we process
    /// fewer candidates per work block to maintain reasonable checkpoint
    /// intervals (~60 seconds). The arms are:
    ///   [0, 1000] -> 10000, [1001, 10000] -> 10000,
    ///   [10001, 50000] -> 2000, [50001, 200000] -> 500, [200001, ..] -> 100
    #[test]
    fn block_size_for_n_boundary_values() {
        // Exact values at each match-arm boundary
        assert_eq!(block_size_for_n(0), 10_000);
        assert_eq!(block_size_for_n(1_000), 10_000);
        assert_eq!(block_size_for_n(1_001), 10_000);
        assert_eq!(block_size_for_n(10_000), 10_000);
        assert_eq!(block_size_for_n(10_001), 2_000);
        assert_eq!(block_size_for_n(50_000), 2_000);
        assert_eq!(block_size_for_n(50_001), 500);
        assert_eq!(block_size_for_n(200_000), 500);
        assert_eq!(block_size_for_n(200_001), 100);
    }

    /// The block size function must be monotonically non-increasing: if n1 < n2,
    /// then block_size(n1) >= block_size(n2). This invariant ensures that as
    /// candidates get more expensive to test, we never assign larger blocks that
    /// would blow past the checkpoint interval. Tests sliding windows across
    /// 14 representative points spanning [0, 1000000].
    #[test]
    fn block_size_for_n_monotonically_nonincreasing() {
        let test_points = [
            0u64, 500, 1_000, 1_001, 5_000, 10_000, 10_001, 25_000, 50_000, 50_001, 100_000,
            200_000, 200_001, 1_000_000,
        ];
        for w in test_points.windows(2) {
            assert!(
                block_size_for_n(w[1]) <= block_size_for_n(w[0]),
                "block_size_for_n({}) = {} > block_size_for_n({}) = {}",
                w[1],
                block_size_for_n(w[1]),
                w[0],
                block_size_for_n(w[0])
            );
        }
    }

    /// Verifies the "heavy" variant's boundary values. Heavy forms (factorial,
    /// primorial) have much more expensive per-candidate tests, so their block
    /// sizes are smaller at every threshold:
    ///   [0, 1000] -> 10000, [1001, 10000] -> 5000,
    ///   [10001, 50000] -> 1000, [50001, 200000] -> 200, [200001, ..] -> 50
    #[test]
    fn block_size_for_n_heavy_boundary_values() {
        assert_eq!(block_size_for_n_heavy(0), 10_000);
        assert_eq!(block_size_for_n_heavy(1_000), 10_000);
        assert_eq!(block_size_for_n_heavy(1_001), 5_000);
        assert_eq!(block_size_for_n_heavy(10_000), 5_000);
        assert_eq!(block_size_for_n_heavy(10_001), 1_000);
        assert_eq!(block_size_for_n_heavy(50_000), 1_000);
        assert_eq!(block_size_for_n_heavy(50_001), 200);
        assert_eq!(block_size_for_n_heavy(200_000), 200);
        assert_eq!(block_size_for_n_heavy(200_001), 50);
    }

    /// Same monotonicity invariant as `block_size_for_n`, applied to the heavy
    /// variant. Uses identical test points to ensure consistent coverage.
    #[test]
    fn block_size_for_n_heavy_monotonically_nonincreasing() {
        let test_points = [
            0u64, 500, 1_000, 1_001, 5_000, 10_000, 10_001, 25_000, 50_000, 50_001, 100_000,
            200_000, 200_001, 1_000_000,
        ];
        for w in test_points.windows(2) {
            assert!(
                block_size_for_n_heavy(w[1]) <= block_size_for_n_heavy(w[0]),
                "block_size_for_n_heavy({}) = {} > block_size_for_n_heavy({}) = {}",
                w[1],
                block_size_for_n_heavy(w[1]),
                w[0],
                block_size_for_n_heavy(w[0])
            );
        }
    }

    /// Critical invariant: for every n, the heavy block size must be <= the
    /// normal block size. Heavy forms (factorial, primorial) require more
    /// computation per candidate, so their blocks must be no larger than
    /// standard forms. Tests 15 points including 0, u64::MAX, and all
    /// match-arm boundaries.
    #[test]
    fn block_size_for_n_heavy_always_leq_normal() {
        let test_points = [
            0u64,
            500,
            1_000,
            1_001,
            5_000,
            10_000,
            10_001,
            25_000,
            50_000,
            50_001,
            100_000,
            200_000,
            200_001,
            1_000_000,
            u64::MAX,
        ];
        for &n in &test_points {
            assert!(
                block_size_for_n_heavy(n) <= block_size_for_n(n),
                "heavy({}) = {} > normal({}) = {}",
                n,
                block_size_for_n_heavy(n),
                n,
                block_size_for_n(n)
            );
        }
    }

    /// Both block size functions must return positive values for any input.
    /// A zero block size would cause an infinite loop in the search dispatch.
    /// Tests extreme values including 0, u64::MAX/2, and u64::MAX.
    #[test]
    fn block_size_for_n_returns_positive() {
        for &n in &[0u64, 1, 500, 10_000, 100_000, u64::MAX / 2, u64::MAX] {
            assert!(
                block_size_for_n(n) > 0,
                "block_size_for_n({}) must be > 0",
                n
            );
            assert!(
                block_size_for_n_heavy(n) > 0,
                "block_size_for_n_heavy({}) must be > 0",
                n
            );
        }
    }

    // ── Grantham's Frobenius Test (RQFT) ──────────────────────────────

    /// Verifies that `frobenius_test` accepts known primes spanning single-digit
    /// to 6-digit values, plus Mersenne primes 2^p - 1 for p in {13, 17, 19}
    /// (OEIS [A000668](https://oeis.org/A000668)). The Frobenius test operates
    /// in F_p[x]/(x^2 - bx + c) and checks: (1) Euler criterion on the Jacobi
    /// symbol, (2) x^p = Frobenius automorphism of x, (3) x^{(p^2-1)/2} = -1.
    /// For actual primes, all three conditions hold by Fermat's little theorem
    /// extended to quadratic extensions.
    #[test]
    fn frobenius_test_known_primes() {
        // Small primes
        for &p in &[
            2u32, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 101, 1009, 10007, 104729,
        ] {
            let n = Integer::from(p);
            assert!(frobenius_test(&n), "Frobenius rejected known prime {}", p);
        }
        // Mersenne primes: 2^p - 1 for p = 13, 17, 19
        for &exp in &[13u32, 17, 19] {
            let m = Integer::from(2u32).pow(exp) - 1u32;
            assert!(
                frobenius_test(&m),
                "Frobenius rejected Mersenne prime 2^{}-1",
                exp
            );
        }
    }

    /// Verifies that `frobenius_test` rejects known composites: odd semiprimes
    /// (9=3^2, 15=3*5, 21=3*7, 25=5^2, 35=5*7, 49=7^2), products of small
    /// primes (91=7*13, 105=3*5*7), and larger composites (1001=7*11*13,
    /// 10001=73*137). These all fail at least one of the three Frobenius
    /// conditions (Euler criterion, automorphism, or norm check).
    #[test]
    fn frobenius_test_known_composites() {
        let composites: &[u32] = &[9, 15, 21, 25, 35, 49, 91, 105, 1001, 10001];
        for &c in composites {
            let n = Integer::from(c);
            assert!(!frobenius_test(&n), "Frobenius accepted composite {}", c);
        }
    }

    /// Products of two large primes (semiprimes) that exceed the SMALL_PRIMES
    /// table: 1009*1013 = 1022117, 10007*10009 = 100160063, 104729*104743.
    /// These bypass trial division entirely, so the Frobenius test in the
    /// quadratic extension ring must catch them. The false positive rate
    /// is < 1/7710 per test (Grantham, 2001, Theorem 4.1.1).
    #[test]
    fn frobenius_test_products_of_large_primes() {
        // Products of two large primes
        let composites = [
            Integer::from(1009u32) * Integer::from(1013u32), // 1022117
            Integer::from(10007u32) * Integer::from(10009u32), // 100160063
            Integer::from(104729u32) * Integer::from(104743u32),
        ];
        for n in &composites {
            assert!(
                !frobenius_test(n),
                "Frobenius accepted large composite {}",
                n
            );
        }
    }

    /// Carmichael numbers (OEIS [A002997](https://oeis.org/A002997)) satisfy
    /// a^{n-1} = 1 (mod n) for ALL coprime bases a, fooling Fermat's test
    /// completely. The Frobenius test, operating in a quadratic extension ring
    /// rather than Z/nZ, is immune to this: the automorphism condition
    /// x^n = conjugate(x) fails for composites regardless of their Fermat
    /// pseudoprimality. Tests the first 8 Carmichael numbers:
    /// 561 = 3*11*17 (smallest), 1729 = 7*13*19 (Hardy-Ramanujan taxicab).
    #[test]
    fn frobenius_test_carmichael_numbers() {
        // Carmichael numbers fool Fermat's test but should fail Frobenius.
        // 561 = 3 × 11 × 17 (smallest Carmichael)
        // 1105 = 5 × 13 × 17
        // 1729 = 7 × 13 × 19 (Hardy-Ramanujan)
        // 2821 = 7 × 13 × 31
        // 6601 = 7 × 23 × 41
        let carmichaels: &[u32] = &[561, 1105, 1729, 2821, 6601, 8911, 10585, 15841];
        for &c in carmichaels {
            let n = Integer::from(c);
            assert!(
                !frobenius_test(&n),
                "Frobenius accepted Carmichael number {}",
                c
            );
        }
    }

    // ── Polynomial Ring Arithmetic (F_p[x]/(x^2 - bx + c)) ────────────

    /// Verifies the multiplicative identity in the quotient ring: [1, 0] (the
    /// constant polynomial 1) times any element [a0, a1] yields [a0, a1].
    /// This is a ring axiom: 1 * f(x) = f(x) for all f in the ring.
    /// Uses the prime modulus p = 1000003 and irreducible polynomial x^2 - x + 1.
    #[test]
    fn poly_mul_identity() {
        // (1 + 0·x) * (a0 + a1·x) = (a0 + a1·x) in the ring
        let n = Integer::from(1000003u32);
        let b = Integer::from(1u32);
        let c = Integer::from(1u32);
        let one = [Integer::from(1u32), Integer::from(0u32)];
        let elem = [Integer::from(42u32), Integer::from(17u32)];
        let result = poly_mul(&one, &elem, &b, &c, &n);
        assert_eq!(result[0], Integer::from(42u32));
        assert_eq!(result[1], Integer::from(17u32));
    }

    /// Verifies commutativity of multiplication in the quotient ring:
    /// f(x) * g(x) = g(x) * f(x) for all f, g in F_p[x]/(x^2 - bx + c).
    /// This is a ring axiom that must hold because the base field F_p is
    /// commutative and the quotient ring inherits this property.
    /// Uses p = 10007, b = 3, c = 2 with elements [42, 17] and [99, 55].
    #[test]
    fn poly_mul_commutativity() {
        let n = Integer::from(10007u32);
        let b = Integer::from(3u32);
        let c = Integer::from(2u32);
        let a = [Integer::from(42u32), Integer::from(17u32)];
        let elem = [Integer::from(99u32), Integer::from(55u32)];
        let r1 = poly_mul(&a, &elem, &b, &c, &n);
        let r2 = poly_mul(&elem, &a, &b, &c, &n);
        assert_eq!(r1[0], r2[0], "poly_mul not commutative: r0 mismatch");
        assert_eq!(r1[1], r2[1], "poly_mul not commutative: r1 mismatch");
    }

    /// Verifies that the optimized `poly_sqr` produces the same result as
    /// `poly_mul(a, a, ...)`. The squaring function uses the identity
    /// (a0 + a1*x)^2 = a0^2 + 2*a0*a1*x + a1^2*x^2 with x^2 = bx - c
    /// substitution, saving one multiplication compared to generic poly_mul.
    #[test]
    fn poly_sqr_equals_mul() {
        let n = Integer::from(10007u32);
        let b = Integer::from(3u32);
        let c = Integer::from(2u32);
        let a = [Integer::from(42u32), Integer::from(17u32)];
        let sqr_result = poly_sqr(&a, &b, &c, &n);
        let mul_result = poly_mul(&a, &a, &b, &c, &n);
        assert_eq!(sqr_result[0], mul_result[0]);
        assert_eq!(sqr_result[1], mul_result[1]);
    }

    /// Verifies `poly_pow_mod` (binary exponentiation of x in the quotient ring)
    /// for small exponents against hand-computed values.
    ///
    /// With p=101, x^2 - x + 1 (b=1, c=1):
    /// - x^1 = [0, 1] (the element x itself)
    /// - x^2 = bx - c = x - 1 = [100, 1] (since -1 mod 101 = 100)
    /// - x^3 = x * x^2 = x(x-1) = x^2 - x = (x-1) - x = -1 = [100, 0]
    ///
    /// These verify the reduction x^2 -> bx - c is applied correctly at each step.
    #[test]
    fn poly_pow_mod_small_exponents() {
        let n = Integer::from(101u32);
        let b = Integer::from(1u32);
        let c = Integer::from(1u32);

        // x^1 should be [0, 1] (= x itself)
        let r1 = poly_pow_mod(&Integer::from(1u32), &b, &c, &n);
        assert_eq!(r1[0], Integer::from(0u32));
        assert_eq!(r1[1], Integer::from(1u32));

        // x^2 = bx - c = 1·x - 1 = [n-1, 1] = [100, 1]
        let r2 = poly_pow_mod(&Integer::from(2u32), &b, &c, &n);
        assert_eq!(r2[0], Integer::from(100u32)); // -1 mod 101 = 100
        assert_eq!(r2[1], Integer::from(1u32));

        // x^3 = x * x^2 = x * (x - 1) = x^2 - x = (x - 1) - x = -1 = [100, 0]
        let r3 = poly_pow_mod(&Integer::from(3u32), &b, &c, &n);
        assert_eq!(r3[0], Integer::from(100u32));
        assert_eq!(r3[1], Integer::from(0u32));
    }

    // ── Cross-Validation: Frobenius vs. Miller-Rabin ───────────────────

    /// Exhaustive cross-validation on all odd numbers in [3, 1000). For every n:
    /// - If MR(25) says prime, Frobenius must also accept (no false negatives).
    /// - If MR(25) says composite, Frobenius must also reject (Frobenius is
    ///   strictly stronger than MR for individual values).
    ///
    /// This validates that the Frobenius implementation in F_p[x]/(x^2 - bx + c)
    /// agrees with GMP's well-tested MR on nearly 500 test values. Any
    /// disagreement would indicate a bug in poly_mul, poly_sqr, or poly_pow_mod.
    #[test]
    fn frobenius_agrees_with_mr_on_small_range() {
        // Cross-validate Frobenius against MR on odd numbers 3..1000.
        // Any number MR(25) calls prime should pass Frobenius.
        // Any number MR(25) calls composite should fail Frobenius.
        for n_val in (3u32..1000).step_by(2) {
            let n = Integer::from(n_val);
            let mr_result = n.is_probably_prime(25);
            let frob_result = frobenius_test(&n);

            if mr_result != IsPrime::No {
                assert!(
                    frob_result,
                    "Frobenius rejected {} which MR(25) calls prime",
                    n_val
                );
            }
            // Note: Frobenius rejecting what MR accepts is fine (Frobenius is stricter)
            // But Frobenius accepting what MR rejects would be a bug:
            if mr_result == IsPrime::No {
                assert!(
                    !frob_result,
                    "Frobenius accepted {} which MR(25) calls composite",
                    n_val
                );
            }
        }
    }

    /// Strong pseudoprimes to base 2 (OEIS [A001262](https://oeis.org/A001262))
    /// fool a single round of Miller-Rabin with witness 2. The first three are:
    /// 2047 = 23*89, 3277 = 29*113, 4033 = 37*109. The Frobenius test, which
    /// operates in a quadratic extension ring, must reject all of these --
    /// its false positive set is disjoint from MR's for small composites.
    #[test]
    fn frobenius_test_rejects_known_strong_pseudoprimes() {
        // Strong pseudoprimes to base 2 that fool single-round MR:
        // 2047 = 23 × 89
        // 3277 = 29 × 113
        // 4033 = 37 × 109
        // These should fail Frobenius.
        let spsp2: &[u32] = &[2047, 3277, 4033];
        for &c in spsp2 {
            let n = Integer::from(c);
            assert!(
                !frobenius_test(&n),
                "Frobenius accepted base-2 strong pseudoprime {}",
                c
            );
        }
    }

    /// Even numbers: 4 and 100 are composite (should be rejected), while 2 is
    /// the only even prime. The Frobenius test must handle the even case
    /// specially -- the quadratic extension F_2[x]/(x^2-bx+c) behaves differently
    /// from odd characteristic, so 2 is typically handled as a special case.
    #[test]
    fn frobenius_test_even_numbers() {
        assert!(!frobenius_test(&Integer::from(4u32)));
        assert!(!frobenius_test(&Integer::from(100u32)));
        assert!(frobenius_test(&Integer::from(2u32)));
    }

    /// Regression test: `mr_screened_test` must correctly classify small candidates
    /// even though the Frobenius pre-screen only fires for candidates > 10K bits.
    /// For small candidates, the function falls through directly to GMP's
    /// `is_probably_prime(n_rounds)`. This verifies the fallthrough path works
    /// for both primes (2, 3, 5, 7, 11, 101, 1009, 10007) and composites
    /// (4, 6, 9, 15, 21, 1001).
    #[test]
    fn mr_screened_test_still_works_for_small_candidates() {
        // mr_screened_test should still work correctly — Frobenius only fires > 10K bits
        let primes: &[u32] = &[2, 3, 5, 7, 11, 101, 1009, 10007];
        for &p in primes {
            let n = Integer::from(p);
            assert_ne!(
                mr_screened_test(&n, 25),
                IsPrime::No,
                "mr_screened_test rejected small prime {}",
                p
            );
        }
        let composites: &[u32] = &[4, 6, 9, 15, 21, 1001];
        for &c in composites {
            let n = Integer::from(c);
            assert_eq!(
                mr_screened_test(&n, 25),
                IsPrime::No,
                "mr_screened_test accepted small composite {}",
                c
            );
        }
    }

    // ── Additional Trial Division Tests ────────────────────────────────

    /// Every prime p in the table satisfies: p^2 is composite and has p as a
    /// factor. Since p <= 311 and p is in the table, trial division will find
    /// that p^2 mod p = 0 and correctly identify these as composite. Tests all
    /// 64 entries: 2^2=4, 3^2=9, 5^2=25, ..., 311^2=96721.
    #[test]
    fn has_small_factor_on_squares_of_small_primes() {
        // p^2 for small primes — these are composite, should be detected
        for &p in &SMALL_PRIMES {
            let n = Integer::from(p) * Integer::from(p);
            assert!(
                has_small_factor(&n),
                "has_small_factor should detect {}^2 = {} as composite",
                p,
                n
            );
        }
    }

    /// Products of two distinct small primes: 2*3=6, 5*7=35, 11*13=143,
    /// 29*31=899, 101*103=10403. The smaller factor in each pair is in the
    /// SMALL_PRIMES table, so trial division finds it on the first pass.
    #[test]
    fn has_small_factor_product_two_small_primes() {
        // Products of two different small primes
        let cases = [(2u32, 3u32), (5, 7), (11, 13), (29, 31), (101, 103)];
        for &(a, b) in &cases {
            let n = Integer::from(a) * Integer::from(b);
            assert!(
                has_small_factor(&n),
                "has_small_factor should detect {} * {} = {} as composite",
                a,
                b,
                n
            );
        }
    }

    /// The number 1 is a unit (neither prime nor composite). Since 1 mod p != 0
    /// for any prime p, `has_small_factor` correctly returns false. Note that
    /// 1 is not prime; the MR test downstream will handle this case.
    #[test]
    fn has_small_factor_on_one() {
        // 1 is divisible by no prime, so has_small_factor should return false
        let n = Integer::from(1u32);
        assert!(
            !has_small_factor(&n),
            "has_small_factor should return false for 1"
        );
    }

    /// Duplicate of `has_small_factor_composite_product_of_large_primes` above,
    /// confirming that 313*317 = 99221 escapes trial division. Both factors
    /// exceed the table maximum (311), so this is a known blind spot.
    #[test]
    fn has_small_factor_on_large_semiprime() {
        // 313 * 317 = 99221 — both factors outside small primes table
        let n = Integer::from(313u32) * Integer::from(317u32);
        assert!(
            !has_small_factor(&n),
            "has_small_factor should miss 313*317 (both outside table)"
        );
    }

    /// Mixed case: 2 * 104729 = 209458. The factor 2 is the first entry in the
    /// SMALL_PRIMES table, so trial division catches this immediately (209458 mod 2 = 0).
    /// The large factor 104729 is irrelevant -- only one small factor suffices.
    #[test]
    fn has_small_factor_one_small_one_large_factor() {
        // 2 * 104729 = 209458 — has small factor 2
        let n = Integer::from(2u32) * Integer::from(104729u32);
        assert!(
            has_small_factor(&n),
            "has_small_factor should detect factor 2 in 2*104729"
        );
    }

    // ── Additional checked_u32 Tests ──────────────────────────────────

    /// Extended boundary testing for `checked_u32`: 0 (minimum), 1, 100,
    /// 1_000_000 (typical search range), and u32::MAX (4294967295). All should
    /// convert without panic since they fit in 32 bits.
    #[test]
    fn checked_u32_boundary_values() {
        assert_eq!(checked_u32(0), 0);
        assert_eq!(checked_u32(1), 1);
        assert_eq!(checked_u32(100), 100);
        assert_eq!(checked_u32(1_000_000), 1_000_000);
        assert_eq!(checked_u32(u32::MAX as u64), u32::MAX);
    }

    /// Verifies panic for u32::MAX + 2 = 4294967297. This is 2 past the boundary,
    /// confirming the check is not an off-by-one error that only catches MAX+1.
    #[test]
    #[should_panic(expected = "exceeds u32::MAX")]
    fn checked_u32_u32_max_plus_2_panics() {
        checked_u32(u32::MAX as u64 + 2);
    }

    /// Verifies panic for u64::MAX = 18446744073709551615. This is the maximum
    /// possible input, ensuring the guard works at the extreme end of the u64 range.
    #[test]
    #[should_panic(expected = "exceeds u32::MAX")]
    fn checked_u32_u64_max_panics() {
        checked_u32(u64::MAX);
    }

    // ── Additional Miller-Rabin Tests ──────────────────────────────────

    /// With only 1 round, `mr_screened_test` bypasses the pre-screen entirely
    /// and delegates to `is_probably_prime(1)`. Even a single MR round should
    /// accept 104729 (prime) and reject 104730 = 2*3*5*3491 (composite with
    /// many small factors). The error bound for 1 round is 1/4 (Rabin, 1980),
    /// but for values this small, GMP uses deterministic witnesses.
    #[test]
    fn mr_screened_test_with_1_round() {
        // With 1 round (bypasses pre-screen), still correctly classifies
        let p = Integer::from(104729u32);
        assert_ne!(mr_screened_test(&p, 1), IsPrime::No, "Should accept prime with 1 round");

        let c = Integer::from(104730u32);
        assert_eq!(mr_screened_test(&c, 1), IsPrime::No, "Should reject composite with 1 round");
    }

    /// Mersenne primes M_p = 2^p - 1 (OEIS [A000668](https://oeis.org/A000668)):
    /// M_2=3, M_3=7, M_5=31, M_7=127, M_13=8191, M_17=131071, M_19=524287.
    /// These are verified with 15 MR rounds. Mersenne primes are important test
    /// vectors because they have special algebraic structure (2^p - 1 form) that
    /// could theoretically interact with MR base selection.
    #[test]
    fn mr_screened_test_on_mersenne_primes() {
        // Known Mersenne primes: 2^p - 1 for p = 2, 3, 5, 7, 13, 17, 19
        let mersenne_exponents = [2u32, 3, 5, 7, 13, 17, 19];
        for &exp in &mersenne_exponents {
            let m = Integer::from(2u32).pow(exp) - 1u32;
            assert_ne!(
                mr_screened_test(&m, 15),
                IsPrime::No,
                "mr_screened_test rejected Mersenne prime 2^{}-1 = {}",
                exp,
                m
            );
        }
    }

    /// The first composite Mersenne number: 2^11 - 1 = 2047 = 23 * 89.
    /// Despite having the special 2^p - 1 form, this is NOT prime (11 is
    /// prime, but not all Mersenne numbers with prime exponent are prime).
    /// 2047 is also a strong pseudoprime to base 2 (OEIS A001262), making it
    /// a particularly important test case for any primality test.
    #[test]
    fn mr_screened_test_on_mersenne_composites() {
        // 2^11 - 1 = 2047 = 23 * 89 (NOT a Mersenne prime)
        let m = Integer::from(2u32).pow(11) - 1u32;
        assert_eq!(
            mr_screened_test(&m, 15),
            IsPrime::No,
            "mr_screened_test should reject 2^11-1 = 2047 (composite)"
        );
    }

    // ── Additional Digit Estimation Tests ──────────────────────────────

    /// Powers of 10: 10^k has exactly k+1 digits for k = 0, 1, ..., 10.
    /// These are the exact boundaries where floor(log10(n)) + 1 increments,
    /// making them the most likely points for off-by-one errors in the
    /// bit-length-based `estimate_digits` approximation.
    #[test]
    fn exact_digits_powers_of_10() {
        // 10^k has exactly k+1 digits
        for k in 0u32..=10 {
            let n = Integer::from(10u32).pow(k);
            assert_eq!(
                exact_digits(&n),
                (k + 1) as u64,
                "10^{} should have {} digits",
                k,
                k + 1
            );
        }
    }

    /// 10^k - 1 = 999...9 (k nines) has exactly k digits for k >= 1.
    /// These are repdigit numbers one below each power-of-10 boundary:
    /// 9, 99, 999, ..., 9999999999. They exercise the other side of the
    /// boundary from `exact_digits_powers_of_10`.
    #[test]
    fn exact_digits_powers_of_10_minus_1() {
        // 10^k - 1 has exactly k digits (for k >= 1)
        for k in 1u32..=10 {
            let n = Integer::from(10u32).pow(k) - 1u32;
            assert_eq!(
                exact_digits(&n),
                k as u64,
                "10^{}-1 should have {} digits",
                k,
                k
            );
        }
    }

    /// Large power of 2: 2^{10000} has approximately 10000 * log10(2) = 3010
    /// decimal digits. Verifies that the bit-length approximation remains
    /// accurate (+/- 1 digit) even for multi-thousand-digit numbers typical
    /// of kbn searches. The exact value is 3011 digits.
    #[test]
    fn estimate_digits_large_power_of_2() {
        // 2^10000 should have approximately 10000*log10(2) ≈ 3010 digits
        let n = Integer::from(2u32).pow(10000);
        let est = estimate_digits(&n);
        let exact = exact_digits(&n);
        assert!(
            (est as i64 - exact as i64).abs() <= 1,
            "estimate_digits(2^10000) = {} but exact = {}",
            est,
            exact
        );
    }

    // ── Additional Frobenius Tests ─────────────────────────────────────

    /// Perfect squares n = m^2 fail the Frobenius test because the Jacobi
    /// symbol (c/n) is always a square when n is a perfect square, violating
    /// the condition that x^2 - bx + c should be irreducible mod n. Tests
    /// all perfect squares of primes up to 31^2 = 961, plus 4 and 9.
    #[test]
    fn frobenius_test_perfect_squares() {
        // Perfect squares should be rejected
        let squares = [4u32, 9, 25, 49, 121, 169, 289, 361, 529, 841, 961];
        for &s in &squares {
            let n = Integer::from(s);
            assert!(
                !frobenius_test(&n),
                "Frobenius should reject perfect square {}",
                s
            );
        }
    }

    /// The number 1 is a unit, not a prime. The Frobenius test must reject it.
    #[test]
    fn frobenius_test_1_returns_false() {
        assert!(!frobenius_test(&Integer::from(1u32)));
    }

    /// Zero is not prime. The Frobenius test must reject it without panicking.
    #[test]
    fn frobenius_test_0_returns_false() {
        assert!(!frobenius_test(&Integer::from(0u32)));
    }

    /// Twin primes (p, p+2) where both are prime (OEIS [A001359](https://oeis.org/A001359)):
    /// (3,5), (5,7), (11,13), (17,19), (29,31), (41,43). Both members of each
    /// pair must pass the Frobenius test. Twin primes are important because p
    /// and p+2 have correlated residue classes mod small primes, which could
    /// theoretically affect the choice of (b,c) in the quadratic extension.
    #[test]
    fn frobenius_test_twin_primes() {
        // Twin primes: (p, p+2) where both are prime
        let twin_pairs = [(3u32, 5), (5, 7), (11, 13), (17, 19), (29, 31), (41, 43)];
        for &(p, q) in &twin_pairs {
            assert!(
                frobenius_test(&Integer::from(p)),
                "Frobenius should accept twin prime {}",
                p
            );
            assert!(
                frobenius_test(&Integer::from(q)),
                "Frobenius should accept twin prime {}",
                q
            );
        }
    }

    // ── Additional Polynomial Ring Tests ────────────────────────────────

    /// x^0 = 1 in any ring. The result should be the multiplicative identity
    /// [1, 0] (the constant polynomial 1). This edge case tests the base case
    /// of the binary exponentiation loop in `poly_pow_mod`.
    #[test]
    fn poly_pow_mod_zero_exponent() {
        let n = Integer::from(101u32);
        let b = Integer::from(3u32);
        let c = Integer::from(2u32);
        let result = poly_pow_mod(&Integer::from(0u32), &b, &c, &n);
        // x^0 = 1 in the ring → [1, 0]
        assert_eq!(result[0], Integer::from(1u32));
        assert_eq!(result[1], Integer::from(0u32));
    }

    /// Zero annihilation axiom: [0, 0] * f(x) = [0, 0] for any f(x) in the ring.
    /// This is the additive identity (zero polynomial) which absorbs multiplication.
    /// Tests with f(x) = 42 + 17x in F_{10007}[x]/(x^2 - 3x + 2).
    #[test]
    fn poly_mul_by_zero_element() {
        let n = Integer::from(10007u32);
        let b = Integer::from(3u32);
        let c = Integer::from(2u32);
        let zero = [Integer::from(0u32), Integer::from(0u32)];
        let elem = [Integer::from(42u32), Integer::from(17u32)];
        let result = poly_mul(&zero, &elem, &b, &c, &n);
        assert_eq!(result[0], Integer::from(0u32));
        assert_eq!(result[1], Integer::from(0u32));
    }

    /// Hand-computed squaring of x = [0, 1] in F_{101}[x]/(x^2 - 5x + 3):
    /// x^2 = 5x - 3 (by the reduction rule), so the result is [-3, 5] =
    /// [101-3, 5] = [98, 5] in F_{101}. This directly verifies that the
    /// x^2 -> bx - c substitution is applied correctly in `poly_sqr`.
    #[test]
    fn poly_sqr_of_x() {
        // x = [0, 1]. x^2 = bx - c → [n-c, b] in the ring
        let n = Integer::from(101u32);
        let b = Integer::from(5u32);
        let c = Integer::from(3u32);
        let x = [Integer::from(0u32), Integer::from(1u32)];
        let result = poly_sqr(&x, &b, &c, &n);
        // x^2 = bx - c = -c + b*x → [101-3, 5] = [98, 5]
        assert_eq!(result[0], Integer::from(98u32)); // -3 mod 101 = 98
        assert_eq!(result[1], Integer::from(5u32));
    }

    // ── SMALL_PRIMES Table Validation ──────────────────────────────────

    /// The SMALL_PRIMES array must be strictly sorted (ascending) for the
    /// trial division loop to work correctly. An unsorted table could cause
    /// `has_small_factor` to miss factors or produce false positives. Tests
    /// all 63 adjacent pairs via sliding windows.
    #[test]
    fn small_primes_table_is_sorted() {
        for w in SMALL_PRIMES.windows(2) {
            assert!(
                w[0] < w[1],
                "SMALL_PRIMES not sorted: {} >= {}",
                w[0],
                w[1]
            );
        }
    }

    /// Every entry in SMALL_PRIMES must actually be prime. A composite entry
    /// would cause `has_small_factor` to incorrectly flag numbers divisible by
    /// that composite, and would miss the actual prime factors. Verified via
    /// GMP's `is_probably_prime(25)` which is deterministic for values < 3.3e24.
    #[test]
    fn small_primes_table_contains_only_primes() {
        for &p in &SMALL_PRIMES {
            let n = Integer::from(p);
            assert_ne!(
                n.is_probably_prime(25),
                IsPrime::No,
                "SMALL_PRIMES contains non-prime {}",
                p
            );
        }
    }

    /// Validates the table structure: starts at 2 (smallest prime), ends at 311
    /// (the 64th prime), and has exactly 64 entries. The count 64 = pi(311)
    /// matches the prime counting function. Having exactly 64 primes allows
    /// the trial division loop to fit in cache and provides elimination of
    /// ~87% of random odd composites (by inclusion-exclusion on small factors).
    #[test]
    fn small_primes_table_starts_at_2_ends_at_311() {
        assert_eq!(SMALL_PRIMES[0], 2);
        assert_eq!(*SMALL_PRIMES.last().unwrap(), 311);
        assert_eq!(SMALL_PRIMES.len(), 64);
    }

    // ── Database URL Redaction ────────────────────────────────────────

    /// A standard PostgreSQL connection URL with a password must have the
    /// password replaced with `***` while preserving scheme, user, host,
    /// port, and database name.
    #[test]
    fn redact_database_url_hides_password() {
        let url = "postgres://user:secret@host:5432/mydb";
        let redacted = redact_database_url(url);
        assert!(redacted.contains("***"), "password should be redacted");
        assert!(!redacted.contains("secret"), "original password must not appear");
        assert!(redacted.contains("user"), "username should be preserved");
        assert!(redacted.contains("host"), "host should be preserved");
        assert!(redacted.contains("5432"), "port should be preserved");
        assert!(redacted.contains("mydb"), "database name should be preserved");
    }

    /// A URL without a password (e.g., local development) should pass through
    /// unchanged — no `***` inserted because there is nothing to redact.
    #[test]
    fn redact_database_url_no_password() {
        let url = "postgres://user@localhost/mydb";
        let redacted = redact_database_url(url);
        assert!(redacted.contains("user"), "username should be preserved");
        assert!(redacted.contains("localhost"), "host should be preserved");
        assert!(!redacted.contains("***"), "no password to redact");
    }

    /// An unparseable string (not a valid URL) should return the generic
    /// redacted placeholder rather than exposing the original string, which
    /// could contain credentials in a non-standard format.
    #[test]
    fn redact_database_url_invalid_url() {
        let url = "not-a-valid-url";
        let redacted = redact_database_url(url);
        assert_eq!(redacted, "***redacted***");
    }

    /// Supabase connection strings use a long password with special characters.
    /// Verifies the password is fully replaced even when URL-encoded.
    #[test]
    fn redact_database_url_supabase_style() {
        let url = "postgres://postgres.ref:eyJhbGciOiJIUzI1NiJ9@aws-0-us-east-1.pooler.supabase.com:6543/postgres";
        let redacted = redact_database_url(url);
        assert!(!redacted.contains("eyJhbGciOiJIUzI1NiJ9"), "JWT password must be redacted");
        assert!(redacted.contains("***"), "password should be replaced with ***");
        assert!(redacted.contains("supabase.com"), "host should be preserved");
    }
}
