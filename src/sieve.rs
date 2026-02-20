//! # Sieve — Prime Generation and Modular Arithmetic Utilities
//!
//! Core number-theoretic infrastructure used by every search module. Provides:
//!
//! 1. **Prime generation** via a wheel-30 sieve of Eratosthenes (26.7% memory
//!    of naive sieve — stores only residues coprime to {2, 3, 5}).
//! 2. **Modular exponentiation** (`pow_mod`) using u128 intermediates.
//! 3. **Montgomery multiplication** (`MontgomeryCtx`) — replaces u128 division
//!    (35–90 cycles) with multiply+shift (4–6 cycles) for repeated modular
//!    arithmetic with a fixed odd modulus.
//! 4. **Discrete logarithm** via baby-step giant-step (BSGS), used by the
//!    algebraic sieve in `kbn`, `twin`, `sophie_germain`, and other modules.
//! 5. **Multiplicative order** computation, used by `wagstaff`, `repunit`,
//!    and `gen_fermat` sieves.
//! 6. **Auto sieve depth** tuning, which balances sieve cost against primality
//!    test cost using a GIMPS-style crossover heuristic.
//!
//! ## Algorithm: Wheel-30 Sieve
//!
//! The sieve tracks only integers coprime to 30 = 2·3·5 (8 residues per 30).
//! Each segment of 30 consecutive integers is packed into a single byte.
//! Complexity: O(n log log n) time, O(n/30) space.
//!
//! ## Algorithm: Montgomery Multiplication
//!
//! For a fixed odd modulus n, Montgomery form represents a as ā = a·R mod n
//! where R = 2^64. Multiplication becomes: REDC(ā·b̄) = (ā·b̄ + m·n) >> 64,
//! where m = (ā·b̄ mod R) · (-n⁻¹ mod R). No division by n is ever performed.
//!
//! ## Algorithm: Baby-Step Giant-Step
//!
//! Solves base^x ≡ target (mod p) in O(√ord) time and space, where ord is
//! the multiplicative order of base mod p. This replaces the naive O(ord)
//! enumeration used by simple modular sieves.
//!
//! ## References
//!
//! - Peter L. Montgomery, "Modular Multiplication Without Trial Division",
//!   Mathematics of Computation, 44(170):519–521, 1985.
//! - Daniel Shanks, "Class Number, a Theory of Factorization, and Genera",
//!   Proceedings of Symposia in Pure Mathematics, 20:415–440, 1971 (BSGS).
//! - GIMPS sieve depth heuristic: <https://www.mersenne.org/various/math.php>

/// Default sieve limit for generating small primes used in modular pre-filtering.
pub const SIEVE_LIMIT: u64 = 10_000_000;

/// Estimate optimal sieve depth based on candidate size and expected test cost.
///
/// Uses the crossover heuristic from GIMPS: continue sieving while the marginal
/// cost of removing one more candidate via sieving is less than the expected cost
/// of testing that candidate.
///
/// `candidate_bits`: bit size of typical candidate in the search range
/// `n_range`: total number of candidates being sieved
///
/// Returns recommended sieve limit (capped at 10^9 to avoid excessive memory).
pub fn auto_sieve_depth(candidate_bits: u64, n_range: u64) -> u64 {
    // Mertens' theorem: survival rate after sieving to depth P ≈ 0.5615 / ln(P)
    // Cost of testing one candidate ≈ candidate_bits^2 (modular exponentiation)
    // Cost of BSGS sieve per prime p ≈ sqrt(ord_p) ≈ sqrt(p) operations
    //
    // Crossover: stop when sqrt(p) / survival_reduction > test_cost
    //   survival_reduction per prime ≈ 1/p (each prime removes ~1/p fraction)
    //   so continue while: sqrt(p) < test_cost / p, i.e., p^1.5 < test_cost
    //
    // Simplified: optimal_depth ≈ test_cost^(2/3)

    if candidate_bits == 0 || n_range == 0 {
        return SIEVE_LIMIT;
    }

    // test_cost ∝ candidate_bits^2 (for modular exponentiation via FFT multiply)
    // Scale factor calibrated: 1000-bit numbers → ~10M sieve (current default)
    let bits = candidate_bits as f64;
    let test_cost = bits * bits;

    // Optimal depth ≈ test_cost^(2/3), with scaling
    let raw_depth = test_cost.powf(2.0 / 3.0) * 10.0;

    // Clamp to reasonable range
    let depth = (raw_depth as u64)
        .clamp(1_000_000, 1_000_000_000); // 1M minimum (meaningful sieve), 1B max (memory/time limit)

    // For very small ranges, deep sieving isn't worth it
    // (BSGS cost is per-prime, amortized over the range)
    if n_range < 100 {
        return depth.min(SIEVE_LIMIT);
    }

    depth
}

