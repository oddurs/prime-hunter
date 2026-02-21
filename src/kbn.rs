//! # KBN — Generalized k·b^n ± 1 Prime Search
//!
//! Searches for primes of the form k·b^n + 1 and k·b^n − 1 over a range of
//! exponents n, using a multi-stage pipeline: algebraic sieve → deterministic
//! proof → probabilistic fallback.
//!
//! ## Algorithm Pipeline
//!
//! 1. **BSGS algebraic sieve** (`bsgs_sieve`): For each sieve prime p, computes
//!    the discrete logarithm to find all n-values where k·b^n ≡ ∓1 (mod p),
//!    eliminating provably composite candidates. Uses baby-step giant-step with
//!    Montgomery multiplication, running in O(√ord) per prime.
//!
//! 2. **Proth test** (`proth_test`): For k·b^n + 1 with k < b^n and base 2,
//!    applies Proth's theorem (1878) — a single modular exponentiation yields a
//!    *deterministic* proof of primality. For non-base-2, generalizes via
//!    Pocklington's theorem.
//!
//! 3. **LLR test** (`llr_test`): For k·2^n − 1 with k odd and k < 2^n, applies
//!    the Lucas–Lehmer–Riesel test — n−2 modular squarings yield a deterministic
//!    proof. Includes Gerbicz-style error checking with automatic rollback.
//!
//! 4. **P-1 pre-filter**: For candidates > 50,000 bits, Pollard's P-1 algorithm
//!    catches 1–5% of composites that survive the algebraic sieve.
//!
//! 5. **GWNUM/PRST acceleration**: For very large candidates (>10K digits),
//!    delegates to external GWNUM or PRST tools for 50–100× speedup.
//!
//! 6. **Miller–Rabin fallback**: 25-round MR with 2-round pre-screen for
//!    candidates where no deterministic test applies.
//!
//! ## Key Functions (pub(crate))
//!
//! - `proth_test`, `llr_test`, `bsgs_sieve`, `test_prime` — reused by `twin`,
//!   `sophie_germain`, `cullen_woodall`, `carol_kynea`, and `gen_fermat`.
//! - `find_rodseth_v1`, `lucas_v_k` — LLR starting value computation.
//!
//! ## Complexity
//!
//! - Sieve: O(π(L) · √p̄) where L is sieve limit and p̄ is mean sieve prime.
//! - Proth/Pocklington: O(n · M(n)) — n squarings of n-bit numbers.
//! - LLR: O(n · M(n)) — (n−2) squarings of n-bit numbers.
//!
//! ## References
//!
//! - François Proth, "Théorèmes sur les nombres premiers", C. R. Acad. Sci. Paris, 1878.
//! - John Brillhart, D.H. Lehmer, J.L. Selfridge, "New Primality Criteria and
//!   Factorizations of 2^m ± 1", Mathematics of Computation, 29(130), 1975.
//! - Hans Riesel, "Lucasian Criteria for the Primality of N = h·2^n − 1",
//!   Mathematics of Computation, 23(108), 1969.
//! - Robert Gerbicz, "Error-Detecting LLR Algorithm", mersenneforum.org, 2017.
//! - OEIS: [A080076](https://oeis.org/A080076) (Proth primes with k=3)

use anyhow::Result;
use rayon::prelude::*;
use rug::integer::IsPrime;
use rug::ops::{Pow, RemRounding};
use rug::Integer;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use tracing::{debug, info, warn};

use crate::certificate::PrimalityCertificate;
use crate::checkpoint::{self, Checkpoint};
use crate::db::Database;
use crate::events::{self, EventBus};
use crate::progress::Progress;
use crate::CoordinationClient;
use crate::{exact_digits, sieve};

/// Proth primality test for p = k*2^n + 1 where k < 2^n.
///
/// By Proth's theorem, if there exists an integer a such that
/// a^((p-1)/2) ≡ -1 (mod p), then p is deterministically prime.
/// A single modular exponentiation replaces 15+ Miller-Rabin rounds.
///
/// Returns `Some((true, Some(a)))` for deterministic prime (with witness base `a`),
/// `Some((false, None))` for composite, `None` if inconclusive.
pub(crate) fn proth_test(p: &Integer) -> Option<(bool, Option<u32>)> {
    let p_minus_1 = Integer::from(p - 1u32);
    let exp = Integer::from(&p_minus_1 >> 1u32); // (p-1)/2

    for &a in &[2u32, 3, 5, 7, 11, 13] {
        if p.is_divisible_u(a) {
            continue; // Skip bases divisible by p (only relevant for tiny p)
        }
        match Integer::from(a).pow_mod(&exp, p) {
            Ok(result) => {
                if result == p_minus_1 {
                    return Some((true, Some(a))); // Deterministically prime
                }
                if result != 1u32 {
                    return Some((false, None)); // Failed Fermat test → composite
                }
                // result == 1: quadratic residue, try next base
            }
            Err(_) => return Some((false, None)), // gcd(a, p) > 1 → composite
        }
    }
    None // All bases inconclusive
}

/// Pocklington N-1 test for p = k*b^n + 1 where k < b^n and b is any base.
///
/// Generalization of Proth's theorem. If a^(p-1) ≡ 1 (mod p) and
/// gcd(a^((p-1)/q) - 1, p) = 1 for each prime factor q of b, then p is prime
/// (provided b^n > sqrt(p), i.e., k < b^n).
fn pocklington_test(p: &Integer, base: u32) -> Option<(bool, Option<u32>)> {
    let p_minus_1 = Integer::from(p - 1u32);

    // Get prime factors of base for the Pocklington condition
    let base_factors = small_prime_factors(base);

    for &a in &[2u32, 3, 5, 7, 11] {
        let a_int = Integer::from(a);

        // Check a^(p-1) ≡ 1 (mod p) (Fermat test)
        let fermat = match a_int.clone().pow_mod(&p_minus_1, p) {
            Ok(r) => r,
            Err(_) => return Some((false, None)),
        };
        if fermat != 1u32 {
            return Some((false, None)); // Composite
        }

        // Check gcd(a^((p-1)/q) - 1, p) = 1 for each prime factor q of base
        let mut all_coprime = true;
        for &q in &base_factors {
            let exp_q = Integer::from(&p_minus_1 / q);
            let r = match a_int.clone().pow_mod(&exp_q, p) {
                Ok(r) => r,
                Err(_) => return Some((false, None)),
            };
            let g = (r - 1u32).gcd(p);
            if g != 1u32 {
                all_coprime = false;
                break;
            }
        }

        if all_coprime {
            return Some((true, Some(a))); // Deterministically prime
        }
    }
    None
}

