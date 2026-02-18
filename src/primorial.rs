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
use crate::proof;
use crate::CoordinationClient;
use crate::{estimate_digits, exact_digits, mr_screened_test, sieve};

/// Incremental modular sieve for primorial primes.
///
/// Maintains p# mod q for each sieve prime q > current p. Since q > p means q
/// does not divide p#, we have p# mod q != 0. We can then cheaply check:
///   - p# + 1 is composite if p# mod q == q - 1 (meaning q | p#+1)
///   - p# - 1 is composite if p# mod q == 1     (meaning q | p#-1)
struct PrimorialSieve {
    /// (sieve_prime, p# mod sieve_prime) for active primes where q > current p.
    entries: Vec<(u64, u64)>,
}

impl PrimorialSieve {
    /// Initialize sieve. Computes primorial of all_primes[0..resume_idx] mod q
    /// for each sieve prime q > all_primes[resume_idx-1].
    fn new(sieve_primes: &[u64], all_primes: &[u64], resume_idx: usize) -> Self {
        let max_included = if resume_idx > 0 {
            all_primes[resume_idx - 1]
        } else {
            0
        };

        let entries: Vec<(u64, u64)> = sieve_primes
            .par_iter()
            .filter(|&&q| q > max_included)
            .map(|&q| {
                let mut pm = 1u64;
                for i in 0..resume_idx {
                    pm = pm * (all_primes[i] % q) % q;
                }
                (q, pm)
            })
            .collect();
        PrimorialSieve { entries }
    }

    /// Advance from previous primorial to p# by multiplying all residues by prime p.
    /// Removes entries where q divides the new primorial (when p >= q).
    fn advance(&mut self, p: u64) {
        if self.entries.len() > 10_000 {
            self.entries.par_iter_mut().for_each(|(q, pm)| {
                *pm = *pm * (p % *q) % *q;
            });
            self.entries.retain(|(_, pm)| *pm != 0);
        } else {
            self.entries.retain_mut(|(q, pm)| {
                *pm = *pm * (p % *q) % *q;
                *pm != 0
            });
        }
    }

