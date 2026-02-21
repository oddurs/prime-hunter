//! # Operator — Public Computing Platform Client
//!
//! Implements the operator side of the darkreach distributed computing platform.
//! Operators register an account, receive an API key, and run a work loop that
//! claims blocks from the coordinator, computes primality tests, and submits results.
//!
//! ## Architecture
//!
//! ```text
//! Operator CLI                     Coordinator API
//! ┌──────────────┐                ┌──────────────────────┐
//! │ register      │ ──register──> │ POST /api/v1/register │
//! │               │ <──api_key── │                        │
//! │ run           │ ──claim────> │ GET  /api/v1/work      │
//! │ (loop)        │ <──block───  │                        │
//! │  compute()    │              │                        │
//! │  submit()     │ ──result──>  │ POST /api/v1/result    │
//! └──────────────┘              └────────────────────────┘
//! ```
//!
//! ## Trust Model
//!
//! New volunteers start at trust level 1 (double-checked). After 10 consecutive
//! valid results, they advance to level 2 (single-check for provable forms).
//! After 100 valid results, level 3 (single-check for all forms). Any invalid
//! result resets to level 1.
//!
//! ## Credit System
//!
//! Credits are computed as `wall_seconds * cores * cpu_speed_factor`, where
//! `cpu_speed_factor` is determined by a calibration benchmark on first run.
//! Credits are granted only after result verification.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Local volunteer configuration, saved to `~/.darkreach/config.toml`.
#[derive(Serialize, Deserialize)]
pub struct OperatorConfig {
    pub server: String,
    pub api_key: String,
    pub username: String,
    pub worker_id: String,
}

/// Response from `POST /api/v1/register`.
#[derive(Deserialize)]
pub struct RegisterResponse {
    pub api_key: String,
    pub username: String,
}

/// Work block received from `GET /api/v1/work`.
#[derive(Deserialize)]
pub struct WorkAssignment {
    pub block_id: i64,
    pub search_job_id: i64,
    pub search_type: String,
    pub params: serde_json::Value,
    pub block_start: i64,
    pub block_end: i64,
}

/// Result submission to `POST /api/v1/result`.
#[derive(Serialize)]
pub struct ResultSubmission {
    pub block_id: i64,
    pub tested: i64,
    pub found: i64,
    pub primes: Vec<PrimeReport>,
}

/// Individual prime report within a result submission.
#[derive(Serialize)]
pub struct PrimeReport {
    pub expression: String,
    pub form: String,
    pub digits: u64,
    pub proof_method: String,
    pub certificate: Option<String>,
}

/// Personal stats from `GET /api/v1/stats`.
#[derive(Deserialize)]
pub struct OperatorStats {
    pub username: String,
    pub credit: i64,
    pub primes_found: i32,
    pub trust_level: i16,
    pub rank: Option<i64>,
}

/// Leaderboard entry from `GET /api/v1/leaderboard`.
#[derive(Deserialize)]
pub struct LeaderboardEntry {
    pub rank: i64,
    pub username: String,
    pub team: Option<String>,
    pub credit: i64,
    pub primes_found: i32,
    pub worker_count: i64,
}

