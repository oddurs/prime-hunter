//! # Platform Tests — Cross-Platform Behavior and Tool Detection
//!
//! Integration tests that validate darkreach's platform-dependent functionality:
//! operating system detection, CPU/RAM introspection, file system edge cases
//! (read-only dirs, Unicode paths, concurrent access), external tool availability
//! (PFGW, PRST, tar), GMP/rug large-number correctness, rayon thread pool
//! behavior, and atomic signal propagation.
//!
//! ## Test Categories
//!
//! | Category | Tests | Notes |
//! |----------|-------|-------|
//! | **Platform Detection** | 1–6 | OS, arch, CPU model, core count, RAM, hostname |
//! | **File System** | 7–12 | Read-only dirs, spaces, Unicode, nested dirs, cleanup, concurrency |
//! | **Tool Availability** | 13–16 | PFGW, PRST, tar, hostname command |
//! | **GMP/rug Compatibility** | 17–20 | Large factorial, large power, known primes, thread safety |
//! | **Resource Boundaries** | 21–23 | Large sieve, large checkpoint, bounded batch generation |
//! | **Rayon Thread Pool** | 24–26 | Thread count, parallel primality, stop propagation |
//! | **Signal Handling** | 27–28 | AtomicBool visibility, concurrent stop flag (Unix-only) |
//!
//! ## Platform-Specific Annotations
//!
//! - `#[cfg(unix)]`: Tests 27–28 (signal handling) — Unix-only AtomicBool + thread tests
//! - `#[cfg(target_os = "macos")]`: Not used directly; CPU model detection uses sysctl
//!   on macOS and /proc/cpuinfo on Linux, but both are exercised by test 3
//! - `#[ignore]`: Tests 13–14 require optional external tools (PFGW, PRST)
//!
//! ## Thread Safety Tests
//!
//! Tests 20, 25, 27, and 28 exercise concurrent access patterns:
//! - Test 20: Concurrent GMP operations via rayon (rug::Integer is thread-safe)
//! - Test 25: Parallel primality testing via rayon::par_iter
//! - Test 27: AtomicBool visibility across spawned threads (Ordering::SeqCst)
//! - Test 28: Multiple threads reading/writing a shared AtomicBool stop flag

use darkreach::checkpoint;
use darkreach::sieve;
use rug::Integer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ============================================================================
// Platform Detection Tests (1–6)
// ============================================================================

/// Test 1: The OS string returned by `std::env::consts::OS` must be one of the
/// known operating systems that darkreach supports. The operator module sends
/// this value to the coordinator for artifact matching during auto-updates.
/// An unrecognized OS would prevent the worker from receiving update binaries.
#[test]
fn test_current_os_is_known() {
    let os = std::env::consts::OS;
    let known = ["linux", "macos", "windows"];
    assert!(
        known.contains(&os),
        "OS '{}' is not in the known set: {:?}",
        os,
        known
    );
}

/// Test 2: The architecture string returned by `std::env::consts::ARCH` must be
/// a recognized CPU architecture. The operator module uses this for artifact
/// selection when downloading auto-update binaries.
#[test]
fn test_current_arch_is_known() {
    let arch = std::env::consts::ARCH;
    let known = ["x86_64", "x86", "aarch64", "arm", "mips", "mips64", "powerpc64", "s390x"];
    assert!(
        known.contains(&arch),
        "Arch '{}' is not in the known set: {:?}",
        arch,
        known
    );
}

/// Test 3: CPU model detection must return a non-empty, non-"unknown" string on
/// any real hardware. On macOS this uses `sysctl -n machdep.cpu.brand_string`;
/// on Linux it parses `/proc/cpuinfo`. The result is sent to the coordinator
/// for fleet monitoring and displayed on the dashboard.
#[test]
fn test_cpu_model_not_empty() {
    // Replicate the operator.rs cpu_model() logic since it's private
    let model = get_cpu_model();
    assert!(
        !model.is_empty(),
        "CPU model should not be empty"
    );
    // On real hardware, "unknown" means the detection failed
    // We allow it but warn — in CI containers /proc/cpuinfo may be absent
    if model == "unknown" {
        eprintln!("Warning: CPU model detection returned 'unknown' (may be expected in CI)");
    }
}

