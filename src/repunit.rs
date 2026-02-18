//! # Repunit — Repunit Prime Search
//!
//! Searches for repunit primes: primes of the form R(b, n) = (b^n − 1)/(b − 1),
//! which are numbers consisting of n repetitions of the digit 1 in base b.
//! In base 10, these are 1, 11, 111, 1111, .... Only prime exponents n can yield
//! repunit primes (since R(b, ab) = R(b, a) · (b^a)^(b−1) + ... has algebraic factors).
//!
//! ## Sieve Strategy
//!
//! The repunit sieve is distinctive: each sieve prime q eliminates at most *one*
//! exponent n. Specifically:
//!
//! - If q ∤ (b−1): R(b, n) ≡ 0 (mod q) iff ord_q(b) | n. Since n is prime,
//!   this means n = ord_q(b) (if that order is itself prime).
//!
//! - If q | (b−1): R(b, n) ≡ n (mod q) by the geometric series formula, so
//!   q | R(b, n) iff n ≡ 0 (mod q), i.e., n = q.
//!
//! This "one elimination per prime" property makes the sieve less effective
//! than for other forms, requiring deeper sieving or more PRP tests.
//!
//! ## Complexity
//!
//! - Sieve construction: O(π(L)²) due to multiplicative order computation.
//! - Sieve per candidate: O(1) lookup (hash map from exponent to index).
//! - PRP test: O(n · M(n)) per survivor.
//!
//! ## References
//!
//! - OEIS: [A004023](https://oeis.org/A004023) — Repunit prime indices in base 10.
//! - OEIS: [A085104](https://oeis.org/A085104) — Generalized repunit primes.
//! - Harvey Dubner, "Generalized Repunit Primes", Mathematics of Computation,
//!   61(204), 1993.

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
use crate::pfgw;
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

        if b_minus_1.is_multiple_of(q) {
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

    // Resolve sieve_limit: auto-tune if 0
    // R(b,n) ≈ b^(n-1) has ~max_n * log2(base) bits
    let candidate_bits = (max_n as f64 * (base as f64).log2()) as u64;
    let n_range = max_n.saturating_sub(min_n) + 1;
    let sieve_limit = sieve::resolve_sieve_limit(sieve_limit, candidate_bits, n_range);

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
    let b_minus_1 = base - 1;

    for chunk in survivors.chunks(block_size) {
        let block_min = chunk[0];
        let block_max = chunk[chunk.len() - 1];

        *progress.current.lock().unwrap() = format!("R({}, [{}..{}])", base, block_min, block_max);

        let found: Vec<_> = chunk
            .par_iter()
            .filter_map(|&n| {
                let val = (Integer::from(base).pow(crate::checked_u32(n)) - 1u32) / b_minus_1;
                let pfgw_expr = format!("({}^{}-1)/{}", base, n, base - 1);

                // Try PFGW acceleration (50-100x faster for large candidates)
                if let Some(pfgw_result) =
                    pfgw::try_test(&pfgw_expr, &val, pfgw::PfgwMode::Prp)
                {
                    match pfgw_result {
                        pfgw::PfgwResult::Prime { method, is_deterministic } => {
                            let certainty = if is_deterministic {
                                format!("deterministic ({})", method)
                            } else {
                                "probabilistic".to_string()
                            };
                            let digits = exact_digits(&val);
                            return Some((n, digits, certainty));
                        }
                        pfgw::PfgwResult::Composite => return None,
                        pfgw::PfgwResult::Unavailable { .. } => {} // fall through
                    }
                }

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
        (Integer::from(base).pow(crate::checked_u32(n)) - 1u32) / (base - 1) as u32
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

    // ---- Additional repunit tests ----

    #[test]
    fn repunit_base5_known_primes() {
        // R(5,3) = (125-1)/4 = 31 (prime)
        // R(5,7) = (5^7-1)/4 = 19531 (prime)
        let r3 = repunit(5, 3);
        assert_eq!(r3, 31);
        assert_ne!(r3.is_probably_prime(25), IsPrime::No, "R(5,3) = 31 should be prime");

        let r7 = repunit(5, 7);
        assert_eq!(r7, 19531);
        assert_ne!(r7.is_probably_prime(25), IsPrime::No, "R(5,7) = 19531 should be prime");
    }

    #[test]
    fn repunit_base2_composites() {
        // R(2,11) = 2^11-1 = 2047 = 23*89 (composite)
        let r11 = repunit(2, 11);
        assert_eq!(r11, 2047);
        assert_eq!(r11.is_probably_prime(25), IsPrime::No, "R(2,11) = 2047 = 23*89");

        // R(2,23) = 2^23-1 = 8388607 = 47*178481 (composite)
        let r23 = repunit(2, 23);
        assert_eq!(r23, 8388607);
        assert_eq!(r23.is_probably_prime(25), IsPrime::No, "R(2,23) composite");
    }

    #[test]
    fn sieve_repunit_divisibility_by_order() {
        // ord_41(10) = 5 (since 10^5 ≡ 1 mod 41), so 41 | R(10,5) = 11111
        let r5 = repunit(10, 5);
        assert_eq!(r5, 11111);
        assert!(
            r5.is_divisible_u(41),
            "41 should divide R(10,5) = 11111 since ord_41(10) = 5"
        );
        // Verify the order
        assert_eq!(sieve::multiplicative_order(10, 41), 5);
    }

    #[test]
    fn sieve_repunit_b_minus_1_case() {
        // 3 | (10-1) = 9, so R(10,n) ≡ n (mod 3)
        // Therefore 3 | R(10,3) = 111 (since 3 ≡ 0 mod 3)
        let r3 = repunit(10, 3);
        assert_eq!(r3, 111);
        assert!(
            r3.is_divisible_u(3),
            "3 should divide R(10,3) = 111 since 3 | (10-1)"
        );

        // R(10,2) = 11 ≡ 2 (mod 3), so 3 does NOT divide R(10,2)
        let r2 = repunit(10, 2);
        assert!(!r2.is_divisible_u(3), "3 should NOT divide R(10,2) = 11");
    }

    #[test]
    fn repunit_composite_exponents_factor() {
        // R(10,6) should be divisible by R(10,2) = 11 and R(10,3) = 111
        // Since 6 = 2*3, algebraic factoring gives R(b,ab) = R(b,a) * something
        let r6 = repunit(10, 6);
        let r2 = repunit(10, 2);
        let r3 = repunit(10, 3);
        assert!(
            r6.is_divisible(&r2),
            "R(10,6) should be divisible by R(10,2) = 11"
        );
        assert!(
            r6.is_divisible(&r3),
            "R(10,6) should be divisible by R(10,3) = 111"
        );
    }

    #[test]
    fn sieve_repunit_preserves_known_primes() {
        // Known repunit prime indices in base 10: 2, 19, 23
        // These should survive the sieve
        let sieve_primes = sieve::generate_primes(10_000);
        let exponents: Vec<u64> = vec![2, 3, 5, 7, 11, 13, 17, 19, 23];
        let sieve_min_n = 5u64;

        let survives = sieve_repunit(&exponents, 10, &sieve_primes, sieve_min_n);

        for &n in &[2u64, 19, 23] {
            let idx = exponents.iter().position(|&e| e == n).unwrap();
            assert!(
                survives[idx],
                "Known repunit prime R(10,{}) should survive sieve", n
            );
        }
    }
}
