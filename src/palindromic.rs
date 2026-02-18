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
use crate::progress::Progress;
use crate::CoordinationClient;
use crate::{mr_screened_test, sieve};

const BATCH_SIZE: u64 = 1000;

/// Convert an Integer to a zero-padded digit array in the given base.
fn integer_to_digits(val: &Integer, len: usize, base: u32) -> Vec<u32> {
    let s = val.to_string_radix(base as i32);
    let parsed: Vec<u32> = s
        .bytes()
        .map(|b| {
            if b.is_ascii_digit() {
                (b - b'0') as u32
            } else {
                (b - b'a') as u32 + 10
            }
        })
        .collect();
    let mut digits = Vec::with_capacity(len);
    for _ in 0..len.saturating_sub(parsed.len()) {
        digits.push(0u32);
    }
    digits.extend_from_slice(&parsed);
    digits
}

/// Increment a digit array by 1 in the given base. Returns true on overflow.
#[inline]
fn increment_digits(digits: &mut [u32], base: u32) -> bool {
    for d in digits.iter_mut().rev() {
        *d += 1;
        if *d < base {
            return false;
        }
        *d = 0;
    }
    true
}

/// Mirror half-digits to form the full palindrome digit array.
/// Odd length: [a,b,c] -> [a,b,c,b,a]. Even length: [a,b] -> [a,b,b,a].
fn mirror_to_palindrome(half_digits: &[u32], is_odd: bool) -> Vec<u32> {
    let full_len = half_digits.len() * 2 - if is_odd { 1 } else { 0 };
    let mut digits = Vec::with_capacity(full_len);
    digits.extend_from_slice(half_digits);
    let mirror_from = if is_odd {
        half_digits.len().saturating_sub(1)
    } else {
        half_digits.len()
    };
    for i in (0..mirror_from).rev() {
        digits.push(half_digits[i]);
    }
    digits
}

/// Evaluate a digit array as number mod m using Horner's method (all u64).
#[inline]
fn digits_mod(digits: &[u32], base: u64, m: u64) -> u64 {
    let mut r: u64 = 0;
    for &d in digits {
        r = (r * base + d as u64) % m;
    }
    r
}

/// Convert a digit array to an Integer.
fn digits_to_integer(digits: &[u32], base: u32) -> Integer {
    let mut result = Integer::new();
    for &d in digits {
        result *= base;
        result += d;
    }
    result
}

/// Check if the palindrome (represented as digits) is divisible by any filter prime.
/// Only uses primes smaller than the minimum candidate value (base^(digits-1))
/// to avoid false positives on candidates that equal a sieve prime.
fn is_filter_composite(digits: &[u32], base: u32, filter_primes: &[u64], digit_count: u64) -> bool {
    let b = base as u64;
    let min_value = b.pow((digit_count - 1) as u32);
    for &p in filter_primes {
        if p >= min_value {
            break;
        }
        if digits_mod(digits, b, p) == 0 {
            return true;
        }
    }
    false
}