/// Resolve sieve_limit: if user specified a value (>0), use it; otherwise auto-tune.
///
/// `sieve_limit`: user-specified limit, or 0 for auto-tuning
/// `candidate_bits`: estimated bit size of typical candidate in the search
/// `n_range`: number of candidates being sieved
pub fn resolve_sieve_limit(sieve_limit: u64, candidate_bits: u64, n_range: u64) -> u64 {
    if sieve_limit > 0 {
        sieve_limit
    } else {
        auto_sieve_depth(candidate_bits, n_range)
    }
}

/// Generate all primes up to `limit` using a wheel-30 sieve.
///
/// Uses a mod-30 wheel to store only numbers coprime to {2,3,5}, reducing
/// memory to 8/30 ≈ 26.7% of the naive sieve. For a 1B limit, this uses
/// ~267MB instead of 1GB.
pub fn generate_primes(limit: u64) -> Vec<u64> {
    if limit < 2 {
        return vec![];
    }
    if limit < 7 {
        // Small cases: return directly
        return [2, 3, 5]
            .iter()
            .copied()
            .filter(|&p| p <= limit)
            .collect();
    }

    // Residues coprime to 30: these are the only positions we track
    const RESIDUES: [u8; 8] = [1, 7, 11, 13, 17, 19, 23, 29];

    // Map residue → index in the wheel (for residues coprime to 30)
    const RES_TO_IDX: [u8; 30] = [
        255, 0, 255, 255, 255, 255, 255, 1, 255, 255, 255, 2, 255, 3, 255, 255,
        255, 4, 255, 5, 255, 255, 255, 6, 255, 255, 255, 255, 255, 7,
    ];

    let limit = limit as usize;
    let num_segments = limit / 30 + 1;
    // Pack 8 residues per byte (one bit each) for each segment of 30
    let mut sieve = vec![0xFFu8; num_segments]; // all bits set = all residues are prime

    // Sieve: for each prime p (starting from 7), mark composites
    let sqrt_limit = (limit as f64).sqrt() as usize + 1;
    for seg in 0..num_segments {
        for &ri in &RESIDUES {
            let n = seg * 30 + ri as usize;
            if n < 7 || n > sqrt_limit {
                continue;
            }
            let idx = RES_TO_IDX[ri as usize] as usize;
            if sieve[seg] & (1 << idx) == 0 {
                continue; // already marked composite
            }
            // Mark multiples of n
            // For each residue r coprime to 30, mark n*k where n*k ≡ r (mod 30)
            let mut m = n * n;
            while m <= limit {
                let ms = m / 30;
                let mr = m % 30;
                if mr < 30 && RES_TO_IDX[mr] != 255 {
                    sieve[ms] &= !(1 << RES_TO_IDX[mr]);
                }
                m += n;
            }
        }
    }

    // Collect primes
    let mut primes = Vec::with_capacity(estimate_prime_count(limit));
    primes.extend_from_slice(&[2, 3, 5]);

    for (seg, &byte) in sieve.iter().enumerate().take(num_segments) {
        if byte == 0 {
            continue;
        }
        for (bit_idx, &r) in RESIDUES.iter().enumerate() {
            if byte & (1 << bit_idx) != 0 {
                let n = seg * 30 + r as usize;
                if n > 5 && n <= limit {
                    primes.push(n as u64);
                }
            }
        }
    }
    primes
}

/// Estimate prime count up to n using the prime counting function approximation.
fn estimate_prime_count(n: usize) -> usize {
    if n < 10 {
        return 4;
    }
    let nf = n as f64;
    (1.3 * nf / nf.ln()) as usize
}

/// Modular exponentiation: base^exp mod modulus.
/// Uses u128 intermediates to avoid overflow for moduli up to ~2^63.
pub fn pow_mod(mut base: u64, mut exp: u64, modulus: u64) -> u64 {
    if modulus == 1 {
        return 0;
    }
    let mut result: u64 = 1;
    base %= modulus;
    while exp > 0 {
        if exp & 1 == 1 {
            result = (result as u128 * base as u128 % modulus as u128) as u64;
        }
        exp >>= 1;
        base = (base as u128 * base as u128 % modulus as u128) as u64;
    }
    result
}

