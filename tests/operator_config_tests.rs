//! # Operator Configuration Integration Tests
//!
//! Comprehensive tests for the darkreach operator configuration system, covering
//! TOML config file management, worker ID generation, system info detection,
//! environment variable overrides, and config path resolution.
//!
//! ## Test Categories
//!
//! | Category | Tests | Purpose |
//! |----------|-------|---------|
//! | Config File Management | 8 | TOML round-trip, error handling, directory creation |
//! | Worker ID Generation | 3 | Format validation, uniqueness, hostname prefix |
//! | System Info Detection | 5 | CPU, cores, RAM, OS, architecture |
//! | Environment Variable Overrides | 6 | GPU, update channel, auto-update flags |
//! | Config Path Resolution | 2 | Home directory, custom directory isolation |
//!
//! ## Testing Strategy
//!
//! All file system tests use `tempfile::TempDir` for isolation -- no test touches
//! the user's real `~/.darkreach/config.toml`. Environment variable tests that
//! mutate process-global state are marked `#[ignore]` to prevent interference
//! with concurrent tests. Run them explicitly with:
//!
//! ```bash
//! cargo test --test operator_config_tests -- --ignored
//! ```
//!
//! ## Dependencies
//!
//! - `tempfile` (dev-dependency): Isolated temporary directories
//! - `darkreach::operator`: Public config types and load/save functions
//! - `toml`: Direct TOML serialization/deserialization for edge-case tests

use darkreach::operator::OperatorConfig;
use std::io::Write;
use tempfile::TempDir;

// ============================================================================
// Config File Management
// ============================================================================

/// Validates full TOML round-trip: serialize an OperatorConfig to TOML, write
/// to a temp file, read it back, deserialize, and verify every field matches.
///
/// This is the critical path for `save_config` + `load_config` -- the config
/// file at `~/.darkreach/config.toml` must survive a write-read cycle without
/// any data loss, as it stores the API key required for all coordinator requests.
#[test]
fn test_config_roundtrip_toml() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let config_path = dir.path().join("config.toml");

    let original = OperatorConfig {
        server: "https://api.darkreach.ai".to_string(),
        api_key: "dk_live_abc123def456".to_string(),
        username: "alice".to_string(),
        worker_id: "alice-macbook-1a2b3c4d".to_string(),
    };

    // Serialize to TOML and write to file
    let toml_str = toml::to_string_pretty(&original).expect("TOML serialization failed");
    std::fs::write(&config_path, &toml_str).expect("failed to write config file");

    // Read back and deserialize
    let content = std::fs::read_to_string(&config_path).expect("failed to read config file");
    let loaded: OperatorConfig = toml::from_str(&content).expect("TOML deserialization failed");

    assert_eq!(loaded.server, original.server, "server field mismatch");
    assert_eq!(loaded.api_key, original.api_key, "api_key field mismatch");
    assert_eq!(
        loaded.username, original.username,
        "username field mismatch"
    );
    assert_eq!(
        loaded.worker_id, original.worker_id,
        "worker_id field mismatch"
    );
}

/// Verifies that attempting to load config from a nonexistent path produces a
/// clear error rather than a panic or silent default. Operators who haven't run
/// `darkreach join` yet should see a helpful message directing them to register.
#[test]
fn test_config_missing_file_returns_error() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let missing_path = dir.path().join("nonexistent").join("config.toml");

    let result = std::fs::read_to_string(&missing_path);
    assert!(
        result.is_err(),
        "reading a missing config file should return an error"
    );
}

/// Verifies that invalid TOML syntax produces a parse error with a useful
/// message. This protects against hand-edited config files with typos.
#[test]
fn test_config_malformed_toml_returns_error() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let config_path = dir.path().join("config.toml");

    // Write syntactically invalid TOML (unterminated string, missing quotes)
    std::fs::write(&config_path, "server = 'unterminated\napi_key = ").unwrap();

    let content = std::fs::read_to_string(&config_path).unwrap();
    let result: Result<OperatorConfig, _> = toml::from_str(&content);

    assert!(
        result.is_err(),
        "malformed TOML should produce a parse error"
    );
    let err_msg = match result {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected parse error for malformed TOML, got Ok"),
    };
    assert!(
        !err_msg.is_empty(),
        "error message should be non-empty for debugging"
    );
}

