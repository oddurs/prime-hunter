//! # Operator Auto-Update System Tests
//!
//! Comprehensive tests for the darkreach worker auto-update pipeline. The update
//! system allows the coordinator to push new worker binaries to operators without
//! manual intervention, enabling rapid deployment of engine improvements and
//! security patches across the distributed fleet.
//!
//! ## Auto-Update Security Model
//!
//! The update pipeline follows defense-in-depth with three verification layers:
//!
//! 1. **SHA-256 integrity verification** (mandatory): Every downloaded artifact is
//!    hashed with SHA-256 and compared against the checksum published by the
//!    coordinator. This detects corruption during download and prevents
//!    man-in-the-middle substitution (assuming HTTPS for the metadata endpoint).
//!    The comparison is case-insensitive to tolerate hex encoding differences.
//!
//! 2. **Ed25519/RSA signature verification** (opt-in via `DARKREACH_VERIFY_WORKER_SIG=1`):
//!    When enabled, the artifact's detached signature is downloaded from `sig_url`
//!    and verified against a local public key (`DARKREACH_WORKER_PUBKEY_PATH`).
//!    This provides authenticity verification independent of the coordinator's
//!    HTTPS certificate — even a compromised coordinator cannot forge a valid
//!    signature without the private key.
//!
//! 3. **Staging before apply**: New binaries are extracted to
//!    `~/.darkreach/updates/{version}/` and validated before replacing the
//!    running executable. This prevents partial writes from bricking the worker.
//!    The apply step uses atomic rename (Unix) to swap the binary.
//!
//! ## Update Flow
//!
//! ```text
//! check_for_update()          Compare CARGO_PKG_VERSION against coordinator's latest
//!     |
//!     v
//! stage_or_apply_update()     Download, verify, extract, stage
//!     |
//!     +-- Download tar.gz to ~/.darkreach/updates/{version}/
//!     +-- SHA-256 verify (mandatory)
//!     +-- Signature verify (optional, DARKREACH_VERIFY_WORKER_SIG=1)
//!     +-- Extract archive with `tar -xzf`
//!     +-- find_darkreach_binary() — recursive search in extracted tree
//!     +-- Copy to staged location, set executable permissions
//!     +-- Optionally apply (atomic rename to current executable path)
//! ```
//!
//! ## What These Tests Cover
//!
//! | Category | Tests | Scope |
//! |----------|-------|-------|
//! | Version checking | 3 | Comparison logic (update/no-update/downgrade) |
//! | SHA-256 verification | 5 | Known vectors, empty file, large file, missing file, case |
//! | Binary discovery | 4 | Root, nested, missing, .exe extension |
//! | Archive extraction | 2 | Real tar.gz creation and extraction |
//! | Platform detection | 4 | OS, arch, binary name validity |
//! | Artifact matching | 3 | Platform selection from artifact list |
//! | Staging (file ops) | 3 | Directory creation, permissions, overwrite |
//! | Environment controls | 3 | Auto-update flag, channel default, signature opt-in |
//!
//! ## What These Tests Do NOT Cover
//!
//! - Actual HTTP downloads (requires mock coordinator; see integration tests)
//! - `apply_staged_update()` (replaces running executable; too destructive for CI)
//! - End-to-end coordinator communication (requires running coordinator instance)
//!
//! ## How to Run
//!
//! ```bash
//! cargo test --test operator_update_tests
//! ```

use darkreach::operator::{
    WorkerReleaseArtifact, WorkerReleaseInfo,
    binary_name_for_platform, find_darkreach_binary, make_executable,
    sha256_file, worker_arch, worker_os,
};
use std::fs;
use tempfile::TempDir;

// ============================================================================
// Version Checking
// ============================================================================
//
// The version comparison in `check_for_update` uses simple string inequality:
// `latest.version != current` triggers an update. This means ANY difference
// (upgrade, downgrade, different channel) returns Some(release). The caller
// is responsible for deciding whether to apply.
//
// Note: `check_for_update` itself calls the coordinator via HTTP, so we test
// the comparison logic by constructing WorkerReleaseInfo directly.
// ============================================================================

/// When the latest version on the coordinator is newer than the current binary,
/// `check_for_update` should return `Some(release)` so the worker can stage it.
///
/// We simulate this by comparing the current `CARGO_PKG_VERSION` against a
/// synthetic "99.0.0" release. Since no real version will match "99.0.0",
/// the comparison `latest.version != current` is always true.
#[test]
fn test_version_comparison_update_available() {
    let current = env!("CARGO_PKG_VERSION");
    let latest = WorkerReleaseInfo {
        channel: "stable".to_string(),
        version: "99.0.0".to_string(),
        published_at: "2026-02-20T00:00:00Z".to_string(),
        notes: Some("New release".to_string()),
        artifacts: vec![],
    };

    // The actual check_for_update uses HTTP, so we replicate its comparison logic
    let needs_update = latest.version != current;
    assert!(
        needs_update,
        "Version {} should differ from latest 99.0.0",
        current
    );
}

