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
pub mod checkpoint;
pub mod cullen_woodall;
pub mod dashboard;
pub mod db;
pub mod deploy;
pub mod events;
pub mod factorial;
#[cfg(feature = "flint")]
pub mod flint;
pub mod fleet;
pub mod gen_fermat;
pub mod gwnum;
pub mod kbn;
pub mod metrics;
pub mod near_repdigit;
pub mod p1;
pub mod palindromic;
pub mod pg_worker;
pub mod primorial;
pub mod progress;
pub mod project;
pub mod proof;
pub mod pfgw;
pub mod prst;
pub mod repunit;
pub mod search_manager;
pub mod sieve;
pub mod sophie_germain;
pub mod twin;
pub mod verify;
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
#[inline]
pub fn mr_screened_test(candidate: &Integer, mr_rounds: u32) -> rug::integer::IsPrime {
    use rug::integer::IsPrime;
    if mr_rounds > 2 && candidate.is_probably_prime(2) == IsPrime::No {
        return IsPrime::No;
    }
    candidate.is_probably_prime(mr_rounds)
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
}
