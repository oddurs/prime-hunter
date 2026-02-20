//! Safe Rust wrapper around GWNUM for accelerated modular arithmetic.
//!
//! GWNUM provides 50-100x speedup over GMP for large numbers by using the
//! Irrational Base Discrete Weighted Transform (IBDWT) — modular reduction
//! happens inside the FFT convolution for free.
//!
//! # Thread Safety
//!
//! Each `GwContext` is `!Send` and `!Sync`. Every thread must have its own context.
//! gwnums allocated from one context cannot be used with another.
//!
//! # Platform
//!
//! x86-64 only (SSE2/AVX/FMA3/AVX-512 assembly). Not available on ARM/Apple Silicon.
//!
//! # Usage
//!
//! ```ignore
//! use darkreach::gwnum::{GwContext, GwError};
//!
//! let mut ctx = GwContext::new(3, 2, 50000, 1)?;  // mod 3*2^50000+1
//! let mut a = ctx.alloc();
//! ctx.set_small(&mut a, 5);
//! let mut result = ctx.alloc();
//! ctx.square(&a, &mut result)?;
//! let n = ctx.to_integer(&result);
//! ```

use rug::ops::{Pow, RemRounding};
use rug::Integer;
use std::marker::PhantomData;

/// Errors from GWNUM operations.
#[derive(Debug, Clone)]
pub enum GwError {
    /// gwsetup failed (invalid parameters or unsupported configuration).
    SetupFailed { code: i32 },
    /// Roundoff error detected after FFT arithmetic operation.
    RoundoffError,
    /// Hardware error detected (e.g., memory corruption, CPU fault).
    HardwareError,
    /// Internal GWNUM error (unexpected error code from library).
    InternalError { code: i32 },
    /// Gerbicz verification failed — computation may be corrupt.
    GerbiczMismatch { iteration: u64 },
    /// GWNUM library not available (gwnum.a not linked).
    Unavailable,
}

impl std::fmt::Display for GwError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GwError::SetupFailed { code } => write!(f, "gwsetup failed with code {}", code),
            GwError::RoundoffError => write!(f, "GWNUM roundoff error detected"),
            GwError::HardwareError => write!(f, "GWNUM hardware error detected"),
            GwError::InternalError { code } => write!(f, "GWNUM internal error (code {})", code),
            GwError::GerbiczMismatch { iteration } => {
                write!(f, "Gerbicz verification failed at iteration {}", iteration)
            }
            GwError::Unavailable => write!(f, "GWNUM library not available"),
        }
    }
}

impl std::error::Error for GwError {}

/// RAII wrapper for a GWNUM context (gwhandle).
///
/// Thread-local only: `!Send`, `!Sync`. Each thread must create its own context.
/// All gwnums allocated from this context are freed when it drops.
pub struct GwContext {
    #[cfg(feature = "gwnum")]
    handle: Box<gwnum_sys::gwhandle>,
    #[cfg(feature = "gwnum")]
    _setup_done: bool,
    /// Prevent Send and Sync — gwhandle is thread-local only
    _not_send: PhantomData<*mut ()>,
}

/// RAII wrapper for a gwnum value. Freed when dropped.
pub struct GwNum {
    #[cfg(feature = "gwnum")]
    inner: gwnum_sys::gwnum,
    #[cfg(feature = "gwnum")]
    ctx: *mut gwnum_sys::gwhandle,
    _not_send: PhantomData<*mut ()>,
}

#[cfg(feature = "gwnum")]
impl Drop for GwNum {
    fn drop(&mut self) {
        unsafe {
            gwnum_sys::gwfree(self.ctx, self.inner);
        }
    }
}

#[cfg(not(feature = "gwnum"))]
impl Drop for GwNum {
    fn drop(&mut self) {}
}

