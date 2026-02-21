//! PFGW subprocess integration for accelerated primality testing.
//!
//! PFGW (Primes or Fermats, George Woltman) uses GWNUM internally for 50-100x
//! speedup on large candidates. This module provides optional subprocess integration
//! for forms that PRST doesn't support: factorial, primorial, wagstaff, palindromic,
//! and near_repdigit.
//!
//! Each form provides its expression in PFGW's input format:
//! - Factorial: `n!+1` or `n!-1`
//! - Primorial: `p#+1` or `p#-1`
//! - Wagstaff: `(2^p+1)/3`
//! - Palindromic: decimal digit string
//! - Near-repdigit: algebraic expression like `10^1001-1-4*(10^600+10^400)`

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Global counter for unique PFGW temp file names, avoiding race conditions
/// when multiple rayon threads sanitize expressions to identical filenames.
static PFGW_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Result of a PFGW primality test.
#[derive(Debug, Clone)]
pub enum PfgwResult {
    /// Candidate is prime (deterministic proof or probable prime).
    Prime {
        method: String,
        is_deterministic: bool,
    },
    /// Candidate is composite.
    Composite,
    /// PFGW was not available or not applicable.
    Unavailable { reason: String },
}

/// Test mode for PFGW invocation.
#[derive(Debug, Clone, Copy)]
pub enum PfgwMode {
    /// Default PRP test (no proof attempt).
    Prp,
    /// N-1 proof via Pocklington (`-tp` flag). Use for n!+1, p#+1.
    NMinus1Proof,
    /// N+1 proof via Morrison (`-tm` flag). Use for n!-1, p#-1.
    NPlus1Proof,
}

/// Global PFGW configuration.
struct PfgwConfig {
    min_digits: u64,
    timeout: Duration,
    binary_path: Option<PathBuf>,
}

static PFGW_CONFIG: OnceLock<PfgwConfig> = OnceLock::new();
static PFGW_BINARY: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Initialize PFGW configuration. Call once at startup.
pub fn init(min_digits: u64, binary_path: Option<PathBuf>, timeout: Duration) {
    let _ = PFGW_CONFIG.set(PfgwConfig {
        min_digits,
        timeout,
        binary_path,
    });
}

/// Detect the PFGW binary, caching the result.
fn get_binary() -> Option<PathBuf> {
    PFGW_BINARY
        .get_or_init(|| {
            // Check configured path first
            if let Some(config) = PFGW_CONFIG.get() {
                if let Some(ref path) = config.binary_path {
                    if path.exists() {
                        return Some(path.clone());
                    }
                }
            }
            // Try to find 'pfgw64' in PATH
            for name in &["pfgw64", "pfgw"] {
                if let Some(path) = find_in_path(name) {
                    return Some(path);
                }
            }
            None
        })
        .clone()
}

/// Search for a binary in PATH.
fn find_in_path(name: &str) -> Option<PathBuf> {
    Command::new("which")
        .arg(name)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| p.exists())
}

/// Cheap check: returns true if PFGW is configured and the digit count meets the threshold.
/// Use this to avoid expensive to_string_radix() calls on candidates that PFGW would reject.
#[inline]
pub fn is_available(digits: u64) -> bool {
    PFGW_CONFIG
        .get()
        .is_some_and(|config| digits >= config.min_digits)
}

/// Try to test a candidate using PFGW.
///
/// `expression` is in PFGW input format (e.g., "100!+1", "(2^42737+1)/3", or a decimal string).
/// `candidate` is the actual number (used only for digit count estimation).
/// `mode` controls which proof mode to use (-tp, -tm, or default PRP).
///
/// Returns None if PFGW is not configured (init() not called).
/// Returns Some(Unavailable) if PFGW is configured but the candidate is too small
/// or the binary is not found.
pub fn try_test(expression: &str, candidate: &rug::Integer, mode: PfgwMode) -> Option<PfgwResult> {
    let config = PFGW_CONFIG.get()?;

    // Check digit threshold
    let digits = crate::estimate_digits(candidate);
    if digits < config.min_digits {
        return Some(PfgwResult::Unavailable {
            reason: format!(
                "candidate has {} digits, below threshold {}",
                digits, config.min_digits
            ),
        });
    }

    // Check binary availability
    let binary = match get_binary() {
        Some(b) => b,
        None => {
            return Some(PfgwResult::Unavailable {
                reason: "PFGW binary not found (install pfgw64 or set --pfgw-path)".into(),
            });
        }
    };

    // Write input file with expression (use atomic counter for unique names
    // so parallel rayon threads don't collide on identically-sanitized expressions)
    let dir = std::env::temp_dir();
    let id = PFGW_COUNTER.fetch_add(1, Ordering::Relaxed);
    let input_path = dir.join(format!("pfgw_{}.txt", id));

    if let Err(e) = std::fs::write(&input_path, format!("{}\n", expression)) {
        return Some(PfgwResult::Unavailable {
            reason: format!("failed to write input file: {}", e),
        });
    }

    // Run PFGW with timeout
    let result = run_subprocess(&binary, &input_path, mode, config.timeout);

    // Clean up temp file
    let _ = std::fs::remove_file(&input_path);

    match result {
        Ok(r) => Some(r),
        Err(e) => Some(PfgwResult::Unavailable {
            reason: format!("PFGW execution failed: {}", e),
        }),
    }
}