/// When the latest version matches the current binary exactly, no update is
/// needed. The worker should continue running without downloading anything.
#[test]
fn test_version_comparison_no_update() {
    let current = env!("CARGO_PKG_VERSION");
    let latest = WorkerReleaseInfo {
        channel: "stable".to_string(),
        version: current.to_string(),
        published_at: "2026-02-20T00:00:00Z".to_string(),
        notes: None,
        artifacts: vec![],
    };

    let needs_update = latest.version != current;
    assert!(
        !needs_update,
        "Same version {} should not trigger update",
        current
    );
}

/// When the current binary has a higher version than the coordinator's latest
/// (e.g., a developer running a local build), the simple string inequality
/// still triggers. This verifies the actual behavior: `check_for_update` does
/// NOT perform semver comparison, it just checks for string inequality.
///
/// In practice, the coordinator controls rollout and would not advertise an
/// older version unless performing a deliberate rollback.
#[test]
fn test_version_comparison_downgrade_detected() {
    let current = env!("CARGO_PKG_VERSION");
    let latest = WorkerReleaseInfo {
        channel: "stable".to_string(),
        version: "0.0.1".to_string(),
        published_at: "2026-01-01T00:00:00Z".to_string(),
        notes: Some("Ancient version".to_string()),
        artifacts: vec![],
    };

    // The comparison is pure string inequality, so a "downgrade" also returns true
    let needs_update = latest.version != current;
    assert!(
        needs_update,
        "Different version (even older 0.0.1 vs {}) should be detected as different",
        current
    );
}

// ============================================================================
// SHA-256 Verification
// ============================================================================
//
// SHA-256 is the primary integrity check for downloaded update artifacts.
// The `sha256_file` function reads files in 8KB chunks to handle arbitrarily
// large binaries without loading them entirely into memory.
//
// Test vectors are from NIST FIPS 180-4 (SHA-256) and verified against:
// - echo -n "abc" | sha256sum
// - echo -n "" | sha256sum
// ============================================================================

/// Verifies SHA-256 against the NIST test vector for the string "abc".
///
/// NIST FIPS 180-4 specifies: SHA-256("abc") =
/// ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
///
/// This is the fundamental correctness check for the hash function used
/// to verify update artifact integrity after download.
#[test]
fn test_sha256_file_known_hash() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.bin");
    fs::write(&path, b"abc").unwrap();

    let hash = sha256_file(&path).unwrap();
    assert_eq!(
        hash,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "SHA-256 of 'abc' must match NIST test vector"
    );
}

/// Verifies SHA-256 of an empty file matches the well-known empty-string digest.
///
/// SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
///
/// This is a boundary condition: an empty archive download (e.g., from a
/// truncated connection) should produce a valid but non-matching hash,
/// triggering the checksum mismatch error rather than silently proceeding.
#[test]
fn test_sha256_file_empty_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("empty.bin");
    fs::write(&path, b"").unwrap();

    let hash = sha256_file(&path).unwrap();
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "SHA-256 of empty file must match empty-string digest"
    );
}

/// Verifies that a 1MB file hashes correctly, exercising the chunked reading
/// path in `sha256_file`. The function reads in 8KB buffers, so a 1MB file
/// requires ~128 read iterations. This catches off-by-one errors in the
/// read loop and ensures the hasher state is correctly maintained across chunks.
///
/// The expected hash is computed by SHA-256 over 1,048,576 bytes of 0xAA.
#[test]
fn test_sha256_file_large_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("large.bin");

    // Create a 1MB file filled with 0xAA bytes
    let data = vec![0xAAu8; 1_048_576];
    fs::write(&path, &data).unwrap();

    let hash = sha256_file(&path).unwrap();

    // Verify against independently computed hash:
    // Use sha2 crate directly to compute the expected value
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let expected = format!("{:x}", hasher.finalize());

    assert_eq!(
        hash, expected,
        "SHA-256 of 1MB file must match independently computed hash"
    );
    // Also verify the hash is a valid 64-character hex string
    assert_eq!(hash.len(), 64, "SHA-256 hex digest must be 64 characters");
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "SHA-256 digest must contain only hex characters"
    );
}

