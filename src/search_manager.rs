use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Child;

const MAX_CONCURRENT: usize = 4;

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "search_type")]
pub enum SearchParams {
    #[serde(rename = "factorial")]
    Factorial { start: u64, end: u64 },
    #[serde(rename = "palindromic")]
    Palindromic {
        base: u32,
        min_digits: u64,
        max_digits: u64,
    },
    #[serde(rename = "kbn")]
    Kbn {
        k: u64,
        base: u32,
        min_n: u64,
        max_n: u64,
    },
    #[serde(rename = "primorial")]
    Primorial { start: u64, end: u64 },
    #[serde(rename = "cullen_woodall")]
    CullenWoodall { min_n: u64, max_n: u64 },
    #[serde(rename = "wagstaff")]
    Wagstaff { min_exp: u64, max_exp: u64 },
    #[serde(rename = "carol_kynea")]
    CarolKynea { min_n: u64, max_n: u64 },
    #[serde(rename = "twin")]
    Twin {
        k: u64,
        base: u32,
        min_n: u64,
        max_n: u64,
    },
    #[serde(rename = "sophie_germain")]
    SophieGermain {
        k: u64,
        base: u32,
        min_n: u64,
        max_n: u64,
    },
    #[serde(rename = "repunit")]
    Repunit { base: u32, min_n: u64, max_n: u64 },
    #[serde(rename = "gen_fermat")]
    GenFermat {
        fermat_exp: u32,
        min_base: u64,
        max_base: u64,
    },
}

