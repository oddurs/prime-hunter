//! # Near-Repdigit — Near-Repdigit Palindromic Prime Search
//!
//! Searches for primes that are palindromes differing from a repdigit (all-9s
//! number) at exactly two symmetric positions. These have a structured algebraic
//! form that enables efficient modular sieving and BLS N+1 deterministic proofs.
//!
//! ## Candidate Form
//!
//! N = 10^(2k+1) − 1 − d·(10^(k+m) + 10^(k−m))
//!
//! where:
//! - k determines the digit count (2k+1 digits)
//! - d ∈ {1..8} is the digit deficiency from 9
//! - m ∈ {0..k} is the position offset from center
//!
//! When m = 0, the two positions overlap at the center digit, giving:
//! N = 10^(2k+1) − 1 − 2d·10^k (center digit = 9 − 2d).
//!
//! Example: k=1, d=4, m=0 → N = 999 − 80 = 919 (a palindromic prime).
//!
//! ## Algorithm
//!
//! 1. **Modular sieve** (`candidate_mod_p`): Evaluates N mod p using modular
//!    exponentiation on the algebraic form — no big integer is constructed.
//!    Each component 10^x mod p is computed via `pow_mod`.
//!
//! 2. **BLS N+1 proof** (`proof::bls_near_repdigit_proof`): N+1 contains
//!    10^(k−m) = 2^(k−m) · 5^(k−m) as a factor, providing (k−m)·log₂(10) bits
//!    of known factorization. When this exceeds N^(1/3), BLS proves primality.
//!
//! ## Complexity
//!
//! - Enumeration: O(k · d_max) = O(k · 8) candidates per digit count.
//! - Sieve per candidate: O(π(L)) modular exponentiations.
//!
//! ## References
//!
//! - Harvey Dubner, "Repunit and Near-Repdigit Primes", Journal of Recreational
//!   Mathematics, 1993.
//! - BLS: Brillhart, Lehmer, Selfridge, "New Primality Criteria and Factorizations
//!   of 2^m ± 1", Mathematics of Computation, 29(130), 1975.

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
use crate::proof;
use crate::CoordinationClient;
use crate::{mr_screened_test, sieve};

/// Check if parameters are valid for a near-repdigit candidate.
///
/// N = 10^(2k+1) - 1 - d*(10^(k+m) + 10^(k-m))
/// Invalid when m > k or (m == 0 and 2*d > 9, since center digit would go negative).
pub fn is_valid_params(k: u64, d: u32, m: u64) -> bool {
    if m > k {
        return false;
    }
    if m == 0 && 2 * d > 9 {
        return false;
    }
    true
}

/// Build a near-repdigit palindrome candidate.
///
/// N = 10^(2k+1) - 1 - d*(10^(k+m) + 10^(k-m))
/// When m=0, positions overlap at center: N = 10^(2k+1) - 1 - 2d*10^k
pub fn build_candidate(k: u64, d: u32, m: u64) -> Integer {
    let digit_count = 2 * k + 1;
    let repdigit = Integer::from(10u32).pow(crate::checked_u32(digit_count)) - 1u32;
    if m == 0 {
        repdigit - Integer::from(2 * d) * Integer::from(10u32).pow(crate::checked_u32(k))
    } else {
        repdigit
            - Integer::from(d)
                * (Integer::from(10u32).pow(crate::checked_u32(k + m))
                    + Integer::from(10u32).pow(crate::checked_u32(k - m)))
    }
}

/// Compute N mod p using modular arithmetic (no big Integer needed).
///
/// N = 10^(2k+1) - 1 - d*(10^(k+m) + 10^(k-m))
/// Each component computed via sieve::pow_mod.
pub fn candidate_mod_p(k: u64, d: u32, m: u64, p: u64) -> u64 {
    let a = sieve::pow_mod(10, 2 * k + 1, p);

    let b = if m == 0 {
        (2 * d as u64 % p) * sieve::pow_mod(10, k, p) % p
    } else {
        let high = sieve::pow_mod(10, k + m, p);
        let low = sieve::pow_mod(10, k - m, p);
        (d as u64 % p) * ((high + low) % p) % p
    };

    // (a - 1 - b) mod p, avoiding unsigned underflow
    let step1 = (a + p - 1) % p;
    (step1 + p - b) % p
}

