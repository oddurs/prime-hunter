//! # Deploy — Remote Worker Deployment Manager
//!
//! Manages the lifecycle of worker processes on remote machines via SSH.
//! The dashboard can deploy, monitor, pause, resume, and stop searches on
//! any reachable host without manual SSH intervention.
//!
//! ## Deployment Lifecycle
//!
//! ```text
//! Deploying → Running → Stopped
//!                ↓          ↑
//!             Paused ───────┘
//!                ↓
//!             Failed (on error)
//! ```
//!
//! ## How It Works
//!
//! Each deployment spawns an SSH subprocess that runs `darkreach` on the
//! remote host. The `DeploymentManager` tracks active deployments by ID,
//! polls remote PIDs for liveness, and stores status + errors for the
//! dashboard to display. Supports custom SSH keys and coordinator URLs.

use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;

use crate::search_manager::SearchParams;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentStatus {
    Deploying,
    Running,
    Paused,
    Failed,
    Stopped,
}

#[derive(Clone, Serialize)]
pub struct Deployment {
    pub id: u64,
    pub hostname: String,
    pub ssh_user: String,
    pub search_type: String,
    pub search_params: String,
    pub worker_id: String,
    pub status: DeploymentStatus,
    pub error: Option<String>,
    pub remote_pid: Option<u32>,
    pub started_at: String,
    #[serde(skip)]
    pub coordinator_url: String,
    #[serde(skip)]
    pub database_url: String,
    #[serde(skip)]
    pub ssh_key: Option<String>,
    #[serde(skip)]
    pub search_params_typed: Option<SearchParams>,
}

pub struct DeploymentManager {
    deployments: HashMap<u64, Deployment>,
    next_id: u64,
}

