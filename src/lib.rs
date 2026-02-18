pub mod carol_kynea;
pub mod checkpoint;
pub mod cullen_woodall;
pub mod dashboard;
pub mod db;
pub mod deploy;
pub mod events;
pub mod factorial;
pub mod fleet;
pub mod gen_fermat;
pub mod kbn;
pub mod metrics;
pub mod near_repdigit;
pub mod palindromic;
pub mod pg_worker;
pub mod primorial;
pub mod progress;
pub mod proof;
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

/// Two-round Miller-Rabin pre-screening: run 2 fast rounds first, full rounds only for survivors.
/// Composites are rejected ~7x faster since most fail within 2 rounds.
pub fn mr_screened_test(candidate: &Integer, mr_rounds: u32) -> rug::integer::IsPrime {
    use rug::integer::IsPrime;
    if mr_rounds > 2 && candidate.is_probably_prime(2) == IsPrime::No {
        return IsPrime::No;
    }
    candidate.is_probably_prime(mr_rounds)
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
