//! # Factorial — n! ± 1 Prime Search
//!
//! Searches for primes of the form n! + 1 and n! − 1 by incrementally computing
//! n! and testing each candidate. Uses a modular sieve to eliminate most
//! composites before the expensive GMP primality test.
//!
//! ## Algorithm
//!
//! 1. **Incremental factorial**: Maintains a running product n! = n · (n−1)!,
//!    avoiding recomputation. FLINT's `fmpz_fac_ui` (SIMD NTTs, 3–10× faster)
//!    is used for the initial computation when available.
//!
//! 2. **Modular sieve** (`FactorialSieve`): For each sieve prime p > n, maintains
//!    n! mod p incrementally. Checks n! mod p = p−1 (meaning p | n!+1) and
//!    n! mod p = 1 (meaning p | n!−1) in a single pass. Primes p ≤ n are
//!    removed since p | n! makes the residue zero.
//!
//! 3. **Wilson's theorem filter**: When n+1 is prime, Wilson's theorem gives
//!    n! ≡ −1 (mod n+1), so n!+1 is divisible by n+1 and therefore composite
//!    (for n > 2). This eliminates ~15% of +1 candidates for free.
//!
//! 4. **Parallel testing**: Surviving n!+1 and n!−1 are tested simultaneously
//!    via `rayon::join`.
//!
//! 5. **Deterministic proofs**: Pocklington N−1 proof for n!+1 and Morrison
//!    N+1 proof for n!−1, since n! has fully known factorization.
//!
//! ## Complexity
//!
//! - Factorial computation: O(n · M(n!)) where M(k) is multiplication cost.
//! - Sieve advance per step: O(π(L)) where L is sieve limit.
//! - Primality test: O(n! · log(n!)) via GMP's BPSW + Miller–Rabin.
//!
//! ## References
//!
//! - OEIS: [A002981](https://oeis.org/A002981) — n such that n! + 1 is prime.
//! - OEIS: [A002982](https://oeis.org/A002982) — n such that n! − 1 is prime.
//! - Wilson's theorem: (p−1)! ≡ −1 (mod p) for prime p.
//! - Caldwell & Gallot, "On the Primality of n! ± 1 and 2·3·5···p ± 1",
//!   Mathematics of Computation, 71(237), 2002.

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
use crate::pfgw;
use crate::progress::Progress;
use crate::proof;
use crate::CoordinationClient;
use crate::{estimate_digits, exact_digits, mr_screened_test, sieve};

/// Incremental modular sieve for factorial primes.
///
/// Maintains n! mod p for each sieve prime p > n. Since p > n means p does
/// not divide n!, we have n! mod p != 0. We can then cheaply check:
///   - n! + 1 is composite if n! mod p == p - 1 (meaning p | n!+1)
///   - n! - 1 is composite if n! mod p == 1     (meaning p | n!-1)
struct FactorialSieve {
    /// (prime, n! mod prime) for active primes where p > current n.
    entries: Vec<(u64, u64)>,
}

impl FactorialSieve {
    /// Initialize sieve. Computes initial_n! mod p for each prime p > initial_n.
    fn new(sieve_primes: &[u64], initial_n: u64) -> Self {
        let entries: Vec<(u64, u64)> = sieve_primes
            .par_iter()
            .filter(|&&p| p > initial_n)
            .map(|&p| {
                let mut fm = 1u64;
                for i in 2..=initial_n {
                    fm = fm * (i % p) % p;
                }
                (p, fm)
            })
            .collect();
        FactorialSieve { entries }
    }

    /// Advance from (n-1)! to n! by multiplying all residues by n.
    /// Removes primes where p divides n! (i.e., when n >= p).
    /// Uses parallel chunks for large entry counts.
    fn advance(&mut self, n: u64) {
        if self.entries.len() > 10_000 {
            // Parallel update: map residues in parallel, then retain non-zero
            self.entries.par_iter_mut().for_each(|(p, fm)| {
                *fm = *fm * (n % *p) % *p;
            });
            self.entries.retain(|(_, fm)| *fm != 0);
        } else {
            self.entries.retain_mut(|(p, fm)| {
                *fm = *fm * (n % *p) % *p;
                *fm != 0
            });
        }
    }