impl SearchParams {
    fn search_type_name(&self) -> &'static str {
        match self {
            SearchParams::Factorial { .. } => "factorial",
            SearchParams::Palindromic { .. } => "palindromic",
            SearchParams::Kbn { .. } => "kbn",
            SearchParams::Primorial { .. } => "primorial",
            SearchParams::CullenWoodall { .. } => "cullen_woodall",
            SearchParams::Wagstaff { .. } => "wagstaff",
            SearchParams::CarolKynea { .. } => "carol_kynea",
            SearchParams::Twin { .. } => "twin",
            SearchParams::SophieGermain { .. } => "sophie_germain",
            SearchParams::Repunit { .. } => "repunit",
            SearchParams::GenFermat { .. } => "gen_fermat",
        }
    }

    fn to_args(&self) -> Vec<String> {
        match self {
            SearchParams::Factorial { start, end } => {
                vec![
                    "factorial".into(),
                    "--start".into(),
                    start.to_string(),
                    "--end".into(),
                    end.to_string(),
                ]
            }
            SearchParams::Palindromic {
                base,
                min_digits,
                max_digits,
            } => {
                vec![
                    "palindromic".into(),
                    "--base".into(),
                    base.to_string(),
                    "--min-digits".into(),
                    min_digits.to_string(),
                    "--max-digits".into(),
                    max_digits.to_string(),
                ]
            }
            SearchParams::Kbn {
                k,
                base,
                min_n,
                max_n,
            } => {
                vec![
                    "kbn".into(),
                    "--k".into(),
                    k.to_string(),
                    "--base".into(),
                    base.to_string(),
                    "--min-n".into(),
                    min_n.to_string(),
                    "--max-n".into(),
                    max_n.to_string(),
                ]
            }
            SearchParams::Primorial { start, end } => {
                vec![
                    "primorial".into(),
                    "--start".into(),
                    start.to_string(),
                    "--end".into(),
                    end.to_string(),
                ]
            }
            SearchParams::CullenWoodall { min_n, max_n } => {
                vec![
                    "cullen-woodall".into(),
                    "--min-n".into(),
                    min_n.to_string(),
                    "--max-n".into(),
                    max_n.to_string(),
                ]
            }
            SearchParams::Wagstaff { min_exp, max_exp } => {
                vec![
                    "wagstaff".into(),
                    "--min-exp".into(),
                    min_exp.to_string(),
                    "--max-exp".into(),
                    max_exp.to_string(),
                ]
            }
            SearchParams::CarolKynea { min_n, max_n } => {
                vec![
                    "carol-kynea".into(),
                    "--min-n".into(),
                    min_n.to_string(),
                    "--max-n".into(),
                    max_n.to_string(),
                ]
            }
            SearchParams::Twin {
                k,
                base,
                min_n,
                max_n,
            } => {
                vec![
                    "twin".into(),
                    "--k".into(),
                    k.to_string(),
                    "--base".into(),
                    base.to_string(),
                    "--min-n".into(),
                    min_n.to_string(),
                    "--max-n".into(),
                    max_n.to_string(),
                ]
            }
            SearchParams::SophieGermain {
                k,
                base,
                min_n,
                max_n,
            } => {
                vec![
                    "sophie-germain".into(),
                    "--k".into(),
                    k.to_string(),
                    "--base".into(),
                    base.to_string(),
                    "--min-n".into(),
                    min_n.to_string(),
                    "--max-n".into(),
                    max_n.to_string(),
                ]
            }
            SearchParams::Repunit { base, min_n, max_n } => {
                vec![
                    "repunit".into(),
                    "--base".into(),
                    base.to_string(),
                    "--min-n".into(),
                    min_n.to_string(),
                    "--max-n".into(),
                    max_n.to_string(),
                ]
            }
            SearchParams::GenFermat {
                fermat_exp,
                min_base,
                max_base,
            } => {
                vec![
                    "gen-fermat".into(),
                    "--fermat-exp".into(),
                    fermat_exp.to_string(),
                    "--min-base".into(),
                    min_base.to_string(),
                    "--max-base".into(),
                    max_base.to_string(),
                ]
            }
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchStatus {
    Running,
    Paused,
    Completed,
    Cancelled,
    Failed { reason: String },
}

#[derive(Clone, Serialize)]
pub struct SearchInfo {
    pub id: u64,
    pub search_type: String,
    pub params: SearchParams,
    pub status: SearchStatus,
    pub started_at: String,
    pub stopped_at: Option<String>,
    pub pid: Option<u32>,
    pub worker_id: String,
    pub tested: u64,
    pub found: u64,
}

struct SearchEntry {
    info: SearchInfo,
    child: Option<Child>,
    checkpoint_path: String,
}

pub struct SearchManager {
    searches: HashMap<u64, SearchEntry>,
    next_id: u64,
    binary_path: String,
    database_url: String,
    coordinator_url: String,
}

impl SearchManager {
    pub fn new(port: u16, database_url: &str) -> Self {
        let binary_path = std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "primehunt".to_string());
        SearchManager {
            searches: HashMap::new(),
            next_id: 1,
            binary_path,
            database_url: database_url.to_string(),
            coordinator_url: format!("http://127.0.0.1:{}", port),
        }
    }

    pub fn active_count(&self) -> usize {
        self.searches
            .values()
            .filter(|e| matches!(e.info.status, SearchStatus::Running))
            .count()
    }

    pub fn start_search(&mut self, params: SearchParams) -> Result<SearchInfo, String> {
        if self.active_count() >= MAX_CONCURRENT {
            return Err(format!(
                "Maximum {} concurrent searches reached",
                MAX_CONCURRENT
            ));
        }

        let id = self.next_id;
        self.next_id += 1;

        let worker_id = format!("search-{}", id);
        let checkpoint_path = format!("search-{}.checkpoint", id);

        match self.spawn_search_child(&params, &checkpoint_path, &worker_id) {
            Ok(child) => {
                let pid = child.id();
                let info = SearchInfo {
                    id,
                    search_type: params.search_type_name().to_string(),
                    params,
                    status: SearchStatus::Running,
                    started_at: Utc::now().to_rfc3339(),
                    stopped_at: None,
                    pid: Some(pid),
                    worker_id,
                    tested: 0,
                    found: 0,
                };
                let entry = SearchEntry {
                    info: info.clone(),
                    child: Some(child),
                    checkpoint_path,
                };
                self.searches.insert(id, entry);
                eprintln!("Search {} started (pid {})", id, pid);
                Ok(info)
            }
            Err(e) => Err(format!("Failed to spawn process: {}", e)),
        }
    }

    pub fn pause_search(&mut self, id: u64) -> Result<SearchInfo, String> {
        let entry = self
            .searches
            .get_mut(&id)
            .ok_or_else(|| format!("Search {} not found", id))?;

        if !matches!(entry.info.status, SearchStatus::Running) {
            return Err(format!("Search {} is not running", id));
        }

        if let Some(child) = &mut entry.child {
            send_term_signal(child);
            let _ = child.wait();
        }
        entry.child = None;
        entry.info.status = SearchStatus::Paused;
        entry.info.pid = None;
        eprintln!("Search {} paused", id);
        Ok(entry.info.clone())
    }

    pub fn resume_search(&mut self, id: u64) -> Result<SearchInfo, String> {
        if self.active_count() >= MAX_CONCURRENT {
            return Err(format!(
                "Maximum {} concurrent searches reached",
                MAX_CONCURRENT
            ));
        }

        let (params, checkpoint_path, worker_id) = {
            let entry = self
                .searches
                .get(&id)
                .ok_or_else(|| format!("Search {} not found", id))?;

            if !matches!(entry.info.status, SearchStatus::Paused) {
                return Err(format!("Search {} is not paused", id));
            }

            (
                entry.info.params.clone(),
                entry.checkpoint_path.clone(),
                entry.info.worker_id.clone(),
            )
        };

        let child = self.spawn_search_child(&params, &checkpoint_path, &worker_id)?;
        let pid = child.id();

        let entry = self
            .searches
            .get_mut(&id)
            .ok_or_else(|| format!("Search {} not found", id))?;
        entry.child = Some(child);
        entry.info.status = SearchStatus::Running;
        entry.info.pid = Some(pid);
        entry.info.stopped_at = None;
        eprintln!("Search {} resumed (pid {})", id, pid);
        Ok(entry.info.clone())
    }

    pub fn stop_search(&mut self, id: u64) -> Result<SearchInfo, String> {
        let entry = self
            .searches
            .get_mut(&id)
            .ok_or_else(|| format!("Search {} not found", id))?;

        if !matches!(
            entry.info.status,
            SearchStatus::Running | SearchStatus::Paused
        ) {
            return Err(format!("Search {} is not running or paused", id));
        }

        if let Some(child) = &mut entry.child {
            send_term_signal(child);
            let _ = child.wait();
        }
        entry.child = None;
        entry.info.status = SearchStatus::Cancelled;
        entry.info.stopped_at = Some(Utc::now().to_rfc3339());
        entry.info.pid = None;
        eprintln!("Search {} cancelled", id);
        Ok(entry.info.clone())
    }

    pub fn poll_completed(&mut self) {
        for entry in self.searches.values_mut() {
            if !matches!(entry.info.status, SearchStatus::Running) {
                continue;
            }
            if let Some(child) = &mut entry.child {
                match child.try_wait() {
                    Ok(Some(exit_status)) => {
                        let now = Utc::now().to_rfc3339();
                        if exit_status.success() {
                            entry.info.status = SearchStatus::Completed;
                        } else {
                            entry.info.status = SearchStatus::Failed {
                                reason: format!("Exit code: {}", exit_status),
                            };
                        }
                        entry.info.stopped_at = Some(now);
                        entry.info.pid = None;
                        entry.child = None;
                        eprintln!("Search {} finished: {:?}", entry.info.id, entry.info.status);
                    }
                    Ok(None) => {} // still running
                    Err(e) => {
                        entry.info.status = SearchStatus::Failed {
                            reason: format!("Wait error: {}", e),
                        };
                        entry.info.stopped_at = Some(Utc::now().to_rfc3339());
                        entry.info.pid = None;
                        entry.child = None;
                    }
                }
            }
        }
    }

    /// Update tested/found counts for running searches from fleet worker data.
    pub fn sync_worker_stats(&mut self, workers: &[(String, u64, u64)]) {
        for (worker_id, tested, found) in workers {
            for entry in self.searches.values_mut() {
                if entry.info.worker_id == *worker_id {
                    entry.info.tested = *tested;
                    entry.info.found = *found;
                }
            }
        }
    }

    pub fn get_all(&self) -> Vec<SearchInfo> {
        self.searches.values().map(|e| e.info.clone()).collect()
    }

    pub fn get(&self, id: u64) -> Option<SearchInfo> {
        self.searches.get(&id).map(|e| e.info.clone())
    }
}

impl SearchManager {
    fn spawn_search_child(
        &self,
        params: &SearchParams,
        checkpoint_path: &str,
        worker_id: &str,
    ) -> Result<Child, String> {
        let mut cmd = std::process::Command::new(&self.binary_path);
        cmd.arg("--database-url")
            .arg(&self.database_url)
            .arg("--checkpoint")
            .arg(checkpoint_path)
            .arg("--coordinator")
            .arg(&self.coordinator_url)
            .arg("--worker-id")
            .arg(worker_id);

        for arg in params.to_args() {
            cmd.arg(arg);
        }

        // Redirect stdout/stderr to avoid blocking on pipes while keeping stderr in logs.
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::inherit());
        cmd.spawn()
            .map_err(|e| format!("Failed to spawn process: {}", e))
    }
}