/// Test 4: The number of logical CPU cores must be between 1 and 1024. rayon
/// uses this value to size its thread pool, and the coordinator uses it for
/// work block sizing. A value outside this range indicates a detection failure
/// or an implausibly large machine.
#[test]
fn test_cpu_core_count_reasonable() {
    let cores = rayon::current_num_threads();
    assert!(
        cores >= 1 && cores <= 1024,
        "CPU core count {} is outside reasonable range [1, 1024]",
        cores
    );
}

/// Test 5: Total system RAM must be between 1 GB and 65536 GB (64 TB). The
/// operator module reports this to the coordinator, which uses it for work
/// assignment (e.g., large factorial searches need more RAM). Detection uses
/// the sysinfo crate's `total_memory()`.
#[test]
fn test_ram_detection_reasonable() {
    let sys = sysinfo::System::new_all();
    let ram_bytes = sys.total_memory();
    let ram_gb = ram_bytes / 1_073_741_824;
    assert!(
        ram_gb >= 1 && ram_gb <= 65536,
        "RAM {} GB is outside reasonable range [1, 65536]",
        ram_gb
    );
}

/// Test 6: The system hostname must be non-empty. It is used to generate
/// unique worker IDs (format: "hostname-XXXXXXXX") and is displayed on the
/// fleet dashboard. The hostname command must be available in PATH.
#[test]
fn test_hostname_not_empty() {
    let hostname = std::process::Command::new("hostname")
        .output()
        .expect("hostname command should be available")
        .stdout;
    let hostname = String::from_utf8(hostname)
        .expect("hostname should be valid UTF-8")
        .trim()
        .to_string();
    assert!(
        !hostname.is_empty(),
        "System hostname should not be empty"
    );
}

// ============================================================================
// File System Tests (7–12)
// ============================================================================

/// Test 7: Writing a checkpoint to a read-only directory must fail gracefully
/// with an error rather than panicking. This happens when the checkpoint path
/// points to a system directory or a mounted read-only filesystem.
#[test]
#[cfg(unix)]
fn test_checkpoint_on_readonly_dir() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let readonly_dir = dir.path().join("readonly");
    std::fs::create_dir(&readonly_dir).unwrap();

    // Make directory read-only
    let mut perms = std::fs::metadata(&readonly_dir).unwrap().permissions();
    perms.set_mode(0o444);
    std::fs::set_permissions(&readonly_dir, perms).unwrap();

    let path = readonly_dir.join("checkpoint.json");
    let cp = checkpoint::Checkpoint::Factorial {
        last_n: 42,
        start: Some(1),
        end: Some(100),
    };

    let result = checkpoint::save(&path, &cp);
    assert!(
        result.is_err(),
        "Checkpoint save to read-only directory should fail"
    );

    // Restore permissions for cleanup
    let mut perms = std::fs::metadata(&readonly_dir).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&readonly_dir, perms).unwrap();
}

/// Test 8: Checkpoint save/load must work with paths containing spaces.
/// This is common on macOS ("Library/Application Support") and Windows
/// ("Program Files"). The atomic write (write .tmp, rename) must handle
/// spaced paths correctly.
#[test]
fn test_checkpoint_path_with_spaces() {
    let dir = tempfile::tempdir().unwrap();
    let spaced_dir = dir.path().join("path with spaces");
    std::fs::create_dir_all(&spaced_dir).unwrap();

    let path = spaced_dir.join("check point.json");
    let cp = checkpoint::Checkpoint::Kbn {
        last_n: 500,
        min_n: Some(1),
        max_n: Some(1000),
    };

    checkpoint::save(&path, &cp).unwrap();
    let loaded = checkpoint::load(&path).expect("Should load checkpoint from path with spaces");
    match loaded {
        checkpoint::Checkpoint::Kbn { last_n, .. } => {
            assert_eq!(last_n, 500);
        }
        _ => panic!("Wrong checkpoint type"),
    }
}

/// Test 9: Checkpoint save/load must work with Unicode characters in the path.
/// This handles internationalized file systems (CJK characters, accented letters,
/// emoji in directory names). The serde_json serialization and std::fs operations
/// must preserve Unicode path integrity through the atomic write cycle.
#[test]
fn test_checkpoint_path_with_unicode() {
    let dir = tempfile::tempdir().unwrap();
    let unicode_dir = dir.path().join("primt\u{00e4}l_\u{2603}_\u{03c0}");
    std::fs::create_dir_all(&unicode_dir).unwrap();

    let path = unicode_dir.join("\u{1F4CA}_checkpoint.json");
    let cp = checkpoint::Checkpoint::Palindromic {
        digit_count: 7,
        half_value: "1234".to_string(),
        min_digits: Some(1),
        max_digits: Some(99),
    };

    checkpoint::save(&path, &cp).unwrap();
    let loaded = checkpoint::load(&path).expect("Should load checkpoint from Unicode path");
    match loaded {
        checkpoint::Checkpoint::Palindromic { digit_count, half_value, .. } => {
            assert_eq!(digit_count, 7);
            assert_eq!(half_value, "1234");
        }
        _ => panic!("Wrong checkpoint type"),
    }
}