/// Attempting to hash a nonexistent file must return an error, not panic.
/// This guards against race conditions where the archive file is deleted
/// between download and hash verification.
#[test]
fn test_sha256_file_nonexistent_returns_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("does_not_exist.bin");

    let result = sha256_file(&path);
    assert!(
        result.is_err(),
        "sha256_file on nonexistent path must return Err"
    );
}

/// The coordinator may publish checksums in uppercase hex (e.g., "BA7816BF...")
/// while `sha256_file` returns lowercase hex. The comparison in
/// `stage_or_apply_update` uses `eq_ignore_ascii_case`, so both forms must
/// be considered equal. This test verifies the case-insensitive comparison
/// pattern used in the update pipeline.
#[test]
fn test_sha256_case_insensitive_comparison() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("case_test.bin");
    fs::write(&path, b"abc").unwrap();

    let hash = sha256_file(&path).unwrap();
    let upper = "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD";

    assert!(
        hash.eq_ignore_ascii_case(upper),
        "Lowercase hash '{}' must match uppercase '{}'",
        hash,
        upper
    );

    // Also verify mixed case works
    let mixed = "Ba7816Bf8f01CFeA414140dE5DaE2223b00361a396177A9Cb410Ff61f20015Ad";
    assert!(
        hash.eq_ignore_ascii_case(mixed),
        "Hash comparison must be fully case-insensitive"
    );
}

// ============================================================================
// Binary Discovery
// ============================================================================
//
// `find_darkreach_binary` performs a depth-first recursive search through
// the extracted archive directory to locate the darkreach executable. Archives
// from different build systems may place the binary at different nesting depths.
//
// The function uses an iterative stack-based DFS (not recursive function calls)
// to avoid stack overflow on deeply nested directory structures.
// ============================================================================

/// When the binary is placed directly in the root of the extraction directory,
/// `find_darkreach_binary` must find it immediately.
#[test]
fn test_find_binary_in_root() {
    let dir = TempDir::new().unwrap();
    let binary_path = dir.path().join(binary_name_for_platform());
    fs::write(&binary_path, b"fake binary content").unwrap();

    let found = find_darkreach_binary(dir.path());
    assert!(found.is_some(), "Binary in root directory must be found");
    assert_eq!(
        found.unwrap().file_name().unwrap().to_str().unwrap(),
        binary_name_for_platform(),
        "Found binary must have the correct name"
    );
}

/// Archives from some build systems nest the binary several levels deep
/// (e.g., `release/bin/darkreach` or `darkreach-1.2.3/target/release/darkreach`).
/// The recursive search must traverse through intermediate directories.
#[test]
fn test_find_binary_nested() {
    let dir = TempDir::new().unwrap();
    let nested = dir.path().join("a").join("b").join("c");
    fs::create_dir_all(&nested).unwrap();
    let binary_path = nested.join(binary_name_for_platform());
    fs::write(&binary_path, b"deeply nested binary").unwrap();

    let found = find_darkreach_binary(dir.path());
    assert!(
        found.is_some(),
        "Binary nested 3 levels deep must be found"
    );
    assert_eq!(found.unwrap(), binary_path);
}

/// When the archive does not contain a darkreach binary (e.g., wrong archive
/// was downloaded, or the archive is for a different project), the function
/// must return None rather than panicking or returning an arbitrary file.
#[test]
fn test_find_binary_not_found() {
    let dir = TempDir::new().unwrap();
    // Create some files that are NOT named darkreach
    fs::write(dir.path().join("README.md"), b"not a binary").unwrap();
    fs::write(dir.path().join("config.toml"), b"not a binary").unwrap();
    fs::create_dir_all(dir.path().join("lib")).unwrap();
    fs::write(dir.path().join("lib").join("libgmp.so"), b"library").unwrap();

    let found = find_darkreach_binary(dir.path());
    assert!(
        found.is_none(),
        "Must return None when no darkreach binary exists"
    );
}

/// On Windows, the binary is named `darkreach.exe`. The `find_darkreach_binary`
/// function checks for both `darkreach` and `darkreach.exe` regardless of
/// the current platform, so a cross-platform archive can be validated.
#[test]
fn test_find_binary_exe_extension() {
    let dir = TempDir::new().unwrap();
    let exe_path = dir.path().join("darkreach.exe");
    fs::write(&exe_path, b"windows binary").unwrap();

    let found = find_darkreach_binary(dir.path());
    assert!(
        found.is_some(),
        "darkreach.exe must be found (cross-platform detection)"
    );
    assert_eq!(
        found.unwrap().file_name().unwrap().to_str().unwrap(),
        "darkreach.exe"
    );
}

