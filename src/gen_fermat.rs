//! # Generalized Fermat — b^(2^n) + 1 Prime Search
//!
//! Searches for generalized Fermat primes: numbers of the form b^(2^n) + 1
//! where b is an even base. The classical Fermat numbers F_n = 2^(2^n) + 1
//! are the special case b = 2.
//!
//! ## Algebraic Background
//!
//! A number b^m + 1 can only be prime if m is a power of 2 (otherwise
//! x + 1 | x^m + 1 has non-trivial algebraic factors). The base b must
//! be even (if b is odd, b^(2^n) + 1 is even and > 2).
//!
//! ## Sieve Strategy
//!
//! For sieve prime q, b^(2^n) + 1 ≡ 0 (mod q) iff b^(2^n) ≡ −1 (mod q).
//! By Fermat's little theorem, this requires that the multiplicative order
//! of b mod q is exactly 2^(n+1). The sieve computes ord_q(b) and checks
//! if it equals 2^(n+1) for any n in the search range.
//!
//! ## Primality Testing
//!
//! ### Pépin/Proth Deterministic Test
//!
//! b^(2^n) + 1 has N−1 = b^(2^n) = (b/2^t)·2^(t·2^n) where t = v₂(b).
//! When 2^t > m (the odd part of b), the 2-power in N−1 exceeds √N,
//! and the Proth/Pépin test provides a deterministic proof. This applies
//! to all power-of-2 bases and many others (e.g., b = 6: t = 1, m = 3,
//! so 2 > 3 fails — Pépin is not provable for b = 6).
//!
//! ### Fallback
//!
//! When Pépin is not provable, the Proth test result is treated as a
//! strong PRP and confirmed with Miller–Rabin.
//!
//! ## Complexity
//!
//! - Sieve: O(π(L)) multiplicative order computations.
//! - Pépin test: O(2^n · M(2^n)) — 2^n squarings of 2^n-bit numbers.
//!
//! ## References
//!
//! - OEIS: [A019434](https://oeis.org/A019434) — Fermat primes (b = 2).
//! - OEIS: [A056993](https://oeis.org/A056993) — Generalized Fermat primes.
//! - Pierre de Fermat, 1640 (conjecture that all F_n are prime).
//! - Pépin's test (1877): F_n is prime iff 3^((F_n−1)/2) ≡ −1 (mod F_n).
//! - PrimeGrid Generalized Fermat Prime Search: <https://www.primegrid.com/>

use anyhow::Result;
use rayon::prelude::*;
use rug::integer::IsPrime;
use rug::ops::Pow;
use rug::Integer;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use tracing::info;

use crate::checkpoint::{self, Checkpoint};
use crate::db::Database;
use crate::events::{self, EventBus};
use crate::kbn;
use crate::pfgw;
use crate::progress::Progress;
use crate::CoordinationClient;
use crate::{exact_digits, sieve};

/// Check if the Proth/Pépin deterministic proof applies for b^(2^n) + 1.
///
/// The condition is: let b = 2^t * m (m odd, t >= 1). The proof is valid
/// when 2^t > m, which ensures the 2-part of N-1 exceeds sqrt(N).
fn is_pepin_provable(b: u64) -> bool {
    if b == 0 || !b.is_multiple_of(2) {
        return false;
    }
    let t = b.trailing_zeros(); // 2-adic valuation
    let m = b >> t; // odd part
    (1u64 << t) > m
}

/// Test a generalized Fermat candidate for primality.
///
/// Uses Proth/Pépin test first (deterministic for qualifying bases),
/// falls back to Miller-Rabin otherwise.
fn test_gf(candidate: &Integer, b: u64, _fermat_n: u32, mr_rounds: u32) -> (IsPrime, &'static str) {
    // Try Proth/Pépin test: a^((N-1)/2) ≡ -1 (mod N)
    if let Some((result, _witness)) = kbn::proth_test(candidate) {
        if result {
            let certainty = if is_pepin_provable(b) {
                "deterministic"
            } else {
                // Pépin test passed but 2-part condition not met — treat as strong PRP
                // Still need MR confirmation
                let mr_result = candidate.is_probably_prime(mr_rounds);
                return if mr_result != IsPrime::No {
                    (mr_result, "probabilistic")
                } else {
                    (IsPrime::No, "probabilistic")
                };
            };
            return (IsPrime::Yes, certainty);
        } else {
            return (IsPrime::No, "deterministic");
        }
    }

    // No quadratic non-residue found (extremely rare), fall back to MR
    let result = candidate.is_probably_prime(mr_rounds);
    if result != IsPrime::No {
        (result, "probabilistic")
    } else {
        (IsPrime::No, "probabilistic")
    }
}

