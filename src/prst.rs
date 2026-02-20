//! PRST subprocess integration for GWNUM-accelerated primality testing.
//!
//! PRST (successor to LLR2) uses GWNUM internally for 50-100x speedup on large
//! k*b^n±1 candidates. This module provides optional subprocess integration with
//! graceful fallback when PRST is not installed.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Result of a PRST primality test.
#[derive(Debug, Clone)]
pub enum PrstResult {
    /// Candidate is prime (deterministic or probable).
    Prime {
        method: String,
        is_deterministic: bool,
    },
    /// Candidate is composite.
    Composite,
    /// PRST was not available or not applicable.
    Unavailable { reason: String },
}

/// Global PRST configuration.
struct PrstConfig {
    min_digits: u64,
    timeout: Duration,
    binary_path: Option<PathBuf>,
}

static PRST_CONFIG: OnceLock<PrstConfig> = OnceLock::new();
static PRST_BINARY: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Initialize PRST configuration. Call once at startup.
pub fn init(min_digits: u64, binary_path: Option<PathBuf>, timeout: Duration) {
    let _ = PRST_CONFIG.set(PrstConfig {
        min_digits,
        timeout,
        binary_path,
    });
}

/// Detect the PRST binary, caching the result.
fn get_binary() -> Option<PathBuf> {
    PRST_BINARY
        .get_or_init(|| {
            // Check configured path first
            if let Some(config) = PRST_CONFIG.get() {
                if let Some(ref path) = config.binary_path {
                    if path.exists() {
                        return Some(path.clone());
                    }
                }
            }
            // Try to find 'prst' in PATH
            Command::new("which")
                .arg("prst")
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| PathBuf::from(s.trim()))
                .filter(|p| p.exists())
        })
        .clone()
}

