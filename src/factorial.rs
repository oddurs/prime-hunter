use anyhow::Result;
use rug::integer::IsPrime;
use rug::Integer;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::checkpoint::{self, Checkpoint};
use crate::db::Database;
use crate::progress::Progress;

pub fn search(
    start: u64,
    end: u64,
    progress: &Arc<Progress>,
    db: &Arc<Mutex<Database>>,
    checkpoint_path: &Path,
    search_params: &str,
) -> Result<()> {
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

    let mut last_checkpoint = Instant::now();

    for n in resume_from..=end {
        factorial *= n;

        let digit_count = factorial.to_string_radix(10).len() as u64;
        *progress.current.lock().unwrap() = format!("{}! ({} digits)", n, digit_count);

        // Test n!+1 and n!-1 in parallel
        let plus = factorial.clone() + 1u32;
        let minus = factorial.clone() - 1u32;

        let (r_plus, r_minus) = rayon::join(
            || plus.is_probably_prime(25),
            || minus.is_probably_prime(25),
        );

        progress.tested.fetch_add(2, Ordering::Relaxed);

        for (result, sign) in [(r_plus, "+"), (r_minus, "-")] {
            if result != IsPrime::No {
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
            eprintln!("Checkpoint saved at n={}", n);
            last_checkpoint = Instant::now();
        }
    }

    checkpoint::clear(checkpoint_path);
    Ok(())
}