/// Sieve generalized Fermat candidates b^(2^n) + 1 over even bases.
///
/// For each sieve prime q, b^(2^n) + 1 ≡ 0 (mod q) iff b^(2^n) ≡ -1 (mod q).
/// Uses Fermat's little theorem to reduce: exp = 2^n mod (q-1).
fn sieve_gf(
    min_b: u64,
    max_b: u64,
    fermat_n: u32,
    sieve_primes: &[u64],
    sieve_min_b: u64,
) -> Vec<bool> {
    // survives[i] corresponds to base = min_b + 2*i (even bases only)
    let range = ((max_b - min_b) / 2 + 1) as usize;
    let mut survives = vec![true; range];

    for &q in sieve_primes {
        if q == 2 {
            continue; // b even → b^(2^n) even → b^(2^n)+1 odd, never divisible by 2
        }
        // exp = 2^fermat_n mod (q-1)
        let exp = sieve::pow_mod(2, fermat_n as u64, q - 1);
        for (i, survives_i) in survives.iter_mut().enumerate().take(range) {
            let b = min_b + 2 * i as u64;
            if b < sieve_min_b {
                continue;
            }
            if b.is_multiple_of(q) {
                // b^(2^n) ≡ 0 (mod q), so b^(2^n)+1 ≡ 1 ≢ 0 (mod q)
                continue;
            }
            let r = sieve::pow_mod(b % q, exp, q);
            if r == q - 1 {
                *survives_i = false;
            }
        }
    }

    survives
}

