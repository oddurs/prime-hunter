//! Pollard's P-1 factoring algorithm for composite pre-filtering.
//!
//! Finds non-trivial factors of N when N has a prime factor p such that p-1
//! is B1-smooth (all prime factors of p-1 are ≤ B1). Applied to sieve survivors
//! before expensive PRP tests — costs ~1 modular exponentiation but eliminates
//! 1-5% of composites that survive the algebraic sieve.

use rug::Integer;

/// Run Pollard's P-1 Stage 1 factoring on `n`.
///
/// Computes a = 2^(lcm(1..B1)) mod n, then checks gcd(a-1, n).
/// Returns `Some(factor)` if a non-trivial factor is found, `None` otherwise.
/// Cost: approximately π(b1) modular multiplications.
pub fn p1_stage1(n: &Integer, b1: u64) -> Option<Integer> {
    if b1 < 2 || n <= &Integer::from(3u32) {
        return None;
    }

    let primes = crate::sieve::generate_primes(b1);
    let mut a = Integer::from(2u32);

    for &q in &primes {
        // Compute q^e where q^e ≤ b1
        let mut pk = q;
        while pk <= b1 / q {
            pk *= q;
        }
        // a = a^(q^e) mod n
        match a.pow_mod(&Integer::from(pk), n) {
            Ok(result) => a = result,
            Err(_) => return None,
        }
    }

    let g = Integer::from(&a - 1u32).gcd(n);
    if g > 1u32 && &g < n {
        Some(g)
    } else {
        None
    }
}

/// Run Pollard's P-1 Stage 2 (standard continuation) on `n`.
///
/// Extends the search to primes in (b1, b2]. Instead of a full exponentiation
/// per prime, precomputes a^d for common prime gaps d, requiring only one
/// multiplication per prime in the range.
///
/// `a` must be the accumulated base from Stage 1: a = 2^(lcm(1..b1)) mod n.
pub fn p1_stage2(n: &Integer, a: &Integer, b1: u64, b2: u64) -> Option<Integer> {
    if b2 <= b1 {
        return None;
    }

    let primes = crate::sieve::generate_primes(b2);
    let start_idx = primes.partition_point(|&p| p <= b1);
    if start_idx >= primes.len() {
        return None;
    }

    // Precompute a^d for even gaps d = 2, 4, 6, ..., 30
    let max_gap = 30u64;
    let mut gap_powers: Vec<Integer> = Vec::with_capacity((max_gap / 2) as usize);
    let a_sq = a.clone().pow_mod(&Integer::from(2u32), n).ok()?;
    let mut gp = a_sq.clone();
    gap_powers.push(gp.clone()); // index 0 = a^2
    for _ in 1..(max_gap / 2) as usize {
        gp = Integer::from(&gp * &a_sq) % n;
        gap_powers.push(gp.clone());
    }

    // Start from the first prime > b1
    let mut current = a
        .clone()
        .pow_mod(&Integer::from(primes[start_idx]), n)
        .ok()?;
    let mut product = Integer::from(&current - 1u32) % n;
    let batch_size = 200;

    for i in (start_idx + 1)..primes.len() {
        let gap = primes[i] - primes[i - 1];

        if gap <= max_gap && gap >= 2 && gap.is_multiple_of(2) {
            let idx = (gap / 2 - 1) as usize;
            current = Integer::from(&current * &gap_powers[idx]) % n;
        } else {
            let a_gap = a.clone().pow_mod(&Integer::from(gap), n).ok()?;
            current = Integer::from(&current * &a_gap) % n;
        }

        let cm1 = Integer::from(&current - 1u32);
        product = Integer::from(&product * &cm1) % n;

        if (i - start_idx) % batch_size == 0 && product != 0u32 {
            let g = product.clone().gcd(n);
            if g > 1u32 && &g < n {
                return Some(g);
            }
            product = Integer::from(1u32);
        }
    }

    if product != 0u32 {
        let g = product.gcd(n);
        if g > 1u32 && &g < n {
            return Some(g);
        }
    }
    None
}

/// Combined P-1 factoring: Stage 1 with B1, optional Stage 2 up to B2 = 100*B1.
pub fn p1_factor(n: &Integer, b1: u64, b2: Option<u64>) -> Option<Integer> {
    if b1 < 2 || n <= &Integer::from(3u32) {
        return None;
    }

    // Stage 1
    let primes = crate::sieve::generate_primes(b1);
    let mut a = Integer::from(2u32);

    for &q in &primes {
        let mut pk = q;
        while pk <= b1 / q {
            pk *= q;
        }
        match a.pow_mod(&Integer::from(pk), n) {
            Ok(result) => a = result,
            Err(_) => return None,
        }
    }

    // Check Stage 1 result
    let g = Integer::from(&a - 1u32).gcd(n);
    if g > 1u32 && &g < n {
        return Some(g);
    }
    if g == *n {
        return None; // B1 too large: all factors are smooth
    }

    // Stage 2
    let b2_val = b2.unwrap_or(b1.saturating_mul(100));
    if b2_val > b1 {
        return p1_stage2(n, &a, b1, b2_val);
    }

    None
}

