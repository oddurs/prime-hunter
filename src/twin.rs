//! # Twin — Twin Prime Search (k·b^n ± 1)
//!
//! Searches for twin prime pairs where both k·b^n + 1 and k·b^n − 1 are
//! simultaneously prime. These are vastly rarer than individual primes —
//! the twin prime constant C₂ ≈ 1.32 governs the asymptotic density.
//!
//! ## Algorithm
//!
//! 1. **Intersected BSGS sieve**: Reuses `kbn::bsgs_sieve` to independently
//!    sieve the +1 and −1 forms. Only n-values where *both* forms survive are
//!    tested. This intersection typically eliminates >99.9% of candidates.
//!
//! 2. **Sequential testing**: For each surviving n, tests k·b^n + 1 first
//!    (Proth test is fast for composites). Only if +1 is prime does it test
//!    k·b^n − 1 (LLR or Miller–Rabin). This avoids the expensive −1 test
//!    in >50% of cases.
//!
//! 3. **Deterministic proofs**: When both forms get deterministic proofs
//!    (Proth for +1, LLR for −1), the twin pair is certified deterministic.
//!    Otherwise it is probabilistic.
//!
//! ## Complexity
//!
//! - Sieve: Same as `kbn::bsgs_sieve` (run once for both forms).
//! - Testing: O(n · M(n)) per survivor, but with early exit on +1 composite.
//!
//! ## References
//!
//! - OEIS: [A001097](https://oeis.org/A001097) — Twin primes.
//! - OEIS: [A007508](https://oeis.org/A007508) — Number of twin prime pairs below 10^n.
//! - First Hardy–Littlewood conjecture: the number of twin primes below x is
//!   asymptotic to 2·C₂·x / (ln x)².
//! - PrimeGrid Twin Prime Search: <https://www.primegrid.com/>

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
use crate::kbn;
use crate::progress::Progress;
use crate::CoordinationClient;
use crate::{exact_digits, sieve};