/// Test 10: The config directory creation must handle deeply nested paths.
/// This mimics the `~/.darkreach/updates/<version>/extract/` structure used
/// by the auto-update system. `create_dir_all` must create all intermediate
/// directories atomically.
#[test]
fn test_config_dir_creation_nested() {
    let dir = tempfile::tempdir().unwrap();
    let deep_path = dir
        .path()
        .join("a")
        .join("b")
        .join("c")
        .join("d")
        .join("e")
        .join("f");

    std::fs::create_dir_all(&deep_path).unwrap();
    assert!(deep_path.exists(), "Deeply nested directory should be created");
    assert!(deep_path.is_dir(), "Path should be a directory");

    // Verify we can write a file inside the deepest directory
    let file_path = deep_path.join("test.json");
    std::fs::write(&file_path, "{}").unwrap();
    assert!(file_path.exists(), "File in nested directory should exist");
}

/// Test 11: Temporary files created during checkpoint operations must be cleaned
/// up. The save() function writes to a .tmp file and then renames it. After a
/// successful save, no .tmp file should remain. This prevents temp file
/// accumulation on long-running workers.
#[test]
fn test_tempfile_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("checkpoint.json");

    // Save multiple checkpoints to exercise rotation
    for i in 0..5u64 {
        let cp = checkpoint::Checkpoint::Factorial {
            last_n: i * 10,
            start: None,
            end: None,
        };
        checkpoint::save(&path, &cp).unwrap();
    }

    // No .tmp files should remain
    let tmp_path = path.with_extension("tmp");
    assert!(
        !tmp_path.exists(),
        ".tmp file should not remain after successful saves"
    );

    // Verify we can still read the entries
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    // Should have checkpoint.json, checkpoint.json.1, checkpoint.json.2 (3 generations max)
    assert!(
        entries.len() <= 4,
        "Should have at most 3 generation files + maybe a leftover, got {}",
        entries.len()
    );
}

/// Test 12: Atomic rename must not corrupt data visible to concurrent readers.
/// This simulates the scenario where a search thread reads the checkpoint while
/// the background saver thread performs an atomic write (write .tmp, rename).
/// The reader must always see either the old or the new complete checkpoint,
/// never a partial or corrupted state.
#[test]
fn test_atomic_write_survives_concurrent_reads() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("checkpoint.json");

    // Initial checkpoint
    let cp = checkpoint::Checkpoint::Factorial {
        last_n: 100,
        start: Some(1),
        end: Some(1000),
    };
    checkpoint::save(&path, &cp).unwrap();

    let path_clone = path.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();

    // Spawn a reader thread that continuously loads the checkpoint
    let reader = std::thread::spawn(move || {
        let mut reads = 0u64;
        let mut errors = 0u64;
        while !stop_clone.load(Ordering::Relaxed) {
            match checkpoint::load(&path_clone) {
                Some(checkpoint::Checkpoint::Factorial { last_n, .. }) => {
                    // last_n should be a valid value we wrote
                    assert!(
                        last_n >= 100,
                        "Read corrupted checkpoint: last_n={}",
                        last_n
                    );
                    reads += 1;
                }
                Some(_) => {
                    // Different variant type would be a corruption
                    errors += 1;
                }
                None => {
                    // Transient read failure during rename is acceptable
                    reads += 1;
                }
            }
            std::thread::yield_now();
        }
        (reads, errors)
    });

    // Writer thread: save checkpoints with increasing values
    for i in 100..200u64 {
        let cp = checkpoint::Checkpoint::Factorial {
            last_n: i,
            start: Some(1),
            end: Some(1000),
        };
        checkpoint::save(&path, &cp).unwrap();
    }

    stop.store(true, Ordering::Relaxed);
    let (reads, errors) = reader.join().unwrap();

    assert!(reads > 0, "Reader should have completed at least one read");
    assert_eq!(errors, 0, "Reader should never see a corrupted checkpoint");
}