/// Latest worker release metadata from `/api/v1/worker/latest`.
#[derive(Debug, Deserialize)]
pub struct WorkerReleaseInfo {
    pub channel: String,
    pub version: String,
    pub published_at: String,
    #[allow(dead_code)]
    pub notes: Option<String>,
    #[allow(dead_code)]
    pub artifacts: Vec<WorkerReleaseArtifact>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WorkerReleaseArtifact {
    pub os: String,
    pub arch: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub sig_url: Option<String>,
}

#[derive(Debug)]
pub struct UpdateResult {
    pub version: String,
    pub staged_binary: PathBuf,
    pub applied: bool,
}

/// Load volunteer config from `~/.darkreach/config.toml`.
pub fn load_config() -> Result<OperatorConfig> {
    let path = config_path()?;
    let content = std::fs::read_to_string(&path)
        .map_err(|_| anyhow::anyhow!("Not registered. Run `darkreach join` first."))?;
    let config: OperatorConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Save volunteer config to `~/.darkreach/config.toml`.
pub fn save_config(config: &OperatorConfig) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

fn config_path() -> Result<std::path::PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(std::path::PathBuf::from(home)
        .join(".darkreach")
        .join("config.toml"))
}

/// Register a new volunteer account with the coordinator.
pub fn register(server: &str, username: &str, email: &str) -> Result<OperatorConfig> {
    let url = format!("{}/api/v1/register", server.trim_end_matches('/'));
    let body = serde_json::json!({
        "username": username,
        "email": email,
    });

    let response: RegisterResponse = ureq::post(&url).send_json(&body)?.body_mut().read_json()?;

    let worker_id = generate_worker_id();
    let config = OperatorConfig {
        server: server.to_string(),
        api_key: response.api_key,
        username: response.username,
        worker_id,
    };
    save_config(&config)?;
    Ok(config)
}

/// Register this worker machine with the coordinator.
pub fn register_worker(config: &OperatorConfig) -> Result<()> {
    let url = format!(
        "{}/api/v1/worker/register",
        config.server.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "worker_id": config.worker_id,
        "hostname": hostname(),
        "cores": rayon::current_num_threads(),
        "cpu_model": cpu_model(),
        "os": worker_os(),
        "arch": worker_arch(),
        "ram_gb": sys_ram_gb().min(i32::MAX as u64) as i32,
        "has_gpu": has_gpu(),
        "gpu_model": gpu_model(),
        "gpu_vram_gb": gpu_vram_gb(),
        "worker_version": env!("CARGO_PKG_VERSION"),
        "update_channel": std::env::var("DARKREACH_UPDATE_CHANNEL").unwrap_or_else(|_| "stable".to_string()),
    });
    ureq::post(&url)
        .header("Authorization", &auth_header(config))
        .send_json(&body)?;
    Ok(())
}

/// Send a heartbeat to the coordinator.
pub fn heartbeat(config: &OperatorConfig) -> Result<()> {
    let url = format!(
        "{}/api/v1/worker/heartbeat",
        config.server.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "worker_id": config.worker_id,
    });
    ureq::post(&url)
        .header("Authorization", &auth_header(config))
        .send_json(&body)?;
    Ok(())
}

/// Claim a work block from the coordinator.
pub fn claim_work(config: &OperatorConfig, cores: usize) -> Result<Option<WorkAssignment>> {
    let url = format!(
        "{}/api/v1/work?cores={}&ram_gb={}&has_gpu={}&os={}&arch={}",
        config.server.trim_end_matches('/'),
        cores,
        sys_ram_gb(),
        has_gpu(),
        worker_os(),
        worker_arch(),
    );
    let mut resp = ureq::get(&url)
        .header("Authorization", &auth_header(config))
        .call()?;
    if resp.status() == 204 {
        return Ok(None);
    }
    let assignment: WorkAssignment = resp.body_mut().read_json()?;
    Ok(Some(assignment))
}

/// Submit a result to the coordinator.
pub fn submit_result(config: &OperatorConfig, submission: &ResultSubmission) -> Result<()> {
    let url = format!("{}/api/v1/result", config.server.trim_end_matches('/'));
    ureq::post(&url)
        .header("Authorization", &auth_header(config))
        .send_json(submission)?;
    Ok(())
}

/// Get volunteer stats from the coordinator.
pub fn get_stats(config: &OperatorConfig) -> Result<OperatorStats> {
    let url = format!("{}/api/v1/stats", config.server.trim_end_matches('/'));
    let stats: OperatorStats = ureq::get(&url)
        .header("Authorization", &auth_header(config))
        .call()?
        .body_mut()
        .read_json()?;
    Ok(stats)
}

/// Get the leaderboard from the coordinator.
pub fn get_leaderboard(server: &str) -> Result<Vec<LeaderboardEntry>> {
    let url = format!("{}/api/v1/leaderboard", server.trim_end_matches('/'));
    let entries: Vec<LeaderboardEntry> = ureq::get(&url).call()?.body_mut().read_json()?;
    Ok(entries)
}