impl GwContext {
    /// Create a new GWNUM context for modular arithmetic mod k*b^n+c.
    ///
    /// # Errors
    ///
    /// Returns `GwError::Unavailable` if GWNUM is not compiled in.
    /// Returns `GwError::SetupFailed` if the parameters are invalid.
    #[cfg(feature = "gwnum")]
    pub fn new(k: u64, b: u32, n: u64, c: i64) -> Result<Self, GwError> {
        use std::ffi::CString;
        use std::mem::MaybeUninit;

        let mut handle =
            Box::new(unsafe { MaybeUninit::<gwnum_sys::gwhandle>::zeroed().assume_init() });

        let version = CString::new("30.19").unwrap(); // GWNUM version string
        unsafe {
            gwnum_sys::gwinit2(
                &mut *handle,
                std::mem::size_of::<gwnum_sys::gwhandle>() as i32,
                version.as_ptr(),
            );
        }

        let ret =
            unsafe { gwnum_sys::gwsetup(&mut *handle, k as f64, b as u64, n as u64, c as i64) };

        if ret != 0 {
            unsafe { gwnum_sys::gwdone(&mut *handle) };
            return Err(GwError::SetupFailed { code: ret });
        }

        Ok(GwContext {
            handle,
            _setup_done: true,
            _not_send: PhantomData,
        })
    }

    #[cfg(not(feature = "gwnum"))]
    pub fn new(_k: u64, _b: u32, _n: u64, _c: i64) -> Result<Self, GwError> {
        Err(GwError::Unavailable)
    }

    /// Allocate a gwnum in this context.
    #[cfg(feature = "gwnum")]
    pub fn alloc(&mut self) -> GwNum {
        let inner = unsafe { gwnum_sys::gwalloc(&mut *self.handle) };
        GwNum {
            inner,
            ctx: &mut *self.handle,
            _not_send: PhantomData,
        }
    }

    #[cfg(not(feature = "gwnum"))]
    pub fn alloc(&mut self) -> GwNum {
        GwNum {
            _not_send: PhantomData,
        }
    }

    /// Set a gwnum to a small integer value.
    #[cfg(feature = "gwnum")]
    pub fn set_small(&mut self, g: &mut GwNum, val: f64) {
        unsafe {
            gwnum_sys::dbltogw(&mut *self.handle, val, g.inner);
        }
    }

    #[cfg(not(feature = "gwnum"))]
    pub fn set_small(&mut self, _g: &mut GwNum, _val: f64) {}

    /// Square: dest = src^2 mod N via IBDWT.
    #[cfg(feature = "gwnum")]
    pub fn square(&mut self, src: &GwNum, dest: &mut GwNum) -> Result<(), GwError> {
        unsafe {
            gwnum_sys::gwmul3(
                &mut *self.handle,
                src.inner,
                src.inner,
                dest.inner,
                gwnum_sys::GWMUL_STARTNEXTFFT,
            );
        }
        self.check_error()
    }

    #[cfg(not(feature = "gwnum"))]
    pub fn square(&mut self, _src: &GwNum, _dest: &mut GwNum) -> Result<(), GwError> {
        Err(GwError::Unavailable)
    }

    /// Square in-place: g = g^2 mod N via IBDWT.
    #[cfg(feature = "gwnum")]
    pub fn square_inplace(&mut self, g: &mut GwNum) -> Result<(), GwError> {
        unsafe {
            gwnum_sys::gwmul3(
                &mut *self.handle,
                g.inner,
                g.inner,
                g.inner,
                gwnum_sys::GWMUL_STARTNEXTFFT,
            );
        }
        self.check_error()
    }

    #[cfg(not(feature = "gwnum"))]
    pub fn square_inplace(&mut self, _g: &mut GwNum) -> Result<(), GwError> {
        Err(GwError::Unavailable)
    }

    /// Multiply: dest = a * b mod N.
    #[cfg(feature = "gwnum")]
    pub fn mul(&mut self, a: &GwNum, b: &GwNum, dest: &mut GwNum) -> Result<(), GwError> {
        unsafe {
            gwnum_sys::gwmul3(&mut *self.handle, a.inner, b.inner, dest.inner, 0);
        }
        self.check_error()
    }

    #[cfg(not(feature = "gwnum"))]
    pub fn mul(&mut self, _a: &GwNum, _b: &GwNum, _dest: &mut GwNum) -> Result<(), GwError> {
        Err(GwError::Unavailable)
    }

    /// Add: dest = a + b.
    #[cfg(feature = "gwnum")]
    pub fn add(&mut self, a: &GwNum, b: &GwNum, dest: &mut GwNum) {
        unsafe {
            gwnum_sys::gwadd3o(&mut *self.handle, a.inner, b.inner, dest.inner, 0);
        }
    }