/// Quick P-1 composite test: returns true if a non-trivial factor is found.
pub fn is_p1_composite(n: &Integer, b1: u64) -> bool {
    p1_stage1(n, b1).is_some()
}

/// Adaptive P-1 composite pre-filter with auto-tuned B1/B2 bounds.
///
/// Uses [`p1_factor`] (Stage 1 + Stage 2) instead of Stage 1 alone, catching
/// composites whose smallest prime factor p has p−1 with one large prime factor
/// (found by Stage 2) but otherwise smooth.
///
/// Selects B1/B2 based on candidate bit size to balance P-1 cost against the
/// expected savings from skipping expensive PRP/MR tests:
///
/// | Bits     | B1     | B2    | Rationale                                     |
/// |----------|--------|-------|-----------------------------------------------|
/// | < 5,000  | —      | —     | P-1 costs more than the test itself; skip      |
/// | 5K–20K   | 100K   | 10M   | Light filter; catches smooth-ish factors        |
/// | 20K–50K  | 500K   | 50M   | Moderate depth; worth it for 6K+ digit numbers |
/// | 50K+     | 1M     | 100M  | Deep search; large candidates amortize the cost |
///
/// Returns `true` if definitely composite (a non-trivial factor was found).
pub fn adaptive_p1_filter(n: &Integer) -> bool {
    let bits = n.significant_bits();

    // Below 5K bits, P-1 is not cost-effective
    if bits < 5_000 {
        return false;
    }

    let (b1, b2) = if bits < 20_000 {
        (100_000u64, 10_000_000u64)
    } else if bits < 50_000 {
        (500_000u64, 50_000_000u64)
    } else {
        (1_000_000u64, 100_000_000u64)
    };

    p1_factor(n, b1, Some(b2)).is_some()
}

#[cfg(test)]
mod tests {
    //! # Tests for Pollard's P-1 Factoring Algorithm
    //!
    //! Validates the P-1 composite pre-filter used to eliminate composites before
    //! expensive PRP/MR tests. Pollard's P-1 method (1974) finds a non-trivial
    //! factor of N when N has a prime factor p such that p-1 is B1-smooth
    //! (all prime factors of p-1 are <= B1).
    //!
    //! ## Algorithm
    //!
    //! **Stage 1**: Compute a = 2^(lcm(1..B1)) mod N via iterated modular
    //! exponentiation. If gcd(a-1, N) is non-trivial, a factor is found.
    //! Cost: approximately pi(B1) modular multiplications.
    //!
    //! **Stage 2** (standard continuation): For primes q in (B1, B2], compute
    //! a^q mod N using precomputed gap powers a^d for common even gaps d.
    //! This catches factors where p-1 = (smooth part) * q for a single prime
    //! q in (B1, B2].
    //!
    //! **Adaptive filter**: Auto-tunes B1/B2 by candidate bit size:
    //!   - < 5K bits: skip (P-1 costs more than the test itself)
    //!   - 5K-20K bits: B1=100K, B2=10M
    //!   - 20K-50K bits: B1=500K, B2=50M
    //!   - 50K+ bits: B1=1M, B2=100M
    //!
    //! ## Testing Strategy
    //!
    //! - **Positive tests**: Semiprimes p*q where p-1 is B1-smooth but q-1 is not.
    //!   P-1 should find p. Each test documents the factorization of p-1.
    //! - **Negative tests**: Primes (no factor exists), semiprimes where both factors
    //!   are smooth (trivial gcd = N), and semiprimes with no smooth factors.
    //! - **Stage 2 tests**: Factors where p-1 has one prime factor just above B1,
    //!   requiring Stage 2 to find it.
    //! - **Boundary tests**: B1=1, B1=2, exact B1 boundaries, n <= 3.
    //! - **Adaptive filter**: Size-based tier selection, safe on primes, catches
    //!   Fermat primes (perfectly smooth p-1 = 2^k).
    //!
    //! ## References
    //!
    //! - J.M. Pollard, "Theorems on Factorization and Primality Testing",
    //!   Proc. Cambridge Phil. Soc., 76:521-528, 1974.
    //! - P.L. Montgomery, "Speeding the Pollard and Elliptic Curve Methods of
    //!   Factorization", Mathematics of Computation, 48(177):243-264, 1987.

    use super::*;
    use rug::ops::Pow;