/// Return distinct prime factors of a small number.
fn small_prime_factors(mut n: u32) -> Vec<u32> {
    let mut factors = Vec::new();
    let mut d = 2u32;
    while d * d <= n {
        if n.is_multiple_of(d) {
            factors.push(d);
            while n.is_multiple_of(d) {
                n /= d;
            }
        }
        d += 1;
    }
    if n > 1 {
        factors.push(n);
    }
    factors
}

/// Find a suitable starting value P for LLR when k is divisible by 3.
///
/// When k % 3 == 0, the default P=4 fails because jacobi(P-2, N) and
/// jacobi(P+2, N) don't satisfy the required conditions. Rodseth's method
/// finds P where jacobi(P-2, N) == 1 AND jacobi(P+2, N) == -1.
pub(crate) fn find_rodseth_v1(n: &Integer) -> u32 {
    // Precomputed candidates that cover >99% of cases
    for &p in &[5u32, 8, 9, 10, 12, 14, 15, 16, 18, 20, 21, 22, 24, 25] {
        let pm2 = Integer::from(p - 2);
        let pp2 = Integer::from(p + 2);
        if pm2.jacobi(n) == 1 && pp2.jacobi(n) == -1 {
            return p;
        }
    }
    // Linear fallback for rare cases
    let mut p = 26u32;
    loop {
        let pm2 = Integer::from(p - 2);
        let pp2 = Integer::from(p + 2);
        if pm2.jacobi(n) == 1 && pp2.jacobi(n) == -1 {
            return p;
        }
        p += 1;
    }
}

/// Compute V_k(P, 1) mod N using the Lucas V binary chain.
///
/// Uses the recurrence: V(2m) = V(m)^2 - 2, V(2m+1) = V(m)*V(m+1) - P.
/// Runs in O(log k) multiplications mod N.
pub(crate) fn lucas_v_k(k: u64, p_val: u32, n: &Integer) -> Integer {
    if k == 0 {
        return Integer::from(2);
    }
    if k == 1 {
        return Integer::from(p_val).rem_euc(n);
    }

    let p_int = Integer::from(p_val);
    // r = V(m), s = V(m+1), starting with m=1
    let mut r = p_int.clone(); // V(1) = P
    let mut s = (Integer::from(&p_int * &p_int) - 2u32).rem_euc(n); // V(2) = P^2 - 2

    // Process bits of k from second-highest down to bit 0
    let bits = 64 - k.leading_zeros(); // number of significant bits
    for i in (0..bits - 1).rev() {
        if (k >> i) & 1 == 0 {
            // m stays even: s = V(2m+1), r = V(2m)
            s = Integer::from(&r * &s - &p_int).rem_euc(n);
            r.square_mut();
            r -= 2u32;
            r = r.rem_euc(n);
        } else {
            // m goes odd: r = V(2m+1), s = V(2m+2)
            r = Integer::from(&r * &s - &p_int).rem_euc(n);
            s.square_mut();
            s -= 2u32;
            s = s.rem_euc(n);
        }
    }
    r
}

/// Lucas-Lehmer-Riesel (LLR) deterministic primality test for N = k*2^n - 1.
///
/// Requires: k odd, k < 2^n, n >= 3.
/// Returns `Some((true, Some(seed_str)))` for proven prime (with seed `u_0 = V_k(P,1) mod N`),
/// `Some((false, None))` for composite, `None` if preconditions not met.
///
/// Includes Gerbicz-style error checking: periodic checkpoints during the
/// squaring loop, with automatic rollback and recomputation if a hardware
/// error corrupts the residue. When a prime is found, the last portion is
/// independently verified from a checkpoint to confirm the result.
pub(crate) fn llr_test(candidate: &Integer, k: u64, n: u64) -> Option<(bool, Option<String>)> {
    // Guard: LLR can give false negatives for very small n
    if n < 3 {
        return None;
    }

    // Choose starting value P
    let p_val = if !k.is_multiple_of(3) {
        4u32
    } else {
        find_rodseth_v1(candidate)
    };

    // Compute u_0 = V_k(P, 1) mod N
    let mut u = lucas_v_k(k, p_val, candidate);
    let seed_str = u.to_string();

    // Iterate n-2 squarings: u = u^2 - 2 mod N
    let iters = n - 2;

    // Gerbicz error checking: save checkpoints every L iterations,
    // verify after each block by recomputing from the previous checkpoint.
    // L = max(100, floor(sqrt(iters))) gives O(sqrt(n)) overhead.
    let check_interval = if iters > 10_000 {
        (iters as f64).sqrt() as u64
    } else {
        iters + 1 // disable checking for small n (not worth the overhead)
    };

    let mut last_checkpoint = u.clone();
    let mut last_checkpoint_iter: u64 = 0;
    let mut verified_checkpoint = u.clone();
    let mut verified_checkpoint_iter: u64 = 0;

    for i in 0..iters {
        if n > 50_000 && i % 10_000 == 0 && i > 0 {
            debug!(
                current = i,
                total = iters,
                percent = format_args!("{:.1}", i as f64 / iters as f64 * 100.0),
                "LLR squaring progress"
            );
        }
        u.square_mut();
        u -= 2u32;
        u = u.rem_euc(candidate);

        // Save checkpoint every check_interval iterations
        if check_interval < iters && (i + 1) % check_interval == 0 {
            // Verify current block: recompute from last_checkpoint
            let mut verify = last_checkpoint.clone();
            let steps = i + 1 - last_checkpoint_iter;
            for _ in 0..steps {
                verify.square_mut();
                verify -= 2u32;
                verify = verify.rem_euc(candidate);
            }

            if verify != u {
                // Hardware error detected! Rollback to last verified checkpoint.
                warn!(
                    iteration = i + 1,
                    rollback_to = verified_checkpoint_iter,
                    "LLR error detected, rolling back"
                );
                u = verified_checkpoint.clone();
                // Recompute from verified checkpoint; skip ahead to redo this block
                let redo_start = verified_checkpoint_iter;
                for j in redo_start..=i {
                    u.square_mut();
                    u -= 2u32;
                    u = u.rem_euc(candidate);
                    if (j + 1) % check_interval == 0 {
                        // Re-verify this block too
                        let mut v2 = if j + 1 - check_interval >= redo_start {
                            last_checkpoint.clone()
                        } else {
                            verified_checkpoint.clone()
                        };
                        let v2_start = if j + 1 - check_interval >= redo_start {
                            j + 1 - check_interval
                        } else {
                            redo_start
                        };
                        for _ in v2_start..=j {
                            v2.square_mut();
                            v2 -= 2u32;
                            v2 = v2.rem_euc(candidate);
                        }
                        if v2 == u {
                            last_checkpoint = u.clone();
                            last_checkpoint_iter = j + 1;
                            verified_checkpoint = last_checkpoint.clone();
                            verified_checkpoint_iter = j + 1;
                        } else {
                            warn!("LLR persistent error, returning inconclusive");
                            return None;
                        }
                    }
                }
            } else {
                // Block verified OK
                verified_checkpoint = last_checkpoint.clone();
                verified_checkpoint_iter = last_checkpoint_iter;
                last_checkpoint = u.clone();
                last_checkpoint_iter = i + 1;
            }
        }
    }

    let is_prime = u == 0u32;

    // When a prime is found, verify by recomputing from the last verified checkpoint
    if is_prime && verified_checkpoint_iter < iters {
        let mut verify = verified_checkpoint;
        for _ in verified_checkpoint_iter..iters {
            verify.square_mut();
            verify -= 2u32;
            verify = verify.rem_euc(candidate);
        }
        if verify != 0u32 {
            warn!("LLR prime verification failed, returning inconclusive");
            return None;
        }
    }

    if is_prime {
        Some((true, Some(seed_str)))
    } else {
        Some((false, None))
    }
}

