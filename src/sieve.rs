/// Sieve limit for generating small primes used in modular pre-filtering.
pub const SIEVE_LIMIT: u64 = 1_000_000;

/// Generate all primes up to `limit` using the Sieve of Eratosthenes.
pub fn generate_primes(limit: u64) -> Vec<u64> {
    if limit < 2 {
        return vec![];
    }
    let limit = limit as usize;
    let mut is_prime = vec![true; limit + 1];
    is_prime[0] = false;
    is_prime[1] = false;
    let mut i = 2;
    while i * i <= limit {
        if is_prime[i] {
            let mut j = i * i;
            while j <= limit {
                is_prime[j] = false;
                j += i;
            }
        }
        i += 1;
    }
    is_prime
        .iter()
        .enumerate()
        .filter(|(_, &p)| p)
        .map(|(i, _)| i as u64)
        .collect()
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