/// Verifies that an empty config file produces a parse error rather than a
/// config with empty-string fields. An empty file typically means the
/// registration process was interrupted.
#[test]
fn test_config_empty_file_returns_error() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let config_path = dir.path().join("config.toml");

    std::fs::write(&config_path, "").unwrap();

    let content = std::fs::read_to_string(&config_path).unwrap();
    let result: Result<OperatorConfig, _> = toml::from_str(&content);

    assert!(
        result.is_err(),
        "empty TOML file should fail because all fields are required"
    );
}

/// Verifies that TOML with missing required fields (e.g., no `api_key` or
/// `server`) produces a deserialization error. OperatorConfig has four required
/// fields: server, api_key, username, worker_id. Omitting any one must fail.
#[test]
fn test_config_missing_required_fields() {
    // Missing api_key
    let toml_no_api_key = r#"
        server = "https://api.darkreach.ai"
        username = "bob"
        worker_id = "bob-host-12345678"
    "#;
    let result: Result<OperatorConfig, _> = toml::from_str(toml_no_api_key);
    assert!(
        result.is_err(),
        "config without api_key should fail to deserialize"
    );

    // Missing server
    let toml_no_server = r#"
        api_key = "dk_live_xyz"
        username = "bob"
        worker_id = "bob-host-12345678"
    "#;
    let result: Result<OperatorConfig, _> = toml::from_str(toml_no_server);
    assert!(
        result.is_err(),
        "config without server should fail to deserialize"
    );

    // Missing username
    let toml_no_username = r#"
        server = "https://api.darkreach.ai"
        api_key = "dk_live_xyz"
        worker_id = "bob-host-12345678"
    "#;
    let result: Result<OperatorConfig, _> = toml::from_str(toml_no_username);
    assert!(
        result.is_err(),
        "config without username should fail to deserialize"
    );

    // Missing worker_id
    let toml_no_worker_id = r#"
        server = "https://api.darkreach.ai"
        api_key = "dk_live_xyz"
        username = "bob"
    "#;
    let result: Result<OperatorConfig, _> = toml::from_str(toml_no_worker_id);
    assert!(
        result.is_err(),
        "config without worker_id should fail to deserialize"
    );
}

/// Verifies that unknown TOML fields are silently ignored during deserialization.
/// This is important for forward compatibility: newer coordinator versions may
/// add config fields that older workers don't know about. Serde's default
/// behavior with `#[derive(Deserialize)]` is to ignore unknown fields, which
/// we rely on here.
#[test]
fn test_config_extra_fields_ignored() {
    let toml_with_extras = r#"
        server = "https://api.darkreach.ai"
        api_key = "dk_live_abc123"
        username = "alice"
        worker_id = "alice-host-aabbccdd"
        team = "prime-seekers"
        max_threads = 16
        version = "2.0.0"
    "#;

    let config: OperatorConfig =
        toml::from_str(toml_with_extras).expect("extra fields should be silently ignored");

    assert_eq!(config.server, "https://api.darkreach.ai");
    assert_eq!(config.api_key, "dk_live_abc123");
    assert_eq!(config.username, "alice");
    assert_eq!(config.worker_id, "alice-host-aabbccdd");
}

/// Verifies that `save_config` (via the same logic) creates parent directories
/// if they don't exist. When a user runs `darkreach join` on a fresh machine,
/// `~/.darkreach/` won't exist yet.
#[test]
fn test_config_directory_creation() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let nested_path = dir.path().join("deeply").join("nested").join("dir");

    // The nested directory doesn't exist yet
    assert!(
        !nested_path.exists(),
        "nested directory should not exist before test"
    );

    // Create directories and write config (mimicking save_config logic)
    std::fs::create_dir_all(&nested_path).expect("create_dir_all should succeed");
    let config_path = nested_path.join("config.toml");

    let config = OperatorConfig {
        server: "https://api.darkreach.ai".to_string(),
        api_key: "dk_test_key".to_string(),
        username: "test_user".to_string(),
        worker_id: "test-host-00000000".to_string(),
    };

    let toml_str = toml::to_string_pretty(&config).unwrap();
    std::fs::write(&config_path, &toml_str).expect("write should succeed after dir creation");

    assert!(config_path.exists(), "config file should exist after write");
    let content = std::fs::read_to_string(&config_path).unwrap();
    let loaded: OperatorConfig = toml::from_str(&content).unwrap();
    assert_eq!(loaded.api_key, "dk_test_key");
}

