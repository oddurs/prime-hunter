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
//! - [`dashboard`], [`db`], [`fleet`], [`checkpoint`], [`progress`], etc.
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
pub mod sieve;
pub mod sophie_germain;
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

/// Trait for coordination clients (HTTP-based WorkerClient or PG-based PgWorkerClient).
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
    use super::*;
    use rug::integer::IsPrime;
    use rug::ops::Pow;

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

    #[test]
    fn has_small_factor_composite_product_of_large_primes() {
        // 313 * 317 = 99221 — both factors are outside our small primes table
        let n = Integer::from(313u32 * 317);
        assert!(
            !has_small_factor(&n),
            "has_small_factor should miss composites with only large factors"
        );
    }

    #[test]
    fn mr_screened_test_known_primes_pass() {
        let primes: &[u32] = &[2, 3, 5, 7, 11, 13, 101, 1009, 10007];
        for &p in primes {
            let n = Integer::from(p);
            let result = mr_screened_test(&n, 25);
            assert_ne!(result, IsPrime::No, "MR rejected known prime {}", p);
        }
    }

    #[test]
    fn mr_screened_test_known_composites_fail() {
        let composites: &[u32] = &[4, 6, 8, 9, 15, 21, 25, 100, 1001, 10000];
        for &c in composites {
            let n = Integer::from(c);
            let result = mr_screened_test(&n, 25);
            assert_eq!(result, IsPrime::No, "MR accepted composite {}", c);
        }
    }

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

    #[test]
    fn estimate_digits_zero() {
        assert_eq!(estimate_digits(&Integer::from(0u32)), 1);
    }

    #[test]
    fn checked_u32_valid_values() {
        assert_eq!(checked_u32(0), 0);
        assert_eq!(checked_u32(1), 1);
        assert_eq!(checked_u32(u32::MAX as u64), u32::MAX);
    }

    #[test]
    #[should_panic(expected = "exceeds u32::MAX")]
    fn checked_u32_overflow_panics() {
        checked_u32(u32::MAX as u64 + 1);
    }

    // ---- block_size_for_n tests ----

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

    // ---- Frobenius test and polynomial ring tests ----

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

    #[test]
    fn frobenius_test_known_composites() {
        let composites: &[u32] = &[9, 15, 21, 25, 35, 49, 91, 105, 1001, 10001];
        for &c in composites {
            let n = Integer::from(c);
            assert!(!frobenius_test(&n), "Frobenius accepted composite {}", c);
        }
    }

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

    #[test]
    fn frobenius_test_even_numbers() {
        assert!(!frobenius_test(&Integer::from(4u32)));
        assert!(!frobenius_test(&Integer::from(100u32)));
        assert!(frobenius_test(&Integer::from(2u32)));
    }

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
}