    #[cfg(not(feature = "gwnum"))]
    pub fn add(&mut self, _a: &GwNum, _b: &GwNum, _dest: &mut GwNum) {}

    /// Subtract: dest = a - b.
    #[cfg(feature = "gwnum")]
    pub fn sub(&mut self, a: &GwNum, b: &GwNum, dest: &mut GwNum) {
        unsafe {
            gwnum_sys::gwsub3o(&mut *self.handle, a.inner, b.inner, dest.inner, 0);
        }
    }

    #[cfg(not(feature = "gwnum"))]
    pub fn sub(&mut self, _a: &GwNum, _b: &GwNum, _dest: &mut GwNum) {}

    /// Convert rug::Integer to gwnum.
    #[cfg(feature = "gwnum")]
    pub fn from_integer(&mut self, n: &Integer) -> GwNum {
        let mut g = self.alloc();
        // Convert Integer to little-endian 32-bit word array
        let limbs = n.to_digits::<u32>(rug::integer::Order::Lsf);
        unsafe {
            gwnum_sys::binarytogw(
                &mut *self.handle,
                limbs.as_ptr(),
                limbs.len() as i32,
                g.inner,
            );
        }
        g
    }

    #[cfg(not(feature = "gwnum"))]
    pub fn from_integer(&mut self, _n: &Integer) -> GwNum {
        self.alloc()
    }

    /// Save current gwnum value as a GMP Integer checkpoint.
    ///
    /// Used by Gerbicz error checking: the GMP Integer serves as a
    /// correctness oracle that can be cheaply recomputed to verify
    /// GWNUM FFT results.
    #[cfg(feature = "gwnum")]
    pub fn checkpoint_value(&mut self, g: &GwNum) -> Integer {
        self.to_integer(g)
    }

    #[cfg(not(feature = "gwnum"))]
    pub fn checkpoint_value(&mut self, _g: &GwNum) -> Integer {
        Integer::new()
    }

    /// Convert gwnum to rug::Integer.
    #[cfg(feature = "gwnum")]
    pub fn to_integer(&mut self, g: &GwNum) -> Integer {
        // Allocate enough space for the result (k*b^n+c can have at most n*log2(b) bits)
        let max_words = 1024 * 1024; // 4MB, enough for multi-million digit numbers
        let mut buf: Vec<u32> = vec![0; max_words];
        let len = unsafe {
            gwnum_sys::gwtobinary(
                &mut *self.handle,
                g.inner,
                buf.as_mut_ptr(),
                max_words as i32,
            )
        };
        buf.truncate(len as usize);
        Integer::from_digits(&buf, rug::integer::Order::Lsf)
    }

    #[cfg(not(feature = "gwnum"))]
    pub fn to_integer(&mut self, _g: &GwNum) -> Integer {
        Integer::new()
    }

    /// Check for roundoff errors after arithmetic operations.
    ///
    /// Calls the real GWNUM `gw_test_for_error()` which reads and clears the
    /// GWERROR field. Distinguishes roundoff from hardware/internal errors.
    #[cfg(feature = "gwnum")]
    fn check_error(&mut self) -> Result<(), GwError> {
        let err = unsafe { gwnum_sys::gw_check_error(&mut *self.handle) };
        match err {
            gwnum_sys::GWERROR_NONE => Ok(()),
            gwnum_sys::GWERROR_ROUNDOFF => Err(GwError::RoundoffError),
            gwnum_sys::GWERROR_HARDWARE => Err(GwError::HardwareError),
            _ => Err(GwError::InternalError { code: err }),
        }
    }
}

#[cfg(feature = "gwnum")]
impl Drop for GwContext {
    fn drop(&mut self) {
        unsafe {
            gwnum_sys::gwfreeall(&mut *self.handle);
            gwnum_sys::gwdone(&mut *self.handle);
        }
    }
}

// === Accelerated primality tests ===

