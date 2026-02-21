//! Property-based tests for darkreach's mathematical primitives.
//!
//! These tests use the `proptest` framework to verify mathematical invariants
//! hold across thousands of randomly generated inputs. Unlike example-based tests
//! that check specific known values, property tests express universal truths that
//! must hold for all valid inputs, making them excellent at finding edge cases.
//!
//! # Prerequisites
//!
//! - No database or network access required.
//! - These tests are purely computational and always run.
//!
//! # How to run
//!
//! ```bash
//! # Run all property tests:
//! cargo test --test property_tests
//!
//! # Run a specific property:
//! cargo test --test property_tests prop_pow_mod_matches_big_int
//!
//! # Increase case count for thorough testing (default is 256):
//! PROPTEST_CASES=10000 cargo test --test property_tests
//! ```
//!
//! # Testing strategy
//!
//! Properties are organized by module:
//! - **Sieve module**: modular exponentiation, modular inverse, GCD, prime generation,
//!   small factor detection, digit estimation
//! - **Checkpoint module**: serialization/deserialization roundtrip
//! - **Near-repdigit module**: palindrome construction invariant
//! - **Montgomery multiplication**: domain conversion roundtrip, pow_mod equivalence
//!
//! Each property is named `prop_<function>_<invariant>` for clarity. The `proptest!`
//! macro generates the test harness, input strategies, and shrinking logic
//! automatically.
//!
//! # References
//!
//! - proptest: <https://proptest-rs.github.io/proptest/>
//! - QuickCheck (inspiration): Claessen & Hughes, 2000

use proptest::prelude::*;
use rug::ops::Pow;
use rug::Integer;

// == Sieve Module Properties ===================================================
// These properties verify the correctness of low-level arithmetic primitives
// in `sieve.rs` that underpin all primality testing. A bug in any of these
// functions would produce incorrect sieve results or false primality claims.
// ==============================================================================

