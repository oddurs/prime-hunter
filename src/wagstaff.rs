use anyhow::Result;
use rayon::prelude::*;
use rug::integer::IsPrime;
use rug::Integer;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use crate::checkpoint::{self, Checkpoint};
use crate::db::Database;
use crate::events::{self, EventBus};
use crate::progress::Progress;
use crate::CoordinationClient;
use crate::{exact_digits, mr_screened_test, sieve};

/// Precomputed sieve data for Wagstaff composites.
///
/// For sieve prime q > 3 with ord_q(2) ≡ 2 (mod 4):
///   (2^p + 1)/3 is divisible by q when p ≡ ord/2 (mod ord).
///
/// Only entries where ord ≡ 2 (mod 4) are kept because ord ≡ 0 (mod 4)
/// yields an even half_ord, and no odd prime p can satisfy p ≡ even (mod ord).
struct WagstaffSieve {
    entries: Vec<(u64, u64)>,
}

impl WagstaffSieve {
    fn new(sieve_primes: &[u64]) -> Self {
        let mut entries: Vec<(u64, u64)> = sieve_primes
            .par_iter()
            .filter(|&&q| q > 3)
            .filter_map(|&q| {
                let ord = sieve::multiplicative_order(2, q);
                if ord % 4 == 2 {
                    Some((ord, ord / 2))
                } else {
                    None
                }
            })
            .collect();
        // Sort by ord so small orders (more eliminating power) are checked first.
        entries.sort_unstable_by_key(|&(ord, _)| ord);
        // Deduplicate: multiple primes can share the same (ord, half) pair.
        entries.dedup();
        WagstaffSieve { entries }
    }

    /// Check if (2^p + 1)/3 has a small factor from the sieve.
    fn is_composite(&self, p: u64) -> bool {
        self.entries.iter().any(|&(ord, half)| p % ord == half)
    }
}

/// Adaptive block size: fewer candidates per block for larger exponents.
fn block_size_for_exp(exp: u64) -> usize {
    match exp {
        0..=10_000 => 500,
        10_001..=100_000 => 100,
        100_001..=1_000_000 => 20,
        _ => 5,
    }
}