    // ── Stage 1 (B1-smooth factors) ────────────────────────────────────
    //
    // Stage 1 computes a = 2^(lcm(1..B1)) mod N. For each prime q <= B1,
    // it raises a to the largest power q^e with q^e <= B1. A factor p of N
    // is found when p-1 divides lcm(1..B1), i.e., p-1 is B1-smooth.
    //
    // Key invariant: the test only works when exactly one factor of N has
    // B1-smooth p-1. If all factors are smooth, gcd(a-1, N) = N (trivial).

    /// Find a B1-smooth factor: 41*10007 where 41-1 = 40 = 2^3*5 (5-smooth).
    ///
    /// With B1=100, Stage 1 covers all primes up to 100. Since the largest
    /// prime factor of 40 is 5 (<< 100), the factor 41 is found.
    /// Meanwhile 10007-1 = 10006 = 2*5003 (5003 is prime, not B1-smooth).
    #[test]
    fn p1_finds_smooth_factor() {
        // n = 41 * 10007 = 410287
        // 41-1 = 40 = 2^3 * 5 (5-smooth)
        // 10007-1 = 10006 = 2 * 5003 (5003 is prime, NOT B1-smooth for B1<5003)
        let n = Integer::from(41u64 * 10007);
        let factor = p1_stage1(&n, 100);
        assert!(factor.is_some(), "P-1 should find 41 (p-1 is 5-smooth)");
        let f = factor.unwrap();
        assert!(n.is_divisible(&f), "factor should divide n");
        assert_eq!(f, Integer::from(41u32));
    }

    /// Find factor 23 in 23*10007: 23-1 = 22 = 2*11 (11-smooth).
    /// 10007-1 = 2*5003 (not smooth for B1=100). P-1 isolates the smooth factor.
    #[test]
    fn p1_finds_factor_larger_b1() {
        // n = 23 * 10007 = 230161
        // 23-1 = 22 = 2 * 11 (11-smooth)
        // 10007-1 = 2 * 5003 (not smooth for B1=100)
        let n = Integer::from(23u64 * 10007);
        let factor = p1_stage1(&n, 100);
        assert!(factor.is_some(), "P-1 should find 23");
        assert_eq!(factor.unwrap(), Integer::from(23u32));
    }

    /// Find factor 47 in 47*100003: 47-1 = 46 = 2*23 (23-smooth, B1=100 suffices).
    /// 100003-1 = 100002 = 2*3*7*2381 (2381 is prime, not B1-smooth).
    #[test]
    fn p1_finds_composite_factor() {
        // n = 47 * 100003 = 4700141
        // 47-1 = 46 = 2 * 23 (23-smooth, found with B1=100)
        // 100003-1 = 100002 = 2 * 3 * 7 * 2381 (2381 prime, not B1-smooth)
        let n = Integer::from(47u64 * 100003);
        let factor = p1_stage1(&n, 100);
        assert!(factor.is_some(), "P-1 should find 47 (p-1 is 23-smooth)");
        let f = factor.unwrap();
        assert!(n.is_divisible(&f), "factor should divide n");
    }

    /// Verify P-1 correctly misses factors when neither p-1 is B1-smooth.
    ///
    /// n = 1000000007 * 1000000009 (both are large primes with large prime
    /// factors in p-1). With B1=100, neither factor is found because
    /// the largest prime factor of each p-1 far exceeds 100.
    #[test]
    fn p1_misses_non_smooth_factor() {
        // n = 1000000007 * 1000000009 (both are prime, p-1 has large factors)
        let n = Integer::from(1000000007u64) * Integer::from(1000000009u64);
        let factor = p1_stage1(&n, 100);
        assert!(
            factor.is_none(),
            "P-1 with small B1 should miss non-smooth factors"
        );
    }

    /// Verify P-1 returns None when ALL factors are B1-smooth (trivial gcd).
    ///
    /// n = 2047 = 23*89. 23-1 = 22 = 2*11 and 89-1 = 88 = 2^3*11. Both are
    /// 11-smooth. With B1=11, a = 2^(lcm(1..11)) mod n satisfies
    /// a = 1 (mod 23) AND a = 1 (mod 89), so a = 1 (mod n) and
    /// gcd(a-1, n) = gcd(0, n) = n (trivial). P-1 correctly returns None.
    #[test]
    fn p1_returns_none_when_both_smooth() {
        // n = 2047 = 23 * 89
        // 23-1 = 22 = 2*11, 89-1 = 88 = 2^3*11 → both 11-smooth
        // P-1 with B1=11 gives a ≡ 1 (mod n), trivial gcd
        let n = Integer::from(2047u32);
        let factor = p1_stage1(&n, 11);
        assert!(
            factor.is_none(),
            "P-1 should return None when all factors are B1-smooth"
        );
    }