/// Check if the candidate is divisible by any sieve prime.
/// Returns true if composite (has a small factor).
pub fn sieve_filter(k: u64, d: u32, m: u64, sieve_primes: &[u64]) -> bool {
    let digit_count = 2 * k + 1;
    // Only use primes smaller than the minimum candidate value to avoid
    // false positives on candidates that equal a sieve prime.
    let max_safe_prime = if digit_count > 18 {
        u64::MAX
    } else {
        10u64.pow((digit_count - 1) as u32)
    };

    for &p in sieve_primes {
        if p >= max_safe_prime {
            break;
        }
        if candidate_mod_p(k, d, m, p) == 0 {
            return true;
        }
    }
    false
}

/// Format the expression for display.
pub fn format_expression(k: u64, d: u32, m: u64) -> String {
    let digit_count = 2 * k + 1;
    if m == 0 {
        format!("10^{} - 1 - {}*10^{}", digit_count, 2 * d, k)
    } else {
        format!(
            "10^{} - 1 - {}*(10^{} + 10^{})",
            digit_count,
            d,
            k + m,
            k - m
        )
    }
}

pub fn search(
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
    // Resolve sieve_limit: auto-tune if 0 (base 10 near-repdigits)
    let candidate_bits = (max_digits as f64 * 10f64.log2()) as u64;
    let n_range = (max_digits.saturating_sub(min_digits)) / 2 + 1;
    let sieve_limit = sieve::resolve_sieve_limit(sieve_limit, candidate_bits, n_range);

    let sieve_primes = sieve::generate_primes(sieve_limit);
    eprintln!(
        "Near-repdigit sieve: {} primes up to {}",
        sieve_primes.len(),
        sieve_limit
    );

    // Ensure we start on an odd digit count
    let first_odd = if min_digits.is_multiple_of(2) {
        min_digits + 1
    } else {
        min_digits
    };

    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::NearRepdigit { digit_count, .. })
            if digit_count >= min_digits && digit_count <= max_digits =>
        {
            // Resume from next odd digit count after the checkpointed one
            let next = if digit_count % 2 == 0 {
                digit_count + 1
            } else {
                digit_count + 2
            };
            eprintln!("Resuming near-repdigit search from {} digits", next);
            next
        }
        _ => first_odd,
    };

    let mut last_checkpoint = Instant::now();
    let mut total_sieved: u64 = 0;

    let mut digit_count = resume_from;
    while digit_count <= max_digits {
        let k = (digit_count - 1) / 2;

        // Sieve phase: collect all surviving (d, m) pairs
        let mut survivors: Vec<(u32, u64)> = Vec::new();
        let mut candidates_checked: u64 = 0;

        for d in 1u32..=8 {
            for m in 0..=k {
                if !is_valid_params(k, d, m) {
                    continue;
                }
                candidates_checked += 1;
                if sieve_filter(k, d, m, &sieve_primes) {
                    total_sieved += 1;
                } else {
                    survivors.push((d, m));
                }
            }
        }

        if !survivors.is_empty() {
            eprintln!(
                "Testing {}-digit near-repdigit palindromes: {} survivors from {} candidates",
                digit_count,
                survivors.len(),
                candidates_checked
            );
        }

        *progress.current.lock().unwrap() = format!("{}-digit near-repdigit", digit_count);

        // MR test phase: parallel over all survivors for this digit count.
        // Try PFGW first for large candidates (50-100x faster), fall back to GMP MR.
        let found_primes: Vec<_> = survivors
            .into_par_iter()
            .filter_map(|(d, m)| {
                let candidate = build_candidate(k, d, m);
                let expr = format_expression(k, d, m);

                // Try PFGW acceleration (near-repdigit: PRP only — N-1 doesn't have a
                // trivially factored form; the BLS proof uses N+1 factorization instead)
                if let Some(pfgw_result) =
                    pfgw::try_test(&expr, &candidate, pfgw::PfgwMode::Prp)
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
                            let digits = candidate.to_string_radix(10).len() as u64;
                            return Some((expr, digits, cert));
                        }
                        pfgw::PfgwResult::Composite => return None,
                        pfgw::PfgwResult::Unavailable { .. } => {} // fall through to GMP
                    }
                }

                // Adaptive P-1 pre-filter (Stage 1 + Stage 2, auto-tuned B1/B2)
                if crate::p1::adaptive_p1_filter(&candidate) {
                    return None;
                }

                let r = mr_screened_test(&candidate, mr_rounds);
                if r != IsPrime::No {
                    let bls_ok = proof::bls_near_repdigit_proof(k, d, m, &candidate, &sieve_primes);
                    let cert = if bls_ok {
                        "deterministic"
                    } else {
                        match r {
                            IsPrime::Yes => "deterministic",
                            _ => "probabilistic",
                        }
                    };
                    let digits = candidate.to_string_radix(10).len() as u64;
                    Some((expr, digits, cert.to_string()))
                } else {
                    None
                }
            })
            .collect();

        progress
            .tested
            .fetch_add(candidates_checked, Ordering::Relaxed);

        for (expr, digits, certainty) in found_primes {
            progress.found.fetch_add(1, Ordering::Relaxed);
            if let Some(eb) = event_bus {
                eb.emit(events::Event::PrimeFound {
                    form: "near_repdigit".into(),
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
                "near_repdigit",
                &expr,
                digits,
                search_params,
                &certainty,
                None,
            )?;
            if let Some(wc) = worker_client {
                wc.report_prime("near_repdigit", &expr, digits, search_params, &certainty);
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::NearRepdigit {
                    digit_count,
                    d: 8,
                    m: k,
                    min_digits: Some(min_digits),
                    max_digits: Some(max_digits),
                },
            )?;
            eprintln!(
                "Checkpoint saved at {} digits (sieved: {})",
                digit_count, total_sieved
            );
            last_checkpoint = Instant::now();
        }

        if worker_client.is_some_and(|wc| wc.is_stop_requested()) {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::NearRepdigit {
                    digit_count,
                    d: 8,
                    m: k,
                    min_digits: Some(min_digits),
                    max_digits: Some(max_digits),
                },
            )?;
            eprintln!(
                "Stop requested by coordinator, checkpoint saved at {} digits",
                digit_count
            );
            return Ok(());
        }

        digit_count += 2; // Only odd digit counts
    }

    if total_sieved > 0 {
        eprintln!(
            "Near-repdigit sieve eliminated {} candidates.",
            total_sieved
        );
    }
    checkpoint::clear(checkpoint_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rug::ops::RemRounding;

    // Known near-repdigit palindromic primes:
    // k=1, d=4, m=0: 10^3 - 1 - 8*10 = 919
    // k=1, d=2, m=1: 10^3 - 1 - 2*(100+1) = 797
    // k=1, d=8, m=1: 10^3 - 1 - 8*(100+1) = 191
    // k=2, d=4, m=1: 10^5 - 1 - 4*(1000+10) = 95959
    // k=2, d=8, m=2: 10^5 - 1 - 8*(10000+1) = 19991

    #[test]
    fn build_candidate_known_values() {
        assert_eq!(build_candidate(1, 4, 0), Integer::from(919u32));
        assert_eq!(build_candidate(1, 2, 1), Integer::from(797u32));
        assert_eq!(build_candidate(1, 8, 1), Integer::from(191u32));
        assert_eq!(build_candidate(2, 4, 1), Integer::from(95959u32));
        assert_eq!(build_candidate(2, 8, 2), Integer::from(19991u32));
    }

    #[test]
    fn build_candidate_is_palindrome() {
        for k in 1..=5u64 {
            for d in 1..=8u32 {
                for m in 0..=k {
                    if !is_valid_params(k, d, m) {
                        continue;
                    }
                    let n = build_candidate(k, d, m);
                    let s = n.to_string_radix(10);
                    let rev: String = s.chars().rev().collect();
                    assert_eq!(
                        s, rev,
                        "k={}, d={}, m={}: {} is not a palindrome",
                        k, d, m, s
                    );
                }
            }
        }
    }

    #[test]
    fn candidate_mod_p_cross_validation() {
        for k in 1..=4u64 {
            for d in 1..=8u32 {
                for m in 0..=k {
                    if !is_valid_params(k, d, m) {
                        continue;
                    }
                    let n = build_candidate(k, d, m);
                    for &p in &[3u64, 7, 11, 13, 97, 1009] {
                        let expected = n.clone().rem_euc(&Integer::from(p)).to_u64().unwrap();
                        let actual = candidate_mod_p(k, d, m, p);
                        assert_eq!(
                            actual, expected,
                            "k={}, d={}, m={}, p={}: mod mismatch",
                            k, d, m, p
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn sieve_filter_cross_validation() {
        let sieve_primes = sieve::generate_primes(1000);
        for k in 1..=3u64 {
            let digit_count = 2 * k + 1;
            let min_candidate = Integer::from(10u32).pow((digit_count - 1) as u32);
            for d in 1..=8u32 {
                for m in 0..=k {
                    if !is_valid_params(k, d, m) {
                        continue;
                    }
                    let n = build_candidate(k, d, m);
                    // Direct divisibility check
                    let mut direct_composite = false;
                    for &p in &sieve_primes {
                        if Integer::from(p) >= min_candidate {
                            break;
                        }
                        if n.is_divisible_u(p as u32) {
                            direct_composite = true;
                            break;
                        }
                    }
                    let sieve_composite = sieve_filter(k, d, m, &sieve_primes);
                    assert_eq!(
                        sieve_composite, direct_composite,
                        "k={}, d={}, m={}: sieve={}, direct={}",
                        k, d, m, sieve_composite, direct_composite
                    );
                }
            }
        }
    }

    #[test]
    fn is_valid_params_edge_cases() {
        // m > k: invalid
        assert!(!is_valid_params(3, 1, 4));
        // m == k: valid
        assert!(is_valid_params(3, 1, 3));
        // m = 0, 2d <= 9: valid
        assert!(is_valid_params(3, 4, 0)); // 2*4=8 <= 9
                                           // m = 0, 2d > 9: invalid
        assert!(!is_valid_params(3, 5, 0)); // 2*5=10 > 9
        assert!(!is_valid_params(3, 8, 0)); // 2*8=16 > 9
                                            // Standard valid cases
        assert!(is_valid_params(5, 8, 3));
        assert!(is_valid_params(1, 1, 0));
    }

    #[test]
    fn format_expression_formatting() {
        assert_eq!(format_expression(3, 2, 0), "10^7 - 1 - 4*10^3");
        assert_eq!(format_expression(3, 5, 2), "10^7 - 1 - 5*(10^5 + 10^1)");
        assert_eq!(format_expression(1, 4, 0), "10^3 - 1 - 8*10^1");
    }

    #[test]
    fn known_primes_pass_mr() {
        let known = [
            (1u64, 4u32, 0u64, 919u64),
            (1, 2, 1, 797),
            (1, 8, 1, 191),
            (2, 4, 1, 95959),
            (2, 8, 2, 19991),
        ];
        for &(k, d, m, expected_val) in &known {
            let n = build_candidate(k, d, m);
            assert_eq!(n, Integer::from(expected_val));
            assert_ne!(
                n.is_probably_prime(25),
                rug::integer::IsPrime::No,
                "{} should be prime",
                expected_val
            );
        }
    }

    #[test]
    fn integration_find_small_primes() {
        // Verify that the sieve + MR pipeline finds expected primes for 3-digit palindromes
        let sieve_primes = sieve::generate_primes(10_000);
        let k = 1u64;
        let mut found: Vec<(u32, u64, Integer)> = Vec::new();

        for d in 1u32..=8 {
            for m in 0..=k {
                if !is_valid_params(k, d, m) {
                    continue;
                }
                if sieve_filter(k, d, m, &sieve_primes) {
                    continue;
                }
                let candidate = build_candidate(k, d, m);
                if candidate.is_probably_prime(25) != IsPrime::No {
                    found.push((d, m, candidate));
                }
            }
        }

        // Should find at least 919, 797, 191
        let values: Vec<u64> = found.iter().map(|(_, _, n)| n.to_u64().unwrap()).collect();
        assert!(values.contains(&919), "Should find 919");
        assert!(values.contains(&797), "Should find 797");
        assert!(values.contains(&191), "Should find 191");
    }
}
