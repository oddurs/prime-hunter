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

/// Adaptive block size (same as kbn/twin).
fn block_size_for_n(n: u64) -> u64 {
    match n {
        0..=1_000 => 10_000,
        1_001..=10_000 => 10_000,
        10_001..=50_000 => 2_000,
        50_001..=200_000 => 500,
        _ => 100,
    }
}

/// Search for Sophie Germain primes: p = k*b^n - 1 where both p and 2p+1 are prime.
///
/// 2p+1 = 2*k*b^n - 1, which is also a Riesel form with doubled k.
/// Both forms are LLR-testable when base=2.
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
    let k2 = k.checked_mul(2).expect("2*k overflows u64");
    let sieve_primes = sieve::generate_primes(sieve_limit);
    eprintln!(
        "Sophie Germain search: p={}*{}^n-1, 2p+1={}*{}^n-1, n=[{}, {}]",
        k, base, k2, base, min_n, max_n
    );
    eprintln!(
        "Sieve initialized with {} primes up to {}",
        sieve_primes.len(),
        sieve_limit
    );

    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::SophieGermain { last_n, .. }) if last_n >= min_n && last_n < max_n => {
            eprintln!("Resuming Sophie Germain search from n={}", last_n + 1);
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
    // For 2k form, sieve_min_n is at most the same (2k is larger)
    eprintln!("Sieve active for n >= {}", sieve_min_n);

    // Sieve for p = k*b^n - 1
    eprintln!(
        "Running sieve for p={}*{}^n-1 over n=[{}..{}]...",
        k, base, resume_from, max_n
    );
    let (_plus_surv_k, minus_surv_k) =
        kbn::bsgs_sieve(resume_from, max_n, k, base, &sieve_primes, sieve_min_n);

    // Sieve for 2p+1 = 2k*b^n - 1
    eprintln!(
        "Running sieve for 2p+1={}*{}^n-1 over n=[{}..{}]...",
        k2, base, resume_from, max_n
    );
    let (_plus_surv_k2, minus_surv_k2) =
        kbn::bsgs_sieve(resume_from, max_n, k2, base, &sieve_primes, sieve_min_n);

    let total_range = max_n - resume_from + 1;
    let sg_survivors: u64 = minus_surv_k
        .iter()
        .zip(minus_surv_k2.iter())
        .filter(|(&a, &b)| a && b)
        .count() as u64;
    eprintln!(
        "Sieve complete: {} Sophie Germain candidates of {} ({:.1}%)",
        sg_survivors,
        total_range,
        sg_survivors as f64 / total_range as f64 * 100.0,
    );

    let mut last_checkpoint = Instant::now();
    let mut block_start = resume_from;
    let mut total_sieved: u64 = 0;

    while block_start <= max_n {
        let bsize = block_size_for_n(block_start);
        let block_end = (block_start + bsize - 1).min(max_n);
        let block_len = block_end - block_start + 1;

        *progress.current.lock().unwrap() =
            format!("{}*{}^[{}..{}]-1 SG", k, base, block_start, block_end);

        // Only keep n where BOTH forms survive the sieve
        let survivors: Vec<u64> = (block_start..=block_end)
            .filter(|&n| {
                let idx = (n - resume_from) as usize;
                minus_surv_k[idx] && minus_surv_k2[idx]
            })
            .collect();

        total_sieved += block_len - survivors.len() as u64;

        let base_pow_start = Integer::from(base).pow(block_start as u32);
        let k_int = Integer::from(k);
        let k2_int = Integer::from(k2);

        let found: Vec<_> = survivors
            .into_par_iter()
            .filter_map(|n| {
                let offset = n - block_start;
                let base_pow = if offset == 0 {
                    base_pow_start.clone()
                } else {
                    Integer::from(&base_pow_start * Integer::from(base).pow(offset as u32))
                };

                // Test p = k*b^n - 1
                let p = Integer::from(&k_int * &base_pow) - 1u32;
                if p <= 0u32 {
                    return None;
                }
                let (r_p, cert_p) = kbn::test_prime(&p, k, base, n, false, mr_rounds);
                if r_p == IsPrime::No {
                    return None;
                }

                // p is (probably) prime, now test 2p+1 = 2k*b^n - 1
                let safe = Integer::from(&k2_int * &base_pow) - 1u32;
                let (r_safe, cert_safe) = kbn::test_prime(&safe, k2, base, n, false, mr_rounds);
                if r_safe == IsPrime::No {
                    return None;
                }

                // Sophie Germain pair found!
                let digits = exact_digits(&p);
                let certainty = match (cert_p, cert_safe) {
                    ("deterministic", "deterministic") => "deterministic",
                    _ => "probabilistic",
                };
                Some((n, digits, certainty.to_string()))
            })
            .collect();

        progress.tested.fetch_add(block_len, Ordering::Relaxed);

        for (n, digits, certainty) in found {
            let expr = format!("{}*{}^{}-1", k, base, n);
            progress.found.fetch_add(1, Ordering::Relaxed);
            if let Some(eb) = event_bus {
                eb.emit(events::Event::PrimeFound {
                    form: "sophie_germain".into(),
                    expression: expr.clone(),
                    digits,
                    proof_method: certainty.clone(),
                    timestamp: Instant::now(),
                });
            } else {
                eprintln!(
                    "*** SOPHIE GERMAIN PRIME FOUND: p={} ({} digits, {}) ***",
                    expr, digits, certainty
                );
            }
            db.insert_prime_sync(
                rt,
                "sophie_germain",
                &expr,
                digits,
                search_params,
                &certainty,
            )?;
            if let Some(wc) = worker_client {
                wc.report_prime("sophie_germain", &expr, digits, search_params, &certainty);
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::SophieGermain {
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
                &Checkpoint::SophieGermain {
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
        "Sophie Germain search complete. Sieve eliminated {} candidates.",
        total_sieved
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb_minus(k: u64, base: u32, n: u64) -> Integer {
        Integer::from(k) * Integer::from(base).pow(n as u32) - 1u32
    }

    #[test]
    fn known_sophie_germain_k1_base2() {
        // k=1, base=2: p = 2^n - 1 (Mersenne), safe = 2^(n+1) - 1
        // n=2: p=3, 2p+1=7 — both prime (SG pair)
        // n=3: p=7, 2p+1=15=3*5 — NOT SG
        // n=5: p=31, 2p+1=63=9*7 — NOT SG
        let p = kb_minus(1, 2, 2);
        assert_eq!(p, 3);
        assert_ne!(p.is_probably_prime(25), IsPrime::No);
        let safe = kb_minus(2, 2, 2); // 2*2^2 - 1 = 7
        assert_eq!(safe, 7);
        assert_ne!(safe.is_probably_prime(25), IsPrime::No);
    }

    #[test]
    fn known_sophie_germain_k3_base2() {
        // k=3, base=2: p = 3*2^n - 1
        // n=1: p=5, 2p+1=11 — both prime (SG pair)
        // n=2: p=11, 2p+1=23 — both prime (SG pair)
        // n=3: p=23, 2p+1=47 — both prime (SG pair)
        for &n in &[1u64, 2, 3] {
            let p = kb_minus(3, 2, n);
            let safe = kb_minus(6, 2, n);
            assert_ne!(
                p.is_probably_prime(25),
                IsPrime::No,
                "3*2^{}-1 = {} should be prime",
                n,
                p
            );
            assert_ne!(
                safe.is_probably_prime(25),
                IsPrime::No,
                "6*2^{}-1 = {} should be prime",
                n,
                safe
            );
        }
    }

    #[test]
    fn non_sophie_germain_p_prime_safe_composite() {
        // k=3, base=2, n=6: p=3*64-1=191 (prime), 2p+1=383 (prime!) — actually SG
        // k=3, base=2, n=4: p=3*16-1=47 (prime), 2p+1=95=5*19 — NOT SG
        let p = kb_minus(3, 2, 4);
        assert_eq!(p, 47);
        assert_ne!(p.is_probably_prime(25), IsPrime::No, "47 is prime");
        let safe = kb_minus(6, 2, 4); // 6*16-1 = 95
        assert_eq!(safe, 95);
        assert_eq!(
            safe.is_probably_prime(25),
            IsPrime::No,
            "95 = 5*19 is composite"
        );
    }

    #[test]
    fn non_sophie_germain_p_composite() {
        // k=3, base=2, n=5: p=3*32-1=95=5*19 (composite)
        let p = kb_minus(3, 2, 5);
        assert_eq!(p, 95);
        assert_eq!(
            p.is_probably_prime(25),
            IsPrime::No,
            "95 = 5*19 is composite"
        );
    }

    #[test]
    fn sophie_germain_deterministic_proof() {
        // k=3, base=2, n=2: p=11, safe=23 — both should get deterministic LLR proofs
        let p = kb_minus(3, 2, 2);
        let safe = kb_minus(6, 2, 2);

        let (r_p, cert_p) = kbn::test_prime(&p, 3, 2, 2, false, 25);
        assert_eq!(r_p, IsPrime::Yes, "3*2^2-1=11 should be prime");
        assert_eq!(cert_p, "deterministic");

        let (r_safe, cert_safe) = kbn::test_prime(&safe, 6, 2, 2, false, 25);
        assert_eq!(r_safe, IsPrime::Yes, "6*2^2-1=23 should be prime");
        assert_eq!(cert_safe, "deterministic");
    }

    #[test]
    fn sieve_intersects_correctly() {
        let sieve_primes = sieve::generate_primes(10_000);
        let k = 3u64;
        let k2 = 6u64;
        let base = 2u32;
        let sieve_min_n = 14u64;

        let (_plus_k, minus_k) = kbn::bsgs_sieve(1, 200, k, base, &sieve_primes, sieve_min_n);
        let (_plus_k2, minus_k2) = kbn::bsgs_sieve(1, 200, k2, base, &sieve_primes, sieve_min_n);

        // Verify sieve correctness: if sieved out, must be composite
        for n in sieve_min_n..=200 {
            let idx = (n - 1) as usize;
            if !minus_k[idx] {
                let p = kb_minus(k, base, n);
                assert_eq!(
                    p.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said 3*2^{}-1 composite but it's prime",
                    n
                );
            }
            if !minus_k2[idx] {
                let s = kb_minus(k2, base, n);
                assert_eq!(
                    s.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said 6*2^{}-1 composite but it's prime",
                    n
                );
            }
        }

        // SG intersection must be subset of both individual sieves
        let sg_count = (sieve_min_n..=200)
            .filter(|&n| {
                let idx = (n - 1) as usize;
                minus_k[idx] && minus_k2[idx]
            })
            .count();
        let k_count = (sieve_min_n..=200)
            .filter(|&n| minus_k[(n - 1) as usize])
            .count();
        let k2_count = (sieve_min_n..=200)
            .filter(|&n| minus_k2[(n - 1) as usize])
            .count();
        assert!(sg_count <= k_count);
        assert!(sg_count <= k2_count);
    }
}