impl Default for DeploymentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DeploymentManager {
    pub fn new() -> Self {
        DeploymentManager {
            deployments: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn deploy(
        &mut self,
        hostname: String,
        ssh_user: String,
        search_type: String,
        search_params: String,
        coordinator_url: String,
        database_url: String,
        ssh_key: Option<String>,
        search_params_typed: Option<SearchParams>,
    ) -> Deployment {
        let id = self.next_id;
        self.next_id += 1;
        let worker_id = format!("deploy-{}", id);
        let deployment = Deployment {
            id,
            hostname,
            ssh_user,
            search_type,
            search_params,
            worker_id,
            status: DeploymentStatus::Deploying,
            error: None,
            remote_pid: None,
            started_at: Utc::now().to_rfc3339(),
            coordinator_url,
            database_url,
            ssh_key,
            search_params_typed,
        };
        self.deployments.insert(id, deployment.clone());
        deployment
    }

    pub fn mark_running(&mut self, id: u64, pid: u32) {
        if let Some(d) = self.deployments.get_mut(&id) {
            d.status = DeploymentStatus::Running;
            d.remote_pid = Some(pid);
        }
    }

    pub fn mark_failed(&mut self, id: u64, error: String) {
        if let Some(d) = self.deployments.get_mut(&id) {
            d.status = DeploymentStatus::Failed;
            d.error = Some(error);
        }
    }

    pub fn mark_paused(&mut self, id: u64) {
        if let Some(d) = self.deployments.get_mut(&id) {
            d.status = DeploymentStatus::Paused;
            d.remote_pid = None;
        }
    }

    pub fn mark_resuming(&mut self, id: u64) {
        if let Some(d) = self.deployments.get_mut(&id) {
            d.status = DeploymentStatus::Deploying;
        }
    }

    pub fn mark_stopped(&mut self, id: u64) {
        if let Some(d) = self.deployments.get_mut(&id) {
            d.status = DeploymentStatus::Stopped;
        }
    }

    pub fn get(&self, id: u64) -> Option<&Deployment> {
        self.deployments.get(&id)
    }

    pub fn get_all(&self) -> Vec<Deployment> {
        let mut list: Vec<_> = self.deployments.values().cloned().collect();
        list.sort_by(|a, b| b.id.cmp(&a.id));
        list
    }
}

/// Build the SSH command string to launch a darkreach worker on a remote host.
fn build_ssh_command(
    deployment_id: u64,
    coordinator_url: &str,
    database_url: &str,
    params: &SearchParams,
) -> String {
    let worker_id = format!("deploy-{}", deployment_id);
    let cp_path = format!("/opt/darkreach/deploy-{}.checkpoint", deployment_id);
    let log_path = format!("/opt/darkreach/deploy-{}.log", deployment_id);

    let subcommand_args = match params {
        SearchParams::Factorial { start, end } => {
            format!("factorial --start {} --end {}", start, end)
        }
        SearchParams::Palindromic {
            base,
            min_digits,
            max_digits,
        } => format!(
            "palindromic --base {} --min-digits {} --max-digits {}",
            base, min_digits, max_digits
        ),
        SearchParams::Kbn {
            k,
            base,
            min_n,
            max_n,
        } => format!(
            "kbn --k {} --base {} --min-n {} --max-n {}",
            k, base, min_n, max_n
        ),
        SearchParams::Primorial { start, end } => {
            format!("primorial --start {} --end {}", start, end)
        }
        SearchParams::CullenWoodall { min_n, max_n } => {
            format!("cullen-woodall --min-n {} --max-n {}", min_n, max_n)
        }
        SearchParams::Wagstaff { min_exp, max_exp } => {
            format!("wagstaff --min-exp {} --max-exp {}", min_exp, max_exp)
        }
        SearchParams::CarolKynea { min_n, max_n } => {
            format!("carol-kynea --min-n {} --max-n {}", min_n, max_n)
        }
        SearchParams::Twin {
            k,
            base,
            min_n,
            max_n,
        } => format!(
            "twin --k {} --base {} --min-n {} --max-n {}",
            k, base, min_n, max_n
        ),
        SearchParams::SophieGermain {
            k,
            base,
            min_n,
            max_n,
        } => format!(
            "sophie-germain --k {} --base {} --min-n {} --max-n {}",
            k, base, min_n, max_n
        ),
        SearchParams::Repunit { base, min_n, max_n } => format!(
            "repunit --base {} --min-n {} --max-n {}",
            base, min_n, max_n
        ),
        SearchParams::GenFermat {
            fermat_exp,
            min_base,
            max_base,
        } => format!(
            "gen-fermat --fermat-exp {} --min-base {} --max-base {}",
            fermat_exp, min_base, max_base
        ),
    };

    format!(
        "mkdir -p /opt/darkreach && DATABASE_URL='{}' nohup /usr/local/bin/darkreach \
         --coordinator {} --worker-id {} --checkpoint {} \
         {} > {} 2>&1 & echo $!",
        database_url, coordinator_url, worker_id, cp_path, subcommand_args, log_path
    )
}

/// Execute SSH deployment as an async task. Returns the remote PID on success.
pub async fn ssh_deploy(
    hostname: &str,
    ssh_user: &str,
    ssh_key: Option<&str>,
    coordinator_url: &str,
    database_url: &str,
    deployment_id: u64,
    params: &SearchParams,
) -> Result<u32, String> {
    let remote_cmd = build_ssh_command(deployment_id, coordinator_url, database_url, params);

    let mut cmd = tokio::process::Command::new("ssh");
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    cmd.arg("-o").arg("ConnectTimeout=10");

    if let Some(key) = ssh_key {
        cmd.arg("-i").arg(key);
    }

    cmd.arg(format!("{}@{}", ssh_user, hostname));
    cmd.arg(&remote_cmd);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("SSH command failed to execute: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("SSH failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pid_str = stdout.trim();
    pid_str
        .parse::<u32>()
        .map_err(|_| format!("Could not parse remote PID from: {:?}", pid_str))
}

/// Stop a remote worker by killing its PID via SSH.
pub async fn ssh_stop(
    hostname: &str,
    ssh_user: &str,
    ssh_key: Option<&str>,
    remote_pid: u32,
) -> Result<(), String> {
    let mut cmd = tokio::process::Command::new("ssh");
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    cmd.arg("-o").arg("ConnectTimeout=10");

    if let Some(key) = ssh_key {
        cmd.arg("-i").arg(key);
    }

    cmd.arg(format!("{}@{}", ssh_user, hostname));
    cmd.arg(format!("kill {}", remote_pid));

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("SSH kill command failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("SSH kill failed: {}", stderr.trim()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deployment_manager_new_empty() {
        let dm = DeploymentManager::new();
        assert!(dm.get_all().is_empty());
        assert!(dm.get(1).is_none());
    }

    #[test]
    fn deploy_creates_deployment() {
        let mut dm = DeploymentManager::new();
        let d = dm.deploy(
            "host1.example.com".into(),
            "root".into(),
            "kbn".into(),
            r#"{"k":3,"base":2}"#.into(),
            "http://coord:7001".into(),
            "postgres://...".into(),
            None,
            None,
        );
        assert_eq!(d.id, 1);
        assert_eq!(d.hostname, "host1.example.com");
        assert_eq!(d.ssh_user, "root");
        assert_eq!(d.status, DeploymentStatus::Deploying);
        assert_eq!(d.worker_id, "deploy-1");
        assert!(d.remote_pid.is_none());

        assert_eq!(dm.get_all().len(), 1);
        assert!(dm.get(1).is_some());
    }

    #[test]
    fn deploy_increments_id() {
        let mut dm = DeploymentManager::new();
        let d1 = dm.deploy(
            "h1".into(),
            "u".into(),
            "t".into(),
            "p".into(),
            "c".into(),
            "d".into(),
            None,
            None,
        );
        let d2 = dm.deploy(
            "h2".into(),
            "u".into(),
            "t".into(),
            "p".into(),
            "c".into(),
            "d".into(),
            None,
            None,
        );
        assert_eq!(d1.id, 1);
        assert_eq!(d2.id, 2);
    }

    #[test]
    fn status_lifecycle_deploying_to_running() {
        let mut dm = DeploymentManager::new();
        dm.deploy(
            "h".into(),
            "u".into(),
            "t".into(),
            "p".into(),
            "c".into(),
            "d".into(),
            None,
            None,
        );

        dm.mark_running(1, 12345);
        let d = dm.get(1).unwrap();
        assert_eq!(d.status, DeploymentStatus::Running);
        assert_eq!(d.remote_pid, Some(12345));
    }

    #[test]
    fn status_lifecycle_running_to_paused() {
        let mut dm = DeploymentManager::new();
        dm.deploy(
            "h".into(),
            "u".into(),
            "t".into(),
            "p".into(),
            "c".into(),
            "d".into(),
            None,
            None,
        );
        dm.mark_running(1, 12345);

        dm.mark_paused(1);
        let d = dm.get(1).unwrap();
        assert_eq!(d.status, DeploymentStatus::Paused);
        assert!(d.remote_pid.is_none()); // cleared on pause
    }

    #[test]
    fn status_lifecycle_paused_to_resuming() {
        let mut dm = DeploymentManager::new();
        dm.deploy(
            "h".into(),
            "u".into(),
            "t".into(),
            "p".into(),
            "c".into(),
            "d".into(),
            None,
            None,
        );
        dm.mark_paused(1);

        dm.mark_resuming(1);
        let d = dm.get(1).unwrap();
        assert_eq!(d.status, DeploymentStatus::Deploying);
    }

    #[test]
    fn status_lifecycle_to_failed() {
        let mut dm = DeploymentManager::new();
        dm.deploy(
            "h".into(),
            "u".into(),
            "t".into(),
            "p".into(),
            "c".into(),
            "d".into(),
            None,
            None,
        );

        dm.mark_failed(1, "SSH connection refused".into());
        let d = dm.get(1).unwrap();
        assert_eq!(d.status, DeploymentStatus::Failed);
        assert_eq!(d.error, Some("SSH connection refused".into()));
    }

    #[test]
    fn status_lifecycle_to_stopped() {
        let mut dm = DeploymentManager::new();
        dm.deploy(
            "h".into(),
            "u".into(),
            "t".into(),
            "p".into(),
            "c".into(),
            "d".into(),
            None,
            None,
        );
        dm.mark_running(1, 999);

        dm.mark_stopped(1);
        let d = dm.get(1).unwrap();
        assert_eq!(d.status, DeploymentStatus::Stopped);
    }

    #[test]
    fn mark_nonexistent_is_noop() {
        let mut dm = DeploymentManager::new();
        dm.mark_running(999, 123); // should not panic
        dm.mark_failed(999, "err".into());
        dm.mark_paused(999);
        dm.mark_stopped(999);
    }

    #[test]
    fn get_all_sorted_by_id_desc() {
        let mut dm = DeploymentManager::new();
        dm.deploy(
            "h1".into(),
            "u".into(),
            "t".into(),
            "p".into(),
            "c".into(),
            "d".into(),
            None,
            None,
        );
        dm.deploy(
            "h2".into(),
            "u".into(),
            "t".into(),
            "p".into(),
            "c".into(),
            "d".into(),
            None,
            None,
        );
        dm.deploy(
            "h3".into(),
            "u".into(),
            "t".into(),
            "p".into(),
            "c".into(),
            "d".into(),
            None,
            None,
        );

        let all = dm.get_all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].id, 3); // newest first
        assert_eq!(all[1].id, 2);
        assert_eq!(all[2].id, 1);
    }

    #[test]
    fn build_ssh_command_factorial() {
        let cmd = build_ssh_command(
            1,
            "http://coord:7001",
            "postgres://db",
            &SearchParams::Factorial { start: 1, end: 100 },
        );
        assert!(cmd.contains("factorial --start 1 --end 100"));
        assert!(cmd.contains("--worker-id deploy-1"));
        assert!(cmd.contains("--coordinator http://coord:7001"));
        assert!(cmd.contains("DATABASE_URL='postgres://db'"));
        assert!(cmd.contains("deploy-1.checkpoint"));
        assert!(cmd.contains("deploy-1.log"));
        assert!(cmd.contains("nohup"));
        assert!(cmd.contains("echo $!"));
    }

    #[test]
    fn build_ssh_command_kbn() {
        let cmd = build_ssh_command(
            5,
            "http://coord:7001",
            "postgres://db",
            &SearchParams::Kbn {
                k: 3,
                base: 2,
                min_n: 1000,
                max_n: 2000,
            },
        );
        assert!(cmd.contains("kbn --k 3 --base 2 --min-n 1000 --max-n 2000"));
        assert!(cmd.contains("deploy-5"));
    }

    #[test]
    fn build_ssh_command_all_forms() {
        // Just verify none of them panic
        let forms: Vec<SearchParams> = vec![
            SearchParams::Factorial { start: 1, end: 10 },
            SearchParams::Palindromic {
                base: 10,
                min_digits: 1,
                max_digits: 9,
            },
            SearchParams::Kbn {
                k: 3,
                base: 2,
                min_n: 1,
                max_n: 100,
            },
            SearchParams::Primorial { start: 2, end: 50 },
            SearchParams::CullenWoodall {
                min_n: 1,
                max_n: 30,
            },
            SearchParams::Wagstaff {
                min_exp: 3,
                max_exp: 50,
            },
            SearchParams::CarolKynea {
                min_n: 1,
                max_n: 30,
            },
            SearchParams::Twin {
                k: 3,
                base: 2,
                min_n: 1,
                max_n: 100,
            },
            SearchParams::SophieGermain {
                k: 1,
                base: 2,
                min_n: 2,
                max_n: 100,
            },
            SearchParams::Repunit {
                base: 10,
                min_n: 2,
                max_n: 50,
            },
            SearchParams::GenFermat {
                fermat_exp: 1,
                min_base: 2,
                max_base: 100,
            },
        ];
        for (i, params) in forms.iter().enumerate() {
            let cmd = build_ssh_command(i as u64 + 1, "http://c:7001", "postgres://db", params);
            assert!(cmd.contains("nohup"), "Missing nohup for form {}", i);
            assert!(cmd.contains(&format!("deploy-{}", i + 1)));
        }
    }

    #[test]
    fn deployment_status_serde() {
        let statuses = vec![
            DeploymentStatus::Deploying,
            DeploymentStatus::Running,
            DeploymentStatus::Paused,
            DeploymentStatus::Failed,
            DeploymentStatus::Stopped,
        ];
        let expected_strings = vec!["deploying", "running", "paused", "failed", "stopped"];
        for (status, expected) in statuses.iter().zip(expected_strings.iter()) {
            let json = serde_json::to_string(status).unwrap();
            assert!(
                json.contains(expected),
                "Status {:?} serialized as {} but expected {}",
                status,
                json,
                expected
            );
        }
    }
}