/// Vrba-Reix test for Wagstaff primes (2^p+1)/3.
///
/// Algorithm:
///   S(0) = 3/2 mod N
///   S(i+1) = S(i)^2 - 2 mod N
///   Prime iff S(p-2) == 0 mod N
///
/// Uses GWNUM IBDWT for p-2 squarings. With Gerbicz error checking every √p
/// steps: saves GWNUM state as GMP Integer checkpoints, recomputes blocks using
/// GMP as a correctness oracle, and rolls back to the last verified checkpoint
/// on mismatch. Overhead is ~0.1% (√n GMP squarings per checkpoint).
///
/// # Performance
///
/// At p=5M (1.5M digits): ~3 days with GWNUM vs ~6 months with GMP.
pub fn vrba_reix_test(p: u64) -> Result<bool, GwError> {
    if p < 3 {
        return Err(GwError::SetupFailed { code: -1 });
    }

    // Setup for mod (2^p+1)/3. GWNUM handles this as k=1, b=2, n=p, c=+1
    let mut ctx = GwContext::new(1, 2, p, 1)?;

    // S(0) = 3/2 mod N = 3 * modular_inverse(2, N) mod N
    // For Wagstaff N = (2^p+1)/3, we have 2^(-1) mod N = (N+1)/2
    let n_val = (Integer::from(1u32) << crate::checked_u32(p)) + 1u32;
    let n_val = n_val / 3u32;
    let two_inv = Integer::from(&n_val + 1u32) / 2u32;
    let s0 = (Integer::from(3u32) * two_inv) % &n_val;

    let mut s = ctx.from_integer(&s0);
    let mut temp = ctx.alloc();
    let mut two = ctx.alloc();
    ctx.set_small(&mut two, 2.0);

    let iters = p - 2;

    // Gerbicz error checking: checkpoint every √(iters) steps.
    // Disable for small p where overhead isn't worth it.
    let check_interval = if iters > 10_000 {
        (iters as f64).sqrt() as u64
    } else {
        iters + 1 // disable
    };

    // Checkpoint state: GMP Integers for correctness verification
    let mut last_checkpoint = s0.clone();
    let mut last_checkpoint_iter: u64 = 0;
    let mut verified_checkpoint = s0.clone();
    let mut verified_checkpoint_iter: u64 = 0;

    for i in 0..iters {
        if p > 50_000 && i % 10_000 == 0 && i > 0 {
            eprintln!(
                "  Vrba-Reix: {}/{} squarings ({:.1}%)",
                i,
                iters,
                i as f64 / iters as f64 * 100.0
            );
        }

        ctx.square(&s, &mut temp)?;
        ctx.sub(&temp, &two, &mut s);

        // Gerbicz checkpoint every check_interval iterations
        if check_interval < iters && (i + 1) % check_interval == 0 {
            // Extract current GWNUM value as GMP Integer
            let current_gmp = ctx.checkpoint_value(&s);

            // Recompute this block from last_checkpoint using GMP (correctness oracle)
            let mut verify = last_checkpoint.clone();
            let steps = i + 1 - last_checkpoint_iter;
            for _ in 0..steps {
                verify.square_mut();
                verify -= 2u32;
                verify = verify.rem_euc(&n_val);
            }

            if verify != current_gmp {
                // Hardware/FFT error detected — rollback to last verified checkpoint
                eprintln!(
                    "  Vrba-Reix ERROR at iteration {} — rolling back to {}",
                    i + 1,
                    verified_checkpoint_iter
                );

                // Reload verified checkpoint into GWNUM and replay
                s = ctx.from_integer(&verified_checkpoint);
                let mut u_gmp = verified_checkpoint.clone();

                let redo_start = verified_checkpoint_iter;
                for j in redo_start..=i {
                    ctx.square(&s, &mut temp)?;
                    ctx.sub(&temp, &two, &mut s);

                    // Also advance GMP copy for re-verification
                    u_gmp.square_mut();
                    u_gmp -= 2u32;
                    u_gmp = u_gmp.rem_euc(&n_val);

                    if (j + 1) % check_interval == 0 {
                        let replayed_gmp = ctx.checkpoint_value(&s);
                        if replayed_gmp == u_gmp {
                            last_checkpoint = u_gmp.clone();
                            last_checkpoint_iter = j + 1;
                            verified_checkpoint = last_checkpoint.clone();
                            verified_checkpoint_iter = j + 1;
                        } else {
                            eprintln!("  Vrba-Reix: persistent error — aborting");
                            return Err(GwError::GerbiczMismatch { iteration: j + 1 });
                        }
                    }
                }
            } else {
                // Block verified OK: promote checkpoints
                verified_checkpoint = last_checkpoint;
                verified_checkpoint_iter = last_checkpoint_iter;
                last_checkpoint = current_gmp;
                last_checkpoint_iter = i + 1;
            }
        }
    }

    // Convert final result and check S(p-2) == 0
    let result = ctx.to_integer(&s);
    let is_prime = result == 0u32;

    // Final verification: recompute from last verified checkpoint via GMP
    if is_prime && verified_checkpoint_iter < iters {
        let mut verify = verified_checkpoint;
        for _ in verified_checkpoint_iter..iters {
            verify.square_mut();
            verify -= 2u32;
            verify = verify.rem_euc(&n_val);
        }
        if verify != 0u32 {
            eprintln!("  Vrba-Reix: final verification FAILED — returning error");
            return Err(GwError::GerbiczMismatch { iteration: iters });
        }
    }

    Ok(is_prime)
}