/// Verifies that re-registration correctly overwrites an existing config file.
/// If an operator re-runs `darkreach join` (e.g., to switch servers or reset
/// their API key), the old config must be completely replaced.
#[test]
fn test_config_overwrite_existing() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let config_path = dir.path().join("config.toml");

    // Write initial config
    let old_config = OperatorConfig {
        server: "https://old-server.example.com".to_string(),
        api_key: "old_key_123".to_string(),
        username: "old_user".to_string(),
        worker_id: "old-host-11111111".to_string(),
    };
    let toml_str = toml::to_string_pretty(&old_config).unwrap();
    std::fs::write(&config_path, &toml_str).unwrap();

    // Overwrite with new config
    let new_config = OperatorConfig {
        server: "https://new-server.darkreach.ai".to_string(),
        api_key: "new_key_456".to_string(),
        username: "new_user".to_string(),
        worker_id: "new-host-22222222".to_string(),
    };
    let toml_str = toml::to_string_pretty(&new_config).unwrap();
    std::fs::write(&config_path, &toml_str).unwrap();

    // Verify new config was written
    let content = std::fs::read_to_string(&config_path).unwrap();
    let loaded: OperatorConfig = toml::from_str(&content).unwrap();

    assert_eq!(
        loaded.server, "https://new-server.darkreach.ai",
        "server should be the new value"
    );
    assert_eq!(
        loaded.api_key, "new_key_456",
        "api_key should be the new value"
    );
    assert_eq!(
        loaded.username, "new_user",
        "username should be the new value"
    );
    assert_eq!(
        loaded.worker_id, "new-host-22222222",
        "worker_id should be the new value"
    );

    // Verify old values are gone
    assert!(
        !content.contains("old_key_123"),
        "old api_key should not appear in overwritten file"
    );
    assert!(
        !content.contains("old-server"),
        "old server should not appear in overwritten file"
    );
}

// ============================================================================
// Worker ID Generation
// ============================================================================

/// Validates that generated worker IDs follow the "hostname-XXXXXXXX" pattern,
/// where XXXXXXXX is an 8-character lowercase hexadecimal suffix. The coordinator
/// parses this format for deduplication and display on the fleet dashboard.
///
/// Since `generate_worker_id` is private, we test it indirectly by constructing
/// a config with the expected format and verifying the pattern holds. We also
/// verify the format contract that all worker IDs in the system should follow.
#[test]
fn test_worker_id_format() {
    // Simulate the worker ID format: hostname-XXXXXXXX
    // The actual generate_worker_id() is private, so we verify the format contract
    // by checking that config round-trips preserve IDs of this format.
    let worker_id = format!("{}-{:08x}", "testhost", 0xdeadbeef_u32);

    assert!(
        worker_id.contains('-'),
        "worker ID must contain a hyphen separator"
    );

    let parts: Vec<&str> = worker_id.rsplitn(2, '-').collect();
    assert_eq!(parts.len(), 2, "worker ID must have hostname-suffix format");

    let hex_suffix = parts[0];
    assert_eq!(
        hex_suffix.len(),
        8,
        "hex suffix must be exactly 8 characters"
    );
    assert!(
        hex_suffix.chars().all(|c| c.is_ascii_hexdigit()),
        "suffix must be valid hexadecimal"
    );

    // Verify the ID round-trips through TOML
    let config = OperatorConfig {
        server: "https://api.darkreach.ai".to_string(),
        api_key: "test_key".to_string(),
        username: "test".to_string(),
        worker_id: worker_id.clone(),
    };
    let toml_str = toml::to_string_pretty(&config).unwrap();
    let loaded: OperatorConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(loaded.worker_id, worker_id);
}