/// Search for generalized Fermat primes: b^(2^n) + 1 for even b in [min_base, max_base].
pub fn search(
    fermat_n: u32,
    min_base: u64,
    max_base: u64,
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
    // Ensure bases are even
    let min_b = if min_base.is_multiple_of(2) {
        min_base.max(2)
    } else {
        min_base + 1
    };
    let max_b = if max_base.is_multiple_of(2) {
        max_base
    } else {
        max_base - 1
    };

    if min_b > max_b {
        info!("no even bases in range, search complete");
        return Ok(());
    }

    // Resolve sieve_limit: auto-tune if 0
    // b^(2^n) + 1 has ~2^n * log2(max_base) bits
    let candidate_bits = ((1u64 << fermat_n) as f64 * (max_b as f64).log2()) as u64;
    let n_range = (max_b - min_b) / 2 + 1;
    let sieve_limit = sieve::resolve_sieve_limit(sieve_limit, candidate_bits, n_range);

    let sieve_primes = sieve::generate_primes(sieve_limit);
    let total_bases = (max_b - min_b) / 2 + 1;
    info!(
        fermat_n,
        min_b,
        max_b,
        total_bases,
        "generalized Fermat search started"
    );
    info!(
        prime_count = sieve_primes.len(),
        sieve_limit,
        "sieve initialized"
    );

    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::GenFermat { last_base, .. })
            if last_base >= min_b && last_base < max_b =>
        {
            let next = last_base + 2;
            info!(resume_b = next, "resuming generalized Fermat search");
            next
        }
        _ => min_b,
    };

    // Minimum b where b^(2^n) + 1 > sieve_limit
    let sieve_min_b =
        ((sieve_limit as f64).powf(1.0 / (1u64 << fermat_n) as f64)).ceil() as u64 + 1;
    info!(sieve_min_b, "sieve active");

    // Sieve
    info!("running sieve");
    let survives = sieve_gf(resume_from, max_b, fermat_n, &sieve_primes, sieve_min_b);
    let survivors: Vec<u64> = (0..survives.len())
        .filter(|&i| survives[i])
        .map(|i| resume_from + 2 * i as u64)
        .collect();

    let total_range = ((max_b - resume_from) / 2 + 1) as usize;
    let eliminated = total_range - survivors.len();
    info!(
        eliminated,
        total = total_range,
        survivors = survivors.len(),
        survivor_pct = survivors.len() as f64 / total_range.max(1) as f64 * 100.0,
        "sieve complete"
    );

    // Process in blocks for checkpointing
    let block_size = 100;
    let mut last_checkpoint = Instant::now();

    for chunk in survivors.chunks(block_size) {
        let block_min = chunk[0];
        let block_max = chunk[chunk.len() - 1];

        *progress.current.lock().unwrap() =
            format!("[{}..{}]^(2^{}) + 1", block_min, block_max, fermat_n);

        let found: Vec<_> = chunk
            .par_iter()
            .filter_map(|&b| {
                // Compute b^(2^n) + 1
                let exponent = crate::checked_u32(1u64 << fermat_n);
                let b_pow = Integer::from(b).pow(exponent);
                let candidate = Integer::from(&b_pow + 1u32);
                let expr = format!("{}^{}+1", b, exponent);

                // Try PFGW acceleration (50-100x faster for large candidates)
                if let Some(pfgw_result) = pfgw::try_test(&expr, &candidate, pfgw::PfgwMode::Prp) {
                    match pfgw_result {
                        pfgw::PfgwResult::Prime {
                            method,
                            is_deterministic,
                        } => {
                            let certainty = if is_deterministic {
                                format!("deterministic ({})", method)
                            } else {
                                "probabilistic".to_string()
                            };
                            let digits = exact_digits(&candidate);
                            return Some((b, digits, certainty));
                        }
                        pfgw::PfgwResult::Composite => return None,
                        pfgw::PfgwResult::Unavailable { .. } => {} // fall through
                    }
                }

                // Adaptive P-1 pre-filter (Stage 1 + Stage 2, auto-tuned B1/B2)
                if crate::p1::adaptive_p1_filter(&candidate) {
                    return None;
                }

                let (result, certainty) = test_gf(&candidate, b, fermat_n, mr_rounds);
                if result == IsPrime::No {
                    return None;
                }

                let digits = exact_digits(&candidate);
                Some((b, digits, certainty.to_string()))
            })
            .collect();

        progress
            .tested
            .fetch_add(chunk.len() as u64, Ordering::Relaxed);

        for (b, digits, certainty) in found {
            let expr = format!("{}^(2^{}) + 1", b, fermat_n);
            progress.found.fetch_add(1, Ordering::Relaxed);
            if let Some(eb) = event_bus {
                eb.emit(events::Event::PrimeFound {
                    form: "gen_fermat".into(),
                    expression: expr.clone(),
                    digits,
                    proof_method: certainty.clone(),
                    timestamp: Instant::now(),
                });
            } else {
                info!(
                    expression = %expr,
                    digits,
                    certainty = %certainty,
                    "generalized Fermat prime found"
                );
            }
            db.insert_prime_sync(
                rt,
                "gen_fermat",
                &expr,
                digits,
                search_params,
                &certainty,
                None,
            )?;
            if let Some(wc) = worker_client {
                wc.report_prime("gen_fermat", &expr, digits, search_params, &certainty);
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::GenFermat {
                    last_base: block_max,
                    fermat_n: Some(fermat_n),
                    min_base: Some(min_b),
                    max_base: Some(max_b),
                },
            )?;
            info!(b = block_max, "checkpoint saved");
            last_checkpoint = Instant::now();
        }

        if worker_client.is_some_and(|wc| wc.is_stop_requested()) {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::GenFermat {
                    last_base: block_max,
                    fermat_n: Some(fermat_n),
                    min_base: Some(min_b),
                    max_base: Some(max_b),
                },
            )?;
            info!(b = block_max, "stop requested by coordinator, checkpoint saved");
            return Ok(());
        }
    }

    checkpoint::clear(checkpoint_path);
    info!("generalized Fermat search complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gf(b: u64, n: u32) -> Integer {
        Integer::from(b).pow(1u32 << n) + 1u32
    }

    #[test]
    fn known_fermat_primes_base2() {
        // Classic Fermat primes: F_0=3, F_1=5, F_2=17, F_3=257, F_4=65537
        for &n in &[0u32, 1, 2, 3, 4] {
            let f = gf(2, n);
            assert_ne!(
                f.is_probably_prime(25),
                IsPrime::No,
                "F_{} = {} should be prime",
                n,
                f
            );
        }
    }

    #[test]
    fn fermat_f5_composite() {
        // F_5 = 2^32 + 1 = 4294967297 = 641 × 6700417
        let f5 = gf(2, 5);
        assert_eq!(
            f5.is_probably_prime(25),
            IsPrime::No,
            "F_5 should be composite"
        );
    }

    #[test]
    fn known_gf_primes_small() {
        // GF(b, 1) = b^2 + 1:
        // b=2: 5 (prime), b=4: 17 (prime), b=6: 37 (prime),
        // b=10: 101 (prime), b=14: 197 (prime)
        for &b in &[2u64, 4, 6, 10, 14] {
            let candidate = gf(b, 1);
            assert_ne!(
                candidate.is_probably_prime(25),
                IsPrime::No,
                "{}^2+1 = {} should be prime",
                b,
                candidate
            );
        }
    }

    #[test]
    fn known_gf_composites_small() {
        // GF(b, 1) = b^2 + 1:
        // b=8: 65 = 5*13, b=12: 145 = 5*29, b=16: 257 (prime!), b=18: 325 = 5*5*13
        let c8 = gf(8, 1);
        assert_eq!(c8, 65);
        assert_eq!(
            c8.is_probably_prime(25),
            IsPrime::No,
            "8^2+1 = 65 should be composite"
        );

        let c12 = gf(12, 1);
        assert_eq!(c12, 145);
        assert_eq!(
            c12.is_probably_prime(25),
            IsPrime::No,
            "12^2+1 = 145 should be composite"
        );
    }

    #[test]
    fn pepin_provable_bases() {
        // b=2 (t=1, m=1): 2 > 1 ✓
        assert!(is_pepin_provable(2));
        // b=4 (t=2, m=1): 4 > 1 ✓
        assert!(is_pepin_provable(4));
        // b=8 (t=3, m=1): 8 > 1 ✓
        assert!(is_pepin_provable(8));
        // b=12 (t=2, m=3): 4 > 3 ✓
        assert!(is_pepin_provable(12));
        // b=6 (t=1, m=3): 2 > 3 ✗
        assert!(!is_pepin_provable(6));
        // b=10 (t=1, m=5): 2 > 5 ✗
        assert!(!is_pepin_provable(10));
        // b=14 (t=1, m=7): 2 > 7 ✗
        assert!(!is_pepin_provable(14));
        // b=16 (t=4, m=1): 16 > 1 ✓
        assert!(is_pepin_provable(16));
    }

    #[test]
    fn test_gf_deterministic_base2() {
        // F_2 = 2^4 + 1 = 17, Pépin-provable since b=2
        let f2 = gf(2, 2);
        let (result, cert) = test_gf(&f2, 2, 2, 25);
        assert_eq!(result, IsPrime::Yes, "F_2 = 17 should be prime");
        assert_eq!(cert, "deterministic");
    }

    #[test]
    fn test_gf_deterministic_base4() {
        // 4^(2^1) + 1 = 17, Pépin-provable since b=4
        let candidate = gf(4, 1);
        assert_eq!(candidate, 17);
        let (result, cert) = test_gf(&candidate, 4, 1, 25);
        assert_eq!(result, IsPrime::Yes);
        assert_eq!(cert, "deterministic");
    }

    #[test]
    fn test_gf_composite() {
        // F_5 = 2^32 + 1 = 4294967297 (composite)
        let f5 = gf(2, 5);
        let (result, _) = test_gf(&f5, 2, 5, 25);
        assert_eq!(result, IsPrime::No);
    }

    #[test]
    fn sieve_eliminates_composites() {
        let sieve_primes = sieve::generate_primes(10_000);
        let fermat_n = 1u32; // b^2 + 1

        // sieve_min_b for sieve_limit=10000, n=1: b > sqrt(10000) = 100
        let sieve_min_b = 102u64;

        let survives = sieve_gf(102, 200, fermat_n, &sieve_primes, sieve_min_b);

        // Verify: if sieved out, must actually be composite
        for i in 0..survives.len() {
            let b = 102 + 2 * i as u64;
            if !survives[i] {
                let candidate = gf(b, fermat_n);
                assert_eq!(
                    candidate.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said {}^2+1 composite but it's prime",
                    b
                );
            }
        }
    }

    #[test]
    fn gf_values_correct() {
        assert_eq!(gf(2, 0), 3); // 2^1 + 1
        assert_eq!(gf(2, 1), 5); // 2^2 + 1
        assert_eq!(gf(2, 2), 17); // 2^4 + 1
        assert_eq!(gf(2, 3), 257); // 2^8 + 1
        assert_eq!(gf(2, 4), 65537); // 2^16 + 1
        assert_eq!(gf(6, 1), 37); // 6^2 + 1
        assert_eq!(gf(10, 1), 101); // 10^2 + 1
    }

    // ---- Additional gen_fermat tests ----

    #[test]
    fn is_pepin_provable_odd_base_returns_false() {
        // Odd bases: b^(2^n) + 1 is always even > 2, can't be prime
        // is_pepin_provable should return false for odd b and b=0
        assert!(!is_pepin_provable(0), "b=0 should not be Pépin-provable");
        assert!(
            !is_pepin_provable(3),
            "b=3 (odd) should not be Pépin-provable"
        );
        assert!(
            !is_pepin_provable(5),
            "b=5 (odd) should not be Pépin-provable"
        );
        assert!(
            !is_pepin_provable(7),
            "b=7 (odd) should not be Pépin-provable"
        );
    }

    #[test]
    fn is_pepin_provable_large_even_bases() {
        // b=24: 24 = 2^3 * 3, t=3, m=3, 8 > 3 ✓
        assert!(is_pepin_provable(24), "b=24 should be Pépin-provable");
        // b=18: 18 = 2^1 * 9, t=1, m=9, 2 > 9 ✗
        assert!(!is_pepin_provable(18), "b=18 should NOT be Pépin-provable");
        // b=48: 48 = 2^4 * 3, t=4, m=3, 16 > 3 ✓
        assert!(is_pepin_provable(48), "b=48 should be Pépin-provable");
        // b=20: 20 = 2^2 * 5, t=2, m=5, 4 > 5 ✗
        assert!(!is_pepin_provable(20), "b=20 should NOT be Pépin-provable");
    }

    #[test]
    fn sieve_gf_even_bases_only() {
        // sieve_gf with min_b=2, max_b=10 should produce 5 entries (2,4,6,8,10)
        let sieve_primes = sieve::generate_primes(100);
        let survives = sieve_gf(2, 10, 1, &sieve_primes, 2);
        assert_eq!(
            survives.len(),
            5,
            "Even bases 2,4,6,8,10 should give 5 sieve entries"
        );
    }

    #[test]
    fn gf_non_power_of_2_exponent_factors() {
        // 2^6 + 1 = 65 = 5*13 — demonstrates that b^m+1 is composite when
        // m is not a power of 2 (6 = 2*3, so x^3+1 = (x+1)(x^2-x+1) applies)
        let val = Integer::from(2u32).pow(6) + 1u32;
        assert_eq!(val, 65);
        assert!(val.is_divisible_u(5), "65 should be divisible by 5");
        assert!(val.is_divisible_u(13), "65 should be divisible by 13");
        assert_eq!(
            val.is_probably_prime(25),
            IsPrime::No,
            "2^6+1 = 65 should be composite"
        );
    }
}