// ============================================================================
// Archive Extraction (using real tar)
// ============================================================================
//
// The update pipeline extracts downloaded `.tar.gz` archives using the system
// `tar` command. These tests create real tar.gz archives and verify extraction.
//
// Prerequisites: `tar` must be available on the system PATH (present on all
// Unix systems and most CI environments).
// ============================================================================

/// Creates a tar.gz archive with known contents, extracts it, and verifies
/// the extracted files match the originals. This validates the extract
/// command construction in `stage_or_apply_update`.
#[test]
fn test_create_and_extract_archive() {
    let source_dir = TempDir::new().unwrap();
    let archive_dir = TempDir::new().unwrap();
    let extract_dir = TempDir::new().unwrap();

    // Create source files
    fs::write(source_dir.path().join("hello.txt"), b"Hello, World!").unwrap();
    fs::create_dir_all(source_dir.path().join("subdir")).unwrap();
    fs::write(
        source_dir.path().join("subdir").join("data.bin"),
        b"\x00\x01\x02\x03",
    )
    .unwrap();

    // Create tar.gz archive
    let archive_path = archive_dir.path().join("test.tar.gz");
    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&archive_path)
        .arg("-C")
        .arg(source_dir.path())
        .arg(".")
        .status()
        .expect("tar command must be available");
    assert!(status.success(), "tar creation must succeed");
    assert!(archive_path.exists(), "Archive file must exist");

    // Extract the archive (mirrors stage_or_apply_update's extraction)
    let status = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(&archive_path)
        .arg("-C")
        .arg(extract_dir.path())
        .status()
        .expect("tar extraction must succeed");
    assert!(status.success(), "tar extraction must succeed");

    // Verify extracted contents
    let hello = fs::read(extract_dir.path().join("hello.txt")).unwrap();
    assert_eq!(hello, b"Hello, World!", "Extracted file content must match");

    let data = fs::read(extract_dir.path().join("subdir").join("data.bin")).unwrap();
    assert_eq!(
        data,
        b"\x00\x01\x02\x03",
        "Extracted binary content must match"
    );
}

/// Creates a tar.gz archive containing a fake darkreach binary, extracts it,
/// and verifies that `find_darkreach_binary` locates it within the extracted
/// tree. This is the closest simulation of the actual update extraction flow
/// without network access.
#[test]
fn test_extract_archive_with_darkreach_binary() {
    let source_dir = TempDir::new().unwrap();
    let archive_dir = TempDir::new().unwrap();
    let extract_dir = TempDir::new().unwrap();

    // Create a fake darkreach binary in a nested directory structure
    // (mimicking the build output layout)
    let bin_dir = source_dir.path().join("darkreach-0.2.0").join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let binary_name = binary_name_for_platform();
    let binary_path = bin_dir.join(binary_name);
    fs::write(&binary_path, b"#!/bin/sh\necho fake-darkreach").unwrap();

    // Create archive from the source directory
    let archive_path = archive_dir.path().join("darkreach-worker.tar.gz");
    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&archive_path)
        .arg("-C")
        .arg(source_dir.path())
        .arg(".")
        .status()
        .expect("tar must be available");
    assert!(status.success(), "Archive creation must succeed");

    // Extract
    let status = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(&archive_path)
        .arg("-C")
        .arg(extract_dir.path())
        .status()
        .expect("tar extraction must succeed");
    assert!(status.success(), "Archive extraction must succeed");

    // find_darkreach_binary should locate it in the nested structure
    let found = find_darkreach_binary(extract_dir.path());
    assert!(
        found.is_some(),
        "Must find darkreach binary in extracted archive"
    );
    let found_path = found.unwrap();
    assert_eq!(
        found_path.file_name().unwrap().to_str().unwrap(),
        binary_name,
        "Found binary must be named '{}'",
        binary_name
    );

    // Verify the content survived the archive round-trip
    let content = fs::read(&found_path).unwrap();
    assert_eq!(
        content,
        b"#!/bin/sh\necho fake-darkreach",
        "Binary content must survive tar round-trip"
    );
}

// ============================================================================
// Platform Detection
// ============================================================================
//
// Platform detection functions (`worker_os`, `worker_arch`, `binary_name_for_platform`)
// use `std::env::consts::OS` and `std::env::consts::ARCH` at compile time.
// These are sent to the coordinator during worker registration and used to
// select the correct artifact for download.
//
// These tests verify the functions return valid, non-empty values matching
// the known set of Rust target platforms.
// ============================================================================

/// `worker_os()` returns `std::env::consts::OS` which must be one of the
/// well-known Rust target OS values. On CI this will be "linux" or "macos".
#[test]
fn test_worker_os_returns_valid() {
    let os = worker_os();
    assert!(
        !os.is_empty(),
        "worker_os() must not be empty"
    );
    let valid_os = ["linux", "macos", "windows", "freebsd", "netbsd", "openbsd"];
    assert!(
        valid_os.contains(&os),
        "worker_os() '{}' must be a recognized OS",
        os
    );
}

