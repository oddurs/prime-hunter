//! # Cullen & Woodall — n·2^n ± 1 Prime Search
//!
//! Searches for Cullen primes (n·2^n + 1) and Woodall primes (n·2^n − 1)
//! simultaneously over a range of n. Both are extremely sparse: only 16 Cullen
//! primes and 45 Woodall primes are known (as of 2025).
//!
//! ## Algorithm
//!
//! 1. **Incremental modular sieve**: For each sieve prime p, tracks
//!    f(n) = n·2^n mod p using the recurrence:
//!    - g(n+1) = 2·g(n) mod p,  where g(n) = 2^n mod p
//!    - f(n+1) = 2·f(n) + g(n+1) mod p
//!      This avoids computing n·2^n from scratch for each n.
//!
//! 2. **Proth test for Cullen** (`test_cullen`): C_n = n·2^n + 1 is of the
//!    form k·2^n + 1 with k = n. Since n < 2^n for all n ≥ 1, Proth's theorem
//!    always applies, yielding deterministic proofs.
//!
//! 3. **LLR test for Woodall** (`test_woodall`): W_n = n·2^n − 1. Decompose
//!    n = m·2^e (m odd), giving W_n = m·2^(n+e) − 1, which is the LLR form
//!    k·2^exp − 1 with k = m (odd) and exp = n + e.
//!
//! ## Complexity
//!
//! - Sieve: O(π(L) · (max_n − min_n)) — linear scan per prime.
//! - Proth test: O(n · M(n)) per Cullen survivor.
//! - LLR test: O((n+e) · M(n)) per Woodall survivor.
//!
//! ## References
//!
//! - OEIS: [A005849](https://oeis.org/A005849) — Cullen primes (n such that n·2^n + 1 is prime).
//! - OEIS: [A002234](https://oeis.org/A002234) — Woodall primes (n such that n·2^n − 1 is prime).
//! - James Cullen, "Question 15897", Educational Times, 1905.
//! - Allan J.C. Cunningham and H.J. Woodall, "Factorisation of Q = (2^q ∓ q)
//!   and q·2^q ∓ 1", Messenger of Mathematics, 1917.

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
use crate::pfgw;
use crate::progress::Progress;
use crate::CoordinationClient;
use crate::{exact_digits, mr_screened_test, sieve};

/// Sieve Cullen/Woodall candidates n*2^n ± 1 using modular arithmetic.
///
/// For each sieve prime p, incrementally tracks n*2^n mod p using the recurrence:
///   g(n+1) = 2*g(n) mod p           where g(n) = 2^n mod p
///   f(n+1) = 2*f(n) + g(n+1) mod p  where f(n) = n*2^n mod p
///
/// Marks n as composite for Cullen if f ≡ p-1 (mod p), for Woodall if f ≡ 1 (mod p).
fn sieve_cullen_woodall(
    min_n: u64,
    max_n: u64,
    sieve_primes: &[u64],
    sieve_min_n: u64,
) -> (Vec<bool>, Vec<bool>) {
    let range = (max_n - min_n + 1) as usize;
    let mut cullen_survives = vec![true; range];
    let mut woodall_survives = vec![true; range];

    let total_primes = sieve_primes.len();
    let log_interval = (total_primes / 20).max(1);

    for (pi, &p) in sieve_primes.iter().enumerate() {
        if pi % log_interval == 0 && pi > 0 {
            eprintln!(
                "  Cullen/Woodall sieve: {}/{} primes ({:.0}%)",
                pi,
                total_primes,
                pi as f64 / total_primes as f64 * 100.0
            );
        }

        if p == 2 {
            continue; // n*2^n is always even, so n*2^n±1 is always odd
        }

        // Initialize: g = 2^min_n mod p, f = min_n * 2^min_n mod p
        let mut g = sieve::pow_mod(2, min_n, p);
        let mut f = (min_n % p) * g % p;

        for n in min_n..=max_n {
            if n >= sieve_min_n {
                let idx = (n - min_n) as usize;
                if cullen_survives[idx] && f == p - 1 {
                    cullen_survives[idx] = false;
                }
                if woodall_survives[idx] && f == 1 {
                    woodall_survives[idx] = false;
                }
            }

            // Advance: g_new = 2*g mod p, f_new = 2*f + g_new mod p
            g = 2 * g % p;
            f = (2 * f + g) % p;
        }
    }

    (cullen_survives, woodall_survives)
}