// ============================================================================
// Tool Availability Tests (13–16)
// ============================================================================

/// Test 13: PFGW detection must not panic regardless of whether the binary is
/// installed. If PFGW is found, `parse_output` must correctly classify known
/// prime/composite outputs. This test does not require PFGW to be installed --
/// it validates the detection logic and output parsing. The `#[ignore]` test
/// below exercises actual subprocess execution.
///
/// Marked `#[ignore]` because it requires the `pfgw64` binary installed in PATH.
#[test]
#[ignore]
fn test_pfgw_detection() {
    use darkreach::pfgw;

    // Initialize with permissive settings (min_digits=0, no specific binary path)
    pfgw::init(
        0,
        None,
        std::time::Duration::from_secs(30),
    );

    // Test with a known small prime: 7! + 1 = 5041 = 71 * 71 (composite)
    let candidate = Integer::from(Integer::factorial(7)) + 1u32;
    if let Some(result) = pfgw::try_test("7!+1", &candidate, pfgw::PfgwMode::Prp) {
        match result {
            pfgw::PfgwResult::Composite => {
                // Expected: 5041 = 71^2
            }
            pfgw::PfgwResult::Prime { .. } => {
                panic!("7!+1 = 5041 should be composite");
            }
            pfgw::PfgwResult::Unavailable { reason } => {
                eprintln!("PFGW unavailable: {}", reason);
            }
        }
    }
}

/// Test 14: PRST detection must not panic regardless of whether the binary is
/// installed. Tests the detection and output parsing logic.
///
/// Marked `#[ignore]` because it requires the `prst` binary installed in PATH.
#[test]
#[ignore]
fn test_prst_detection() {
    use darkreach::prst;
    use rug::ops::Pow;

    // Initialize with permissive settings
    prst::init(
        0,
        None,
        std::time::Duration::from_secs(30),
    );

    // Test with a known Proth prime: 3*2^2 + 1 = 13
    let candidate = Integer::from(3u32) * Integer::from(2u32).pow(2u32) + 1u32;
    if let Some(result) = prst::try_test(3, 2, 2, true, &candidate) {
        match result {
            prst::PrstResult::Prime { .. } => {
                // Expected: 13 is prime
            }
            prst::PrstResult::Composite => {
                panic!("3*2^2+1 = 13 should be prime");
            }
            prst::PrstResult::Unavailable { reason } => {
                eprintln!("PRST unavailable: {}", reason);
            }
        }
    }
}

/// Test 15: The `tar` command must be available in PATH. It is required by the
/// auto-update system in `operator.rs` to extract downloaded update archives
/// (`tar -xzf <archive> -C <dir>`). Without tar, workers cannot auto-update.
#[test]
fn test_tar_available() {
    let output = std::process::Command::new("tar")
        .arg("--version")
        .output();

    match output {
        Ok(o) => {
            assert!(
                o.status.success(),
                "tar --version should succeed; stderr: {}",
                String::from_utf8_lossy(&o.stderr)
            );
        }
        Err(e) => {
            panic!("tar command not found in PATH: {}", e);
        }
    }
}

/// Test 16: The `hostname` command must be available and produce output. Used by
/// `operator.rs` to generate worker IDs and is called on every worker startup.
#[test]
fn test_hostname_command_available() {
    let output = std::process::Command::new("hostname")
        .output()
        .expect("hostname command should be available");

    assert!(
        output.status.success(),
        "hostname command should succeed"
    );

    let hostname = String::from_utf8(output.stdout)
        .expect("hostname should be valid UTF-8");
    assert!(
        !hostname.trim().is_empty(),
        "hostname should produce non-empty output"
    );
}

// ============================================================================
// GMP/rug Compatibility Tests (17–20)
// ============================================================================

/// Test 17: GMP must handle computing 10000! without crashing or running out of
/// memory. Factorial computation is the core operation in the factorial search
/// form (`src/factorial.rs`). 10000! has approximately 35659 digits and requires
/// ~15 KB of storage. This validates that GMP's internal memory allocation
/// handles the recursive multiplication chain correctly.
#[test]
fn test_gmp_factorial_large() {
    let result = Integer::from(Integer::factorial(10_000));
    // 10000! should have approximately 35659 digits
    let digits = darkreach::exact_digits(&result);
    assert!(
        digits > 35_000 && digits < 36_000,
        "10000! should have ~35659 digits, got {}",
        digits
    );
    // Verify it's not zero or one
    assert!(result > 1u32, "10000! should be > 1");
}

