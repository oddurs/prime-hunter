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
use crate::kbn;
use crate::progress::Progress;
use crate::CoordinationClient;
use crate::{exact_digits, sieve};

/// Adaptive block size for twin prime search (same as kbn).
fn block_size_for_n(n: u64) -> u64 {
    match n {
        0..=1_000 => 10_000,
        1_001..=10_000 => 10_000,
        10_001..=50_000 => 2_000,
        50_001..=200_000 => 500,
        _ => 100,
    }
}

pub fn search(
    k: u64,
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
    let sieve_primes = sieve::generate_primes(sieve_limit);
    eprintln!(
        "Twin prime search: {}*{}^n ± 1, n=[{}, {}]",
        k, base, min_n, max_n
    );
    eprintln!(
        "Sieve initialized with {} primes up to {}",
        sieve_primes.len(),
        sieve_limit
    );

    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::Twin { last_n, .. }) if last_n >= min_n && last_n < max_n => {
            eprintln!("Resuming twin prime search from n={}", last_n + 1);
            last_n + 1
        }
        _ => min_n,
    };

    // Minimum n where k*b^n > sieve_limit
    let sieve_min_n = if base >= 2 {
        let log_b = (base as f64).log10();
        let log_limit = (sieve_limit as f64).log10();
        ((log_limit - (k as f64).log10().max(0.0)) / log_b).ceil() as u64 + 1
    } else {
        u64::MAX
    };
    eprintln!("Sieve active for n >= {}", sieve_min_n);

    eprintln!(
        "Running twin sieve over n=[{}..{}] ({} candidates)...",
        resume_from,
        max_n,
        max_n - resume_from + 1
    );
    let (plus_survives, minus_survives) =
        kbn::bsgs_sieve(resume_from, max_n, k, base, &sieve_primes, sieve_min_n);

    let total_range = max_n - resume_from + 1;
    let twin_survivors: u64 = plus_survives
        .iter()
        .zip(minus_survives.iter())
        .filter(|(&p, &m)| p && m)
        .count() as u64;
    eprintln!(
        "Sieve complete: {} twin pair candidates of {} ({:.1}%)",
        twin_survivors,
        total_range,
        twin_survivors as f64 / total_range as f64 * 100.0,
    );

    let mut last_checkpoint = Instant::now();
    let mut block_start = resume_from;
    let mut total_sieved: u64 = 0;

    while block_start <= max_n {
        let bsize = block_size_for_n(block_start);
        let block_end = (block_start + bsize - 1).min(max_n);
        let block_len = block_end - block_start + 1;

        *progress.current.lock().unwrap() =
            format!("{}*{}^[{}..{}]±1 twin", k, base, block_start, block_end);

        // Only keep n where BOTH forms survive the sieve
        let survivors: Vec<u64> = (block_start..=block_end)
            .filter(|&n| {
                let idx = (n - resume_from) as usize;
                plus_survives[idx] && minus_survives[idx]
            })
            .collect();

        total_sieved += block_len - survivors.len() as u64;

        let base_pow_start = Integer::from(base).pow(block_start as u32);
        let k_int = Integer::from(k);

        let found_twins: Vec<_> = survivors
            .into_par_iter()
            .filter_map(|n| {
                let offset = n - block_start;
                let base_pow = if offset == 0 {
                    base_pow_start.clone()
                } else {
                    Integer::from(&base_pow_start * Integer::from(base).pow(offset as u32))
                };
                let kb = Integer::from(&k_int * &base_pow);

                // Test +1 first (Proth is fast for composites)
                let plus = Integer::from(&kb + 1u32);
                let (r_plus, cert_plus) = kbn::test_prime(&plus, k, base, n, true, mr_rounds);
                if r_plus == IsPrime::No {
                    return None;
                }

                // +1 is (probably) prime, now test -1
                let minus = Integer::from(&kb - 1u32);
                if minus <= 0u32 {
                    return None;
                }
                let (r_minus, cert_minus) = kbn::test_prime(&minus, k, base, n, false, mr_rounds);
                if r_minus == IsPrime::No {
                    return None;
                }

                // Both are prime — twin pair found!
                let digits = exact_digits(&plus);
                let certainty = match (cert_plus, cert_minus) {
                    ("deterministic", "deterministic") => "deterministic",
                    _ => "probabilistic",
                };
                Some((n, digits, certainty.to_string()))
            })
            .collect();

        progress.tested.fetch_add(block_len, Ordering::Relaxed);

        for (n, digits, certainty) in found_twins {
            let expr = format!("{}*{}^{} +/- 1", k, base, n);
            progress.found.fetch_add(1, Ordering::Relaxed);
            if let Some(eb) = event_bus {
                eb.emit(events::Event::PrimeFound {
                    form: "twin".into(),
                    expression: expr.clone(),
                    digits,
                    proof_method: certainty.clone(),
                    timestamp: Instant::now(),
                });
            } else {
                eprintln!(
                    "*** TWIN PRIME PAIR FOUND: {} ({} digits, {}) ***",
                    expr, digits, certainty
                );
            }
            db.insert_prime_sync(rt, "twin", &expr, digits, search_params, &certainty)?;
            if let Some(wc) = worker_client {
                wc.report_prime("twin", &expr, digits, search_params, &certainty);
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Twin {
                    last_n: block_end,
                    k: Some(k),
                    base: Some(base),
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
                &Checkpoint::Twin {
                    last_n: block_end,
                    k: Some(k),
                    base: Some(base),
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
    eprintln!(
        "Twin prime search complete. Sieve eliminated {} candidates.",
        total_sieved
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb_plus(k: u64, base: u32, n: u64) -> Integer {
        Integer::from(k) * Integer::from(base).pow(n as u32) + 1u32
    }

    fn kb_minus(k: u64, base: u32, n: u64) -> Integer {
        Integer::from(k) * Integer::from(base).pow(n as u32) - 1u32
    }

    #[test]
    fn known_twin_pairs_k3_base2() {
        // k=3, base=2: twin pairs at n=1 (5,7), n=2 (11,13), n=6 (191,193)
        for &n in &[1u64, 2, 6] {
            let plus = kb_plus(3, 2, n);
            let minus = kb_minus(3, 2, n);
            assert_ne!(
                plus.is_probably_prime(25),
                IsPrime::No,
                "3*2^{}+1 = {} should be prime",
                n,
                plus
            );
            assert_ne!(
                minus.is_probably_prime(25),
                IsPrime::No,
                "3*2^{}-1 = {} should be prime",
                n,
                minus
            );
        }
    }

    #[test]
    fn known_twin_pairs_various_k() {
        // k=15, b=2, n=1: (29, 31)
        assert_ne!(kb_plus(15, 2, 1).is_probably_prime(25), IsPrime::No);
        assert_ne!(kb_minus(15, 2, 1).is_probably_prime(25), IsPrime::No);

        // k=9, b=2, n=3: (71, 73)
        assert_ne!(kb_plus(9, 2, 3).is_probably_prime(25), IsPrime::No);
        assert_ne!(kb_minus(9, 2, 3).is_probably_prime(25), IsPrime::No);
    }

    #[test]
    fn non_twin_one_composite() {
        // k=3, b=2, n=3: 3*8+1=25 (composite), 3*8-1=23 (prime)
        assert_eq!(
            kb_plus(3, 2, 3).is_probably_prime(25),
            IsPrime::No,
            "3*2^3+1=25 should be composite"
        );

        // k=3, b=2, n=4: 3*16+1=49=7^2 (composite), 3*16-1=47 (prime)
        assert_eq!(
            kb_plus(3, 2, 4).is_probably_prime(25),
            IsPrime::No,
            "3*2^4+1=49 should be composite"
        );
    }

    #[test]
    fn twin_deterministic_proof() {
        // k=3, base=2, n=6: (191, 193) — both should get deterministic proofs
        let plus = kb_plus(3, 2, 6);
        let minus = kb_minus(3, 2, 6);

        let (r_plus, cert_plus) = kbn::test_prime(&plus, 3, 2, 6, true, 25);
        assert_eq!(r_plus, IsPrime::Yes, "3*2^6+1=193 should be prime");
        assert_eq!(cert_plus, "deterministic");

        let (r_minus, cert_minus) = kbn::test_prime(&minus, 3, 2, 6, false, 25);
        assert_eq!(r_minus, IsPrime::Yes, "3*2^6-1=191 should be prime");
        assert_eq!(cert_minus, "deterministic");
    }

    #[test]
    fn twin_sieve_intersects_correctly() {
        let sieve_primes = sieve::generate_primes(10_000);
        let k = 3u64;
        let base = 2u32;
        let sieve_min_n = 14u64;

        let (plus_surv, minus_surv) = kbn::bsgs_sieve(1, 200, k, base, &sieve_primes, sieve_min_n);

        // Verify: when BOTH survive, at least check that the sieve was correct
        for n in sieve_min_n..=200 {
            let idx = (n - 1) as usize;
            if !plus_surv[idx] {
                let p = kb_plus(k, base, n);
                assert_eq!(
                    p.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said 3*2^{}+1 composite but it's prime",
                    n
                );
            }
            if !minus_surv[idx] {
                let m = kb_minus(k, base, n);
                assert_eq!(
                    m.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said 3*2^{}-1 composite but it's prime",
                    n
                );
            }
        }

        // Twin intersection should be a subset of both individual sieves
        let twin_count = (sieve_min_n..=200)
            .filter(|&n| {
                let idx = (n - 1) as usize;
                plus_surv[idx] && minus_surv[idx]
            })
            .count();
        let plus_count = (sieve_min_n..=200)
            .filter(|&n| plus_surv[(n - 1) as usize])
            .count();
        let minus_count = (sieve_min_n..=200)
            .filter(|&n| minus_surv[(n - 1) as usize])
            .count();
        assert!(twin_count <= plus_count);
        assert!(twin_count <= minus_count);
    }
}