pub fn search(
    base: u32,
    min_digits: u64,
    max_digits: u64,
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
    let (resume_digits, resume_half) = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::Palindromic {
            digit_count,
            half_value,
            ..
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

    // Leading digit filter: for a palindrome, the first digit equals the last digit.
    // A prime > base must have its last digit coprime to the base.
    // For base 10: valid leading digits are {1, 3, 7, 9} (not divisible by 2 or 5).
    let all_digits: Vec<u32> = (1..base).collect();
    let valid_digits: Vec<u32> = (1..base).filter(|&d| sieve::gcd(d, base) == 1).collect();
    let total_digits = base - 1;
    eprintln!(
        "Leading digit filter: {}/{} digits valid for base {} ({:?})",
        valid_digits.len(),
        total_digits,
        base,
        valid_digits
    );

    let sieve_primes = sieve::generate_primes(sieve_limit);
    let filter_primes: Vec<u64> = sieve_primes.iter().copied().filter(|&p| p >= 3).collect();
    eprintln!(
        "Digit pre-filter initialized with {} primes up to {}",
        filter_primes.len(),
        filter_primes.last().copied().unwrap_or(0)
    );

    let mut last_checkpoint = Instant::now();
    let mut total_filtered: u64 = 0;

    for digit_count in resume_digits..=max_digits {
        // Even-digit palindromes are always divisible by (base+1).
        // Only (base+1) itself can be prime — it's "11" in that base with 2 digits.
        if digit_count % 2 == 0 {
            if digit_count == 2 {
                let candidate = Integer::from(base + 1);
                let r = candidate.is_probably_prime(mr_rounds);
                progress.tested.fetch_add(1, Ordering::Relaxed);
                if r != IsPrime::No {
                    let cert = match r {
                        IsPrime::Yes => "deterministic",
                        _ => "probabilistic",
                    };
                    let digits = candidate.to_string_radix(10).len() as u64;
                    let expr = format!("{}", candidate);
                    progress.found.fetch_add(1, Ordering::Relaxed);
                    if let Some(eb) = event_bus {
                        eb.emit(events::Event::PrimeFound {
                            form: "palindromic".into(),
                            expression: expr.clone(),
                            digits,
                            proof_method: cert.to_string(),
                            timestamp: Instant::now(),
                        });
                    } else {
                        eprintln!(
                            "*** PRIME FOUND: {} ({} digits, {}) ***",
                            expr, digits, cert
                        );
                    }
                    db.insert_prime_sync(rt, "palindromic", &expr, digits, search_params, cert)?;
                    if let Some(wc) = worker_client {
                        wc.report_prime("palindromic", &expr, digits, search_params, cert);
                    }
                }
            }
            continue;
        }

        let half_len = ((digit_count + 1) / 2) as usize;
        let is_odd = digit_count % 2 == 1;
        let base_pow_half = Integer::from(base).pow((half_len - 1) as u32);

        // For single-digit palindromes, check all digits (2, 3, 5, 7 are all prime).
        // The coprime filter only applies to multi-digit numbers where last digit matters.
        let digits_to_check: &[u32] = if digit_count == 1 {
            &all_digits
        } else {
            &valid_digits
        };

        for &lead_digit in digits_to_check {
            let sub_start = Integer::from(lead_digit) * &base_pow_half;
            let sub_end = Integer::from(lead_digit + 1) * &base_pow_half - 1u32;

            // Handle resume: skip sub-ranges we've already completed
            let mut start_half = sub_start.clone();
            if digit_count == resume_digits {
                if let Some(ref rh) = resume_half {
                    if *rh > sub_end {
                        continue;
                    }
                    if *rh > start_half {
                        start_half = rh.clone();
                    }
                }
            }

            if digit_count == resume_digits || lead_digit == digits_to_check[0] {
                eprintln!(
                    "Searching {}-digit palindromes in base {} (leading digit {}, half {} to {})",
                    digit_count, base, lead_digit, start_half, sub_end
                );
            }

            // Use digit arrays for the inner loop to avoid per-candidate Integer arithmetic
            let mut half_digits = integer_to_digits(&start_half, half_len, base);
            let end_digits = integer_to_digits(&sub_end, half_len, base);

            let mut exhausted = false;
            while !exhausted && half_digits <= end_digits {
                let mut batch: Vec<Integer> = Vec::new();
                let mut batch_total: u64 = 0;

                for _ in 0..BATCH_SIZE {
                    if half_digits > end_digits {
                        break;
                    }
                    batch_total += 1;

                    let full_digits = mirror_to_palindrome(&half_digits, is_odd);

                    // Digit-based pre-filter: check divisibility without building Integer
                    if is_filter_composite(&full_digits, base, &filter_primes, digit_count) {
                        total_filtered += 1;
                    } else {
                        batch.push(digits_to_integer(&full_digits, base));
                    }

                    if increment_digits(&mut half_digits, base) {
                        exhausted = true;
                        break;
                    }
                }

                if batch_total == 0 {
                    break;
                }

                *progress.current.lock().unwrap() = format!(
                    "{}-digit palindrome (base {}, d={})",
                    digit_count, base, lead_digit
                );

                // Only candidates surviving the digit filter need primality testing
                let found_primes: Vec<_> = batch
                    .into_par_iter()
                    .filter_map(|num| {
                        let r = mr_screened_test(&num, mr_rounds);
                        if r != IsPrime::No {
                            let cert = match r {
                                IsPrime::Yes => "deterministic",
                                _ => "probabilistic",
                            };
                            // String conversion only for actual primes (very rare)
                            let expr = num.to_string_radix(10);
                            let digits = expr.len() as u64;
                            Some((expr, digits, cert.to_string()))
                        } else {
                            None
                        }
                    })
                    .collect();

                progress.tested.fetch_add(batch_total, Ordering::Relaxed);

                for (expr, digits, certainty) in found_primes {
                    progress.found.fetch_add(1, Ordering::Relaxed);
                    if let Some(eb) = event_bus {
                        eb.emit(events::Event::PrimeFound {
                            form: "palindromic".into(),
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
                    db.insert_prime_sync(
                        rt,
                        "palindromic",
                        &expr,
                        digits,
                        search_params,
                        &certainty,
                    )?;
                    if let Some(wc) = worker_client {
                        wc.report_prime("palindromic", &expr, digits, search_params, &certainty);
                    }
                }

                if last_checkpoint.elapsed().as_secs() >= 60 {
                    let half_int = digits_to_integer(&half_digits, base);
                    let half_val = half_int.to_string_radix(10);
                    checkpoint::save(
                        checkpoint_path,
                        &Checkpoint::Palindromic {
                            digit_count,
                            half_value: half_val.clone(),
                            min_digits: Some(min_digits),
                            max_digits: Some(max_digits),
                        },
                    )?;
                    eprintln!(
                        "Checkpoint saved at {} digits, half={} (filtered: {})",
                        digit_count, half_val, total_filtered
                    );
                    last_checkpoint = Instant::now();
                }

                if worker_client.is_some_and(|wc| wc.is_stop_requested()) {
                    let half_int = digits_to_integer(&half_digits, base);
                    let half_val = half_int.to_string_radix(10);
                    checkpoint::save(
                        checkpoint_path,
                        &Checkpoint::Palindromic {
                            digit_count,
                            half_value: half_val.clone(),
                            min_digits: Some(min_digits),
                            max_digits: Some(max_digits),
                        },
                    )?;
                    eprintln!(
                        "Stop requested by coordinator, checkpoint saved at {} digits, half={}",
                        digit_count, half_val
                    );
                    return Ok(());
                }
            }
        }
    }

    if total_filtered > 0 {
        eprintln!("Digit pre-filter eliminated {} candidates.", total_filtered);
    }
    checkpoint::clear(checkpoint_path);
    Ok(())
}