/// Greatest common divisor.
pub fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Montgomery multiplication context for a fixed odd modulus.
///
/// Replaces u128 division (35-90 cycles/op) with multiply+shift (4-6 cycles/op).
/// All arithmetic is performed in Montgomery form: ā = a·R mod n, where R = 2^64.
#[derive(Clone, Copy, Debug)]
pub struct MontgomeryCtx {
    /// The modulus (must be odd, > 1).
    pub n: u64,
    /// -n⁻¹ mod 2^64 (precomputed via Hensel lifting).
    n_prime: u64,
    /// R mod n = 2^64 mod n (Montgomery form of 1).
    r_mod_n: u64,
    /// R² mod n (used for converting to Montgomery form).
    r2_mod_n: u64,
}

impl MontgomeryCtx {
    /// Create a Montgomery context for the given odd modulus n > 1.
    pub fn new(n: u64) -> Self {
        debug_assert!(n > 1 && n & 1 == 1, "Montgomery requires odd modulus > 1");

        // Hensel lifting: compute n⁻¹ mod 2^64.
        // Starting with n⁻¹ ≡ 1 (mod 2) for odd n, each iteration doubles precision.
        // 6 iterations: 2^1 → 2^2 → 2^4 → 2^8 → 2^16 → 2^32 → 2^64.
        let mut inv: u64 = 1;
        for _ in 0..6 {
            inv = inv.wrapping_mul(2u64.wrapping_sub(n.wrapping_mul(inv)));
        }
        let n_prime = inv.wrapping_neg(); // -n⁻¹ mod 2^64

        let r_mod_n = ((1u128 << 64) % n as u128) as u64;
        let r2_mod_n = ((r_mod_n as u128 * r_mod_n as u128) % n as u128) as u64;

        MontgomeryCtx {
            n,
            n_prime,
            r_mod_n,
            r2_mod_n,
        }
    }

    /// Convert a normal value to Montgomery form: ā = a·R mod n.
    #[inline]
    pub fn to_mont(&self, a: u64) -> u64 {
        self.mul(a % self.n, self.r2_mod_n)
    }

    /// Convert from Montgomery form back to normal: a = ā·R⁻¹ mod n.
    #[inline]
    pub fn from_mont(&self, a: u64) -> u64 {
        self.reduce(a as u128)
    }

    /// Montgomery reduction (REDC): compute t·R⁻¹ mod n.
    #[inline]
    fn reduce(&self, t: u128) -> u64 {
        let m = (t as u64).wrapping_mul(self.n_prime);
        let u = t + (m as u128) * (self.n as u128);
        let result = (u >> 64) as u64;
        if result >= self.n {
            result - self.n
        } else {
            result
        }
    }

    /// Montgomery multiplication: compute a·b·R⁻¹ mod n.
    /// Both inputs and output are in Montgomery form.
    #[inline]
    pub fn mul(&self, a: u64, b: u64) -> u64 {
        self.reduce((a as u128) * (b as u128))
    }

    /// Montgomery squaring.
    #[inline]
    pub fn sqr(&self, a: u64) -> u64 {
        self.mul(a, a)
    }

    /// Modular exponentiation in Montgomery form.
    /// Input base must be in Montgomery form; returns result in Montgomery form.
    pub fn pow_mod(&self, base: u64, mut exp: u64) -> u64 {
        let mut result = self.r_mod_n; // 1 in Montgomery form
        let mut b = base;
        while exp > 0 {
            if exp & 1 == 1 {
                result = self.mul(result, b);
            }
            exp >>= 1;
            if exp > 0 {
                b = self.sqr(b);
            }
        }
        result
    }

    /// Modular inverse in Montgomery form via Fermat's little theorem: a^(n-2).
    /// Input and output are in Montgomery form. Returns None if a ≡ 0 (mod n).
    pub fn mod_inverse(&self, a_mont: u64) -> Option<u64> {
        if a_mont == 0 {
            return None;
        }
        Some(self.pow_mod(a_mont, self.n - 2))
    }

    /// The Montgomery form of 1 (= R mod n).
    #[inline]
    pub fn one(&self) -> u64 {
        self.r_mod_n
    }
}

