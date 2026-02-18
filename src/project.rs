//! # Project — Campaign-Style Prime Discovery Management
//!
//! Organizes prime-hunting searches into goal-driven **projects**: multi-phase
//! campaigns with objectives like record-hunting, systematic surveys, and
//! verification. Each project is defined in TOML (version-controlled), imported
//! into PostgreSQL, and orchestrated by a 30-second tick loop that advances
//! phases, creates search jobs, and tracks costs.
//!
//! ## Architecture
//!
//! ```text
//! TOML project definition (version-controlled in projects/)
//!     ↓ import
//! PostgreSQL runtime state (projects, phases, records, events)
//!     ↓ orchestrate (30s tick)
//! Search jobs + work blocks (existing infrastructure in db.rs / search_manager.rs)
//!     ↓ claim
//! Workers execute, report primes
//!     ↓ react
//! Orchestration engine advances phases, tracks records, alerts on budget
//! ```
//!
//! ## TOML Format
//!
//! See `projects/templates/` for examples. A project TOML defines:
//! - `[project]` — name, objective, form, tags
//! - `[target]` — digit/range goals
//! - `[competitive]` — current world record for comparison
//! - `[strategy]` — manual or auto-generated phases
//! - `[infrastructure]` — hardware requirements
//! - `[budget]` — cost limits and cloud pricing
//! - `[workers]` — fleet sizing
//!
//! ## Orchestration
//!
//! The `orchestrate_tick` function runs every 30 seconds in the dashboard and:
//! 1. Checks active phases for completion (all blocks done, first prime found, etc.)
//! 2. Activates next eligible phases (dependencies met, conditions satisfied)
//! 3. Aggregates progress and cost to the project level
//! 4. Marks projects completed when all phases are done
//! 5. Checks budget alerts
//!
//! ## Cost Model
//!
//! Per-form empirical power-law timing model estimates core-hours from candidate
//! count and digit size. Cloud pricing defaults to Hetzner AX42 ($0.04/core-hr).
//! PFGW gives ~50× speedup, GWNUM ~100×.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::db::Database;

// ── TOML Configuration Structs ──────────────────────────────────

/// Top-level project configuration parsed from TOML files.
///
/// Maps directly to the `[project]`, `[target]`, `[competitive]`, `[strategy]`,
/// `[infrastructure]`, `[budget]`, and `[workers]` sections of a project TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectMeta,
    #[serde(default)]
    pub target: TargetConfig,
    pub competitive: Option<CompetitiveConfig>,
    #[serde(default)]
    pub strategy: StrategyConfig,
    pub infrastructure: Option<InfrastructureConfig>,
    pub budget: Option<BudgetConfig>,
    pub workers: Option<WorkerConfig>,
}

/// The `[project]` section: identity and classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub objective: Objective,
    pub form: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Project objective type — determines default strategy and success criteria.
///
/// - **Record**: hunt for a new world record in digit count.
/// - **Survey**: systematically enumerate a range for completeness.
/// - **Verification**: re-verify existing results.
/// - **Custom**: user-defined phases with no built-in strategy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Objective {
    Record,
    Survey,
    Verification,
    Custom,
}

impl std::fmt::Display for Objective {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Objective::Record => write!(f, "record"),
            Objective::Survey => write!(f, "survey"),
            Objective::Verification => write!(f, "verification"),
            Objective::Custom => write!(f, "custom"),
        }
    }
}

/// The `[target]` section: what the project aims to achieve.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TargetConfig {
    pub target_digits: Option<u64>,
    pub range_start: Option<u64>,
    pub range_end: Option<u64>,
}

/// The `[competitive]` section: current world record for comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitiveConfig {
    pub current_record_expression: Option<String>,
    pub current_record_digits: Option<u64>,
    pub current_record_holder: Option<String>,
    pub oeis_sequence: Option<String>,
    pub reference_urls: Option<Vec<String>>,
}

/// The `[strategy]` section: manual or auto-generated phase definitions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StrategyConfig {
    #[serde(default)]
    pub auto_strategy: bool,
    #[serde(default)]
    pub phases: Vec<PhaseConfig>,
}

/// A single phase within the strategy: a self-contained search job with
/// dependencies and activation conditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseConfig {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub search_params: serde_json::Value,
    pub block_size: Option<i64>,
    pub depends_on: Option<Vec<String>>,
    pub activation_condition: Option<String>,
    #[serde(default = "default_completion")]
    pub completion: String,
}

fn default_completion() -> String {
    "all_blocks_done".to_string()
}

/// The `[infrastructure]` section: hardware requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureConfig {
    pub min_ram_gb: Option<u32>,
    pub min_cores: Option<u32>,
    pub recommended_cores: Option<u32>,
    #[serde(default)]
    pub required_tools: Vec<String>,
    #[serde(default)]
    pub preferred_tools: Vec<String>,
}

/// The `[budget]` section: cost limits and cloud pricing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub max_cost_usd: Option<f64>,
    pub cost_alert_threshold_usd: Option<f64>,
    #[serde(default = "default_cloud_rate")]
    pub cloud_rate_usd_per_core_hour: f64,
}

fn default_cloud_rate() -> f64 {
    0.04
}

/// The `[workers]` section: fleet sizing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub min_workers: Option<u32>,
    pub max_workers: Option<u32>,
    pub recommended_workers: Option<u32>,
}