proptest! {
    /// Verifies modular exponentiation matches arbitrary-precision computation.
    ///
    /// **Mathematical property**: pow_mod(b, e, m) == b^e mod m
    ///
    /// This is the foundational operation for Miller-Rabin, Fermat, and Proth tests.
    /// We compare our u64 implementation against GMP's `pow_mod` on `rug::Integer`
    /// to ensure no overflow or off-by-one errors in the binary exponentiation loop.
    ///
    /// Input ranges: base in [1, 1000), exp in [0, 100), modulus in [2, 10000).
    /// These ranges exercise both small (fits in u64) and moderate (requires
    /// 128-bit intermediate) cases.
    #[test]
    fn prop_pow_mod_matches_big_int(
        base in 1u64..1000,
        exp in 0u64..100,
        modulus in 2u64..10000,
    ) {
        let result = darkreach::sieve::pow_mod(base, exp, modulus);
        let expected = {
            let b = Integer::from(base);
            let m = Integer::from(modulus);
            let e = Integer::from(exp);
            let big_result = b.pow_mod(&e, &m).unwrap();
            big_result.to_u64().unwrap()
        };
        prop_assert_eq!(result, expected,
            "pow_mod({}, {}, {}) = {} but expected {}", base, exp, modulus, result, expected);
    }

    /// Verifies the modular inverse satisfies a * a^(-1) == 1 (mod p).
    ///
    /// **Mathematical property**: For prime p and a not divisible by p,
    /// mod_inverse(a, p) returns a^(-1) such that a * a^(-1) == 1 (mod p).
    ///
    /// This is used in the BSGS (Baby-Step Giant-Step) sieve for computing
    /// discrete logarithms, and in Montgomery multiplication setup.
    ///
    /// We test against the first 20 primes to guarantee p is actually prime
    /// (modular inverse is only guaranteed to exist when gcd(a, p) = 1, which
    /// holds for all a in [1, p-1] when p is prime).
    #[test]
    fn prop_mod_inverse_roundtrip(
        // Use small primes to guarantee p is actually prime
        p_idx in 0usize..20,
        a_mul in 1u64..100,
    ) {
        let small_primes = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29,
                            31, 37, 41, 43, 47, 53, 59, 61, 67, 71];
        let p = small_primes[p_idx];
        let a = (a_mul % (p - 1)) + 1; // a in [1, p-1]

        if let Some(inv) = darkreach::sieve::mod_inverse(a, p) {
            let product = darkreach::sieve::pow_mod(a, 1, p) as u128
                * darkreach::sieve::pow_mod(inv, 1, p) as u128;
            prop_assert_eq!((product % p as u128) as u64, 1,
                "mod_inverse({}, {}) = {}, but a*inv mod p = {}",
                a, p, inv, (product % p as u128));
        }
    }

    /// Verifies GCD is commutative and divides both arguments.
    ///
    /// **Mathematical properties**:
    /// 1. Symmetry: gcd(a, b) == gcd(b, a)
    /// 2. Divisibility: gcd(a, b) | a  AND  gcd(a, b) | b
    ///
    /// GCD is used throughout the sieve for coprimality checks and in the
    /// wheel factorization optimization. The Euclidean algorithm must satisfy
    /// these fundamental properties for all positive inputs.
    #[test]
    fn prop_gcd_symmetric_and_divides(
        a in 1u32..10000,
        b in 1u32..10000,
    ) {
        let g = darkreach::sieve::gcd(a, b);
        let g2 = darkreach::sieve::gcd(b, a);
        prop_assert_eq!(g, g2, "gcd({},{}) != gcd({},{})", a, b, b, a);
        prop_assert_eq!(a % g, 0, "gcd({},{})={} does not divide {}", a, b, g, a);
        prop_assert_eq!(b % g, 0, "gcd({},{})={} does not divide {}", a, b, g, b);
    }

    /// Verifies all values from generate_primes are actually prime.
    ///
    /// **Mathematical property**: For all p in generate_primes(limit),
    /// p is prime AND p <= limit.
    ///
    /// The Sieve of Eratosthenes in `generate_primes` is the foundation for
    /// trial division and sieve pre-computation. We cross-check every returned
    /// value against GMP's Miller-Rabin with 25 rounds (deterministic for
    /// values up to 3.3 * 10^24).
    ///
    /// Input range: limit in [10, 10000). This exercises both very small sieves
    /// (where edge cases around 2, 3 are important) and moderate sieves (where
    /// the bit array logic must work correctly).
    #[test]
    fn prop_generate_primes_all_prime(
        limit in 10u64..10000,
    ) {
        let primes = darkreach::sieve::generate_primes(limit);
        for &p in &primes {
            let n = Integer::from(p);
            prop_assert!(
                n.is_probably_prime(25) != rug::integer::IsPrime::No,
                "{} returned by generate_primes({}) is not prime", p, limit
            );
            prop_assert!(p <= limit, "{} > limit {}", p, limit);
        }
    }

    /// Verifies has_small_factor returns false for known Mersenne primes.
    ///
    /// **Mathematical property**: For known Mersenne primes M_p = 2^p - 1,
    /// has_small_factor(M_p) == false.
    ///
    /// This function is the first filter in the primality testing pipeline.
    /// A false positive (flagging a prime as having a small factor) would cause
    /// the candidate to be skipped, missing a genuine prime discovery.
    ///
    /// We test against the first 7 Mersenne primes: M_2 through M_19.
    /// These are small enough to verify exhaustively but large enough to
    /// exercise the trial division loop.
    #[test]
    fn prop_has_small_factor_false_for_primes(
        // Test with known Mersenne primes 2^p - 1 for small p
        p_idx in 0usize..7,
    ) {
        let mersenne_exps = [2, 3, 5, 7, 13, 17, 19];
        let exp = mersenne_exps[p_idx];
        let prime = Integer::from(1u32) << exp as u32;
        let prime = prime - 1u32;
        prop_assert!(
            !darkreach::has_small_factor(&prime),
            "has_small_factor incorrectly flagged Mersenne prime M{}", exp
        );
    }

    /// Verifies estimate_digits is within 1 of the exact digit count.
    ///
    /// **Mathematical property**: |estimate_digits(n) - exact_digits(n)| <= 1
    ///
    /// `estimate_digits` uses log10 approximation for speed (O(1) vs O(n) for
    /// string conversion). The approximation is used for progress reporting and
    /// search space estimation where off-by-one is acceptable. We verify the
    /// error bound against powers of 2 (which stress the log10 approximation
    /// at digit boundaries like 2^10 = 1024, a 4-digit number).
    ///
    /// Input range: exp in [1, 500), giving numbers from 2 to 2^499 (~150 digits).
    #[test]
    fn prop_estimate_digits_within_one(
        exp in 1u32..500,
    ) {
        let n = Integer::from(2u32).pow(exp);
        let est = darkreach::estimate_digits(&n);
        let exact = darkreach::exact_digits(&n);
        let diff = if est > exact { est - exact } else { exact - est };
        prop_assert!(diff <= 1,
            "estimate_digits(2^{}) = {} but exact = {} (diff={})", exp, est, exact, diff);
    }
}