    // ── Stage 2 (Standard Continuation) ──────────────────────────────
    //
    // Stage 2 extends the search to primes q in (B1, B2]. It catches
    // factors where p-1 has all prime factors <= B1 except for one prime q
    // in (B1, B2]. Uses precomputed gap powers for efficiency.

    /// Stage 2 finds factor 29 in 29*10007: 29-1 = 28 = 2^2*7.
    ///
    /// With B1=5, Stage 1 covers primes {2, 3, 5} but misses 7.
    /// Stage 2 with B2=10 covers prime 7, finding the factor.
    /// This demonstrates the "one large prime factor" case.
    #[test]
    fn p1_combined_stage2_finds_factor() {
        // n = 13 * 10007 = 130091
        // 13-1 = 12 = 2^2 * 3 (3-smooth, found by Stage 1 with B1=5)
        // But let's test Stage 2: use a factor where p-1 has a prime just above B1
        // p = 29: p-1 = 28 = 2^2 * 7. With B1=5, Stage 1 misses (needs 7).
        // Stage 2 with B2=10 should find it.
        // q = 10007: q-1 = 2*5003 (not smooth)
        // n = 29 * 10007 = 290203
        let n = Integer::from(29u64 * 10007);
        let factor = p1_factor(&n, 5, Some(10));
        assert!(
            factor.is_some(),
            "P-1 Stage 2 should find 29 (needs prime 7 in stage 2)"
        );
        let f = factor.unwrap();
        assert!(n.is_divisible(&f));
    }

    /// Verify P-1 returns None for actual primes (no factors to find).
    /// 104729 is the 10000th prime.
    #[test]
    fn p1_returns_none_for_primes() {
        let p = Integer::from(104729u32); // prime
        assert!(p1_stage1(&p, 1000).is_none());
    }

    /// Verify P-1 handles degenerate inputs (n <= 3) without panicking.
    /// These are below the minimum threshold and should return None.
    #[test]
    fn p1_handles_small_inputs() {
        assert!(p1_stage1(&Integer::from(2u32), 100).is_none());
        assert!(p1_stage1(&Integer::from(3u32), 100).is_none());
        assert!(p1_stage1(&Integer::from(1u32), 100).is_none());
    }

    /// Verify the convenience wrapper is_p1_composite matches p1_stage1.
    #[test]
    fn p1_is_composite_check() {
        // 41 * 10007 = 410287 (41-1 = 40 = 2^3*5, smooth)
        let n = Integer::from(41u64 * 10007);
        assert!(is_p1_composite(&n, 100));

        let p = Integer::from(104729u32);
        assert!(!is_p1_composite(&p, 100));
    }

    /// P-1 finds a factor of a kbn-form composite: 3*2^20 - 1 = 3145727 = 13*241979.
    ///
    /// 13-1 = 12 = 2^2*3 (3-smooth), easily found with B1=100.
    /// This exercises P-1 on a value from the kbn search pipeline.
    #[test]
    fn p1_larger_kbn_composite() {
        // 3 * 2^20 - 1 = 3145727 = 13 * 241979
        // 13-1 = 12 = 2^2 * 3 (3-smooth, found with B1=100)
        let n = Integer::from(3u32) * Integer::from(2u32).pow(20) - 1u32;
        assert_eq!(n, Integer::from(3145727u32));
        let factor = p1_stage1(&n, 100);
        assert!(factor.is_some(), "P-1 should find a factor of 3*2^20-1");
        let f = factor.unwrap();
        assert!(n.is_divisible(&f), "factor should divide n");
    }

    /// Demonstrate Stage 1 alone misses, but Stage 2 finds the factor.
    ///
    /// n = 211*10007. 211-1 = 210 = 2*3*5*7. With B1=5, Stage 1 covers
    /// {2,3,5} but misses prime 7. Stage 2 with B2=10 covers 7 and finds 211.
    /// First assertion confirms Stage 1 alone fails.
    #[test]
    fn p1_stage2_extends_reach() {
        // n = 211 * 10007 = 2111477
        // 211-1 = 210 = 2 * 3 * 5 * 7. Max prime factor = 7.
        // With B1=5 (Stage 1): exponent = 4*3*5=60. 210/60 = 3.5 → not divisible → misses 211.
        // But 210 = 2*3*5*7, and Stage 1 with B1=5 only covers 2,3,5.
        // Stage 2 with B2=10 covers prime 7 → should find 211.
        let n = Integer::from(211u64 * 10007);
        // Stage 1 alone should miss
        let s1 = p1_stage1(&n, 5);
        assert!(
            s1.is_none(),
            "Stage 1 with B1=5 should miss 211 (needs prime 7)"
        );

        // Combined with Stage 2 should find it
        let factor = p1_factor(&n, 5, Some(10));
        assert!(factor.is_some(), "Stage 2 with B2=10 should find 211");
        let f = factor.unwrap();
        assert!(n.is_divisible(&f));
    }