// ── TOML Parsing ────────────────────────────────────────────────

/// Parse a project configuration from a TOML string.
pub fn parse_toml(content: &str) -> Result<ProjectConfig> {
    let config: ProjectConfig = toml::from_str(content)?;
    validate_config(&config)?;
    Ok(config)
}

/// Parse a project configuration from a TOML file path.
pub fn parse_toml_file(path: &std::path::Path) -> Result<ProjectConfig> {
    let content = std::fs::read_to_string(path)?;
    parse_toml(&content)
}

/// Validate a project configuration for logical consistency.
fn validate_config(config: &ProjectConfig) -> Result<()> {
    if config.project.name.is_empty() {
        anyhow::bail!("project.name is required");
    }
    if config.project.form.is_empty() {
        anyhow::bail!("project.form is required");
    }

    // Validate form name
    let valid_forms = [
        "factorial",
        "primorial",
        "kbn",
        "palindromic",
        "near_repdigit",
        "cullen_woodall",
        "wagstaff",
        "carol_kynea",
        "twin",
        "sophie_germain",
        "repunit",
        "gen_fermat",
    ];
    if !valid_forms.contains(&config.project.form.as_str()) {
        anyhow::bail!(
            "Unknown form '{}'. Valid forms: {}",
            config.project.form,
            valid_forms.join(", ")
        );
    }

    // For record objective, target_digits is expected
    if config.project.objective == Objective::Record && config.target.target_digits.is_none() {
        eprintln!(
            "Warning: record objective without target_digits — will use world record as target"
        );
    }

    // For survey objective, range is expected
    if config.project.objective == Objective::Survey
        && config.target.range_start.is_none()
        && config.strategy.phases.is_empty()
        && !config.strategy.auto_strategy
    {
        anyhow::bail!("Survey objective requires target.range_start/range_end or strategy.phases");
    }

    Ok(())
}

/// Generate a URL-safe slug from a project name.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ── Cost Estimation ─────────────────────────────────────────────

/// Estimated resource usage and cost for a project or phase.
#[derive(Debug, Clone, Serialize)]
pub struct CostEstimate {
    pub estimated_candidates: u64,
    pub estimated_test_time_secs: f64,
    pub total_core_hours: f64,
    pub total_cost_usd: f64,
    pub estimated_duration_hours: f64,
    pub workers_recommended: u32,
}

/// Empirical seconds per candidate by form and digit count.
/// Power-law model: base_secs * (digits / 1000)^exponent.
/// Values calibrated against GIMPS and primehunt benchmarks.
fn secs_per_candidate(form: &str, digits: u64, has_pfgw: bool) -> f64 {
    let d = (digits as f64) / 1000.0;
    let base = match form {
        "factorial" | "primorial" => 0.5 * d.powf(2.5),
        "kbn" | "twin" | "sophie_germain" => 0.1 * d.powf(2.0),
        "cullen_woodall" | "carol_kynea" => 0.2 * d.powf(2.2),
        "wagstaff" => 0.8 * d.powf(2.5),
        "palindromic" | "near_repdigit" => 0.3 * d.powf(2.0),
        "repunit" => 0.4 * d.powf(2.3),
        "gen_fermat" => 0.3 * d.powf(2.2),
        _ => 0.5 * d.powf(2.5),
    };

    // PFGW/GWNUM accelerated forms are ~50× faster for large candidates
    if has_pfgw && digits >= 10_000 {
        base / 50.0
    } else {
        base
    }
}

/// Estimate project cost from configuration.
pub fn estimate_project_cost(config: &ProjectConfig) -> CostEstimate {
    let cloud_rate = config
        .budget
        .as_ref()
        .map(|b| b.cloud_rate_usd_per_core_hour)
        .unwrap_or(0.04);
    let has_pfgw = config
        .infrastructure
        .as_ref()
        .map(|i| {
            i.preferred_tools.contains(&"pfgw".to_string())
                || i.preferred_tools.contains(&"gwnum".to_string())
        })
        .unwrap_or(false);
    let workers = config
        .workers
        .as_ref()
        .and_then(|w| w.recommended_workers)
        .unwrap_or(4);
    let cores_per_worker = config
        .infrastructure
        .as_ref()
        .and_then(|i| i.recommended_cores)
        .unwrap_or(16);

    // Estimate candidate count from phases or target
    let (candidates, avg_digits) = estimate_candidates(config);

    let spc = secs_per_candidate(&config.project.form, avg_digits, has_pfgw);
    let total_test_secs = candidates as f64 * spc;
    let total_core_hours = total_test_secs / 3600.0;
    let total_cores = workers * cores_per_worker;
    let duration_hours = total_core_hours / total_cores as f64;
    let cost = total_core_hours * cloud_rate;

    CostEstimate {
        estimated_candidates: candidates,
        estimated_test_time_secs: total_test_secs,
        total_core_hours,
        total_cost_usd: cost,
        estimated_duration_hours: duration_hours,
        workers_recommended: workers,
    }
}

