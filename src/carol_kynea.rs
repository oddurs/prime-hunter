use anyhow::Result;
use rayon::prelude::*;
use rug::integer::IsPrime;
use rug::ops::{Pow, RemRounding};
use rug::Integer;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use crate::checkpoint::{self, Checkpoint};
use crate::db::Database;
use crate::events::{self, EventBus};
use crate::kbn;
use crate::progress::Progress;
use crate::proof;
use crate::CoordinationClient;
use crate::{exact_digits, mr_screened_test, sieve};

/// Sieve Carol/Kynea candidates using incremental modular arithmetic.
///
/// Carol_n = (2^n - 1)^2 - 2 = 4^n - 2·2^n - 1
/// Kynea_n = (2^n + 1)^2 - 2 = 4^n + 2·2^n - 1
///
/// For each sieve prime q > 3, tracks g2 = 2^n mod q and g4 = 4^n mod q.
/// Carol composite when g4 ≡ 2·g2 + 1 (mod q).
/// Kynea composite when g4 + 2·g2 ≡ 1 (mod q).
fn sieve_carol_kynea(
    min_n: u64,
    max_n: u64,
    sieve_primes: &[u64],
    sieve_min_n: u64,
) -> (Vec<bool>, Vec<bool>) {
    let range = (max_n - min_n + 1) as usize;
    let mut carol_survives = vec![true; range];
    let mut kynea_survives = vec![true; range];

    let total_primes = sieve_primes.len();
    let log_interval = (total_primes / 20).max(1);

    for (pi, &q) in sieve_primes.iter().enumerate() {
        if pi % log_interval == 0 && pi > 0 {
            eprintln!(
                "  Carol/Kynea sieve: {}/{} primes ({:.0}%)",
                pi,
                total_primes,
                pi as f64 / total_primes as f64 * 100.0
            );
        }

        // Carol and Kynea are never divisible by 2 or 3
        if q <= 3 {
            continue;
        }

        let mut g2 = sieve::pow_mod(2, min_n, q);
        let mut g4 = sieve::pow_mod(4, min_n, q);

        for n in min_n..=max_n {
            if n >= sieve_min_n {
                let idx = (n - min_n) as usize;
                let two_g2 = 2 * g2 % q;

                // Carol: g4 ≡ 2·g2 + 1 (mod q) means Carol_n divisible by q
                if carol_survives[idx] && g4 == (two_g2 + 1) % q {
                    carol_survives[idx] = false;
                }

                // Kynea: g4 + 2·g2 ≡ 1 (mod q) means Kynea_n divisible by q
                if kynea_survives[idx] && (g4 + two_g2) % q == 1 {
                    kynea_survives[idx] = false;
                }
            }

            g2 = 2 * g2 % q;
            g4 = 4 * g4 % q;
        }
    }

    (carol_survives, kynea_survives)
}

/// LLR test for N = k·2^exp - 1 where k is an arbitrary-precision Integer.
/// Used when k exceeds u64 (n > 64 for Carol/Kynea).
fn llr_test_big(candidate: &Integer, k: &Integer, exp: u64) -> Option<bool> {
    if exp < 3 {
        return None;
    }

    let p_val = if !k.is_divisible_u(3) {
        4u32
    } else {
        kbn::find_rodseth_v1(candidate)
    };

    let mut u = proof::lucas_v_big(k, p_val, candidate);

    let iters = exp - 2;
    for i in 0..iters {
        if exp > 50_000 && i % 10_000 == 0 && i > 0 {
            eprintln!(
                "  LLR: {}/{} squarings ({:.1}%)",
                i,
                iters,
                i as f64 / iters as f64 * 100.0
            );
        }
        u.square_mut();
        u -= 2u32;
        u = u.rem_euc(candidate);
    }

    Some(u == 0u32)
}