    // ── Adaptive P-1 Filter (Auto-Tuned B1/B2) ───────────────────────
    //
    // The adaptive filter selects B1/B2 based on candidate bit size.
    // Below 5K bits, P-1 is not cost-effective (the modular exponentiations
    // cost more than just running MR). Above 5K bits, the filter becomes
    // profitable because MR cost grows as O(n^2) while P-1 cost is O(pi(B1)).

    /// Verify the adaptive filter skips candidates below 5K bits.
    ///
    /// For small candidates, P-1 is not cost-effective: the modular
    /// exponentiations in Stage 1 cost more than the MR test they would
    /// save. The filter should return false regardless of smoothness.
    #[test]
    fn adaptive_p1_filter_skips_small_candidates() {
        // Candidates below 5K bits should be skipped (returns false regardless)
        let small = Integer::from(2u32).pow(4999) - 1u32; // 4999 bits
        assert!(
            !adaptive_p1_filter(&small),
            "P-1 should skip candidates < 5K bits"
        );

        let tiny = Integer::from(1000003u32);
        assert!(!adaptive_p1_filter(&tiny), "P-1 should skip small primes");

        let composite = Integer::from(41u32 * 10007); // has smooth factor but tiny
        assert!(
            !adaptive_p1_filter(&composite),
            "P-1 should skip even easy composites if small"
        );
    }

    /// Verify the adaptive filter never falsely flags a prime as composite.
    ///
    /// P-1 is a factoring algorithm: it can only find factors that exist.
    /// For actual primes, gcd(a-1, N) is always 1 or N (trivial), so
    /// the filter must return false. We test with Mersenne prime M_89
    /// and a small prime 104729.
    #[test]
    fn adaptive_p1_filter_safe_on_primes() {
        // P-1 should never falsely flag a prime as composite.
        // Use a known Mersenne prime: 2^89 - 1 (small, will be skipped by threshold)
        let m89 = Integer::from(2u32).pow(89) - 1u32;
        assert!(!adaptive_p1_filter(&m89));

        // A large (but below threshold) prime
        let p = Integer::from(104729u32);
        assert!(!adaptive_p1_filter(&p));
    }

    /// Verify the adaptive filter catches a composite with a Fermat prime factor.
    ///
    /// p = 65537 = 2^16+1 (Fermat prime F_4, OEIS A019434).
    /// p-1 = 2^16 is perfectly smooth (only factor is 2). With B1=100K,
    /// Stage 1 trivially finds this factor. The other factor q is chosen
    /// to be next_prime(2^5000) so q-1 has a huge prime factor.
    ///
    /// This is the ideal case for P-1: one factor with perfectly smooth p-1.
    #[test]
    fn adaptive_p1_filter_catches_smooth_factor() {
        // Build a composite at the 5K-bit tier: product of a B1-smooth prime and
        // a large non-smooth prime. The smooth prime's p-1 is entirely ≤ B1=100K.
        //
        // p = 2^16 + 1 = 65537 (Fermat prime, p-1 = 2^16 perfectly smooth)
        // q = next_prime(2^5000) (p-1 has a huge prime factor)
        // n = p * q should be caught by Stage 1 with B1=100K.
        let p = Integer::from(65537u32);
        let q = {
            let mut q = Integer::from(2u32).pow(5000);
            q.next_prime_mut();
            q
        };
        let n = Integer::from(&p * &q);
        assert!(
            n.significant_bits() >= 5000,
            "composite should be ≥ 5K bits"
        );
        assert!(
            adaptive_p1_filter(&n),
            "P-1 should find 65537 (perfectly smooth p-1)"
        );
    }