// == Checkpoint Roundtrip ======================================================
// Verifies that the checkpoint serialization format is lossless. Checkpoints
// save search progress every 60 seconds so work can resume after crashes.
// A roundtrip failure would mean lost progress or corrupted resume state.
// ==============================================================================

proptest! {
    /// Verifies checkpoint save/load roundtrip preserves all fields.
    ///
    /// **Property**: For any valid Checkpoint::Factorial { last_n, start, end },
    /// save(path, cp) followed by load(path) returns an identical checkpoint.
    ///
    /// This tests the JSON serialization/deserialization path including:
    /// - Optional fields (start and end can be None)
    /// - Atomic file writes (save uses write-then-rename)
    /// - Large values (up to 1,000,000)
    ///
    /// Uses `tempfile::tempdir()` for isolated filesystem access.
    #[test]
    fn prop_checkpoint_roundtrip(
        last_n in 0u64..1_000_000,
        start in proptest::option::of(0u64..1_000_000),
        end in proptest::option::of(0u64..1_000_000),
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prop_test.json");

        let cp = darkreach::checkpoint::Checkpoint::Factorial { last_n, start, end };
        darkreach::checkpoint::save(&path, &cp).unwrap();

        let loaded = darkreach::checkpoint::load(&path).unwrap();
        match loaded {
            darkreach::checkpoint::Checkpoint::Factorial {
                last_n: ln, start: s, end: e,
            } => {
                prop_assert_eq!(ln, last_n);
                prop_assert_eq!(s, start);
                prop_assert_eq!(e, end);
            }
            _ => prop_assert!(false, "Wrong checkpoint variant"),
        }
    }
}

// == Near-Repdigit Palindrome Invariant ========================================
// Verifies that the near-repdigit candidate builder always produces palindromes.
// Near-repdigit primes are palindromes where all digits are the same except
// possibly the middle digit. A bug in the builder would generate non-palindromes
// that waste computation on structurally invalid candidates.
// ==============================================================================