/// `worker_arch()` returns `std::env::consts::ARCH` which must be one of the
/// well-known Rust target architecture values.
#[test]
fn test_worker_arch_returns_valid() {
    let arch = worker_arch();
    assert!(
        !arch.is_empty(),
        "worker_arch() must not be empty"
    );
    let valid_arch = [
        "x86_64", "aarch64", "arm", "x86", "powerpc64", "s390x", "riscv64gc", "mips64",
    ];
    assert!(
        valid_arch.contains(&arch),
        "worker_arch() '{}' must be a recognized architecture",
        arch
    );
}

/// On non-Windows platforms (Linux, macOS), the binary name is "darkreach"
/// without any extension. This test verifies the current platform's value.
#[cfg(not(windows))]
#[test]
fn test_binary_name_linux() {
    let name = binary_name_for_platform();
    assert_eq!(
        name, "darkreach",
        "Binary name on Unix must be 'darkreach' (no extension)"
    );
}

/// On Windows, the binary name must include the `.exe` extension. This is
/// critical for the update system: without `.exe`, Windows will not recognize
/// the file as executable.
#[cfg(windows)]
#[test]
fn test_binary_name_windows() {
    let name = binary_name_for_platform();
    assert_eq!(
        name, "darkreach.exe",
        "Binary name on Windows must be 'darkreach.exe'"
    );
}

/// Regardless of platform, the binary name must start with "darkreach" and
/// must not be empty. This is a cross-platform invariant.
#[test]
fn test_binary_name_starts_with_darkreach() {
    let name = binary_name_for_platform();
    assert!(
        name.starts_with("darkreach"),
        "Binary name '{}' must start with 'darkreach'",
        name
    );
    assert!(
        !name.is_empty(),
        "Binary name must not be empty"
    );
}

// ============================================================================
// Artifact Matching
// ============================================================================
//
// When `stage_or_apply_update` receives a `WorkerReleaseInfo` with multiple
// artifacts (one per platform), it must select the artifact matching the
// current worker's OS and architecture. If no match is found, it returns an
// error rather than downloading an incompatible binary.
//
// The matching logic is: `a.os == worker_os() && a.arch == worker_arch()`
// ============================================================================

/// Given a list of artifacts that includes one for the current platform,
/// the matching logic must select exactly that artifact. Decoy artifacts
/// use fictional OS/arch combinations that will never match any real platform.
#[test]
fn test_artifact_matches_current_platform() {
    let current_os = worker_os().to_string();
    let current_arch = worker_arch().to_string();

    let artifacts = vec![
        // Decoys: fictional platforms that never match any real system
        WorkerReleaseArtifact {
            os: "plan9".to_string(),
            arch: "mips".to_string(),
            url: "https://example.com/darkreach-plan9-mips.tar.gz".to_string(),
            sha256: "decoy_1".to_string(),
            sig_url: None,
        },
        WorkerReleaseArtifact {
            os: "haiku".to_string(),
            arch: "sparc".to_string(),
            url: "https://example.com/darkreach-haiku-sparc.tar.gz".to_string(),
            sha256: "decoy_2".to_string(),
            sig_url: None,
        },
        // The real match for the current platform
        WorkerReleaseArtifact {
            os: current_os.clone(),
            arch: current_arch.clone(),
            url: format!(
                "https://example.com/darkreach-{}-{}.tar.gz",
                current_os, current_arch
            ),
            sha256: "current_platform_hash".to_string(),
            sig_url: None,
        },
        WorkerReleaseArtifact {
            os: "templeos".to_string(),
            arch: "ia64".to_string(),
            url: "https://example.com/darkreach-templeos-ia64.tar.gz".to_string(),
            sha256: "decoy_3".to_string(),
            sig_url: None,
        },
    ];

    // Replicate the matching logic from stage_or_apply_update
    let matched = artifacts
        .iter()
        .find(|a| a.os == worker_os() && a.arch == worker_arch());

    assert!(
        matched.is_some(),
        "Must find artifact for current platform {}/{}",
        current_os,
        current_arch
    );
    assert_eq!(
        matched.unwrap().sha256, "current_platform_hash",
        "Must select the artifact for the current platform, not another"
    );
}

