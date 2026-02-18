use anyhow::Result;
use rayon::prelude::*;
use rug::integer::IsPrime;
use rug::ops::Pow;
use rug::Integer;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::checkpoint::{self, Checkpoint};
use crate::db::Database;
use crate::progress::Progress;

const BLOCK_SIZE: u64 = 100;

pub fn search(
    k: u64,
    base: u32,
    min_n: u64,
    max_n: u64,
    progress: &Arc<Progress>,
    db: &Arc<Mutex<Database>>,
    checkpoint_path: &Path,
    search_params: &str,
) -> Result<()> {
    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::Kbn { last_n }) if last_n >= min_n && last_n < max_n => {
            eprintln!("Resuming kbn search from n={}", last_n + 1);
            last_n + 1
        }
        _ => min_n,
    };

    let mut last_checkpoint = Instant::now();
    let mut block_start = resume_from;

    while block_start <= max_n {
        let block_end = (block_start + BLOCK_SIZE - 1).min(max_n);
        let block_len = block_end - block_start + 1;

        *progress.current.lock().unwrap() =
            format!("{}*{}^[{}..{}]+-1", k, base, block_start, block_end);

        let found_primes: Vec<_> = (block_start..=block_end)
            .into_par_iter()
            .flat_map_iter(|n| {
                let base_pow = Integer::from(base).pow(n as u32);
                let kb = Integer::from(k) * &base_pow;
                let mut results = Vec::new();

                // Test k*b^n + 1
                let plus = kb.clone() + 1u32;
                let digits = plus.to_string_radix(10).len() as u64;
                let r = plus.is_probably_prime(25);
                if r != IsPrime::No {
                    let cert = match r {
                        IsPrime::Yes => "deterministic",
                        _ => "probabilistic",
                    };
                    results.push((
                        format!("{}*{}^{} + 1", k, base, n),
                        digits,
                        cert.to_string(),
                    ));
                }

                // Test k*b^n - 1
                let minus = kb - 1u32;
                let digits = minus.to_string_radix(10).len() as u64;
                let r = minus.is_probably_prime(25);
                if r != IsPrime::No {
                    let cert = match r {
                        IsPrime::Yes => "deterministic",
                        _ => "probabilistic",
                    };
                    results.push((
                        format!("{}*{}^{} - 1", k, base, n),
                        digits,
                        cert.to_string(),
                    ));
                }

                results
            })
            .collect();

        progress.tested.fetch_add(block_len * 2, Ordering::Relaxed);

        for (expr, digits, certainty) in found_primes {
            progress.found.fetch_add(1, Ordering::Relaxed);
            eprintln!(
                "*** PRIME FOUND: {} ({} digits, {}) ***",
                expr, digits, certainty
            );
            db.lock()
                .unwrap()
                .insert_prime("kbn", &expr, digits, search_params)?;
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(checkpoint_path, &Checkpoint::Kbn { last_n: block_end })?;
            eprintln!("Checkpoint saved at n={}", block_end);
            last_checkpoint = Instant::now();
        }

        block_start = block_end + 1;
    }

    checkpoint::clear(checkpoint_path);
    Ok(())
}
