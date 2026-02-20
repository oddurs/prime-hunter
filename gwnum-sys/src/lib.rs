//! Raw FFI bindings to the GWNUM library.
//!
//! These bindings are manually declared based on the GWNUM C API.
//! When bindgen is available (gwnum headers installed), these can be
//! auto-generated instead via `build.rs`.
//!
//! # Safety
//!
//! All functions in this crate are `unsafe` FFI calls. Use the safe
//! wrapper in `src/gwnum.rs` instead.
//!
//! # Platform
//!
//! x86-64 only (GWNUM uses hand-tuned SSE2/AVX/FMA3/AVX-512 assembly).

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::os::raw::{c_char, c_double, c_int, c_long, c_uint, c_ulong};

/// Opaque gwhandle structure. Must be allocated on the stack or heap.
/// Each thread MUST have its own gwhandle — gwnums are not transferable between handles.
///
/// The actual struct is large (~4KB) but we treat it as opaque via a fixed-size array.
/// The real size is checked at runtime in gwinit2 via the struct_size parameter.
#[repr(C)]
pub struct gwhandle {
    _opaque: [u8; 8192], // Conservative size; real gwhandle is ~4KB
}

/// A gwnum is a pointer to FFT-domain data (double array).
pub type gwnum = *mut c_double;

/// Options for gwmul3 and related functions.
pub const GWMUL_FFT_S1: c_int = 1;
pub const GWMUL_FFT_S2: c_int = 2;
pub const GWMUL_STARTNEXTFFT: c_int = 4;

// Possible GWERROR values returned by gw_test_for_error / gw_get_error
pub const GWERROR_NONE: c_int = 0;
pub const GWERROR_ROUNDOFF: c_int = 1;
pub const GWERROR_HARDWARE: c_int = 2;
pub const GWERROR_INTERNAL: c_int = 3;

extern "C" {
    /// Read the GWERROR field from a gwhandle and clear it.
    /// Returns GWERROR_NONE (0) if no error, GWERROR_ROUNDOFF (1) if roundoff
    /// exceeded, GWERROR_HARDWARE (2) for hardware errors, GWERROR_INTERNAL (3)
    /// for internal errors.
    pub fn gw_test_for_error(gwdata: *mut gwhandle) -> c_int;

    /// Initialize a gwhandle structure.
    /// `struct_size` should be `std::mem::size_of::<gwhandle>()`.
    /// `version_string` should be the GWNUM version string.
    pub fn gwinit2(gwdata: *mut gwhandle, struct_size: c_int, version_string: *const c_char);

    /// Configure gwhandle for modular arithmetic mod k*b^n+c.
    /// Returns 0 on success.
    pub fn gwsetup(gwdata: *mut gwhandle, k: c_double, b: c_ulong, n: c_ulong, c: c_long) -> c_int;

    /// Clean up gwhandle (frees all gwnums and internal state).
    pub fn gwdone(gwdata: *mut gwhandle);

    /// Allocate a gwnum in this context.
    pub fn gwalloc(gwdata: *mut gwhandle) -> gwnum;

    /// Free a single gwnum.
    pub fn gwfree(gwdata: *mut gwhandle, g: gwnum);

    /// Free all gwnums allocated from this handle.
    pub fn gwfreeall(gwdata: *mut gwhandle);

    /// Multiply: d = s1 * s2 mod N.
    /// Use options to control FFT behavior (e.g., GWMUL_STARTNEXTFFT).
    pub fn gwmul3(gwdata: *mut gwhandle, s1: gwnum, s2: gwnum, d: gwnum, options: c_int);

    /// Add: d = s1 + s2.
    pub fn gwadd3o(gwdata: *mut gwhandle, s1: gwnum, s2: gwnum, d: gwnum, options: c_int);

    /// Subtract: d = s1 - s2.
    pub fn gwsub3o(gwdata: *mut gwhandle, s1: gwnum, s2: gwnum, d: gwnum, options: c_int);

    /// Convert binary array (little-endian 32-bit words) to gwnum.
    pub fn binarytogw(gwdata: *mut gwhandle, array: *const c_uint, len: c_int, g: gwnum);

    /// Convert gwnum to binary array (little-endian 32-bit words).
    /// Returns the number of words written.
    pub fn gwtobinary(gwdata: *mut gwhandle, g: gwnum, array: *mut c_uint, len: c_int) -> c_int;

    /// Set a gwnum to a small integer value.
    pub fn dbltogw(gwdata: *mut gwhandle, val: c_double, g: gwnum);
}

/// Macro to square: d = s^2 mod N.
/// Equivalent to gwmul3(gwdata, s, s, d, options).
#[macro_export]
macro_rules! gwsquare2 {
    ($gwdata:expr, $s:expr, $d:expr, $opt:expr) => {
        gwmul3($gwdata, $s, $s, $d, $opt)
    };
}

/// Check for roundoff errors. Wraps the GWNUM library's `gw_test_for_error`.
///
/// When the `gwnum` feature is enabled, this calls the real GWNUM function which
/// reads and clears the GWERROR field from the gwhandle struct. Without the
/// feature, returns GWERROR_NONE since no FFT operations are occurring.
///
/// # Safety
/// The gwdata pointer must point to a valid, initialized gwhandle.
#[inline]
pub unsafe fn gw_check_error(gwdata: *mut gwhandle) -> c_int {
    gw_test_for_error(gwdata)
}

/// Stub for non-gwnum builds: always returns GWERROR_NONE.
///
/// On builds without the real GWNUM library linked, the extern `gw_test_for_error`
/// symbol won't resolve. Use this function as a safe fallback.
#[inline]
pub fn gw_error_none() -> c_int {
    GWERROR_NONE
}