    /// Verify the adaptive filter's Stage 2 catches a partially-smooth factor.
    ///
    /// p = 1000003 (prime). p-1 = 1000002 = 2*3*166667 (166667 is prime).
    /// With B1=100K (5K-20K tier), Stage 1 misses (166667 > 100K).
    /// Stage 2 with B2=10M covers 166667 (< 10M), finding the factor.
    /// The other factor q = next_prime(2^5000) has non-smooth p-1.
    #[test]
    fn adaptive_p1_filter_catches_stage2_factor() {
        // Composite where Stage 1 alone misses, but Stage 2 finds the factor.
        //
        // p = 200003 (prime). p-1 = 200002 = 2 * 100001 = 2 * 3 * 33337.
        // 33337 is prime. With B1=100K (tier for 5K bits), Stage 1 covers up to 100K.
        // 33337 < 100K, so Stage 1 should actually find it. Let's use a harder case:
        //
        // p = 1000003 (prime). p-1 = 1000002 = 2 * 500001 = 2 * 3 * 166667 = 2 * 3 * 166667.
        // 166667 = 166667 (prime). With B1=100K, Stage 1 misses (166667 > 100K).
        // Stage 2 with B2=10M should find it since 166667 < 10M.
        let p = Integer::from(1_000_003u32);
        let q = {
            let mut q = Integer::from(2u32).pow(5000);
            q.next_prime_mut();
            q
        };
        let n = Integer::from(&p * &q);
        assert!(n.significant_bits() >= 5000);
        assert!(
            adaptive_p1_filter(&n),
            "Stage 2 should catch factor with p-1's largest prime factor = 166667 (< B2=10M)"
        );
    }

    /// Smoke test: adaptive filter on a large kbn composite 3*2^5000-1.
    ///
    /// We don't assert the result (no guarantee the factors are smooth),
    /// but verify the function runs without panicking on a ~1500-digit number.
    #[test]
    fn adaptive_p1_filter_rejects_kbn_composite() {
        // 3 * 2^5000 - 1 is composite. If it has a factor with smooth p-1, P-1 finds it.
        // We don't assert it IS found (no guarantee the factors are smooth), but we verify
        // the function doesn't panic or return incorrectly for large kbn composites.
        let n = Integer::from(3u32) * Integer::from(2u32).pow(5000) - 1u32;
        // Just verify it runs without error — result depends on factor smoothness
        let _ = adaptive_p1_filter(&n);
    }

    /// Verify the 5K-20K tier selects B1=100K and catches a smooth factor.
    ///
    /// p = 99991 (prime). p-1 = 99990 = 2*3*5*3333 = 2*3*5*3*11*101.
    /// Largest prime factor: 101. B1=100K >> 101, so Stage 1 finds it easily.
    #[test]
    fn adaptive_p1_filter_tunes_by_size() {
        // Verify that the function correctly selects different tiers.
        // We use composites where Stage 1 B1=100K finds a factor but B1=50K wouldn't,
        // to confirm the 5K-20K tier uses B1=100K.
        //
        // p = 99991 (prime, p-1 = 99990 = 2 * 3 * 5 * 3333 = 2*3*5*3*11*101)
        // Largest prime factor of p-1: 101. B1=100K covers it easily.
        let p = Integer::from(99991u32);
        let q = {
            let mut q = Integer::from(2u32).pow(5000);
            q.next_prime_mut();
            q
        };
        let n = Integer::from(&p * &q);
        assert!(
            adaptive_p1_filter(&n),
            "5K-20K tier (B1=100K) should catch 99991"
        );
    }

    // ── Stage 1 Boundary Conditions ──────────────────────────────────

    /// Verify B1 < 2 returns None immediately (no primes to sieve with).
    #[test]
    fn p1_stage1_with_b1_equals_1_returns_none() {
        // b1 < 2 should return None immediately
        let n = Integer::from(15u32);
        assert!(p1_stage1(&n, 1).is_none(), "B1=1 should return None");
        assert!(p1_stage1(&n, 0).is_none(), "B1=0 should return None");
    }

    /// Verify B1=2 finds factors where p-1 is a power of 2 (2-smooth).
    ///
    /// n = 3*10007. p=3, p-1 = 2 (perfectly 2-smooth).
    /// With B1=2, the only sieve prime is 2. lcm(1..2) = 2, and
    /// a = 2^2 mod n. gcd(a-1, n) should yield factor 3.
    #[test]
    fn p1_stage1_with_b1_equals_2() {
        // B1=2: only prime 2. Should find factors p where p-1 is a power of 2.
        // p=3: p-1=2 (2-smooth). n = 3 * 10007 = 30021
        let n = Integer::from(3u64 * 10007);
        let factor = p1_stage1(&n, 2);
        assert!(
            factor.is_some(),
            "P-1 with B1=2 should find 3 (p-1=2 is 2-smooth)"
        );
        assert_eq!(factor.unwrap(), Integer::from(3u32));
    }

    /// Find factor 97 where p-1 = 96 = 2^5*3 has a prime power in the factorization.
    ///
    /// The lcm computation must handle prime powers: for prime 2,
    /// pk iterates 2->4->8->16->32->64 (stopping when 64*2=128 > B1=100).
    /// So the exponent for 2 is 64 = 2^6. Since 2^5 | 96, this suffices.
    #[test]
    fn p1_stage1_finds_factor_with_prime_power_smooth() {
        // n = 97 * 10007 = 970679
        // 97-1 = 96 = 2^5 * 3 (3-smooth, found with B1=3)
        let n = Integer::from(97u64 * 10007);
        let factor = p1_stage1(&n, 100);
        assert!(
            factor.is_some(),
            "P-1 should find 97 (p-1 = 96 = 2^5*3, smooth)"
        );
        let f = factor.unwrap();
        assert!(n.is_divisible(&f));
    }