/// Send SIGTERM (Unix) or kill (other) to a child process.
fn send_term_signal(child: &Child) {
    #[cfg(unix)]
    {
        // Use the kill command to send SIGTERM without unsafe code
        let _ = std::process::Command::new("kill")
            .arg("-TERM")
            .arg(child.id().to_string())
            .status();
    }
    #[cfg(not(unix))]
    {
        // On non-Unix, fall back to kill (SIGKILL equivalent)
        let _ = child.kill();
    }
}

impl Drop for SearchManager {
    fn drop(&mut self) {
        for entry in self.searches.values_mut() {
            if let Some(child) = &mut entry.child {
                send_term_signal(child);
                let _ = child.wait();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_variants() -> Vec<SearchParams> {
        vec![
            SearchParams::Factorial { start: 1, end: 100 },
            SearchParams::Palindromic {
                base: 10,
                min_digits: 1,
                max_digits: 9,
            },
            SearchParams::Kbn {
                k: 3,
                base: 2,
                min_n: 1,
                max_n: 1000,
            },
            SearchParams::Primorial { start: 2, end: 100 },
            SearchParams::CullenWoodall {
                min_n: 1,
                max_n: 100,
            },
            SearchParams::Wagstaff {
                min_exp: 3,
                max_exp: 100,
            },
            SearchParams::CarolKynea {
                min_n: 1,
                max_n: 100,
            },
            SearchParams::Twin {
                k: 3,
                base: 2,
                min_n: 1,
                max_n: 1000,
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
        ]
    }

    #[test]
    fn search_params_serde_roundtrip_all_variants() {
        for params in all_variants() {
            let json = serde_json::to_string(&params).unwrap();
            let parsed: SearchParams = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&parsed).unwrap();
            assert_eq!(json, json2, "Serde roundtrip failed for: {}", json);
        }
    }

    #[test]
    fn search_params_serde_roundtrip_factorial() {
        let p = SearchParams::Factorial { start: 1, end: 100 };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"search_type\":\"factorial\""));
        assert!(json.contains("\"start\":1"));
        assert!(json.contains("\"end\":100"));
        let parsed: SearchParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.search_type_name(), "factorial");
    }

