//! # Palindromic — Palindromic Prime Search in Arbitrary Bases
//!
//! Searches for primes whose digit representation in a given base reads the
//! same forwards and backwards. Uses a half-digit enumeration strategy with
//! modular pre-filtering to avoid constructing most candidates as big integers.
//!
//! ## Algorithm
//!
//! 1. **Half-digit generation**: A d-digit palindrome is fully determined by its
//!    first ⌈d/2⌉ digits (the "half"). The search iterates over half-values
//!    and mirrors them to produce full palindromes.
//!
//! 2. **Even-digit skip**: Even-length palindromes in base b are always divisible
//!    by b+1 (e.g., all 4-digit base-10 palindromes are divisible by 11). The
//!    only possible even-length palindromic prime is b+1 itself (= "11" in base b).
//!
//! 3. **Leading digit filter**: The first digit of a palindrome equals its last
//!    digit. A prime > b must have its last digit coprime to b (e.g., in base 10,
//!    valid leading digits are {1, 3, 7, 9}). This eliminates (b − φ(b))/b of
//!    the search space.
//!
//! 4. **Modular digit filter** (`is_filter_composite`): Evaluates the palindrome
//!    mod each sieve prime using Horner's method on the digit array (all u64
//!    arithmetic — no big integer allocation). Eliminates ~85–95% of candidates.
//!
//! 5. **Batch parallel testing**: Survivors are collected into batches and tested
//!    in parallel via `rayon::par_iter`.
//!
//! ## Complexity
//!
//! - Enumeration: O(b^(d/2)) candidates per d-digit length.
//! - Filter per candidate: O(π(L) · d) where L is sieve limit.
//! - Primality test: O(d · M(d)) per survivor.
//!
//! ## References
//!
//! - OEIS: [A002385](https://oeis.org/A002385) — Palindromic primes in base 10.
//! - Harvey Dubner, "Palindromic Primes", Journal of Recreational Mathematics, 1989.
//! - Divisibility rule: A 2k-digit base-b palindrome has (b+1) as a factor because
//!   the alternating digit sum is always zero.

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
    let mut digits = vec![0u32; len.saturating_sub(parsed.len())];
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
                .unwrap_or_else(|_| Integer::from(base).pow((digit_count.div_ceil(2) - 1) as u32));
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

    // Resolve sieve_limit: auto-tune if 0
    let candidate_bits = (max_digits as f64 * (base as f64).log2()) as u64;
    let n_range = max_digits.saturating_sub(min_digits) + 1;
    let sieve_limit = sieve::resolve_sieve_limit(sieve_limit, candidate_bits, n_range);

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
                    let expr = candidate.to_string_radix(10);
                    let digits = expr.len() as u64;
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
                    db.insert_prime_sync(
                        rt,
                        "palindromic",
                        &expr,
                        digits,
                        search_params,
                        cert,
                        None,
                    )?;
                    if let Some(wc) = worker_client {
                        wc.report_prime("palindromic", &expr, digits, search_params, cert);
                    }
                }
            }
            continue;
        }

        let half_len = digit_count.div_ceil(2) as usize;
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

                // Only candidates surviving the digit filter need primality testing.
                // Try PFGW first for large candidates (50-100x faster), fall back to GMP MR.
                let found_primes: Vec<_> = batch
                    .into_par_iter()
                    .filter_map(|num| {
                        // Try PFGW acceleration (palindromic: PRP via decimal string).
                        // pfgw::is_available() is a cheap check; only compute the
                        // expensive to_string_radix(10) when PFGW will actually run.
                        if pfgw::is_available(digit_count) {
                            let decimal = num.to_string_radix(10);
                            match pfgw::try_test(&decimal, &num, pfgw::PfgwMode::Prp) {
                                Some(pfgw::PfgwResult::Prime {
                                    is_deterministic, ..
                                }) => {
                                    let cert = if is_deterministic {
                                        "deterministic"
                                    } else {
                                        "probabilistic"
                                    };
                                    return Some((decimal, digit_count, cert.to_string()));
                                }
                                Some(pfgw::PfgwResult::Composite) => return None,
                                _ => {} // Unavailable or not configured — fall through to GMP
                            }
                        }

                        // Adaptive P-1 pre-filter (Stage 1 + Stage 2, auto-tuned B1/B2)
                        if crate::p1::adaptive_p1_filter(&num) {
                            return None;
                        }

                        // GMP Miller-Rabin fallback — defer to_string_radix until prime is found
                        let r = mr_screened_test(&num, mr_rounds);
                        if r != IsPrime::No {
                            let cert = match r {
                                IsPrime::Yes => "deterministic",
                                _ => "probabilistic",
                            };
                            let decimal = num.to_string_radix(10);
                            Some((decimal, digit_count, cert.to_string()))
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
                        None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_to_palindrome_odd_length() {
        // [1, 2, 3] with is_odd=true → [1, 2, 3, 2, 1] (5 digits)
        let half = vec![1, 2, 3];
        let full = mirror_to_palindrome(&half, true);
        assert_eq!(full, vec![1, 2, 3, 2, 1]);
    }

    #[test]
    fn mirror_to_palindrome_even_length() {
        // [1, 2] with is_odd=false → [1, 2, 2, 1] (4 digits)
        let half = vec![1, 2];
        let full = mirror_to_palindrome(&half, false);
        assert_eq!(full, vec![1, 2, 2, 1]);
    }

    #[test]
    fn mirror_to_palindrome_single_digit() {
        // [5] with is_odd=true → [5] (1 digit)
        let half = vec![5];
        let full = mirror_to_palindrome(&half, true);
        assert_eq!(full, vec![5]);
    }

    #[test]
    fn generated_palindromes_are_actually_palindromic() {
        // Generate palindromes in base 10 and verify they read the same forwards and backwards
        for num_digits in [3, 5, 7] {
            let half_len = (num_digits + 1) / 2;
            let mut half_digits = vec![0u32; half_len];
            half_digits[0] = 1; // leading digit

            for _ in 0..100 {
                let full = mirror_to_palindrome(&half_digits, true);
                assert_eq!(full.len(), num_digits);

                // Verify palindrome property
                let reversed: Vec<u32> = full.iter().rev().cloned().collect();
                assert_eq!(full, reversed, "Not a palindrome: {:?}", full);

                if increment_digits(&mut half_digits, 10) {
                    break;
                }
            }
        }
    }

    #[test]
    fn increment_digits_basic() {
        let mut digits = vec![1, 2, 3];
        let overflow = increment_digits(&mut digits, 10);
        assert!(!overflow);
        assert_eq!(digits, vec![1, 2, 4]);
    }

    #[test]
    fn increment_digits_carry() {
        let mut digits = vec![1, 2, 9];
        let overflow = increment_digits(&mut digits, 10);
        assert!(!overflow);
        assert_eq!(digits, vec![1, 3, 0]);
    }

    #[test]
    fn increment_digits_multi_carry() {
        let mut digits = vec![1, 9, 9];
        let overflow = increment_digits(&mut digits, 10);
        assert!(!overflow);
        assert_eq!(digits, vec![2, 0, 0]);
    }

    #[test]
    fn increment_digits_overflow() {
        let mut digits = vec![9, 9, 9];
        let overflow = increment_digits(&mut digits, 10);
        assert!(overflow);
        assert_eq!(digits, vec![0, 0, 0]);
    }

    #[test]
    fn increment_digits_base_3() {
        let mut digits = vec![1, 2];
        let overflow = increment_digits(&mut digits, 3);
        assert!(!overflow);
        assert_eq!(digits, vec![2, 0]); // 12 + 1 = 20 in base 3
    }

    #[test]
    fn digits_to_integer_and_back() {
        let digits = vec![1, 2, 3];
        let n = digits_to_integer(&digits, 10);
        assert_eq!(n, Integer::from(123));

        let back = integer_to_digits(&n, 3, 10);
        assert_eq!(back, digits);
    }

    #[test]
    fn digits_to_integer_base_2() {
        let digits = vec![1, 0, 1, 1]; // 1011 in base 2 = 11
        let n = digits_to_integer(&digits, 2);
        assert_eq!(n, Integer::from(11));
    }

    #[test]
    fn integer_to_digits_with_padding() {
        let n = Integer::from(42);
        let digits = integer_to_digits(&n, 5, 10);
        assert_eq!(digits, vec![0, 0, 0, 4, 2]); // zero-padded to 5
    }

    #[test]
    fn digits_mod_correct() {
        // 12321 mod 7 = 12321 % 7 = 1760*7 + 1 = 1
        let digits = vec![1, 2, 3, 2, 1];
        assert_eq!(digits_mod(&digits, 10, 7), 12321 % 7);

        // 999 mod 37 = 999 % 37 = 27*37 = 999, so mod = 0
        let digits2 = vec![9, 9, 9];
        assert_eq!(digits_mod(&digits2, 10, 37), 999 % 37);
    }

    #[test]
    fn is_filter_composite_catches_divisible() {
        // 12321 = 3 * 4107 = 3 * 3 * 1369 = 9 * 1369
        let digits = vec![1, 2, 3, 2, 1];
        let primes = vec![3, 7, 11, 13];
        assert!(is_filter_composite(&digits, 10, &primes, 5));
    }

    #[test]
    fn is_filter_composite_passes_prime() {
        // 10301 is prime
        let digits = vec![1, 0, 3, 0, 1];
        let primes: Vec<u64> = sieve::generate_primes(1000)
            .into_iter()
            .filter(|&p| p >= 3)
            .collect();
        assert!(!is_filter_composite(&digits, 10, &primes, 5));
    }

    #[test]
    fn even_digit_palindromes_divisible_by_base_plus_one() {
        // All even-digit palindromes in base 10 are divisible by 11
        // e.g., 1221, 3443, 9009, etc.
        for lead in 1..10u32 {
            for inner in 0..10u32 {
                let palindrome = lead * 1000 + inner * 100 + inner * 10 + lead;
                assert_eq!(
                    palindrome % 11,
                    0,
                    "4-digit palindrome {} should be divisible by 11",
                    palindrome
                );
            }
        }
    }

    #[test]
    fn palindrome_batch_count() {
        // For 3-digit base-10 palindromes with leading digit 1:
        // half goes from 10 to 19 (10 values), each generating 101..191
        let mut half_digits = vec![1u32, 0];
        let end_digits = vec![1u32, 9];
        let mut count = 0;
        loop {
            count += 1;
            let full = mirror_to_palindrome(&half_digits, true);
            assert_eq!(full.len(), 3);
            assert_eq!(full[0], full[2]); // palindrome check
            if half_digits >= end_digits {
                break;
            }
            increment_digits(&mut half_digits, 10);
        }
        assert_eq!(count, 10, "Should be 10 palindromes for leading digit 1");
    }
}