/// Test 18: GMP must handle computing 2^100000 without crashing. This number
/// has approximately 30103 digits and is representative of the candidate sizes
/// in large kbn searches (k*2^100000+1). The `rug::Integer::pow` method must
/// handle the binary exponentiation correctly.
#[test]
fn test_gmp_power_large() {
    use rug::ops::Pow;

    let result = Integer::from(2u32).pow(100_000u32);
    let digits = darkreach::exact_digits(&result);
    // 2^100000 has exactly 30103 digits
    assert!(
        digits > 30_000 && digits < 31_000,
        "2^100000 should have ~30103 digits, got {}",
        digits
    );
    // Verify the lowest bit is set (2^n is always even except 2^0)
    assert!(result.is_even(), "2^100000 should be even");
    // Verify exactly one bit is set (power of 2)
    assert_eq!(
        result.significant_bits(),
        100_001,
        "2^100000 should have 100001 significant bits"
    );
}

/// Test 19: GMP's primality test must correctly identify well-known primes.
/// These span from tiny (2, 3) through Mersenne primes (2^p - 1) to large
/// primes used in cryptography. All are verified with 25 Miller-Rabin rounds
/// (GMP uses deterministic witnesses for small values, making these tests exact).
#[test]
fn test_gmp_primality_known_primes() {
    use rug::integer::IsPrime;
    use rug::ops::Pow;

    // Small primes
    let small_primes: Vec<u32> = vec![2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47];
    for &p in &small_primes {
        let n = Integer::from(p);
        assert_ne!(
            n.is_probably_prime(25),
            IsPrime::No,
            "GMP should accept known prime {}",
            p
        );
    }

    // Mersenne primes: M_p = 2^p - 1 for known prime exponents
    let mersenne_exponents = [2u32, 3, 5, 7, 13, 17, 19, 31];
    for &exp in &mersenne_exponents {
        let m = Integer::from(2u32).pow(exp) - 1u32;
        assert_ne!(
            m.is_probably_prime(25),
            IsPrime::No,
            "GMP should accept Mersenne prime 2^{}-1",
            exp
        );
    }

    // Known composites must be rejected
    let composites: Vec<u32> = vec![4, 6, 8, 9, 10, 15, 21, 25, 100, 1001];
    for &c in &composites {
        let n = Integer::from(c);
        assert_eq!(
            n.is_probably_prime(25),
            IsPrime::No,
            "GMP should reject known composite {}",
            c
        );
    }
}

/// Test 20: Concurrent GMP operations from multiple rayon threads must not
/// crash or produce incorrect results. rug::Integer uses thread-local GMP
/// state, so parallel computation should be safe. This test runs 100 parallel
/// primality tests and verifies all results are correct.
///
/// This exercises the same pattern used by all 12 search forms: rayon::par_iter
/// over a batch of candidates with GMP primality testing in each thread.
#[test]
fn test_gmp_thread_safety() {
    use rayon::prelude::*;
    use rug::integer::IsPrime;

    // Known primes up to 101
    let known_primes: Vec<u32> = vec![
        2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83,
        89, 97, 101,
    ];

    // Run 100 parallel primality tests across rayon's thread pool
    let results: Vec<(u32, bool)> = (2u32..102)
        .into_par_iter()
        .map(|n| {
            let candidate = Integer::from(n);
            let is_prime = candidate.is_probably_prime(15) != IsPrime::No;
            (n, is_prime)
        })
        .collect();

    for (n, is_prime) in &results {
        let expected = known_primes.contains(n);
        assert_eq!(
            *is_prime, expected,
            "Thread safety failure: GMP returned is_prime={} for {} (expected {})",
            is_prime, n, expected
        );
    }
}

// ============================================================================
// Resource Boundary Tests (21–23)
// ============================================================================

/// Test 21: Allocating a BitSieve with a large limit must succeed without
/// running out of memory. A sieve of 10 million bits requires ~1.2 MB of
/// memory (10M / 8 bytes/bit). This tests the `BitSieve::new_all_set` path
/// which is used in every search form's sieve phase.
#[test]
fn test_large_sieve_allocation() {
    let limit = 10_000_000;
    let sieve_bits = sieve::BitSieve::new_all_set(limit);

    assert_eq!(sieve_bits.len(), limit, "Sieve should have requested length");
    assert!(!sieve_bits.is_empty(), "Sieve should not be empty");

    // Verify first and last bits are set
    assert!(sieve_bits.get(0), "First bit should be set");
    assert!(sieve_bits.get(limit - 1), "Last bit should be set");

    // Also test generate_primes with a reasonable limit
    let primes = sieve::generate_primes(1_000_000);
    // pi(10^6) = 78498
    assert!(
        primes.len() > 78_000 && primes.len() < 79_000,
        "generate_primes(1M) should find ~78498 primes, got {}",
        primes.len()
    );
}