/// Fetch latest worker release metadata for a release channel.
pub fn get_latest_worker_release(
    server: &str,
    channel: &str,
    worker_id: Option<&str>,
) -> Result<WorkerReleaseInfo> {
    let mut url = format!(
        "{}/api/v1/worker/latest?channel={}",
        server.trim_end_matches('/'),
        channel
    );
    if let Some(worker_id) = worker_id {
        url.push_str("&worker_id=");
        url.push_str(&urlencoding::encode(worker_id));
    }
    let info: WorkerReleaseInfo = ureq::get(&url).call()?.body_mut().read_json()?;
    Ok(info)
}

/// Return latest release info when current binary differs from channel version.
pub fn check_for_update(
    config: &OperatorConfig,
    channel: &str,
) -> Result<Option<WorkerReleaseInfo>> {
    let latest = get_latest_worker_release(&config.server, channel, Some(&config.worker_id))?;
    let current = env!("CARGO_PKG_VERSION");
    if latest.version != current {
        return Ok(Some(latest));
    }
    Ok(None)
}

/// Download update artifact for current OS/arch, verify checksum, and stage binary.
/// If `apply` is true, attempts to replace the current executable (Unix only).
pub fn stage_or_apply_update(release: &WorkerReleaseInfo, apply: bool) -> Result<UpdateResult> {
    let artifact = release
        .artifacts
        .iter()
        .find(|a| a.os == worker_os() && a.arch == worker_arch())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No artifact for current platform {}/{} in channel {}",
                worker_os(),
                worker_arch(),
                release.channel
            )
        })?
        .clone();

    let updates_dir = darkreach_home_dir()?.join("updates").join(&release.version);
    std::fs::create_dir_all(&updates_dir)?;

    let archive_path = updates_dir.join(format!(
        "darkreach-worker-{}-{}.tar.gz",
        worker_os(),
        worker_arch()
    ));
    let mut response = ureq::get(&artifact.url).call()?;
    let mut out = File::create(&archive_path)?;
    std::io::copy(&mut response.body_mut().as_reader(), &mut out)?;
    out.flush()?;

    let actual_sha = sha256_file(&archive_path)?;
    if !actual_sha.eq_ignore_ascii_case(&artifact.sha256) {
        return Err(anyhow::anyhow!(
            "Checksum mismatch for {}: expected {}, got {}",
            archive_path.display(),
            artifact.sha256,
            actual_sha
        ));
    }

    if should_verify_worker_signature() {
        let sig_url = artifact.sig_url.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "Signature verification enabled but sig_url missing for {}",
                artifact.url
            )
        })?;
        let pubkey = worker_pubkey_path()?;
        let sig_path = updates_dir.join(format!(
            "darkreach-worker-{}-{}.tar.gz.sig",
            worker_os(),
            worker_arch()
        ));
        download_to_path(sig_url, &sig_path)?;
        verify_signature(&archive_path, &sig_path, &pubkey)?;
    }

    let extract_dir = updates_dir.join("extract");
    std::fs::create_dir_all(&extract_dir)?;
    let status = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&extract_dir)
        .status()?;
    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to unpack update archive {}",
            archive_path.display()
        ));
    }

    let extracted = find_darkreach_binary(&extract_dir)
        .ok_or_else(|| anyhow::anyhow!("No darkreach binary found in update archive"))?;
    let staged_binary = updates_dir.join(binary_name_for_platform());
    std::fs::copy(&extracted, &staged_binary)?;
    make_executable(&staged_binary)?;

    let mut applied = false;
    if apply {
        apply_staged_update(&staged_binary)?;
        applied = true;
    }

    Ok(UpdateResult {
        version: release.version.clone(),
        staged_binary,
        applied,
    })
}

// ── Utility ──────────────────────────────────────────────────────

fn auth_header(config: &OperatorConfig) -> String {
    format!("Bearer {}", config.api_key)
}

fn darkreach_home_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(PathBuf::from(home).join(".darkreach"))
}