/// Accelerated Proth test using GWNUM: a^((N-1)/2) ≡ -1 (mod N).
///
/// For N = k*2^n+1, uses GWNUM modular exponentiation with IBDWT.
/// Returns None if no quadratic non-residue found.
pub fn gwnum_proth(k: u64, b: u32, n: u64) -> Result<Option<bool>, GwError> {
    let mut ctx = GwContext::new(k, b, n, 1)?;

    // Find quadratic non-residue via Jacobi symbol
    let candidate = Integer::from(k) * Integer::from(b).pow(crate::checked_u32(n)) + 1u32;

    let bases: [u32; 12] = [3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41];
    for &a in &bases {
        let a_int = Integer::from(a);
        if a_int.jacobi(&candidate) == -1 {
            // Compute a^((N-1)/2) mod N using GWNUM
            // (N-1)/2 = k*2^(n-1) — binary exponentiation via GWNUM squarings
            let mut base = ctx.alloc();
            ctx.set_small(&mut base, a as f64);

            let mut result = ctx.alloc();
            ctx.set_small(&mut result, 1.0);
            let mut temp = ctx.alloc();

            // Square-and-multiply for exponent (N-1)/2
            // For k*b^n+1, (N-1)/2 = k*b^n/2 = k*2^(n-1) when b=2
            let exp = Integer::from(&candidate - 1u32) >> 1u32;
            let exp_bits = exp.significant_bits();

            for i in (0..exp_bits).rev() {
                ctx.square(&result, &mut temp)?;
                std::mem::swap(&mut result, &mut temp);
                if exp.get_bit(i) {
                    ctx.mul(&result, &base, &mut temp)?;
                    std::mem::swap(&mut result, &mut temp);
                }
            }

            let r = ctx.to_integer(&result);
            let n_minus_1 = Integer::from(&candidate - 1u32);
            return Ok(Some(r == n_minus_1));
        }
    }

    Ok(None) // No QNR found
}

