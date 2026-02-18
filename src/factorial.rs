use anyhow::Result;
use rayon::prelude::*;
use rug::integer::IsPrime;
use rug::Integer;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::checkpoint::{self, Checkpoint};
use crate::db::Database;
use crate::progress::Progress;
use crate::{estimate_digits, exact_digits, sieve};

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
    fn advance(&mut self, n: u64) {
        self.entries.retain_mut(|(p, fm)| {
            *fm = *fm * (n % *p) % *p;
            *fm != 0
        });
    }

    /// Returns true if n!+1 is provably composite via the sieve.
    fn is_plus_composite(&self) -> bool {
        self.entries.iter().any(|&(p, fm)| fm == p - 1)
    }

    /// Returns true if n!-1 is provably composite via the sieve.
    fn is_minus_composite(&self) -> bool {
        self.entries.iter().any(|&(_, fm)| fm == 1)
    }
}

pub fn search(
    start: u64,
    end: u64,
    progress: &Arc<Progress>,
    db: &Arc<Mutex<Database>>,
    checkpoint_path: &Path,
    search_params: &str,
) -> Result<()> {
    let sieve_primes = sieve::generate_primes(sieve::SIEVE_LIMIT);
    eprintln!(
        "Sieve initialized with {} primes up to {}",
        sieve_primes.len(),
        sieve::SIEVE_LIMIT
    );

    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::Factorial { last_n }) if last_n >= start && last_n < end => {
            eprintln!("Resuming factorial search from n={}", last_n + 1);
            last_n + 1
        }
        _ => start,
    };

    // Compute factorial up to the starting point
    if resume_from > 2 {
        eprintln!("Precomputing {}!...", resume_from - 1);
    }
    let mut factorial = Integer::from(1u32);
    for i in 2..resume_from {
        factorial *= i;
    }
    if resume_from > 2 {
        eprintln!("Precomputation complete.");
    }

    // Initialize modular sieve at (resume_from - 1)!
    eprintln!("Initializing modular sieve...");
    let mut fsieve = FactorialSieve::new(&sieve_primes, resume_from.saturating_sub(1));
    eprintln!("Modular sieve ready ({} active primes).", fsieve.entries.len());

    let mut last_checkpoint = Instant::now();
    let mut sieved_out: u64 = 0;

    for n in resume_from..=end {
        factorial *= n;
        fsieve.advance(n);

        let approx_digits = estimate_digits(&factorial);
        *progress.current.lock().unwrap() = format!("{}! (~{} digits)", n, approx_digits);
        progress.tested.fetch_add(2, Ordering::Relaxed);

        // Sieve is safe when n! > SIEVE_LIMIT (true for n >= 10 since 10! > 10^6).
        // For small n, skip the sieve and test directly.
        let sieve_safe = n >= 10;
        let test_plus = !sieve_safe || !fsieve.is_plus_composite();
        let test_minus = !sieve_safe || !fsieve.is_minus_composite();

        if !test_plus && !test_minus {
            sieved_out += 1;
            continue;
        }

        // Only construct the huge n!±1 Integers for candidates that survived the sieve
        let (r_plus, r_minus) = rayon::join(
            || {
                if test_plus {
                    let plus = factorial.clone() + 1u32;
                    plus.is_probably_prime(25)
                } else {
                    IsPrime::No
                }
            },
            || {
                if test_minus {
                    let minus = factorial.clone() - 1u32;
                    minus.is_probably_prime(25)
                } else {
                    IsPrime::No
                }
            },
        );

        for (result, sign) in [(r_plus, "+"), (r_minus, "-")] {
            if result != IsPrime::No {
                let digit_count = exact_digits(&factorial);
                let certainty = match result {
                    IsPrime::Yes => "deterministic",
                    IsPrime::Probably => "probabilistic",
                    IsPrime::No => unreachable!(),
                };
                let expr = format!("{}! {} 1", n, sign);
                progress.found.fetch_add(1, Ordering::Relaxed);
                eprintln!(
                    "*** PRIME FOUND: {} ({} digits, {}) ***",
                    expr, digit_count, certainty
                );
                db.lock()
                    .unwrap()
                    .insert_prime("factorial", &expr, digit_count, search_params)?;
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(checkpoint_path, &Checkpoint::Factorial { last_n: n })?;
            eprintln!("Checkpoint saved at n={} (sieved out: {})", n, sieved_out);
            last_checkpoint = Instant::now();
        }
    }

    checkpoint::clear(checkpoint_path);
    eprintln!("Factorial sieve eliminated {} candidates.", sieved_out);
    Ok(())
}
