//! FLINT integration for accelerated large-integer arithmetic.
//!
//! FLINT's `fft_small` module uses SIMD-vectorized NTTs (AVX2 on x86, NEON on ARM)
//! for 3-10x faster multiplication than GMP at large sizes. This module provides
//! optional wrappers for factorial and primorial computation.
//!
//! Enable with `--features flint`. Requires `libflint` installed on the system:
//! - macOS: `brew install flint`
//! - Linux: `apt install libflint-dev`

#[cfg(feature = "flint")]
mod inner {
    use flint3_sys::*;
    use rug::Integer;
    use std::mem::MaybeUninit;

    /// RAII wrapper for FLINT's fmpz type.
    struct Fmpz {
        inner: fmpz,
    }

    impl Fmpz {
        fn new() -> Self {
            let mut f = MaybeUninit::<fmpz>::zeroed();
            unsafe {
                fmpz_init(f.as_mut_ptr());
                Fmpz {
                    inner: f.assume_init(),
                }
            }
        }

        fn as_ptr(&self) -> *const fmpz {
            &self.inner
        }

        fn as_mut_ptr(&mut self) -> *mut fmpz {
            &mut self.inner
        }

        /// Convert FLINT fmpz to rug::Integer via GMP mpz_t.
        fn to_integer(&self) -> Integer {
            unsafe {
                let mut result = Integer::new();
                // fmpz_get_mpz copies the value into an mpz_t
                fmpz_get_mpz(result.as_raw_mut(), self.as_ptr());
                result
            }
        }
    }

    impl Drop for Fmpz {
        fn drop(&mut self) {
            unsafe {
                fmpz_clear(self.as_mut_ptr());
            }
        }
    }

    /// Compute n! using FLINT's binary-splitting factorial.
    ///
    /// FLINT's fmpz_fac_ui uses a prime-swing algorithm with SIMD-accelerated
    /// multiplication, typically 3-10x faster than GMP's mpz_fac_ui for large n.
    pub fn factorial(n: u64) -> Integer {
        let mut result = Fmpz::new();
        unsafe {
            fmpz_fac_ui(result.as_mut_ptr(), n as ulong);
        }
        result.to_integer()
    }

    /// Compute p# (primorial of n) using FLINT.
    ///
    /// FLINT's fmpz_primorial computes the product of all primes <= n.
    pub fn primorial(n: u64) -> Integer {
        let mut result = Fmpz::new();
        unsafe {
            fmpz_primorial(result.as_mut_ptr(), n as ulong);
        }
        result.to_integer()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use rug::ops::Pow;

        #[test]
        fn factorial_small_values() {
            assert_eq!(factorial(0), Integer::from(1u32));
            assert_eq!(factorial(1), Integer::from(1u32));
            assert_eq!(factorial(5), Integer::from(120u32));
            assert_eq!(factorial(10), Integer::from(3628800u32));
            assert_eq!(factorial(20), Integer::from(2432902008176640000u64));
        }

        #[test]
        fn factorial_matches_gmp() {
            for n in [50, 100, 500, 1000, 5000] {
                let flint_result = factorial(n);
                let gmp_result = Integer::from(Integer::factorial(n as u32));
                assert_eq!(
                    flint_result, gmp_result,
                    "FLINT and GMP disagree on {}!",
                    n
                );
            }
        }

        #[test]
        fn primorial_small_values() {
            // 2# = 2, 3# = 6, 5# = 30, 7# = 210, 11# = 2310
            assert_eq!(primorial(2), Integer::from(2u32));
            assert_eq!(primorial(3), Integer::from(6u32));
            assert_eq!(primorial(5), Integer::from(30u32));
            assert_eq!(primorial(7), Integer::from(210u32));
            assert_eq!(primorial(11), Integer::from(2310u32));
        }

        #[test]
        fn primorial_matches_gmp() {
            for p in [31, 89, 100, 317, 500] {
                let flint_result = primorial(p);
                let gmp_result = Integer::from(Integer::primorial(p as u32));
                assert_eq!(
                    flint_result, gmp_result,
                    "FLINT and GMP disagree on {}#",
                    p
                );
            }
        }
    }
}

// Re-export inner module functions when feature is enabled
#[cfg(feature = "flint")]
pub use inner::{factorial, primorial};
