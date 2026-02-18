use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;

use crate::search_manager::SearchParams;

#[derive(Clone, Serialize, PartialEq)]
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

/// Build the SSH command string to launch a primehunt worker on a remote host.
fn build_ssh_command(
    deployment_id: u64,
    coordinator_url: &str,
    database_url: &str,
    params: &SearchParams,
) -> String {
    let worker_id = format!("deploy-{}", deployment_id);
    let cp_path = format!("/opt/primehunt/deploy-{}.checkpoint", deployment_id);
    let log_path = format!("/opt/primehunt/deploy-{}.log", deployment_id);

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
        "mkdir -p /opt/primehunt && DATABASE_URL='{}' nohup /usr/local/bin/primehunt \
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