/// Estimate candidate count and average digit size from config.
fn estimate_candidates(config: &ProjectConfig) -> (u64, u64) {
    // If phases are defined, sum their ranges
    if !config.strategy.phases.is_empty() {
        let mut total = 0u64;
        let mut total_digits = 0u64;
        let mut phase_count = 0u64;
        for phase in &config.strategy.phases {
            let (start, end) = extract_range_from_params(&phase.search_params);
            if end > start {
                total += end - start;
                // Rough digit estimate from range midpoint
                let mid = (start + end) / 2;
                total_digits += estimate_digits_for_form(&config.project.form, mid);
                phase_count += 1;
            }
        }
        let avg_digits = if phase_count > 0 {
            total_digits / phase_count
        } else {
            1000
        };
        return (total.max(1), avg_digits.max(1));
    }

    // Fall back to target range
    if let (Some(start), Some(end)) = (config.target.range_start, config.target.range_end) {
        let mid = (start + end) / 2;
        let digits = estimate_digits_for_form(&config.project.form, mid);
        return ((end - start).max(1), digits.max(1));
    }

    // Default: assume 10K candidates at 1K digits
    (10_000, 1000)
}

/// Extract (start, end) range from search_params JSON.
fn extract_range_from_params(params: &serde_json::Value) -> (u64, u64) {
    let start = params
        .get("start")
        .or_else(|| params.get("min_n"))
        .or_else(|| params.get("min_exp"))
        .or_else(|| params.get("min_digits"))
        .or_else(|| params.get("min_base"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let end = params
        .get("end")
        .or_else(|| params.get("max_n"))
        .or_else(|| params.get("max_exp"))
        .or_else(|| params.get("max_digits"))
        .or_else(|| params.get("max_base"))
        .and_then(|v| v.as_u64())
        .unwrap_or(start);
    (start, end)
}

/// Rough decimal digit estimate for a form at parameter value n.
fn estimate_digits_for_form(form: &str, n: u64) -> u64 {
    match form {
        // n! has ~n*log10(n/e) digits (Stirling)
        "factorial" => {
            if n < 3 {
                return 1;
            }
            let nf = n as f64;
            (nf * (nf / std::f64::consts::E).log10()) as u64
        }
        // p# has ~p/ln(10) digits (prime number theorem)
        "primorial" => (n as f64 / std::f64::consts::LN_10) as u64,
        // k*b^n has ~n*log10(b) digits
        "kbn" | "twin" | "sophie_germain" => (n as f64 * 2.0f64.log10()) as u64,
        // n*2^n+1 has ~n*log10(2) digits
        "cullen_woodall" => (n as f64 * 2.0f64.log10()) as u64,
        // (2^p+1)/3 has ~p*log10(2) digits
        "wagstaff" => (n as f64 * 2.0f64.log10()) as u64,
        // (2^n±1)²-2 has ~2n*log10(2) digits
        "carol_kynea" => (2.0 * n as f64 * 2.0f64.log10()) as u64,
        // Palindromic primes: n is the digit count
        "palindromic" | "near_repdigit" => n,
        // (b^n-1)/(b-1): n digits in base b, ~n*log10(b) decimal digits
        "repunit" => n,
        // b^(2^n)+1: digits depend on base range
        "gen_fermat" => (n as f64 * 2.0f64.log10()) as u64,
        _ => n,
    }
}

// ── Auto-Strategy Generation ────────────────────────────────────

/// Generate form-specific default phases when `auto_strategy = true` and
/// no manual phases are defined.
pub fn generate_auto_strategy(config: &ProjectConfig) -> Vec<PhaseConfig> {
    let form = &config.project.form;
    let objective = &config.project.objective;

    match (form.as_str(), objective) {
        ("factorial", Objective::Record) => {
            let start = config.target.range_start.unwrap_or(1000);
            let end = config.target.range_end.unwrap_or(start + 10_000);
            vec![PhaseConfig {
                name: "sweep".to_string(),
                description: format!("Sequential factorial search n={}..{}", start, end),
                search_params: serde_json::json!({
                    "search_type": "factorial",
                    "start": start,
                    "end": end,
                }),
                block_size: Some(100),
                depends_on: None,
                activation_condition: None,
                completion: "all_blocks_done".to_string(),
            }]
        }
        ("factorial", Objective::Survey) => {
            let start = config.target.range_start.unwrap_or(1);
            let end = config.target.range_end.unwrap_or(10_000);
            vec![PhaseConfig {
                name: "survey".to_string(),
                description: format!("Complete factorial survey n={}..{}", start, end),
                search_params: serde_json::json!({
                    "search_type": "factorial",
                    "start": start,
                    "end": end,
                }),
                block_size: Some(100),
                depends_on: None,
                activation_condition: None,
                completion: "all_blocks_done".to_string(),
            }]
        }
        ("wagstaff", Objective::Record) => {
            let start = config.target.range_start.unwrap_or(13_400_000);
            let end = config.target.range_end.unwrap_or(start + 5_000_000);
            let mid = start + (end - start) / 2;
            vec![
                PhaseConfig {
                    name: "sweep".to_string(),
                    description: format!("Sieve exponents {}..{}", start, mid),
                    search_params: serde_json::json!({
                        "search_type": "wagstaff",
                        "min_exp": start,
                        "max_exp": mid,
                    }),
                    block_size: Some(1000),
                    depends_on: None,
                    activation_condition: None,
                    completion: "all_blocks_done".to_string(),
                },
                PhaseConfig {
                    name: "extend".to_string(),
                    description: format!("Extend to {} if no discovery", end),
                    search_params: serde_json::json!({
                        "search_type": "wagstaff",
                        "min_exp": mid + 1,
                        "max_exp": end,
                    }),
                    block_size: Some(1000),
                    depends_on: Some(vec!["sweep".to_string()]),
                    activation_condition: Some("previous_phase_found_zero".to_string()),
                    completion: "all_blocks_done".to_string(),
                },
            ]
        }
        ("kbn" | "twin" | "sophie_germain", _) => {
            let start = config.target.range_start.unwrap_or(1);
            let end = config.target.range_end.unwrap_or(1_000_000);
            vec![PhaseConfig {
                name: "sweep".to_string(),
                description: format!("Sweep n={}..{}", start, end),
                search_params: serde_json::json!({
                    "search_type": form,
                    "k": 1,
                    "base": 2,
                    "min_n": start,
                    "max_n": end,
                }),
                block_size: Some(100_000),
                depends_on: None,
                activation_condition: None,
                completion: "all_blocks_done".to_string(),
            }]
        }
        ("palindromic" | "near_repdigit", _) => {
            let start = config.target.range_start.unwrap_or(1);
            let end = config.target.range_end.unwrap_or(21);
            vec![PhaseConfig {
                name: "sweep".to_string(),
                description: format!("Sweep digit counts {}..{}", start, end),
                search_params: serde_json::json!({
                    "search_type": form,
                    "min_digits": start,
                    "max_digits": end,
                }),
                block_size: Some(2),
                depends_on: None,
                activation_condition: None,
                completion: "all_blocks_done".to_string(),
            }]
        }
        _ => {
            // Generic: single phase from target range
            let start = config.target.range_start.unwrap_or(1);
            let end = config.target.range_end.unwrap_or(10_000);
            vec![PhaseConfig {
                name: "sweep".to_string(),
                description: format!("{} search {}..{}", form, start, end),
                search_params: serde_json::json!({
                    "search_type": form,
                    "start": start,
                    "end": end,
                }),
                block_size: Some(1000),
                depends_on: None,
                activation_condition: None,
                completion: "all_blocks_done".to_string(),
            }]
        }
    }
}

// ── Orchestration Engine ────────────────────────────────────────

/// Run one orchestration tick for all active projects.
/// Called every 30 seconds from the dashboard background task.
///
/// For each active project:
/// 1. Check active phases for completion
/// 2. Activate next eligible phases (creates search jobs)
/// 3. Aggregate progress and cost
/// 4. Check if all phases are done → mark project completed
/// 5. Check budget alerts
pub async fn orchestrate_tick(db: &Database) -> Result<()> {
    let projects = db.get_projects(Some("active")).await?;

    for project in &projects {
        if let Err(e) = orchestrate_project(db, project).await {
            eprintln!(
                "Orchestration error for project '{}': {}",
                project.slug, e
            );
            db.insert_project_event(
                project.id,
                "error",
                &format!("Orchestration error: {}", e),
                None,
            )
            .await
            .ok();
        }
    }

    Ok(())
}

/// Orchestrate a single project: advance phases, aggregate progress.
async fn orchestrate_project(db: &Database, project: &ProjectRow) -> Result<()> {
    let phases = db.get_project_phases(project.id).await?;

    // 1. Check active phases for completion
    for phase in phases.iter().filter(|p| p.status == "active") {
        if let Some(job_id) = phase.search_job_id {
            let summary = db.get_job_block_summary(job_id).await?;

            // Update phase progress
            db.update_phase_progress(phase.id, summary.total_tested, summary.total_found)
                .await?;

            // Check completion condition
            if is_phase_complete(&phase.completion_condition, &summary) {
                db.update_phase_status(phase.id, "completed").await?;
                db.insert_project_event(
                    project.id,
                    "phase_completed",
                    &format!(
                        "Phase '{}' completed: {} tested, {} found",
                        phase.name, summary.total_tested, summary.total_found
                    ),
                    None,
                )
                .await?;
                eprintln!(
                    "Project '{}': phase '{}' completed ({} tested, {} found)",
                    project.slug, phase.name, summary.total_tested, summary.total_found
                );
            }
        }
    }

    // Reload phases after potential status changes
    let phases = db.get_project_phases(project.id).await?;

    // 2. Activate next eligible phases
    for phase in phases.iter().filter(|p| p.status == "pending") {
        if should_activate(phase, &phases) {
            activate_phase(db, project, phase).await?;
        }
    }

    // 3. Aggregate progress to project level
    let phases = db.get_project_phases(project.id).await?;
    let total_tested: i64 = phases.iter().map(|p| p.total_tested).sum();
    let total_found: i64 = phases.iter().map(|p| p.total_found).sum();
    db.update_project_progress(project.id, total_tested, total_found)
        .await?;

    // 4. Check if all phases are terminal (completed/skipped/failed)
    let all_terminal = phases
        .iter()
        .all(|p| matches!(p.status.as_str(), "completed" | "skipped" | "failed"));
    if all_terminal && !phases.is_empty() {
        let any_failed = phases.iter().any(|p| p.status == "failed");
        let new_status = if any_failed { "failed" } else { "completed" };
        db.update_project_status(project.id, new_status).await?;
        db.insert_project_event(
            project.id,
            "project_completed",
            &format!(
                "Project '{}' {}: {} tested, {} found",
                project.name, new_status, total_tested, total_found
            ),
            None,
        )
        .await?;
        eprintln!(
            "Project '{}' {}: {} tested, {} found",
            project.slug, new_status, total_tested, total_found
        );
    }

    // 5. Check budget alerts
    if let Some(max_cost) = project.budget.get("max_cost_usd").and_then(serde_json::Value::as_f64)
    {
        if project.total_cost_usd >= max_cost {
            db.update_project_status(project.id, "paused").await?;
            db.insert_project_event(
                project.id,
                "budget_exceeded",
                &format!(
                    "Budget exceeded: ${:.2} >= ${:.2} — project paused",
                    project.total_cost_usd, max_cost
                ),
                None,
            )
            .await?;
            eprintln!(
                "Project '{}' paused: budget exceeded (${:.2} >= ${:.2})",
                project.slug, project.total_cost_usd, max_cost
            );
        } else if let Some(alert) = project
            .budget
            .get("cost_alert_threshold_usd")
            .and_then(serde_json::Value::as_f64)
        {
            if project.total_cost_usd >= alert {
                db.insert_project_event(
                    project.id,
                    "budget_alert",
                    &format!(
                        "Cost alert: ${:.2} >= ${:.2} threshold",
                        project.total_cost_usd, alert
                    ),
                    None,
                )
                .await?;
            }
        }
    }

    Ok(())
}

/// Check if a phase's completion condition is satisfied.
fn is_phase_complete(condition: &str, summary: &crate::db::JobBlockSummary) -> bool {
    match condition {
        "all_blocks_done" => summary.available == 0 && summary.claimed == 0,
        "first_prime_found" => summary.total_found > 0,
        _ => summary.available == 0 && summary.claimed == 0,
    }
}

/// Check if a pending phase should be activated (all dependencies met,
/// activation condition satisfied).
fn should_activate(phase: &ProjectPhaseRow, all_phases: &[ProjectPhaseRow]) -> bool {
    // Check all depends_on phases are completed
    for dep_name in &phase.depends_on {
        let dep = all_phases.iter().find(|p| p.name == *dep_name);
        match dep {
            Some(d) if d.status == "completed" => {}
            _ => return false,
        }
    }

    // Check activation condition
    if let Some(condition) = &phase.activation_condition {
        match condition.as_str() {
            "previous_phase_found_zero" => {
                // The phase this depends on must have found zero primes
                for dep_name in &phase.depends_on {
                    if let Some(dep) = all_phases.iter().find(|p| p.name == *dep_name) {
                        if dep.total_found > 0 {
                            return false;
                        }
                    }
                }
            }
            "previous_phase_found_prime" => {
                // The phase this depends on must have found at least one prime
                let any_found = phase.depends_on.iter().any(|dep_name| {
                    all_phases
                        .iter()
                        .find(|p| p.name == *dep_name)
                        .map(|dep| dep.total_found > 0)
                        .unwrap_or(false)
                });
                if !any_found {
                    return false;
                }
            }
            _ => {}
        }
    }

    true
}

/// Activate a phase: create a search job via the existing infrastructure.
async fn activate_phase(
    db: &Database,
    project: &ProjectRow,
    phase: &ProjectPhaseRow,
) -> Result<()> {
    let search_type = phase
        .search_params
        .get("search_type")
        .and_then(|v| v.as_str())
        .unwrap_or(&project.form);

    let (range_start, range_end) = extract_range_from_params(&phase.search_params);
    if range_end <= range_start {
        anyhow::bail!(
            "Phase '{}' has invalid range: {}..{}",
            phase.name,
            range_start,
            range_end
        );
    }

    // Create search job using existing infrastructure
    let job_id = db
        .create_search_job(
            search_type,
            &phase.search_params,
            range_start as i64,
            range_end as i64,
            phase.block_size,
        )
        .await?;

    // Link job to project
    db.link_search_job_to_project(job_id, project.id).await?;

    // Update phase with job reference
    db.activate_phase(phase.id, job_id).await?;

    db.insert_project_event(
        project.id,
        "phase_activated",
        &format!(
            "Phase '{}' activated: search job {} (range {}..{}, {} blocks)",
            phase.name,
            job_id,
            range_start,
            range_end,
            (range_end - range_start).div_ceil(phase.block_size as u64)
        ),
        Some(&serde_json::json!({
            "phase_id": phase.id,
            "search_job_id": job_id,
        })),
    )
    .await?;

    eprintln!(
        "Project '{}': activated phase '{}' → search job {}",
        project.slug, phase.name, job_id
    );

    Ok(())
}

// ── Records Tracking ────────────────────────────────────────────

/// Known t5k.org Top 20 page IDs for each form.
/// Used by `fetch_t5k_record` to scrape current world records.
pub fn t5k_page_id(form: &str) -> Option<u32> {
    match form {
        "factorial" => Some(15),
        "primorial" => Some(41),
        "wagstaff" => Some(67),
        "palindromic" => Some(39),
        "twin" => Some(1),
        "sophie_germain" => Some(2),
        "repunit" => Some(44),
        "gen_fermat" => Some(16),
        _ => None,
    }
}

/// OEIS sequence IDs for each form.
pub fn oeis_sequence(form: &str) -> Option<&'static str> {
    match form {
        "factorial" => Some("A002981"),
        "primorial" => Some("A014545"),
        "wagstaff" => Some("A000978"),
        "palindromic" => Some("A002385"),
        "sophie_germain" => Some("A005384"),
        "repunit" => Some("A004023"),
        "gen_fermat" => Some("A019434"),
        _ => None,
    }
}

/// Fetch the current world record for a form from t5k.org (The Prime Pages).
/// Parses the first table row from the Top 20 page.
pub async fn fetch_t5k_record(form: &str) -> Result<Option<RecordInfo>> {
    let page_id = match t5k_page_id(form) {
        Some(id) => id,
        None => return Ok(None),
    };

    let url = format!("https://t5k.org/top20/page.php?id={}", page_id);
    let response = reqwest::get(&url).await?;
    let html = response.text().await?;

    parse_t5k_html(&html, form)
}

/// Parsed record information from t5k.org.
#[derive(Debug, Clone, Serialize)]
pub struct RecordInfo {
    pub expression: String,
    pub digits: u64,
    pub holder: String,
    pub discovered_at: Option<String>,
    pub source_url: String,
}

/// Parse t5k.org Top 20 HTML to extract the first (largest) entry.
pub fn parse_t5k_html(html: &str, form: &str) -> Result<Option<RecordInfo>> {
    let document = scraper::Html::parse_document(html);
    let table_sel = scraper::Selector::parse("table.list").unwrap();
    let row_sel = scraper::Selector::parse("tr").unwrap();
    let cell_sel = scraper::Selector::parse("td").unwrap();

    let table = match document.select(&table_sel).next() {
        Some(t) => t,
        None => return Ok(None),
    };

    // Skip header row, get first data row
    let row = match table.select(&row_sel).nth(1) {
        Some(r) => r,
        None => return Ok(None),
    };

    let cells: Vec<String> = row
        .select(&cell_sel)
        .map(|c| c.text().collect::<String>().trim().to_string())
        .collect();

    // t5k.org Top 20 tables typically have columns:
    // rank | prime | digits | who | when | comment
    if cells.len() < 4 {
        return Ok(None);
    }

    let expression = cells[1].clone();
    let digits = cells[2]
        .replace(',', "")
        .parse::<u64>()
        .unwrap_or(0);
    let holder = cells[3].clone();
    let discovered_at = cells.get(4).cloned();

    let page_id = t5k_page_id(form).unwrap_or(0);

    Ok(Some(RecordInfo {
        expression,
        digits,
        holder,
        discovered_at,
        source_url: format!("https://t5k.org/top20/page.php?id={}", page_id),
    }))
}

/// Refresh all records for forms that have t5k.org pages.
/// Called on dashboard startup and every 24 hours.
pub async fn refresh_all_records(db: &Database) -> Result<u32> {
    let forms = [
        "factorial",
        "primorial",
        "wagstaff",
        "palindromic",
        "twin",
        "sophie_germain",
        "repunit",
        "gen_fermat",
    ];
    let mut updated = 0u32;

    for form in &forms {
        match fetch_t5k_record(form).await {
            Ok(Some(record)) => {
                // Get our best prime for this form
                let our_best = db.get_best_prime_for_form(form).await.unwrap_or(None);
                let (our_best_id, our_best_digits) = our_best
                    .map(|p| (Some(p.id), p.digits))
                    .unwrap_or((None, 0));

                db.upsert_record(
                    form,
                    "overall",
                    &record.expression,
                    record.digits as i64,
                    Some(&record.holder),
                    record.discovered_at.as_deref(),
                    Some("t5k.org"),
                    Some(&record.source_url),
                    our_best_id,
                    our_best_digits,
                )
                .await?;
                updated += 1;
                eprintln!(
                    "Record updated: {} — {} ({} digits, by {})",
                    form, record.expression, record.digits, record.holder
                );
            }
            Ok(None) => {
                eprintln!("No t5k record found for form '{}'", form);
            }
            Err(e) => {
                eprintln!("Error fetching record for '{}': {}", form, e);
            }
        }
    }

    Ok(updated)
}

// ── Database Row Types ──────────────────────────────────────────

/// Database row for a project.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProjectRow {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub objective: String,
    pub form: String,
    pub status: String,
    pub toml_source: Option<String>,
    pub target: serde_json::Value,
    pub competitive: serde_json::Value,
    pub strategy: serde_json::Value,
    pub infrastructure: serde_json::Value,
    pub budget: serde_json::Value,
    pub total_tested: i64,
    pub total_found: i64,
    pub best_prime_id: Option<i64>,
    pub best_digits: i64,
    pub total_core_hours: f64,
    pub total_cost_usd: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Database row for a project phase.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProjectPhaseRow {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub description: String,
    pub phase_order: i32,
    pub status: String,
    pub search_params: serde_json::Value,
    pub block_size: i64,
    pub depends_on: Vec<String>,
    pub activation_condition: Option<String>,
    pub completion_condition: String,
    pub search_job_id: Option<i64>,
    pub total_tested: i64,
    pub total_found: i64,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Database row for a world record.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct RecordRow {
    pub id: i64,
    pub form: String,
    pub category: String,
    pub expression: String,
    pub digits: i64,
    pub holder: Option<String>,
    pub discovered_at: Option<chrono::NaiveDate>,
    pub source: Option<String>,
    pub source_url: Option<String>,
    pub our_best_id: Option<i64>,
    pub our_best_digits: i64,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Database row for a project event.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProjectEventRow {
    pub id: i64,
    pub project_id: i64,
    pub event_type: String,
    pub summary: String,
    pub detail: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_toml() {
        let toml = r#"
[project]
name = "test-project"
objective = "survey"
form = "factorial"

[target]
range_start = 1
range_end = 1000

[[strategy.phases]]
name = "sweep"
search_params = { search_type = "factorial", start = 1, end = 1000 }
"#;
        let config = parse_toml(toml).unwrap();
        assert_eq!(config.project.name, "test-project");
        assert_eq!(config.project.objective, Objective::Survey);
        assert_eq!(config.project.form, "factorial");
        assert_eq!(config.strategy.phases.len(), 1);
        assert_eq!(config.strategy.phases[0].name, "sweep");
    }

    #[test]
    fn parse_full_toml() {
        let toml = r#"
[project]
name = "wagstaff-record-2026"
description = "Hunt for new Wagstaff primes"
objective = "record"
form = "wagstaff"
author = "oddurs"
tags = ["wagstaff", "record"]

[target]
target_digits = 5000000

[competitive]
current_record_expression = "(2^13347311+1)/3"
current_record_digits = 4017941
current_record_holder = "Ryan Propper"
oeis_sequence = "A000978"

[strategy]
auto_strategy = false

[[strategy.phases]]
name = "sweep"
description = "Sieve exponents 15M..20M"
search_params = { search_type = "wagstaff", min_exp = 15135397, max_exp = 20000000 }
block_size = 1000
completion = "all_blocks_done"

[[strategy.phases]]
name = "extend"
description = "Extend to 25M if no discovery"
search_params = { search_type = "wagstaff", min_exp = 20000001, max_exp = 25000000 }
block_size = 1000
depends_on = ["sweep"]
activation_condition = "previous_phase_found_zero"

[infrastructure]
min_ram_gb = 8
min_cores = 4
recommended_cores = 16
preferred_tools = ["gwnum"]

[budget]
max_cost_usd = 500.0
cost_alert_threshold_usd = 100.0
cloud_rate_usd_per_core_hour = 0.04

[workers]
min_workers = 1
max_workers = 8
recommended_workers = 4
"#;
        let config = parse_toml(toml).unwrap();
        assert_eq!(config.project.name, "wagstaff-record-2026");
        assert_eq!(config.project.objective, Objective::Record);
        assert_eq!(config.strategy.phases.len(), 2);
        assert_eq!(
            config.strategy.phases[1].depends_on,
            Some(vec!["sweep".to_string()])
        );
        assert_eq!(
            config.strategy.phases[1].activation_condition,
            Some("previous_phase_found_zero".to_string())
        );
        assert!(config.budget.is_some());
        assert_eq!(
            config.budget.as_ref().unwrap().max_cost_usd,
            Some(500.0)
        );
    }

    #[test]
    fn parse_invalid_form_rejected() {
        let toml = r#"
[project]
name = "test"
objective = "survey"
form = "invalid_form"

[target]
range_start = 1
range_end = 100

[[strategy.phases]]
name = "x"
search_params = { search_type = "invalid_form", start = 1, end = 100 }
"#;
        assert!(parse_toml(toml).is_err());
    }

    #[test]
    fn slugify_names() {
        assert_eq!(slugify("wagstaff-record-2026"), "wagstaff-record-2026");
        assert_eq!(slugify("My Test Project!"), "my-test-project");
        assert_eq!(slugify("  hello   world  "), "hello-world");
        assert_eq!(
            slugify("factorial survey (n=1..1000)"),
            "factorial-survey-n-1-1000"
        );
    }

    #[test]
    fn cost_estimate_produces_nonzero() {
        let config = parse_toml(
            r#"
[project]
name = "test"
objective = "survey"
form = "factorial"

[target]
range_start = 1
range_end = 1000

[[strategy.phases]]
name = "sweep"
search_params = { search_type = "factorial", start = 1, end = 1000 }
"#,
        )
        .unwrap();

        let est = estimate_project_cost(&config);
        assert!(est.estimated_candidates > 0);
        assert!(est.total_core_hours >= 0.0);
        assert!(est.total_cost_usd >= 0.0);
        assert!(est.workers_recommended > 0);
    }

    #[test]
    fn auto_strategy_factorial_record() {
        let config = parse_toml(
            r#"
[project]
name = "test"
objective = "record"
form = "factorial"

[strategy]
auto_strategy = true

[target]
range_start = 500
range_end = 1500
"#,
        )
        .unwrap();
        let phases = generate_auto_strategy(&config);
        assert!(!phases.is_empty());
        assert_eq!(phases[0].name, "sweep");
    }

    #[test]
    fn auto_strategy_wagstaff_record_two_phases() {
        let config = parse_toml(
            r#"
[project]
name = "test"
objective = "record"
form = "wagstaff"

[strategy]
auto_strategy = true

[target]
range_start = 14000000
range_end = 20000000
"#,
        )
        .unwrap();
        let phases = generate_auto_strategy(&config);
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].name, "sweep");
        assert_eq!(phases[1].name, "extend");
        assert_eq!(
            phases[1].activation_condition,
            Some("previous_phase_found_zero".to_string())
        );
    }

    #[test]
    fn should_activate_no_deps() {
        let phase = ProjectPhaseRow {
            id: 1,
            project_id: 1,
            name: "sweep".into(),
            description: String::new(),
            phase_order: 0,
            status: "pending".into(),
            search_params: serde_json::json!({}),
            block_size: 1000,
            depends_on: vec![],
            activation_condition: None,
            completion_condition: "all_blocks_done".into(),
            search_job_id: None,
            total_tested: 0,
            total_found: 0,
            started_at: None,
            completed_at: None,
        };
        assert!(should_activate(&phase, &[phase.clone()]));
    }

    #[test]
    fn should_activate_unmet_dep() {
        let sweep = ProjectPhaseRow {
            id: 1,
            project_id: 1,
            name: "sweep".into(),
            description: String::new(),
            phase_order: 0,
            status: "active".into(),
            search_params: serde_json::json!({}),
            block_size: 1000,
            depends_on: vec![],
            activation_condition: None,
            completion_condition: "all_blocks_done".into(),
            search_job_id: None,
            total_tested: 0,
            total_found: 0,
            started_at: None,
            completed_at: None,
        };
        let extend = ProjectPhaseRow {
            id: 2,
            project_id: 1,
            name: "extend".into(),
            description: String::new(),
            phase_order: 1,
            status: "pending".into(),
            search_params: serde_json::json!({}),
            block_size: 1000,
            depends_on: vec!["sweep".into()],
            activation_condition: None,
            completion_condition: "all_blocks_done".into(),
            search_job_id: None,
            total_tested: 0,
            total_found: 0,
            started_at: None,
            completed_at: None,
        };
        assert!(!should_activate(&extend, &[sweep, extend.clone()]));
    }

    #[test]
    fn should_activate_met_dep_with_condition() {
        let sweep = ProjectPhaseRow {
            id: 1,
            project_id: 1,
            name: "sweep".into(),
            description: String::new(),
            phase_order: 0,
            status: "completed".into(),
            search_params: serde_json::json!({}),
            block_size: 1000,
            depends_on: vec![],
            activation_condition: None,
            completion_condition: "all_blocks_done".into(),
            search_job_id: None,
            total_tested: 1000,
            total_found: 0, // found nothing
            started_at: None,
            completed_at: None,
        };
        let extend = ProjectPhaseRow {
            id: 2,
            project_id: 1,
            name: "extend".into(),
            description: String::new(),
            phase_order: 1,
            status: "pending".into(),
            search_params: serde_json::json!({}),
            block_size: 1000,
            depends_on: vec!["sweep".into()],
            activation_condition: Some("previous_phase_found_zero".into()),
            completion_condition: "all_blocks_done".into(),
            search_job_id: None,
            total_tested: 0,
            total_found: 0,
            started_at: None,
            completed_at: None,
        };
        assert!(should_activate(&extend, &[sweep, extend.clone()]));
    }

    #[test]
    fn phase_complete_all_blocks_done() {
        let summary = crate::db::JobBlockSummary {
            available: 0,
            claimed: 0,
            completed: 10,
            failed: 0,
            total_tested: 10000,
            total_found: 2,
        };
        assert!(is_phase_complete("all_blocks_done", &summary));
    }

    #[test]
    fn phase_complete_first_prime_found() {
        let summary = crate::db::JobBlockSummary {
            available: 5,
            claimed: 2,
            completed: 3,
            failed: 0,
            total_tested: 3000,
            total_found: 1,
        };
        assert!(is_phase_complete("first_prime_found", &summary));
    }

    #[test]
    fn t5k_html_parsing() {
        // Minimal mock of t5k.org table structure
        let html = r#"
<html><body>
<table class="list">
<tr><th>rank</th><th>prime</th><th>digits</th><th>who</th><th>when</th></tr>
<tr><td>1</td><td>208003! - 1</td><td>1,015,843</td><td>Fujii</td><td>2023</td></tr>
<tr><td>2</td><td>150209! + 1</td><td>712,355</td><td>Kuosa</td><td>2021</td></tr>
</table>
</body></html>"#;
        let record = parse_t5k_html(html, "factorial").unwrap().unwrap();
        assert_eq!(record.expression, "208003! - 1");
        assert_eq!(record.digits, 1015843);
        assert_eq!(record.holder, "Fujii");
    }

    #[test]
    fn template_files_parse() {
        let templates_dir = std::path::Path::new("projects/templates");
        if !templates_dir.exists() {
            return; // skip if templates not yet created
        }
        for entry in std::fs::read_dir(templates_dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().map_or(false, |e| e == "toml") {
                let content = std::fs::read_to_string(&path).unwrap();
                let result = parse_toml(&content);
                assert!(
                    result.is_ok(),
                    "Template {:?} failed to parse: {:?}",
                    path.file_name().unwrap(),
                    result.err()
                );
                let config = result.unwrap();
                assert!(!config.project.name.is_empty());
                assert!(!config.project.form.is_empty());
            }
        }
    }

    #[test]
    fn extract_range_from_params_variants() {
        let params = serde_json::json!({"start": 100, "end": 500});
        assert_eq!(extract_range_from_params(&params), (100, 500));

        let params = serde_json::json!({"min_n": 1000, "max_n": 5000});
        assert_eq!(extract_range_from_params(&params), (1000, 5000));

        let params = serde_json::json!({"min_exp": 14000000, "max_exp": 20000000});
        assert_eq!(extract_range_from_params(&params), (14000000, 20000000));
    }
}