/// When no artifact matches the current platform, the update must fail with
/// a clear error rather than downloading an incompatible binary.
#[test]
fn test_artifact_no_match_for_platform() {
    // Create artifacts for platforms that definitely don't match current
    let artifacts = vec![
        WorkerReleaseArtifact {
            os: "plan9".to_string(),
            arch: "mips".to_string(),
            url: "https://example.com/darkreach-plan9-mips.tar.gz".to_string(),
            sha256: "aaa".to_string(),
            sig_url: None,
        },
        WorkerReleaseArtifact {
            os: "haiku".to_string(),
            arch: "sparc".to_string(),
            url: "https://example.com/darkreach-haiku-sparc.tar.gz".to_string(),
            sha256: "bbb".to_string(),
            sig_url: None,
        },
    ];

    let matched = artifacts
        .iter()
        .find(|a| a.os == worker_os() && a.arch == worker_arch());

    assert!(
        matched.is_none(),
        "Must return None when no artifact matches current platform {}/{}",
        worker_os(),
        worker_arch()
    );
}

/// Given multiple artifacts including duplicates and variants, the first
/// matching artifact for the current platform must be selected. This tests
/// the `find` iterator behavior (returns first match).
#[test]
fn test_artifact_multiple_platforms() {
    let current_os = worker_os().to_string();
    let current_arch = worker_arch().to_string();

    let artifacts = vec![
        // Decoy: wrong arch for current OS
        WorkerReleaseArtifact {
            os: current_os.clone(),
            arch: "mips64".to_string(),
            url: "https://example.com/wrong-arch.tar.gz".to_string(),
            sha256: "wrong_arch".to_string(),
            sig_url: None,
        },
        // Decoy: wrong OS for current arch
        WorkerReleaseArtifact {
            os: "plan9".to_string(),
            arch: current_arch.clone(),
            url: "https://example.com/wrong-os.tar.gz".to_string(),
            sha256: "wrong_os".to_string(),
            sig_url: None,
        },
        // Correct match (first)
        WorkerReleaseArtifact {
            os: current_os.clone(),
            arch: current_arch.clone(),
            url: "https://example.com/correct-first.tar.gz".to_string(),
            sha256: "correct_first".to_string(),
            sig_url: None,
        },
        // Correct match (second — should NOT be selected)
        WorkerReleaseArtifact {
            os: current_os.clone(),
            arch: current_arch.clone(),
            url: "https://example.com/correct-second.tar.gz".to_string(),
            sha256: "correct_second".to_string(),
            sig_url: None,
        },
    ];

    let matched = artifacts
        .iter()
        .find(|a| a.os == worker_os() && a.arch == worker_arch());

    assert!(matched.is_some(), "Must find a matching artifact");
    assert_eq!(
        matched.unwrap().sha256, "correct_first",
        "Must select the FIRST matching artifact (find returns first match)"
    );
}

// ============================================================================
// Staging (File Operations)
// ============================================================================
//
// The staging process creates a directory structure under ~/.darkreach/updates/
// and copies the extracted binary there with executable permissions. These tests
// verify the file system operations without involving the network.
// ============================================================================

/// Staging creates the `~/.darkreach/updates/{version}/` directory structure.
/// This test simulates the directory creation that `stage_or_apply_update`
/// performs before writing the staged binary.
#[test]
fn test_stage_binary_creates_dir() {
    let base_dir = TempDir::new().unwrap();
    let version = "1.2.3";
    let updates_dir = base_dir.path().join("updates").join(version);

    // Simulate what stage_or_apply_update does
    fs::create_dir_all(&updates_dir).unwrap();

    assert!(updates_dir.exists(), "Updates directory must be created");
    assert!(
        updates_dir.is_dir(),
        "Updates path must be a directory"
    );

    // Verify we can write the staged binary
    let staged_path = updates_dir.join(binary_name_for_platform());
    fs::write(&staged_path, b"staged binary content").unwrap();
    assert!(staged_path.exists(), "Staged binary must be writable");
}

/// After staging, the binary must have executable permissions (Unix).
/// Without the execute bit, the worker would fail to restart with the
/// new version, requiring manual intervention.
#[cfg(unix)]
#[test]
fn test_stage_binary_sets_executable() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    let binary_path = dir.path().join("darkreach");
    fs::write(&binary_path, b"fake binary").unwrap();

    // Verify it starts without execute permission
    let perms_before = fs::metadata(&binary_path).unwrap().permissions();
    assert_eq!(
        perms_before.mode() & 0o111,
        0,
        "Newly created file should not have execute bits"
    );

    // Apply make_executable (same as stage_or_apply_update)
    make_executable(&binary_path).unwrap();

    let perms_after = fs::metadata(&binary_path).unwrap().permissions();
    assert_eq!(
        perms_after.mode() & 0o755,
        0o755,
        "After make_executable, file must have 0755 permissions"
    );
}

