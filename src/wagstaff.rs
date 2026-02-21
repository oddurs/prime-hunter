//! # Wagstaff — (2^p + 1)/3 Prime Search
//!
//! Searches for Wagstaff primes: numbers of the form (2^p + 1)/3 where p is
//! an odd prime. These are related to Mersenne primes but much rarer — only
//! ~30 are known (as of 2025).
//!
//! ## Algebraic Background
//!
//! For odd p, 2^p + 1 ≡ 0 (mod 3) because 2 ≡ −1 (mod 3), so 2^p ≡ −1 (mod 3)
//! for odd p. The quotient (2^p + 1)/3 is an integer. If it is prime, p must
//! itself be prime (since 2^(ab) + 1 has algebraic factors).
//!
//! ## Algorithm
//!
//! 1. **Multiplicative order sieve** (`WagstaffSieve`): For each sieve prime q > 3,
//!    computes ord_q(2). If ord ≡ 2 (mod 4), then 2^(ord/2) ≡ −1 (mod q), so
//!    (2^p + 1)/3 is divisible by q whenever p ≡ ord/2 (mod ord). Entries with
//!    ord ≡ 0 (mod 4) are excluded because their half-order is even and can never
//!    match an odd prime exponent. Entries are deduplicated and sorted by order.
//!
//! 2. **No deterministic proof exists**: Unlike Mersenne or Proth primes, there
//!    is no known efficient deterministic test for Wagstaff primes. All results
//!    are probabilistic (PRP). The Vrba-Reix test (via GWNUM) provides a fast
//!    PRP test specific to this form.
//!
//! ## Complexity
//!
//! - Sieve construction: O(π(L)² ) due to multiplicative order computation.
//! - Sieve per candidate: O(S) where S is the number of sieve entries.
//! - PRP test: O(p · M(p)) via GMP modular exponentiation.
//!
//! ## References
//!
//! - OEIS: [A000978](https://oeis.org/A000978) — Wagstaff prime exponents.
//! - Samuel S. Wagstaff Jr., "Divisors of Mersenne Numbers", Mathematics of
//!   Computation, 40(161), 1983.
//! - Tony Forbes, "A Search for Wagstaff Primes", 2011.

use anyhow::Result;
use rayon::prelude::*;
use rug::integer::IsPrime;
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
use crate::CoordinationClient;
use crate::{exact_digits, mr_screened_test, sieve};

/// Precomputed sieve data for Wagstaff composites.
///
/// For sieve prime q > 3 with ord_q(2) ≡ 2 (mod 4):
///   (2^p + 1)/3 is divisible by q when p ≡ ord/2 (mod ord).
///
/// Only entries where ord ≡ 2 (mod 4) are kept because ord ≡ 0 (mod 4)
/// yields an even half_ord, and no odd prime p can satisfy p ≡ even (mod ord).
struct WagstaffSieve {
    entries: Vec<(u64, u64)>,
}

impl WagstaffSieve {
    fn new(sieve_primes: &[u64]) -> Self {
        let mut entries: Vec<(u64, u64)> = sieve_primes
            .par_iter()
            .filter(|&&q| q > 3)
            .filter_map(|&q| {
                let ord = sieve::multiplicative_order(2, q);
                if ord % 4 == 2 {
                    Some((ord, ord / 2))
                } else {
                    None
                }
            })
            .collect();
        // Sort by ord so small orders (more eliminating power) are checked first.
        entries.sort_unstable_by_key(|&(ord, _)| ord);
        // Deduplicate: multiple primes can share the same (ord, half) pair.
        entries.dedup();
        WagstaffSieve { entries }
    }

    /// Check if (2^p + 1)/3 has a small factor from the sieve.
    fn is_composite(&self, p: u64) -> bool {
        self.entries.iter().any(|&(ord, half)| p % ord == half)
    }
}

/// Adaptive block size: fewer candidates per block for larger exponents.
fn block_size_for_exp(exp: u64) -> usize {
    match exp {
        0..=10_000 => 500,
        10_001..=100_000 => 100,
        100_001..=1_000_000 => 20,
        _ => 5,
    }
}

