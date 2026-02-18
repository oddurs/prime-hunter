use anyhow::Result;
use rayon::prelude::*;
use rug::integer::IsPrime;
use rug::ops::Pow;
use rug::Integer;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::checkpoint::{self, Checkpoint};
use crate::db::Database;
use crate::progress::Progress;
use crate::{exact_digits, sieve};

const BLOCK_SIZE: u64 = 10_000;

/// Sieve a block of k*b^n±1 candidates using modular arithmetic.
///
/// For each n in [block_start, block_end], checks divisibility of k*b^n+1 and
/// k*b^n-1 by all sieve primes. Uses incremental computation of b^n mod p
/// within the block. Returns survivors that need full primality testing.
///
/// For n < sieve_min_n, candidates are too small for the sieve to be safe
/// (they might equal a sieve prime), so they bypass sieve and always survive.
fn sieve_block(
    block_start: u64,
    block_end: u64,
    k: u64,
    base: u32,
    sieve_primes: &[u64],
    sieve_min_n: u64,
) -> Vec<(u64, bool, bool)> {
    let base_u64 = base as u64;

    // Precompute per-prime constants
    let k_mod: Vec<u64> = sieve_primes.iter().map(|&p| k % p).collect();
    let b_mod: Vec<u64> = sieve_primes.iter().map(|&p| base_u64 % p).collect();
    let mut b_pow: Vec<u64> = sieve_primes
        .iter()
        .map(|&p| sieve::pow_mod(base_u64, block_start, p))
        .collect();

    let mut survivors = Vec::new();

    for n in block_start..=block_end {
        if n < sieve_min_n {
            // Small candidates bypass the sieve
            survivors.push((n, true, true));
        } else {
            let mut plus_survives = true;
            let mut minus_survives = true;

            // Check divisibility by sieve primes (with early exit)
            for i in 0..sieve_primes.len() {
                let p = sieve_primes[i];
                // k*b^n mod p — safe: both operands < p < 10^6, product < 10^12 fits u64
                let kb_mod = k_mod[i] * b_pow[i] % p;

                if plus_survives && kb_mod == p - 1 {
                    plus_survives = false;
                }
                if minus_survives && kb_mod == 1 {
                    minus_survives = false;
                }
                if !plus_survives && !minus_survives {
                    break;
                }
            }

            if plus_survives || minus_survives {
                survivors.push((n, plus_survives, minus_survives));
            }
        }

        // Always increment: b^(n+1) mod p = b^n * b mod p
        for i in 0..sieve_primes.len() {
            b_pow[i] = b_pow[i] * b_mod[i] % sieve_primes[i];
        }
    }

    survivors
}

pub fn search(
    k: u64,
    base: u32,
    min_n: u64,
    max_n: u64,
    progress: &Arc<Progress>,
    db: &Arc<Mutex<Database>>,
    checkpoint_path: &Path,
    search_params: &str,
) -> Result<()> {
    let sieve_primes = sieve::generate_primes(sieve::SIEVE_LIMIT);
    eprintln!(
        "Sieve initialized with {} primes up to {}",
        sieve_primes.len(),
        sieve::SIEVE_LIMIT
    );

    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::Kbn { last_n }) if last_n >= min_n && last_n < max_n => {
            eprintln!("Resuming kbn search from n={}", last_n + 1);
            last_n + 1
        }
        _ => min_n,
    };

    // Minimum n where k*b^n > SIEVE_LIMIT, making the sieve safe.
    // Below this, candidates might equal a sieve prime, so they bypass sieve.
    let sieve_min_n = if base >= 2 {
        let log_b = (base as f64).log10();
        let log_limit = (sieve::SIEVE_LIMIT as f64).log10();
        ((log_limit - (k as f64).log10().max(0.0)) / log_b).ceil() as u64 + 1
    } else {
        u64::MAX
    };
    eprintln!("Sieve active for n >= {}", sieve_min_n);

    let mut last_checkpoint = Instant::now();
    let mut block_start = resume_from;
    let mut total_sieved: u64 = 0;

    while block_start <= max_n {
        let block_end = (block_start + BLOCK_SIZE - 1).min(max_n);
        let block_len = block_end - block_start + 1;

        *progress.current.lock().unwrap() =
            format!("{}*{}^[{}..{}]+-1", k, base, block_start, block_end);

        let survivors = sieve_block(block_start, block_end, k, base, &sieve_primes, sieve_min_n);

        total_sieved += block_len - survivors.len() as u64;

        // Test survivors in parallel — only NOW do we compute huge Integers
        let found_primes: Vec<_> = survivors
            .into_par_iter()
            .flat_map_iter(|(n, test_plus, test_minus)| {
                let base_pow = Integer::from(base).pow(n as u32);
                let kb = Integer::from(k) * &base_pow;
                let mut results = Vec::new();

                if test_plus {
                    let plus = kb.clone() + 1u32;
                    let r = plus.is_probably_prime(25);
                    if r != IsPrime::No {
                        let digits = exact_digits(&plus);
                        let cert = match r {
                            IsPrime::Yes => "deterministic",
                            _ => "probabilistic",
                        };
                        results.push((
                            format!("{}*{}^{} + 1", k, base, n),
                            digits,
                            cert.to_string(),
                        ));
                    }
                }

                if test_minus {
                    let minus = kb - 1u32;
                    let r = minus.is_probably_prime(25);
                    if r != IsPrime::No {
                        let digits = exact_digits(&minus);
                        let cert = match r {
                            IsPrime::Yes => "deterministic",
                            _ => "probabilistic",
                        };
                        results.push((
                            format!("{}*{}^{} - 1", k, base, n),
                            digits,
                            cert.to_string(),
                        ));
                    }
                }

                results
            })
            .collect();

        progress.tested.fetch_add(block_len * 2, Ordering::Relaxed);

        for (expr, digits, certainty) in found_primes {
            progress.found.fetch_add(1, Ordering::Relaxed);
            eprintln!(
                "*** PRIME FOUND: {} ({} digits, {}) ***",
                expr, digits, certainty
            );
            db.lock()
                .unwrap()
                .insert_prime("kbn", &expr, digits, search_params)?;
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(checkpoint_path, &Checkpoint::Kbn { last_n: block_end })?;
            eprintln!(
                "Checkpoint saved at n={} (sieved out: {})",
                block_end, total_sieved
            );
            last_checkpoint = Instant::now();
        }

        block_start = block_end + 1;
    }

    checkpoint::clear(checkpoint_path);
    eprintln!("KBN sieve eliminated {} candidates.", total_sieved);
    Ok(())
}