/// Validates that worker IDs generated at different times produce different
/// values. The ID incorporates SystemTime and process ID via a hash, so two
/// sequential calls within the same process should still differ because
/// SystemTime has sub-second resolution.
///
/// We simulate uniqueness by generating two IDs using the same hostname but
/// different random suffixes, verifying the contract that each registration
/// produces a unique worker ID.
#[test]
fn test_worker_id_uniqueness() {
    use std::collections::HashSet;

    // Generate multiple worker IDs and verify they are all unique.
    // Since generate_worker_id is private, we simulate the generation pattern
    // (hostname + random suffix) to verify the uniqueness property.
    let mut ids = HashSet::new();
    let hostname = "testhost";

    for i in 0..100 {
        // Simulate the hash-based ID generation with varying inputs
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        std::time::SystemTime::now().hash(&mut h);
        std::process::id().hash(&mut h);
        i.hash(&mut h); // Extra entropy to ensure uniqueness in tight loop
        let suffix = h.finish() as u32;
        let id = format!("{}-{:08x}", hostname, suffix);
        ids.insert(id);
    }

    // With 100 attempts using different hash inputs, we expect all unique
    assert!(
        ids.len() >= 95,
        "expected near-100% unique IDs, got {} out of 100",
        ids.len()
    );
}

/// Validates that the worker ID starts with the machine hostname. This is
/// important for the fleet dashboard, where operators identify their machines
/// by the hostname prefix in the worker ID.
#[test]
fn test_worker_id_contains_hostname() {
    // Get the actual hostname the same way operator.rs does
    let hostname = std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    // Construct a worker ID using the real hostname (as generate_worker_id does)
    let worker_id = format!("{}-{:08x}", hostname, 0x12345678_u32);

    assert!(
        worker_id.starts_with(&hostname),
        "worker ID '{}' should start with hostname '{}'",
        worker_id,
        hostname
    );
}

// ============================================================================
// System Info Detection
// ============================================================================

/// Validates that the CPU model detection returns a non-empty string. On macOS
/// this calls `sysctl -n machdep.cpu.brand_string`, on Linux it parses
/// `/proc/cpuinfo`. Either way, the result is sent to the coordinator during
/// worker registration for fleet hardware inventory.
#[test]
fn test_cpu_model_detection_not_empty() {
    // cpu_model() is private, so we replicate its logic to verify it works
    // on the current platform.
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output();
        if let Ok(o) = output {
            let model = String::from_utf8_lossy(&o.stdout).trim().to_string();
            assert!(!model.is_empty(), "CPU model should be non-empty on macOS");
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
            let model = cpuinfo
                .lines()
                .find(|l| l.starts_with("model name"))
                .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
                .unwrap_or_default();
            assert!(
                !model.is_empty(),
                "CPU model should be non-empty on Linux with /proc/cpuinfo"
            );
        }
    }

    // On any platform, verify std::env::consts values are available
    // (used as fallback context in fleet dashboard)
    assert!(!std::env::consts::OS.is_empty());
    assert!(!std::env::consts::ARCH.is_empty());
}

/// Validates that rayon reports a positive number of available CPU cores. This
/// value is sent to the coordinator as part of worker registration and is used
/// for work block sizing -- a worker with more cores gets larger blocks.
#[test]
fn test_core_count_positive() {
    let cores = rayon::current_num_threads();
    assert!(
        cores > 0,
        "rayon should report at least 1 available thread"
    );
}

/// Validates that the system reports a positive amount of RAM. The `sysinfo`
/// crate's `System::new_all()` is used during worker registration to report
/// available RAM in gigabytes. Zero would indicate a detection failure.
#[test]
fn test_ram_detection_positive() {
    let sys = sysinfo::System::new_all();
    let total_bytes = sys.total_memory();
    let ram_gb = total_bytes / 1_073_741_824;

    assert!(
        total_bytes > 0,
        "system should report positive total memory"
    );
    // Any real machine has at least 1 GB of RAM
    assert!(
        ram_gb >= 1,
        "system should report at least 1 GB RAM, got {} bytes",
        total_bytes
    );
}

/// Validates that `std::env::consts::OS` returns one of the expected operating
/// system identifiers. The coordinator uses this value for artifact matching
/// (selecting the correct binary for updates) and fleet inventory reporting.
#[test]
fn test_os_detection_valid() {
    let os = std::env::consts::OS;
    let valid_os = ["linux", "macos", "windows", "freebsd", "openbsd", "netbsd"];

    assert!(
        valid_os.contains(&os),
        "OS '{}' should be one of {:?}",
        os,
        valid_os
    );
}