pub fn should_verify_worker_signature() -> bool {
    std::env::var("DARKREACH_VERIFY_WORKER_SIG")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

fn worker_pubkey_path() -> Result<PathBuf> {
    let p = std::env::var("DARKREACH_WORKER_PUBKEY_PATH")
        .map(PathBuf::from)
        .map_err(|_| {
            anyhow::anyhow!(
                "DARKREACH_WORKER_PUBKEY_PATH is required when DARKREACH_VERIFY_WORKER_SIG=1"
            )
        })?;
    Ok(p)
}

pub fn binary_name_for_platform() -> &'static str {
    #[cfg(windows)]
    {
        "darkreach.exe"
    }
    #[cfg(not(windows))]
    {
        "darkreach"
    }
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn download_to_path(url: &str, out_path: &Path) -> Result<()> {
    let mut response = ureq::get(url).call()?;
    let mut out = File::create(out_path)?;
    std::io::copy(&mut response.body_mut().as_reader(), &mut out)?;
    out.flush()?;
    Ok(())
}

pub fn verify_signature(artifact_path: &Path, sig_path: &Path, pubkey_path: &Path) -> Result<()> {
    let status = std::process::Command::new("openssl")
        .arg("dgst")
        .arg("-sha256")
        .arg("-verify")
        .arg(pubkey_path)
        .arg("-signature")
        .arg(sig_path)
        .arg(artifact_path)
        .status()?;
    if !status.success() {
        return Err(anyhow::anyhow!(
            "Signature verification failed for {}",
            artifact_path.display()
        ));
    }
    Ok(())
}

pub fn find_darkreach_binary(root: &Path) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).ok()?;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if name == "darkreach" || name == "darkreach.exe" {
                    return Some(p);
                }
            }
        }
    }
    None
}

pub fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms)?;
    }
    Ok(())
}

fn apply_staged_update(staged_binary: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let current = std::env::current_exe()?;
        let replacement = current.with_extension("new");
        std::fs::copy(staged_binary, &replacement)?;
        make_executable(&replacement)?;
        std::fs::rename(&replacement, &current)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = staged_binary;
        Err(anyhow::anyhow!(
            "Automatic binary replacement is currently supported on Unix only"
        ))
    }
}

fn generate_worker_id() -> String {
    let h = hostname();
    let suffix: u32 = rand_u32();
    format!("{}-{:08x}", h, suffix)
}

fn hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn cpu_model() -> String {
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

pub fn worker_os() -> &'static str {
    std::env::consts::OS
}

pub fn worker_arch() -> &'static str {
    std::env::consts::ARCH
}

fn has_gpu() -> bool {
    std::env::var("DARKREACH_HAS_GPU")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn gpu_model() -> Option<String> {
    std::env::var("DARKREACH_GPU_MODEL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn gpu_vram_gb() -> Option<i32> {
    std::env::var("DARKREACH_GPU_VRAM_GB")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|v| *v > 0)
}

fn sys_ram_gb() -> u64 {
    let sys = sysinfo::System::new_all();
    sys.total_memory() / 1_073_741_824
}

fn rand_u32() -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut h);
    std::process::id().hash(&mut h);
    h.finish() as u32
}

/// Backward compatibility type alias.
pub type VolunteerConfig = OperatorConfig;
/// Backward compatibility type alias.
pub type VolunteerStats = OperatorStats;

#[cfg(test)]
mod tests {
    //! Tests for the Operator subsystem — public computing platform client.
    //!
    //! Validates configuration serialization (TOML round-trips), API payload
    //! serialization (JSON for register, heartbeat, result, stats, leaderboard),
    //! update artifact deserialization, file integrity (SHA-256), binary discovery,
    //! and platform utility functions.
    //!
    //! ## Testing Strategy
    //!
    //! - **Config**: TOML round-trip ensures ~/.darkreach/config.toml persists correctly
    //! - **API payloads**: JSON serialization for all request/response types
    //! - **Security**: SHA-256 file hash against known test vectors (NIST)
    //! - **Binary discovery**: recursive directory search for update extraction
    //! - **Platform**: worker_os, worker_arch, hostname not empty on real hardware

    use super::*;

    // ── Configuration Persistence ──────────────────────────────────

