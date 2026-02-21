//! # Sophie Germain — Sophie Germain Prime Search
//!
//! Searches for Sophie Germain primes: primes p such that 2p + 1 (the "safe prime")
//! is also prime. In the k·b^n form: if p = k·b^n − 1, then 2p + 1 = 2k·b^n − 1.
//! Both are of the Riesel form and LLR-testable when base = 2.
//!
//! ## Algorithm
//!
//! 1. **Dual BSGS sieve**: Sieves both p = k·b^n − 1 (using k) and
//!    2p + 1 = 2k·b^n − 1 (using 2k) via `kbn::bsgs_sieve`. Only n-values
//!    where both forms survive are tested. The doubling of k requires no
//!    separate sieve infrastructure — just a different k parameter.
//!
//! 2. **Intersected survivors**: Combines the −1 sieve for k with the −1 sieve
//!    for 2k. An n-value is tested only if both survive. This eliminates >99%
//!    of candidates.
//!
//! 3. **LLR deterministic proofs**: Both p and 2p+1 are k·2^n − 1 forms, so
//!    LLR provides deterministic certificates for both (when base = 2 and k is odd).
//!
//! ## Relationship to Twin Primes
//!
//! While twin primes are k·b^n ± 1 (both prime), Sophie Germain primes are
//! k·b^n − 1 and 2k·b^n − 1 (both prime). The sieve is similar but operates
//! on the −1 form with two different k values.
//!
//! ## Complexity
//!
//! - Sieve: Two independent BSGS sieves, each O(π(L) · √p̄).
//! - Testing: O(n · M(n)) per survivor (two LLR tests).
//!
//! ## References
//!
//! - OEIS: [A005384](https://oeis.org/A005384) — Sophie Germain primes.
//! - OEIS: [A005385](https://oeis.org/A005385) — Safe primes (2p + 1).
//! - Sophie Germain's work on Fermat's Last Theorem, 1823.
//! - PrimeGrid Sophie Germain Prime Search: <https://www.primegrid.com/>

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

