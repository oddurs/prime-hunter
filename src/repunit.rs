use anyhow::Result;
use rayon::prelude::*;
use rug::integer::IsPrime;
use rug::ops::Pow;
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

/// Sieve repunit exponents: for each sieve prime q, find the unique prime n
/// where q | R(b,n), and mark it composite.
///
/// For q ∤ (b-1): R(b,n) ≡ 0 (mod q) iff ord_q(b) | n.
///   Since n is prime, this means n = ord_q(b) (if ord_q(b) is prime).
/// For q | (b-1): R(b,n) ≡ n (mod q), so q | R(b,n) iff n = q.
fn sieve_repunit(
    exponents: &[u64],
    base: u32,
    sieve_primes: &[u64],
    sieve_min_n: u64,
) -> Vec<bool> {
    let mut survives = vec![true; exponents.len()];

    // Build exponent -> index map for O(1) lookup (only for n >= sieve_min_n)
    let n_to_idx: std::collections::HashMap<u64, usize> = exponents
        .iter()
        .enumerate()
        .filter(|(_, &n)| n >= sieve_min_n)
        .map(|(i, &n)| (n, i))
        .collect();

    let b = base as u64;
    let b_minus_1 = b - 1;

    for &q in sieve_primes {
        if q <= 1 || q == b {
            continue;
        }

        if b_minus_1 % q == 0 {
            // q | (b-1): R(b,n) ≡ n (mod q), composite iff n = q
            if let Some(&idx) = n_to_idx.get(&q) {
                survives[idx] = false;
            }
        } else {
            // q ∤ (b-1): R(b,n) has factor q iff n = ord_q(b) and n is prime
            let ord = sieve::multiplicative_order(b, q);
            if let Some(&idx) = n_to_idx.get(&ord) {
                survives[idx] = false;
            }
        }
    }

    survives
}