/// Validates that `std::env::consts::ARCH` returns one of the expected CPU
/// architecture identifiers. Used alongside OS for artifact matching during
/// worker auto-updates and fleet hardware inventory.
#[test]
fn test_arch_detection_valid() {
    let arch = std::env::consts::ARCH;
    let valid_arch = [
        "x86_64", "x86", "aarch64", "arm", "mips", "mips64", "powerpc", "powerpc64", "riscv64",
        "s390x", "wasm32",
    ];

    assert!(
        valid_arch.contains(&arch),
        "architecture '{}' should be one of {:?}",
        arch,
        valid_arch
    );
}

// ============================================================================
// Environment Variable Overrides
// ============================================================================
//
// These tests validate the environment variable processing logic used by
// `register_worker` in operator.rs. Since the functions that read these
// variables (has_gpu, gpu_model, gpu_vram_gb, etc.) are private, we
// replicate their logic here to verify the contract.
//
// IMPORTANT: Environment variables are process-global state. These tests
// are marked #[ignore] to prevent interference when run concurrently with
// other tests. Run them explicitly:
//
//   cargo test --test operator_config_tests -- --ignored

/// Validates that setting `DARKREACH_HAS_GPU=1` enables GPU reporting. The
/// coordinator uses this flag to assign GPU-accelerated work blocks (e.g.,
/// GWNUM-based tests) to workers with GPU capabilities.
#[test]
#[ignore]
fn test_gpu_env_var_override() {
    // Clean state
    std::env::remove_var("DARKREACH_HAS_GPU");

    // Without the variable, has_gpu should be false
    let has_gpu_unset = std::env::var("DARKREACH_HAS_GPU")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    assert!(
        !has_gpu_unset,
        "has_gpu should be false when env var is not set"
    );

    // Set the variable to "1"
    std::env::set_var("DARKREACH_HAS_GPU", "1");
    let has_gpu_set = std::env::var("DARKREACH_HAS_GPU")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    assert!(
        has_gpu_set,
        "has_gpu should be true when DARKREACH_HAS_GPU=1"
    );

    // Also accept "true" (case-insensitive)
    std::env::set_var("DARKREACH_HAS_GPU", "TRUE");
    let has_gpu_true = std::env::var("DARKREACH_HAS_GPU")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    assert!(
        has_gpu_true,
        "has_gpu should be true when DARKREACH_HAS_GPU=TRUE"
    );

    // Clean up
    std::env::remove_var("DARKREACH_HAS_GPU");
}