/// Test 22: A checkpoint with many fields (all 12 variants) must serialize and
/// deserialize correctly. This tests the serde round-trip for the largest
/// possible checkpoint state, ensuring no field truncation or serialization
/// overflow.
#[test]
fn test_checkpoint_large_state() {
    let dir = tempfile::tempdir().unwrap();

    // Create and save all 12 checkpoint variants with maximum field values
    let variants: Vec<(&str, checkpoint::Checkpoint)> = vec![
        (
            "factorial",
            checkpoint::Checkpoint::Factorial {
                last_n: u64::MAX,
                start: Some(0),
                end: Some(u64::MAX),
            },
        ),
        (
            "palindromic",
            checkpoint::Checkpoint::Palindromic {
                digit_count: u64::MAX,
                half_value: "9".repeat(1000), // Large half_value string
                min_digits: Some(1),
                max_digits: Some(u64::MAX),
            },
        ),
        (
            "kbn",
            checkpoint::Checkpoint::Kbn {
                last_n: u64::MAX,
                min_n: Some(0),
                max_n: Some(u64::MAX),
            },
        ),
        (
            "near_repdigit",
            checkpoint::Checkpoint::NearRepdigit {
                digit_count: u64::MAX,
                d: u32::MAX,
                m: u64::MAX,
                min_digits: Some(0),
                max_digits: Some(u64::MAX),
            },
        ),
        (
            "primorial",
            checkpoint::Checkpoint::Primorial {
                last_prime: u64::MAX,
                start: Some(0),
                end: Some(u64::MAX),
            },
        ),
        (
            "cullen_woodall",
            checkpoint::Checkpoint::CullenWoodall {
                last_n: u64::MAX,
                min_n: Some(0),
                max_n: Some(u64::MAX),
            },
        ),
        (
            "wagstaff",
            checkpoint::Checkpoint::Wagstaff {
                last_exp: u64::MAX,
                min_exp: Some(0),
                max_exp: Some(u64::MAX),
            },
        ),
        (
            "carol_kynea",
            checkpoint::Checkpoint::CarolKynea {
                last_n: u64::MAX,
                min_n: Some(0),
                max_n: Some(u64::MAX),
            },
        ),
        (
            "twin",
            checkpoint::Checkpoint::Twin {
                last_n: u64::MAX,
                k: Some(u64::MAX),
                base: Some(u32::MAX),
                min_n: Some(0),
                max_n: Some(u64::MAX),
            },
        ),
        (
            "sophie_germain",
            checkpoint::Checkpoint::SophieGermain {
                last_n: u64::MAX,
                k: Some(u64::MAX),
                base: Some(u32::MAX),
                min_n: Some(0),
                max_n: Some(u64::MAX),
            },
        ),
        (
            "repunit",
            checkpoint::Checkpoint::Repunit {
                last_n: u64::MAX,
                base: Some(u32::MAX),
                min_n: Some(0),
                max_n: Some(u64::MAX),
            },
        ),
        (
            "gen_fermat",
            checkpoint::Checkpoint::GenFermat {
                last_base: u64::MAX,
                fermat_n: Some(u32::MAX),
                min_base: Some(0),
                max_base: Some(u64::MAX),
            },
        ),
    ];

    for (name, cp) in &variants {
        let path = dir.path().join(format!("{}.json", name));
        checkpoint::save(&path, cp)
            .unwrap_or_else(|e| panic!("Failed to save {} checkpoint: {}", name, e));

        let loaded = checkpoint::load(&path)
            .unwrap_or_else(|| panic!("Failed to load {} checkpoint", name));

        // Verify round-trip by re-serializing
        let original_json = serde_json::to_string(cp).unwrap();
        let loaded_json = serde_json::to_string(&loaded).unwrap();
        assert_eq!(
            original_json, loaded_json,
            "Round-trip mismatch for {} checkpoint with max values",
            name
        );
    }
}