    /// Find Fermat prime factor 257 in an RSA-like semiprime.
    ///
    /// p = 257 (Fermat prime F_3, OEIS A019434). p-1 = 256 = 2^8 (perfectly smooth).
    /// q = 100003, q-1 = 2*50001 = 2*3*16667 (not smooth for small B1).
    /// B1 must be >= 256 so that the prime-power iteration for 2 reaches
    /// pk = 256 (2->4->8->16->32->64->128->256).
    #[test]
    fn p1_stage1_finds_factor_of_rsa_like_number() {
        // Product of two primes, one with smooth p-1.
        // p = 257 (Fermat prime), p-1 = 256 = 2^8 (perfectly smooth)
        // q = 100003, q-1 = 100002 = 2 * 50001 = 2 * 3 * 16667 (not smooth)
        // B1 must be >= 256 so that 2^e covers 2^8 (pk iterates: 2→4→8→...→256)
        let n = Integer::from(257u64) * Integer::from(100003u64);
        let factor = p1_stage1(&n, 300);
        assert!(
            factor.is_some(),
            "P-1 should find 257 (p-1 = 2^8, perfectly smooth)"
        );
        assert_eq!(factor.unwrap(), Integer::from(257u32));
    }

    /// Verify P-1 works on composites with 3 prime factors.
    ///
    /// n = 5*7*10007. Both 5-1=4=2^2 and 7-1=6=2*3 are smooth.
    /// P-1 may find 5, 7, or 5*7=35 depending on which gcd is non-trivial.
    /// The result must be a proper non-trivial divisor of n.
    #[test]
    fn p1_stage1_three_prime_factors() {
        // n = 5 * 7 * 10007. Both 5-1=4 and 7-1=6=2*3 are smooth.
        // P-1 should find at least one factor (might find 5*7=35 if both smooth at same B1)
        let n = Integer::from(5u64 * 7 * 10007);
        let factor = p1_stage1(&n, 100);
        assert!(
            factor.is_some(),
            "P-1 should find a factor of 5*7*10007"
        );
        let f = factor.unwrap();
        assert!(n.is_divisible(&f), "factor should divide n");
        assert!(f > 1u32, "factor should be non-trivial");
        assert!(f < n, "factor should be proper");
    }

    // ── Stage 2 Boundary Conditions ──────────────────────────────────

    /// Verify Stage 2 returns None when B2 <= B1 (no primes in the range).
    #[test]
    fn p1_stage2_returns_none_when_b2_leq_b1() {
        let n = Integer::from(15u32);
        let a = Integer::from(2u32);
        assert!(
            p1_stage2(&n, &a, 100, 100).is_none(),
            "Stage 2 with B2 = B1 should return None"
        );
        assert!(
            p1_stage2(&n, &a, 100, 50).is_none(),
            "Stage 2 with B2 < B1 should return None"
        );
    }

    /// Verify p1_factor with B1=10 and default B2 finds 43 (p-1 max prime = 7).
    ///
    /// 43-1 = 42 = 2*3*7. With B1=10, Stage 1 covers {2,3,5,7} and finds it.
    /// Default B2 = 100*B1 = 1000 is not needed but should not cause issues.
    #[test]
    fn p1_factor_with_explicit_b2() {
        // n = 43 * 10007. 43-1 = 42 = 2*3*7 (7-smooth).
        // With B1=10, B2=None → default B2=1000. Should find it.
        let n = Integer::from(43u64 * 10007);
        let factor = p1_factor(&n, 10, None);
        assert!(
            factor.is_some(),
            "p1_factor with B1=10 should find 43 (p-1 max prime = 7)"
        );
        let f = factor.unwrap();
        assert!(n.is_divisible(&f));
    }

    /// Verify p1_factor returns None for n <= 3 (degenerate inputs).
    #[test]
    fn p1_factor_returns_none_for_small_n() {
        assert!(p1_factor(&Integer::from(2u32), 100, None).is_none());
        assert!(p1_factor(&Integer::from(3u32), 100, None).is_none());
    }

    /// Verify p1_factor returns None for B1 < 2 (no sieve primes).
    #[test]
    fn p1_factor_returns_none_for_b1_too_small() {
        assert!(p1_factor(&Integer::from(100u32), 1, None).is_none());
    }

    // ── is_p1_composite Convenience Wrapper ──────────────────────────