    /// Check if p#+1 or p#-1 is provably composite via the sieve.
    /// Returns (plus_composite, minus_composite) in a single pass.
    fn check_composites(&self) -> (bool, bool) {
        let mut plus_composite = false;
        let mut minus_composite = false;
        for &(q, pm) in &self.entries {
            if !plus_composite && pm == q - 1 {
                plus_composite = true;
            }
            if !minus_composite && pm == 1 {
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
    // Generate all primes up to end — these are the p values we iterate over
    let all_primes = sieve::generate_primes(end);
    let search_count = all_primes.iter().filter(|&&p| p >= start).count();

    if search_count == 0 {
        eprintln!("No primes in range [{}, {}]", start, end);
        return Ok(());
    }

    let sieve_primes = sieve::generate_primes(sieve_limit);
    eprintln!(
        "Sieve initialized with {} primes up to {}",
        sieve_primes.len(),
        sieve_limit
    );
    eprintln!(
        "Testing {} primes in range [{}, {}]",
        search_count, start, end
    );

    // Determine resume point from checkpoint
    let resume_prime = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::Primorial { last_prime, .. })
            if last_prime >= start && last_prime < end =>
        {
            eprintln!("Resuming primorial search from after p={}", last_prime);
            last_prime
        }
        _ => 0,
    };

    // Find the index in all_primes where we should start computing
    let resume_idx = if resume_prime > 0 {
        all_primes
            .iter()
            .position(|&p| p > resume_prime)
            .unwrap_or(all_primes.len())
    } else {
        0
    };

    // Precompute primorial up to the prime before resume point using GMP's optimized algorithm
    let mut primorial = if resume_idx > 0 {
        let prev_prime = all_primes[resume_idx - 1];
        eprintln!("Precomputing {}#...", prev_prime);
        let p = Integer::from(Integer::primorial(prev_prime as u32));
        eprintln!("Precomputation complete.");
        p
    } else {
        Integer::from(1u32)
    };

    // Initialize modular sieve
    eprintln!("Initializing modular sieve...");
    let mut psieve = PrimorialSieve::new(&sieve_primes, &all_primes, resume_idx);
    eprintln!(
        "Modular sieve ready ({} active primes).",
        psieve.entries.len()
    );

    // Compute minimum prime where p# > sieve_limit, making the sieve safe
    let sieve_min_prime: u64 = {
        let mut prod: u128 = 1;
        let mut min_p = 2u64;
        for &p in &all_primes {
            prod = prod.saturating_mul(p as u128);
            if prod > sieve_limit as u128 {
                min_p = p;
                break;
            }
        }
        min_p
    };
    eprintln!("Sieve active for p >= {}", sieve_min_prime);

    let mut last_checkpoint = Instant::now();
    let mut sieved_out: u64 = 0;

    for &p in &all_primes[resume_idx..] {
        primorial *= p;
        psieve.advance(p);

        // Only test primes in the search range
        if p < start {
            continue;
        }

        let approx_digits = estimate_digits(&primorial);
        *progress.current.lock().unwrap() = format!("{}# (~{} digits)", p, approx_digits);
        progress.tested.fetch_add(2, Ordering::Relaxed);

        let sieve_safe = p >= sieve_min_prime;
        let (plus_composite, minus_composite) = if sieve_safe {
            psieve.check_composites()
        } else {
            (false, false)
        };
        let test_plus = !plus_composite;
        let test_minus = !minus_composite;

        if !test_plus && !test_minus {
            sieved_out += 1;
            continue;
        }

        // Only construct the huge p#±1 Integers for candidates that survived the sieve
        let (r_plus, r_minus) = rayon::join(
            || {
                if test_plus {
                    let plus = Integer::from(&primorial + 1u32);
                    mr_screened_test(&plus, mr_rounds)
                } else {
                    IsPrime::No
                }
            },
            || {
                if test_minus {
                    let minus = Integer::from(&primorial - 1u32);
                    mr_screened_test(&minus, mr_rounds)
                } else {
                    IsPrime::No
                }
            },
        );

        for (result, sign) in [(r_plus, "+"), (r_minus, "-")] {
            if result != IsPrime::No {
                let digit_count = exact_digits(&primorial);
                let mut certainty = match result {
                    IsPrime::Yes => "deterministic",
                    IsPrime::Probably => "probabilistic",
                    IsPrime::No => unreachable!(),
                };

                // Attempt deterministic proof for probable primes.
                // p# has the same distinct prime factors as p! (all primes ≤ p),
                // so we can reuse the factorial proof functions.
                if result == IsPrime::Probably {
                    let proven = if sign == "+" {
                        let candidate = Integer::from(&primorial + 1u32);
                        proof::pocklington_factorial_proof(p, &candidate, &sieve_primes)
                    } else {
                        let candidate = Integer::from(&primorial - 1u32);
                        proof::morrison_factorial_proof(p, &candidate, &sieve_primes)
                    };
                    if proven {
                        certainty = if sign == "+" {
                            "deterministic (Pocklington N-1)"
                        } else {
                            "deterministic (Morrison N+1)"
                        };
                    }
                }

                let expr = format!("{}# {} 1", p, sign);
                progress.found.fetch_add(1, Ordering::Relaxed);
                if let Some(eb) = event_bus {
                    eb.emit(events::Event::PrimeFound {
                        form: "primorial".into(),
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
                    "primorial",
                    &expr,
                    digit_count,
                    search_params,
                    certainty,
                )?;
                if let Some(wc) = worker_client {
                    wc.report_prime("primorial", &expr, digit_count, search_params, certainty);
                }
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Primorial {
                    last_prime: p,
                    start: Some(start),
                    end: Some(end),
                },
            )?;
            eprintln!("Checkpoint saved at p={} (sieved out: {})", p, sieved_out);
            last_checkpoint = Instant::now();
        }

        if worker_client.is_some_and(|wc| wc.is_stop_requested()) {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Primorial {
                    last_prime: p,
                    start: Some(start),
                    end: Some(end),
                },
            )?;
            eprintln!("Stop requested by coordinator, checkpoint saved at p={}", p);
            return Ok(());
        }
    }

    checkpoint::clear(checkpoint_path);
    eprintln!("Primorial sieve eliminated {} candidates.", sieved_out);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sieve;

    /// Known primorial primes p#+1: p = 2, 3, 5, 7, 11, 31, 379, 1019, 1021
    /// Known primorial primes p#-1: p = 3, 5, 11, 13, 41, 89, 317, 337
    fn primorial(p: u64) -> Integer {
        Integer::from(Integer::primorial(p as u32))
    }

    #[test]
    fn known_primorial_primes_plus() {
        for &p in &[2u64, 3, 5, 7, 11, 31] {
            let candidate = primorial(p) + 1u32;
            assert_ne!(
                candidate.is_probably_prime(25),
                IsPrime::No,
                "{}# + 1 should be prime",
                p
            );
        }
    }

    #[test]
    fn known_primorial_primes_minus() {
        for &p in &[3u64, 5, 11, 13, 41, 89] {
            let candidate = primorial(p) - 1u32;
            assert_ne!(
                candidate.is_probably_prime(25),
                IsPrime::No,
                "{}# - 1 should be prime",
                p
            );
        }
    }

    #[test]
    fn known_primorial_composites_plus() {
        // 13# + 1 = 30031 = 59 * 509
        let candidate = primorial(13) + 1u32;
        assert_eq!(
            candidate.is_probably_prime(25),
            IsPrime::No,
            "13# + 1 should be composite"
        );
    }

    #[test]
    fn known_primorial_composites_minus() {
        // 7# - 1 = 209 = 11 * 19
        let candidate = primorial(7) - 1u32;
        assert_eq!(
            candidate.is_probably_prime(25),
            IsPrime::No,
            "7# - 1 should be composite"
        );
    }

    #[test]
    fn sieve_correctly_eliminates() {
        let sieve_primes = sieve::generate_primes(10_000);
        let all_primes = sieve::generate_primes(100);

        // Compute sieve_min_prime: smallest p where p# > 10000
        let sieve_min_prime: u64 = {
            let mut prod: u128 = 1;
            let mut min_p = 2u64;
            for &p in &all_primes {
                prod = prod.saturating_mul(p as u128);
                if prod > 10_000 {
                    min_p = p;
                    break;
                }
            }
            min_p
        };

        // Initialize sieve at the beginning
        let mut psieve = PrimorialSieve::new(&sieve_primes, &all_primes, 0);

        // Advance through primes and verify sieve claims for safe range
        for (idx, &p) in all_primes.iter().enumerate() {
            psieve.advance(p);

            // Only check sieve claims when p# > sieve_limit (avoids false
            // composites where the candidate equals a sieve prime)
            if p < sieve_min_prime {
                continue;
            }

            let prim = primorial(p);
            let (plus_comp, minus_comp) = psieve.check_composites();

            if plus_comp {
                let candidate = Integer::from(&prim + 1u32);
                assert_eq!(
                    candidate.is_probably_prime(25),
                    IsPrime::No,
                    "Sieve said {}#+1 composite but it's prime (idx={})",
                    p,
                    idx
                );
            }
            if minus_comp {
                let candidate = Integer::from(&prim - 1u32);
                assert_eq!(
                    candidate.is_probably_prime(25),
                    IsPrime::No,
                    "Sieve said {}#-1 composite but it's prime (idx={})",
                    p,
                    idx
                );
            }
        }
    }

    #[test]
    fn pocklington_proves_primorial_plus() {
        let sieve_primes = sieve::generate_primes(1000);
        // 31# + 1 is prime; verify Pocklington proof works
        let prim = primorial(31);
        let candidate = Integer::from(&prim + 1u32);
        assert_ne!(candidate.is_probably_prime(25), IsPrime::No);
        assert!(
            proof::pocklington_factorial_proof(31, &candidate, &sieve_primes),
            "Pocklington should prove 31#+1 prime"
        );
    }

    #[test]
    fn morrison_proves_primorial_minus() {
        let sieve_primes = sieve::generate_primes(1000);
        // 89# - 1 is prime; verify Morrison proof works
        let prim = primorial(89);
        let candidate = Integer::from(&prim - 1u32);
        assert_ne!(candidate.is_probably_prime(25), IsPrime::No);
        assert!(
            proof::morrison_factorial_proof(89, &candidate, &sieve_primes),
            "Morrison should prove 89#-1 prime"
        );
    }
}