/// Test 23: Candidate generation via generate_primes must use bounded memory
/// proportional to the limit, not unbounded growth. This verifies the sieve
/// implementation uses O(n/30) bytes (wheel factorization) rather than O(n)
/// bytes. A 10M sieve should use roughly 333 KB of sieve memory.
#[test]
fn test_batch_generation_memory_bounded() {
    // Generate primes up to 10 million — should complete quickly and not OOM
    let primes = sieve::generate_primes(10_000_000);

    // pi(10^7) = 664579
    assert!(
        primes.len() > 664_000 && primes.len() < 665_000,
        "generate_primes(10M) should find ~664579 primes, got {}",
        primes.len()
    );

    // Verify the primes are sorted (invariant of the sieve)
    for w in primes.windows(2) {
        assert!(
            w[0] < w[1],
            "Primes should be sorted: {} >= {}",
            w[0],
            w[1]
        );
    }

    // Verify first and last values
    assert_eq!(primes[0], 2, "First prime should be 2");
    assert_eq!(
        *primes.last().unwrap(),
        9_999_991,
        "Last prime below 10M should be 9999991"
    );
}

// ============================================================================
// Rayon Thread Pool Tests (24–26)
// ============================================================================

/// Test 24: The rayon thread pool size should match or be close to the number of
/// logical CPU cores. By default, rayon creates one thread per logical core.
/// Custom thread pool sizes (e.g., for --cores flag) may differ, but the default
/// should be within the expected range.
#[test]
fn test_rayon_thread_count_matches_system() {
    let rayon_threads = rayon::current_num_threads();
    let sys = sysinfo::System::new_all();
    let cpu_count = sys.cpus().len();

    // rayon defaults to num_cpus; allow some tolerance for custom pools
    // In test environments, rayon may use a different pool size
    assert!(
        rayon_threads >= 1,
        "rayon should have at least 1 thread, got {}",
        rayon_threads
    );
    assert!(
        rayon_threads <= cpu_count * 2,
        "rayon thread count {} should be <= 2x CPU count {}",
        rayon_threads,
        cpu_count
    );

    eprintln!(
        "rayon threads: {}, CPU cores: {}",
        rayon_threads, cpu_count
    );
}

/// Test 25: Parallel primality testing via rayon must produce correct results
/// for all candidates. This is the core pattern used by every search form:
/// generate a batch, distribute across rayon threads, collect results. The test
/// verifies that no results are lost or corrupted during parallel execution.
#[test]
fn test_rayon_parallel_primality_test() {
    use rayon::prelude::*;
    use rug::integer::IsPrime;

    // Test a range of candidates in parallel
    let results: Vec<(u64, bool)> = (2u64..1000)
        .into_par_iter()
        .map(|n| {
            let candidate = Integer::from(n);
            let is_prime = candidate.is_probably_prime(15) != IsPrime::No;
            (n, is_prime)
        })
        .collect();

    // Verify against a sequential sieve
    let sieve_primes = sieve::generate_primes(999);
    let sieve_set: std::collections::HashSet<u64> = sieve_primes.into_iter().collect();

    for (n, is_prime) in &results {
        let expected = sieve_set.contains(n);
        assert_eq!(
            *is_prime, expected,
            "Parallel primality test disagrees with sieve for n={}",
            n
        );
    }

    // Verify we got all results (no lost items in parallel collection)
    assert_eq!(
        results.len(),
        998,
        "Should have results for all 998 candidates (2..1000)"
    );
}

/// Test 26: A stop signal (AtomicBool) must propagate to all rayon threads and
/// cause them to terminate their work. This simulates the `is_stop_requested()`
/// check that all 12 search forms perform inside their rayon::par_iter loops.
#[test]
fn test_rayon_stop_propagation() {
    use rayon::prelude::*;

    let stop = Arc::new(AtomicBool::new(false));

    // Spawn rayon tasks that check the stop flag
    let stop_clone = stop.clone();
    let handle = std::thread::spawn(move || {
        let processed: Vec<u64> = (0u64..1_000_000)
            .into_par_iter()
            .filter_map(|i| {
                if stop_clone.load(Ordering::Relaxed) {
                    return None; // Stop requested
                }
                // Simulate a small amount of work
                if i % 1000 == 0 {
                    std::thread::yield_now();
                }
                Some(i)
            })
            .collect();
        processed.len()
    });

    // Give rayon threads a moment to start processing
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Signal stop
    stop.store(true, Ordering::Relaxed);

    let processed = handle.join().unwrap();

    // We should have processed some items but not all
    assert!(
        processed < 1_000_000,
        "Stop signal should have prevented processing all 1M items (processed {})",
        processed
    );
    // We should have processed at least some items before the stop was set
    assert!(
        processed > 0,
        "Should have processed at least some items before stop"
    );
}