pub fn search(
    min_exp: u64,
    max_exp: u64,
    progress: &Arc<Progress>,
    db: &Arc<Database>,
    rt: &tokio::runtime::Handle,
    checkpoint_path: &Path,
    search_params: &str,
    mr_rounds: u32,
    sieve_limit: u64,
    worker_client: Option<&dyn CoordinationClient>,
    event_bus: Option<&EventBus>,
) -> Result<()> {
    // Generate prime exponents in range (p must be odd prime >= 3)
    let all_primes = sieve::generate_primes(max_exp);
    let candidate_exponents: Vec<u64> = all_primes
        .iter()
        .copied()
        .filter(|&p| p >= min_exp.max(3))
        .collect();

    if candidate_exponents.is_empty() {
        eprintln!("No prime exponents in range [{}, {}]", min_exp, max_exp);
        return Ok(());
    }

    eprintln!(
        "Testing {} prime exponents in [{}, {}] for Wagstaff primes (2^p+1)/3",
        candidate_exponents.len(),
        candidate_exponents[0],
        candidate_exponents.last().unwrap()
    );

    // Build modular sieve
    let sieve_primes = sieve::generate_primes(sieve_limit);
    eprintln!(
        "Computing multiplicative orders for {} sieve primes...",
        sieve_primes.len()
    );
    let wsieve = WagstaffSieve::new(&sieve_primes);
    eprintln!(
        "Wagstaff sieve ready ({} active entries)",
        wsieve.entries.len()
    );

    // Minimum exponent where (2^p+1)/3 > sieve_limit, making sieve safe
    let sieve_min_exp = ((sieve_limit as f64 * 3.0).log2().ceil()) as u64;
    eprintln!("Sieve active for p >= {}", sieve_min_exp);

    // Load checkpoint
    let resume_exp = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::Wagstaff { last_exp, .. })
            if last_exp >= min_exp && last_exp < max_exp =>
        {
            eprintln!("Resuming Wagstaff search from after p={}", last_exp);
            last_exp
        }
        _ => 0,
    };

    let candidates: Vec<u64> = candidate_exponents
        .iter()
        .copied()
        .filter(|&p| p > resume_exp)
        .collect();

    if candidates.is_empty() {
        eprintln!("All candidates already processed.");
        checkpoint::clear(checkpoint_path);
        return Ok(());
    }

    let mut last_checkpoint = Instant::now();
    let mut sieved_out: u64 = 0;
    let mut pos = 0;

    while pos < candidates.len() {
        let bsize = block_size_for_exp(candidates[pos]);
        let block_end = (pos + bsize).min(candidates.len());
        let block = &candidates[pos..block_end];
        let block_min = block[0];
        let block_max = *block.last().unwrap();

        *progress.current.lock().unwrap() = format!("(2^p+1)/3 p=[{}..{}]", block_min, block_max);

        // Apply sieve filter (parallelized across candidates)
        let survivors: Vec<u64> = block
            .par_iter()
            .copied()
            .filter(|&p| p < sieve_min_exp || !wsieve.is_composite(p))
            .collect();

        sieved_out += (block.len() - survivors.len()) as u64;

        // Test survivors with Miller-Rabin
        let found_primes: Vec<_> = survivors
            .into_par_iter()
            .filter_map(|p| {
                let two_p_plus_1 = (Integer::from(1u32) << p as u32) + 1u32;
                debug_assert!(
                    two_p_plus_1.is_divisible_u(3),
                    "2^{} + 1 must be divisible by 3 for odd prime p",
                    p
                );
                let candidate = two_p_plus_1 / 3u32;

                let r = mr_screened_test(&candidate, mr_rounds);
                if r != IsPrime::No {
                    let digits = exact_digits(&candidate);
                    let certainty = match r {
                        IsPrime::Yes => "deterministic",
                        IsPrime::Probably => "probabilistic",
                        IsPrime::No => unreachable!(),
                    };
                    Some((p, digits, certainty.to_string()))
                } else {
                    None
                }
            })
            .collect();

        progress
            .tested
            .fetch_add(block.len() as u64, Ordering::Relaxed);

        for (p, digits, certainty) in found_primes {
            let expr = format!("(2^{}+1)/3", p);
            progress.found.fetch_add(1, Ordering::Relaxed);
            if let Some(eb) = event_bus {
                eb.emit(events::Event::PrimeFound {
                    form: "wagstaff".into(),
                    expression: expr.clone(),
                    digits,
                    proof_method: certainty.clone(),
                    timestamp: Instant::now(),
                });
            } else {
                eprintln!(
                    "*** PRIME FOUND: {} ({} digits, {}) ***",
                    expr, digits, certainty
                );
            }
            db.insert_prime_sync(rt, "wagstaff", &expr, digits, search_params, &certainty)?;
            if let Some(wc) = worker_client {
                wc.report_prime("wagstaff", &expr, digits, search_params, &certainty);
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Wagstaff {
                    last_exp: block_max,
                    min_exp: Some(min_exp),
                    max_exp: Some(max_exp),
                },
            )?;
            eprintln!(
                "Checkpoint saved at p={} (sieved out: {})",
                block_max, sieved_out
            );
            last_checkpoint = Instant::now();
        }

        if worker_client.is_some_and(|wc| wc.is_stop_requested()) {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Wagstaff {
                    last_exp: block_max,
                    min_exp: Some(min_exp),
                    max_exp: Some(max_exp),
                },
            )?;
            eprintln!(
                "Stop requested by coordinator, checkpoint saved at p={}",
                block_max
            );
            return Ok(());
        }

        pos = block_end;
    }

    checkpoint::clear(checkpoint_path);
    eprintln!(
        "Wagstaff sieve eliminated {} of {} candidates.",
        sieved_out,
        candidates.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sieve;

    fn wagstaff(p: u64) -> Integer {
        ((Integer::from(1u32) << p as u32) + 1u32) / 3u32
    }

    #[test]
    fn known_wagstaff_primes() {
        // OEIS A000978: exponents p where (2^p+1)/3 is prime
        for &p in &[3u64, 5, 7, 11, 13, 17, 19, 23, 31, 43] {
            let w = wagstaff(p);
            assert_ne!(
                w.is_probably_prime(25),
                IsPrime::No,
                "(2^{}+1)/3 = {} should be prime",
                p,
                w
            );
        }
    }

    #[test]
    fn known_wagstaff_composites() {
        for &p in &[29u64, 37, 41, 47, 53, 59, 67, 71, 73, 83, 89, 97] {
            let w = wagstaff(p);
            assert_eq!(
                w.is_probably_prime(25),
                IsPrime::No,
                "(2^{}+1)/3 should be composite",
                p
            );
        }
    }

    #[test]
    fn wagstaff_requires_odd_exponent() {
        // Even exponents: 2^p + 1 is not divisible by 3
        let val = (Integer::from(1u32) << 2u32) + 1u32;
        assert!(!val.is_divisible_u(3));
        let val = (Integer::from(1u32) << 4u32) + 1u32;
        assert!(!val.is_divisible_u(3));

        // Odd exponents: 2^p + 1 is always divisible by 3
        let val = (Integer::from(1u32) << 3u32) + 1u32;
        assert!(val.is_divisible_u(3));
        let val = (Integer::from(1u32) << 5u32) + 1u32;
        assert!(val.is_divisible_u(3));
    }

    #[test]
    fn sieve_correctly_eliminates() {
        let sieve_primes = sieve::generate_primes(100_000);
        let wsieve = WagstaffSieve::new(&sieve_primes);
        let sieve_min_exp = ((100_000f64 * 3.0).log2().ceil()) as u64;

        let test_primes = sieve::generate_primes(500);
        for &p in &test_primes {
            if p < 3 || p < sieve_min_exp {
                continue;
            }
            if wsieve.is_composite(p) {
                let w = wagstaff(p);
                assert_eq!(
                    w.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said (2^{}+1)/3 composite but MR says prime",
                    p
                );
            }
        }
    }

    #[test]
    fn sieve_preserves_known_primes() {
        let sieve_primes = sieve::generate_primes(100_000);
        let wsieve = WagstaffSieve::new(&sieve_primes);
        let sieve_min_exp = ((100_000f64 * 3.0).log2().ceil()) as u64;

        for &p in &[61u64, 79, 101, 127, 167, 191, 199] {
            if p >= sieve_min_exp {
                assert!(
                    !wsieve.is_composite(p),
                    "Sieve incorrectly eliminated known Wagstaff prime p={}",
                    p
                );
            }
        }
    }

    #[test]
    fn multiplicative_order_sieve_condition() {
        // ord_11(2) = 10, 10 % 4 == 2 → included in sieve, half = 5
        assert_eq!(sieve::multiplicative_order(2, 11), 10);
        // 2^5 ≡ 32 ≡ 10 ≡ -1 (mod 11)
        assert_eq!(sieve::pow_mod(2, 5, 11), 10);

        // ord_5(2) = 4, 4 % 4 == 0 → excluded (half=2 is even, never matches odd prime)
        assert_eq!(sieve::multiplicative_order(2, 5), 4);

        // ord_23(2) = 11, odd → excluded (2^p can never be -1 mod 23)
        assert_eq!(sieve::multiplicative_order(2, 23), 11);
    }

    #[test]
    fn sieve_optimization_only_odd_half() {
        // Verify that entries with even half_ord are excluded
        let sieve_primes = sieve::generate_primes(1000);
        let wsieve = WagstaffSieve::new(&sieve_primes);

        for &(ord, half) in &wsieve.entries {
            assert_eq!(
                ord % 4,
                2,
                "Entry with ord={} should not be in sieve (ord%4={})",
                ord,
                ord % 4
            );
            assert_eq!(half % 2, 1, "Entry with half={} should be odd", half);
        }
    }
}
