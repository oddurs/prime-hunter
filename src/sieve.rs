/// Sieve limit for generating small primes used in modular pre-filtering.
pub const SIEVE_LIMIT: u64 = 10_000_000;

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

/// Modular inverse via Fermat's little theorem: a^(p-2) mod p.
/// Returns None if a ≡ 0 (mod p). Requires p prime.
pub fn mod_inverse(a: u64, p: u64) -> Option<u64> {
    if a % p == 0 {
        return None;
    }
    Some(pow_mod(a, p - 2, p))
}

/// Trial-division factorization of a u64 into (prime, exponent) pairs.
pub fn factor_u64(mut n: u64) -> Vec<(u64, u32)> {
    let mut factors = Vec::new();
    let mut d = 2u64;
    while d * d <= n {
        if n % d == 0 {
            let mut exp = 0u32;
            while n % d == 0 {
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
/// Requires p prime and base not divisible by p.
pub fn multiplicative_order(base: u64, p: u64) -> u64 {
    let mut order = p - 1;
    let factors = factor_u64(order);
    for (q, _) in factors {
        while order % q == 0 && pow_mod(base, order / q, p) == 1 {
            order /= q;
        }
    }
    order
}

/// Baby-step giant-step discrete logarithm.
/// Finds x in [0, order) such that base^x ≡ target (mod p), or None if no solution.
pub fn discrete_log_bsgs(base: u64, target: u64, p: u64, order: u64) -> Option<u64> {
    use std::collections::HashMap;

    let m = (order as f64).sqrt().ceil() as u64;
    if m == 0 {
        return None;
    }

    // Baby steps: base^j mod p → j, for j = 0..m-1
    let mut table = HashMap::with_capacity(m as usize);
    let mut power = 1u64;
    for j in 0..m {
        table.insert(power, j);
        power = (power as u128 * base as u128 % p as u128) as u64;
    }

    // Giant step factor: base^(-m) mod p
    let base_inv = match mod_inverse(base, p) {
        Some(inv) => inv,
        None => return None,
    };
    let giant_step = pow_mod(base_inv, m, p);

    // Giant steps: check target * (base^(-m))^i for i = 0..m
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
    fn test_pow_mod() {
        assert_eq!(pow_mod(2, 10, 1000), 24); // 1024 mod 1000
        assert_eq!(pow_mod(3, 4, 100), 81);
        assert_eq!(pow_mod(5, 0, 7), 1);
    }
}