/// Test primality using the best available method.
/// Uses Proth/Pocklington for k*b^n+1 when applicable, falls back to Miller-Rabin.
/// Returns `(IsPrime, proof_method_label, Option<PrimalityCertificate>)`.
pub(crate) fn test_prime(
    candidate: &Integer,
    k: u64,
    base: u32,
    n: u64,
    is_plus: bool,
    mr_rounds: u32,
) -> (IsPrime, &'static str, Option<PrimalityCertificate>) {
    // Proth/Pocklington only applies to +1 form where k < b^n
    let can_use_n1_test = is_plus && {
        if n >= 64 {
            true // k (u64) is always < b^64 <= b^n for b >= 2
        } else {
            (k as u128) < (base as u128).pow(n as u32)
        }
    };

    if can_use_n1_test {
        let result = if base == 2 {
            proth_test(candidate)
        } else {
            pocklington_test(candidate, base)
        };

        match result {
            Some((true, witness_base)) => {
                let cert = PrimalityCertificate::Proth {
                    base: witness_base.unwrap_or(0),
                };
                return (IsPrime::Yes, "deterministic", Some(cert));
            }
            Some((false, _)) => return (IsPrime::No, "", None),
            None => {} // fall through to Miller-Rabin
        }
    }

    // LLR test for k*2^n - 1 where k is odd and k < 2^n
    if !is_plus && base == 2 && k % 2 == 1 {
        let can_use_llr = if n >= 64 { true } else { k < (1u64 << n) };
        if can_use_llr {
            // Quick MR pre-screen (1 round) rejects ~75% of composites before
            // the expensive O(n-2) LLR squaring loop, for the cost of a single
            // modular exponentiation.
            if candidate.is_probably_prime(1) == IsPrime::No {
                return (IsPrime::No, "", None);
            }
            match llr_test(candidate, k, n) {
                Some((true, seed)) => {
                    let cert = PrimalityCertificate::Llr {
                        k,
                        n,
                        seed: seed.unwrap_or_default(),
                    };
                    return (IsPrime::Yes, "deterministic", Some(cert));
                }
                Some((false, _)) => return (IsPrime::No, "", None),
                None => {} // fall through to Miller-Rabin
            }
        }
    }

    // Adaptive P-1 composite pre-filter — auto-tunes B1/B2 by candidate size,
    // uses Stage 1 + Stage 2 to catch composites with one partially-smooth factor.
    if crate::p1::adaptive_p1_filter(candidate) {
        return (IsPrime::No, "", None);
    }

    // Try GWNUM direct FFI for large candidates (when --features gwnum is enabled)
    #[cfg(feature = "gwnum")]
    {
        let digits = crate::estimate_digits(candidate);
        if digits >= 10_000 {
            if is_plus && base == 2 {
                match crate::gwnum::gwnum_proth(k, base, n) {
                    Ok(Some(true)) => return (IsPrime::Yes, "deterministic", None),
                    Ok(Some(false)) => return (IsPrime::No, "", None),
                    Ok(None) | Err(_) => {} // fall through
                }
            } else if !is_plus && base == 2 && k % 2 == 1 {
                match crate::gwnum::gwnum_llr(k, n) {
                    Ok(Some(true)) => return (IsPrime::Yes, "deterministic", None),
                    Ok(Some(false)) => return (IsPrime::No, "", None),
                    Ok(None) | Err(_) => {} // fall through
                }
            }
        }
    }

    // Try PRST for large candidates (50-100x faster than GMP for large numbers)
    if let Some(result) = crate::prst::try_test(k, base, n, is_plus, candidate) {
        match result {
            crate::prst::PrstResult::Prime {
                is_deterministic, ..
            } => {
                let cert = Some(PrimalityCertificate::Prst {
                    method: format!("k={}*{}^{}{}1", k, base, n, if is_plus { "+" } else { "-" }),
                });
                if is_deterministic {
                    return (IsPrime::Yes, "deterministic", cert);
                } else {
                    return (IsPrime::Probably, "probabilistic", cert);
                }
            }
            crate::prst::PrstResult::Composite => return (IsPrime::No, "", None),
            crate::prst::PrstResult::Unavailable { .. } => {} // fall through to MR
        }
    }

    // Two-round MR pre-screen before full Miller-Rabin
    if mr_rounds > 2 && candidate.is_probably_prime(2) == IsPrime::No {
        return (IsPrime::No, "", None);
    }

    // Standard Miller-Rabin
    let r = candidate.is_probably_prime(mr_rounds);
    let (cert_label, cert) = match r {
        IsPrime::Yes => ("deterministic", None),
        IsPrime::Probably => (
            "probabilistic",
            Some(PrimalityCertificate::MillerRabin { rounds: mr_rounds }),
        ),
        IsPrime::No => ("", None),
    };
    (r, cert_label, cert)
}