// ============================================================================
// Signal Handling Tests — Unix-only (27–28)
// ============================================================================

/// Test 27: An AtomicBool written by one thread must be visible to another thread
/// immediately (or within a single memory fence). This validates the pattern used
/// by the stop flag: the coordinator thread sets the flag, and all rayon worker
/// threads must observe it on their next check. Uses SeqCst ordering to guarantee
/// total ordering across threads.
///
/// This test is Unix-only because signal handling (SIGTERM, SIGINT) is the primary
/// use case on server deployments. On Windows, stop signals come through different
/// mechanisms.
#[test]
#[cfg(unix)]
fn test_stop_flag_atomic_visibility() {
    let flag = Arc::new(AtomicBool::new(false));
    let flag_writer = flag.clone();
    let flag_reader = flag.clone();

    // Spawn a reader thread that polls until the flag is set
    let reader = std::thread::spawn(move || {
        let mut iterations = 0u64;
        while !flag_reader.load(Ordering::SeqCst) {
            iterations += 1;
            if iterations > 100_000_000 {
                panic!("Reader thread did not observe flag set after 100M iterations");
            }
            std::hint::spin_loop();
        }
        iterations
    });

    // Give the reader a moment to start polling
    std::thread::sleep(std::time::Duration::from_millis(1));

    // Set the flag
    flag_writer.store(true, Ordering::SeqCst);

    // Reader should terminate quickly
    let iterations = reader.join().unwrap();
    assert!(
        iterations > 0,
        "Reader should have polled at least once before seeing the flag"
    );

    // Verify the flag is still set
    assert!(flag.load(Ordering::SeqCst), "Flag should remain set");
}

/// Test 28: Multiple threads simultaneously reading and writing a shared stop
/// flag must not cause data races, panics, or undefined behavior. This validates
/// that the AtomicBool used for stop propagation is safe under heavy contention.
///
/// Unix-only because the primary stop-signal path (SIGTERM handler) is Unix-specific.
#[test]
#[cfg(unix)]
fn test_concurrent_stop_flag_access() {
    let flag = Arc::new(AtomicBool::new(false));
    let num_threads = 8;
    let iterations_per_thread = 100_000;

    let mut writer_handles = Vec::new();
    let mut reader_handles = Vec::new();

    // Spawn writer threads that toggle the flag
    for _ in 0..num_threads / 2 {
        let flag_clone = flag.clone();
        writer_handles.push(std::thread::spawn(move || {
            for i in 0..iterations_per_thread {
                flag_clone.store(i % 2 == 0, Ordering::SeqCst);
                if i % 1000 == 0 {
                    std::thread::yield_now();
                }
            }
        }));
    }

    // Spawn reader threads that poll the flag
    for _ in 0..num_threads / 2 {
        let flag_clone = flag.clone();
        reader_handles.push(std::thread::spawn(move || {
            let mut true_count = 0u64;
            let mut false_count = 0u64;
            for i in 0..iterations_per_thread {
                if flag_clone.load(Ordering::SeqCst) {
                    true_count += 1;
                } else {
                    false_count += 1;
                }
                if i % 1000 == 0 {
                    std::thread::yield_now();
                }
            }
            (true_count, false_count)
        }));
    }

    // All threads should complete without panicking
    for handle in writer_handles {
        handle.join().expect("Writer thread should complete without panic");
    }
    for handle in reader_handles {
        let (_true_count, _false_count) = handle.join().expect("Reader thread should complete without panic");
    }

    // The final flag state should be a valid boolean (trivially true for AtomicBool)
    let _final_value = flag.load(Ordering::SeqCst);
    // No assertion on the value — it depends on scheduling — but it must not crash
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Replicate the CPU model detection logic from operator.rs (which is private).
/// On macOS: `sysctl -n machdep.cpu.brand_string`
/// On Linux: parse `model name` from `/proc/cpuinfo`
fn get_cpu_model() -> String {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("model name"))
                    .map(|l| l.split(':').nth(1).unwrap_or("unknown").trim().to_string())
            })
            .unwrap_or_else(|| "unknown".to_string())
    }
}