/// Try to test a k*b^n±1 candidate using PRST.
///
/// Returns None if PRST is not configured (init() not called).
/// Returns Some(Unavailable) if PRST is configured but the candidate is too small
/// or the binary is not found.
pub fn try_test(
    k: u64,
    base: u32,
    n: u64,
    is_plus: bool,
    candidate: &rug::Integer,
) -> Option<PrstResult> {
    let config = PRST_CONFIG.get()?;

    // Check digit threshold
    let digits = crate::estimate_digits(candidate);
    if digits < config.min_digits {
        return Some(PrstResult::Unavailable {
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
            return Some(PrstResult::Unavailable {
                reason: "PRST binary not found".into(),
            });
        }
    };

    // Write ABC input file
    let sign = if is_plus { "+" } else { "-" };
    let dir = std::env::temp_dir();
    let input_path = dir.join(format!(
        "prst_{}_{}_{}{}.txt",
        k,
        base,
        n,
        if is_plus { "p" } else { "m" }
    ));

    let abc_content = format!("ABC $a*$b^$c{}1\n{} {} {}\n", sign, k, base, n);

    if let Err(e) = std::fs::write(&input_path, &abc_content) {
        return Some(PrstResult::Unavailable {
            reason: format!("failed to write input file: {}", e),
        });
    }

    // Run PRST with timeout
    let result = run_subprocess(&binary, &input_path, config.timeout);

    // Clean up temp file
    let _ = std::fs::remove_file(&input_path);

    match result {
        Ok(r) => Some(r),
        Err(e) => Some(PrstResult::Unavailable {
            reason: format!("PRST execution failed: {}", e),
        }),
    }
}

/// Execute the PRST binary with timeout enforcement via poll loop.
fn run_subprocess(
    binary: &Path,
    input_path: &Path,
    timeout: Duration,
) -> std::io::Result<PrstResult> {
    let mut child = Command::new(binary)
        .arg(input_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let start = Instant::now();
    loop {
        match child.try_wait()? {
            Some(_status) => {
                let mut stdout = String::new();
                if let Some(mut out) = child.stdout.take() {
                    out.read_to_string(&mut stdout)?;
                }
                return Ok(parse_output(&stdout));
            }
            None => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(PrstResult::Unavailable {
                        reason: format!("timed out after {}s", timeout.as_secs()),
                    });
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Parse PRST stdout to determine the result.
pub fn parse_output(stdout: &str) -> PrstResult {
    // Check for prime results (deterministic proofs)
    if stdout.contains("is prime!") {
        let method = if stdout.contains("Proth") {
            "PRST/Proth"
        } else if stdout.contains("LLR") {
            "PRST/LLR"
        } else if stdout.contains("Morrison") {
            "PRST/Morrison"
        } else {
            "PRST"
        };
        return PrstResult::Prime {
            method: method.to_string(),
            is_deterministic: true,
        };
    }

    // Probable prime (no deterministic proof available)
    if stdout.contains("is a probable prime") || stdout.contains("PRP") {
        return PrstResult::Prime {
            method: "PRST/PRP".to_string(),
            is_deterministic: false,
        };
    }

    // Composite
    if stdout.contains("is not prime") || stdout.contains("composite") {
        return PrstResult::Composite;
    }

    // Couldn't parse output
    PrstResult::Unavailable {
        reason: format!(
            "unrecognized PRST output: {}",
            stdout.chars().take(200).collect::<String>()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_proth_prime() {
        let output = "3*2^50000+1 is prime! (Proth test)";
        match parse_output(output) {
            PrstResult::Prime {
                method,
                is_deterministic,
            } => {
                assert!(method.contains("Proth"));
                assert!(is_deterministic);
            }
            other => panic!("expected Prime, got {:?}", other),
        }
    }

    #[test]
    fn parse_llr_prime() {
        let output = "3*2^50000-1 is prime! (LLR test)";
        match parse_output(output) {
            PrstResult::Prime {
                method,
                is_deterministic,
            } => {
                assert!(method.contains("LLR"));
                assert!(is_deterministic);
            }
            other => panic!("expected Prime, got {:?}", other),
        }
    }

    #[test]
    fn parse_morrison_prime() {
        let output = "5*3^10000+1 is prime! (Morrison test)";
        match parse_output(output) {
            PrstResult::Prime {
                method,
                is_deterministic,
            } => {
                assert!(method.contains("Morrison"));
                assert!(is_deterministic);
            }
            other => panic!("expected Prime, got {:?}", other),
        }
    }

    #[test]
    fn parse_probable_prime() {
        let output = "7*3^10000+1 is a probable prime";
        match parse_output(output) {
            PrstResult::Prime {
                is_deterministic, ..
            } => {
                assert!(!is_deterministic);
            }
            other => panic!("expected Prime (PRP), got {:?}", other),
        }
    }

    #[test]
    fn parse_prp_tag() {
        let output = "Testing 7*3^10000+1 ... PRP";
        match parse_output(output) {
            PrstResult::Prime {
                is_deterministic, ..
            } => {
                assert!(!is_deterministic);
            }
            other => panic!("expected Prime (PRP), got {:?}", other),
        }
    }

    #[test]
    fn parse_composite() {
        let output = "3*2^50001+1 is not prime";
        match parse_output(output) {
            PrstResult::Composite => {}
            other => panic!("expected Composite, got {:?}", other),
        }
    }

    #[test]
    fn parse_composite_alt() {
        let output = "3*2^50001+1 composite";
        match parse_output(output) {
            PrstResult::Composite => {}
            other => panic!("expected Composite, got {:?}", other),
        }
    }

    #[test]
    fn parse_unknown_output() {
        let output = "some unexpected text";
        match parse_output(output) {
            PrstResult::Unavailable { reason } => {
                assert!(reason.contains("unrecognized"));
            }
            other => panic!("expected Unavailable, got {:?}", other),
        }
    }

    #[test]
    fn parse_empty_output() {
        match parse_output("") {
            PrstResult::Unavailable { .. } => {}
            other => panic!("expected Unavailable for empty output, got {:?}", other),
        }
    }

    #[test]
    fn threshold_digit_check() {
        // Verify the digit estimation works correctly for threshold decisions
        let small = rug::Integer::from(7u32);
        assert!(crate::estimate_digits(&small) < 10_000);

        let large = {
            use rug::ops::Pow;
            rug::Integer::from(2u32).pow(100_000)
        };
        assert!(crate::estimate_digits(&large) > 10_000);
    }

    #[test]
    #[ignore] // Requires PRST binary installed
    fn prst_integration_known_prime() {
        init(0, None, Duration::from_secs(60));
        let candidate = {
            use rug::ops::Pow;
            rug::Integer::from(3u32) * rug::Integer::from(2u32).pow(50_000) + 1u32
        };
        if let Some(result) = try_test(3, 2, 50_000, true, &candidate) {
            match result {
                PrstResult::Prime {
                    is_deterministic, ..
                } => {
                    assert!(
                        is_deterministic,
                        "3*2^50000+1 should be deterministic prime"
                    );
                }
                PrstResult::Unavailable { .. } => {
                    eprintln!("PRST not available, skipping integration test");
                }
                PrstResult::Composite => panic!("3*2^50000+1 should be prime!"),
            }
        }
    }
}