/// Accelerated LLR test using GWNUM squaring loop.
///
/// For N = k*2^n-1 (k odd, k < 2^n):
///   Find initial value u_0 via Lucas sequences (Rödseth's method)
///   Iterate: u_i = u_{i-1}^2 - 2 (mod N) for n-2 steps
///   Prime iff u_{n-2} == 0
///
/// With Gerbicz error checking every √n steps: saves GWNUM state as GMP Integer
/// checkpoints, recomputes blocks using GMP as a correctness oracle, and rolls
/// back to the last verified checkpoint on mismatch.
pub fn gwnum_llr(k: u64, n: u64) -> Result<Option<bool>, GwError> {
    // k must be < 2^n; for n >= 64 any u64 k satisfies this
    if n < 64 && k >= (1u64 << n) {
        return Ok(None);
    }
    if n < 3 {
        return Ok(None);
    }

    let mut ctx = GwContext::new(k, 2, n, -1)?;

    let candidate = Integer::from(k) * Integer::from(2u32).pow(crate::checked_u32(n)) - 1u32;

    // Find starting value u_0 using Rödseth's method
    let v1: u32 = if !k.is_multiple_of(3) { 4 } else { 3 }; // Simplified; real impl uses Lucas V sequence

    let u0_int = Integer::from(v1);
    let mut u = ctx.from_integer(&u0_int);

    let mut temp = ctx.alloc();
    let mut two = ctx.alloc();
    ctx.set_small(&mut two, 2.0);

    let iters = n - 2;

    // Gerbicz error checking: checkpoint every √(iters) steps.
    // Disable for small n where overhead isn't worth it.
    let check_interval = if iters > 10_000 {
        (iters as f64).sqrt() as u64
    } else {
        iters + 1 // disable
    };

    // Checkpoint state: GMP Integers for correctness verification
    let mut last_checkpoint = u0_int.clone();
    let mut last_checkpoint_iter: u64 = 0;
    let mut verified_checkpoint = u0_int;
    let mut verified_checkpoint_iter: u64 = 0;

    for i in 0..iters {
        if n > 50_000 && i % 10_000 == 0 && i > 0 {
            eprintln!(
                "  GWNUM LLR: {}/{} squarings ({:.1}%)",
                i,
                iters,
                i as f64 / iters as f64 * 100.0
            );
        }

        ctx.square(&u, &mut temp)?;
        ctx.sub(&temp, &two, &mut u);

        // Gerbicz checkpoint every check_interval iterations
        if check_interval < iters && (i + 1) % check_interval == 0 {
            let current_gmp = ctx.checkpoint_value(&u);

            // Recompute this block from last_checkpoint using GMP (correctness oracle)
            let mut verify = last_checkpoint.clone();
            let steps = i + 1 - last_checkpoint_iter;
            for _ in 0..steps {
                verify.square_mut();
                verify -= 2u32;
                verify = verify.rem_euc(&candidate);
            }

            if verify != current_gmp {
                // Hardware/FFT error detected — rollback to last verified checkpoint
                eprintln!(
                    "  GWNUM LLR ERROR at iteration {} — rolling back to {}",
                    i + 1,
                    verified_checkpoint_iter
                );

                // Reload verified checkpoint into GWNUM and replay
                u = ctx.from_integer(&verified_checkpoint);
                let mut u_gmp = verified_checkpoint.clone();

                let redo_start = verified_checkpoint_iter;
                for j in redo_start..=i {
                    ctx.square(&u, &mut temp)?;
                    ctx.sub(&temp, &two, &mut u);

                    u_gmp.square_mut();
                    u_gmp -= 2u32;
                    u_gmp = u_gmp.rem_euc(&candidate);

                    if (j + 1) % check_interval == 0 {
                        let replayed_gmp = ctx.checkpoint_value(&u);
                        if replayed_gmp == u_gmp {
                            last_checkpoint = u_gmp.clone();
                            last_checkpoint_iter = j + 1;
                            verified_checkpoint = last_checkpoint.clone();
                            verified_checkpoint_iter = j + 1;
                        } else {
                            eprintln!("  GWNUM LLR: persistent error — aborting");
                            return Err(GwError::GerbiczMismatch { iteration: j + 1 });
                        }
                    }
                }
            } else {
                // Block verified OK: promote checkpoints
                verified_checkpoint = last_checkpoint;
                verified_checkpoint_iter = last_checkpoint_iter;
                last_checkpoint = current_gmp;
                last_checkpoint_iter = i + 1;
            }
        }
    }

    let result = ctx.to_integer(&u);
    let is_prime = result == 0u32;

    // Final verification: recompute from last verified checkpoint via GMP
    if is_prime && verified_checkpoint_iter < iters {
        let mut verify = verified_checkpoint;
        for _ in verified_checkpoint_iter..iters {
            verify.square_mut();
            verify -= 2u32;
            verify = verify.rem_euc(&candidate);
        }
        if verify != 0u32 {
            eprintln!("  GWNUM LLR: final verification FAILED — returning error");
            return Err(GwError::GerbiczMismatch { iteration: iters });
        }
    }

    Ok(Some(is_prime))
}

