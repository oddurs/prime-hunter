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

    // Compute factorial up to the starting point using GMP's optimized binary-splitting
    let mut factorial = if resume_from > 2 {
        eprintln!("Precomputing {}!...", resume_from - 1);
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

        // Only construct the huge n!±1 Integers for candidates that survived the sieve
        let (r_plus, r_minus) = rayon::join(
            || {
                if test_plus {
                    let plus = factorial.clone() + 1u32;
                    mr_screened_test(&plus, mr_rounds)
                } else {
                    IsPrime::No
                }
            },
            || {
                if test_minus {
                    let minus = factorial.clone() - 1u32;
                    mr_screened_test(&minus, mr_rounds)
                } else {
                    IsPrime::No
                }
            },
        );

        for (result, sign) in [(r_plus, "+"), (r_minus, "-")] {
            if result != IsPrime::No {
                let digit_count = exact_digits(&factorial);
                let mut certainty = match result {
                    IsPrime::Yes => "deterministic",
                    IsPrime::Probably => "probabilistic",
                    IsPrime::No => unreachable!(),
                };

                // Attempt deterministic proof for probable primes
                if result == IsPrime::Probably {
                    let proven = if sign == "+" {
                        let candidate = Integer::from(&factorial + 1u32);
                        proof::pocklington_factorial_proof(n, &candidate, &sieve_primes)
                    } else {
                        let candidate = Integer::from(&factorial - 1u32);
                        proof::morrison_factorial_proof(n, &candidate, &sieve_primes)
                    };
                    if proven {
                        certainty = if sign == "+" {
                            "deterministic (Pocklington N-1)"
                        } else {
                            "deterministic (Morrison N+1)"
                        };
                    }
                }

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