/// Modular inverse via Fermat's little theorem: a^(p-2) mod p.
/// Uses Montgomery multiplication internally for odd primes.
/// Returns None if a ≡ 0 (mod p). Requires p prime.
pub fn mod_inverse(a: u64, p: u64) -> Option<u64> {
    if a.is_multiple_of(p) {
        return None;
    }
    if p > 2 {
        let ctx = MontgomeryCtx::new(p);
        let a_mont = ctx.to_mont(a);
        let inv_mont = ctx.pow_mod(a_mont, p - 2);
        Some(ctx.from_mont(inv_mont))
    } else {
        Some(pow_mod(a, p - 2, p))
    }
}

/// Trial-division factorization of a u64 into (prime, exponent) pairs.
pub fn factor_u64(mut n: u64) -> Vec<(u64, u32)> {
    let mut factors = Vec::new();
    let mut d = 2u64;
    while d * d <= n {
        if n.is_multiple_of(d) {
            let mut exp = 0u32;
            while n.is_multiple_of(d) {
                n /= d;
                exp += 1;
            }
            factors.push((d, exp));
        }
        d += 1;
    }
    if n > 1 {
        factors.push((n, 1));
    }
    factors
}

/// Multiplicative order of `base` modulo `p`: smallest d > 0 with base^d ≡ 1 (mod p).
/// Uses Montgomery multiplication internally for odd primes.
/// Requires p prime and base not divisible by p.
pub fn multiplicative_order(base: u64, p: u64) -> u64 {
    let mut order = p - 1;
    let factors = factor_u64(order);
    if p > 2 {
        let ctx = MontgomeryCtx::new(p);
        let base_mont = ctx.to_mont(base % p);
        let one = ctx.one();
        for (q, _) in factors {
            while order.is_multiple_of(q) && ctx.pow_mod(base_mont, order / q) == one {
                order /= q;
            }
        }
    } else {
        for (q, _) in factors {
            while order.is_multiple_of(q) && pow_mod(base, order / q, p) == 1 {
                order /= q;
            }
        }
    }
    order
}

/// Baby-step giant-step discrete logarithm.
/// Uses Montgomery multiplication internally for odd primes.
/// Finds x in [0, order) such that base^x ≡ target (mod p), or None if no solution.
pub fn discrete_log_bsgs(base: u64, target: u64, p: u64, order: u64) -> Option<u64> {
    use std::collections::HashMap;

    let m = (order as f64).sqrt().ceil() as u64;
    if m == 0 {
        return None;
    }

    if p > 2 {
        // Montgomery path: all arithmetic uses multiply+shift instead of division
        let ctx = MontgomeryCtx::new(p);
        let base_mont = ctx.to_mont(base % p);
        let target_mont = ctx.to_mont(target % p);

        // Baby steps: base^j → j, for j = 0..m-1
        let mut table = HashMap::with_capacity(m as usize);
        let mut power = ctx.one();
        for j in 0..m {
            table.insert(power, j);
            power = ctx.mul(power, base_mont);
        }

        // Giant step factor: base^(-m) mod p
        let base_inv_mont = ctx.mod_inverse(base_mont)?;
        let giant_step = ctx.pow_mod(base_inv_mont, m);

        // Giant steps: check target * (base^(-m))^i for i = 0..m
        let mut gamma = target_mont;
        for i in 0..=m {
            if let Some(&j) = table.get(&gamma) {
                let x = i * m + j;
                if x < order {
                    return Some(x);
                }
            }
            gamma = ctx.mul(gamma, giant_step);
        }
        None
    } else {
        // Fallback for p=2 (rare: sieve primes are almost always odd)
        let mut table = HashMap::with_capacity(m as usize);
        let mut power = 1u64;
        for j in 0..m {
            table.insert(power, j);
            power = (power as u128 * base as u128 % p as u128) as u64;
        }

        let base_inv = mod_inverse(base, p)?;
        let giant_step = pow_mod(base_inv, m, p);

        let mut gamma = target;
        for i in 0..=m {
            if let Some(&j) = table.get(&gamma) {
                let x = i * m + j;
                if x < order {
                    return Some(x);
                }
            }
            gamma = (gamma as u128 * giant_step as u128 % p as u128) as u64;
        }
        None
    }
}

/// Packed bit array for sieve results.
///
/// 8× memory reduction over `Vec<bool>`: a 10M-candidate search drops from
/// 10 MB to 1.25 MB, fitting entirely in L2 cache on most architectures.
/// Uses hardware `POPCNT` (via `count_ones()`) for O(n/64) survivor counting.
///
/// Bit layout: bit `i` is stored in word `i / 64`, bit position `i % 64`.
/// A set bit (1) means the candidate **survives** the sieve; a clear bit (0)
/// means it was eliminated.
pub struct BitSieve {
    words: Vec<u64>,
    len: usize,
}