proptest! {
    /// Verifies build_candidate always produces a decimal palindrome.
    ///
    /// **Mathematical property**: For valid (k, d, m) parameters,
    /// build_candidate(k, d, m).to_string() reads the same forwards and backwards.
    ///
    /// A near-repdigit palindrome has the form ddd...d'...ddd where d is the
    /// repeated digit and d' is the (possibly different) middle digit. Parameters:
    /// - k: half-length (number of repeated digits on each side)
    /// - d: the repeated digit (1-9)
    /// - m: the middle digit offset
    ///
    /// Invalid parameter combinations are skipped via `is_valid_params` check.
    /// Zero or negative candidates (which can occur for edge-case parameters)
    /// are also skipped.
    #[test]
    fn prop_build_candidate_is_palindrome(
        k in 1u64..50,
        d in 1u32..5,
        m_pct in 0u64..100,
    ) {
        // m must be <= k, and if m==0, 2*d must be <= 9
        let m = (m_pct * k) / 100; // scale m to [0, k)
        if !darkreach::near_repdigit::is_valid_params(k, d, m) {
            return Ok(());
        }

        let candidate = darkreach::near_repdigit::build_candidate(k, d, m);
        if candidate <= 0 {
            return Ok(());
        }

        let s = candidate.to_string_radix(10);
        let rev: String = s.chars().rev().collect();
        prop_assert_eq!(&s, &rev,
            "build_candidate(k={}, d={}, m={}) = {} is not a palindrome", k, d, m, s);
    }
}

// == Montgomery Multiplication Properties ======================================
// Montgomery multiplication replaces expensive division-based modular reduction
// with cheaper multiply-and-shift operations. These properties verify the
// Montgomery domain conversion is lossless and that Montgomery-space pow_mod
// matches the standard implementation.
//
// Reference: Peter L. Montgomery, "Modular Multiplication Without Trial Division"
// (Mathematics of Computation, 1985).
// ==============================================================================

proptest! {
    /// Verifies Montgomery domain roundtrip: from_mont(to_mont(a)) == a mod n.
    ///
    /// **Mathematical property**: The Montgomery representation maps a -> aR mod n
    /// (where R = 2^64). Converting back gives the original value modulo n.
    ///
    /// This roundtrip must be exact for Montgomery multiplication to be correct.
    /// We test with odd moduli > 1 (Montgomery form requires odd modulus).
    ///
    /// Input ranges: n_half in [1, 50000) -> n = 2*n_half+1 (odd, in [3, 99999]),
    /// a in [0, 100000).
    #[test]
    fn prop_montgomery_roundtrip(
        // Use odd moduli > 1 for Montgomery
        n_half in 1u64..50000,
        a in 0u64..100000,
    ) {
        let n = 2 * n_half + 1; // ensure odd
        let ctx = darkreach::sieve::MontgomeryCtx::new(n);
        let a_mont = ctx.to_mont(a);
        let a_back = ctx.from_mont(a_mont);
        prop_assert_eq!(a_back, a % n,
            "Montgomery roundtrip failed: to_mont/from_mont({}) mod {} = {} (expected {})",
            a, n, a_back, a % n);
    }

    /// Verifies Montgomery pow_mod matches the standard pow_mod implementation.
    ///
    /// **Mathematical property**: For odd modulus n,
    /// montgomery_pow_mod(base, exp, n) == standard_pow_mod(base, exp, n)
    ///
    /// Montgomery pow_mod performs all intermediate multiplications in Montgomery
    /// form, which should produce identical results to standard modular
    /// exponentiation. Any discrepancy indicates a bug in the Montgomery
    /// multiply, reduce, or conversion routines.
    ///
    /// This is the critical correctness property: Montgomery pow_mod is used
    /// in the sieve's modular arithmetic hot path, so any error here propagates
    /// to incorrect sieve results and missed/false primes.
    #[test]
    fn prop_montgomery_pow_mod_matches(
        n_half in 1u64..10000,
        base in 1u64..1000,
        exp in 0u64..100,
    ) {
        let n = 2 * n_half + 1; // ensure odd
        let ctx = darkreach::sieve::MontgomeryCtx::new(n);
        let base_mont = ctx.to_mont(base % n);
        let result_mont = ctx.pow_mod(base_mont, exp);
        let result = ctx.from_mont(result_mont);
        let expected = darkreach::sieve::pow_mod(base, exp, n);
        prop_assert_eq!(result, expected,
            "Montgomery pow_mod({}, {}, {}) = {} but expected {}",
            base, exp, n, result, expected);
    }
}