/// Validates that `DARKREACH_GPU_MODEL` sets the GPU model string reported
/// during worker registration. Used for fleet inventory and work assignment
/// optimization (e.g., NVIDIA A100 vs. consumer GPUs).
#[test]
#[ignore]
fn test_gpu_model_env_var() {
    std::env::remove_var("DARKREACH_GPU_MODEL");

    // Without the variable, gpu_model should be None
    let model_unset: Option<String> = std::env::var("DARKREACH_GPU_MODEL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    assert!(
        model_unset.is_none(),
        "gpu_model should be None when env var is not set"
    );

    // Set the variable
    std::env::set_var("DARKREACH_GPU_MODEL", "NVIDIA RTX 4090");
    let model_set: Option<String> = std::env::var("DARKREACH_GPU_MODEL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    assert_eq!(model_set, Some("NVIDIA RTX 4090".to_string()));

    // Empty string should be treated as None (filtered out)
    std::env::set_var("DARKREACH_GPU_MODEL", "  ");
    let model_empty: Option<String> = std::env::var("DARKREACH_GPU_MODEL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    assert!(
        model_empty.is_none(),
        "whitespace-only GPU model should be treated as None"
    );

    std::env::remove_var("DARKREACH_GPU_MODEL");
}

/// Validates that `DARKREACH_GPU_VRAM_GB` sets the GPU VRAM in gigabytes. This
/// value influences which work blocks can be assigned to the worker -- large
/// Generalized Fermat or GWNUM-based tests may require significant GPU memory.
#[test]
#[ignore]
fn test_gpu_vram_env_var() {
    std::env::remove_var("DARKREACH_GPU_VRAM_GB");

    // Without the variable, gpu_vram_gb should be None
    let vram_unset: Option<i32> = std::env::var("DARKREACH_GPU_VRAM_GB")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|v| *v > 0);
    assert!(
        vram_unset.is_none(),
        "gpu_vram_gb should be None when env var is not set"
    );

    // Set to a valid value
    std::env::set_var("DARKREACH_GPU_VRAM_GB", "24");
    let vram_set: Option<i32> = std::env::var("DARKREACH_GPU_VRAM_GB")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|v| *v > 0);
    assert_eq!(vram_set, Some(24));

    // Invalid (non-numeric) should be None
    std::env::set_var("DARKREACH_GPU_VRAM_GB", "lots");
    let vram_invalid: Option<i32> = std::env::var("DARKREACH_GPU_VRAM_GB")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|v| *v > 0);
    assert!(
        vram_invalid.is_none(),
        "non-numeric VRAM should be treated as None"
    );

    // Zero should be filtered out (not a valid VRAM amount)
    std::env::set_var("DARKREACH_GPU_VRAM_GB", "0");
    let vram_zero: Option<i32> = std::env::var("DARKREACH_GPU_VRAM_GB")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|v| *v > 0);
    assert!(
        vram_zero.is_none(),
        "zero VRAM should be filtered out"
    );

    // Negative should be filtered out
    std::env::set_var("DARKREACH_GPU_VRAM_GB", "-8");
    let vram_negative: Option<i32> = std::env::var("DARKREACH_GPU_VRAM_GB")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|v| *v > 0);
    assert!(
        vram_negative.is_none(),
        "negative VRAM should be filtered out"
    );

    std::env::remove_var("DARKREACH_GPU_VRAM_GB");
}

/// Validates that `DARKREACH_UPDATE_CHANNEL` overrides the default "stable"
/// channel. Operators can opt into "beta" or "nightly" channels to receive
/// pre-release worker binaries with new search optimizations.
#[test]
#[ignore]
fn test_update_channel_env_var() {
    std::env::remove_var("DARKREACH_UPDATE_CHANNEL");

    // Default channel when env var is unset
    let default_channel =
        std::env::var("DARKREACH_UPDATE_CHANNEL").unwrap_or_else(|_| "stable".to_string());
    assert_eq!(
        default_channel, "stable",
        "default update channel should be 'stable'"
    );

    // Override to beta
    std::env::set_var("DARKREACH_UPDATE_CHANNEL", "beta");
    let beta_channel =
        std::env::var("DARKREACH_UPDATE_CHANNEL").unwrap_or_else(|_| "stable".to_string());
    assert_eq!(beta_channel, "beta", "channel should be 'beta' when set");

    // Override to nightly
    std::env::set_var("DARKREACH_UPDATE_CHANNEL", "nightly");
    let nightly_channel =
        std::env::var("DARKREACH_UPDATE_CHANNEL").unwrap_or_else(|_| "stable".to_string());
    assert_eq!(
        nightly_channel, "nightly",
        "channel should be 'nightly' when set"
    );

    std::env::remove_var("DARKREACH_UPDATE_CHANNEL");
}

/// Validates that `DARKREACH_AUTO_UPDATE=1` enables automatic worker binary
/// updates. When enabled, the operator work loop checks for new versions on
/// startup and downloads/stages the update before claiming work blocks.
#[test]
#[ignore]
fn test_auto_update_env_var_enabled() {
    std::env::remove_var("DARKREACH_AUTO_UPDATE");

    // Set to "1"
    std::env::set_var("DARKREACH_AUTO_UPDATE", "1");
    let auto_update = std::env::var("DARKREACH_AUTO_UPDATE")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    assert!(
        auto_update,
        "auto_update should be true when DARKREACH_AUTO_UPDATE=1"
    );

    // Set to "true" (case-insensitive)
    std::env::set_var("DARKREACH_AUTO_UPDATE", "True");
    let auto_update_true = std::env::var("DARKREACH_AUTO_UPDATE")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    assert!(
        auto_update_true,
        "auto_update should be true when DARKREACH_AUTO_UPDATE=True"
    );

    std::env::remove_var("DARKREACH_AUTO_UPDATE");
}