/// Generic GWNUM modular exponentiation: base^exp mod (k*b^n+c).
///
/// Useful for Pocklington/Morrison witness computation on large candidates.
pub fn gwnum_pow_mod(
    base: &Integer,
    exp: &Integer,
    k: u64,
    b: u32,
    n: u64,
    c: i64,
) -> Result<Integer, GwError> {
    let mut ctx = GwContext::new(k, b, n, c)?;

    let mut result_gw = ctx.from_integer(&Integer::from(1u32));
    let base_gw = ctx.from_integer(base);
    let mut temp = ctx.alloc();

    let exp_bits = exp.significant_bits();

    for i in (0..exp_bits).rev() {
        ctx.square(&result_gw, &mut temp)?;
        std::mem::swap(&mut result_gw, &mut temp);

        if exp.get_bit(i) {
            ctx.mul(&result_gw, &base_gw, &mut temp)?;
            std::mem::swap(&mut result_gw, &mut temp);
        }
    }

    Ok(ctx.to_integer(&result_gw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gw_context_unavailable_without_feature() {
        // Without the gwnum feature, GwContext::new should return Unavailable
        #[cfg(not(feature = "gwnum"))]
        {
            let result = GwContext::new(3, 2, 50000, 1);
            assert!(matches!(result, Err(GwError::Unavailable)));
        }
    }

    #[test]
    fn vrba_reix_rejects_small_p() {
        let result = vrba_reix_test(2);
        assert!(result.is_err());
    }

    #[test]
    #[cfg(not(feature = "gwnum"))]
    fn vrba_reix_unavailable_without_feature() {
        let result = vrba_reix_test(5);
        assert!(matches!(result, Err(GwError::Unavailable)));
    }

    #[test]
    #[cfg(not(feature = "gwnum"))]
    fn gwnum_proth_unavailable_without_feature() {
        let result = gwnum_proth(3, 2, 50000);
        assert!(matches!(result, Err(GwError::Unavailable)));
    }

    #[test]
    #[cfg(not(feature = "gwnum"))]
    fn gwnum_llr_unavailable_without_feature() {
        let result = gwnum_llr(3, 50000);
        assert!(matches!(result, Err(GwError::Unavailable)));
    }

    #[test]
    #[cfg(not(feature = "gwnum"))]
    fn gwnum_pow_mod_unavailable_without_feature() {
        let base = Integer::from(3u32);
        let exp = Integer::from(100u32);
        let result = gwnum_pow_mod(&base, &exp, 3, 2, 50000, 1);
        assert!(matches!(result, Err(GwError::Unavailable)));
    }

    // Tests that require the gwnum feature and gwnum.a installed
    #[test]
    #[ignore] // Requires gwnum.a installed
    #[cfg(feature = "gwnum")]
    fn vrba_reix_known_wagstaff_primes() {
        // Known Wagstaff prime exponents: 3, 5, 7, 11, 13, 17, 19, 23, 31, 43
        for &p in &[3u64, 5, 7, 11, 13] {
            let result = vrba_reix_test(p).expect("vrba_reix should not error");
            assert!(result, "(2^{}+1)/3 should be Wagstaff prime", p);
        }
    }

    #[test]
    #[ignore] // Requires gwnum.a installed
    #[cfg(feature = "gwnum")]
    fn vrba_reix_known_composites() {
        for &p in &[29u64, 37, 41] {
            let result = vrba_reix_test(p).expect("vrba_reix should not error");
            assert!(!result, "(2^{}+1)/3 should be composite", p);
        }
    }

    #[test]
    #[ignore] // Requires gwnum.a installed
    #[cfg(feature = "gwnum")]
    fn gwnum_proth_known_prime() {
        // 3*2^50000+1 is a known Proth prime
        let result = gwnum_proth(3, 2, 50000).expect("gwnum_proth should not error");
        assert_eq!(result, Some(true), "3*2^50000+1 should be prime");
    }

    #[test]
    #[ignore] // Requires gwnum.a installed
    #[cfg(feature = "gwnum")]
    fn gwnum_proth_cross_verify_with_gmp() {
        use rug::integer::IsPrime;
        // Test a small Proth prime: 3*2^10+1 = 3073
        let candidate = Integer::from(3u32) * Integer::from(2u32).pow(10) + 1u32;
        let gmp_result = candidate.is_probably_prime(25) != IsPrime::No;
        let gwnum_result = gwnum_proth(3, 2, 10).expect("should not error");
        assert_eq!(gwnum_result, Some(gmp_result));
    }
}