    #[test]
    fn search_params_serde_roundtrip_kbn() {
        let p = SearchParams::Kbn {
            k: 7,
            base: 3,
            min_n: 10,
            max_n: 500,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"search_type\":\"kbn\""));
        let parsed: SearchParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.search_type_name(), "kbn");
    }

    #[test]
    fn search_type_name_matches_serde_tag() {
        let expected_names = [
            "factorial",
            "palindromic",
            "kbn",
            "primorial",
            "cullen_woodall",
            "wagstaff",
            "carol_kynea",
            "twin",
            "sophie_germain",
            "repunit",
            "gen_fermat",
        ];
        for (params, expected) in all_variants().iter().zip(expected_names.iter()) {
            assert_eq!(
                params.search_type_name(),
                *expected,
                "Mismatch for {:?}",
                serde_json::to_string(params).unwrap()
            );
            // Also verify the serde tag matches
            let json = serde_json::to_string(params).unwrap();
            assert!(
                json.contains(&format!("\"search_type\":\"{}\"", expected)),
                "Serde tag doesn't match for {}",
                expected
            );
        }
    }

    #[test]
    fn to_args_produces_valid_cli_args() {
        let cases: Vec<(SearchParams, Vec<&str>)> = vec![
            (
                SearchParams::Factorial { start: 1, end: 100 },
                vec!["factorial", "--start", "1", "--end", "100"],
            ),
            (
                SearchParams::Palindromic {
                    base: 10,
                    min_digits: 1,
                    max_digits: 9,
                },
                vec!["palindromic", "--base", "10", "--min-digits", "1", "--max-digits", "9"],
            ),
            (
                SearchParams::Kbn {
                    k: 3,
                    base: 2,
                    min_n: 1,
                    max_n: 1000,
                },
                vec!["kbn", "--k", "3", "--base", "2", "--min-n", "1", "--max-n", "1000"],
            ),
            (
                SearchParams::CullenWoodall {
                    min_n: 1,
                    max_n: 30,
                },
                vec!["cullen-woodall", "--min-n", "1", "--max-n", "30"],
            ),
            (
                SearchParams::Wagstaff {
                    min_exp: 3,
                    max_exp: 50,
                },
                vec!["wagstaff", "--min-exp", "3", "--max-exp", "50"],
            ),
            (
                SearchParams::CarolKynea {
                    min_n: 1,
                    max_n: 30,
                },
                vec!["carol-kynea", "--min-n", "1", "--max-n", "30"],
            ),
            (
                SearchParams::GenFermat {
                    fermat_exp: 2,
                    min_base: 2,
                    max_base: 100,
                },
                vec!["gen-fermat", "--fermat-exp", "2", "--min-base", "2", "--max-base", "100"],
            ),
        ];
        for (params, expected) in &cases {
            let args = params.to_args();
            let expected_strings: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
            assert_eq!(args, expected_strings, "to_args mismatch for {:?}", params.search_type_name());
        }
    }

    #[test]
    fn to_args_first_element_is_subcommand() {
        for params in all_variants() {
            let args = params.to_args();
            assert!(!args.is_empty());
            // First arg should be the subcommand name (not a flag)
            assert!(!args[0].starts_with('-'), "First arg should be subcommand, got: {}", args[0]);
        }
    }

    #[test]
    fn search_status_serde() {
        let running = serde_json::to_string(&SearchStatus::Running).unwrap();
        assert!(running.contains("running"));

        let failed = serde_json::to_string(&SearchStatus::Failed {
            reason: "exit 1".into(),
        })
        .unwrap();
        assert!(failed.contains("failed"));
        assert!(failed.contains("exit 1"));
    }
}
