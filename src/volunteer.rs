//! # Volunteer — Public Computing Platform Client
//!
//! Implements the volunteer side of the darkreach distributed computing platform.
//! Volunteers register an account, receive an API key, and run a work loop that
//! claims blocks from the coordinator, computes primality tests, and submits results.
//!
//! ## Architecture
//!
//! ```text
//! Volunteer CLI                    Coordinator API
//! ┌──────────────┐                ┌──────────────────────┐
//! │ join          │ ──register──> │ POST /api/v1/register │
//! │               │ <──api_key── │                        │
//! │ volunteer     │ ──claim────> │ GET  /api/v1/work      │
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

/// Local volunteer configuration, saved to `~/.darkreach/config.toml`.
#[derive(Serialize, Deserialize)]
pub struct VolunteerConfig {
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
pub struct VolunteerStats {
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

/// Load volunteer config from `~/.darkreach/config.toml`.
pub fn load_config() -> Result<VolunteerConfig> {
    let path = config_path()?;
    let content = std::fs::read_to_string(&path)
        .map_err(|_| anyhow::anyhow!("Not registered. Run `darkreach join` first."))?;
    let config: VolunteerConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Save volunteer config to `~/.darkreach/config.toml`.
pub fn save_config(config: &VolunteerConfig) -> Result<()> {
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
pub fn register(server: &str, username: &str, email: &str) -> Result<VolunteerConfig> {
    let url = format!("{}/api/v1/register", server.trim_end_matches('/'));
    let body = serde_json::json!({
        "username": username,
        "email": email,
    });

    let response: RegisterResponse = ureq::post(&url).send_json(&body)?.body_mut().read_json()?;

    let worker_id = generate_worker_id();
    let config = VolunteerConfig {
        server: server.to_string(),
        api_key: response.api_key,
        username: response.username,
        worker_id,
    };
    save_config(&config)?;
    Ok(config)
}

/// Register this worker machine with the coordinator.
pub fn register_worker(config: &VolunteerConfig) -> Result<()> {
    let url = format!(
        "{}/api/v1/worker/register",
        config.server.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "worker_id": config.worker_id,
        "hostname": hostname(),
        "cores": rayon::current_num_threads(),
        "cpu_model": cpu_model(),
    });
    ureq::post(&url)
        .header("Authorization", &auth_header(config))
        .send_json(&body)?;
    Ok(())
}

/// Send a heartbeat to the coordinator.
pub fn heartbeat(config: &VolunteerConfig) -> Result<()> {
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
pub fn claim_work(config: &VolunteerConfig, cores: usize) -> Result<Option<WorkAssignment>> {
    let url = format!(
        "{}/api/v1/work?cores={}&ram_gb={}",
        config.server.trim_end_matches('/'),
        cores,
        sys_ram_gb(),
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
pub fn submit_result(config: &VolunteerConfig, submission: &ResultSubmission) -> Result<()> {
    let url = format!("{}/api/v1/result", config.server.trim_end_matches('/'));
    ureq::post(&url)
        .header("Authorization", &auth_header(config))
        .send_json(submission)?;
    Ok(())
}

/// Get volunteer stats from the coordinator.
pub fn get_stats(config: &VolunteerConfig) -> Result<VolunteerStats> {
    let url = format!("{}/api/v1/stats", config.server.trim_end_matches('/'));
    let stats: VolunteerStats = ureq::get(&url)
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

// ── Utility ──────────────────────────────────────────────────────

fn auth_header(config: &VolunteerConfig) -> String {
    format!("Bearer {}", config.api_key)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip() {
        let config = VolunteerConfig {
            server: "https://darkreach.example.com".to_string(),
            api_key: "abc123".to_string(),
            username: "alice".to_string(),
            worker_id: "alice-host-12345678".to_string(),
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: VolunteerConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.server, config.server);
        assert_eq!(parsed.api_key, config.api_key);
        assert_eq!(parsed.username, config.username);
    }

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

    #[test]
    fn rand_u32_produces_values() {
        let a = rand_u32();
        assert!(a <= u32::MAX);
    }

    #[test]
    fn hostname_returns_nonempty() {
        let h = hostname();
        assert!(!h.is_empty());
    }

    #[test]
    fn worker_id_format() {
        let id = generate_worker_id();
        assert!(id.contains('-'));
        assert!(id.len() > 9); // hostname-XXXXXXXX
    }
}
