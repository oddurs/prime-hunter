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
    let mut current = a.clone().pow_mod(&Integer::from(primes[start_idx]), n).ok()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use rug::ops::Pow;

    // Test composites with ONE B1-smooth factor and one non-smooth factor.
    // If both factors are smooth at the same B1, P-1 gives trivial gcd(=n) and returns None.

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

    #[test]
    fn p1_misses_non_smooth_factor() {
        // n = 1000000007 * 1000000009 (both are prime, p-1 has large factors)
        let n = Integer::from(1000000007u64) * Integer::from(1000000009u64);
        let factor = p1_stage1(&n, 100);
        assert!(factor.is_none(), "P-1 with small B1 should miss non-smooth factors");
    }

    #[test]
    fn p1_returns_none_when_both_smooth() {
        // n = 2047 = 23 * 89
        // 23-1 = 22 = 2*11, 89-1 = 88 = 2^3*11 → both 11-smooth
        // P-1 with B1=11 gives a ≡ 1 (mod n), trivial gcd
        let n = Integer::from(2047u32);
        let factor = p1_stage1(&n, 11);
        assert!(factor.is_none(), "P-1 should return None when all factors are B1-smooth");
    }

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
        assert!(factor.is_some(), "P-1 Stage 2 should find 29 (needs prime 7 in stage 2)");
        let f = factor.unwrap();
        assert!(n.is_divisible(&f));
    }

    #[test]
    fn p1_returns_none_for_primes() {
        let p = Integer::from(104729u32); // prime
        assert!(p1_stage1(&p, 1000).is_none());
    }

    #[test]
    fn p1_handles_small_inputs() {
        assert!(p1_stage1(&Integer::from(2u32), 100).is_none());
        assert!(p1_stage1(&Integer::from(3u32), 100).is_none());
        assert!(p1_stage1(&Integer::from(1u32), 100).is_none());
    }

    #[test]
    fn p1_is_composite_check() {
        // 41 * 10007 = 410287 (41-1 = 40 = 2^3*5, smooth)
        let n = Integer::from(41u64 * 10007);
        assert!(is_p1_composite(&n, 100));

        let p = Integer::from(104729u32);
        assert!(!is_p1_composite(&p, 100));
    }

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
        assert!(s1.is_none(), "Stage 1 with B1=5 should miss 211 (needs prime 7)");

        // Combined with Stage 2 should find it
        let factor = p1_factor(&n, 5, Some(10));
        assert!(factor.is_some(), "Stage 2 with B2=10 should find 211");
        let f = factor.unwrap();
        assert!(n.is_divisible(&f));
    }
}