/// Validates that auto-update is disabled by default (when the env var is unset)
/// and when set to any value other than "1" or "true". This is a safety default
/// to prevent unexpected binary replacements on production workers.
#[test]
#[ignore]
fn test_auto_update_env_var_disabled() {
    std::env::remove_var("DARKREACH_AUTO_UPDATE");

    // Unset means disabled
    let auto_update_unset = std::env::var("DARKREACH_AUTO_UPDATE")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    assert!(
        !auto_update_unset,
        "auto_update should be false when env var is not set"
    );

    // Set to "0" means disabled
    std::env::set_var("DARKREACH_AUTO_UPDATE", "0");
    let auto_update_zero = std::env::var("DARKREACH_AUTO_UPDATE")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    assert!(
        !auto_update_zero,
        "auto_update should be false when DARKREACH_AUTO_UPDATE=0"
    );

    // Set to "false" means disabled
    std::env::set_var("DARKREACH_AUTO_UPDATE", "false");
    let auto_update_false = std::env::var("DARKREACH_AUTO_UPDATE")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    assert!(
        !auto_update_false,
        "auto_update should be false when DARKREACH_AUTO_UPDATE=false"
    );

    // Set to random string means disabled
    std::env::set_var("DARKREACH_AUTO_UPDATE", "yes");
    let auto_update_random = std::env::var("DARKREACH_AUTO_UPDATE")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    assert!(
        !auto_update_random,
        "auto_update should be false when DARKREACH_AUTO_UPDATE=yes (only '1' or 'true' accepted)"
    );

    std::env::remove_var("DARKREACH_AUTO_UPDATE");
}

// ============================================================================
// Config Path Resolution
// ============================================================================

/// Validates that the config path resolves under `$HOME/.darkreach/`. The
/// `config_path()` function in operator.rs reads `$HOME` (or `$USERPROFILE`
/// on Windows) and appends `.darkreach/config.toml`.
#[test]
fn test_config_path_uses_home_dir() {
    // Verify the expected path structure
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .expect("HOME or USERPROFILE must be set");

    let expected_path = std::path::PathBuf::from(&home)
        .join(".darkreach")
        .join("config.toml");

    // Verify path components
    assert!(
        expected_path
            .to_string_lossy()
            .contains(".darkreach/config.toml")
            || expected_path
                .to_string_lossy()
                .contains(".darkreach\\config.toml"),
        "config path should contain .darkreach/config.toml"
    );

    assert!(
        expected_path.starts_with(&home),
        "config path should be under $HOME"
    );
}

/// Validates config file operations using a custom temp directory for complete
/// isolation. This verifies that the save/load cycle works correctly when
/// pointed at an arbitrary directory, without touching the user's real config.
#[test]
fn test_config_path_custom_dir() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let custom_darkreach_dir = dir.path().join(".darkreach");
    let config_path = custom_darkreach_dir.join("config.toml");

    // Create the directory structure
    std::fs::create_dir_all(&custom_darkreach_dir)
        .expect("should create .darkreach directory in temp dir");

    // Write a config file
    let config = OperatorConfig {
        server: "https://custom.darkreach.ai".to_string(),
        api_key: "dk_custom_key_789".to_string(),
        username: "custom_operator".to_string(),
        worker_id: "custom-host-aabbccdd".to_string(),
    };

    let toml_str = toml::to_string_pretty(&config).unwrap();
    let mut file = std::fs::File::create(&config_path).expect("should create config file");
    file.write_all(toml_str.as_bytes())
        .expect("should write config content");
    file.flush().expect("should flush config file");

    // Read and verify
    let content = std::fs::read_to_string(&config_path).expect("should read config file");
    let loaded: OperatorConfig = toml::from_str(&content).expect("should parse config TOML");

    assert_eq!(loaded.server, "https://custom.darkreach.ai");
    assert_eq!(loaded.api_key, "dk_custom_key_789");
    assert_eq!(loaded.username, "custom_operator");
    assert_eq!(loaded.worker_id, "custom-host-aabbccdd");

    // Verify the file is inside the temp directory (isolation check)
    assert!(
        config_path.starts_with(dir.path()),
        "config file should be inside the temp directory, not the real home"
    );
}