impl BitSieve {
    /// Create a sieve of `len` bits, all set to 1 (all candidates survive).
    pub fn new_all_set(len: usize) -> Self {
        let num_words = (len + 63) / 64;
        let mut words = vec![u64::MAX; num_words];
        // Clear unused high bits in the last word
        let extra = num_words * 64 - len;
        if extra > 0 && num_words > 0 {
            words[num_words - 1] >>= extra;
        }
        BitSieve { words, len }
    }

    /// Create a sieve of `len` bits, all cleared to 0 (all eliminated).
    pub fn new_all_clear(len: usize) -> Self {
        let num_words = (len + 63) / 64;
        BitSieve {
            words: vec![0u64; num_words],
            len,
        }
    }

    /// Number of bits in this sieve.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if sieve has zero length.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get bit `index`. Returns `true` if the bit is set (candidate survives).
    ///
    /// # Panics
    /// Panics if `index >= len`.
    #[inline]
    pub fn get(&self, index: usize) -> bool {
        debug_assert!(index < self.len, "BitSieve index out of bounds: {} >= {}", index, self.len);
        let word = self.words[index / 64];
        word & (1u64 << (index % 64)) != 0
    }

    /// Set bit `index` to 1 (candidate survives).
    #[inline]
    pub fn set(&mut self, index: usize) {
        debug_assert!(index < self.len);
        self.words[index / 64] |= 1u64 << (index % 64);
    }

    /// Clear bit `index` to 0 (candidate eliminated).
    #[inline]
    pub fn clear(&mut self, index: usize) {
        debug_assert!(index < self.len);
        self.words[index / 64] &= !(1u64 << (index % 64));
    }

    /// Count the number of set bits (surviving candidates) using hardware POPCNT.
    pub fn count_ones(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Iterate over the indices of all set bits in ascending order.
    pub fn iter_set_bits(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(wi, &word)| {
            let base = wi * 64;
            BitIter { word, base }
        })
    }
}

/// Iterator over set bits within a single u64 word.
struct BitIter {
    word: u64,
    base: usize,
}