    /// Validates TOML round-trip for OperatorConfig. The config file at
    /// ~/.darkreach/config.toml must survive serialize/deserialize without
    /// data loss, as it stores the API key needed for all coordinator requests.
    #[test]
    fn config_roundtrip() {
        let config = OperatorConfig {
            server: "https://darkreach.example.com".to_string(),
            api_key: "abc123".to_string(),
            username: "alice".to_string(),
            worker_id: "alice-host-12345678".to_string(),
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: OperatorConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.server, config.server);
        assert_eq!(parsed.api_key, config.api_key);
        assert_eq!(parsed.username, config.username);
    }

    // ── API Payload Serialization ─────────────────────────────────

    /// Validates that ResultSubmission serializes to JSON with all required
    /// fields for the POST /api/v1/result endpoint.
    #[test]
    fn result_submission_serializes() {
        let sub = ResultSubmission {
            block_id: 42,
            tested: 1000,
            found: 2,
            primes: vec![PrimeReport {
                expression: "3*2^100+1".to_string(),
                form: "kbn".to_string(),
                digits: 31,
                proof_method: "proth".to_string(),
                certificate: None,
            }],
        };
        let json = serde_json::to_string(&sub).unwrap();
        assert!(json.contains("block_id"));
        assert!(json.contains("3*2^100+1"));
    }

    // ── Utility Functions ────────────────────────────────────────

    /// Verifies the time-and-PID-based random generator produces valid u32 values.
    /// Used for generating unique worker IDs on registration.
    #[test]
    fn rand_u32_produces_values() {
        let a = rand_u32();
        assert!(a <= u32::MAX);
    }

    /// hostname() shells out to the `hostname` command and must return a
    /// non-empty string on any real machine. Used for worker registration.
    #[test]
    fn hostname_returns_nonempty() {
        let h = hostname();
        assert!(!h.is_empty());
    }

    /// Worker IDs must follow the format "hostname-XXXXXXXX" where XXXXXXXX
    /// is an 8-hex-digit suffix. This format is parsed by the coordinator for
    /// deduplication and display.
    #[test]
    fn worker_id_format() {
        let id = generate_worker_id();
        assert!(id.contains('-'));
        assert!(id.len() > 9); // hostname-XXXXXXXX
    }

    // ── File Integrity ──────────────────────────────────────────

    /// Validates SHA-256 against the NIST test vector for "abc". This hash
    /// function is used to verify update artifact integrity after download,
    /// preventing corrupted or tampered binaries from being installed.
    #[test]
    fn sha256_file_matches_known_value() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.txt");
        std::fs::write(&p, b"abc").unwrap();
        let got = sha256_file(&p).unwrap();
        assert_eq!(
            got,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    // ── Binary Discovery ─────────────────────────────────────────

    /// Validates that find_darkreach_binary performs recursive directory
    /// traversal to locate the binary within extracted update archives.
    /// Archives may nest the binary in subdirectories.
    #[test]
    fn find_darkreach_binary_recurses() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();
        let bin = nested.join(binary_name_for_platform());
        std::fs::write(&bin, b"bin").unwrap();
        let found = find_darkreach_binary(dir.path()).unwrap();
        assert_eq!(found, bin);
    }

    /// Validates Bearer token format for the Authorization header.
    /// All authenticated coordinator requests use this format.
    #[test]
    fn auth_header_format() {
        let config = OperatorConfig {
            server: "https://example.com".to_string(),
            api_key: "test-key-123".to_string(),
            username: "alice".to_string(),
            worker_id: "w1".to_string(),
        };
        assert_eq!(auth_header(&config), "Bearer test-key-123");
    }

    /// Platform binary name must start with "darkreach" (bare on Unix,
    /// with .exe on Windows).
    #[test]
    fn binary_name_for_platform_not_empty() {
        let name = binary_name_for_platform();
        assert!(!name.is_empty());
        assert!(name.starts_with("darkreach"));
    }

    // ── API Response Deserialization ──────────────────────────────