/// Search for repunit primes: R(b,n) = (b^n - 1)/(b - 1) for prime n.
pub fn search(
    base: u32,
    min_n: u64,
    max_n: u64,
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
    assert!(base >= 2, "Base must be >= 2");

    let sieve_primes = sieve::generate_primes(sieve_limit);
    eprintln!(
        "Repunit search: R({}, n) = ({}^n-1)/{}, n prime in [{}, {}]",
        base,
        base,
        base - 1,
        min_n,
        max_n
    );
    eprintln!(
        "Sieve initialized with {} primes up to {}",
        sieve_primes.len(),
        sieve_limit
    );

    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::Repunit { last_n, .. }) if last_n >= min_n && last_n < max_n => {
            eprintln!("Resuming repunit search from n={}", last_n + 1);
            last_n + 1
        }
        _ => min_n,
    };

    // Generate prime exponents in [resume_from, max_n]
    let all_primes = sieve::generate_primes(max_n);
    let prime_exponents: Vec<u64> = all_primes
        .into_iter()
        .filter(|&p| p >= resume_from)
        .collect();

    eprintln!("{} prime exponents in range", prime_exponents.len());

    if prime_exponents.is_empty() {
        checkpoint::clear(checkpoint_path);
        eprintln!("No prime exponents in range. Search complete.");
        return Ok(());
    }

    // Minimum n where R(b,n) > sieve_limit (so any factor found is a proper divisor)
    // R(b,n) ≈ b^(n-1), so n > log(sieve_limit) / log(b) + 1
    let sieve_min_n = if base >= 2 {
        let log_b = (base as f64).ln();
        ((sieve_limit as f64).ln() / log_b).ceil() as u64 + 1
    } else {
        u64::MAX
    };
    eprintln!("Sieve active for n >= {}", sieve_min_n);

    // Sieve
    let survives = sieve_repunit(&prime_exponents, base, &sieve_primes, sieve_min_n);
    let survivors: Vec<u64> = prime_exponents
        .iter()
        .zip(survives.iter())
        .filter(|(_, &s)| s)
        .map(|(&n, _)| n)
        .collect();

    let eliminated = prime_exponents.len() - survivors.len();
    eprintln!(
        "Sieve eliminated {} of {} candidates ({} survivors, {:.1}%)",
        eliminated,
        prime_exponents.len(),
        survivors.len(),
        survivors.len() as f64 / prime_exponents.len().max(1) as f64 * 100.0,
    );

    // Process in blocks for checkpointing
    let block_size = 100;
    let mut last_checkpoint = Instant::now();
    let b_minus_1 = (base - 1) as u32;

    for chunk in survivors.chunks(block_size) {
        let block_min = chunk[0];
        let block_max = chunk[chunk.len() - 1];

        *progress.current.lock().unwrap() = format!("R({}, [{}..{}])", base, block_min, block_max);

        let found: Vec<_> = chunk
            .par_iter()
            .filter_map(|&n| {
                let val = Integer::from(Integer::from(base).pow(n as u32) - 1u32) / b_minus_1;
                let result = mr_screened_test(&val, mr_rounds);
                if result == IsPrime::No {
                    return None;
                }
                let digits = exact_digits(&val);
                let certainty = if result == IsPrime::Yes {
                    "deterministic"
                } else {
                    "probabilistic"
                };
                Some((n, digits, certainty.to_string()))
            })
            .collect();

        progress
            .tested
            .fetch_add(chunk.len() as u64, Ordering::Relaxed);

        for (n, digits, certainty) in found {
            let expr = format!("R({}, {})", base, n);
            progress.found.fetch_add(1, Ordering::Relaxed);
            if let Some(eb) = event_bus {
                eb.emit(events::Event::PrimeFound {
                    form: "repunit".into(),
                    expression: expr.clone(),
                    digits,
                    proof_method: certainty.clone(),
                    timestamp: Instant::now(),
                });
            } else {
                eprintln!(
                    "*** REPUNIT PRIME FOUND: {} ({} digits, {}) ***",
                    expr, digits, certainty
                );
            }
            db.insert_prime_sync(rt, "repunit", &expr, digits, search_params, &certainty)?;
            if let Some(wc) = worker_client {
                wc.report_prime("repunit", &expr, digits, search_params, &certainty);
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Repunit {
                    last_n: block_max,
                    base: Some(base),
                    min_n: Some(min_n),
                    max_n: Some(max_n),
                },
            )?;
            eprintln!("Checkpoint saved at n={}", block_max);
            last_checkpoint = Instant::now();
        }

        if worker_client.is_some_and(|wc| wc.is_stop_requested()) {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Repunit {
                    last_n: block_max,
                    base: Some(base),
                    min_n: Some(min_n),
                    max_n: Some(max_n),
                },
            )?;
            eprintln!(
                "Stop requested by coordinator, checkpoint saved at n={}",
                block_max
            );
            return Ok(());
        }
    }

    checkpoint::clear(checkpoint_path);
    eprintln!("Repunit search complete.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repunit(base: u32, n: u64) -> Integer {
        (Integer::from(base).pow(n as u32) - 1u32) / (base - 1) as u32
    }

    #[test]
    fn known_repunit_primes_base10() {
        // R(10,2) = 11, R(10,19) = 1111111111111111111
        // Both are prime (OEIS A004023)
        for &n in &[2u64, 19, 23] {
            let r = repunit(10, n);
            assert_ne!(
                r.is_probably_prime(25),
                IsPrime::No,
                "R(10,{}) = {} should be prime",
                n,
                r
            );
        }
    }

    #[test]
    fn known_repunit_composites_base10() {
        // R(10,3) = 111 = 3*37
        // R(10,5) = 11111 = 41*271
        // R(10,7) = 1111111 = 239*4649
        // R(10,11) = 21649*513239
        for &n in &[3u64, 5, 7, 11, 13, 17] {
            let r = repunit(10, n);
            assert_eq!(
                r.is_probably_prime(25),
                IsPrime::No,
                "R(10,{}) = {} should be composite",
                n,
                r
            );
        }
    }

    #[test]
    fn known_repunit_primes_base2() {
        // R(2,n) = 2^n - 1 (Mersenne numbers)
        // Prime for n = 2, 3, 5, 7, 13, 17, 19
        for &n in &[2u64, 3, 5, 7, 13, 17, 19] {
            let r = repunit(2, n);
            assert_ne!(
                r.is_probably_prime(25),
                IsPrime::No,
                "R(2,{}) = {} should be prime (Mersenne)",
                n,
                r
            );
        }
    }

    #[test]
    fn known_repunit_primes_base3() {
        // R(3,n) prime for n = 3 (13), 7 (1093), 13 (797161)
        // OEIS A028491
        for &n in &[3u64, 7, 13] {
            let r = repunit(3, n);
            assert_ne!(
                r.is_probably_prime(25),
                IsPrime::No,
                "R(3,{}) = {} should be prime",
                n,
                r
            );
        }
    }

    #[test]
    fn repunit_composites_base3() {
        // R(3,5) = (243-1)/2 = 121 = 11^2
        let r = repunit(3, 5);
        assert_eq!(r, 121);
        assert_eq!(r.is_probably_prime(25), IsPrime::No, "R(3,5) = 121 = 11^2");

        // R(3,11) should be composite
        let r11 = repunit(3, 11);
        assert_eq!(
            r11.is_probably_prime(25),
            IsPrime::No,
            "R(3,11) should be composite"
        );
    }

    #[test]
    fn sieve_eliminates_composites() {
        let sieve_primes = sieve::generate_primes(10_000);
        let base = 10u32;

        // Minimum n where R(10,n) > 10000: R(10,n) ≈ 10^(n-1), so n >= 5
        let sieve_min_n = 5u64;

        // Get all prime exponents up to 200
        let all_primes = sieve::generate_primes(200);
        let exponents: Vec<u64> = all_primes.into_iter().filter(|&p| p >= 2).collect();

        let survives = sieve_repunit(&exponents, base, &sieve_primes, sieve_min_n);

        // Verify: if sieved out, must actually be composite
        for (i, &n) in exponents.iter().enumerate() {
            if !survives[i] {
                let r = repunit(base, n);
                assert_eq!(
                    r.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said R({},{}) composite but it's prime",
                    base,
                    n
                );
            }
        }

        // Verify known primes survive
        for &n in &[2u64, 19, 23] {
            if let Some(i) = exponents.iter().position(|&p| p == n) {
                assert!(
                    survives[i],
                    "Sieve incorrectly eliminated R(10,{}) which is prime",
                    n
                );
            }
        }
    }

    #[test]
    fn repunit_values_correct() {
        assert_eq!(repunit(10, 1), 1);
        assert_eq!(repunit(10, 2), 11);
        assert_eq!(repunit(10, 3), 111);
        assert_eq!(repunit(10, 4), 1111);
        assert_eq!(repunit(2, 3), 7); // 2^3-1 = 7
        assert_eq!(repunit(3, 3), 13); // (27-1)/2 = 13
    }
}