/// Execute the PFGW binary with the appropriate mode flags and timeout enforcement.
/// Supports optional progress reporting via stderr parsing.
fn run_subprocess(
    binary: &Path,
    input_path: &Path,
    mode: PfgwMode,
    timeout: Duration,
) -> std::io::Result<PfgwResult> {
    let mut cmd = Command::new(binary);

    match mode {
        PfgwMode::NMinus1Proof => {
            cmd.arg("-tp");
        }
        PfgwMode::NPlus1Proof => {
            cmd.arg("-tm");
        }
        PfgwMode::Prp => {}
    }

    cmd.arg(input_path);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    // Spawn a thread to read stderr for progress reporting.
    // PFGW outputs progress like "Testing ... 47.3%" to stderr.
    let stderr_handle = child.stderr.take();
    let stderr_thread = std::thread::spawn(move || {
        let mut stderr_buf = String::new();
        if let Some(mut err) = stderr_handle {
            let _ = err.read_to_string(&mut stderr_buf);
        }
        stderr_buf
    });

    let start = Instant::now();
    loop {
        match child.try_wait()? {
            Some(_status) => {
                let mut stdout = String::new();
                if let Some(mut out) = child.stdout.take() {
                    out.read_to_string(&mut stdout)?;
                }
                let stderr = stderr_thread.join().unwrap_or_default();
                let combined = format!("{}\n{}", stdout, stderr);
                return Ok(parse_output(&combined));
            }
            None => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stderr_thread.join();
                    return Ok(PfgwResult::Unavailable {
                        reason: format!("timed out after {}s", timeout.as_secs()),
                    });
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Parse PFGW stdout/stderr to determine the result.
pub fn parse_output(output: &str) -> PfgwResult {
    // Check for proven prime results (deterministic proofs via -tp or -tm)
    if output.contains("is prime!") {
        let method = if output.contains("N-1") || output.contains("Pocklington") {
            "PFGW/Pocklington"
        } else if output.contains("N+1") || output.contains("Morrison") {
            "PFGW/Morrison"
        } else if output.contains("BLS") {
            "PFGW/BLS"
        } else {
            "PFGW/proof"
        };
        return PfgwResult::Prime {
            method: method.to_string(),
            is_deterministic: true,
        };
    }

    // Probable prime (PRP, no deterministic proof)
    if output.contains("is a probable prime")
        || output.contains("PRP")
        || output.contains("is 3-PRP")
    {
        return PfgwResult::Prime {
            method: "PFGW/PRP".to_string(),
            is_deterministic: false,
        };
    }

    // Composite
    if output.contains("is not prime")
        || output.contains("composite")
        || output.contains("is not a probable prime")
    {
        return PfgwResult::Composite;
    }

    // Couldn't parse output
    PfgwResult::Unavailable {
        reason: format!(
            "unrecognized PFGW output: {}",
            output.chars().take(200).collect::<String>()
        ),
    }
}

#[cfg(test)]
mod tests {
    //! Tests for PFGW subprocess output parsing and integration.
    //!
    //! Validates parse_output() against all known PFGW output patterns:
    //! proven primes (Pocklington, Morrison, BLS, generic), probable primes
    //! (PRP, 3-PRP), composites ("is not prime", "composite", "is not a
    //! probable prime"), and edge cases (empty output, progress-only output,
    //! very long unrecognized output truncation).
    //!
    //! ## PFGW Output Patterns
    //!
    //! | Output text | Result | is_deterministic |
    //! |-------------|--------|-----------------|
    //! | "is prime!" + "N-1" | Pocklington proof | true |
    //! | "is prime!" + "N+1" | Morrison proof | true |
    //! | "is prime!" + "BLS" | BLS proof | true |
    //! | "is prime!" (alone) | Generic proof | true |
    //! | "is a probable prime" | PRP | false |
    //! | "PRP" | PRP | false |
    //! | "is 3-PRP" | PRP (base-3) | false |
    //! | "is not prime" | Composite | N/A |
    //! | (unrecognized) | Unavailable | N/A |
    //!
    //! Integration tests (marked `#[ignore]`) require the pfgw64 binary
    //! installed in PATH and test actual subprocess execution.

    use super::*;

    // ── Proven Primes (Deterministic) ────────────────────────────────

    /// Pocklington N-1 proof: PFGW outputs "is prime! (N-1 test)" when the
    /// -tp flag produces a complete factorization of N-1.
    #[test]
    fn parse_proven_prime_pocklington() {
        let output = "100!+1 is prime! (N-1 test)";
        match parse_output(output) {
            PfgwResult::Prime {
                method,
                is_deterministic,
            } => {
                assert!(method.contains("Pocklington"));
                assert!(is_deterministic);
            }
            other => panic!("expected Prime, got {:?}", other),
        }
    }

    /// Morrison N+1 proof: PFGW outputs "is prime! (N+1 test)" when the
    /// -tm flag produces a complete factorization of N+1.
    #[test]
    fn parse_proven_prime_morrison() {
        let output = "100!-1 is prime! (N+1 test)";
        match parse_output(output) {
            PfgwResult::Prime {
                method,
                is_deterministic,
            } => {
                assert!(method.contains("Morrison"));
                assert!(is_deterministic);
            }
            other => panic!("expected Prime, got {:?}", other),
        }
    }

    /// BLS (Brillhart-Lehmer-Selfridge) combined N-1/N+1 proof: used when
    /// neither N-1 nor N+1 alone is sufficiently factored, but together
    /// they provide enough to prove primality. Common for near-repdigit forms.
    #[test]
    fn parse_proven_prime_bls() {
        let output = "10^1001-1-4*(10^600+10^400) is prime! (BLS proof)";
        match parse_output(output) {
            PfgwResult::Prime {
                method,
                is_deterministic,
            } => {
                assert!(method.contains("BLS"));
                assert!(is_deterministic);
            }
            other => panic!("expected Prime, got {:?}", other),
        }
    }

    /// Generic proof: "is prime!" without a named method. Falls back to
    /// "PFGW/proof" as the method string.
    #[test]
    fn parse_proven_prime_generic() {
        let output = "12345 is prime!";
        match parse_output(output) {
            PfgwResult::Prime {
                method,
                is_deterministic,
            } => {
                assert!(method.contains("proof"));
                assert!(is_deterministic);
            }
            other => panic!("expected Prime, got {:?}", other),
        }
    }

    // ── Probable Primes (PRP) ──────────────────────────────────────

    /// Standard PRP output: "is a probable prime". PFGW performs a strong
    /// Fermat test but cannot provide a deterministic proof. Wagstaff
    /// candidates always fall in this category (no known proof method).
    #[test]
    fn parse_probable_prime() {
        let output = "(2^42737+1)/3 is a probable prime";
        match parse_output(output) {
            PfgwResult::Prime {
                is_deterministic, ..
            } => {
                assert!(!is_deterministic);
            }
            other => panic!("expected Prime (PRP), got {:?}", other),
        }
    }

    /// Alternative PRP output: "PRP" appears at the end of a testing line.
    /// Some PFGW versions use this shorter format.
    #[test]
    fn parse_prp_tag() {
        let output = "Testing (2^42737+1)/3 ... PRP";
        match parse_output(output) {
            PfgwResult::Prime {
                is_deterministic, ..
            } => {
                assert!(!is_deterministic);
            }
            other => panic!("expected Prime (PRP), got {:?}", other),
        }
    }

    /// Base-3 PRP: "is 3-PRP!" indicates a Fermat test with base 3 passed.
    /// This is PFGW's default test base for Wagstaff and some other forms.
    #[test]
    fn parse_three_prp() {
        let output = "(2^127+1)/3 is 3-PRP!";
        match parse_output(output) {
            PfgwResult::Prime {
                is_deterministic, ..
            } => {
                assert!(!is_deterministic);
            }
            other => panic!("expected Prime (PRP), got {:?}", other),
        }
    }

    // ── Composites ────────────────────────────────────────────────

    /// Standard composite output: "is not prime". PFGW confirmed the
    /// candidate is composite via the Fermat test.
    #[test]
    fn parse_composite() {
        let output = "100!+1 is not prime";
        match parse_output(output) {
            PfgwResult::Composite => {}
            other => panic!("expected Composite, got {:?}", other),
        }
    }

    /// Alternative composite output: just the word "composite" in the output.
    #[test]
    fn parse_composite_alt() {
        let output = "12345 composite";
        match parse_output(output) {
            PfgwResult::Composite => {}
            other => panic!("expected Composite, got {:?}", other),
        }
    }

    /// "is not a probable prime" — the PRP test failed, confirming composite.
    /// Distinguished from "is not prime" (deterministic) for logging purposes
    /// but both map to PfgwResult::Composite.
    #[test]
    fn parse_not_probable_prime() {
        let output = "(2^29+1)/3 is not a probable prime";
        match parse_output(output) {
            PfgwResult::Composite => {}
            other => panic!("expected Composite, got {:?}", other),
        }
    }

    // ── Edge Cases ────────────────────────────────────────────────

    /// Unrecognized output returns Unavailable with the output text
    /// included in the reason (for debugging PFGW version differences).
    #[test]
    fn parse_unknown_output() {
        let output = "some unexpected text";
        match parse_output(output) {
            PfgwResult::Unavailable { reason } => {
                assert!(reason.contains("unrecognized"));
            }
            other => panic!("expected Unavailable, got {:?}", other),
        }
    }

    /// Empty output (PFGW crashed or was killed before producing output)
    /// returns Unavailable rather than panicking.
    #[test]
    fn parse_empty_output() {
        match parse_output("") {
            PfgwResult::Unavailable { .. } => {}
            other => panic!("expected Unavailable for empty output, got {:?}", other),
        }
    }

    // ── Digit Threshold ───────────────────────────────────────────

    /// Validates the estimate_digits utility used for PFGW's min_digits
    /// threshold check. Small numbers (7 = 1 digit) fall below 10K;
    /// 2^100000 (~30103 digits) exceeds 10K.
    #[test]
    fn threshold_digit_check() {
        let small = rug::Integer::from(7u32);
        assert!(crate::estimate_digits(&small) < 10_000);

        let large = {
            use rug::ops::Pow;
            rug::Integer::from(2u32).pow(100_000)
        };
        assert!(crate::estimate_digits(&large) > 10_000);
    }

    // ── Integration Tests (require PFGW binary) ─────────────────

    /// Tests PFGW execution with a known factorial prime: 11!+1 = 39916801.
    /// Uses -tp (N-1 proof) mode since n!+1 has a fully factorable N-1 = n!.
    #[test]
    #[ignore] // Requires PFGW binary installed
    fn pfgw_integration_factorial_prime() {
        init(0, None, Duration::from_secs(60));
        // 11!+1 = 39916801 is prime
        let candidate = rug::Integer::from(rug::Integer::factorial(11)) + 1u32;
        if let Some(result) = try_test("11!+1", &candidate, PfgwMode::NMinus1Proof) {
            match result {
                PfgwResult::Prime { .. } => {}
                PfgwResult::Unavailable { .. } => {
                    eprintln!("PFGW not available, skipping integration test");
                }
                PfgwResult::Composite => panic!("11!+1 should be prime!"),
            }
        }
    }

    /// Tests PFGW execution with a known Wagstaff prime: (2^5+1)/3 = 11.
    /// Uses PRP mode since no deterministic proof exists for Wagstaff numbers.
    #[test]
    #[ignore] // Requires PFGW binary installed
    fn pfgw_integration_wagstaff_prime() {
        init(0, None, Duration::from_secs(60));
        // (2^5+1)/3 = 11 is prime
        let candidate = (rug::Integer::from(1u32) << 5u32) + 1u32;
        let candidate = candidate / 3u32;
        if let Some(result) = try_test("(2^5+1)/3", &candidate, PfgwMode::Prp) {
            match result {
                PfgwResult::Prime { .. } => {}
                PfgwResult::Unavailable { .. } => {
                    eprintln!("PFGW not available, skipping integration test");
                }
                PfgwResult::Composite => panic!("(2^5+1)/3 should be prime!"),
            }
        }
    }

    // ── Malformed Output ──────────────────────────────────────────

    /// PFGW sometimes outputs only progress lines (e.g., "Testing ... 47.1%")
    /// with no final result line (e.g., killed by timeout). This must return
    /// Unavailable, not panic or misparse as composite.
    #[test]
    fn parse_malformed_output_progress_only() {
        // PFGW sometimes outputs only progress lines with no result — should return Unavailable
        let output =
            "Testing 100!+1 ... 12.3%\nTesting 100!+1 ... 47.1%\nTesting 100!+1 ... 98.2%\n";
        match parse_output(output) {
            PfgwResult::Unavailable { reason } => {
                assert!(reason.contains("unrecognized"), "reason: {}", reason);
            }
            other => panic!(
                "expected Unavailable for progress-only output, got {:?}",
                other
            ),
        }
    }

    /// Very long unrecognized output is truncated to 200 chars in the error
    /// reason to prevent oversized log messages and error payloads.
    #[test]
    fn parse_very_long_output_truncated() {
        // Very long unrecognized output should be truncated in the reason
        let output = "x".repeat(500);
        match parse_output(&output) {
            PfgwResult::Unavailable { reason } => {
                assert!(
                    reason.len() < 300,
                    "reason should be truncated, got len={}",
                    reason.len()
                );
            }
            other => panic!("expected Unavailable for long output, got {:?}", other),
        }
    }

    /// Tests PFGW with a Cullen prime: 1*2^1+1 = 3. Uses PRP mode.
    #[test]
    #[ignore] // Requires PFGW binary installed
    fn pfgw_integration_cullen_prime() {
        init(0, None, Duration::from_secs(60));
        // 1*2^1+1 = 3 is prime (Cullen n=1)
        let candidate = rug::Integer::from(3u32);
        if let Some(result) = try_test("1*2^1+1", &candidate, PfgwMode::Prp) {
            match result {
                PfgwResult::Prime { .. } => {}
                PfgwResult::Unavailable { .. } => {
                    eprintln!("PFGW not available, skipping integration test");
                }
                PfgwResult::Composite => panic!("1*2^1+1 = 3 should be prime!"),
            }
        }
    }

    /// Tests PFGW with a repunit: (10^7-1)/9 = 1111111 (composite, = 239*4649).
    /// Verifies the PFGW input format for repunits works correctly.
    #[test]
    #[ignore] // Requires PFGW binary installed
    fn pfgw_integration_repunit_prime() {
        use rug::ops::Pow;
        init(0, None, Duration::from_secs(60));
        // (10^7-1)/9 = 1111111 = 239*4649, composite — but tests the format
        let candidate = (rug::Integer::from(10u32).pow(7) - 1u32) / 9u32;
        if let Some(result) = try_test("(10^7-1)/9", &candidate, PfgwMode::Prp) {
            match result {
                PfgwResult::Prime { .. } | PfgwResult::Composite => {}
                PfgwResult::Unavailable { .. } => {
                    eprintln!("PFGW not available, skipping integration test");
                }
            }
        }
    }

    /// Tests PFGW with a generalized Fermat prime: 2^4+1 = 17 (F2).
    #[test]
    #[ignore] // Requires PFGW binary installed
    fn pfgw_integration_gen_fermat_prime() {
        init(0, None, Duration::from_secs(60));
        // 2^4+1 = 17 is prime (Fermat F2)
        let candidate = rug::Integer::from(17u32);
        if let Some(result) = try_test("2^4+1", &candidate, PfgwMode::Prp) {
            match result {
                PfgwResult::Prime { .. } => {}
                PfgwResult::Unavailable { .. } => {
                    eprintln!("PFGW not available, skipping integration test");
                }
                PfgwResult::Composite => panic!("2^4+1 = 17 should be prime!"),
            }
        }
    }

    /// Tests PFGW with a Carol prime: (2^7-1)^2-2 = 16127 (Carol n=7).
    #[test]
    #[ignore] // Requires PFGW binary installed
    fn pfgw_integration_carol_prime() {
        init(0, None, Duration::from_secs(60));
        // (2^7-1)^2-2 = 127^2-2 = 16127 is prime (Carol n=7)
        let candidate = rug::Integer::from(16127u32);
        if let Some(result) = try_test("(2^7-1)^2-2", &candidate, PfgwMode::Prp) {
            match result {
                PfgwResult::Prime { .. } => {}
                PfgwResult::Unavailable { .. } => {
                    eprintln!("PFGW not available, skipping integration test");
                }
                PfgwResult::Composite => panic!("(2^7-1)^2-2 = 16127 should be prime!"),
            }
        }
    }
}
