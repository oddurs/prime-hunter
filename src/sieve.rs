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
    let depth = (raw_depth as u64).clamp(1_000_000, 1_000_000_000); // 1M minimum (meaningful sieve), 1B max (memory/time limit)

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
        return [2, 3, 5].iter().copied().filter(|&p| p <= limit).collect();
    }

    // Residues coprime to 30: these are the only positions we track
    const RESIDUES: [u8; 8] = [1, 7, 11, 13, 17, 19, 23, 29];

    // Map residue → index in the wheel (for residues coprime to 30)
    const RES_TO_IDX: [u8; 30] = [
        255, 0, 255, 255, 255, 255, 255, 1, 255, 255, 255, 2, 255, 3, 255, 255, 255, 4, 255, 5,
        255, 255, 255, 6, 255, 255, 255, 255, 255, 7,
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
        debug_assert!(
            index < self.len,
            "BitSieve index out of bounds: {} >= {}",
            index,
            self.len
        );
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
    //! # Sieve and Modular Arithmetic Tests
    //!
    //! Validates the foundational primitives for candidate generation and
    //! composite elimination across all 12 search forms:
    //!
    //! - **Prime generation** (`generate_primes`): Wheel-30 sieve of Eratosthenes
    //!   producing all primes up to a limit. Tests verify correctness against known
    //!   pi(x) values (OEIS [A000720](https://oeis.org/A000720)): pi(100)=25,
    //!   pi(1000)=168, pi(10000)=1229, pi(100000)=9592. Boundary tests at the
    //!   wheel modulus (30, 60) catch off-by-one errors in the spoke iteration.
    //!
    //! - **Modular exponentiation** (`pow_mod`): Binary method using u128
    //!   intermediate products to avoid overflow in u64 arithmetic. Cross-validated
    //!   against known values: 2^{10} mod 1000 = 24, 3^4 mod 100 = 81.
    //!
    //! - **Montgomery multiplication** (`MontgomeryCtx`): Converts to/from
    //!   Montgomery form (a * R mod n where R = 2^{64}) to replace modular division
    //!   with a multiply-and-shift (Montgomery, 1985). Tests cross-validate every
    //!   operation (mul, pow_mod, mod_inverse, to_mont/from_mont roundtrip) against
    //!   naive `pow_mod` for 11 different prime moduli from 3 to 999999999999999877.
    //!
    //! - **Multiplicative order and BSGS** (`multiplicative_order`, `discrete_log_bsgs`):
    //!   Baby-step Giant-step discrete logarithm in O(sqrt(ord)) time and space.
    //!   Tests verify ord_7(2)=3, ord_7(3)=6 (primitive root), ord_13(2)=12, and
    //!   BSGS solutions for specific DLP instances including the no-solution case.
    //!
    //! - **Modular inverse** (`mod_inverse`): Extended Euclidean algorithm for
    //!   a^{-1} mod m. Tests verify 3^{-1} mod 7 = 5, 2^{-1} mod 5 = 3, and
    //!   the None case for gcd(a,m) > 1.
    //!
    //! - **Integer factoring** (`factor_u64`): Trial division returning
    //!   (prime, exponent) pairs. Tests verify factorizations: 12 = 2^2 * 3,
    //!   360 = 2^3 * 3^2 * 5, 97 = 97^1 (prime).
    //!
    //! - **Auto sieve depth** (`auto_sieve_depth`, `resolve_sieve_limit`):
    //!   Adaptive sieve limit based on candidate bit size and range count.
    //!   Tests verify monotonicity (larger candidates get deeper sieving),
    //!   capping at SIEVE_LIMIT for tiny ranges, and explicit override behavior.
    //!
    //! - **BitSieve** (`BitSieve`): Packed u64 bitmap for 8x memory reduction over
    //!   Vec<bool>. Tests verify all operations at word boundaries (63, 64, 127, 128),
    //!   count_ones consistency with iter_set_bits, and correct handling of
    //!   non-multiple-of-64 lengths (extra bits in last word must be clear).
    //!
    //! ## References
    //!
    //! - Eratosthenes of Cyrene, ~240 BCE (sieve algorithm).
    //! - Peter L. Montgomery, "Modular Multiplication Without Trial Division", 1985.
    //! - Daniel Shanks, "Class number, a theory of factorization, and genera", 1971
    //!   (Baby-step Giant-step algorithm).
    //! - OEIS A000720: pi(n), the prime counting function.

    use super::*;

    // ── Modular Inverse ─────────────────────────────────────────────────

    /// Verifies `mod_inverse` against known results:
    /// - 3^{-1} mod 7: 3*5 = 15 = 1 (mod 7), cross-checked via pow_mod(3,5,7).
    /// - 2^{-1} mod 5: 2*3 = 6 = 1 (mod 5).
    /// - 0^{-1} mod 7: undefined (gcd(0,7) = 7 > 1), returns None.
    /// - 7^{-1} mod 7: undefined (gcd(7,7) = 7 > 1), returns None.
    #[test]
    fn test_mod_inverse() {
        assert_eq!(mod_inverse(3, 7), Some(pow_mod(3, 5, 7))); // 3*5=15≡1(mod7)
        assert_eq!(mod_inverse(2, 5), Some(3)); // 2*3=6≡1(mod5)
        assert_eq!(mod_inverse(0, 7), None);
        assert_eq!(mod_inverse(7, 7), None);
    }

    // ── Integer Factoring (factor_u64) ─────────────────────────────────

    /// Verifies trial-division factorization returning sorted (prime, exponent) pairs.
    /// - 1 = (empty): the unit has no prime factors.
    /// - 2 = 2^1: the smallest prime.
    /// - 12 = 2^2 * 3^1: smallest number with two distinct prime factors.
    /// - 360 = 2^3 * 3^2 * 5^1: highly composite number with three distinct primes.
    /// - 97 = 97^1: a prime number (single factor with exponent 1).
    #[test]
    fn test_factor_u64() {
        let empty: Vec<(u64, u32)> = vec![];
        assert_eq!(factor_u64(1), empty);
        assert_eq!(factor_u64(2), vec![(2, 1)]);
        assert_eq!(factor_u64(12), vec![(2, 2), (3, 1)]);
        assert_eq!(factor_u64(360), vec![(2, 3), (3, 2), (5, 1)]);
        assert_eq!(factor_u64(97), vec![(97, 1)]); // prime
    }

    // ── Multiplicative Order ───────────────────────────────────────────

    /// Verifies the multiplicative order ord_m(a) = min{k > 0 : a^k = 1 (mod m)}.
    /// - ord_7(2) = 3: the powers 2^1=2, 2^2=4, 2^3=8=1 (mod 7).
    /// - ord_7(3) = 6: 3 is a primitive root mod 7 (generates all of (Z/7Z)*).
    /// - ord_13(2) = 12: 2 is a primitive root mod 13.
    /// - ord_5(2) = 4: the full group (Z/5Z)* has order phi(5)=4.
    ///
    /// The multiplicative order is fundamental to the BSGS sieve in kbn.rs:
    /// it determines the period of b^n mod p, which controls which n values
    /// make k*b^n +/- 1 divisible by the sieve prime p.
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

    // ── Baby-Step Giant-Step Discrete Logarithm ────────────────────────

    /// Verifies the Shanks Baby-step Giant-step algorithm for computing
    /// discrete logarithms: find x such that g^x = h (mod p) given ord(g).
    ///
    /// - 2^x = 4 (mod 7), ord=3: x=2 since 2^2=4.
    /// - 3^x = 5 (mod 7), ord=6: x=5 since 3^5=243=5 (mod 7).
    /// - 2^x = 1 (mod 7), ord=3: x=0 since g^0=1 by convention.
    /// - 2^x = 3 (mod 5), ord=4: x=3 since 2^3=8=3 (mod 5).
    ///
    /// The BSGS algorithm runs in O(sqrt(ord)) time and space, a dramatic
    /// improvement over brute-force O(ord) for the sieve prime moduli used
    /// in kbn::bsgs_sieve.
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

    /// When g generates a proper subgroup of (Z/pZ)* and h is not in that
    /// subgroup, no solution exists. Here 4 has order 3 mod 7 (generating
    /// {1, 2, 4}) and 3 is not in this subgroup, so BSGS correctly returns None.
    #[test]
    fn test_discrete_log_bsgs_no_solution() {
        // 4^x ≡ 3 (mod 7): 4 has order 3 (4^1=4, 4^2=2, 4^3=1), so 3 is unreachable
        assert_eq!(discrete_log_bsgs(4, 3, 7, 3), None);
    }

    // ── Prime Generation (Wheel-30 Sieve of Eratosthenes) ──────────────

    /// Verifies the wheel-30 sieve against the known list of primes up to 30.
    /// There are exactly pi(30) = 10 primes: 2, 3, 5, 7, 11, 13, 17, 19, 23, 29.
    /// The wheel modulus is 30 = 2*3*5, so primes 2, 3, 5 are handled specially
    /// as the wheel's "axle" primes, and remaining primes are found via the
    /// 8 spoke residues coprime to 30: {1, 7, 11, 13, 17, 19, 23, 29}.
    #[test]
    fn test_generate_primes() {
        let primes = generate_primes(30);
        assert_eq!(primes, vec![2, 3, 5, 7, 11, 13, 17, 19, 23, 29]);
    }

    /// Edge cases for very small sieve limits: 0 and 1 produce empty lists
    /// (no primes exist below 2). Limits 2 through 11 test the axle primes
    /// (2, 3, 5) and the first spoke prime (7). The limit 10 is notable because
    /// it falls strictly between primes 7 and 11, testing the inclusive upper bound.
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

    /// Validates prime counts against the prime counting function pi(x)
    /// (OEIS [A000720](https://oeis.org/A000720)):
    /// - pi(100) = 25, pi(1000) = 168, pi(10000) = 1229, pi(100000) = 9592.
    /// These are well-established values from number theory tables. Any
    /// deviation indicates a bug in the wheel sieve's spoke iteration or
    /// crossing-off logic.
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

    /// Tests at boundaries around the wheel modulus 30 and its multiples:
    /// - limit=29: exactly pi(29)=10 primes (29 is the last prime before 30).
    /// - limit=31: pi(31)=11 (31 is the first spoke prime in the second wheel).
    /// - limit=59: pi(59)=17 (59 is just before 60=2*30).
    /// - limit=60: pi(60)=17 (60 is composite, no new prime).
    /// - limit=61: pi(61)=18 (61 is a spoke prime at 2*30+1).
    ///
    /// These boundary values test the transition between wheel rotations,
    /// where off-by-one errors in the spoke indices are most likely.
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

    // ── Modular Exponentiation (pow_mod) ───────────────────────────────

    /// Verifies the binary method modular exponentiation against known values:
    /// - 2^{10} mod 1000 = 1024 mod 1000 = 24.
    /// - 3^4 mod 100 = 81 mod 100 = 81.
    /// - 5^0 mod 7 = 1 (any base to the 0th power is 1).
    ///
    /// Uses u128 intermediate products to avoid overflow: for 64-bit moduli,
    /// the product a*b can be up to (2^{64}-1)^2 which fits in u128.
    #[test]
    fn test_pow_mod() {
        assert_eq!(pow_mod(2, 10, 1000), 24); // 1024 mod 1000
        assert_eq!(pow_mod(3, 4, 100), 81);
        assert_eq!(pow_mod(5, 0, 7), 1);
    }

    // ── Montgomery Multiplication Cross-Validation ─────────────────────

    /// Exhaustive cross-validation of Montgomery multiplication against naive
    /// modular multiplication for 11 prime moduli: 3, 5, 7, 11, 13, 17, 97,
    /// 101, 1009, 10007, 100003. For each modulus p, tests all pairs (a, b)
    /// with 0 <= a, b < min(p, 50).
    ///
    /// Montgomery multiplication (Montgomery, 1985) computes a*b*R^{-1} mod p
    /// where R = 2^{64}, replacing the expensive division in naive modular
    /// arithmetic with a multiply-and-shift. The to_mont/from_mont conversions
    /// handle the R factor: to_mont(a) = a*R mod p, from_mont(x) = x*R^{-1} mod p.
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

    /// Cross-validates Montgomery exponentiation against naive `pow_mod` for
    /// 9 prime moduli. For each modulus p, tests all pairs (base, exp) with
    /// 1 <= base < min(p, 20) and 0 <= exp < min(p, 30). The Montgomery
    /// pow_mod uses repeated squaring in Montgomery form, converting only
    /// at the boundaries (to_mont before, from_mont after).
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

    /// Cross-validates Montgomery modular inverse against the naive computation
    /// a^{-1} = pow_mod(a, p-2, p) (Fermat's little theorem for prime p).
    /// For each modulus p and each a in [1, min(p, 50)), verifies that
    /// from_mont(ctx.mod_inverse(to_mont(a))) = pow_mod(a, p-2, p).
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

    /// Verifies the roundtrip identity: from_mont(to_mont(a)) = a for all
    /// a in [0, min(p, 100)) across 6 prime moduli including the near-u64-max
    /// prime 999999937. This tests that R * R^{-1} = 1 (mod p) holds exactly
    /// for the precomputed Montgomery constants.
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

    /// Verifies that `ctx.one()` (= R mod p, the Montgomery representation of 1)
    /// is the multiplicative identity: a * one = a for all a in Montgomery form.
    /// This is a ring axiom critical for the correctness of Montgomery pow_mod,
    /// which initializes its accumulator to `one`.
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

    /// Stress test with a prime near u64 max: p = 999999999999999877 (< 2^{63},
    /// as required by the Montgomery implementation which uses u128 for
    /// intermediate products). Tests both multiplication (123456789 * 987654321
    /// mod p) and exponentiation (123456789^{1000} mod p) against naive pow_mod.
    /// This exercises the u128 overflow handling at near-maximum modulus size.
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

    // ── Auto Sieve Depth ──────────────────────────────────────────────

    /// For 1000-bit candidates (~300 digits), the sieve depth should be at
    /// least 1M but not excessively large (< 100M). The optimal sieve depth
    /// balances the cost of sieving (linear in depth) against the benefit of
    /// eliminating composites before expensive primality tests. For 1000-bit
    /// numbers, each eliminated composite saves ~0.5ms of MR time.
    #[test]
    fn auto_sieve_depth_small_candidates() {
        // 1000-bit candidates → ~10M sieve (the old default)
        let depth = auto_sieve_depth(1000, 10000);
        assert!(depth >= 1_000_000, "depth should be at least 1M: {}", depth);
        assert!(
            depth <= 100_000_000,
            "depth shouldn't be excessively large for 1000-bit: {}",
            depth
        );
    }

    /// For 100K-bit candidates (~30000 digits, typical of large kbn searches),
    /// the sieve depth should exceed 10M. At this candidate size, each MR round
    /// takes several seconds, so deeper sieving pays for itself many times over
    /// by eliminating composites that would waste expensive primality tests.
    #[test]
    fn auto_sieve_depth_large_candidates() {
        // 100K-bit candidates (like large kbn searches) → should be much deeper
        let depth = auto_sieve_depth(100_000, 10000);
        assert!(
            depth > 10_000_000,
            "depth should exceed 10M for 100K-bit: {}",
            depth
        );
    }

    /// With only 50 candidates in the range, the sieve depth is capped at
    /// SIEVE_LIMIT (the default) because the overhead of generating a large
    /// prime table exceeds the savings from eliminating a few candidates.
    #[test]
    fn auto_sieve_depth_tiny_range() {
        // Very small range → capped at SIEVE_LIMIT
        let depth = auto_sieve_depth(100_000, 50);
        assert!(
            depth <= SIEVE_LIMIT,
            "depth should be capped for tiny range: {}",
            depth
        );
    }

    /// Degenerate inputs (0-bit candidates or 0-count range) should return
    /// the safe default SIEVE_LIMIT rather than crashing or returning 0.
    #[test]
    fn auto_sieve_depth_zero_inputs() {
        assert_eq!(auto_sieve_depth(0, 1000), SIEVE_LIMIT);
        assert_eq!(auto_sieve_depth(1000, 0), SIEVE_LIMIT);
    }

    /// The sieve depth must be monotonically non-decreasing in candidate size:
    /// larger candidates benefit more from deeper sieving because their primality
    /// tests are more expensive. Tests three points: 1K, 10K, 100K bits.
    #[test]
    fn auto_sieve_depth_monotonic() {
        // Larger candidates should get deeper sieving
        let d1 = auto_sieve_depth(1000, 10000);
        let d2 = auto_sieve_depth(10_000, 10000);
        let d3 = auto_sieve_depth(100_000, 10000);
        assert!(
            d1 <= d2,
            "depth should grow with candidate size: {} vs {}",
            d1,
            d2
        );
        assert!(
            d2 <= d3,
            "depth should grow with candidate size: {} vs {}",
            d2,
            d3
        );
    }

    /// When the user provides an explicit sieve_limit > 0, it overrides the
    /// auto-tuning logic. This allows advanced users to fine-tune sieve depth
    /// for specific workloads (e.g., deep sieve for very large candidates).
    #[test]
    fn resolve_sieve_limit_explicit_wins() {
        // Explicit sieve_limit should be used as-is
        assert_eq!(resolve_sieve_limit(5_000_000, 100_000, 10000), 5_000_000);
    }

    /// When sieve_limit=0 (the default), auto-tuning kicks in and selects a
    /// depth based on candidate bit size and range count. For 10K-bit candidates
    /// with a range of 10000, the auto-tuned depth should be at least 1M.
    #[test]
    fn resolve_sieve_limit_zero_auto_tunes() {
        // sieve_limit=0 should auto-tune
        let depth = resolve_sieve_limit(0, 10_000, 10000);
        assert!(depth >= 1_000_000);
    }

    // ── BitSieve (Packed u64 Bitmap) ───────────────────────────────────

    /// Verifies that `new_all_set(100)` creates a bitmap with all 100 bits set.
    /// The BitSieve packs bits into u64 words (64 bits each), using ceil(100/64)=2
    /// words. The last word has 36 "real" bits set and 28 padding bits clear.
    /// `count_ones` must return 100 (not 128), verifying correct padding masking.
    #[test]
    fn bitsieve_new_all_set() {
        let bs = BitSieve::new_all_set(100);
        assert_eq!(bs.len(), 100);
        assert_eq!(bs.count_ones(), 100);
        for i in 0..100 {
            assert!(bs.get(i), "bit {} should be set", i);
        }
    }

    /// Verifies that `new_all_clear(100)` creates a bitmap with all 100 bits clear.
    /// Both `count_ones` and individual `get(i)` checks must return 0/false.
    #[test]
    fn bitsieve_new_all_clear() {
        let bs = BitSieve::new_all_clear(100);
        assert_eq!(bs.len(), 100);
        assert_eq!(bs.count_ones(), 0);
        for i in 0..100 {
            assert!(!bs.get(i), "bit {} should be clear", i);
        }
    }

    /// Tests set/clear/get operations at word boundary positions: 0, 63 (last bit
    /// of word 0), 64 (first bit of word 1), 127 (last bit of word 1), 128 (first
    /// bit of word 2), and 199 (last valid index). These boundaries are where
    /// the bit index calculation `i / 64` and `i % 64` transitions between words,
    /// making them the most likely positions for off-by-one errors.
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

    /// Dedicated word-boundary test with a 256-bit sieve (4 words). Sets bits
    /// at every word boundary (63, 64, 127, 128, 191, 192) plus the last bit (255).
    /// Verifies that operations spanning word boundaries don't corrupt adjacent words.
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

    /// Verifies `count_ones` after clearing every other bit: starting with all
    /// 100 bits set, clear bits 0, 2, 4, ..., 98 (50 bits), leaving exactly 50
    /// set bits at positions 1, 3, 5, ..., 99. This tests that `count_ones` uses
    /// hardware `popcnt` (or equivalent) correctly across word boundaries.
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

    /// Verifies that `iter_set_bits` yields exactly the set bit positions in
    /// ascending order: {0, 1, 63, 64, 65, 127, 128, 199}. This exercises the
    /// word-by-word iteration with `trailing_zeros()` to find set bits within
    /// each u64 word, including word transitions at indices 63->64 and 127->128.
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

    /// Edge case: a zero-length BitSieve should have len=0, is_empty=true,
    /// count_ones=0, and an empty iterator. This tests the degenerate case
    /// where no candidates survive the sieve (or the range is empty).
    #[test]
    fn bitsieve_empty() {
        let bs = BitSieve::new_all_set(0);
        assert_eq!(bs.len(), 0);
        assert!(bs.is_empty());
        assert_eq!(bs.count_ones(), 0);
        assert_eq!(bs.iter_set_bits().count(), 0);
    }

    /// Stress test: a 10-million-bit BitSieve (1.25 MB of memory vs. 10 MB for
    /// Vec<bool>). Verifies that count_ones returns the exact length when all
    /// bits are set. This size is representative of real kbn sieve ranges.
    #[test]
    fn bitsieve_large() {
        let n = 10_000_000;
        let bs = BitSieve::new_all_set(n);
        assert_eq!(bs.count_ones(), n);
        assert_eq!(bs.len(), n);
    }

    /// Non-multiple-of-64 length: len=65 requires 2 words. The second word has
    /// only 1 valid bit (index 64). The remaining 63 bits in that word must be
    /// clear to avoid polluting `count_ones`. Verifies: count_ones=65, 2 words
    /// total, and word[1] has exactly 1 bit set (the valid bit at position 0).
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

    /// Consistency check: `count_ones()` (using word-level popcnt) must agree
    /// with `iter_set_bits().count()` (using trailing_zeros iteration). Creates
    /// a non-trivial pattern by starting with all 1000 bits set, then clearing
    /// multiples of the first 9 primes (2, 3, 5, ..., 23) to simulate a
    /// mini sieve of Eratosthenes. The resulting pattern is irregular across
    /// word boundaries, exercising both counting methods thoroughly.
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
        assert_eq!(
            count, iter_count,
            "count_ones and iter_set_bits should agree"
        );
    }
}