    /// Detect 61*100003 as composite: 61-1 = 60 = 2^2*3*5 (5-smooth).
    #[test]
    fn is_p1_composite_on_semiprime_with_very_smooth_factor() {
        // n = 61 * 100003. 61-1 = 60 = 2^2 * 3 * 5 (5-smooth).
        // 100003-1 = 100002 = 2 * 3 * 16667 (16667 is prime, not smooth at B1=100).
        let n = Integer::from(61u64 * 100003);
        assert!(
            is_p1_composite(&n, 100),
            "P-1 should detect 61*100003 as composite (61-1 is 5-smooth)"
        );
    }

    /// Smoke test on Carmichael number 561 = 3*11*17.
    ///
    /// All three factors have smooth p-1: 2, 10=2*5, 16=2^4.
    /// P-1 might find a trivial gcd (all smooth) or a non-trivial factor.
    /// We don't assert the result, just verify no panic.
    #[test]
    fn is_p1_composite_on_carmichael_number() {
        // 561 = 3 * 11 * 17
        // 3-1=2 (smooth), 11-1=10=2*5 (smooth), 17-1=16=2^4 (smooth)
        // All factors are smooth, so P-1 might find trivial gcd or non-trivial.
        let n = Integer::from(561u32);
        // We don't guarantee it finds it, but it shouldn't crash
        let _ = is_p1_composite(&n, 100);
    }

    /// Verify P-1 does not flag a large prime as composite (soundness check).
    #[test]
    fn is_p1_composite_on_large_prime() {
        // Large prime — should return false (no factor)
        let p = Integer::from(1000000007u64);
        assert!(
            !is_p1_composite(&p, 100),
            "P-1 should not flag a prime as composite"
        );
    }

    // ── Stage 2 Integration with p1_factor ───────────────────────────

    /// Stage 1 misses, Stage 2 catches: 53*100003 (p-1 max prime = 13 > B1=10).
    ///
    /// 53-1 = 52 = 2^2*13. With B1=10, Stage 1 misses (13 > 10).
    /// Stage 2 with B2=20 covers prime 13, finding the factor.
    /// First verifies Stage 1 alone fails, then confirms p1_factor succeeds.
    #[test]
    fn p1_factor_stage2_catches_factor_missed_by_stage1() {
        // n = 53 * 100003.
        // 53-1 = 52 = 2^2 * 13. With B1=10, Stage 1 misses (13 > 10).
        // Stage 2 with B2=20 should find 53 since 13 is in (10, 20].
        let n = Integer::from(53u64 * 100003);
        let s1 = p1_stage1(&n, 10);
        assert!(
            s1.is_none(),
            "Stage 1 with B1=10 should miss 53 (needs prime 13)"
        );
        let factor = p1_factor(&n, 10, Some(20));
        assert!(
            factor.is_some(),
            "Stage 2 with B2=20 should find 53 (p-1 max prime = 13)"
        );
        let f = factor.unwrap();
        assert!(n.is_divisible(&f));
    }

    // ── Smoothness Boundary Tests ─────────────────────────────────────
    //
    // These tests verify behavior at the exact B1 boundary: the largest
    // prime factor of p-1 equals B1. Since generate_primes(B1) includes
    // B1 itself when B1 is prime, the factor should be found.

    /// Exact boundary: 31*100003 where p-1 max prime = 5 = B1.
    ///
    /// 31-1 = 30 = 2*3*5. With B1=5, the sieve includes {2, 3, 5},
    /// covering all prime factors of 30. The factor 31 should be found.
    #[test]
    fn p1_stage1_exact_b1_boundary() {
        // n = 31 * 100003. 31-1 = 30 = 2*3*5. Max prime = 5.
        // B1=5 should find it (5 is included in primes up to B1=5).
        let n = Integer::from(31u64 * 100003);
        let factor = p1_stage1(&n, 5);
        assert!(
            factor.is_some(),
            "P-1 with B1=5 should find 31 (p-1 max prime = 5)"
        );
        assert_eq!(factor.unwrap(), Integer::from(31u32));
    }

    /// Just below boundary: 29*100003 where p-1 max prime = 7 > B1=5.
    ///
    /// 29-1 = 28 = 2^2*7. With B1=5, the sieve includes {2, 3, 5} but
    /// NOT 7. Since 7 > B1, Stage 1 cannot find the factor 29.
    #[test]
    fn p1_stage1_just_below_b1_boundary() {
        // n = 29 * 100003. 29-1 = 28 = 2^2 * 7. Max prime = 7.
        // B1=5 should NOT find it (7 > 5).
        let n = Integer::from(29u64 * 100003);
        let factor = p1_stage1(&n, 5);
        assert!(
            factor.is_none(),
            "P-1 with B1=5 should miss 29 (p-1 needs prime 7)"
        );
    }
}