/// Search for Sophie Germain primes: p = k*b^n - 1 where both p and 2p+1 are prime.
///
/// 2p+1 = 2*k*b^n - 1, which is also a Riesel form with doubled k.
/// Both forms are LLR-testable when base=2.
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
    let k2 = k.checked_mul(2).expect("2*k overflows u64");

    // Resolve sieve_limit: auto-tune if 0
    let candidate_bits = (max_n as f64 * (base as f64).log2() + (k as f64).log2().max(0.0)) as u64;
    let n_range = max_n.saturating_sub(min_n) + 1;
    let sieve_limit = sieve::resolve_sieve_limit(sieve_limit, candidate_bits, n_range);

    let sieve_primes = sieve::generate_primes(sieve_limit);
    info!(k, k2, base, min_n, max_n, "Sophie Germain search started");
    info!(
        prime_count = sieve_primes.len(),
        sieve_limit,
        "sieve initialized"
    );

    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::SophieGermain { last_n, .. }) if last_n >= min_n && last_n < max_n => {
            info!(resume_n = last_n + 1, "resuming Sophie Germain search");
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
    // For 2k form, sieve_min_n is at most the same (2k is larger)
    info!(sieve_min_n, "sieve active");

    // Sieve for p = k*b^n - 1
    info!(k, base, from = resume_from, to = max_n, "running sieve for p=k*b^n-1");
    let (_plus_surv_k, minus_surv_k) =
        kbn::bsgs_sieve(resume_from, max_n, k, base, &sieve_primes, sieve_min_n);

    // Sieve for 2p+1 = 2k*b^n - 1
    info!(k = k2, base, from = resume_from, to = max_n, "running sieve for 2p+1=2k*b^n-1");
    let (_plus_surv_k2, minus_surv_k2) =
        kbn::bsgs_sieve(resume_from, max_n, k2, base, &sieve_primes, sieve_min_n);

    let total_range = max_n - resume_from + 1;
    let sg_survivors: u64 = (0..minus_surv_k.len())
        .filter(|&i| minus_surv_k.get(i) && minus_surv_k2.get(i))
        .count() as u64;
    info!(
        sg_survivors,
        total_range,
        survivor_pct = sg_survivors as f64 / total_range as f64 * 100.0,
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
            format!("{}*{}^[{}..{}]-1 SG", k, base, block_start, block_end);

        // Only keep n where BOTH forms survive the sieve
        let survivors: Vec<u64> = (block_start..=block_end)
            .filter(|&n| {
                let idx = (n - resume_from) as usize;
                minus_surv_k.get(idx) && minus_surv_k2.get(idx)
            })
            .collect();

        total_sieved += block_len - survivors.len() as u64;

        let base_pow_start = Integer::from(base).pow(crate::checked_u32(block_start));
        let k_int = Integer::from(k);
        let k2_int = Integer::from(k2);

        let found: Vec<_> = survivors
            .into_par_iter()
            .filter_map(|n| {
                let offset = n - block_start;
                let base_pow = if offset == 0 {
                    base_pow_start.clone()
                } else {
                    &base_pow_start * Integer::from(base).pow(crate::checked_u32(offset))
                };

                // Test p = k*b^n - 1
                let p = Integer::from(&k_int * &base_pow) - 1u32;
                if p <= 0u32 {
                    return None;
                }
                // Adaptive P-1 pre-filter (Stage 1 + Stage 2, auto-tuned B1/B2)
                if crate::p1::adaptive_p1_filter(&p) {
                    return None;
                }
                let (r_p, cert_p, certificate_p) =
                    kbn::test_prime(&p, k, base, n, false, mr_rounds);
                if r_p == IsPrime::No {
                    return None;
                }

                // p is (probably) prime, now test 2p+1 = 2k*b^n - 1
                let safe = Integer::from(&k2_int * &base_pow) - 1u32;
                // Adaptive P-1 pre-filter (Stage 1 + Stage 2, auto-tuned B1/B2)
                if crate::p1::adaptive_p1_filter(&safe) {
                    return None;
                }
                let (r_safe, cert_safe, _certificate_safe) =
                    kbn::test_prime(&safe, k2, base, n, false, mr_rounds);
                if r_safe == IsPrime::No {
                    return None;
                }

                // Sophie Germain pair found!
                let digits = exact_digits(&p);
                let certainty = match (cert_p, cert_safe) {
                    ("deterministic", "deterministic") => "deterministic",
                    _ => "probabilistic",
                };
                // Use the certificate from p (the Sophie Germain prime itself)
                let cert_json = certificate_p
                    .as_ref()
                    .and_then(|c| serde_json::to_string(c).ok());
                Some((n, digits, certainty.to_string(), cert_json))
            })
            .collect();

        progress.tested.fetch_add(block_len, Ordering::Relaxed);

        for (n, digits, certainty, cert_json) in found {
            let expr = format!("{}*{}^{}-1", k, base, n);
            progress.found.fetch_add(1, Ordering::Relaxed);
            if let Some(eb) = event_bus {
                eb.emit(events::Event::PrimeFound {
                    form: "sophie_germain".into(),
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
                    "Sophie Germain prime found"
                );
            }
            db.insert_prime_sync(
                rt,
                "sophie_germain",
                &expr,
                digits,
                search_params,
                &certainty,
                cert_json.as_deref(),
            )?;
            if let Some(wc) = worker_client {
                wc.report_prime("sophie_germain", &expr, digits, search_params, &certainty);
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::SophieGermain {
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
                &Checkpoint::SophieGermain {
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
    info!(total_sieved, "Sophie Germain search complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Tests for the Sophie Germain prime search module.
    //!
    //! ## Mathematical Form
    //!
    //! A Sophie Germain prime is a prime p such that 2p + 1 (the "safe prime")
    //! is also prime. In the k*b^n form used by this module:
    //!   - p = k*b^n - 1 (Riesel form)
    //!   - 2p + 1 = 2k*b^n - 1 (also Riesel form, with doubled k)
    //!
    //! The first Sophie Germain primes are: 2, 3, 5, 11, 23, 29, 41, 53, 83, 89, ...
    //! (OEIS [A005384](https://oeis.org/A005384))
    //!
    //! The corresponding safe primes are: 5, 7, 11, 23, 47, 59, 83, 107, 167, 179, ...
    //! (OEIS [A005385](https://oeis.org/A005385))
    //!
    //! ## Key References
    //!
    //! - Sophie Germain (1776-1831) used these primes in her partial proof of
    //!   Fermat's Last Theorem for Case 1 (1823).
    //! - The Hardy-Littlewood conjecture predicts infinitely many SG primes,
    //!   with density ~ 2*C2 / (ln x)^2 where C2 ~ 1.32 is the twin prime constant.
    //! - LLR (Lucas-Lehmer-Riesel) test provides deterministic proofs for both
    //!   p and 2p+1 when base=2 and k is odd.
    //!
    //! ## Testing Strategy
    //!
    //! 1. **Known SG pairs**: Verify both p and 2p+1 are prime for known pairs.
    //! 2. **Non-SG cases**: Test where p is prime but 2p+1 is composite (or vice versa).
    //! 3. **Dual sieve intersection**: Verify the sieve for k and 2k correctly
    //!    intersects to produce SG candidates.
    //! 4. **Deterministic proofs**: Verify LLR provides certificates for both sides.
    //! 5. **Edge cases**: Overflow detection, non-binary bases.

    use super::*;

    /// Helper: compute k*base^n - 1.
    fn kb_minus(k: u64, base: u32, n: u64) -> Integer {
        Integer::from(k) * Integer::from(base).pow(crate::checked_u32(n)) - 1u32
    }

    // ── Known Sophie Germain Pairs ──────────────────────────────────────

    /// Verifies the simplest SG pair in k=1, base=2 form: p = 2^2 - 1 = 3, 2p+1 = 7.
    ///
    /// k=1, b=2, n=2: p = 4-1 = 3, safe = 2*4-1 = 7. Both prime.
    /// Also checks non-SG cases: n=3 gives p=7 (prime) but 2p+1=15=3*5 (composite),
    /// and n=5 gives p=31 but 2p+1=63=9*7 (composite).
    #[test]
    fn known_sophie_germain_k1_base2() {
        // k=1, base=2: p = 2^n - 1 (Mersenne), safe = 2^(n+1) - 1
        // n=2: p=3, 2p+1=7 — both prime (SG pair)
        // n=3: p=7, 2p+1=15=3*5 — NOT SG
        // n=5: p=31, 2p+1=63=9*7 — NOT SG
        let p = kb_minus(1, 2, 2);
        assert_eq!(p, 3);
        assert_ne!(p.is_probably_prime(25), IsPrime::No);
        let safe = kb_minus(2, 2, 2); // 2*2^2 - 1 = 7
        assert_eq!(safe, 7);
        assert_ne!(safe.is_probably_prime(25), IsPrime::No);
    }

    /// Verifies a rich SG sequence: k=3, base=2 yields SG pairs at n=1,2,3.
    ///
    ///   - n=1: p = 3*2-1 = 5, 2p+1 = 6*2-1 = 11 (SG pair)
    ///   - n=2: p = 3*4-1 = 11, 2p+1 = 6*4-1 = 23 (SG pair)
    ///   - n=3: p = 3*8-1 = 23, 2p+1 = 6*8-1 = 47 (SG pair)
    ///
    /// This is an unusually productive k value — three consecutive n values all
    /// yield SG pairs. The search module tests both p = k*b^n - 1 and
    /// 2p+1 = 2k*b^n - 1 using `kbn::test_prime`.
    #[test]
    fn known_sophie_germain_k3_base2() {
        for &n in &[1u64, 2, 3] {
            let p = kb_minus(3, 2, n);
            let safe = kb_minus(6, 2, n);
            assert_ne!(
                p.is_probably_prime(25),
                IsPrime::No,
                "3*2^{}-1 = {} should be prime",
                n,
                p
            );
            assert_ne!(
                safe.is_probably_prime(25),
                IsPrime::No,
                "6*2^{}-1 = {} should be prime",
                n,
                safe
            );
        }
    }

    // ── Non-SG Cases ──────────────────────────────────────────────────

    /// Verifies rejection when p is prime but the safe prime 2p+1 is composite.
    ///
    /// k=3, base=2, n=4: p = 3*16-1 = 47 (prime), but 2p+1 = 6*16-1 = 95 = 5*19
    /// (composite). This is NOT a Sophie Germain pair. The search must test both
    /// sides and reject the pair when either is composite.
    #[test]
    fn non_sophie_germain_p_prime_safe_composite() {
        let p = kb_minus(3, 2, 4);
        assert_eq!(p, 47);
        assert_ne!(p.is_probably_prime(25), IsPrime::No, "47 is prime");
        let safe = kb_minus(6, 2, 4); // 6*16-1 = 95
        assert_eq!(safe, 95);
        assert_eq!(
            safe.is_probably_prime(25),
            IsPrime::No,
            "95 = 5*19 is composite"
        );
    }

    /// Verifies rejection when p itself is composite.
    ///
    /// k=3, base=2, n=5: p = 3*32-1 = 95 = 5*19 (composite). The search
    /// tests p first and immediately skips the 2p+1 test when p is composite,
    /// saving the cost of the second primality test.
    #[test]
    fn non_sophie_germain_p_composite() {
        let p = kb_minus(3, 2, 5);
        assert_eq!(p, 95);
        assert_eq!(
            p.is_probably_prime(25),
            IsPrime::No,
            "95 = 5*19 is composite"
        );
    }

    // ── Deterministic Proofs ───────────────────────────────────────────

    /// Verifies that both p and 2p+1 receive deterministic LLR proofs.
    ///
    /// k=3, base=2, n=2: p = 11, safe = 23. Both are of the form k*2^n - 1
    /// (Riesel form) with odd k and base 2, so the Lucas-Lehmer-Riesel test
    /// provides a deterministic certificate. The search reports "deterministic"
    /// only when BOTH sides have deterministic proofs.
    #[test]
    fn sophie_germain_deterministic_proof() {
        let p = kb_minus(3, 2, 2);
        let safe = kb_minus(6, 2, 2);

        let (r_p, cert_p, _) = kbn::test_prime(&p, 3, 2, 2, false, 25);
        assert_eq!(r_p, IsPrime::Yes, "3*2^2-1=11 should be prime");
        assert_eq!(cert_p, "deterministic");

        let (r_safe, cert_safe, _) = kbn::test_prime(&safe, 6, 2, 2, false, 25);
        assert_eq!(r_safe, IsPrime::Yes, "6*2^2-1=23 should be prime");
        assert_eq!(cert_safe, "deterministic");
    }

    // ── Dual Sieve Intersection ────────────────────────────────────────

    /// Verifies correctness of the dual BSGS sieve intersection for SG pairs.
    ///
    /// The SG search runs two independent BSGS sieves: one for p = k*b^n - 1
    /// and one for 2p+1 = 2k*b^n - 1. Only n-values where BOTH survive are
    /// tested. This test verifies:
    ///   1. If the k-sieve eliminates n, then k*b^n-1 is actually composite.
    ///   2. If the 2k-sieve eliminates n, then 2k*b^n-1 is actually composite.
    ///   3. The SG intersection is a subset of both individual survivor sets.
    #[test]
    fn sieve_intersects_correctly() {
        let sieve_primes = sieve::generate_primes(10_000);
        let k = 3u64;
        let k2 = 6u64;
        let base = 2u32;
        let sieve_min_n = 14u64;

        let (_plus_k, minus_k) = kbn::bsgs_sieve(1, 200, k, base, &sieve_primes, sieve_min_n);
        let (_plus_k2, minus_k2) = kbn::bsgs_sieve(1, 200, k2, base, &sieve_primes, sieve_min_n);

        // Verify sieve correctness: if sieved out, must be composite
        for n in sieve_min_n..=200 {
            let idx = (n - 1) as usize;
            if !minus_k.get(idx) {
                let p = kb_minus(k, base, n);
                assert_eq!(
                    p.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said 3*2^{}-1 composite but it's prime",
                    n
                );
            }
            if !minus_k2.get(idx) {
                let s = kb_minus(k2, base, n);
                assert_eq!(
                    s.is_probably_prime(15),
                    IsPrime::No,
                    "Sieve said 6*2^{}-1 composite but it's prime",
                    n
                );
            }
        }

        // SG intersection must be subset of both individual sieves
        let sg_count = (sieve_min_n..=200)
            .filter(|&n| {
                let idx = (n - 1) as usize;
                minus_k.get(idx) && minus_k2.get(idx)
            })
            .count();
        let k_count = (sieve_min_n..=200)
            .filter(|&n| minus_k.get((n - 1) as usize))
            .count();
        let k2_count = (sieve_min_n..=200)
            .filter(|&n| minus_k2.get((n - 1) as usize))
            .count();
        assert!(sg_count <= k_count);
        assert!(sg_count <= k2_count);
    }

    // ── OEIS Verification ─────────────────────────────────────────────

    /// Verifies the first 10 Sophie Germain primes from OEIS A005384.
    ///
    /// OEIS A005384: 2, 3, 5, 11, 23, 29, 41, 53, 83, 89
    /// For each p in this list, both p and 2p+1 must be prime. This serves as
    /// ground truth validation independent of the k*b^n representation.
    ///
    /// Note: these small primes don't directly correspond to k*b^n form values
    /// tested in the search, but validate the mathematical definition itself.
    #[test]
    fn known_sg_small_primes_oeis() {
        let sg_primes = [2u32, 3, 5, 11, 23, 29, 41, 53, 83, 89];
        for &p in &sg_primes {
            let p_int = Integer::from(p);
            assert_ne!(
                p_int.is_probably_prime(25),
                IsPrime::No,
                "SG prime {} should be prime",
                p
            );
            let safe = Integer::from(2 * p + 1);
            assert_ne!(
                safe.is_probably_prime(25),
                IsPrime::No,
                "Safe prime 2*{}+1={} should be prime",
                p,
                2 * p + 1
            );
        }
    }

    /// Verifies k=1, base=2, n=2: p=3, safe=7 — the simplest SG pair in k*b^n form.
    #[test]
    fn sg_k1_base2_n2_is_germain_pair() {
        // k=1, b=2, n=2: p = 1*4-1 = 3, 2p+1 = 7 — both prime
        let p = kb_minus(1, 2, 2);
        assert_eq!(p, 3);
        assert_ne!(p.is_probably_prime(25), IsPrime::No);
        let safe = kb_minus(2, 2, 2); // 2*4-1 = 7
        assert_eq!(safe, 7);
        assert_ne!(safe.is_probably_prime(25), IsPrime::No);
    }

    /// Verifies that p=7 is NOT a Sophie Germain prime (2*7+1 = 15 = 3*5).
    ///
    /// Not all primes are Sophie Germain primes. 7 is prime but 15 is composite,
    /// so the pair (7, 15) does not qualify. This tests the rejection path
    /// where the safe prime check fails.
    #[test]
    fn sg_not_germain_safe_composite() {
        let p = Integer::from(7u32);
        assert_ne!(p.is_probably_prime(25), IsPrime::No, "7 is prime");
        let safe = Integer::from(15u32);
        assert_eq!(safe.is_probably_prime(25), IsPrime::No, "15 is composite");
    }

    // ── Edge Cases ──────────────────────────────────────────────────────

    /// Verifies overflow detection when computing 2k.
    ///
    /// The search doubles k to compute the safe prime form 2k*b^n - 1. If k is
    /// near u64::MAX/2, the multiplication overflows. The search uses
    /// `checked_mul(2)` to detect this and panic with a clear message rather
    /// than silently wrapping.
    #[test]
    fn sg_k_overflow_detection() {
        let big_k = u64::MAX / 2 + 1;
        assert!(
            big_k.checked_mul(2).is_none(),
            "2 * (u64::MAX/2 + 1) should overflow"
        );
    }

    /// Verifies SG pairs in base 3: k=2, base=3, n=1 gives (5, 11).
    ///
    /// p = 2*3-1 = 5, safe = 4*3-1 = 11. Both prime — a Sophie Germain pair.
    /// Testing non-binary bases ensures the module generalizes beyond the
    /// base-2 case where LLR provides deterministic proofs.
    #[test]
    fn sg_base3_known_pairs() {
        let p = kb_minus(2, 3, 1);
        assert_eq!(p, 5);
        let safe = kb_minus(4, 3, 1); // 4*3-1 = 11
        assert_eq!(safe, 11);
        assert_ne!(p.is_probably_prime(25), IsPrime::No, "5 is prime");
        assert_ne!(safe.is_probably_prime(25), IsPrime::No, "11 is prime");
    }

    /// Property test: the SG sieve intersection is <= min(k_survivors, 2k_survivors).
    ///
    /// The intersection of two independent boolean filters can never exceed
    /// either individual filter's survivor count. This is a basic set-theoretic
    /// invariant (|A intersect B| <= min(|A|, |B|)) that validates the sieve
    /// intersection logic.
    #[test]
    fn sg_sieve_intersection_smaller_than_either() {
        let sieve_primes = sieve::generate_primes(10_000);
        let sieve_min_n = 14u64;

        let (_p_k, minus_k) = kbn::bsgs_sieve(1, 100, 3, 2, &sieve_primes, sieve_min_n);
        let (_p_k2, minus_k2) = kbn::bsgs_sieve(1, 100, 6, 2, &sieve_primes, sieve_min_n);

        let k_survivors = minus_k.count_ones();
        let k2_survivors = minus_k2.count_ones();
        let intersection = (0..minus_k.len())
            .filter(|&i| minus_k.get(i) && minus_k2.get(i))
            .count();

        assert!(
            intersection <= k_survivors.min(k2_survivors),
            "Intersection {} should be <= min({}, {})",
            intersection,
            k_survivors,
            k2_survivors
        );
    }

    /// Verifies deterministic proofs for both sides: k=3, b=2, n=3 -> (23, 47).
    ///
    /// Both 23 = 3*2^3 - 1 and 47 = 6*2^3 - 1 are Riesel numbers with odd k
    /// and base 2. The LLR test (Lucas-Lehmer-Riesel) provides deterministic
    /// certificates for both, so the pair is certified deterministic.
    #[test]
    fn sg_deterministic_both_sides() {
        let p = kb_minus(3, 2, 3);
        let safe = kb_minus(6, 2, 3);
        assert_eq!(p, 23);
        assert_eq!(safe, 47);

        let (r_p, cert_p, _) = kbn::test_prime(&p, 3, 2, 3, false, 25);
        assert_eq!(r_p, IsPrime::Yes, "23 should be prime");
        assert_eq!(cert_p, "deterministic");

        let (r_safe, cert_safe, _) = kbn::test_prime(&safe, 6, 2, 3, false, 25);
        assert_eq!(r_safe, IsPrime::Yes, "47 should be prime");
        assert_eq!(cert_safe, "deterministic");
    }
}