impl Iterator for BitIter {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        if self.word == 0 {
            return None;
        }
        let tz = self.word.trailing_zeros() as usize;
        self.word &= self.word - 1; // clear lowest set bit
        Some(self.base + tz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mod_inverse() {
        assert_eq!(mod_inverse(3, 7), Some(pow_mod(3, 5, 7))); // 3*5=15≡1(mod7)
        assert_eq!(mod_inverse(2, 5), Some(3)); // 2*3=6≡1(mod5)
        assert_eq!(mod_inverse(0, 7), None);
        assert_eq!(mod_inverse(7, 7), None);
    }

    #[test]
    fn test_factor_u64() {
        let empty: Vec<(u64, u32)> = vec![];
        assert_eq!(factor_u64(1), empty);
        assert_eq!(factor_u64(2), vec![(2, 1)]);
        assert_eq!(factor_u64(12), vec![(2, 2), (3, 1)]);
        assert_eq!(factor_u64(360), vec![(2, 3), (3, 2), (5, 1)]);
        assert_eq!(factor_u64(97), vec![(97, 1)]); // prime
    }

    #[test]
    fn test_multiplicative_order() {
        // ord_7(2) = 3: 2^1=2, 2^2=4, 2^3=8≡1(mod7)
        assert_eq!(multiplicative_order(2, 7), 3);
        // ord_7(3) = 6: 3 is a primitive root mod 7
        assert_eq!(multiplicative_order(3, 7), 6);
        // ord_13(2) = 12
        assert_eq!(multiplicative_order(2, 13), 12);
        // ord_5(2) = 4
        assert_eq!(multiplicative_order(2, 5), 4);
    }

    #[test]
    fn test_discrete_log_bsgs() {
        // 2^x ≡ 4 (mod 7), order=3 → x=2
        assert_eq!(discrete_log_bsgs(2, 4, 7, 3), Some(2));
        // 3^x ≡ 5 (mod 7), order=6 → 3^5=243≡5(mod7) → x=5
        assert_eq!(discrete_log_bsgs(3, 5, 7, 6), Some(5));
        // 2^x ≡ 1 (mod 7), order=3 → x=0
        assert_eq!(discrete_log_bsgs(2, 1, 7, 3), Some(0));
        // No solution: 2^x ≡ 3 (mod 5), order=4 → powers are {1,2,4,3}, so x=3
        assert_eq!(discrete_log_bsgs(2, 3, 5, 4), Some(3));
    }

    #[test]
    fn test_discrete_log_bsgs_no_solution() {
        // 4^x ≡ 3 (mod 7): 4 has order 3 (4^1=4, 4^2=2, 4^3=1), so 3 is unreachable
        assert_eq!(discrete_log_bsgs(4, 3, 7, 3), None);
    }

    #[test]
    fn test_generate_primes() {
        let primes = generate_primes(30);
        assert_eq!(primes, vec![2, 3, 5, 7, 11, 13, 17, 19, 23, 29]);
    }

    #[test]
    fn test_generate_primes_small_limits() {
        assert_eq!(generate_primes(0), Vec::<u64>::new());
        assert_eq!(generate_primes(1), Vec::<u64>::new());
        assert_eq!(generate_primes(2), vec![2]);
        assert_eq!(generate_primes(3), vec![2, 3]);
        assert_eq!(generate_primes(4), vec![2, 3]);
        assert_eq!(generate_primes(5), vec![2, 3, 5]);
        assert_eq!(generate_primes(6), vec![2, 3, 5]);
        assert_eq!(generate_primes(7), vec![2, 3, 5, 7]);
        assert_eq!(generate_primes(10), vec![2, 3, 5, 7]);
        assert_eq!(generate_primes(11), vec![2, 3, 5, 7, 11]);
    }

    #[test]
    fn test_generate_primes_known_count() {
        // pi(100) = 25
        assert_eq!(generate_primes(100).len(), 25);
        // pi(1000) = 168
        assert_eq!(generate_primes(1000).len(), 168);
        // pi(10000) = 1229
        assert_eq!(generate_primes(10000).len(), 1229);
        // pi(100000) = 9592
        assert_eq!(generate_primes(100000).len(), 9592);
    }

    #[test]
    fn test_generate_primes_boundary_around_30() {
        // Test values around wheel modulus boundaries
        let p29 = generate_primes(29);
        assert_eq!(p29, vec![2, 3, 5, 7, 11, 13, 17, 19, 23, 29]);
        let p31 = generate_primes(31);
        assert_eq!(p31, vec![2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31]);
        let p59 = generate_primes(59);
        assert_eq!(p59.len(), 17); // pi(59) = 17
        let p60 = generate_primes(60);
        assert_eq!(p60.len(), 17); // pi(60) = 17 (60 is composite)
        let p61 = generate_primes(61);
        assert_eq!(p61.len(), 18); // pi(61) = 18
    }

    #[test]
    fn test_pow_mod() {
        assert_eq!(pow_mod(2, 10, 1000), 24); // 1024 mod 1000
        assert_eq!(pow_mod(3, 4, 100), 81);
        assert_eq!(pow_mod(5, 0, 7), 1);
    }

    // ---- Montgomery cross-validation tests ----

    #[test]
    fn mont_mul_matches_naive() {
        for &p in &[3u64, 5, 7, 11, 13, 17, 97, 101, 1009, 10007, 100003] {
            let ctx = MontgomeryCtx::new(p);
            for a in 0..p.min(50) {
                for b in 0..p.min(50) {
                    let expected = (a as u128 * b as u128 % p as u128) as u64;
                    let a_mont = ctx.to_mont(a);
                    let b_mont = ctx.to_mont(b);
                    let result = ctx.from_mont(ctx.mul(a_mont, b_mont));
                    assert_eq!(
                        result, expected,
                        "p={}, a={}, b={}: mont={}, naive={}",
                        p, a, b, result, expected
                    );
                }
            }
        }
    }

    #[test]
    fn mont_pow_mod_matches_pow_mod() {
        for &p in &[3u64, 5, 7, 11, 97, 101, 1009, 10007, 100003] {
            let ctx = MontgomeryCtx::new(p);
            for base in 1..p.min(20) {
                for exp in 0..p.min(30) {
                    let expected = pow_mod(base, exp, p);
                    let base_mont = ctx.to_mont(base);
                    let result = ctx.from_mont(ctx.pow_mod(base_mont, exp));
                    assert_eq!(
                        result, expected,
                        "p={}, base={}, exp={}: mont={}, naive={}",
                        p, base, exp, result, expected
                    );
                }
            }
        }
    }

    #[test]
    fn mont_inverse_matches_naive() {
        for &p in &[3u64, 5, 7, 11, 97, 101, 1009, 10007, 100003] {
            let ctx = MontgomeryCtx::new(p);
            for a in 1..p.min(50) {
                let naive_inv = pow_mod(a, p - 2, p);
                let a_mont = ctx.to_mont(a);
                let inv_mont = ctx.mod_inverse(a_mont).unwrap();
                let mont_inv = ctx.from_mont(inv_mont);
                assert_eq!(
                    mont_inv, naive_inv,
                    "p={}, a={}: mont={}, naive={}",
                    p, a, mont_inv, naive_inv
                );
            }
        }
    }

    #[test]
    fn mont_context_identity() {
        // Verify to_mont/from_mont roundtrip
        for &p in &[3u64, 7, 101, 10007, 100003, 999999937] {
            let ctx = MontgomeryCtx::new(p);
            for a in 0..p.min(100) {
                let mont = ctx.to_mont(a);
                let back = ctx.from_mont(mont);
                assert_eq!(back, a, "p={}, a={}: roundtrip failed", p, a);
            }
        }
    }

    #[test]
    fn mont_one_is_identity() {
        for &p in &[3u64, 7, 101, 10007] {
            let ctx = MontgomeryCtx::new(p);
            let one = ctx.one();
            for a in 0..p.min(50) {
                let a_mont = ctx.to_mont(a);
                // a * 1 = a in Montgomery form
                assert_eq!(ctx.mul(a_mont, one), a_mont, "p={}, a={}", p, a);
            }
        }
    }

    #[test]
    fn mont_large_prime() {
        // Test with a prime near u64 max (< 2^63 as required)
        // Large prime < 2^63 for stress-testing Montgomery with big values
        let p = 999999999999999877u64;
        let ctx = MontgomeryCtx::new(p);

        let a = 123456789u64;
        let b = 987654321u64;
        let expected = (a as u128 * b as u128 % p as u128) as u64;
        let result = ctx.from_mont(ctx.mul(ctx.to_mont(a), ctx.to_mont(b)));
        assert_eq!(result, expected);

        // pow_mod with large prime
        let exp = 1000u64;
        let expected_pow = pow_mod(a, exp, p);
        let result_pow = ctx.from_mont(ctx.pow_mod(ctx.to_mont(a), exp));
        assert_eq!(result_pow, expected_pow);
    }

    // ---- Auto sieve depth tests ----

    #[test]
    fn auto_sieve_depth_small_candidates() {
        // 1000-bit candidates → ~10M sieve (the old default)
        let depth = auto_sieve_depth(1000, 10000);
        assert!(depth >= 1_000_000, "depth should be at least 1M: {}", depth);
        assert!(depth <= 100_000_000, "depth shouldn't be excessively large for 1000-bit: {}", depth);
    }

    #[test]
    fn auto_sieve_depth_large_candidates() {
        // 100K-bit candidates (like large kbn searches) → should be much deeper
        let depth = auto_sieve_depth(100_000, 10000);
        assert!(depth > 10_000_000, "depth should exceed 10M for 100K-bit: {}", depth);
    }

    #[test]
    fn auto_sieve_depth_tiny_range() {
        // Very small range → capped at SIEVE_LIMIT
        let depth = auto_sieve_depth(100_000, 50);
        assert!(depth <= SIEVE_LIMIT, "depth should be capped for tiny range: {}", depth);
    }

    #[test]
    fn auto_sieve_depth_zero_inputs() {
        assert_eq!(auto_sieve_depth(0, 1000), SIEVE_LIMIT);
        assert_eq!(auto_sieve_depth(1000, 0), SIEVE_LIMIT);
    }

    #[test]
    fn auto_sieve_depth_monotonic() {
        // Larger candidates should get deeper sieving
        let d1 = auto_sieve_depth(1000, 10000);
        let d2 = auto_sieve_depth(10_000, 10000);
        let d3 = auto_sieve_depth(100_000, 10000);
        assert!(d1 <= d2, "depth should grow with candidate size: {} vs {}", d1, d2);
        assert!(d2 <= d3, "depth should grow with candidate size: {} vs {}", d2, d3);
    }

    #[test]
    fn resolve_sieve_limit_explicit_wins() {
        // Explicit sieve_limit should be used as-is
        assert_eq!(resolve_sieve_limit(5_000_000, 100_000, 10000), 5_000_000);
    }

    #[test]
    fn resolve_sieve_limit_zero_auto_tunes() {
        // sieve_limit=0 should auto-tune
        let depth = resolve_sieve_limit(0, 10_000, 10000);
        assert!(depth >= 1_000_000);
    }

    // ---- BitSieve tests ----

    #[test]
    fn bitsieve_new_all_set() {
        let bs = BitSieve::new_all_set(100);
        assert_eq!(bs.len(), 100);
        assert_eq!(bs.count_ones(), 100);
        for i in 0..100 {
            assert!(bs.get(i), "bit {} should be set", i);
        }
    }

    #[test]
    fn bitsieve_new_all_clear() {
        let bs = BitSieve::new_all_clear(100);
        assert_eq!(bs.len(), 100);
        assert_eq!(bs.count_ones(), 0);
        for i in 0..100 {
            assert!(!bs.get(i), "bit {} should be clear", i);
        }
    }

    #[test]
    fn bitsieve_set_clear_get() {
        let mut bs = BitSieve::new_all_clear(200);
        bs.set(0);
        bs.set(63);
        bs.set(64);
        bs.set(127);
        bs.set(128);
        bs.set(199);
        assert!(bs.get(0));
        assert!(bs.get(63));
        assert!(bs.get(64));
        assert!(bs.get(127));
        assert!(bs.get(128));
        assert!(bs.get(199));
        assert!(!bs.get(1));
        assert!(!bs.get(65));
        assert_eq!(bs.count_ones(), 6);

        bs.clear(64);
        assert!(!bs.get(64));
        assert_eq!(bs.count_ones(), 5);
    }

    #[test]
    fn bitsieve_word_boundary() {
        // Test at exact word boundaries: 63, 64, 127, 128
        let mut bs = BitSieve::new_all_clear(256);
        for &i in &[63usize, 64, 127, 128, 191, 192, 255] {
            bs.set(i);
        }
        assert_eq!(bs.count_ones(), 7);
        for &i in &[63usize, 64, 127, 128, 191, 192, 255] {
            assert!(bs.get(i), "bit {} should be set", i);
        }
    }

    #[test]
    fn bitsieve_count_ones() {
        let mut bs = BitSieve::new_all_set(100);
        assert_eq!(bs.count_ones(), 100);
        // Clear every other bit
        for i in (0..100).step_by(2) {
            bs.clear(i);
        }
        assert_eq!(bs.count_ones(), 50);
    }

    #[test]
    fn bitsieve_iter_set_bits() {
        let mut bs = BitSieve::new_all_clear(200);
        let expected = vec![0, 1, 63, 64, 65, 127, 128, 199];
        for &i in &expected {
            bs.set(i);
        }
        let collected: Vec<usize> = bs.iter_set_bits().collect();
        assert_eq!(collected, expected);
    }

    #[test]
    fn bitsieve_empty() {
        let bs = BitSieve::new_all_set(0);
        assert_eq!(bs.len(), 0);
        assert!(bs.is_empty());
        assert_eq!(bs.count_ones(), 0);
        assert_eq!(bs.iter_set_bits().count(), 0);
    }

    #[test]
    fn bitsieve_large() {
        let n = 10_000_000;
        let bs = BitSieve::new_all_set(n);
        assert_eq!(bs.count_ones(), n);
        assert_eq!(bs.len(), n);
    }

    #[test]
    fn bitsieve_non_multiple_of_64() {
        // len=65 → 2 words. Last word should only have bit 0 set.
        let bs = BitSieve::new_all_set(65);
        assert_eq!(bs.count_ones(), 65);
        // Verify the "extra" bits in the last word are clear (don't pollute count)
        assert_eq!(bs.words.len(), 2);
        // Word 1 should have exactly 1 bit set (bit 0 = index 64)
        assert_eq!(bs.words[1].count_ones(), 1);
    }

    #[test]
    fn bitsieve_iter_set_bits_matches_count() {
        let mut bs = BitSieve::new_all_set(1000);
        // Clear primes (just to get a non-trivial pattern)
        for p in &[2u64, 3, 5, 7, 11, 13, 17, 19, 23] {
            let mut i = *p as usize;
            while i < 1000 {
                bs.clear(i);
                i += *p as usize;
            }
        }
        let count = bs.count_ones();
        let iter_count = bs.iter_set_bits().count();
        assert_eq!(count, iter_count, "count_ones and iter_set_bits should agree");
    }
}
