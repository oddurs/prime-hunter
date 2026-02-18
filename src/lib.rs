pub mod checkpoint;
pub mod dashboard;
pub mod db;
pub mod factorial;
pub mod kbn;
pub mod palindromic;
pub mod progress;
pub mod sieve;

use rug::Integer;

/// Small primes for trial division pre-filter.
const SMALL_PRIMES: [u32; 64] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89,
    97, 101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191,
    193, 197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281, 283, 293,
    307, 311,
];

/// Quick check if n is divisible by any small prime.
/// Returns true if n is definitely composite (has a small factor).
/// Returns false if n might be prime (passed trial division).
pub fn has_small_factor(n: &Integer) -> bool {
    for &p in &SMALL_PRIMES {
        if n.is_divisible_u(p) {
            // If n equals the small prime itself, it's prime, not composite
            return n > &Integer::from(p);
        }
    }
    false
}

/// Estimate decimal digit count from bit length, avoiding expensive to_string conversion.
pub fn estimate_digits(n: &Integer) -> u64 {
    let bits = n.significant_bits();
    if bits == 0 {
        return 1;
    }
    (bits as f64 * std::f64::consts::LOG10_2) as u64 + 1
}

/// Exact decimal digit count (expensive for very large numbers).
pub fn exact_digits(n: &Integer) -> u64 {
    n.to_string_radix(10).len() as u64
}