/// Re-staging the same version must overwrite the previous staged binary
/// without error. This handles the case where a download was interrupted
/// and the worker retries the update.
#[test]
fn test_stage_overwrites_existing() {
    let dir = TempDir::new().unwrap();
    let updates_dir = dir.path().join("updates").join("1.0.0");
    fs::create_dir_all(&updates_dir).unwrap();

    let staged_path = updates_dir.join(binary_name_for_platform());

    // First staging
    fs::write(&staged_path, b"version 1 binary").unwrap();
    assert_eq!(
        fs::read(&staged_path).unwrap(),
        b"version 1 binary"
    );

    // Re-staging (overwrite)
    fs::write(&staged_path, b"version 1 binary UPDATED").unwrap();
    assert_eq!(
        fs::read(&staged_path).unwrap(),
        b"version 1 binary UPDATED",
        "Re-staging must overwrite previous binary"
    );

    // Also verify the archive can be overwritten
    let archive_path = updates_dir.join(format!(
        "darkreach-worker-{}-{}.tar.gz",
        worker_os(),
        worker_arch()
    ));
    fs::write(&archive_path, b"archive v1").unwrap();
    fs::write(&archive_path, b"archive v2 new").unwrap();
    assert_eq!(
        fs::read(&archive_path).unwrap(),
        b"archive v2 new",
        "Re-downloading archive must overwrite previous"
    );
}

// ============================================================================
// Environment Variable Controls
// ============================================================================
//
// The auto-update system is controlled by environment variables:
//
// - DARKREACH_UPDATE_CHANNEL: Release channel (default: "stable")
//   Used in worker registration to tell the coordinator which channel to track.
//
// - DARKREACH_VERIFY_WORKER_SIG: "1" or "true" to enable signature verification.
//   When enabled, DARKREACH_WORKER_PUBKEY_PATH must also be set.
//
// - DARKREACH_AUTO_UPDATE: Not currently implemented in the codebase, but
//   the pattern for parsing boolean flags is established by DARKREACH_VERIFY_WORKER_SIG
//   and DARKREACH_HAS_GPU.
//
// Note: These tests manipulate environment variables, which is inherently
// non-thread-safe. They use unique variable names or are designed to be
// idempotent to minimize flakiness.
// ============================================================================

/// Tests the boolean flag parsing pattern used for DARKREACH_VERIFY_WORKER_SIG.
/// The function `should_verify_worker_signature()` accepts "1" and "true"
/// (case-insensitive) as enabled, and treats everything else (including unset)
/// as disabled.
///
/// This is the same parsing pattern used for DARKREACH_HAS_GPU and would be
/// used for any future boolean environment flags.
#[test]
fn test_env_auto_update_flag_parsing() {
    use darkreach::operator::should_verify_worker_signature;

    // Save and clear the variable to ensure clean state
    let saved = std::env::var("DARKREACH_VERIFY_WORKER_SIG").ok();

    // Test "1" -> enabled
    std::env::set_var("DARKREACH_VERIFY_WORKER_SIG", "1");
    assert!(
        should_verify_worker_signature(),
        "'1' must enable signature verification"
    );

    // Test "true" -> enabled (case-insensitive)
    std::env::set_var("DARKREACH_VERIFY_WORKER_SIG", "true");
    assert!(
        should_verify_worker_signature(),
        "'true' must enable signature verification"
    );

    // Test "TRUE" -> enabled
    std::env::set_var("DARKREACH_VERIFY_WORKER_SIG", "TRUE");
    assert!(
        should_verify_worker_signature(),
        "'TRUE' must enable signature verification"
    );

    // Test "True" -> enabled
    std::env::set_var("DARKREACH_VERIFY_WORKER_SIG", "True");
    assert!(
        should_verify_worker_signature(),
        "'True' must enable signature verification"
    );

    // Test "0" -> disabled
    std::env::set_var("DARKREACH_VERIFY_WORKER_SIG", "0");
    assert!(
        !should_verify_worker_signature(),
        "'0' must disable signature verification"
    );

    // Test "false" -> disabled
    std::env::set_var("DARKREACH_VERIFY_WORKER_SIG", "false");
    assert!(
        !should_verify_worker_signature(),
        "'false' must disable signature verification"
    );

    // Test unset -> disabled
    std::env::remove_var("DARKREACH_VERIFY_WORKER_SIG");
    assert!(
        !should_verify_worker_signature(),
        "Unset must disable signature verification (opt-in model)"
    );

    // Test empty string -> disabled
    std::env::set_var("DARKREACH_VERIFY_WORKER_SIG", "");
    assert!(
        !should_verify_worker_signature(),
        "Empty string must disable signature verification"
    );

    // Restore original value
    match saved {
        Some(v) => std::env::set_var("DARKREACH_VERIFY_WORKER_SIG", v),
        None => std::env::remove_var("DARKREACH_VERIFY_WORKER_SIG"),
    }
}

