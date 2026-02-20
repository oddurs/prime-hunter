//! Property-based tests using proptest.
//!
//! These tests verify mathematical invariants hold across random inputs.
//! Run with: cargo test --test property_tests

use proptest::prelude::*;
use rug::Integer;
use rug::ops::Pow;

// --- Sieve module properties ---

proptest! {
    /// pow_mod(b, e, m) == (b^e) % m for small values
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

    /// mod_inverse(a, p) * a ≡ 1 (mod p) for prime p
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

    /// gcd(a, b) == gcd(b, a) and divides both
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

    /// All elements returned by generate_primes pass Miller-Rabin
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

    /// has_small_factor returns false for known primes
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

    /// estimate_digits is within 1 of exact digit count
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

// --- Checkpoint roundtrip ---

proptest! {
    /// Checkpoint save/load roundtrip preserves data
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

// --- Near-repdigit palindrome invariant ---

proptest! {
    /// build_candidate produces a palindrome (digits read the same forwards and backwards)
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

// --- Montgomery multiplication properties ---

proptest! {
    /// Montgomery roundtrip: from_mont(to_mont(a)) == a % n
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

    /// Montgomery pow_mod matches regular pow_mod
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