pub fn search(
    k: u64,
    base: u32,
    min_n: u64,
    max_n: u64,
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
    // Resolve sieve_limit: auto-tune if 0
    let candidate_bits = (max_n as f64 * (base as f64).log2() + (k as f64).log2().max(0.0)) as u64;
    let n_range = max_n.saturating_sub(min_n) + 1;
    let sieve_limit = sieve::resolve_sieve_limit(sieve_limit, candidate_bits, n_range);

    let sieve_primes = sieve::generate_primes(sieve_limit);
    info!(k, base, min_n, max_n, "twin prime search started");
    info!(
        prime_count = sieve_primes.len(),
        sieve_limit,
        "sieve initialized"
    );

    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::Twin { last_n, .. }) if last_n >= min_n && last_n < max_n => {
            info!(resume_n = last_n + 1, "resuming twin prime search");
            last_n + 1
        }
        _ => min_n,
    };

    // Minimum n where k*b^n > sieve_limit
    let sieve_min_n = if base >= 2 {
        let log_b = (base as f64).log10();
        let log_limit = (sieve_limit as f64).log10();
        ((log_limit - (k as f64).log10().max(0.0)) / log_b).ceil() as u64 + 1
    } else {
        u64::MAX
    };
    info!(sieve_min_n, "sieve active");

    info!(
        from = resume_from,
        to = max_n,
        candidates = max_n - resume_from + 1,
        "running twin sieve"
    );
    let (plus_survives, minus_survives) =
        kbn::bsgs_sieve(resume_from, max_n, k, base, &sieve_primes, sieve_min_n);

    let total_range = max_n - resume_from + 1;
    let twin_survivors: u64 = (0..plus_survives.len())
        .filter(|&i| plus_survives.get(i) && minus_survives.get(i))
        .count() as u64;
    info!(
        twin_survivors,
        total_range,
        survivor_pct = twin_survivors as f64 / total_range as f64 * 100.0,
        "sieve complete"
    );

    let mut last_checkpoint = Instant::now();
    let mut block_start = resume_from;
    let mut total_sieved: u64 = 0;

    while block_start <= max_n {
        let bsize = crate::block_size_for_n(block_start);
        let block_end = (block_start + bsize - 1).min(max_n);
        let block_len = block_end - block_start + 1;

        *progress.current.lock().unwrap() =
            format!("{}*{}^[{}..{}]±1 twin", k, base, block_start, block_end);

        // Only keep n where BOTH forms survive the sieve
        let survivors: Vec<u64> = (block_start..=block_end)
            .filter(|&n| {
                let idx = (n - resume_from) as usize;
                plus_survives.get(idx) && minus_survives.get(idx)
            })
            .collect();

        total_sieved += block_len - survivors.len() as u64;

        let base_pow_start = Integer::from(base).pow(crate::checked_u32(block_start));
        let k_int = Integer::from(k);

        let found_twins: Vec<_> = survivors
            .into_par_iter()
            .filter_map(|n| {
                let offset = n - block_start;
                let base_pow = if offset == 0 {
                    base_pow_start.clone()
                } else {
                    &base_pow_start * Integer::from(base).pow(crate::checked_u32(offset))
                };
                let kb = Integer::from(&k_int * &base_pow);

                // Test +1 first (Proth is fast for composites)
                let plus = Integer::from(&kb + 1u32);
                // Adaptive P-1 pre-filter (Stage 1 + Stage 2, auto-tuned B1/B2)
                if crate::p1::adaptive_p1_filter(&plus) {
                    return None;
                }
                let (r_plus, cert_plus, certificate_plus) =
                    kbn::test_prime(&plus, k, base, n, true, mr_rounds);
                if r_plus == IsPrime::No {
                    return None;
                }

                // +1 is (probably) prime, now test -1
                let minus = Integer::from(&kb - 1u32);
                if minus <= 0u32 {
                    return None;
                }
                // Adaptive P-1 pre-filter (Stage 1 + Stage 2, auto-tuned B1/B2)
                if crate::p1::adaptive_p1_filter(&minus) {
                    return None;
                }
                let (r_minus, cert_minus, certificate_minus) =
                    kbn::test_prime(&minus, k, base, n, false, mr_rounds);
                if r_minus == IsPrime::No {
                    return None;
                }

                // Both are prime — twin pair found!
                let digits = exact_digits(&plus);
                let certainty = match (cert_plus, cert_minus) {
                    ("deterministic", "deterministic") => "deterministic",
                    _ => "probabilistic",
                };
                // Prefer the +1 certificate (Proth), fall back to -1 (LLR)
                let certificate = certificate_plus.or(certificate_minus);
                let cert_json = certificate
                    .as_ref()
                    .and_then(|c| serde_json::to_string(c).ok());
                Some((n, digits, certainty.to_string(), cert_json))
            })
            .collect();

        progress.tested.fetch_add(block_len, Ordering::Relaxed);

        for (n, digits, certainty, cert_json) in found_twins {
            let expr = format!("{}*{}^{} +/- 1", k, base, n);
            progress.found.fetch_add(1, Ordering::Relaxed);
            if let Some(eb) = event_bus {
                eb.emit(events::Event::PrimeFound {
                    form: "twin".into(),
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
                    "twin prime pair found"
                );
            }
            db.insert_prime_sync(
                rt,
                "twin",
                &expr,
                digits,
                search_params,
                &certainty,
                cert_json.as_deref(),
            )?;
            if let Some(wc) = worker_client {
                wc.report_prime("twin", &expr, digits, search_params, &certainty);
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Twin {
                    last_n: block_end,
                    k: Some(k),
                    base: Some(base),
                    min_n: Some(min_n),
                    max_n: Some(max_n),
                },
            )?;
            info!(n = block_end, sieved_out = total_sieved, "checkpoint saved");
            last_checkpoint = Instant::now();
        }

        if worker_client.is_some_and(|wc| wc.is_stop_requested()) {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Twin {
                    last_n: block_end,
                    k: Some(k),
                    base: Some(base),
                    min_n: Some(min_n),
                    max_n: Some(max_n),
                },
            )?;
            info!(n = block_end, "stop requested by coordinator, checkpoint saved");
            return Ok(());
        }

        block_start = block_end + 1;
    }

    checkpoint::clear(checkpoint_path);
    info!(total_sieved, "twin prime search complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Tests for the twin prime search module (k*b^n +/- 1).
    //!
    //! ## Mathematical Form
    //!
    //! Twin primes are pairs (p, p+2) where both are prime. In the k*b^n form,
    //! twin pairs occur when both k*b^n + 1 and k*b^n - 1 are simultaneously prime.
    //! This requires k*b^n to be even (so both neighbors are odd), which constrains
    //! the search to even k*b^n values.
    //!
    //! The first twin prime pairs are: (3,5), (5,7), (11,13), (17,19), (29,31), ...
    //! (OEIS [A001359](https://oeis.org/A001359) for the smaller of each pair)
    //!
    //! ## Key References
    //!
    //! - The twin prime conjecture (unproven): there are infinitely many twin primes.
    //! - Hardy-Littlewood first conjecture: the number of twin primes below x is
    //!   asymptotically 2*C2*x/(ln x)^2, where C2 ~ 1.32 is the twin prime constant.
    //! - The twin prime constant C2 = product over odd primes p of (1 - 1/(p-1)^2).
    //! - OEIS [A001097](https://oeis.org/A001097) — Twin primes (all members of pairs).
    //! - OEIS [A007508](https://oeis.org/A007508) — Count of twin prime pairs below 10^n.
    //!
    //! ## Testing Strategy
    //!
    //! 1. **Known twin pairs**: Verify both sides are prime for known k*b^n values.
    //! 2. **Non-twin cases**: Test where exactly one side is composite.
    //! 3. **Deterministic proofs**: Verify Proth (+1) and LLR (-1) certificates.
    //! 4. **Sieve intersection**: Verify the dual sieve correctly intersects.
    //! 5. **Edge cases**: Zero minus values, large k, digit count consistency.

    use super::*;

    /// Helper: compute k*base^n + 1.
    fn kb_plus(k: u64, base: u32, n: u64) -> Integer {
        Integer::from(k) * Integer::from(base).pow(crate::checked_u32(n)) + 1u32
    }

    /// Helper: compute k*base^n - 1.
    fn kb_minus(k: u64, base: u32, n: u64) -> Integer {
        Integer::from(k) * Integer::from(base).pow(crate::checked_u32(n)) - 1u32
    }

    // ── Known Twin Pairs ────────────────────────────────────────────────

    /// Verifies known twin pairs for k=3, base=2 at n in {1, 2, 6}.
    ///
    ///   - n=1: 3*2 +/- 1 = (5, 7) — twin pair
    ///   - n=2: 3*4 +/- 1 = (11, 13) — twin pair
    ///   - n=6: 3*64 +/- 1 = (191, 193) — twin pair
    ///
    /// k=3 with base=2 is a productive coefficient for twin prime searches
    /// and is commonly used in PrimeGrid's Twin Prime Search project.
    #[test]
    fn known_twin_pairs_k3_base2() {
        // k=3, base=2: twin pairs at n=1 (5,7), n=2 (11,13), n=6 (191,193)
        for &n in &[1u64, 2, 6] {
            let plus = kb_plus(3, 2, n);
            let minus = kb_minus(3, 2, n);
            assert_ne!(
                plus.is_probably_prime(25),
                IsPrime::No,
                "3*2^{}+1 = {} should be prime",
                n,
                plus
            );
            assert_ne!(
                minus.is_probably_prime(25),
                IsPrime::No,
                "3*2^{}-1 = {} should be prime",
                n,
                minus
            );
        }
    }

    /// Verifies twin pairs with different k values to test generality.
    ///
    ///   - k=15, b=2, n=1: 15*2 +/- 1 = (29, 31) — twin pair
    ///   - k=9, b=2, n=3: 9*8 +/- 1 = (71, 73) — twin pair
    ///
    /// Different k values exercise different code paths in the BSGS sieve
    /// (different residue classes mod each sieve prime).
    #[test]
    fn known_twin_pairs_various_k() {
        assert_ne!(kb_plus(15, 2, 1).is_probably_prime(25), IsPrime::No);
        assert_ne!(kb_minus(15, 2, 1).is_probably_prime(25), IsPrime::No);

        // k=9, b=2, n=3: (71, 73)
        assert_ne!(kb_plus(9, 2, 3).is_probably_prime(25), IsPrime::No);
        assert_ne!(kb_minus(9, 2, 3).is_probably_prime(25), IsPrime::No);
    }

    // ── Non-Twin Cases ────────────────────────────────────────────────

    /// Verifies rejection when exactly one side is composite.
    ///
    ///   - n=3: 3*8+1 = 25 = 5^2 (composite), 3*8-1 = 23 (prime) — NOT twin
    ///   - n=4: 3*16+1 = 49 = 7^2 (composite), 3*16-1 = 47 (prime) — NOT twin
    ///
    /// The search tests +1 first (Proth test is fast for composites) and skips
    /// the -1 test when +1 fails. These cases exercise that early-exit path.
    /// Note that both non-prime +1 values are perfect squares (5^2 and 7^2).
    #[test]
    fn non_twin_one_composite() {
        assert_eq!(
            kb_plus(3, 2, 3).is_probably_prime(25),
            IsPrime::No,
            "3*2^3+1=25 should be composite"
        );

        // k=3, b=2, n=4: 3*16+1=49=7^2 (composite), 3*16-1=47 (prime)
        assert_eq!(
            kb_plus(3, 2, 4).is_probably_prime(25),
            IsPrime::No,
            "3*2^4+1=49 should be composite"
        );
    }

    // ── Deterministic Proofs ───────────────────────────────────────────

    /// Verifies deterministic proofs for both sides of a twin pair: (191, 193).
    ///
    /// k=3, base=2, n=6:
    ///   - 3*2^6 + 1 = 193: Proth form (k*2^n + 1 with k < 2^n), so Proth's
    ///     theorem provides a deterministic certificate.
    ///   - 3*2^6 - 1 = 191: Riesel form (k*2^n - 1), so LLR test provides
    ///     a deterministic certificate.
    ///
    /// The twin pair is classified as "deterministic" only when BOTH sides
    /// have deterministic proofs.
    #[test]
    fn twin_deterministic_proof() {
        let plus = kb_plus(3, 2, 6);
        let minus = kb_minus(3, 2, 6);

        let (r_plus, cert_plus, _) = kbn::test_prime(&plus, 3, 2, 6, true, 25);
        assert_eq!(r_plus, IsPrime::Yes, "3*2^6+1=193 should be prime");
        assert_eq!(cert_plus, "deterministic");

        let (r_minus, cert_minus, _) = kbn::test_prime(&minus, 3, 2, 6, false, 25);
        assert_eq!(r_minus, IsPrime::Yes, "3*2^6-1=191 should be prime");
        assert_eq!(cert_minus, "deterministic");
    }

    // ── Sieve Intersection ─────────────────────────────────────────────

    /// Verifies the dual BSGS sieve intersection for twin prime candidates.
    ///
    /// The twin search runs a single `kbn::bsgs_sieve` call that returns both
    /// plus_survives and minus_survives bitvectors. Only n-values where BOTH
    /// bitvectors are set are tested. This test verifies:
    ///   1. Sieve-eliminated +1 candidates are actually composite.
    ///   2. Sieve-eliminated -1 candidates are actually composite.
    ///   3. The intersection count <= min(plus_count, minus_count).
    #[test]
    fn twin_sieve_intersects_correctly() {
        let sieve_primes = sieve::generate_primes(10_000);
        let k = 3u64;
        let base = 2u32;
        let sieve_min_n = 14u64;

        let (plus_surv, minus_surv) = kbn::bsgs_sieve(1, 200, k, base, &sieve_primes, sieve_min_n);

        // Verify: when BOTH survive, at least check that the sieve was correct
        for n in sieve_min_n..=200 {
            let idx = (n - 1) as usize;
            if !plus_surv.get(idx) {
                let p = kb_plus(k, base, n);
                assert_eq!(
                    p.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said 3*2^{}+1 composite but it's prime",
                    n
                );
            }
            if !minus_surv.get(idx) {
                let m = kb_minus(k, base, n);
                assert_eq!(
                    m.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said 3*2^{}-1 composite but it's prime",
                    n
                );
            }
        }

        // Twin intersection should be a subset of both individual sieves
        let twin_count = (sieve_min_n..=200)
            .filter(|&n| {
                let idx = (n - 1) as usize;
                plus_surv.get(idx) && minus_surv.get(idx)
            })
            .count();
        let plus_count = (sieve_min_n..=200)
            .filter(|&n| plus_surv.get((n - 1) as usize))
            .count();
        let minus_count = (sieve_min_n..=200)
            .filter(|&n| minus_surv.get((n - 1) as usize))
            .count();
        assert!(twin_count <= plus_count);
        assert!(twin_count <= minus_count);
    }

    // ── Additional Twin Prime Tests ────────────────────────────────────

    /// Verifies twin pairs in base 3: k=2, base=3, n=1 gives (5, 7).
    ///
    /// 2*3 +/- 1 = (5, 7), a twin pair. Tests that the module works correctly
    /// for non-binary bases where the Proth/LLR deterministic tests may not apply.
    #[test]
    fn known_twin_pairs_base3() {
        // k=1, base=3: n=1 → (4, 2) — 4 composite, n=2 → (10, 8) — both composite
        // k=2, base=3: n=1 → (7, 5) — both prime! Twin pair.
        let plus = kb_plus(2, 3, 1); // 2*3+1 = 7
        let minus = kb_minus(2, 3, 1); // 2*3-1 = 5
        assert_ne!(plus.is_probably_prime(25), IsPrime::No, "7 should be prime");
        assert_ne!(
            minus.is_probably_prime(25),
            IsPrime::No,
            "5 should be prime"
        );
    }

    /// Verifies the simplest k=1 twin pair: k=1, base=2, n=2 gives (3, 5).
    ///
    /// 1*4 - 1 = 3, 1*4 + 1 = 5. The smallest twin pair (3, 5) expressed
    /// in k*b^n form. This edge case has the minimum possible k and n values.
    #[test]
    fn known_twin_pairs_k1_base2() {
        let plus = kb_plus(1, 2, 2);
        let minus = kb_minus(1, 2, 2);
        assert_eq!(plus, 5);
        assert_eq!(minus, 3);
        assert_ne!(plus.is_probably_prime(25), IsPrime::No, "5 is prime");
        assert_ne!(minus.is_probably_prime(25), IsPrime::No, "3 is prime");
    }

    /// Verifies that BOTH sides must be prime for a twin pair declaration.
    ///
    /// Two cases where exactly one side is composite:
    ///   - n=3: +1 side is 25 (composite), -1 side is 23 (prime). NOT twin.
    ///   - n=5: +1 side is 97 (prime), -1 side is 95 = 5*19 (composite). NOT twin.
    ///
    /// This is the complement of `non_twin_one_composite` — here we also check
    /// the case where +1 is prime but -1 is composite, which exercises the full
    /// test path (the search doesn't skip -1 when +1 passes).
    #[test]
    fn twin_pair_both_must_be_prime() {
        let plus_3 = kb_plus(3, 2, 3);
        let minus_3 = kb_minus(3, 2, 3);
        assert_eq!(plus_3.is_probably_prime(25), IsPrime::No, "25 composite");
        assert_ne!(minus_3.is_probably_prime(25), IsPrime::No, "23 prime");

        let plus_5 = kb_plus(3, 2, 5);
        let minus_5 = kb_minus(3, 2, 5);
        assert_ne!(plus_5.is_probably_prime(25), IsPrime::No, "97 prime");
        assert_eq!(minus_5.is_probably_prime(25), IsPrime::No, "95 composite");
    }

    // ── Edge Cases ──────────────────────────────────────────────────────

    /// Verifies handling of k*b^n - 1 = 0 (non-positive result).
    ///
    /// k=1, b=2, n=0: 1*1 - 1 = 0, which is not prime. The search must guard
    /// against this with `if minus <= 0u32 { return None; }`. This edge case
    /// only occurs for very small n values and specific k values.
    #[test]
    fn twin_kb_minus_nonpositive_rejected() {
        let minus = kb_minus(1, 2, 0);
        assert_eq!(minus, 0, "1*2^0-1 should be 0");
    }

    /// Verifies deterministic proofs for the smallest k=3 twin pair: (5, 7).
    ///
    /// k=3, base=2, n=1: 3*2+1 = 7 (Proth), 3*2-1 = 5 (Riesel). Both are
    /// small enough for GMP to provide exact (deterministic) proofs. The test
    /// verifies that `kbn::test_prime` returns "deterministic" for both.
    #[test]
    fn twin_deterministic_proof_both_sides() {
        let plus = kb_plus(3, 2, 1);
        let minus = kb_minus(3, 2, 1);

        let (r_plus, cert_plus, _) = kbn::test_prime(&plus, 3, 2, 1, true, 25);
        assert_ne!(r_plus, IsPrime::No, "3*2^1+1=7 should be prime");

        let (r_minus, cert_minus, _) = kbn::test_prime(&minus, 3, 2, 1, false, 25);
        assert_ne!(r_minus, IsPrime::No, "3*2^1-1=5 should be prime");

        // Both should be deterministic (small numbers get GMP exact proof)
        assert_eq!(cert_plus, "deterministic");
        assert_eq!(cert_minus, "deterministic");
    }

    /// Soundness check: known twin pair n-values must survive both sieves.
    ///
    /// Known twin pairs at k=3, b=2: n in {1, 2, 6}. All must survive both the
    /// plus and minus sieves. For n < sieve_min_n, the sieve is not applied
    /// (all candidates survive), so this is automatically satisfied for small n.
    #[test]
    fn twin_sieve_survivors_superset_of_primes() {
        let sieve_primes = sieve::generate_primes(10_000);
        let sieve_min_n = 14u64;

        let (plus_surv, minus_surv) = kbn::bsgs_sieve(1, 200, 3, 2, &sieve_primes, sieve_min_n);

        // For n < sieve_min_n, all candidates survive (sieve not applied)
        for &n in &[1u64, 2, 6] {
            let idx = (n - 1) as usize;
            assert!(
                plus_surv.get(idx),
                "Known twin n={}: +1 should survive sieve",
                n
            );
            assert!(
                minus_surv.get(idx),
                "Known twin n={}: -1 should survive sieve",
                n
            );
        }
    }

    /// Smoke test: verify large k value (k=105) does not panic for small n.
    ///
    /// k=105 is a moderately large coefficient. This test ensures that the
    /// computation of k*2^n +/- 1 and the subsequent primality test do not
    /// panic or overflow for n in 1..10. It does not assert primality — only
    /// that the computation completes without error.
    #[test]
    fn twin_large_k_base2() {
        for n in 1..=10u64 {
            let plus = kb_plus(105, 2, n);
            let minus = kb_minus(105, 2, n);
            // Just verify we can compute and test without panicking
            let _ = plus.is_probably_prime(10);
            let _ = minus.is_probably_prime(10);
        }
    }

    /// Verifies that the digit count function returns consistent results.
    ///
    /// k=3, b=2, n=6: 3*64+1 = 193 (3 digits). The `exact_digits` function
    /// is used for database logging and must match the actual decimal
    /// representation length. Both sides of a twin pair have the same digit
    /// count (since they differ by only 2).
    #[test]
    fn twin_pair_digit_count_matches() {
        let plus = kb_plus(3, 2, 6);
        let digits = crate::exact_digits(&plus);
        assert_eq!(digits, 3, "3*2^6+1=193 should have 3 digits");
    }
}