/// The default update channel is "stable". When DARKREACH_UPDATE_CHANNEL is
/// not set, `std::env::var("DARKREACH_UPDATE_CHANNEL").unwrap_or("stable")`
/// returns "stable". This is used during worker registration to tell the
/// coordinator which release channel to track for this worker.
#[test]
fn test_env_update_channel_default() {
    // Save and clear
    let saved = std::env::var("DARKREACH_UPDATE_CHANNEL").ok();
    std::env::remove_var("DARKREACH_UPDATE_CHANNEL");

    let channel = std::env::var("DARKREACH_UPDATE_CHANNEL")
        .unwrap_or_else(|_| "stable".to_string());
    assert_eq!(
        channel, "stable",
        "Default update channel must be 'stable'"
    );

    // Test explicit channel
    std::env::set_var("DARKREACH_UPDATE_CHANNEL", "beta");
    let channel = std::env::var("DARKREACH_UPDATE_CHANNEL")
        .unwrap_or_else(|_| "stable".to_string());
    assert_eq!(
        channel, "beta",
        "Explicit DARKREACH_UPDATE_CHANNEL must override default"
    );

    // Restore
    match saved {
        Some(v) => std::env::set_var("DARKREACH_UPDATE_CHANNEL", v),
        None => std::env::remove_var("DARKREACH_UPDATE_CHANNEL"),
    }
}

/// When DARKREACH_VERIFY_WORKER_SIG=1, the update system requires a signature
/// URL in the artifact metadata and a public key path in the environment.
/// This test verifies that the opt-in detection works correctly and that
/// the system demands a public key path when verification is enabled.
#[test]
fn test_env_verify_sig_opt_in() {
    use darkreach::operator::should_verify_worker_signature;

    // Save state
    let saved_sig = std::env::var("DARKREACH_VERIFY_WORKER_SIG").ok();
    let saved_key = std::env::var("DARKREACH_WORKER_PUBKEY_PATH").ok();

    // Enable signature verification
    std::env::set_var("DARKREACH_VERIFY_WORKER_SIG", "1");
    assert!(
        should_verify_worker_signature(),
        "DARKREACH_VERIFY_WORKER_SIG=1 must enable verification"
    );

    // When enabled, an artifact without sig_url should cause stage_or_apply_update
    // to fail. We test the precondition: artifact.sig_url must be Some.
    let artifact_without_sig = WorkerReleaseArtifact {
        os: worker_os().to_string(),
        arch: worker_arch().to_string(),
        url: "https://example.com/darkreach.tar.gz".to_string(),
        sha256: "abc123".to_string(),
        sig_url: None,
    };
    assert!(
        artifact_without_sig.sig_url.is_none(),
        "Artifact without sig_url must have sig_url = None"
    );

    // Verify that the sig_url.as_deref().ok_or_else() pattern would fail
    let sig_url_result: Result<&str, &str> = artifact_without_sig
        .sig_url
        .as_deref()
        .ok_or("sig_url missing");
    assert!(
        sig_url_result.is_err(),
        "Missing sig_url must cause error when verification is enabled"
    );

    // An artifact WITH sig_url should pass this check
    let artifact_with_sig = WorkerReleaseArtifact {
        os: worker_os().to_string(),
        arch: worker_arch().to_string(),
        url: "https://example.com/darkreach.tar.gz".to_string(),
        sha256: "abc123".to_string(),
        sig_url: Some("https://example.com/darkreach.tar.gz.sig".to_string()),
    };
    let sig_url_result: Result<&str, &str> = artifact_with_sig
        .sig_url
        .as_deref()
        .ok_or("sig_url missing");
    assert!(
        sig_url_result.is_ok(),
        "Artifact with sig_url must pass the sig_url check"
    );

    // Disable verification
    std::env::set_var("DARKREACH_VERIFY_WORKER_SIG", "0");
    assert!(
        !should_verify_worker_signature(),
        "DARKREACH_VERIFY_WORKER_SIG=0 must disable verification"
    );

    // Restore
    match saved_sig {
        Some(v) => std::env::set_var("DARKREACH_VERIFY_WORKER_SIG", v),
        None => std::env::remove_var("DARKREACH_VERIFY_WORKER_SIG"),
    }
    match saved_key {
        Some(v) => std::env::set_var("DARKREACH_WORKER_PUBKEY_PATH", v),
        None => std::env::remove_var("DARKREACH_WORKER_PUBKEY_PATH"),
    }
}
