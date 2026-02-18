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

const BATCH_SIZE: u64 = 1000;

/// Construct a palindrome from its first-half value in the given base.
///
/// For digit_count=5, base=10, first_half=123: palindrome = 12321
/// For digit_count=4, base=10, first_half=12:  palindrome = 1221
fn make_palindrome(first_half: &Integer, digit_count: u64, base: u32) -> Integer {
    let half_len = ((digit_count + 1) / 2) as usize;
    let is_odd = digit_count % 2 == 1;

    // Extract digits of first_half in the given base
    let half_str = first_half.to_string_radix(base as i32);
    let half_digits: Vec<u32> = half_str
        .bytes()
        .map(|b| {
            if b.is_ascii_digit() {
                (b - b'0') as u32
            } else {
                // a=10, b=11, ... for bases > 10
                (b - b'a') as u32 + 10
            }
        })
        .collect();

    // Pad with leading zeros if needed
    let mut digits = Vec::with_capacity(digit_count as usize);
    for _ in 0..half_len.saturating_sub(half_digits.len()) {
        digits.push(0u32);
    }
    digits.extend_from_slice(&half_digits);

    // Mirror to form full palindrome
    let mirror: Vec<u32> = if is_odd {
        digits[..digits.len() - 1].iter().rev().copied().collect()
    } else {
        digits.iter().rev().copied().collect()
    };
    digits.extend_from_slice(&mirror);

    // Convert digit array back to Integer
    let mut result = Integer::new();
    for &d in &digits {
        result *= base;
        result += d;
    }
    result
}

pub fn search(
    base: u32,
    min_digits: u64,
    max_digits: u64,
    progress: &Arc<Progress>,
    db: &Arc<Mutex<Database>>,
    checkpoint_path: &Path,
    search_params: &str,
) -> Result<()> {
    let (resume_digits, resume_half) = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::Palindromic {
            digit_count,
            half_value,
        }) if digit_count >= min_digits && digit_count <= max_digits => {
            let half: Integer = half_value
                .parse()
                .unwrap_or_else(|_| Integer::from(base).pow(((digit_count + 1) / 2 - 1) as u32));
            eprintln!(
                "Resuming palindromic search from {} digits, half={}",
                digit_count, half_value
            );
            (digit_count, Some(half))
        }
        _ => (min_digits, None),
    };

    let mut last_checkpoint = Instant::now();

    for digit_count in resume_digits..=max_digits {
        // Even-digit palindromes are always divisible by (base+1).
        // The only possible prime is (base+1) itself, which is "11" in that base.
        if digit_count % 2 == 0 {
            if digit_count == 2 {
                let candidate = Integer::from(base + 1);
                let r = candidate.is_probably_prime(25);
                progress.tested.fetch_add(1, Ordering::Relaxed);
                if r != IsPrime::No {
                    let cert = match r {
                        IsPrime::Yes => "deterministic",
                        _ => "probabilistic",
                    };
                    let digits = candidate.to_string_radix(10).len() as u64;
                    let expr = format!("{}", candidate);
                    progress.found.fetch_add(1, Ordering::Relaxed);
                    eprintln!(
                        "*** PRIME FOUND: {} ({} digits, {}) ***",
                        expr, digits, cert
                    );
                    db.lock().unwrap().insert_prime(
                        "palindromic",
                        &expr,
                        digits,
                        search_params,
                    )?;
                }
            }
            continue;
        }

        let half_len = ((digit_count + 1) / 2) as u32;
        let half_start = if digit_count == resume_digits {
            resume_half
                .clone()
                .unwrap_or_else(|| Integer::from(base).pow(half_len - 1))
        } else {
            Integer::from(base).pow(half_len - 1)
        };
        let half_end = Integer::from(Integer::from(base).pow(half_len)) - 1u32;

        eprintln!(
            "Searching {}-digit palindromes in base {} (half {} to {})",
            digit_count, base, half_start, half_end
        );

        let mut current_half = half_start;

        while current_half <= half_end {
            // Generate a batch of palindromes
            let mut batch: Vec<(Integer, String)> = Vec::new();
            for _ in 0..BATCH_SIZE {
                if current_half > half_end {
                    break;
                }
                let palindrome = make_palindrome(&current_half, digit_count, base);
                let expr = palindrome.to_string_radix(10);
                batch.push((palindrome, expr));
                current_half += 1u32;
            }

            if batch.is_empty() {
                break;
            }

            let batch_size = batch.len() as u64;
            *progress.current.lock().unwrap() =
                format!("{}-digit palindrome (base {})", digit_count, base);

            let found_primes: Vec<_> = batch
                .into_par_iter()
                .filter_map(|(num, expr)| {
                    let r = num.is_probably_prime(25);
                    if r != IsPrime::No {
                        let cert = match r {
                            IsPrime::Yes => "deterministic",
                            _ => "probabilistic",
                        };
                        let digits = expr.len() as u64;
                        Some((expr, digits, cert.to_string()))
                    } else {
                        None
                    }
                })
                .collect();

            progress.tested.fetch_add(batch_size, Ordering::Relaxed);

            for (expr, digits, certainty) in found_primes {
                progress.found.fetch_add(1, Ordering::Relaxed);
                eprintln!(
                    "*** PRIME FOUND: {} ({} digits, {}) ***",
                    expr, digits, certainty
                );
                db.lock()
                    .unwrap()
                    .insert_prime("palindromic", &expr, digits, search_params)?;
            }

            if last_checkpoint.elapsed().as_secs() >= 60 {
                let half_val = current_half.to_string_radix(10);
                checkpoint::save(
                    checkpoint_path,
                    &Checkpoint::Palindromic {
                        digit_count,
                        half_value: half_val.clone(),
                    },
                )?;
                eprintln!(
                    "Checkpoint saved at {} digits, half={}",
                    digit_count, half_val
                );
                last_checkpoint = Instant::now();
            }
        }
    }

    checkpoint::clear(checkpoint_path);
    Ok(())
}
