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

use tracing::info;

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
    info!(prime_count = sieve_primes.len(), sieve_limit, "near-repdigit sieve initialized");

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
            info!(digit_count = next, "resuming near-repdigit search");
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
            info!(
                digit_count,
                survivors = survivors.len(),
                candidates_checked,
                "testing near-repdigit palindromes"
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
                if let Some(pfgw_result) = pfgw::try_test(&expr, &candidate, pfgw::PfgwMode::Prp) {
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
                info!(
                    expression = %expr,
                    digits,
                    certainty = %certainty,
                    "*** PRIME FOUND ***"
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
            info!(digit_count, total_sieved, "checkpoint saved");
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
            info!(digit_count, "stop requested by coordinator, checkpoint saved");
            return Ok(());
        }

        digit_count += 2; // Only odd digit counts
    }

    if total_sieved > 0 {
        info!(total_sieved, "near-repdigit sieve eliminated candidates");
    }
    checkpoint::clear(checkpoint_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Tests for the near-repdigit palindromic prime search module.
    //!
    //! ## Mathematical Form
    //!
    //! Near-repdigit palindromic primes are palindromes differing from a repdigit
    //! (all-9s number) at exactly two symmetric positions. Their algebraic form is:
    //!
    //!   N = 10^(2k+1) - 1 - d*(10^(k+m) + 10^(k-m))
    //!
    //! where k determines the digit count (2k+1 digits), d in {1..8} is the digit
    //! deficiency from 9, and m in {0..k} is the position offset from center.
    //! When m=0, the two positions overlap at the center digit.
    //!
    //! ## Key References
    //!
    //! - Harvey Dubner, "Repunit and Near-Repdigit Primes", Journal of Recreational
    //!   Mathematics, 1993.
    //! - BLS: Brillhart, Lehmer, Selfridge, "New Primality Criteria and Factorizations
    //!   of 2^m +/- 1", Mathematics of Computation, 29(130), 1975.
    //! - N+1 has 10^(k-m) = 2^(k-m) * 5^(k-m) as a factor, enabling BLS proofs
    //!   when this provides enough known factorization (> N^(1/3) bits).
    //!
    //! ## Testing Strategy
    //!
    //! 1. **Candidate construction**: Verify `build_candidate` produces correct
    //!    values and palindromic digit patterns.
    //! 2. **Modular sieve**: Cross-validate `candidate_mod_p` against direct
    //!    big-integer modular arithmetic.
    //! 3. **Known primes/composites**: Confirm correctness against OEIS data.
    //! 4. **Edge cases**: Parameter validation, boundary positions (m=0, m=k).

    use super::*;
    use rug::ops::RemRounding;

    // ── Known Primes ────────────────────────────────────────────────────
    //
    // Known near-repdigit palindromic primes used across multiple tests:
    //   k=1, d=4, m=0: 10^3 - 1 - 8*10^1 = 919
    //   k=1, d=2, m=1: 10^3 - 1 - 2*(10^2 + 10^0) = 797
    //   k=1, d=8, m=1: 10^3 - 1 - 8*(10^2 + 10^0) = 191
    //   k=2, d=4, m=1: 10^5 - 1 - 4*(10^3 + 10^1) = 95959
    //   k=2, d=8, m=2: 10^5 - 1 - 8*(10^4 + 10^0) = 19991

    // ── Form Generation ─────────────────────────────────────────────────

    /// Verifies `build_candidate` against hand-computed known values.
    ///
    /// Each test case corresponds to a specific (k, d, m) triple and its
    /// expected decimal value. These serve as regression anchors — if the
    /// algebraic formula changes, these will catch the error immediately.
    #[test]
    fn build_candidate_known_values() {
        assert_eq!(build_candidate(1, 4, 0), Integer::from(919u32));
        assert_eq!(build_candidate(1, 2, 1), Integer::from(797u32));
        assert_eq!(build_candidate(1, 8, 1), Integer::from(191u32));
        assert_eq!(build_candidate(2, 4, 1), Integer::from(95959u32));
        assert_eq!(build_candidate(2, 8, 2), Integer::from(19991u32));
    }

    /// Property test: every valid near-repdigit candidate is a palindrome.
    ///
    /// The algebraic form N = 10^(2k+1) - 1 - d*(10^(k+m) + 10^(k-m)) is
    /// constructed by subtracting the same amount from symmetric positions
    /// in an all-9s repdigit, which is itself a palindrome. Subtracting equal
    /// values from positions (k+m) and (k-m) preserves the palindrome property.
    /// This test exhaustively checks all valid parameter combinations for k in 1..5.
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

    // ── Sieve Correctness ─────────────────────────────────────────────

    /// Cross-validates `candidate_mod_p` against direct big-integer modular arithmetic.
    ///
    /// For every valid (k, d, m) triple with k in 1..4, computes N mod p two ways:
    ///   1. Via `candidate_mod_p` (modular exponentiation on the algebraic form,
    ///      all u64 arithmetic, no big integer allocation)
    ///   2. Via `build_candidate` followed by `rem_euc` (big integer construction
    ///      then modular reduction)
    ///
    /// These must agree for all test primes {3, 7, 11, 13, 97, 1009}. Disagreement
    /// would indicate an error in the modular exponentiation path, which would
    /// cause the sieve to miss composites or (worse) eliminate primes.
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

    /// Cross-validates `sieve_filter` against direct divisibility checks.
    ///
    /// For each valid (k, d, m) triple with k in 1..3, compares the modular
    /// sieve's composite verdict against brute-force trial division of the
    /// actual big integer candidate. This is the integration test for the entire
    /// sieve pipeline: if `sieve_filter` disagrees with direct division, the
    /// modular arithmetic has a bug.
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

    // ── Edge Cases ──────────────────────────────────────────────────────

    /// Verifies parameter validation for boundary conditions.
    ///
    /// The near-repdigit form has two validity constraints:
    ///   - m > k: Invalid because 10^(k-m) would be a fractional power.
    ///   - m = 0 and 2d > 9: Invalid because the center digit (9 - 2d) would
    ///     be negative, producing a non-palindromic number.
    ///
    /// This test checks both valid and invalid parameter combinations to ensure
    /// the search never generates malformed candidates.
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

    /// Verifies human-readable expression formatting for logging and database storage.
    ///
    /// The expression format distinguishes between m=0 (single center digit change)
    /// and m>0 (two symmetric position changes) cases. These expressions are stored
    /// in the database and must be parseable for verification.
    #[test]
    fn format_expression_formatting() {
        assert_eq!(format_expression(3, 2, 0), "10^7 - 1 - 4*10^3");
        assert_eq!(format_expression(3, 5, 2), "10^7 - 1 - 5*(10^5 + 10^1)");
        assert_eq!(format_expression(1, 4, 0), "10^3 - 1 - 8*10^1");
    }

    // ── Known Primes (Dubner 1993) ─────────────────────────────────────

    /// Verifies that known near-repdigit palindromic primes pass Miller-Rabin.
    ///
    /// Tests 5 known primes covering both m=0 (center digit change) and m>0
    /// (symmetric pair change) cases. Each is verified against its expected
    /// decimal value to catch formula errors. All pass 25-round Miller-Rabin.
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

    /// Integration test: verify the full sieve + MR pipeline finds expected 3-digit primes.
    ///
    /// Runs the complete near-repdigit search pipeline (sieve filter, then MR test)
    /// for k=1 (3-digit palindromes) and verifies that the known primes 919, 797,
    /// and 191 are all discovered. This exercises the interaction between the
    /// modular sieve and the primality test, catching any systematic errors that
    /// individual unit tests might miss.
    #[test]
    fn integration_find_small_primes() {
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

    /// Verifies that known near-repdigit composites are correctly identified.
    ///
    /// Not all near-repdigit palindromes are prime. Two notable composites:
    ///   - k=1, d=1, m=0: 979 = 11 * 89 (divisible by 11 = base+1)
    ///   - k=1, d=3, m=0: 939 = 3 * 313 (divisible by 3)
    ///
    /// The 979 case is interesting because 11 = base+1, suggesting a pattern
    /// where near-repdigit palindromes with certain d values inherit the
    /// (base+1) divisibility from even-length palindromes.
    #[test]
    fn known_composites_fail_mr() {
        let n = build_candidate(1, 1, 0);
        assert_eq!(n, Integer::from(979u32));
        assert_eq!(
            n.is_probably_prime(25),
            IsPrime::No,
            "979 = 11*89 should be composite"
        );

        // k=1, d=3, m=0: 10^3 - 1 - 6*10 = 939 = 3*313
        let n2 = build_candidate(1, 3, 0);
        assert_eq!(n2, Integer::from(939u32));
        assert_eq!(
            n2.is_probably_prime(25),
            IsPrime::No,
            "939 = 3*313 should be composite"
        );
    }

    /// Property test: every near-repdigit candidate has exactly 2k+1 digits.
    ///
    /// Since N = 10^(2k+1) - 1 - d*(...) and the subtracted term is always
    /// less than 10^(2k+1) - 10^(2k) (the gap between consecutive digit counts),
    /// the result always has exactly 2k+1 digits. This invariant is crucial:
    /// if a candidate had fewer digits, its sieve_min_n guard would be wrong.
    #[test]
    fn build_candidate_digit_count_correct() {
        for k in 1..=6u64 {
            let expected_digits = 2 * k + 1;
            for d in 1..=4u32 {
                for m in 0..=k.min(3) {
                    if !is_valid_params(k, d, m) {
                        continue;
                    }
                    let n = build_candidate(k, d, m);
                    let s = n.to_string_radix(10);
                    assert_eq!(
                        s.len() as u64, expected_digits,
                        "k={}, d={}, m={}: expected {} digits, got {} (value={})",
                        k, d, m, expected_digits, s.len(), s
                    );
                }
            }
        }
    }

    /// Property test: near-repdigit candidates have at most 2 non-nine digits.
    ///
    /// The defining property of near-repdigit palindromes:
    ///   - m = 0: Only the center digit differs from 9 (digit = 9 - 2d)
    ///   - m > 0: Exactly two symmetric positions differ from 9 (digit = 9 - d)
    ///
    /// This structural invariant distinguishes near-repdigits from general
    /// palindromic primes and enables the specialized modular sieve that
    /// operates directly on the algebraic form.
    #[test]
    fn build_candidate_all_digits_are_nines_except_two() {
        for k in 1..=4u64 {
            for d in 1..=4u32 {
                for m in 0..=k {
                    if !is_valid_params(k, d, m) {
                        continue;
                    }
                    let n = build_candidate(k, d, m);
                    let s = n.to_string_radix(10);
                    let non_nine_count = s.chars().filter(|&c| c != '9').count();
                    if m == 0 {
                        // Center digit = 9 - 2d, one position differs
                        assert!(
                            non_nine_count <= 1,
                            "k={}, d={}, m=0: expected at most 1 non-nine digit, got {} (value={})",
                            k, d, non_nine_count, s
                        );
                    } else {
                        // Two symmetric positions at k+m and k-m
                        assert!(
                            non_nine_count <= 2,
                            "k={}, d={}, m={}: expected at most 2 non-nine digits, got {} (value={})",
                            k, d, m, non_nine_count, s
                        );
                    }
                }
            }
        }
    }

    /// Verifies modular arithmetic for the smallest prime p=2.
    ///
    /// All near-repdigit palindromes are odd because they consist of mostly 9s
    /// with some smaller digits. The alternating-digit-sum parity ensures the
    /// number is odd for odd digit counts. 919 mod 2 = 1 confirms this.
    /// This is an edge case because the modular exponentiation must handle
    /// the minimal modulus correctly.
    #[test]
    fn candidate_mod_p_edge_case_p_equals_2() {
        let actual = candidate_mod_p(1, 4, 0, 2);
        assert_eq!(actual, 1, "919 mod 2 should be 1");
    }

    /// Soundness check: known near-repdigit primes must survive the sieve.
    ///
    /// The sieve must never produce false positives (marking a prime as composite).
    /// This tests all 5 known small near-repdigit primes against a 10000-prime
    /// sieve. Any failure would indicate a bug in `candidate_mod_p` or the
    /// sieve's max_safe_prime guard logic.
    #[test]
    fn sieve_filter_does_not_eliminate_primes() {
        let sieve_primes = sieve::generate_primes(10_000);
        let known_primes = [
            (1u64, 4u32, 0u64), // 919
            (1, 2, 1),          // 797
            (1, 8, 1),          // 191
            (2, 4, 1),          // 95959
            (2, 8, 2),          // 19991
        ];
        for &(k, d, m) in &known_primes {
            assert!(
                !sieve_filter(k, d, m, &sieve_primes),
                "Known prime k={}, d={}, m={} should survive sieve",
                k, d, m
            );
        }
    }

    /// Verifies the m=k boundary case where changed positions are first and last digits.
    ///
    /// When m = k, the two modified positions are at indices 0 and 2k (the
    /// outermost digits). For k=2, d=1, m=2:
    ///   N = 10^5 - 1 - 1*(10^4 + 10^0) = 99999 - 10001 = 89998
    /// The first and last digits become 8 (= 9 - d), while all middle digits
    /// remain 9. This is the maximum spread for the modified positions.
    #[test]
    fn build_candidate_m_equals_k_symmetric_positions() {
        // k=2, d=1, m=2: 10^5 - 1 - 1*(10^4 + 10^0) = 99999 - 10001 = 89998
        let n = build_candidate(2, 1, 2);
        assert_eq!(n, Integer::from(89998u32));
        let s = n.to_string_radix(10);
        // First and last digits should be 8 (= 9 - d), middle digits should be 9
        assert_eq!(s.as_bytes()[0], b'8', "First digit should be 8");
        assert_eq!(s.as_bytes()[4], b'8', "Last digit should be 8");
        assert_eq!(s.as_bytes()[1], b'9', "Second digit should be 9");
        assert_eq!(s.as_bytes()[2], b'9', "Third digit should be 9");
        assert_eq!(s.as_bytes()[3], b'9', "Fourth digit should be 9");
    }

    /// Verifies 5-digit near-repdigit primes for broader coverage across digit counts.
    ///
    /// Tests k=2 cases to ensure the algebraic formula scales correctly beyond
    /// the 3-digit (k=1) base case:
    ///   - 95959 = 10^5 - 1 - 4*(10^3 + 10^1) — two inner positions changed
    ///   - 19991 = 10^5 - 1 - 8*(10^4 + 10^0) — first/last positions changed
    ///
    /// Both are prime, confirming the formula and MR testing work for larger k.
    #[test]
    fn build_candidate_5_digit_known_primes() {
        // 95959 = 10^5 - 1 - 4*(10^3 + 10^1)
        let n = build_candidate(2, 4, 1);
        assert_eq!(n, Integer::from(95959u32));
        assert_ne!(
            n.is_probably_prime(25),
            IsPrime::No,
            "95959 should be prime"
        );

        // 19991 = 10^5 - 1 - 8*(10^4 + 10^0)
        let n2 = build_candidate(2, 8, 2);
        assert_eq!(n2, Integer::from(19991u32));
        assert_ne!(
            n2.is_probably_prime(25),
            IsPrime::No,
            "19991 should be prime"
        );
    }
}