/// Test primality of a Carol number (2^n - 1)^2 - 2 using LLR.
///
/// Carol_n = (2^(n-1) - 1) · 2^(n+1) - 1, so k = 2^(n-1) - 1 (odd for n ≥ 2), exp = n+1.
fn test_carol(candidate: &Integer, n: u64, mr_rounds: u32) -> (IsPrime, &'static str) {
    if n >= 2 {
        let exp = n + 1;
        if n <= 64 {
            let k = (1u64 << (n - 1)) - 1;
            if let Some(result) = kbn::llr_test(candidate, k, exp) {
                return if result {
                    (IsPrime::Yes, "deterministic (LLR)")
                } else {
                    (IsPrime::No, "")
                };
            }
        } else {
            let k = (Integer::from(1u32) << (n - 1) as u32) - 1u32;
            if let Some(result) = llr_test_big(candidate, &k, exp) {
                return if result {
                    (IsPrime::Yes, "deterministic (LLR)")
                } else {
                    (IsPrime::No, "")
                };
            }
        }
    }

    let r = mr_screened_test(candidate, mr_rounds);
    let cert = match r {
        IsPrime::Yes => "deterministic",
        IsPrime::Probably => "probabilistic",
        IsPrime::No => "",
    };
    (r, cert)
}

/// Test primality of a Kynea number (2^n + 1)^2 - 2 using LLR.
///
/// Kynea_n = (2^(n-1) + 1) · 2^(n+1) - 1, so k = 2^(n-1) + 1 (odd for n ≥ 2), exp = n+1.
fn test_kynea(candidate: &Integer, n: u64, mr_rounds: u32) -> (IsPrime, &'static str) {
    if n >= 2 {
        let exp = n + 1;
        if n <= 64 {
            let k = (1u64 << (n - 1)) + 1;
            if let Some(result) = kbn::llr_test(candidate, k, exp) {
                return if result {
                    (IsPrime::Yes, "deterministic (LLR)")
                } else {
                    (IsPrime::No, "")
                };
            }
        } else {
            let k = (Integer::from(1u32) << (n - 1) as u32) + 1u32;
            if let Some(result) = llr_test_big(candidate, &k, exp) {
                return if result {
                    (IsPrime::Yes, "deterministic (LLR)")
                } else {
                    (IsPrime::No, "")
                };
            }
        }
    }

    let r = mr_screened_test(candidate, mr_rounds);
    let cert = match r {
        IsPrime::Yes => "deterministic",
        IsPrime::Probably => "probabilistic",
        IsPrime::No => "",
    };
    (r, cert)
}

/// Adaptive block size for Carol/Kynea search.
fn block_size_for_n(n: u64) -> u64 {
    match n {
        0..=1_000 => 10_000,
        1_001..=10_000 => 5_000,
        10_001..=50_000 => 1_000,
        50_001..=200_000 => 200,
        _ => 50,
    }
}