/// Test primality of a Cullen number n*2^n + 1 using Proth's theorem.
///
/// Proth's theorem applies because k=n < 2^n for all n ≥ 1.
/// Returns (IsPrime, certainty_string).
fn test_cullen(candidate: &Integer, n: u64, mr_rounds: u32) -> (IsPrime, &'static str) {
    // Proth test: deterministic for k < 2^n, always true for Cullen (n ≥ 1)
    if n >= 1 {
        match kbn::proth_test(candidate) {
            Some(true) => return (IsPrime::Yes, "deterministic (Proth)"),
            Some(false) => return (IsPrime::No, ""),
            None => {} // fall through to Miller-Rabin
        }
    }

    // Fallback: Miller-Rabin with 2-round pre-screen
    let r = mr_screened_test(candidate, mr_rounds);
    let cert = match r {
        IsPrime::Yes => "deterministic",
        IsPrime::Probably => "probabilistic",
        IsPrime::No => "",
    };
    (r, cert)
}

/// Test primality of a Woodall number n*2^n - 1 using LLR.
///
/// Decomposes n = m * 2^e (m odd) so that n*2^n - 1 = m * 2^(n+e) - 1,
/// which is the form k*2^exp - 1 suitable for LLR (k=m odd, exp=n+e).
/// Returns (IsPrime, certainty_string).
fn test_woodall(candidate: &Integer, n: u64, mr_rounds: u32) -> (IsPrime, &'static str) {
    if n >= 1 {
        // Extract odd part: n = m * 2^e
        let e = n.trailing_zeros();
        let m = n >> e;
        let exp = n + e as u64;

        // LLR requires k < 2^exp and k odd and exp >= 3
        // m is odd by construction; m < 2^exp since m ≤ n < 2^n ≤ 2^(n+e) = 2^exp
        if exp >= 3 {
            match kbn::llr_test(candidate, m, exp) {
                Some(true) => return (IsPrime::Yes, "deterministic (LLR)"),
                Some(false) => return (IsPrime::No, ""),
                None => {} // fall through to Miller-Rabin
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
    // Resolve sieve_limit: auto-tune if 0
    // Cullen/Woodall: n*2^n has ~max_n + log2(max_n) bits
    let candidate_bits = max_n + (max_n as f64).log2() as u64;
    let n_range = max_n.saturating_sub(min_n) + 1;
    let sieve_limit = sieve::resolve_sieve_limit(sieve_limit, candidate_bits, n_range);

    let sieve_primes = sieve::generate_primes(sieve_limit);
    eprintln!(
        "Sieve initialized with {} primes up to {}",
        sieve_primes.len(),
        sieve_limit
    );

    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::CullenWoodall { last_n, .. }) if last_n >= min_n && last_n < max_n => {
            eprintln!("Resuming Cullen/Woodall search from n={}", last_n + 1);
            last_n + 1
        }
        _ => min_n,
    };

    // Minimum n where n*2^n > sieve_limit, making the sieve safe
    let sieve_min_n: u64 = {
        let mut n = 1u64;
        while (n as u128) * (1u128 << n.min(63)) <= sieve_limit as u128 {
            n += 1;
            if n > 63 {
                break;
            }
        }
        n
    };
    eprintln!("Sieve active for n >= {}", sieve_min_n);

    // Run sieve over the entire range
    eprintln!(
        "Running Cullen/Woodall sieve over n=[{}..{}] ({} candidates)...",
        resume_from,
        max_n,
        max_n - resume_from + 1
    );
    let (cullen_survives, woodall_survives) =
        sieve_cullen_woodall(resume_from, max_n, &sieve_primes, sieve_min_n);
    let cullen_survivors: u64 = cullen_survives.iter().filter(|&&b| b).count() as u64;
    let woodall_survivors: u64 = woodall_survives.iter().filter(|&&b| b).count() as u64;
    let total_range = max_n - resume_from + 1;
    eprintln!(
        "Sieve complete: Cullen survivors {}/{} ({:.1}%), Woodall survivors {}/{} ({:.1}%)",
        cullen_survivors,
        total_range,
        cullen_survivors as f64 / total_range as f64 * 100.0,
        woodall_survivors,
        total_range,
        woodall_survivors as f64 / total_range as f64 * 100.0,
    );

    let mut last_checkpoint = Instant::now();
    let mut block_start = resume_from;
    let mut total_sieved: u64 = 0;

    while block_start <= max_n {
        let bsize = crate::block_size_for_n_heavy(block_start);
        let block_end = (block_start + bsize - 1).min(max_n);
        let block_len = block_end - block_start + 1;

        *progress.current.lock().unwrap() = format!("n*2^n±1 n=[{}..{}]", block_start, block_end);

        // Collect survivors from sieve bitmap
        let survivors: Vec<(u64, bool, bool)> = (block_start..=block_end)
            .filter_map(|n| {
                let idx = (n - resume_from) as usize;
                let tc = cullen_survives[idx];
                let tw = woodall_survives[idx];
                if tc || tw {
                    Some((n, tc, tw))
                } else {
                    None
                }
            })
            .collect();

        total_sieved += block_len - survivors.len() as u64;

        let found_primes: Vec<_> = survivors
            .into_par_iter()
            .flat_map_iter(|(n, test_cullen_flag, test_woodall_flag)| {
                let n_2_n = Integer::from(n) * Integer::from(2u32).pow(crate::checked_u32(n));

                let cullen_result = if test_cullen_flag {
                    let cullen = Integer::from(&n_2_n + 1u32);
                    let expr = format!("{}*2^{}+1", n, n);

                    // Try PFGW acceleration (50-100x faster for large candidates)
                    match pfgw::try_test(&expr, &cullen, pfgw::PfgwMode::Prp) {
                        Some(pfgw::PfgwResult::Prime { method, is_deterministic }) => {
                            let cert = if is_deterministic {
                                format!("deterministic ({})", method)
                            } else {
                                "probabilistic".to_string()
                            };
                            let digits = exact_digits(&cullen);
                            Some((format!("{}*2^{} + 1", n, n), digits, cert, "cullen"))
                        }
                        Some(pfgw::PfgwResult::Composite) => None,
                        _ => {
                            // Unavailable or not configured — fall through to GMP
                            let (r, cert) = test_cullen(&cullen, n, mr_rounds);
                            if r != IsPrime::No {
                                let digits = exact_digits(&cullen);
                                Some((format!("{}*2^{} + 1", n, n), digits, cert.to_string(), "cullen"))
                            } else {
                                None
                            }
                        }
                    }
                } else {
                    None
                };

                let woodall_result = if test_woodall_flag {
                    let woodall = Integer::from(&n_2_n - 1u32);
                    if woodall > 0u32 {
                        let expr = format!("{}*2^{}-1", n, n);

                        // Try PFGW acceleration
                        match pfgw::try_test(&expr, &woodall, pfgw::PfgwMode::Prp) {
                            Some(pfgw::PfgwResult::Prime { method, is_deterministic }) => {
                                let cert = if is_deterministic {
                                    format!("deterministic ({})", method)
                                } else {
                                    "probabilistic".to_string()
                                };
                                let digits = exact_digits(&woodall);
                                Some((format!("{}*2^{} - 1", n, n), digits, cert, "woodall"))
                            }
                            Some(pfgw::PfgwResult::Composite) => None,
                            _ => {
                                // Unavailable or not configured — fall through to GMP
                                let (r, cert) = test_woodall(&woodall, n, mr_rounds);
                                if r != IsPrime::No {
                                    let digits = exact_digits(&woodall);
                                    Some((format!("{}*2^{} - 1", n, n), digits, cert.to_string(), "woodall"))
                                } else {
                                    None
                                }
                            }
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                cullen_result.into_iter().chain(woodall_result)
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
                &Checkpoint::CullenWoodall {
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
                &Checkpoint::CullenWoodall {
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
    eprintln!(
        "Cullen/Woodall sieve eliminated {} candidates.",
        total_sieved
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sieve;

    fn cullen(n: u64) -> Integer {
        Integer::from(n) * Integer::from(2u32).pow(crate::checked_u32(n)) + 1u32
    }

    fn woodall(n: u64) -> Integer {
        Integer::from(n) * Integer::from(2u32).pow(crate::checked_u32(n)) - 1u32
    }

    #[test]
    fn known_cullen_primes() {
        // Known Cullen primes: C_1 = 3, C_141 = 141*2^141+1
        for &n in &[1u64, 141] {
            let c = cullen(n);
            assert_ne!(
                c.is_probably_prime(25),
                IsPrime::No,
                "C_{} should be prime",
                n
            );
        }
    }

    #[test]
    fn known_cullen_composites() {
        // Most Cullen numbers are composite
        for &n in &[2u64, 3, 4, 5, 6, 7, 8, 9, 10, 100] {
            let c = cullen(n);
            assert_eq!(
                c.is_probably_prime(25),
                IsPrime::No,
                "C_{} should be composite",
                n
            );
        }
    }

    #[test]
    fn known_woodall_primes() {
        // Known Woodall primes: W_2=7, W_3=23, W_6=383, W_30, W_75, W_81, W_115, W_123
        for &n in &[2u64, 3, 6, 30, 75, 81, 115, 123] {
            let w = woodall(n);
            assert_ne!(
                w.is_probably_prime(25),
                IsPrime::No,
                "W_{} should be prime",
                n
            );
        }
    }

    #[test]
    fn known_woodall_composites() {
        for &n in &[4u64, 5, 7, 8, 9, 10, 50, 100] {
            let w = woodall(n);
            assert_eq!(
                w.is_probably_prime(25),
                IsPrime::No,
                "W_{} should be composite",
                n
            );
        }
    }

    #[test]
    fn proth_proves_cullen() {
        // C_1 = 3, C_141 both should get deterministic Proth proofs
        let c1 = cullen(1);
        let (r, cert) = test_cullen(&c1, 1, 25);
        assert_eq!(r, IsPrime::Yes);
        assert!(
            cert.contains("deterministic"),
            "C_1 should be deterministic"
        );

        let c141 = cullen(141);
        let (r, cert) = test_cullen(&c141, 141, 25);
        assert_eq!(r, IsPrime::Yes);
        assert!(
            cert.contains("Proth"),
            "C_141 should get Proth proof, got: {}",
            cert
        );
    }

    #[test]
    fn llr_proves_woodall() {
        // W_3 = 23, W_75, W_81 should get deterministic LLR proofs
        for &n in &[3u64, 75, 81, 115, 123] {
            let w = woodall(n);
            let (r, cert) = test_woodall(&w, n, 25);
            assert_eq!(r, IsPrime::Yes, "W_{} should be prime", n);
            assert!(
                cert.contains("deterministic"),
                "W_{} should be deterministic, got: {}",
                n,
                cert
            );
        }
    }

    #[test]
    fn sieve_correctly_eliminates() {
        let sieve_primes = sieve::generate_primes(10_000);
        let sieve_min_n = 14u64; // 14*2^14 = 229376 > 10000

        let (cullen_surv, woodall_surv) = sieve_cullen_woodall(1, 200, &sieve_primes, sieve_min_n);

        // Verify: if sieve says composite, it really is composite
        for n in sieve_min_n..=200 {
            let idx = (n - 1) as usize;
            if !cullen_surv[idx] {
                let c = cullen(n);
                assert_eq!(
                    c.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said C_{} composite but it's prime",
                    n
                );
            }
            if !woodall_surv[idx] {
                let w = woodall(n);
                assert_eq!(
                    w.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said W_{} composite but it's prime",
                    n
                );
            }
        }
    }

    #[test]
    fn woodall_decomposition_correct() {
        // Verify the odd-part decomposition for LLR
        // n=6: 6 = 3 * 2^1, so W_6 = 3 * 2^7 - 1 = 383
        let n = 6u64;
        let e = n.trailing_zeros();
        let m = n >> e;
        assert_eq!(m, 3);
        assert_eq!(e, 1);
        let exp = n + e as u64;
        assert_eq!(exp, 7);
        // Verify: m * 2^exp - 1 = 3 * 128 - 1 = 383
        let reconstructed = Integer::from(m) * Integer::from(2u32).pow(crate::checked_u32(exp)) - 1u32;
        assert_eq!(reconstructed, woodall(n));
    }

    // ---- Edge case tests ----

    #[test]
    fn woodall_n1_equals_1() {
        // W_1 = 1*2^1 - 1 = 1, not prime
        let w = woodall(1);
        assert_eq!(w, 1, "W_1 should be 1");
    }

    #[test]
    fn cullen_n0_edge_case() {
        // C_0 = 0*2^0 + 1 = 1, not prime
        let c = Integer::from(0u32) * Integer::from(1u32) + 1u32;
        assert_eq!(c, 1, "C_0 should be 1");
    }

    #[test]
    fn woodall_decomposition_various_n() {
        // Verify decomposition n = m * 2^e for various n values
        for &n in &[12u64, 30, 81] {
            let e = n.trailing_zeros();
            let m = n >> e;
            let exp = n + e as u64;

            // m must be odd
            assert!(m % 2 == 1, "m={} should be odd for n={}", m, n);
            // Reconstruction: m * 2^e == n
            assert_eq!(m << e, n, "m * 2^e should reconstruct n={}", n);
            // W_n = m * 2^exp - 1
            let reconstructed = Integer::from(m) * Integer::from(2u32).pow(crate::checked_u32(exp)) - 1u32;
            assert_eq!(reconstructed, woodall(n), "Decomposition failed for W_{}", n);
        }
    }

    #[test]
    fn sieve_recurrence_matches_direct_computation() {
        // Verify the sieve recurrence f(n) = n*2^n mod p matches direct computation
        let p = 97u64;
        let mut g = sieve::pow_mod(2, 1, p); // 2^1 mod p
        let mut f = (1 % p) * g % p;          // 1*2^1 mod p

        for n in 1..=20u64 {
            // Direct computation
            let direct = (n % p) * sieve::pow_mod(2, n, p) % p;
            assert_eq!(f, direct, "n*2^n mod {} mismatch at n={}", p, n);

            // Advance recurrence
            g = 2 * g % p;
            f = (2 * f + g) % p;
        }
    }

    #[test]
    fn cullen_proth_always_applies() {
        // For Cullen numbers C_n = n*2^n + 1, Proth requires k=n < 2^n
        // This holds for all n >= 1
        for n in 1..=64u64 {
            assert!(
                n < (1u128 << n) as u64 || n > 63,
                "Proth condition n < 2^n should hold for n={}", n
            );
        }
    }

    #[test]
    fn test_cullen_rejects_composites() {
        // C_2 = 2*4+1 = 9 = 3^2, C_3 = 3*8+1 = 25 = 5^2, C_4 = 4*16+1 = 65 = 5*13
        for &n in &[2u64, 3, 4] {
            let c = cullen(n);
            let (r, _) = test_cullen(&c, n, 25);
            assert_eq!(r, IsPrime::No, "C_{} = {} should be composite", n, c);
        }
    }

    #[test]
    fn test_woodall_rejects_composites() {
        // W_4 = 4*16-1 = 63 = 9*7, W_5 = 5*32-1 = 159 = 3*53, W_7 = 7*128-1 = 895 = 5*179
        for &n in &[4u64, 5, 7] {
            let w = woodall(n);
            let (r, _) = test_woodall(&w, n, 25);
            assert_eq!(r, IsPrime::No, "W_{} = {} should be composite", n, w);
        }
    }

    #[test]
    fn sieve_cullen_woodall_soundness() {
        // Verify that sieved-out candidates are actually composite
        let sieve_primes = sieve::generate_primes(1_000);
        let sieve_min_n = 10u64; // 10*2^10 = 10240 > 1000

        let (cullen_surv, woodall_surv) = sieve_cullen_woodall(10, 100, &sieve_primes, sieve_min_n);

        for n in sieve_min_n..=100 {
            let idx = (n - 10) as usize;
            if !cullen_surv[idx] {
                let c = cullen(n);
                assert_eq!(
                    c.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said C_{} composite but it's prime", n
                );
            }
            if !woodall_surv[idx] {
                let w = woodall(n);
                assert_eq!(
                    w.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said W_{} composite but it's prime", n
                );
            }
        }
    }
}