    /// Check if n!+1 or n!-1 is provably composite via the sieve.
    /// Returns (plus_composite, minus_composite) in a single pass.
    fn check_composites(&self) -> (bool, bool) {
        let mut plus_composite = false;
        let mut minus_composite = false;
        for &(p, fm) in &self.entries {
            if !plus_composite && fm == p - 1 {
                plus_composite = true;
            }
            if !minus_composite && fm == 1 {
                minus_composite = true;
            }
            if plus_composite && minus_composite {
                break;
            }
        }
        (plus_composite, minus_composite)
    }
}

pub fn search(
    start: u64,
    end: u64,
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
    // Stirling's approximation: log2(n!) ≈ n*log2(n/e) + 0.5*log2(2*pi*n)
    let candidate_bits = if end > 2 {
        (end as f64 * (end as f64 / std::f64::consts::E).log2()) as u64
    } else {
        10
    };
    let n_range = end.saturating_sub(start) + 1;
    let sieve_limit = sieve::resolve_sieve_limit(sieve_limit, candidate_bits, n_range);

    let sieve_primes = sieve::generate_primes(sieve_limit);
    eprintln!(
        "Sieve initialized with {} primes up to {}",
        sieve_primes.len(),
        sieve_limit
    );

    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::Factorial { last_n, .. }) if last_n >= start && last_n < end => {
            eprintln!("Resuming factorial search from n={}", last_n + 1);
            last_n + 1
        }
        _ => start,
    };

    // Compute factorial up to the starting point.
    // Use FLINT when available (3-10x faster via SIMD NTTs), otherwise GMP binary-splitting.
    let mut factorial = if resume_from > 2 {
        eprintln!("Precomputing {}!...", resume_from - 1);
        #[cfg(feature = "flint")]
        let f = {
            eprintln!("  (using FLINT fmpz_fac_ui)");
            crate::flint::factorial(resume_from - 1)
        };
        #[cfg(not(feature = "flint"))]
        let f = Integer::from(Integer::factorial((resume_from - 1) as u32));
        eprintln!("Precomputation complete.");
        f
    } else {
        Integer::from(1u32)
    };

    // Initialize modular sieve at (resume_from - 1)!
    eprintln!("Initializing modular sieve...");
    let mut fsieve = FactorialSieve::new(&sieve_primes, resume_from.saturating_sub(1));
    eprintln!(
        "Modular sieve ready ({} active primes).",
        fsieve.entries.len()
    );

    // Compute minimum n where n! > sieve_limit, making the sieve safe.
    let sieve_min_n: u64 = {
        let mut fact: u128 = 1;
        let mut i = 2u64;
        while fact <= sieve_limit as u128 {
            fact = fact.saturating_mul(i as u128);
            i += 1;
        }
        i - 1
    };
    eprintln!("Sieve active for n >= {}", sieve_min_n);

    let mut last_checkpoint = Instant::now();
    let mut sieved_out: u64 = 0;
    let mut wilson_eliminated: u64 = 0;

    for n in resume_from..=end {
        factorial *= n;
        fsieve.advance(n);

        let approx_digits = estimate_digits(&factorial);
        *progress.current.lock().unwrap() = format!("{}! (~{} digits)", n, approx_digits);
        progress.tested.fetch_add(2, Ordering::Relaxed);

        let sieve_safe = n >= sieve_min_n;
        let (plus_composite, minus_composite) = if sieve_safe {
            fsieve.check_composites()
        } else {
            (false, false)
        };
        let mut test_plus = !plus_composite;
        let test_minus = !minus_composite;

        // Wilson's theorem: if n+1 is prime and n > 2, then (n+1) | (n!+1), so skip +1 test.
        // By Wilson's theorem, n! ≡ -1 (mod n+1) when n+1 is prime, so n!+1 ≡ 0 (mod n+1).
        let wilson_eliminates_plus = n > 2 && sieve_primes.binary_search(&(n + 1)).is_ok();
        if wilson_eliminates_plus && test_plus {
            test_plus = false;
            wilson_eliminated += 1;
        }

        if !test_plus && !test_minus {
            sieved_out += 1;
            continue;
        }

        // Only construct the huge n!±1 Integers for candidates that survived the sieve.
        // Try PFGW first for large candidates (50-100x faster), fall back to GMP MR.
        let (r_plus, r_minus) = rayon::join(
            || {
                if !test_plus {
                    return (IsPrime::No, None);
                }
                let plus = factorial.clone() + 1u32;
                // Adaptive P-1 pre-filter (Stage 1 + Stage 2, auto-tuned B1/B2)
                if crate::p1::adaptive_p1_filter(&plus) {
                    return (IsPrime::No, None);
                }
                // Try PFGW acceleration for large candidates
                if let Some(pfgw_result) =
                    pfgw::try_test(&format!("{}!+1", n), &plus, pfgw::PfgwMode::NMinus1Proof)
                {
                    match pfgw_result {
                        pfgw::PfgwResult::Prime {
                            method,
                            is_deterministic,
                        } => {
                            let cert = if is_deterministic {
                                format!("deterministic ({})", method)
                            } else {
                                "probabilistic".to_string()
                            };
                            return (IsPrime::Probably, Some(cert));
                        }
                        pfgw::PfgwResult::Composite => return (IsPrime::No, None),
                        pfgw::PfgwResult::Unavailable { .. } => {} // fall through to GMP
                    }
                }
                (mr_screened_test(&plus, mr_rounds), None)
            },
            || {
                if !test_minus {
                    return (IsPrime::No, None);
                }
                let minus = factorial.clone() - 1u32;
                // Adaptive P-1 pre-filter (Stage 1 + Stage 2, auto-tuned B1/B2)
                if crate::p1::adaptive_p1_filter(&minus) {
                    return (IsPrime::No, None);
                }
                // Try PFGW acceleration for large candidates
                if let Some(pfgw_result) =
                    pfgw::try_test(&format!("{}!-1", n), &minus, pfgw::PfgwMode::NPlus1Proof)
                {
                    match pfgw_result {
                        pfgw::PfgwResult::Prime {
                            method,
                            is_deterministic,
                        } => {
                            let cert = if is_deterministic {
                                format!("deterministic ({})", method)
                            } else {
                                "probabilistic".to_string()
                            };
                            return (IsPrime::Probably, Some(cert));
                        }
                        pfgw::PfgwResult::Composite => return (IsPrime::No, None),
                        pfgw::PfgwResult::Unavailable { .. } => {} // fall through to GMP
                    }
                }
                (mr_screened_test(&minus, mr_rounds), None)
            },
        );

        for ((result, pfgw_cert), sign) in [(r_plus, "+"), (r_minus, "-")] {
            if result != IsPrime::No {
                let digit_count = exact_digits(&factorial);

                // Use PFGW-provided certainty if available, otherwise determine from MR result
                let certainty_owned: String;
                let certainty: &str = if let Some(ref cert) = pfgw_cert {
                    cert.as_str()
                } else {
                    let mut cert = match result {
                        IsPrime::Yes => "deterministic",
                        IsPrime::Probably => "probabilistic",
                        IsPrime::No => unreachable!(),
                    };

                    // Attempt deterministic proof for probable primes (GMP path)
                    if result == IsPrime::Probably {
                        let proven = if sign == "+" {
                            let candidate = Integer::from(&factorial + 1u32);
                            proof::pocklington_factorial_proof(n, &candidate, &sieve_primes)
                        } else {
                            let candidate = Integer::from(&factorial - 1u32);
                            proof::morrison_factorial_proof(n, &candidate, &sieve_primes)
                        };
                        if proven {
                            cert = if sign == "+" {
                                "deterministic (Pocklington N-1)"
                            } else {
                                "deterministic (Morrison N+1)"
                            };
                        }
                    }
                    certainty_owned = cert.to_string();
                    &certainty_owned
                };

                let expr = format!("{}! {} 1", n, sign);
                progress.found.fetch_add(1, Ordering::Relaxed);
                if let Some(eb) = event_bus {
                    eb.emit(events::Event::PrimeFound {
                        form: "factorial".into(),
                        expression: expr.clone(),
                        digits: digit_count,
                        proof_method: certainty.to_string(),
                        timestamp: Instant::now(),
                    });
                } else {
                    eprintln!(
                        "*** PRIME FOUND: {} ({} digits, {}) ***",
                        expr, digit_count, certainty
                    );
                }
                db.insert_prime_sync(
                    rt,
                    "factorial",
                    &expr,
                    digit_count,
                    search_params,
                    certainty,
                    None,
                )?;
                if let Some(wc) = worker_client {
                    wc.report_prime("factorial", &expr, digit_count, search_params, certainty);
                }
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Factorial {
                    last_n: n,
                    start: Some(start),
                    end: Some(end),
                },
            )?;
            eprintln!("Checkpoint saved at n={} (sieved out: {})", n, sieved_out);
            last_checkpoint = Instant::now();
        }

        if worker_client.is_some_and(|wc| wc.is_stop_requested()) {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Factorial {
                    last_n: n,
                    start: Some(start),
                    end: Some(end),
                },
            )?;
            eprintln!("Stop requested by coordinator, checkpoint saved at n={}", n);
            return Ok(());
        }
    }

    checkpoint::clear(checkpoint_path);
    eprintln!("Factorial sieve eliminated {} candidates.", sieved_out);
    if wilson_eliminated > 0 {
        eprintln!(
            "Wilson's theorem eliminated {} n!+1 candidates.",
            wilson_eliminated
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factorial_sieve_advance_produces_correct_residues() {
        // Test that advancing from 1! to n! gives correct n! mod p.
        // Use small primes where we can verify manually.
        let sieve_primes: Vec<u64> = vec![7, 11, 13];
        let mut fsieve = FactorialSieve::new(&sieve_primes, 1);

        // After new(primes, 1): residues are 1! mod p = 1 for all p > 1
        for &(p, fm) in &fsieve.entries {
            assert_eq!(fm, 1, "1! mod {} should be 1", p);
        }

        // Advance to 2! = 2
        fsieve.advance(2);
        for &(p, fm) in &fsieve.entries {
            assert_eq!(fm, 2 % p, "2! mod {} should be {}", p, 2 % p);
        }

        // Advance to 3! = 6
        fsieve.advance(3);
        for &(p, fm) in &fsieve.entries {
            assert_eq!(fm, 6 % p, "3! mod {} should be {}", p, 6 % p);
        }

        // Advance to 4! = 24
        fsieve.advance(4);
        for &(p, fm) in &fsieve.entries {
            assert_eq!(fm, 24 % p, "4! mod {} should be {}", p, 24 % p);
        }

        // Advance to 5! = 120
        fsieve.advance(5);
        for &(p, fm) in &fsieve.entries {
            assert_eq!(fm, 120 % p, "5! mod {} should be {}", p, 120 % p);
        }

        // Advance to 6! = 720
        fsieve.advance(6);
        for &(p, fm) in &fsieve.entries {
            assert_eq!(fm, 720 % p, "6! mod {} should be {}", p, 720 % p);
        }
    }

    #[test]
    fn factorial_sieve_removes_primes_when_p_divides_n() {
        // When n >= p, then p | n!, so the residue becomes 0 and the entry is removed
        let sieve_primes: Vec<u64> = vec![3, 5, 7, 11];
        let mut fsieve = FactorialSieve::new(&sieve_primes, 1);
        assert_eq!(fsieve.entries.len(), 4);

        // Advance to 3: p=3 divides 3!, so it should be removed
        fsieve.advance(2);
        fsieve.advance(3);
        assert_eq!(
            fsieve.entries.len(),
            3,
            "p=3 should be removed after advancing to n=3"
        );

        // Advance to 5: p=5 divides 5!
        fsieve.advance(4);
        fsieve.advance(5);
        assert_eq!(fsieve.entries.len(), 2, "p=5 should be removed after n=5");
    }

    #[test]
    fn factorial_sieve_check_composites_correct() {
        // 4! = 24. 4!+1 = 25 = 5*5 (composite), 4!-1 = 23 (prime)
        // For p=5: 24 mod 5 = 4 = p-1, so plus_composite = true
        // For p=23: 24 mod 23 = 1, so minus_composite = true
        let sieve_primes: Vec<u64> = vec![5, 23, 29];
        let mut fsieve = FactorialSieve::new(&sieve_primes, 1);
        for n in 2..=4 {
            fsieve.advance(n);
        }
        let (plus, minus) = fsieve.check_composites();
        assert!(plus, "4!+1=25 should be detected as composite (5|25)");
        assert!(minus, "4!-1=23 should be detected as composite (23|23)");
    }

    #[test]
    fn factorial_sieve_known_primes_survive() {
        // Known factorial primes (n!+1): n = 1, 2, 3, 11, 27, 37, 41
        // Known factorial primes (n!-1): n = 3, 4, 6, 7, 12, 14
        // The sieve should NOT eliminate these candidates
        let sieve_primes = sieve::generate_primes(10000);
        let known_plus: Vec<u64> = vec![1, 2, 3, 11, 27, 37, 41];
        let known_minus: Vec<u64> = vec![3, 4, 6, 7, 12, 14];

        // For each known factorial prime, verify the sieve does not eliminate it
        for &n in &known_plus {
            let mut fsieve = FactorialSieve::new(&sieve_primes, 1);
            for i in 2..=n {
                fsieve.advance(i);
            }
            // Sieve is only safe when n! > sieve_limit
            // For small n, n! may be smaller than sieve primes, so skip the sieve check
            if n >= 14 {
                let (plus_composite, _) = fsieve.check_composites();
                assert!(
                    !plus_composite,
                    "{}!+1 is prime but sieve marked plus_composite",
                    n
                );
            }
        }

        for &n in &known_minus {
            let mut fsieve = FactorialSieve::new(&sieve_primes, 1);
            for i in 2..=n {
                fsieve.advance(i);
            }
            if n >= 14 {
                let (_, minus_composite) = fsieve.check_composites();
                assert!(
                    !minus_composite,
                    "{}!-1 is prime but sieve marked minus_composite",
                    n
                );
            }
        }
    }

    #[test]
    fn factorial_values_correct() {
        // Verify incremental factorial computation matches GMP's factorial
        let mut factorial = Integer::from(1u32);
        for n in 2..=20u32 {
            factorial *= n;
            let expected = Integer::from(Integer::factorial(n));
            assert_eq!(factorial, expected, "Incremental {}! doesn't match GMP", n);
        }
    }

    #[test]
    fn factorial_known_primes_pass_mr() {
        // Verify that known factorial primes actually pass MR
        let cases: Vec<(u32, &str)> = vec![
            (1, "+"),  // 1!+1 = 2
            (2, "+"),  // 2!+1 = 3
            (3, "+"),  // 3!+1 = 7
            (3, "-"),  // 3!-1 = 5
            (4, "-"),  // 4!-1 = 23
            (6, "-"),  // 6!-1 = 719
            (7, "-"),  // 7!-1 = 5039
            (11, "+"), // 11!+1 = 39916801
            (12, "-"), // 12!-1 = 479001599
        ];
        for (n, sign) in cases {
            let f = Integer::from(Integer::factorial(n));
            let candidate = if sign == "+" {
                Integer::from(&f + 1u32)
            } else {
                Integer::from(&f - 1u32)
            };
            let result = mr_screened_test(&candidate, 25);
            assert_ne!(
                result,
                IsPrime::No,
                "{}!{}1 = {} should be prime",
                n,
                sign,
                candidate
            );
        }
    }

    #[test]
    fn factorial_known_composites_fail_mr() {
        // 4!+1 = 25 = 5^2, 5!+1 = 121 = 11^2
        let composites: Vec<(u32, &str)> = vec![(4, "+"), (5, "+"), (5, "-"), (8, "+"), (8, "-")];
        for (n, sign) in composites {
            let f = Integer::from(Integer::factorial(n));
            let candidate = if sign == "+" {
                Integer::from(&f + 1u32)
            } else {
                Integer::from(&f - 1u32)
            };
            let result = mr_screened_test(&candidate, 25);
            assert_eq!(
                result,
                IsPrime::No,
                "{}!{}1 = {} should be composite",
                n,
                sign,
                candidate
            );
        }
    }

    // ---- Wilson's theorem tests ----

    #[test]
    fn wilson_theorem_eliminates_correctly() {
        // Wilson's theorem: if n+1 is prime and n > 2, then (n+1) | (n!+1)
        // n=4: 5 is prime, 4!+1 = 25 = 5*5 ✓
        // n=6: 7 is prime, 6!+1 = 721 = 7*103 ✓
        // n=10: 11 is prime, 10!+1 = 3628801 = 11*329891 ✓
        for &n in &[4u64, 6, 10] {
            let f = Integer::from(Integer::factorial(n as u32));
            let plus = Integer::from(&f + 1u32);
            let np1 = Integer::from(n + 1);
            assert!(
                plus.is_divisible(&np1),
                "Wilson: ({}+1) should divide {}!+1", n, n
            );
        }
    }

    #[test]
    fn wilson_theorem_does_not_eliminate_when_np1_composite() {
        // When n+1 is composite, Wilson's theorem does NOT apply
        // n=3: n+1=4 (composite), 3!+1=7 (prime) — not eliminated by 4
        // n=7: n+1=8 (composite), 7!+1=5041 — not eliminated by 8
        for &n in &[3u64, 7] {
            let f = Integer::from(Integer::factorial(n as u32));
            let plus = Integer::from(&f + 1u32);
            let np1 = Integer::from(n + 1);
            // n+1 is composite
            assert_eq!(
                np1.is_probably_prime(10),
                IsPrime::No,
                "n+1={} should be composite", n + 1
            );
            // Wilson doesn't apply, so (n+1) should NOT divide n!+1
            assert!(
                !plus.is_divisible(&np1),
                "(n+1)={} should NOT divide {}!+1={} when n+1 is composite",
                n + 1, n, plus
            );
        }
    }

    #[test]
    fn factorial_sieve_new_with_initial_n_gt_1() {
        // Initialize sieve at n=5 — residues should match 5! = 120 mod p
        let sieve_primes: Vec<u64> = vec![7, 11, 13, 17];
        let fsieve = FactorialSieve::new(&sieve_primes, 5);

        for &(p, fm) in &fsieve.entries {
            assert_eq!(
                fm,
                120 % p,
                "5! mod {} should be {}, got {}",
                p, 120 % p, fm
            );
        }
    }

    #[test]
    fn factorial_sieve_check_composites_neither() {
        // When no sieve prime divides n!±1, both should return false
        // 3! = 6, 3!+1 = 7 (prime), 3!-1 = 5 (prime)
        // Use sieve primes that don't divide 7 or 5
        let sieve_primes: Vec<u64> = vec![11, 13, 17]; // all > 7
        let mut fsieve = FactorialSieve::new(&sieve_primes, 1);
        for n in 2..=3 {
            fsieve.advance(n);
        }
        let (plus, minus) = fsieve.check_composites();
        assert!(!plus, "3!+1=7 should not be sieved as composite by [11,13,17]");
        assert!(!minus, "3!-1=5 should not be sieved as composite by [11,13,17]");
    }

    #[test]
    fn factorial_sieve_advance_removes_exact_prime() {
        // p=5 should be removed when we advance to n=5 (since 5|5!)
        // p=7 should be removed when we advance to n=7
        let sieve_primes: Vec<u64> = vec![5, 7, 11, 13];
        let mut fsieve = FactorialSieve::new(&sieve_primes, 1);

        // Advance to 4 — p=5 still present
        for n in 2..=4 {
            fsieve.advance(n);
        }
        assert!(
            fsieve.entries.iter().any(|&(p, _)| p == 5),
            "p=5 should still be present at n=4"
        );

        // Advance to 5 — p=5 removed
        fsieve.advance(5);
        assert!(
            !fsieve.entries.iter().any(|&(p, _)| p == 5),
            "p=5 should be removed at n=5"
        );
        assert!(
            fsieve.entries.iter().any(|&(p, _)| p == 7),
            "p=7 should still be present at n=5"
        );

        // Advance to 7 — p=7 removed
        fsieve.advance(6);
        fsieve.advance(7);
        assert!(
            !fsieve.entries.iter().any(|&(p, _)| p == 7),
            "p=7 should be removed at n=7"
        );
    }

    #[test]
    fn factorial_start_equals_end() {
        // Single-n search (n=11): 11!+1 = 39916801 should be prime
        let f = Integer::from(Integer::factorial(11u32));
        let plus = Integer::from(&f + 1u32);
        assert_ne!(
            plus.is_probably_prime(25),
            IsPrime::No,
            "11!+1 should be prime"
        );
        // 11!-1 = 39916799 should be composite
        let minus = Integer::from(&f - 1u32);
        assert_eq!(
            minus.is_probably_prime(25),
            IsPrime::No,
            "11!-1 should be composite"
        );
    }

    #[test]
    fn factorial_n_equals_1_and_2() {
        // 1!+1=2 (prime), 1!-1=0 (not prime)
        assert_eq!(Integer::from(Integer::factorial(1u32)) + 1u32, 2);
        assert_ne!(
            Integer::from(2u32).is_probably_prime(25),
            IsPrime::No,
            "1!+1=2 should be prime"
        );
        assert_eq!(Integer::from(Integer::factorial(1u32)) - 1u32, 0);

        // 2!+1=3 (prime), 2!-1=1 (not prime by convention)
        assert_eq!(Integer::from(Integer::factorial(2u32)) + 1u32, 3);
        assert_ne!(
            Integer::from(3u32).is_probably_prime(25),
            IsPrime::No,
            "2!+1=3 should be prime"
        );
        assert_eq!(Integer::from(Integer::factorial(2u32)) - 1u32, 1);
    }
}