pub fn search(
    min_exp: u64,
    max_exp: u64,
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
    // Generate prime exponents in range (p must be odd prime >= 3)
    let all_primes = sieve::generate_primes(max_exp);
    let candidate_exponents: Vec<u64> = all_primes
        .iter()
        .copied()
        .filter(|&p| p >= min_exp.max(3))
        .collect();

    if candidate_exponents.is_empty() {
        info!(min_exp, max_exp, "no prime exponents in range");
        return Ok(());
    }

    info!(
        count = candidate_exponents.len(),
        first = candidate_exponents[0],
        last = candidate_exponents.last().unwrap(),
        "testing prime exponents for Wagstaff primes (2^p+1)/3"
    );

    // Resolve sieve_limit: auto-tune if 0
    // Wagstaff: (2^p+1)/3 has ~max_exp bits
    let candidate_bits = max_exp;
    let n_range = candidate_exponents.len() as u64;
    let sieve_limit = sieve::resolve_sieve_limit(sieve_limit, candidate_bits, n_range);

    // Build modular sieve
    let sieve_primes = sieve::generate_primes(sieve_limit);
    info!(prime_count = sieve_primes.len(), "computing multiplicative orders for sieve primes");
    let wsieve = WagstaffSieve::new(&sieve_primes);
    info!(active_entries = wsieve.entries.len(), "Wagstaff sieve ready");

    // Minimum exponent where (2^p+1)/3 > sieve_limit, making sieve safe
    let sieve_min_exp = ((sieve_limit as f64 * 3.0).log2().ceil()) as u64;
    info!(sieve_min_exp, "sieve active");

    // Load checkpoint
    let resume_exp = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::Wagstaff { last_exp, .. })
            if last_exp >= min_exp && last_exp < max_exp =>
        {
            info!(last_exp, "resuming Wagstaff search");
            last_exp
        }
        _ => 0,
    };

    let candidates: Vec<u64> = candidate_exponents
        .iter()
        .copied()
        .filter(|&p| p > resume_exp)
        .collect();

    if candidates.is_empty() {
        info!("all candidates already processed");
        checkpoint::clear(checkpoint_path);
        return Ok(());
    }

    let mut last_checkpoint = Instant::now();
    let mut sieved_out: u64 = 0;
    let mut pos = 0;

    while pos < candidates.len() {
        let bsize = block_size_for_exp(candidates[pos]);
        let block_end = (pos + bsize).min(candidates.len());
        let block = &candidates[pos..block_end];
        let block_min = block[0];
        let block_max = *block.last().unwrap();

        *progress.current.lock().unwrap() = format!("(2^p+1)/3 p=[{}..{}]", block_min, block_max);

        // Apply sieve filter (parallelized across candidates)
        let survivors: Vec<u64> = block
            .par_iter()
            .copied()
            .filter(|&p| p < sieve_min_exp || !wsieve.is_composite(p))
            .collect();

        sieved_out += (block.len() - survivors.len()) as u64;

        // Test survivors: try PFGW first (50-100x faster), fall back to GMP MR.
        // Note: PRST does not support Wagstaff form — PRST requires k*b^n±1 with integer k,
        // but (2^p+1)/3 does not map to this form. PFGW is the correct accelerator here.
        // Future: GWNUM direct FFI with Vrba-Reix test (Phase 3) will be even faster.
        let found_primes: Vec<_> = survivors
            .into_par_iter()
            .filter_map(|p| {
                let two_p_plus_1 = (Integer::from(1u32) << crate::checked_u32(p)) + 1u32;
                debug_assert!(
                    two_p_plus_1.is_divisible_u(3),
                    "2^{} + 1 must be divisible by 3 for odd prime p",
                    p
                );
                let candidate = two_p_plus_1 / 3u32;

                // Try GWNUM Vrba-Reix test (when --features gwnum is enabled)
                #[cfg(feature = "gwnum")]
                {
                    let digits = crate::estimate_digits(&candidate);
                    if digits >= 10_000 {
                        match crate::gwnum::vrba_reix_test(p) {
                            Ok(true) => {
                                let digits = exact_digits(&candidate);
                                return Some((p, digits, "probabilistic (Vrba-Reix)".to_string()));
                            }
                            Ok(false) => return None,
                            Err(_) => {} // fall through to PFGW
                        }
                    }
                }

                // Try PFGW acceleration (Wagstaff: PRP only, no deterministic test exists)
                if let Some(pfgw_result) =
                    pfgw::try_test(&format!("(2^{}+1)/3", p), &candidate, pfgw::PfgwMode::Prp)
                {
                    match pfgw_result {
                        pfgw::PfgwResult::Prime {
                            method,
                            is_deterministic,
                        } => {
                            let digits = exact_digits(&candidate);
                            let certainty = if is_deterministic {
                                format!("deterministic ({})", method)
                            } else {
                                "probabilistic".to_string()
                            };
                            return Some((p, digits, certainty));
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
                    let digits = exact_digits(&candidate);
                    let certainty = match r {
                        IsPrime::Yes => "deterministic",
                        IsPrime::Probably => "probabilistic",
                        IsPrime::No => unreachable!(),
                    };
                    Some((p, digits, certainty.to_string()))
                } else {
                    None
                }
            })
            .collect();

        progress
            .tested
            .fetch_add(block.len() as u64, Ordering::Relaxed);

        for (p, digits, certainty) in found_primes {
            let expr = format!("(2^{}+1)/3", p);
            progress.found.fetch_add(1, Ordering::Relaxed);
            if let Some(eb) = event_bus {
                eb.emit(events::Event::PrimeFound {
                    form: "wagstaff".into(),
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
                "wagstaff",
                &expr,
                digits,
                search_params,
                &certainty,
                None,
            )?;
            if let Some(wc) = worker_client {
                wc.report_prime("wagstaff", &expr, digits, search_params, &certainty);
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Wagstaff {
                    last_exp: block_max,
                    min_exp: Some(min_exp),
                    max_exp: Some(max_exp),
                },
            )?;
            info!(p = block_max, sieved_out, "checkpoint saved");
            last_checkpoint = Instant::now();
        }

        if worker_client.is_some_and(|wc| wc.is_stop_requested()) {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Wagstaff {
                    last_exp: block_max,
                    min_exp: Some(min_exp),
                    max_exp: Some(max_exp),
                },
            )?;
            info!(p = block_max, "stop requested by coordinator, checkpoint saved");
            return Ok(());
        }

        pos = block_end;
    }

    checkpoint::clear(checkpoint_path);
    info!(sieved_out, total = candidates.len(), "Wagstaff sieve eliminated candidates");
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Tests for the Wagstaff prime search module ((2^p + 1)/3).
    //!
    //! ## Mathematical Form
    //!
    //! Wagstaff primes are primes of the form W(p) = (2^p + 1)/3 where p is an
    //! odd prime. The division by 3 is exact because for odd p, 2^p = -1 (mod 3),
    //! so 2^p + 1 = 0 (mod 3).
    //!
    //! Known Wagstaff prime exponents:
    //! p in {3, 5, 7, 11, 13, 17, 19, 23, 31, 43, 61, 79, 101, 127, 167, 191, 199, ...}
    //! (OEIS [A000978](https://oeis.org/A000978))
    //!
    //! ## Key References
    //!
    //! - Samuel S. Wagstaff Jr., "Divisors of Mersenne Numbers", Mathematics of
    //!   Computation, 40(161), 1983.
    //! - Tony Forbes, "A Search for Wagstaff Primes", 2011.
    //! - Unlike Mersenne primes, there is NO known deterministic test for Wagstaff
    //!   primes. All results are probabilistic (PRP). The Vrba-Reix test (via
    //!   GWNUM) provides a specialized PRP test.
    //!
    //! ## Testing Strategy
    //!
    //! 1. **Known primes**: Verify known Wagstaff primes pass MR.
    //! 2. **Known composites**: Verify non-Wagstaff exponents produce composites.
    //! 3. **Sieve correctness**: Verify the multiplicative-order sieve eliminates
    //!    composites without affecting known primes.
    //! 4. **Sieve theory**: Validate the ord % 4 == 2 filter and sorted/deduped entries.
    //! 5. **No deterministic proof**: Confirm GMP never returns IsPrime::Yes for
    //!    large Wagstaff primes.
    //! 6. **Edge cases**: Smallest Wagstaff prime, adaptive block sizing, formula values.

    use super::*;
    use crate::sieve;

    /// Helper: compute the Wagstaff number W(p) = (2^p + 1) / 3.
    fn wagstaff(p: u64) -> Integer {
        ((Integer::from(1u32) << crate::checked_u32(p)) + 1u32) / 3u32
    }

    // ── Known Primes (OEIS A000978) ─────────────────────────────────────

    /// Verifies that known Wagstaff primes pass Miller-Rabin.
    ///
    /// OEIS A000978 lists the prime exponents p where (2^p+1)/3 is prime.
    /// Tests the first 10 known exponents: 3, 5, 7, 11, 13, 17, 19, 23, 31, 43.
    /// The Wagstaff number at p=43 has 13 digits, well within GMP's fast range.
    #[test]
    fn known_wagstaff_primes() {
        for &p in &[3u64, 5, 7, 11, 13, 17, 19, 23, 31, 43] {
            let w = wagstaff(p);
            assert_ne!(
                w.is_probably_prime(25),
                IsPrime::No,
                "(2^{}+1)/3 = {} should be prime",
                p,
                w
            );
        }
    }

    /// Verifies that non-Wagstaff prime exponents produce composite values.
    ///
    /// All prime exponents not in OEIS A000978 should give composite W(p).
    /// Tests 12 composite cases: p in {29, 37, 41, 47, 53, 59, 67, 71, 73, 83, 89, 97}.
    /// The gap between consecutive Wagstaff prime exponents grows rapidly —
    /// after p=23, the next is p=31, then p=43, highlighting the rarity of
    /// these primes.
    #[test]
    fn known_wagstaff_composites() {
        for &p in &[29u64, 37, 41, 47, 53, 59, 67, 71, 73, 83, 89, 97] {
            let w = wagstaff(p);
            assert_eq!(
                w.is_probably_prime(25),
                IsPrime::No,
                "(2^{}+1)/3 should be composite",
                p
            );
        }
    }

    // ── Algebraic Properties ──────────────────────────────────────────

    /// Verifies the divisibility-by-3 requirement: 2^p + 1 is divisible by 3
    /// only for odd exponents p.
    ///
    /// Since 2 = -1 (mod 3):
    ///   - 2^(even) = 1 (mod 3), so 2^(even) + 1 = 2 (mod 3) — NOT divisible by 3
    ///   - 2^(odd) = -1 (mod 3), so 2^(odd) + 1 = 0 (mod 3) — divisible by 3
    ///
    /// This is why the search restricts to odd prime exponents (p >= 3).
    /// Even exponents would make (2^p + 1)/3 non-integer.
    #[test]
    fn wagstaff_requires_odd_exponent() {
        let val = (Integer::from(1u32) << 2u32) + 1u32;
        assert!(!val.is_divisible_u(3));
        let val = (Integer::from(1u32) << 4u32) + 1u32;
        assert!(!val.is_divisible_u(3));

        // Odd exponents: 2^p + 1 is always divisible by 3
        let val = (Integer::from(1u32) << 3u32) + 1u32;
        assert!(val.is_divisible_u(3));
        let val = (Integer::from(1u32) << 5u32) + 1u32;
        assert!(val.is_divisible_u(3));
    }

    // ── Sieve Correctness ─────────────────────────────────────────────

    /// Verifies the multiplicative-order sieve never produces false positives.
    ///
    /// For each odd prime p >= sieve_min_exp where the sieve declares W(p)
    /// composite, we verify via Miller-Rabin that it is actually composite.
    /// Tests all primes up to 500 against a 100000-prime sieve.
    ///
    /// The sieve works by exploiting the multiplicative order: for sieve prime q,
    /// if ord_q(2) = 2 (mod 4), then 2^(ord/2) = -1 (mod q), so q divides
    /// 2^p + 1 when p = ord/2 (mod ord).
    #[test]
    fn sieve_correctly_eliminates() {
        let sieve_primes = sieve::generate_primes(100_000);
        let wsieve = WagstaffSieve::new(&sieve_primes);
        let sieve_min_exp = ((100_000f64 * 3.0).log2().ceil()) as u64;

        let test_primes = sieve::generate_primes(500);
        for &p in &test_primes {
            if p < 3 || p < sieve_min_exp {
                continue;
            }
            if wsieve.is_composite(p) {
                let w = wagstaff(p);
                assert_eq!(
                    w.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said (2^{}+1)/3 composite but MR says prime",
                    p
                );
            }
        }
    }

    /// Soundness check: known Wagstaff prime exponents must survive the sieve.
    ///
    /// Tests exponents {61, 79, 101, 127, 167, 191, 199} — the larger known
    /// Wagstaff prime exponents that exceed sieve_min_exp. If the sieve
    /// eliminates any of these, the multiplicative-order computation or the
    /// ord % 4 filter has a bug.
    #[test]
    fn sieve_preserves_known_primes() {
        let sieve_primes = sieve::generate_primes(100_000);
        let wsieve = WagstaffSieve::new(&sieve_primes);
        let sieve_min_exp = ((100_000f64 * 3.0).log2().ceil()) as u64;

        for &p in &[61u64, 79, 101, 127, 167, 191, 199] {
            if p >= sieve_min_exp {
                assert!(
                    !wsieve.is_composite(p),
                    "Sieve incorrectly eliminated known Wagstaff prime p={}",
                    p
                );
            }
        }
    }

    // ── Sieve Theory Verification ──────────────────────────────────────

    /// Verifies the sieve's multiplicative-order filter conditions.
    ///
    /// The sieve includes a (q, ord, half) entry only when ord_q(2) = 2 (mod 4):
    ///
    ///   - ord_11(2) = 10, 10 % 4 = 2: INCLUDED, half = 5 (odd).
    ///     Verification: 2^5 = 32 = 10 = -1 (mod 11), so 11 | (2^p + 1) when p = 5 (mod 10).
    ///
    ///   - ord_5(2) = 4, 4 % 4 = 0: EXCLUDED. half = 2 (even).
    ///     An even half can never match an odd prime p, so including it would
    ///     waste computation without eliminating any candidate.
    ///
    ///   - ord_23(2) = 11 (odd): EXCLUDED.
    ///     When ord is odd, 2^(ord/2) is not an integer power, so 2^p can never
    ///     be -1 (mod q). The sieve cannot use this entry.
    #[test]
    fn multiplicative_order_sieve_condition() {
        assert_eq!(sieve::multiplicative_order(2, 11), 10);
        // 2^5 ≡ 32 ≡ 10 ≡ -1 (mod 11)
        assert_eq!(sieve::pow_mod(2, 5, 11), 10);

        // ord_5(2) = 4, 4 % 4 == 0 → excluded (half=2 is even, never matches odd prime)
        assert_eq!(sieve::multiplicative_order(2, 5), 4);

        // ord_23(2) = 11, odd → excluded (2^p can never be -1 mod 23)
        assert_eq!(sieve::multiplicative_order(2, 23), 11);
    }

    // ── No Deterministic Proof ─────────────────────────────────────────

    /// Verifies that GMP never returns IsPrime::Yes for large Wagstaff primes.
    ///
    /// Unlike Mersenne primes (Lucas-Lehmer), Proth primes (Proth's theorem),
    /// or factorial primes (Pocklington/Morrison), there is NO known polynomial-
    /// time deterministic primality test specific to Wagstaff numbers. All
    /// results must be probabilistic (PRP).
    ///
    /// GMP's `is_probably_prime(25)` returns `IsPrime::Yes` for very small
    /// numbers (< 2^64) that it can prove deterministically via trial division
    /// or BPSW. For p >= 67, the Wagstaff number exceeds GMP's exact-proof
    /// range, so the result must be `IsPrime::Probably`, never `IsPrime::Yes`.
    ///
    /// This is a fundamental limitation documented in GOTCHAS.md: "Wagstaff:
    /// no deterministic proof exists — results always PRP."
    #[test]
    fn wagstaff_never_deterministic() {
        for &p in &[5u64, 7, 11, 13, 17, 19, 23, 31, 43, 61, 79, 101, 127] {
            let w = wagstaff(p);
            let result = w.is_probably_prime(25);
            assert_ne!(result, IsPrime::No, "(2^{}+1)/3 should pass MR", p);
            // For large enough Wagstaff numbers, GMP can't prove primality deterministically.
            // GMP returns IsPrime::Yes for very small numbers (< 2^64), so only check p >= 67.
            if p >= 67 {
                assert_eq!(
                    result,
                    IsPrime::Probably,
                    "(2^{}+1)/3 should be Probably, not Yes — no deterministic proof exists",
                    p
                );
            }
        }
    }

    /// Verifies that all sieve entries have ord % 4 == 2 (thus odd half-order).
    ///
    /// The optimization: entries with even half_ord (from ord % 4 == 0) can
    /// never match an odd prime p, so they are excluded during sieve construction.
    /// This reduces the sieve size by roughly 50% without losing any elimination
    /// power. Every entry must satisfy: ord % 4 == 2 and half % 2 == 1.
    #[test]
    fn sieve_optimization_only_odd_half() {
        let sieve_primes = sieve::generate_primes(1000);
        let wsieve = WagstaffSieve::new(&sieve_primes);

        for &(ord, half) in &wsieve.entries {
            assert_eq!(
                ord % 4,
                2,
                "Entry with ord={} should not be in sieve (ord%4={})",
                ord,
                ord % 4
            );
            assert_eq!(half % 2, 1, "Entry with half={} should be odd", half);
        }
    }

    // ── Form Generation ─────────────────────────────────────────────────

    /// Verifies the Wagstaff formula W(p) = (2^p + 1)/3 for small exponents.
    ///
    /// Cross-checks computed values against known decimal equivalents:
    ///   - W(3) = (8+1)/3 = 3
    ///   - W(5) = (32+1)/3 = 11
    ///   - W(7) = (128+1)/3 = 43
    ///   - W(11) = (2048+1)/3 = 683
    ///   - W(13) = (8192+1)/3 = 2731
    ///
    /// These are all prime (OEIS A000978).
    #[test]
    fn wagstaff_values_correct() {
        assert_eq!(wagstaff(3), Integer::from(3u32)); // (8+1)/3 = 3
        assert_eq!(wagstaff(5), Integer::from(11u32)); // (32+1)/3 = 11
        assert_eq!(wagstaff(7), Integer::from(43u32)); // (128+1)/3 = 43
        assert_eq!(wagstaff(11), Integer::from(683u32)); // (2048+1)/3 = 683
        assert_eq!(wagstaff(13), Integer::from(2731u32)); // (8192+1)/3 = 2731
    }

    // ── Edge Cases ──────────────────────────────────────────────────────

    /// Verifies the smallest Wagstaff prime: W(3) = (8+1)/3 = 3.
    ///
    /// p=3 is the smallest valid exponent (must be an odd prime >= 3).
    /// W(3) = 3 is itself a prime. Curiously, this means 3 is a "self-referential"
    /// Wagstaff prime — the exponent and the result are the same number.
    #[test]
    fn wagstaff_p3_is_smallest() {
        let w = wagstaff(3);
        assert_eq!(w, 3);
        assert_ne!(w.is_probably_prime(25), IsPrime::No, "(2^3+1)/3 = 3 is prime");
    }

    /// Verifies adaptive block sizing: larger exponents get smaller blocks.
    ///
    /// The block size decreases as the exponent grows because larger Wagstaff
    /// numbers take exponentially longer to test. The schedule:
    ///   - exp <= 10000: 500 candidates per block
    ///   - 10001-100000: 100 per block
    ///   - 100001-1000000: 20 per block
    ///   - > 1000000: 5 per block
    ///
    /// This ensures checkpoints and progress updates happen at reasonable
    /// intervals regardless of candidate size.
    #[test]
    fn block_size_for_exp_decreases_with_size() {
        let small = block_size_for_exp(100);
        let medium = block_size_for_exp(50_000);
        let large = block_size_for_exp(500_000);
        let huge = block_size_for_exp(5_000_000);

        assert!(
            small >= medium,
            "Block size for small exp ({}) should be >= medium ({})",
            small, medium
        );
        assert!(
            medium >= large,
            "Block size for medium exp ({}) should be >= large ({})",
            medium, large
        );
        assert!(
            large >= huge,
            "Block size for large exp ({}) should be >= huge ({})",
            large, huge
        );
    }

    /// Verifies that sieve entries are sorted by ord and deduplicated.
    ///
    /// Sorting by ord ensures small orders (with more eliminating power) are
    /// checked first, enabling early exit in `is_composite`. Deduplication is
    /// necessary because multiple sieve primes q can share the same (ord, half)
    /// pair — e.g., if q1 and q2 both have ord_q(2) = 10, they produce identical
    /// sieve entries and only one needs to be kept.
    #[test]
    fn sieve_entries_sorted_and_deduped() {
        let sieve_primes = sieve::generate_primes(10_000);
        let wsieve = WagstaffSieve::new(&sieve_primes);

        // Check sorted by ord
        for window in wsieve.entries.windows(2) {
            assert!(
                window[0].0 <= window[1].0,
                "Sieve entries not sorted: ({}, {}) before ({}, {})",
                window[0].0, window[0].1, window[1].0, window[1].1
            );
        }

        // Check deduped (no two consecutive identical entries)
        for window in wsieve.entries.windows(2) {
            assert_ne!(
                window[0], window[1],
                "Sieve has duplicate entry: ({}, {})",
                window[0].0, window[0].1
            );
        }
    }

    /// Verifies specific sieve entry patterns for known multiplicative orders.
    ///
    /// Checks that the sieve contains expected entries:
    ///   - q=11: ord_11(2) = 10, 10 % 4 = 2, half = 5. The sieve should have
    ///     an entry with ord=10.
    ///
    /// Also explores the subtlety that when a sieve entry (10, 5) fires for p=5,
    /// it identifies W(5) = 11 as having factor 11 — but 11 IS prime (it divides
    /// itself). This is only a problem when the candidate equals the sieve prime,
    /// which is handled by the sieve_min_exp guard in the main search.
    #[test]
    fn sieve_is_composite_specific_elimination() {
        // ord_11(2) = 10, half = 5: (2^p+1)/3 composite when p ≡ 5 (mod 10)
        // p=5: (2^5+1)/3 = 11. But 11 IS prime! The sieve should not misfire because
        // the candidate equals the sieve prime (11 divides itself trivially).
        // For p >= sieve_min_exp, the candidate is much larger than the sieve prime.
        //
        // p=5 gives W(5)=11 which is prime. At p=15 (not prime, skip).
        // p=25 (not prime). p=35 (not prime).
        // The sieve entry (10, 5) means: any prime p where p % 10 == 5 has q|W(p).
        // But only primes matter, and there are no primes of the form 10k+5 > 5.
        // So this specific entry never fires for large enough p.
        //
        // Instead test with ord_43(2) = 14, half = 7:
        // prime p=7 with p%14=7 → (2^7+1)/3 = 43. 43 is prime (and equals q),
        // but for p >= sieve_min_exp this would correctly identify composites.
        let sieve_primes = sieve::generate_primes(1000);
        let wsieve = WagstaffSieve::new(&sieve_primes);

        // Verify the sieve has the expected entry pattern
        assert!(
            wsieve.entries.iter().any(|&(ord, _)| ord == 10),
            "Should have entry with ord=10 (from q=11)"
        );
    }

    /// Verifies that a composite Wagstaff number has an identifiable small factor.
    ///
    /// W(29) = (2^29 + 1)/3 = 178956971 = 59 * 3032321. The factor 59 is small
    /// enough to be found by trial division, demonstrating that the sieve (or a
    /// simple factor check) can efficiently identify this composite. This tests
    /// the practical effectiveness of small-factor elimination for Wagstaff candidates.
    #[test]
    fn wagstaff_composite_has_small_factor() {
        let w = wagstaff(29);
        assert_eq!(
            w.is_probably_prime(25),
            IsPrime::No,
            "(2^29+1)/3 should be composite"
        );
        // Verify it has a factor < 100
        assert!(
            w.is_divisible_u(59),
            "(2^29+1)/3 should be divisible by 59"
        );
    }
}