pub fn search(
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
    let sieve_primes = sieve::generate_primes(sieve_limit);
    eprintln!(
        "Sieve initialized with {} primes up to {}",
        sieve_primes.len(),
        sieve_limit
    );

    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::CarolKynea { last_n, .. }) if last_n >= min_n && last_n < max_n => {
            eprintln!("Resuming Carol/Kynea search from n={}", last_n + 1);
            last_n + 1
        }
        _ => min_n,
    };

    // Minimum n where Carol_n = (2^n-1)^2-2 > sieve_limit
    let sieve_min_n = {
        let mut n = 2u64;
        while n <= 63 {
            let two_n: u128 = 1 << n;
            if (two_n - 1) * (two_n - 1) - 2 > sieve_limit as u128 {
                break;
            }
            n += 1;
        }
        n
    };
    eprintln!("Sieve active for n >= {}", sieve_min_n);

    eprintln!(
        "Running Carol/Kynea sieve over n=[{}..{}] ({} candidates)...",
        resume_from,
        max_n,
        max_n - resume_from + 1
    );
    let (carol_survives, kynea_survives) =
        sieve_carol_kynea(resume_from, max_n, &sieve_primes, sieve_min_n);
    let carol_survivors: u64 = carol_survives.iter().filter(|&&b| b).count() as u64;
    let kynea_survivors: u64 = kynea_survives.iter().filter(|&&b| b).count() as u64;
    let total_range = max_n - resume_from + 1;
    eprintln!(
        "Sieve complete: Carol survivors {}/{} ({:.1}%), Kynea survivors {}/{} ({:.1}%)",
        carol_survivors,
        total_range,
        carol_survivors as f64 / total_range as f64 * 100.0,
        kynea_survivors,
        total_range,
        kynea_survivors as f64 / total_range as f64 * 100.0,
    );

    let mut last_checkpoint = Instant::now();
    let mut block_start = resume_from;
    let mut total_sieved: u64 = 0;

    while block_start <= max_n {
        let bsize = block_size_for_n(block_start);
        let block_end = (block_start + bsize - 1).min(max_n);
        let block_len = block_end - block_start + 1;

        *progress.current.lock().unwrap() =
            format!("(2^n±1)^2-2 n=[{}..{}]", block_start, block_end);

        let survivors: Vec<(u64, bool, bool)> = (block_start..=block_end)
            .filter_map(|n| {
                let idx = (n - resume_from) as usize;
                let tc = carol_survives[idx] && n >= 2; // Carol invalid for n < 2
                let tk = kynea_survives[idx];
                if tc || tk {
                    Some((n, tc, tk))
                } else {
                    None
                }
            })
            .collect();

        total_sieved += block_len - survivors.len() as u64;

        let found_primes: Vec<_> = survivors
            .into_par_iter()
            .flat_map_iter(|(n, test_carol_flag, test_kynea_flag)| {
                let two_n = Integer::from(1u32) << n as u32;
                let mut results = Vec::new();

                if test_carol_flag {
                    let carol = Integer::from(&two_n - 1u32).pow(2) - 2u32;
                    let (r, cert) = test_carol(&carol, n, mr_rounds);
                    if r != IsPrime::No {
                        let digits = exact_digits(&carol);
                        results.push((
                            format!("(2^{}-1)^2-2", n),
                            digits,
                            cert.to_string(),
                            "carol",
                        ));
                    }
                }

                if test_kynea_flag {
                    let kynea = Integer::from(&two_n + 1u32).pow(2) - 2u32;
                    let (r, cert) = test_kynea(&kynea, n, mr_rounds);
                    if r != IsPrime::No {
                        let digits = exact_digits(&kynea);
                        results.push((
                            format!("(2^{}+1)^2-2", n),
                            digits,
                            cert.to_string(),
                            "kynea",
                        ));
                    }
                }

                results
            })
            .collect();

        progress.tested.fetch_add(block_len * 2, Ordering::Relaxed);

        for (expr, digits, certainty, form) in found_primes {
            progress.found.fetch_add(1, Ordering::Relaxed);
            if let Some(eb) = event_bus {
                eb.emit(events::Event::PrimeFound {
                    form: form.to_string(),
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
            db.insert_prime_sync(rt, form, &expr, digits, search_params, &certainty)?;
            if let Some(wc) = worker_client {
                wc.report_prime(form, &expr, digits, search_params, &certainty);
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::CarolKynea {
                    last_n: block_end,
                    min_n: Some(min_n),
                    max_n: Some(max_n),
                },
            )?;
            eprintln!(
                "Checkpoint saved at n={} (sieved out: {})",
                block_end, total_sieved
            );
            last_checkpoint = Instant::now();
        }

        if worker_client.is_some_and(|wc| wc.is_stop_requested()) {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::CarolKynea {
                    last_n: block_end,
                    min_n: Some(min_n),
                    max_n: Some(max_n),
                },
            )?;
            eprintln!(
                "Stop requested by coordinator, checkpoint saved at n={}",
                block_end
            );
            return Ok(());
        }

        block_start = block_end + 1;
    }

    checkpoint::clear(checkpoint_path);
    eprintln!("Carol/Kynea sieve eliminated {} candidates.", total_sieved);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sieve;

    fn carol(n: u64) -> Integer {
        let two_n = Integer::from(1u32) << n as u32;
        Integer::from(&two_n - 1u32).pow(2) - 2u32
    }

    fn kynea(n: u64) -> Integer {
        let two_n = Integer::from(1u32) << n as u32;
        Integer::from(&two_n + 1u32).pow(2) - 2u32
    }

    #[test]
    fn known_carol_primes() {
        // OEIS A091515: Carol primes at n = 2, 3, 4, 6, 7, 10, 12, 15, 18, 19, 21, 25, 27
        for &n in &[2u64, 3, 4, 6, 7, 10, 12, 15, 18, 19] {
            let c = carol(n);
            assert_ne!(
                c.is_probably_prime(25),
                IsPrime::No,
                "Carol({}) = {} should be prime",
                n,
                c
            );
        }
    }

    #[test]
    fn known_carol_composites() {
        for &n in &[5u64, 8, 9, 11, 13, 14, 16, 17, 20, 22] {
            let c = carol(n);
            assert_eq!(
                c.is_probably_prime(25),
                IsPrime::No,
                "Carol({}) should be composite",
                n
            );
        }
    }

    #[test]
    fn known_kynea_primes() {
        // OEIS A091513: Kynea primes at n = 0, 1, 2, 3, 5, 8, 9, 12, 15, 17, 18, 21, 23
        for &n in &[0u64, 1, 2, 3, 5, 8, 9, 12, 15, 17, 18, 21, 23] {
            let k = kynea(n);
            assert_ne!(
                k.is_probably_prime(25),
                IsPrime::No,
                "Kynea({}) = {} should be prime",
                n,
                k
            );
        }
    }

    #[test]
    fn known_kynea_composites() {
        for &n in &[4u64, 6, 7, 10, 11, 13, 14, 16, 19, 20] {
            let k = kynea(n);
            assert_eq!(
                k.is_probably_prime(25),
                IsPrime::No,
                "Kynea({}) should be composite",
                n
            );
        }
    }

    #[test]
    fn llr_proves_carol() {
        for &n in &[2u64, 3, 4, 6, 7, 10, 12, 15, 18, 19] {
            let c = carol(n);
            let (r, cert) = test_carol(&c, n, 25);
            assert_eq!(r, IsPrime::Yes, "Carol({}) should be prime", n);
            assert!(
                cert.contains("deterministic"),
                "Carol({}) should be deterministic, got: {}",
                n,
                cert
            );
        }
    }

    #[test]
    fn llr_proves_kynea() {
        // Skip n=0,1 since LLR requires n >= 2 (exp >= 3)
        for &n in &[2u64, 3, 5, 8, 9, 12, 15, 17, 18, 21, 23] {
            let k = kynea(n);
            let (r, cert) = test_kynea(&k, n, 25);
            assert_eq!(r, IsPrime::Yes, "Kynea({}) should be prime", n);
            assert!(
                cert.contains("deterministic"),
                "Kynea({}) should be deterministic, got: {}",
                n,
                cert
            );
        }
    }

    #[test]
    fn sieve_correctly_eliminates() {
        let sieve_primes = sieve::generate_primes(10_000);
        let sieve_min_n = {
            let mut n = 2u64;
            while n <= 63 {
                let two_n: u128 = 1 << n;
                if (two_n - 1) * (two_n - 1) - 2 > 10_000 {
                    break;
                }
                n += 1;
            }
            n
        };

        let (carol_surv, kynea_surv) = sieve_carol_kynea(2, 200, &sieve_primes, sieve_min_n);

        for n in sieve_min_n..=200 {
            let idx = (n - 2) as usize;
            if !carol_surv[idx] {
                let c = carol(n);
                assert_eq!(
                    c.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said Carol({}) composite but it's prime",
                    n
                );
            }
            if !kynea_surv[idx] {
                let k = kynea(n);
                assert_eq!(
                    k.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said Kynea({}) composite but it's prime",
                    n
                );
            }
        }
    }

    #[test]
    fn carol_kynea_decomposition() {
        // Verify k*2^exp - 1 decomposition for LLR
        for n in 2..=20u64 {
            let k_carol = (1u64 << (n - 1)) - 1;
            let exp = n + 1;
            let reconstructed = Integer::from(k_carol) * Integer::from(2u32).pow(exp as u32) - 1u32;
            assert_eq!(
                reconstructed,
                carol(n),
                "Carol decomposition failed for n={}",
                n
            );

            let k_kynea = (1u64 << (n - 1)) + 1;
            let reconstructed = Integer::from(k_kynea) * Integer::from(2u32).pow(exp as u32) - 1u32;
            assert_eq!(
                reconstructed,
                kynea(n),
                "Kynea decomposition failed for n={}",
                n
            );
        }
    }
}