/// BSGS-based sieve: for each sieve prime, compute the discrete log to find
/// all n-values where k*b^n ≡ ∓1 (mod p), then mark them as composite.
/// Returns (plus_survives, minus_survives) bitmaps indexed by (n - min_n).
pub(crate) fn bsgs_sieve(
    min_n: u64,
    max_n: u64,
    k: u64,
    base: u32,
    sieve_primes: &[u64],
    sieve_min_n: u64,
) -> (sieve::BitSieve, sieve::BitSieve) {
    let range = (max_n - min_n + 1) as usize;
    let mut plus_survives = sieve::BitSieve::new_all_set(range);
    let mut minus_survives = sieve::BitSieve::new_all_set(range);

    let base_u64 = base as u64;
    let total_primes = sieve_primes.len();
    let log_interval = (total_primes / 20).max(1); // every 5%

    for (pi, &p) in sieve_primes.iter().enumerate() {
        if pi % log_interval == 0 && pi > 0 {
            debug!(
                current = pi,
                total = total_primes,
                percent = format_args!("{:.0}", pi as f64 / total_primes as f64 * 100.0),
                "BSGS sieve progress"
            );
        }

        // Skip if p divides base or k — neither form is divisible by p
        if base_u64.is_multiple_of(p) || k.is_multiple_of(p) {
            continue;
        }

        let k_inv = match sieve::mod_inverse(k, p) {
            Some(v) => v,
            None => continue,
        };

        let order = sieve::multiplicative_order(base_u64, p);

        // +1 form: k*b^n + 1 ≡ 0 (mod p) → b^n ≡ -k^{-1} (mod p)
        let neg_k_inv = p - k_inv; // -k_inv mod p
        if let Some(n0) = sieve::discrete_log_bsgs(base_u64, neg_k_inv, p, order) {
            // All n = n0 + i*order in range are composite for +1 form
            let first = if n0 >= min_n {
                n0
            } else if order == 0 {
                continue;
            } else {
                let gap = min_n - n0;
                let steps = gap.div_ceil(order);
                n0 + steps * order
            };
            let mut n = first;
            while n <= max_n {
                if n >= sieve_min_n {
                    plus_survives.clear((n - min_n) as usize);
                }
                n += order;
            }
        }

        // -1 form: k*b^n - 1 ≡ 0 (mod p) → b^n ≡ k^{-1} (mod p)
        if let Some(n0) = sieve::discrete_log_bsgs(base_u64, k_inv, p, order) {
            let first = if n0 >= min_n {
                n0
            } else if order == 0 {
                continue;
            } else {
                let gap = min_n - n0;
                let steps = gap.div_ceil(order);
                n0 + steps * order
            };
            let mut n = first;
            while n <= max_n {
                if n >= sieve_min_n {
                    minus_survives.clear((n - min_n) as usize);
                }
                n += order;
            }
        }
    }

    (plus_survives, minus_survives)
}

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
    info!(
        prime_count = sieve_primes.len(),
        sieve_limit,
        "Sieve initialized"
    );

    let resume_from = match checkpoint::load(checkpoint_path) {
        Some(Checkpoint::Kbn { last_n, .. }) if last_n >= min_n && last_n < max_n => {
            info!(resume_n = last_n + 1, "Resuming kbn search");
            last_n + 1
        }
        _ => min_n,
    };

    // Minimum n where k*b^n > sieve_limit, making the sieve safe.
    let sieve_min_n = if base >= 2 {
        let log_b = (base as f64).log10();
        let log_limit = (sieve_limit as f64).log10();
        ((log_limit - (k as f64).log10().max(0.0)) / log_b).ceil() as u64 + 1
    } else {
        u64::MAX
    };
    info!(sieve_min_n, "Sieve active");

    // Run BSGS sieve once over the entire range
    info!(
        min_n = resume_from,
        max_n,
        candidates = max_n - resume_from + 1,
        "Running BSGS sieve"
    );
    let (plus_survives, minus_survives) =
        bsgs_sieve(resume_from, max_n, k, base, &sieve_primes, sieve_min_n);
    let bsgs_plus_survivors = plus_survives.count_ones() as u64;
    let bsgs_minus_survivors = minus_survives.count_ones() as u64;
    let total_range = max_n - resume_from + 1;
    info!(
        plus_survivors = bsgs_plus_survivors,
        minus_survivors = bsgs_minus_survivors,
        total_range,
        plus_pct = format_args!("{:.1}", bsgs_plus_survivors as f64 / total_range as f64 * 100.0),
        minus_pct = format_args!("{:.1}", bsgs_minus_survivors as f64 / total_range as f64 * 100.0),
        "BSGS sieve complete"
    );

    let mut last_checkpoint = Instant::now();
    let mut block_start = resume_from;
    let mut total_sieved: u64 = 0;

    while block_start <= max_n {
        let bsize = crate::block_size_for_n(block_start);
        let block_end = (block_start + bsize - 1).min(max_n);
        let block_len = block_end - block_start + 1;

        *progress.current.lock().unwrap() =
            format!("{}*{}^[{}..{}]+-1", k, base, block_start, block_end);

        let survivors: Vec<(u64, bool, bool)> = (block_start..=block_end)
            .filter_map(|n| {
                let idx = (n - resume_from) as usize;
                let tp = plus_survives.get(idx);
                let tm = minus_survives.get(idx);
                if tp || tm {
                    Some((n, tp, tm))
                } else {
                    None
                }
            })
            .collect();

        total_sieved += block_len - survivors.len() as u64;

        // Pre-compute b^block_start once; each survivor computes b^offset (much smaller)
        let base_pow_start = Integer::from(base).pow(crate::checked_u32(block_start));
        let k_int = Integer::from(k);

        let found_primes: Vec<_> = survivors
            .into_par_iter()
            .flat_map_iter(|(n, test_plus, test_minus)| {
                let offset = n - block_start;
                let base_pow = if offset == 0 {
                    base_pow_start.clone()
                } else {
                    &base_pow_start * Integer::from(base).pow(crate::checked_u32(offset))
                };
                let kb = Integer::from(&k_int * &base_pow);

                let plus_result = if test_plus {
                    let plus = Integer::from(&kb + 1u32);
                    let (r, cert_label, certificate) =
                        test_prime(&plus, k, base, n, true, mr_rounds);
                    if r != IsPrime::No {
                        let digits = exact_digits(&plus);
                        let cert_json = certificate
                            .as_ref()
                            .and_then(|c| serde_json::to_string(c).ok());
                        Some((
                            format!("{}*{}^{} + 1", k, base, n),
                            digits,
                            cert_label.to_string(),
                            cert_json,
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                };

                let minus_result = if test_minus {
                    let minus = Integer::from(&kb - 1u32);
                    let (r, cert_label, certificate) =
                        test_prime(&minus, k, base, n, false, mr_rounds);
                    if r != IsPrime::No {
                        let digits = exact_digits(&minus);
                        let cert_json = certificate
                            .as_ref()
                            .and_then(|c| serde_json::to_string(c).ok());
                        Some((
                            format!("{}*{}^{} - 1", k, base, n),
                            digits,
                            cert_label.to_string(),
                            cert_json,
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                };

                plus_result.into_iter().chain(minus_result)
            })
            .collect();

        progress.tested.fetch_add(block_len * 2, Ordering::Relaxed);

        for (expr, digits, certainty, cert_json) in found_primes {
            progress.found.fetch_add(1, Ordering::Relaxed);
            if let Some(eb) = event_bus {
                eb.emit(events::Event::PrimeFound {
                    form: "kbn".into(),
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
                    "Prime found"
                );
            }
            db.insert_prime_sync(
                rt,
                "kbn",
                &expr,
                digits,
                search_params,
                &certainty,
                cert_json.as_deref(),
            )?;
            if let Some(wc) = worker_client {
                wc.report_prime("kbn", &expr, digits, search_params, &certainty);
            }
        }

        if last_checkpoint.elapsed().as_secs() >= 60 {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Kbn {
                    last_n: block_end,
                    min_n: Some(min_n),
                    max_n: Some(max_n),
                },
            )?;
            info!(
                n = block_end,
                sieved_out = total_sieved,
                "Checkpoint saved"
            );
            last_checkpoint = Instant::now();
        }

        if worker_client.is_some_and(|wc| wc.is_stop_requested()) {
            checkpoint::save(
                checkpoint_path,
                &Checkpoint::Kbn {
                    last_n: block_end,
                    min_n: Some(min_n),
                    max_n: Some(max_n),
                },
            )?;
            info!(
                n = block_end,
                "Stop requested by coordinator, checkpoint saved"
            );
            return Ok(());
        }

        block_start = block_end + 1;
    }

    checkpoint::clear(checkpoint_path);
    info!(eliminated = total_sieved, "KBN sieve complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    //! # k*b^n +/- 1 Primality Test Suite
    //!
    //! Validates the three deterministic primality tests and the BSGS sieve
    //! for the k*b^n +/- 1 family of prime forms:
    //!
    //! - **LLR test** (`llr_test`): Lucas-Lehmer-Riesel deterministic test for
    //!   k*2^n - 1. Requires finding a Lucas V-sequence parameter P via
    //!   Rodseth's method when k is divisible by 3, or the standard Jacobi
    //!   symbol selection otherwise. Tests against known Mersenne primes
    //!   (OEIS [A000668](https://oeis.org/A000668)): M_3=7, M_5=31, M_7=127,
    //!   M_13=8191, M_17=131071, M_19=524287, M_31=2147483647.
    //!
    //! - **Proth test** (`proth_test`): Deterministic test for k*2^n + 1 where
    //!   k < 2^n. Based on Proth's theorem (1878): N is prime iff there exists
    //!   a such that a^{(N-1)/2} = -1 (mod N). Tests with the smallest Proth
    //!   prime 13 = 3*2^2 + 1.
    //!
    //! - **Riesel primes**: k*2^n - 1 for k > 1, tested via LLR. Includes
    //!   k=3 (which triggers the Rodseth path for finding the Lucas parameter P
    //!   when k mod 3 = 0) and k=5.
    //!
    //! - **BSGS sieve** (`bsgs_sieve`): Baby-step Giant-step sieve that eliminates
    //!   candidates divisible by small primes in O(sqrt(ord) * |primes|) time
    //!   instead of O(ord * |primes|). Cross-validated against a naive reference
    //!   implementation for k in {1, 3, 5, 7} and base in {2, 3, 10}.
    //!
    //! - **test_prime integration**: Verifies the unified entry point returns
    //!   correct (IsPrime, proof_method, Option<Certificate>) tuples.
    //!
    //! - **Lucas V-sequence** (`lucas_v_k`): Binary-chain computation of V_k(P, 1)
    //!   mod N used as the LLR initial seed. Verified at base cases V_0=2, V_1=P
    //!   and small values V_2, V_3.
    //!
    //! ## References
    //!
    //! - Francois Proth, "Theoremes sur les nombres premiers", 1878.
    //! - D.H. Lehmer, "An Extended Theory of Lucas' Functions", 1930.
    //! - Hans Riesel, "Lucasian Criteria for the Primality of N = h*2^n - 1", 1969.
    //! - Oystein J. Rodseth, "A Note on Primality Testing Using Lucas Sequences", 1994.
    //! - OEIS A000668: Mersenne primes 2^p - 1.
    //! - OEIS A001770: Primes of form 3*2^n - 1.

    use super::*;
    use rug::Integer;

    /// Helper: build N = k*2^n - 1 for testing the minus-form of kbn.
    fn make_candidate(k: u64, n: u64) -> Integer {
        Integer::from(k) * Integer::from(Integer::from(2u32).pow(crate::checked_u32(n))) - 1u32
    }

    // ── Mersenne Primes (k=1, base=2, minus form) ─────────────────────

    /// Tests LLR on known Mersenne primes M_p = 2^p - 1 for p in {3, 5, 7, 13,
    /// 17, 19, 31} (OEIS [A000668](https://oeis.org/A000668)). The LLR test
    /// computes the sequence s_i = s_{i-1}^2 - 2 (mod M_p) starting from
    /// s_0 = V_k(P, 1) mod M_p. If s_{n-2} = 0, then M_p is prime.
    ///
    /// For k=1, the Lucas parameter P defaults to 4 (the classical Lucas-Lehmer
    /// seed), giving s_0 = V_1(4, 1) = 4. Each result should be Some((true, Some(seed)))
    /// where the seed is the LLR certificate value.
    #[test]
    fn llr_mersenne_primes() {
        for &n in &[3u64, 5, 7, 13, 17, 19, 31] {
            let candidate = make_candidate(1, n);
            let result = llr_test(&candidate, 1, n);
            assert!(
                matches!(result, Some((true, Some(_)))),
                "2^{} - 1 should be prime",
                n
            );
        }
    }

    /// Tests LLR on composite Mersenne numbers 2^n - 1 for n in {4, 6, 8, 11}:
    /// - 2^4 - 1 = 15 = 3*5 (n=4 is not prime, and M_4 is composite).
    /// - 2^6 - 1 = 63 = 7*9 (n=6 is composite).
    /// - 2^8 - 1 = 255 = 3*5*17 (n=8 is composite).
    /// - 2^{11} - 1 = 2047 = 23*89 (n=11 is prime but M_{11} is composite).
    ///
    /// 2047 is particularly important: it's the smallest composite Mersenne
    /// number with prime exponent, and also a strong pseudoprime to base 2.
    /// Each result should be Some((false, None)).
    #[test]
    fn llr_mersenne_composites() {
        for &n in &[4u64, 6, 8, 11] {
            let candidate = make_candidate(1, n);
            let result = llr_test(&candidate, 1, n);
            assert!(
                matches!(result, Some((false, None))),
                "2^{} - 1 should be composite",
                n
            );
        }
    }

    // ── Riesel Primes (k > 1, base=2, minus form) ─────────────────────

    /// Tests LLR on primes of the form 3*2^n - 1 (OEIS [A001770](https://oeis.org/A001770))
    /// for n in {3, 4, 6, 7, 11, 18}. k=3 is divisible by 3, which triggers
    /// the `find_rodseth_v1` code path for Lucas parameter selection.
    ///
    /// When k mod 3 = 0, the standard Jacobi-symbol method for choosing P
    /// may fail because the discriminant P^2 - 4 factors in a way that makes
    /// the Jacobi symbol trivially 1. Rodseth's method (1994) finds P such that
    /// Jacobi(P-2, N) = 1 and Jacobi(P+2, N) = -1, which guarantees the
    /// Lucas sequence has the correct algebraic properties for the LLR test.
    #[test]
    fn llr_riesel_k3_primes() {
        // k=3 is divisible by 3, so this exercises find_rodseth_v1
        for &n in &[3u64, 4, 6, 7, 11, 18] {
            let candidate = make_candidate(3, n);
            let result = llr_test(&candidate, 3, n);
            assert!(
                matches!(result, Some((true, Some(_)))),
                "3*2^{} - 1 should be prime",
                n
            );
        }
    }

    /// Composite values of 3*2^n - 1 for n in {5, 8}:
    /// - 3*2^5 - 1 = 95 = 5*19.
    /// - 3*2^8 - 1 = 767 = 11*69 + 8 (actually 767 = 11*69 + 8... let's check:
    ///   767/11 = 69.7, so 767 is not 11*69+8. 767 = 7*109 + 4... actually
    ///   767 = 13*59). The exact factorization doesn't matter -- LLR must reject them.
    #[test]
    fn llr_riesel_k3_composites() {
        for &n in &[5u64, 8] {
            let candidate = make_candidate(3, n);
            let result = llr_test(&candidate, 3, n);
            assert!(
                matches!(result, Some((false, None))),
                "3*2^{} - 1 should be composite",
                n
            );
        }
    }

    /// Primes of the form 5*2^n - 1 for n in {4, 8, 10}:
    /// - 5*2^4 - 1 = 79 (prime).
    /// - 5*2^8 - 1 = 1279 (prime, also a Mersenne exponent: M_{1279} is unknown).
    /// - 5*2^{10} - 1 = 5119 (prime).
    ///
    /// k=5 does NOT trigger the Rodseth path (5 mod 3 = 2 != 0), so the standard
    /// Jacobi-symbol method is used for P selection.
    #[test]
    fn llr_riesel_k5_primes() {
        for &n in &[4u64, 8, 10] {
            let candidate = make_candidate(5, n);
            let result = llr_test(&candidate, 5, n);
            assert!(
                matches!(result, Some((true, Some(_)))),
                "5*2^{} - 1 should be prime",
                n
            );
        }
    }

    // ── LLR Edge Cases ────────────────────────────────────────────────

    /// For n < 3, the LLR sequence has fewer than n-2 = 0 or 1 iterations,
    /// making the test degenerate. The function returns None to signal that
    /// the caller should fall back to Miller-Rabin. Tests n=1 (2^1-1=1, not
    /// prime) and n=2 (2^2-1=3, prime but too small for LLR).
    #[test]
    fn llr_small_n_returns_none() {
        // n < 3 should return None (fall back to MR)
        for &n in &[1u64, 2] {
            let candidate = make_candidate(1, n);
            let result = llr_test(&candidate, 1, n);
            assert_eq!(result, None, "n={} should return None", n);
        }
    }

    /// Verifies that `find_rodseth_v1` produces a valid Lucas parameter P for
    /// k=3 (divisible by 3). The Rodseth conditions are:
    /// (1) P > 4 (to avoid degenerate cases).
    /// (2) Jacobi(P-2, N) = +1 (P-2 is a quadratic residue mod N).
    /// (3) Jacobi(P+2, N) = -1 (P+2 is a quadratic non-residue mod N).
    ///
    /// These conditions ensure the Lucas V-sequence V_k(P, 1) generates the
    /// correct algebraic structure in Z/NZ for the LLR primality criterion.
    #[test]
    fn llr_rodseth_path_used_for_k_div_3() {
        // k=3 triggers find_rodseth_v1; verify it finds a valid P
        let candidate = make_candidate(3, 11);
        let p = find_rodseth_v1(&candidate);
        assert!(p > 4, "Rodseth should find P > 4 for k divisible by 3");
        let pm2 = Integer::from(p - 2);
        let pp2 = Integer::from(p + 2);
        assert_eq!(pm2.jacobi(&candidate), 1);
        assert_eq!(pp2.jacobi(&candidate), -1);
    }

    // ── test_prime Integration (Unified Entry Point) ───────────────────

    /// The unified `test_prime` function routes k=1, base=2, is_plus=false to
    /// the LLR test and should return a "deterministic" proof method with an
    /// LLR certificate. Tests with M_{31} = 2^{31} - 1 = 2147483647, the
    /// largest Mersenne prime that fits in u32 and was discovered by Euler (1772).
    #[test]
    fn test_prime_llr_deterministic() {
        // 2^31 - 1 = 2147483647 (Mersenne prime)
        let candidate = make_candidate(1, 31);
        let (result, cert, certificate) = test_prime(&candidate, 1, 2, 31, false, 25);
        assert_eq!(result, IsPrime::Yes);
        assert_eq!(cert, "deterministic");
        assert!(matches!(
            certificate,
            Some(PrimalityCertificate::Llr { .. })
        ));
    }

    /// test_prime must return IsPrime::No for the composite Mersenne number
    /// 2^{11} - 1 = 2047 = 23*89. The MR pre-screen catches this before the
    /// expensive LLR iteration even begins.
    #[test]
    fn test_prime_llr_composite() {
        // 2^11 - 1 = 2047 = 23 * 89 (composite)
        let candidate = make_candidate(1, 11);
        let (result, _, _) = test_prime(&candidate, 1, 2, 11, false, 25);
        assert_eq!(result, IsPrime::No);
    }

    /// For non-base-2 forms (base=3 here), test_prime cannot use LLR or Proth
    /// (which require base=2), so it falls back to MR. The result should NOT be
    /// "deterministic". Tests with 3*3^5 - 1 = 728, which is composite (728 = 2^3*7*13).
    #[test]
    fn test_prime_non_base2_still_probabilistic() {
        // 3*3^5 - 1 = 728, not prime; but check that non-base-2 doesn't use LLR
        let candidate = Integer::from(3u32) * Integer::from(3u32).pow(5) - 1u32;
        let (result, cert, _) = test_prime(&candidate, 3, 3, 5, false, 25);
        // Should not be "deterministic" from LLR (it's not base-2)
        assert_eq!(result, IsPrime::No);
        assert_ne!(cert, "deterministic");
    }

    // ── Lucas V-Sequence (lucas_v_k) ───────────────────────────────────

    /// Verifies the Lucas V-sequence base cases defined by the recurrence
    /// V_k(P, Q) with Q=1:
    /// - V_0(P, 1) = 2 for all P (the constant term of the characteristic polynomial).
    /// - V_1(P, 1) = P (the trace of the companion matrix).
    ///
    /// These base cases initialize the binary chain doubling algorithm:
    /// V_{2m} = V_m^2 - 2, V_{2m+1} = V_m * V_{m+1} - P.
    #[test]
    fn lucas_v_k_base_cases() {
        let n = Integer::from(101u32);
        // V_0(P, 1) = 2
        assert_eq!(lucas_v_k(0, 4, &n), Integer::from(2u32));
        // V_1(P, 1) = P mod N
        assert_eq!(lucas_v_k(1, 4, &n), Integer::from(4u32));
    }

    /// Verifies small Lucas V values computed by hand:
    /// - V_2(4, 1) = V_1^2 - 2*Q^1 = 4^2 - 2 = 14 (using the doubling formula).
    /// - V_3(4, 1) = V_1 * V_2 - P*Q^1 = 4*14 - 4 = 52.
    ///
    /// Uses modulus N=1000 (large enough that no reduction occurs for these
    /// small values), verifying the algorithm against pure arithmetic.
    #[test]
    fn lucas_v_k_small_values() {
        // V_2(4, 1) = 4^2 - 2 = 14
        let n = Integer::from(1000u32);
        assert_eq!(lucas_v_k(2, 4, &n), Integer::from(14u32));
        // V_3(4, 1) = 4*14 - 4 = 52
        assert_eq!(lucas_v_k(3, 4, &n), Integer::from(52u32));
    }

    // ── BSGS Sieve Cross-Validation ────────────────────────────────────

    /// Reference (naive) sieve implementation with O(|primes| * block_size) complexity.
    /// For each sieve prime p and each n in the block, computes k*b^n mod p
    /// directly and checks if k*b^n + 1 = 0 (mod p) or k*b^n - 1 = 0 (mod p).
    ///
    /// This is used as the ground truth to validate the BSGS sieve, which
    /// achieves the same result in O(|primes| * sqrt(ord_p(b))) time by
    /// precomputing baby steps and looking up giant steps.
    fn sieve_block(
        block_start: u64,
        block_end: u64,
        k: u64,
        base: u32,
        sieve_primes: &[u64],
        sieve_min_n: u64,
    ) -> Vec<(u64, bool, bool)> {
        let base_u64 = base as u64;
        let k_mod: Vec<u64> = sieve_primes.iter().map(|&p| k % p).collect();
        let b_mod: Vec<u64> = sieve_primes.iter().map(|&p| base_u64 % p).collect();
        let mut b_pow: Vec<u64> = sieve_primes
            .iter()
            .map(|&p| sieve::pow_mod(base_u64, block_start, p))
            .collect();

        let mut survivors = Vec::new();
        for n in block_start..=block_end {
            if n < sieve_min_n {
                survivors.push((n, true, true));
            } else {
                let mut plus_survives = true;
                let mut minus_survives = true;
                for i in 0..sieve_primes.len() {
                    let p = sieve_primes[i];
                    let kb_mod = k_mod[i] * b_pow[i] % p;
                    if plus_survives && kb_mod == p - 1 {
                        plus_survives = false;
                    }
                    if minus_survives && kb_mod == 1 {
                        minus_survives = false;
                    }
                    if !plus_survives && !minus_survives {
                        break;
                    }
                }
                if plus_survives || minus_survives {
                    survivors.push((n, plus_survives, minus_survives));
                }
            }
            for i in 0..sieve_primes.len() {
                b_pow[i] = b_pow[i] * b_mod[i] % sieve_primes[i];
            }
        }
        survivors
    }

    /// Exhaustive cross-validation of `bsgs_sieve` against the naive `sieve_block`
    /// reference for 4 parameter combinations:
    /// - (k=1, base=2): Mersenne form, the most common search.
    /// - (k=3, base=2): Riesel form with k divisible by 3.
    /// - (k=5, base=10): Decimal Riesel, tests non-power-of-2 base.
    /// - (k=7, base=3): Ternary form.
    ///
    /// For each combination, sieve n in [1, 500] using all primes up to 10000
    /// (~1229 primes). Every (n, +1_survives, -1_survives) triple from the BSGS
    /// sieve must exactly match the naive implementation. Any mismatch indicates
    /// a bug in the discrete log computation, baby/giant step tables, or the
    /// Montgomery-accelerated modular arithmetic within the BSGS sieve.
    #[test]
    fn bsgs_matches_sieve_block() {
        let sieve_primes = sieve::generate_primes(10_000);

        for &(k, base) in &[(1u64, 2u32), (3, 2), (5, 10), (7, 3)] {
            let min_n = 1u64;
            let max_n = 500u64;

            // Compute sieve_min_n the same way search() does
            let sieve_min_n = {
                let log_b = (base as f64).log10();
                let log_limit = (10_000f64).log10();
                ((log_limit - (k as f64).log10().max(0.0)) / log_b).ceil() as u64 + 1
            };

            let (bsgs_plus, bsgs_minus) =
                bsgs_sieve(min_n, max_n, k, base, &sieve_primes, sieve_min_n);
            let old_survivors = sieve_block(min_n, max_n, k, base, &sieve_primes, sieve_min_n);

            // Build equivalent maps from the old sieve_block output
            let range = (max_n - min_n + 1) as usize;
            let mut old_plus = vec![false; range];
            let mut old_minus = vec![false; range];
            for (n, tp, tm) in &old_survivors {
                let idx = (n - min_n) as usize;
                old_plus[idx] = *tp;
                old_minus[idx] = *tm;
            }

            for n in min_n..=max_n {
                let idx = (n - min_n) as usize;
                assert_eq!(
                    bsgs_plus.get(idx),
                    old_plus[idx],
                    "k={} base={} n={}: +1 mismatch (bsgs={}, old={})",
                    k,
                    base,
                    n,
                    bsgs_plus.get(idx),
                    old_plus[idx]
                );
                assert_eq!(
                    bsgs_minus.get(idx),
                    old_minus[idx],
                    "k={} base={} n={}: -1 mismatch (bsgs={}, old={})",
                    k,
                    base,
                    n,
                    bsgs_minus.get(idx),
                    old_minus[idx]
                );
            }
        }
    }

    // ── MR Pre-Screen Integration ──────────────────────────────────────

    /// Verifies that the MR pre-screen rejects the composite 2^{11} - 1 = 2047
    /// BEFORE entering the expensive LLR iteration loop. This is a performance
    /// optimization: a single MR round takes O(n^2) time, while the LLR loop
    /// takes O(n^3) time (n-2 squarings of an n-bit number). For composites,
    /// the pre-screen saves ~n iterations of the LLR sequence.
    ///
    /// 2047 = 23*89 is notable because it's a strong pseudoprime to base 2
    /// (OEIS A001262), but `is_probably_prime(1)` with GMP's implementation
    /// still rejects it because GMP uses multiple deterministic witnesses.
    #[test]
    fn mr_prescreen_rejects_composite_before_llr() {
        // 2^11 - 1 = 2047 = 23 × 89 (composite Mersenne number).
        // The MR pre-screen should reject this before entering the expensive LLR loop.
        let candidate = make_candidate(1, 11);
        assert_eq!(candidate, Integer::from(2047u32));
        // Verify the pre-screen catches it (1-round MR rejects composites)
        assert_eq!(candidate.is_probably_prime(1), IsPrime::No);
        // And test_prime returns No
        let (result, _, _) = test_prime(&candidate, 1, 2, 11, false, 25);
        assert_eq!(result, IsPrime::No);
    }

    /// Verifies that the MR pre-screen does NOT reject the Mersenne prime
    /// M_{13} = 2^{13} - 1 = 8191. The pre-screen must pass true primes through
    /// to the LLR test, which then produces a deterministic proof. If the
    /// pre-screen incorrectly rejected 8191, we would lose a provable prime
    /// and only get a probabilistic result (or worse, miss it entirely).
    #[test]
    fn mr_prescreen_passes_primes_through() {
        // M_13 = 2^13 - 1 = 8191 (Mersenne prime).
        // The MR pre-screen must not reject this — it should pass through to LLR
        // and return a deterministic proof.
        let candidate = make_candidate(1, 13);
        assert_eq!(candidate, Integer::from(8191u32));
        assert_ne!(candidate.is_probably_prime(1), IsPrime::No);
        let (result, cert, _) = test_prime(&candidate, 1, 2, 13, false, 25);
        assert_eq!(result, IsPrime::Yes);
        assert_eq!(cert, "deterministic");
    }

    // ── Certificate Witness Tests ──────────────────────────────────────

    /// Verifies that `proth_test` returns a non-zero witness base for the Proth
    /// prime 13 = 3*2^2 + 1. The witness base `a` satisfies Proth's criterion:
    /// a^{(N-1)/2} = -1 (mod N). For N=13, (N-1)/2 = 6, so we need a^6 = 12 (mod 13).
    /// The smallest such base is a=2: 2^6 = 64 = 12 (mod 13). The test verifies
    /// that the returned base is positive (the actual value depends on which base
    /// the implementation tries first).
    #[test]
    fn proth_test_returns_witness_base() {
        // 3*2^2 + 1 = 13 (prime, Proth form with k=3 < 2^2=4)
        let candidate = Integer::from(13u32);
        let result = proth_test(&candidate);
        assert!(
            matches!(result, Some((true, Some(base))) if base > 0),
            "Proth should return witness base for 13"
        );
    }

    /// Verifies that `llr_test` returns a non-empty seed string for the Mersenne
    /// prime M_{13} = 8191. The seed is s_0 = V_k(P, 1) mod N, the initial
    /// value of the LLR iteration sequence. For k=1, P=4 (the standard Lucas-Lehmer
    /// seed), so s_0 = V_1(4, 1) mod 8191 = 4. The seed is stored as a decimal
    /// string in the certificate for independent re-verification.
    #[test]
    fn llr_test_returns_seed() {
        // M_13 = 2^13 - 1 = 8191 (Mersenne prime)
        let candidate = make_candidate(1, 13);
        let result = llr_test(&candidate, 1, 13);
        match result {
            Some((true, Some(seed))) => {
                assert!(!seed.is_empty(), "LLR seed should be non-empty");
            }
            other => panic!("Expected Some((true, Some(seed))), got {:?}", other),
        }
    }

    /// Verifies that `test_prime` returns a Proth certificate for 13 = 3*2^2 + 1
    /// (is_plus=true). The certificate should contain a witness base > 0.
    /// The full return tuple is (IsPrime::Yes, "deterministic", Some(Proth{base})),
    /// confirming that the proof is deterministic (not probabilistic) and includes
    /// exportable witness data.
    #[test]
    fn test_prime_proth_returns_certificate() {
        // 3*2^2 + 1 = 13 (prime)
        let candidate = Integer::from(13u32);
        let (result, cert, certificate) = test_prime(&candidate, 3, 2, 2, true, 25);
        assert_eq!(result, IsPrime::Yes);
        assert_eq!(cert, "deterministic");
        match certificate {
            Some(PrimalityCertificate::Proth { base }) => {
                assert!(base > 0, "Proth certificate should have base > 0");
            }
            other => panic!("Expected Proth certificate, got {:?}", other),
        }
    }

    /// Verifies that `test_prime` returns an LLR certificate for M_{13} = 8191.
    /// The certificate should contain k=1, n=13, and a non-empty seed string.
    /// These three fields are sufficient for independent re-verification:
    /// a verifier computes s_0 = V_k(P, 1) mod N from the seed, then iterates
    /// s_{i+1} = s_i^2 - 2 (mod N) for n-2 steps, checking that s_{n-2} = 0.
    #[test]
    fn test_prime_llr_returns_certificate() {
        // 2^13 - 1 = 8191 (Mersenne prime)
        let candidate = make_candidate(1, 13);
        let (result, cert, certificate) = test_prime(&candidate, 1, 2, 13, false, 25);
        assert_eq!(result, IsPrime::Yes);
        assert_eq!(cert, "deterministic");
        match certificate {
            Some(PrimalityCertificate::Llr { k, n, seed }) => {
                assert_eq!(k, 1);
                assert_eq!(n, 13);
                assert!(!seed.is_empty(), "LLR seed should be non-empty");
            }
            other => panic!("Expected LLR certificate, got {:?}", other),
        }
    }
}