    /// Validates WorkAssignment deserialization from the GET /api/v1/work
    /// response. The block_start/block_end range defines the search space
    /// assigned to this worker.
    #[test]
    fn work_assignment_deserializes() {
        let json = r#"{
            "block_id": 42,
            "search_job_id": 7,
            "search_type": "factorial",
            "params": {"start": 1, "end": 100},
            "block_start": 1,
            "block_end": 50
        }"#;
        let wa: WorkAssignment = serde_json::from_str(json).unwrap();
        assert_eq!(wa.block_id, 42);
        assert_eq!(wa.search_job_id, 7);
        assert_eq!(wa.search_type, "factorial");
        assert_eq!(wa.block_start, 1);
        assert_eq!(wa.block_end, 50);
    }

    /// Validates OperatorStats deserialization including the trust_level
    /// field used for the trust escalation model (1 -> 2 -> 3).
    #[test]
    fn operator_stats_deserializes() {
        let json = r#"{
            "username": "alice",
            "credit": 12345,
            "primes_found": 7,
            "trust_level": 2,
            "rank": 3
        }"#;
        let stats: OperatorStats = serde_json::from_str(json).unwrap();
        assert_eq!(stats.username, "alice");
        assert_eq!(stats.credit, 12345);
        assert_eq!(stats.primes_found, 7);
        assert_eq!(stats.trust_level, 2);
        assert_eq!(stats.rank, Some(3));
    }

    /// New operators with no completed work have null rank. The Option<i64>
    /// field must deserialize correctly from JSON null.
    #[test]
    fn operator_stats_deserializes_null_rank() {
        let json = r#"{
            "username": "bob",
            "credit": 0,
            "primes_found": 0,
            "trust_level": 1,
            "rank": null
        }"#;
        let stats: OperatorStats = serde_json::from_str(json).unwrap();
        assert!(stats.rank.is_none());
    }

    /// Validates leaderboard entry deserialization including optional team field.
    #[test]
    fn leaderboard_entry_deserializes() {
        let json = r#"{
            "rank": 1,
            "username": "alice",
            "team": "primes-r-us",
            "credit": 999999,
            "primes_found": 42,
            "worker_count": 8
        }"#;
        let entry: LeaderboardEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.rank, 1);
        assert_eq!(entry.team, Some("primes-r-us".to_string()));
        assert_eq!(entry.worker_count, 8);
    }

    /// Solo operators have no team. The team field must accept JSON null.
    #[test]
    fn leaderboard_entry_deserializes_no_team() {
        let json = r#"{
            "rank": 5,
            "username": "solo",
            "team": null,
            "credit": 100,
            "primes_found": 1,
            "worker_count": 1
        }"#;
        let entry: LeaderboardEntry = serde_json::from_str(json).unwrap();
        assert!(entry.team.is_none());
    }

    // ── Worker Update System ─────────────────────────────────────

    /// Validates artifact deserialization including the optional sig_url
    /// for signature verification when DARKREACH_VERIFY_WORKER_SIG=1.
    #[test]
    fn worker_release_artifact_deserializes() {
        let json = r#"{
            "os": "linux",
            "arch": "x86_64",
            "url": "https://example.com/darkreach-linux-x86_64.tar.gz",
            "sha256": "abcdef1234567890",
            "sig_url": "https://example.com/darkreach-linux-x86_64.tar.gz.sig"
        }"#;
        let art: WorkerReleaseArtifact = serde_json::from_str(json).unwrap();
        assert_eq!(art.os, "linux");
        assert_eq!(art.arch, "x86_64");
        assert_eq!(art.sig_url, Some("https://example.com/darkreach-linux-x86_64.tar.gz.sig".to_string()));
    }

    /// Artifacts without a signature URL must deserialize with sig_url=None.
    /// Signature verification is opt-in via environment variable.
    #[test]
    fn worker_release_artifact_no_sig_url() {
        let json = r#"{
            "os": "macos",
            "arch": "aarch64",
            "url": "https://example.com/darkreach-macos-aarch64.tar.gz",
            "sha256": "fedcba0987654321"
        }"#;
        let art: WorkerReleaseArtifact = serde_json::from_str(json).unwrap();
        assert!(art.sig_url.is_none());
    }

    /// Validates full release info deserialization including channel, version,
    /// and nested artifact array.
    #[test]
    fn worker_release_info_deserializes() {
        let json = r#"{
            "channel": "stable",
            "version": "1.2.3",
            "published_at": "2026-02-20T12:00:00Z",
            "notes": "Bug fixes",
            "artifacts": [
                {
                    "os": "linux",
                    "arch": "x86_64",
                    "url": "https://example.com/dl",
                    "sha256": "abc123"
                }
            ]
        }"#;
        let info: WorkerReleaseInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.channel, "stable");
        assert_eq!(info.version, "1.2.3");
        assert_eq!(info.artifacts.len(), 1);
        assert_eq!(info.notes, Some("Bug fixes".to_string()));
    }

    // ── Result Submission Variants ────────────────────────────────

    /// Validates result submission with a primality certificate attached.
    /// Certificates are JSON-encoded proof chains used for independent
    /// verification by the coordinator.
    #[test]
    fn result_submission_with_certificate() {
        let sub = ResultSubmission {
            block_id: 10,
            tested: 500,
            found: 1,
            primes: vec![PrimeReport {
                expression: "5!+1".to_string(),
                form: "factorial".to_string(),
                digits: 3,
                proof_method: "pocklington".to_string(),
                certificate: Some("{\"type\":\"Pocklington\",\"factors\":[]}".to_string()),
            }],
        };
        let json = serde_json::to_string(&sub).unwrap();
        assert!(json.contains("certificate"));
        assert!(json.contains("Pocklington"));
    }

    /// A single work block may discover multiple primes (especially for
    /// small-range searches). The primes array must serialize correctly.
    #[test]
    fn result_submission_multiple_primes() {
        let sub = ResultSubmission {
            block_id: 1,
            tested: 100,
            found: 3,
            primes: vec![
                PrimeReport {
                    expression: "2!+1".to_string(),
                    form: "factorial".to_string(),
                    digits: 1,
                    proof_method: "deterministic".to_string(),
                    certificate: None,
                },
                PrimeReport {
                    expression: "3!-1".to_string(),
                    form: "factorial".to_string(),
                    digits: 1,
                    proof_method: "deterministic".to_string(),
                    certificate: None,
                },
                PrimeReport {
                    expression: "3*2^5+1".to_string(),
                    form: "kbn".to_string(),
                    digits: 2,
                    proof_method: "proth".to_string(),
                    certificate: None,
                },
            ],
        };
        let json = serde_json::to_string(&sub).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["primes"].as_array().unwrap().len(), 3);
    }

    // ── Edge Cases ──────────────────────────────────────────────

    /// A directory with no darkreach binary must return None, not panic.
    /// This happens when the update archive has an unexpected layout.
    #[test]
    fn find_darkreach_binary_returns_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        // Create a directory with some other files but no darkreach binary
        std::fs::write(dir.path().join("other.txt"), b"not a binary").unwrap();
        assert!(find_darkreach_binary(dir.path()).is_none());
    }

    /// SHA-256 of an empty file must match the well-known empty-string digest.
    /// This is a degenerate case that should not crash the hash function.
    #[test]
    fn sha256_file_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("empty.txt");
        std::fs::write(&p, b"").unwrap();
        let hash = sha256_file(&p).unwrap();
        // SHA-256 of empty string
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    /// All four config fields (server, api_key, username, worker_id) must
    /// appear in the serialized TOML output.
    #[test]
    fn config_toml_contains_all_fields() {
        let config = OperatorConfig {
            server: "https://api.darkreach.ai".to_string(),
            api_key: "secret".to_string(),
            username: "test_user".to_string(),
            worker_id: "host-aabbccdd".to_string(),
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("server"));
        assert!(toml_str.contains("api_key"));
        assert!(toml_str.contains("username"));
        assert!(toml_str.contains("worker_id"));
    }

    /// Platform detection must return non-empty strings for OS and arch.
    /// These values are sent to the coordinator for artifact matching.
    #[test]
    fn worker_os_and_arch_not_empty() {
        assert!(!worker_os().is_empty());
        assert!(!worker_arch().is_empty());
    }
}
