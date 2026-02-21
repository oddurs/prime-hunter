//! # AI Strategy Engine — Autonomous Search Form Selection
//!
//! A pure Rust module that analyzes discovery data, scores all 12 search forms,
//! and automatically creates projects/search jobs to keep the node pool productive
//! without manual intervention.
//!
//! ## Pipeline
//!
//! Each strategy tick runs: **Survey → Score → Decide → Execute → Monitor**.
//!
//! 1. **Survey**: Gathers a snapshot of world records, fleet status, active jobs,
//!    form yield rates, cost calibrations, and idle capacity.
//! 2. **Score**: Ranks all 12 forms by a composite score (5 weighted components).
//! 3. **Decide**: Generates decisions (create project, pause job, verify result, or no-action).
//! 4. **Execute**: Logs decisions and executes them (project creation, job pausing, etc.).
//! 5. **Monitor**: Checks for stalled jobs, high failure rates, and near-record discoveries.
//!
//! ## Scoring Model
//!
//! | Component      | Weight | Source                                        |
//! |----------------|--------|-----------------------------------------------|
//! | Record gap     | 0.25   | `our_best_digits / record_digits`             |
//! | Yield rate     | 0.20   | `found / tested` from yield view              |
//! | Cost efficiency| 0.20   | `yield_rate / secs_per_candidate`              |
//! | Coverage gap   | 0.20   | Uncovered range / total searchable range       |
//! | Fleet fit      | 0.15   | Worker count vs form requirements              |
//!
//! Preferred forms get a 1.5× multiplier; excluded forms are zeroed.
//!
//! ## References
//!
//! - Cost model: [`project::cost`]
//! - Orchestration: [`project::orchestration`]
//! - Yield data: `form_yield_rates` SQL view (migration 027)

use anyhow::Result;
use serde::Serialize;
use tracing::{info, warn};

use crate::db::Database;

/// All 12 search forms the engine can score and schedule.
pub const ALL_FORMS: &[&str] = &[
    "factorial",
    "primorial",
    "kbn",
    "twin",
    "sophie_germain",
    "palindromic",
    "near_repdigit",
    "cullen_woodall",
    "carol_kynea",
    "wagstaff",
    "repunit",
    "gen_fermat",
];

/// Snapshot of the fleet/search state at tick time.
#[derive(Debug, Clone, Serialize)]
pub struct Survey {
    pub records: Vec<crate::project::RecordRow>,
    pub worker_count: u32,
    pub total_cores: u32,
    pub active_jobs: Vec<crate::db::SearchJobRow>,
    pub active_projects: Vec<crate::project::ProjectRow>,
    pub yield_rates: Vec<crate::db::FormYieldRateRow>,
    pub idle_workers: u32,
}

/// Score breakdown for a single form.
#[derive(Debug, Clone, Serialize)]
pub struct FormScore {
    pub form: String,
    /// Our best digits / world record digits (0.0 = no progress, 1.0 = at record).
    pub record_gap: f64,
    /// Historical yield: found / tested.
    pub yield_rate: f64,
    /// Cost efficiency: yield_rate / secs_per_candidate.
    pub cost_efficiency: f64,
    /// Fraction of searchable range not yet covered.
    pub coverage_gap: f64,
    /// How well the current fleet fits this form's requirements.
    pub fleet_fit: f64,
    /// Weighted composite score.
    pub total: f64,
}

/// A decision produced by the strategy engine.
#[derive(Debug, Clone, Serialize)]
pub struct StrategyDecision {
    pub decision_type: DecisionType,
    pub form: Option<String>,
    pub summary: String,
    pub reasoning: String,
    pub params: Option<serde_json::Value>,
    pub estimated_cost_usd: Option<f64>,
    pub scores: Option<serde_json::Value>,
}

/// Types of decisions the engine can make.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionType {
    CreateProject,
    CreateJob,
    PauseJob,
    VerifyResult,
    NoAction,
}

impl DecisionType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::CreateProject => "create_project",
            Self::CreateJob => "create_job",
            Self::PauseJob => "pause_job",
            Self::VerifyResult => "verify_result",
            Self::NoAction => "no_action",
        }
    }
}

/// Result of a strategy tick.
#[derive(Debug, Clone, Serialize)]
pub struct TickResult {
    pub decisions: Vec<StrategyDecision>,
    pub scores: Vec<FormScore>,
    pub survey: Survey,
}

// ── Scoring Weights ─────────────────────────────────────────────

const W_RECORD_GAP: f64 = 0.25;
const W_YIELD_RATE: f64 = 0.20;
const W_COST_EFFICIENCY: f64 = 0.20;
const W_COVERAGE_GAP: f64 = 0.20;
const W_FLEET_FIT: f64 = 0.15;

/// Preferred forms multiplier.
const PREFERRED_MULTIPLIER: f64 = 1.5;

/// Default searchable range per form (for coverage gap calculation).
/// These are rough upper bounds on the parameter space.
fn searchable_range(form: &str) -> u64 {
    match form {
        "factorial" | "primorial" => 1_000_000,
        "kbn" | "twin" | "sophie_germain" => 10_000_000,
        "palindromic" | "near_repdigit" => 50_000,
        "cullen_woodall" | "carol_kynea" => 5_000_000,
        "wagstaff" => 50_000_000,
        "repunit" => 1_000_000,
        "gen_fermat" => 1_000_000,
        _ => 1_000_000,
    }
}

/// Minimum cores recommended for a form (for fleet fit scoring).
fn min_cores_for_form(form: &str) -> u32 {
    match form {
        "wagstaff" | "repunit" => 16,
        "factorial" | "primorial" => 8,
        _ => 4,
    }
}

// ── Survey ──────────────────────────────────────────────────────

/// Gather a snapshot of the current fleet and search state.
pub async fn survey(db: &Database) -> Result<Survey> {
    let records = db.get_records().await.unwrap_or_default();
    let workers = db.get_all_workers().await.unwrap_or_default();
    let active_jobs = db.get_search_jobs().await.unwrap_or_default();
    let active_projects = db.get_projects(Some("active")).await.unwrap_or_default();
    let yield_rates = db.get_form_yield_rates().await.unwrap_or_default();

    let worker_count = workers.len() as u32;
    let total_cores: u32 = workers.iter().map(|w| w.cores as u32).sum();

    // Count idle workers: those whose search_type is empty or "idle"
    let busy_workers = active_jobs
        .iter()
        .filter(|j| j.status == "running")
        .count() as u32;
    let idle_workers = worker_count.saturating_sub(busy_workers.min(worker_count));

    Ok(Survey {
        records,
        worker_count,
        total_cores,
        active_jobs,
        active_projects,
        yield_rates,
        idle_workers,
    })
}

// ── Score ────────────────────────────────────────────────────────

/// Score all 12 forms against the current survey data.
pub fn score_forms(
    survey: &Survey,
    config: &crate::db::StrategyConfigRow,
) -> Vec<FormScore> {
    let mut scores: Vec<FormScore> = Vec::with_capacity(ALL_FORMS.len());

    for &form in ALL_FORMS {
        // Skip excluded forms entirely
        if config.excluded_forms.iter().any(|f| f == form) {
            scores.push(FormScore {
                form: form.to_string(),
                record_gap: 0.0,
                yield_rate: 0.0,
                cost_efficiency: 0.0,
                coverage_gap: 0.0,
                fleet_fit: 0.0,
                total: 0.0,
            });
            continue;
        }

        // 1. Record gap: how close are we to the world record?
        let record_gap = survey
            .records
            .iter()
            .find(|r| r.form == form)
            .map(|r| {
                if r.digits > 0 {
                    // Invert: higher score = more room to improve
                    1.0 - (r.our_best_digits as f64 / r.digits as f64).min(1.0)
                } else {
                    1.0 // No record known = maximum opportunity
                }
            })
            .unwrap_or(1.0);

        // 2. Yield rate: historical found/tested ratio
        let yr = survey
            .yield_rates
            .iter()
            .find(|y| y.form == form)
            .map(|y| y.yield_rate)
            .unwrap_or(0.001); // Default: assume low but non-zero yield
        // Normalize: log scale to handle wide range (1e-8 to 1e-1)
        let yield_score = ((yr * 1e6).ln().max(0.0) / 15.0).min(1.0);

        // 3. Cost efficiency: yield per compute second
        let digits_estimate = 1000u64; // baseline comparison point
        let spc = secs_per_candidate_estimate(form, digits_estimate);
        let cost_eff = if spc > 0.0 {
            ((yr / spc) * 1e8).ln().max(0.0) / 20.0
        } else {
            0.0
        };
        let cost_efficiency = cost_eff.min(1.0);

        // 4. Coverage gap: how much of the searchable range is uncovered?
        let max_searched = survey
            .yield_rates
            .iter()
            .find(|y| y.form == form)
            .map(|y| y.max_range_searched)
            .unwrap_or(0);
        let total_range = searchable_range(form);
        let coverage_gap = if total_range > 0 {
            1.0 - (max_searched as f64 / total_range as f64).min(1.0)
        } else {
            1.0
        };

        // 5. Fleet fit: can the current fleet handle this form?
        let min_cores = min_cores_for_form(form);
        let fleet_fit = if survey.total_cores >= min_cores {
            1.0
        } else if survey.total_cores > 0 {
            survey.total_cores as f64 / min_cores as f64
        } else {
            0.0
        };

        // Composite score
        let mut total = record_gap * W_RECORD_GAP
            + yield_score * W_YIELD_RATE
            + cost_efficiency * W_COST_EFFICIENCY
            + coverage_gap * W_COVERAGE_GAP
            + fleet_fit * W_FLEET_FIT;

        // Preferred forms multiplier
        if config.preferred_forms.iter().any(|f| f == form) {
            total *= PREFERRED_MULTIPLIER;
        }

        scores.push(FormScore {
            form: form.to_string(),
            record_gap,
            yield_rate: yield_score,
            cost_efficiency,
            coverage_gap,
            fleet_fit,
            total,
        });
    }

    // Sort by total score descending
    scores.sort_by(|a, b| b.total.partial_cmp(&a.total).unwrap_or(std::cmp::Ordering::Equal));
    scores
}

/// Simplified secs_per_candidate for scoring (no PFGW, baseline digits).
pub fn secs_per_candidate_estimate(form: &str, digits: u64) -> f64 {
    let d = (digits as f64) / 1000.0;
    match form {
        "factorial" | "primorial" => 0.5 * d.powf(2.5),
        "kbn" | "twin" | "sophie_germain" => 0.1 * d.powf(2.0),
        "cullen_woodall" | "carol_kynea" => 0.2 * d.powf(2.2),
        "wagstaff" => 0.8 * d.powf(2.5),
        "palindromic" | "near_repdigit" => 0.3 * d.powf(2.0),
        "repunit" => 0.4 * d.powf(2.3),
        "gen_fermat" => 0.3 * d.powf(2.2),
        _ => 0.5 * d.powf(2.5),
    }
}

// ── Decide ──────────────────────────────────────────────────────

/// Generate decisions based on survey data and form scores.
pub fn decide(
    survey: &Survey,
    scores: &[FormScore],
    config: &crate::db::StrategyConfigRow,
    monthly_spend: f64,
) -> Vec<StrategyDecision> {
    let mut decisions = Vec::new();

    // Monitor: check for stalled jobs
    for job in &survey.active_jobs {
        if job.status != "running" {
            continue;
        }
        // Check if job has stalled (no completions — we approximate by checking
        // if started_at is > 30 min ago and total_tested is still 0)
        if let Some(started_at) = job.started_at {
            let elapsed = (chrono::Utc::now() - started_at).num_seconds();
            if elapsed > 1800 && job.total_tested == 0 {
                decisions.push(StrategyDecision {
                    decision_type: DecisionType::PauseJob,
                    form: Some(job.search_type.clone()),
                    summary: format!("Pause stalled job #{} ({})", job.id, job.search_type),
                    reasoning: format!(
                        "Job {} has been running for {}s with 0 candidates tested. \
                         Likely stalled or misconfigured.",
                        job.id, elapsed
                    ),
                    params: Some(serde_json::json!({"job_id": job.id})),
                    estimated_cost_usd: None,
                    scores: None,
                });
            }
        }
    }

    // Monitor: check for near-record discoveries
    for record in &survey.records {
        if record.our_best_digits > 0 && record.digits > 0 {
            let proximity = record.our_best_digits as f64 / record.digits as f64;
            if proximity >= (1.0 - config.record_proximity_threshold) {
                // Check if there's already a verification project for this form
                let already_verifying = survey
                    .active_projects
                    .iter()
                    .any(|p| p.form == record.form && p.objective == "verification");
                if !already_verifying {
                    decisions.push(StrategyDecision {
                        decision_type: DecisionType::VerifyResult,
                        form: Some(record.form.clone()),
                        summary: format!(
                            "Verify near-record {} discovery ({} vs {} digit record)",
                            record.form, record.our_best_digits, record.digits
                        ),
                        reasoning: format!(
                            "Our best {} prime has {} digits, within {:.0}% of the \
                             {}-digit world record. Independent verification recommended.",
                            record.form,
                            record.our_best_digits,
                            (1.0 - proximity) * 100.0,
                            record.digits
                        ),
                        params: Some(serde_json::json!({
                            "form": record.form,
                            "our_best_digits": record.our_best_digits,
                            "record_digits": record.digits,
                        })),
                        estimated_cost_usd: None,
                        scores: None,
                    });
                }
            }
        }
    }

    // Create project: if idle workers >= threshold AND budget allows
    let active_project_count = survey.active_projects.len() as i32;
    let budget_remaining = config.max_monthly_budget_usd - monthly_spend;

    if survey.idle_workers >= config.min_idle_workers_to_create as u32
        && active_project_count < config.max_concurrent_projects
        && budget_remaining > config.max_per_project_budget_usd * 0.5
    {
        // Find the top-scoring form that doesn't have an active project
        let active_forms: Vec<&str> = survey
            .active_projects
            .iter()
            .map(|p| p.form.as_str())
            .collect();

        if let Some(best) = scores.iter().find(|s| {
            s.total > 0.0
                && !active_forms.contains(&s.form.as_str())
                && !config.excluded_forms.contains(&s.form)
        }) {
            let max_searched = survey
                .yield_rates
                .iter()
                .find(|y| y.form == best.form)
                .map(|y| y.max_range_searched)
                .unwrap_or(0);

            let budget = budget_remaining.min(config.max_per_project_budget_usd);

            decisions.push(StrategyDecision {
                decision_type: DecisionType::CreateProject,
                form: Some(best.form.clone()),
                summary: format!(
                    "Create {} project (score {:.3}, {} idle workers)",
                    best.form, best.total, survey.idle_workers
                ),
                reasoning: format!(
                    "Form '{}' scored highest ({:.3}) among forms without active projects. \
                     {} idle workers available, budget ${:.2} remaining this month. \
                     Score breakdown: record_gap={:.2}, yield={:.2}, cost_eff={:.2}, \
                     coverage={:.2}, fleet_fit={:.2}.",
                    best.form,
                    best.total,
                    survey.idle_workers,
                    budget_remaining,
                    best.record_gap,
                    best.yield_rate,
                    best.cost_efficiency,
                    best.coverage_gap,
                    best.fleet_fit,
                ),
                params: Some(serde_json::json!({
                    "form": best.form,
                    "continue_from": max_searched,
                    "budget_usd": budget,
                })),
                estimated_cost_usd: Some(budget),
                scores: serde_json::to_value(scores).ok(),
            });
        }
    }

    // If no actionable decisions, log a no-action
    if decisions.is_empty() {
        let reason = if survey.worker_count == 0 {
            "No workers connected".to_string()
        } else if survey.idle_workers < config.min_idle_workers_to_create as u32 {
            format!(
                "Only {} idle workers (need {})",
                survey.idle_workers, config.min_idle_workers_to_create
            )
        } else if active_project_count >= config.max_concurrent_projects {
            format!(
                "{} active projects (max {})",
                active_project_count, config.max_concurrent_projects
            )
        } else if budget_remaining <= 0.0 {
            "Monthly budget exhausted".to_string()
        } else {
            "No actionable conditions met".to_string()
        };

        decisions.push(StrategyDecision {
            decision_type: DecisionType::NoAction,
            form: None,
            summary: format!("No action: {}", reason),
            reasoning: format!(
                "Survey: {} workers ({} idle), {} active projects, ${:.2} budget remaining. {}",
                survey.worker_count,
                survey.idle_workers,
                active_project_count,
                budget_remaining,
                reason,
            ),
            params: None,
            estimated_cost_usd: None,
            scores: serde_json::to_value(scores).ok(),
        });
    }

    decisions
}

// ── Execute ─────────────────────────────────────────────────────

/// Execute a single strategy decision: log it and perform the action.
pub async fn execute_decision(
    db: &Database,
    decision: &StrategyDecision,
) -> Result<()> {
    let mut project_id = None;
    let mut search_job_id = None;

    match decision.decision_type {
        DecisionType::CreateProject => {
            if let Some(params) = &decision.params {
                let form = params["form"].as_str().unwrap_or("kbn");
                let continue_from = params["continue_from"].as_i64().unwrap_or(0);
                let budget = params["budget_usd"].as_f64().unwrap_or(25.0);

                // Build a ProjectConfig for this form
                let config = build_auto_project_config(form, continue_from, budget);
                match db.create_project(&config, None).await {
                    Ok(pid) => {
                        project_id = Some(pid);
                        // Activate the project
                        if let Err(e) = db.update_project_status(pid, "active").await {
                            warn!(project_id = pid, error = %e, "Strategy: failed to activate project");
                        }
                        info!(project_id = pid, form, "Strategy: created project");
                    }
                    Err(e) => {
                        warn!(form, error = %e, "Strategy: failed to create project");
                    }
                }
            }
        }
        DecisionType::PauseJob => {
            if let Some(params) = &decision.params {
                if let Some(job_id) = params["job_id"].as_i64() {
                    search_job_id = Some(job_id);
                    if let Err(e) = db.update_search_job_status(job_id, "paused", None).await {
                        warn!(job_id, error = %e, "Strategy: failed to pause job");
                    } else {
                        info!(job_id, "Strategy: paused stalled job");
                    }
                }
            }
        }
        DecisionType::VerifyResult | DecisionType::CreateJob | DecisionType::NoAction => {
            // VerifyResult: logged only (verification handled by auto-verify background task)
            // NoAction: logged only
        }
    }

    // Log the decision
    db.insert_strategy_decision(
        decision.decision_type.as_str(),
        decision.form.as_deref(),
        &decision.summary,
        &decision.reasoning,
        decision.params.as_ref(),
        decision.estimated_cost_usd,
        "executed",
        project_id,
        search_job_id,
        decision.scores.as_ref(),
    )
    .await?;

    Ok(())
}

/// Build a ProjectConfig for automatic project creation.
pub fn build_auto_project_config(
    form: &str,
    continue_from: i64,
    budget_usd: f64,
) -> crate::project::ProjectConfig {
    let range_start = if continue_from > 0 {
        continue_from as u64 + 1
    } else {
        default_range_start(form)
    };
    let range_end = range_start + default_range_size(form);

    crate::project::ProjectConfig {
        project: crate::project::ProjectMeta {
            name: format!("auto-{}-{}", form, range_start),
            description: format!(
                "Auto-generated by strategy engine: {} search from {} to {}",
                form, range_start, range_end
            ),
            objective: crate::project::Objective::Survey,
            form: form.to_string(),
            author: "strategy-engine".to_string(),
            tags: vec!["auto".to_string(), "strategy".to_string()],
        },
        target: crate::project::TargetConfig {
            target_digits: None,
            range_start: Some(range_start),
            range_end: Some(range_end),
        },
        competitive: None,
        strategy: crate::project::StrategyConfig {
            auto_strategy: true,
            phases: vec![],
        },
        infrastructure: None,
        budget: Some(crate::project::BudgetConfig {
            max_cost_usd: Some(budget_usd),
            cloud_rate_usd_per_core_hour: 0.04,
            cost_alert_threshold_usd: Some(budget_usd * 0.8),
        }),
        workers: None,
    }
}

/// Default starting range for each form.
fn default_range_start(form: &str) -> u64 {
    match form {
        "factorial" => 1,
        "primorial" => 1,
        "kbn" | "twin" | "sophie_germain" => 1,
        "palindromic" => 3,
        "near_repdigit" => 3,
        "cullen_woodall" | "carol_kynea" => 1,
        "wagstaff" => 3,
        "repunit" => 2,
        "gen_fermat" => 2,
        _ => 1,
    }
}

/// Default range size for auto-generated projects.
fn default_range_size(form: &str) -> u64 {
    match form {
        "factorial" | "primorial" => 10_000,
        "kbn" | "twin" | "sophie_germain" => 100_000,
        "palindromic" | "near_repdigit" => 2,
        "cullen_woodall" | "carol_kynea" => 50_000,
        "wagstaff" => 1_000_000,
        "repunit" => 10_000,
        "gen_fermat" => 100_000,
        _ => 10_000,
    }
}

// ── Public Entry Point ──────────────────────────────────────────

/// Run a single strategy tick: Survey → Score → Decide → Execute.
///
/// Called every `tick_interval_secs` (default 300s = 5 minutes) from the
/// dashboard background loop. Returns the tick result for WebSocket push.
pub async fn strategy_tick(db: &Database) -> Result<TickResult> {
    let config = db.get_strategy_config().await?;

    if !config.enabled {
        return Ok(TickResult {
            decisions: vec![],
            scores: vec![],
            survey: Survey {
                records: vec![],
                worker_count: 0,
                total_cores: 0,
                active_jobs: vec![],
                active_projects: vec![],
                yield_rates: vec![],
                idle_workers: 0,
            },
        });
    }

    let survey_data = survey(db).await?;
    let scores = score_forms(&survey_data, &config);
    let monthly_spend = db.get_monthly_strategy_spend().await.unwrap_or(0.0);
    let decisions = decide(&survey_data, &scores, &config, monthly_spend);

    // Execute each decision
    for decision in &decisions {
        if let Err(e) = execute_decision(db, decision).await {
            warn!(error = %e, "Strategy: failed to execute decision");
        }
    }

    Ok(TickResult {
        decisions,
        scores,
        survey: survey_data,
    })
}

/// Force an immediate strategy tick (for the manual trigger API endpoint).
pub async fn force_tick(db: &Database) -> Result<TickResult> {
    strategy_tick(db).await
}

/// Get current form scores without executing any decisions.
pub async fn get_current_scores(db: &Database) -> Result<Vec<FormScore>> {
    let config = db.get_strategy_config().await?;
    let survey_data = survey(db).await?;
    Ok(score_forms(&survey_data, &config))
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_forms_list_has_12_entries() {
        assert_eq!(ALL_FORMS.len(), 12);
    }

    #[test]
    fn decision_type_as_str() {
        assert_eq!(DecisionType::CreateProject.as_str(), "create_project");
        assert_eq!(DecisionType::CreateJob.as_str(), "create_job");
        assert_eq!(DecisionType::PauseJob.as_str(), "pause_job");
        assert_eq!(DecisionType::VerifyResult.as_str(), "verify_result");
        assert_eq!(DecisionType::NoAction.as_str(), "no_action");
    }

    #[test]
    fn secs_per_candidate_estimate_positive() {
        for &form in ALL_FORMS {
            let spc = secs_per_candidate_estimate(form, 1000);
            assert!(
                spc > 0.0,
                "secs_per_candidate_estimate({}, 1000) should be positive, got {}",
                form,
                spc
            );
        }
    }

    #[test]
    fn secs_per_candidate_estimate_scales_with_digits() {
        for &form in ALL_FORMS {
            let small = secs_per_candidate_estimate(form, 500);
            let large = secs_per_candidate_estimate(form, 5000);
            assert!(
                large > small,
                "secs_per_candidate_estimate({}) should increase with digits: {} vs {}",
                form,
                small,
                large
            );
        }
    }

    #[test]
    fn searchable_range_positive() {
        for &form in ALL_FORMS {
            assert!(
                searchable_range(form) > 0,
                "searchable_range({}) must be positive",
                form
            );
        }
    }

    #[test]
    fn min_cores_positive() {
        for &form in ALL_FORMS {
            assert!(
                min_cores_for_form(form) > 0,
                "min_cores_for_form({}) must be positive",
                form
            );
        }
    }

    #[test]
    fn default_range_start_and_size_sensible() {
        for &form in ALL_FORMS {
            let start = default_range_start(form);
            let size = default_range_size(form);
            assert!(size > 0, "default_range_size({}) must be positive", form);
            assert!(
                start + size > start,
                "default range must not overflow for {}",
                form
            );
        }
    }

    #[test]
    fn build_auto_project_config_valid() {
        let config = build_auto_project_config("kbn", 1000, 25.0);
        assert_eq!(config.project.form, "kbn");
        assert!(config.project.name.contains("kbn"));
        assert_eq!(config.target.range_start, Some(1001));
        assert!(config.strategy.auto_strategy);
        assert_eq!(
            config.budget.as_ref().unwrap().max_cost_usd,
            Some(25.0)
        );
    }

    #[test]
    fn build_auto_project_config_from_zero() {
        let config = build_auto_project_config("factorial", 0, 10.0);
        assert_eq!(config.target.range_start, Some(1)); // default_range_start
    }

    #[test]
    fn score_forms_with_empty_survey() {
        let survey = Survey {
            records: vec![],
            worker_count: 0,
            total_cores: 0,
            active_jobs: vec![],
            active_projects: vec![],
            yield_rates: vec![],
            idle_workers: 0,
        };
        let config = crate::db::StrategyConfigRow {
            id: 1,
            enabled: true,
            max_concurrent_projects: 3,
            max_monthly_budget_usd: 100.0,
            max_per_project_budget_usd: 25.0,
            preferred_forms: vec![],
            excluded_forms: vec![],
            min_idle_workers_to_create: 2,
            record_proximity_threshold: 0.1,
            tick_interval_secs: 300,
            updated_at: chrono::Utc::now(),
        };
        let scores = score_forms(&survey, &config);
        assert_eq!(scores.len(), 12);
        // All forms should have some score (coverage_gap = 1.0, record_gap = 1.0)
        for s in &scores {
            assert!(s.total >= 0.0, "Score for {} should be non-negative", s.form);
        }
    }

    #[test]
    fn score_forms_excludes_forms() {
        let survey = Survey {
            records: vec![],
            worker_count: 8,
            total_cores: 64,
            active_jobs: vec![],
            active_projects: vec![],
            yield_rates: vec![],
            idle_workers: 8,
        };
        let config = crate::db::StrategyConfigRow {
            id: 1,
            enabled: true,
            max_concurrent_projects: 3,
            max_monthly_budget_usd: 100.0,
            max_per_project_budget_usd: 25.0,
            preferred_forms: vec![],
            excluded_forms: vec!["wagstaff".to_string(), "repunit".to_string()],
            min_idle_workers_to_create: 2,
            record_proximity_threshold: 0.1,
            tick_interval_secs: 300,
            updated_at: chrono::Utc::now(),
        };
        let scores = score_forms(&survey, &config);
        let wagstaff = scores.iter().find(|s| s.form == "wagstaff").unwrap();
        assert_eq!(wagstaff.total, 0.0, "Excluded form should have zero score");
        let repunit = scores.iter().find(|s| s.form == "repunit").unwrap();
        assert_eq!(repunit.total, 0.0, "Excluded form should have zero score");
    }

    #[test]
    fn score_forms_prefers_forms() {
        let survey = Survey {
            records: vec![],
            worker_count: 8,
            total_cores: 64,
            active_jobs: vec![],
            active_projects: vec![],
            yield_rates: vec![],
            idle_workers: 8,
        };
        let config_no_pref = crate::db::StrategyConfigRow {
            id: 1,
            enabled: true,
            max_concurrent_projects: 3,
            max_monthly_budget_usd: 100.0,
            max_per_project_budget_usd: 25.0,
            preferred_forms: vec![],
            excluded_forms: vec![],
            min_idle_workers_to_create: 2,
            record_proximity_threshold: 0.1,
            tick_interval_secs: 300,
            updated_at: chrono::Utc::now(),
        };
        let config_with_pref = crate::db::StrategyConfigRow {
            preferred_forms: vec!["factorial".to_string()],
            ..config_no_pref.clone()
        };

        let scores_no = score_forms(&survey, &config_no_pref);
        let scores_yes = score_forms(&survey, &config_with_pref);

        let fac_no = scores_no.iter().find(|s| s.form == "factorial").unwrap();
        let fac_yes = scores_yes.iter().find(|s| s.form == "factorial").unwrap();
        assert!(
            fac_yes.total > fac_no.total,
            "Preferred form should have higher score: {} vs {}",
            fac_yes.total,
            fac_no.total,
        );
    }

    #[test]
    fn decide_no_action_when_no_workers() {
        let survey = Survey {
            records: vec![],
            worker_count: 0,
            total_cores: 0,
            active_jobs: vec![],
            active_projects: vec![],
            yield_rates: vec![],
            idle_workers: 0,
        };
        let config = crate::db::StrategyConfigRow {
            id: 1,
            enabled: true,
            max_concurrent_projects: 3,
            max_monthly_budget_usd: 100.0,
            max_per_project_budget_usd: 25.0,
            preferred_forms: vec![],
            excluded_forms: vec![],
            min_idle_workers_to_create: 2,
            record_proximity_threshold: 0.1,
            tick_interval_secs: 300,
            updated_at: chrono::Utc::now(),
        };
        let scores = score_forms(&survey, &config);
        let decisions = decide(&survey, &scores, &config, 0.0);
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].decision_type, DecisionType::NoAction);
        assert!(decisions[0].summary.contains("No action"));
    }

    #[test]
    fn decide_no_action_when_budget_exhausted() {
        let survey = Survey {
            records: vec![],
            worker_count: 4,
            total_cores: 32,
            active_jobs: vec![],
            active_projects: vec![],
            yield_rates: vec![],
            idle_workers: 4,
        };
        let config = crate::db::StrategyConfigRow {
            id: 1,
            enabled: true,
            max_concurrent_projects: 3,
            max_monthly_budget_usd: 100.0,
            max_per_project_budget_usd: 25.0,
            preferred_forms: vec![],
            excluded_forms: vec![],
            min_idle_workers_to_create: 2,
            record_proximity_threshold: 0.1,
            tick_interval_secs: 300,
            updated_at: chrono::Utc::now(),
        };
        let scores = score_forms(&survey, &config);
        // Monthly spend exceeds budget
        let decisions = decide(&survey, &scores, &config, 100.0);
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].decision_type, DecisionType::NoAction);
    }

    #[test]
    fn decide_creates_project_when_idle_workers() {
        let survey = Survey {
            records: vec![],
            worker_count: 4,
            total_cores: 32,
            active_jobs: vec![],
            active_projects: vec![],
            yield_rates: vec![],
            idle_workers: 4,
        };
        let config = crate::db::StrategyConfigRow {
            id: 1,
            enabled: true,
            max_concurrent_projects: 3,
            max_monthly_budget_usd: 100.0,
            max_per_project_budget_usd: 25.0,
            preferred_forms: vec![],
            excluded_forms: vec![],
            min_idle_workers_to_create: 2,
            record_proximity_threshold: 0.1,
            tick_interval_secs: 300,
            updated_at: chrono::Utc::now(),
        };
        let scores = score_forms(&survey, &config);
        let decisions = decide(&survey, &scores, &config, 0.0);
        assert!(
            decisions
                .iter()
                .any(|d| d.decision_type == DecisionType::CreateProject),
            "Should create a project when idle workers available"
        );
    }

    #[test]
    fn scores_are_sorted_descending() {
        let survey = Survey {
            records: vec![],
            worker_count: 8,
            total_cores: 64,
            active_jobs: vec![],
            active_projects: vec![],
            yield_rates: vec![],
            idle_workers: 8,
        };
        let config = crate::db::StrategyConfigRow {
            id: 1,
            enabled: true,
            max_concurrent_projects: 3,
            max_monthly_budget_usd: 100.0,
            max_per_project_budget_usd: 25.0,
            preferred_forms: vec![],
            excluded_forms: vec![],
            min_idle_workers_to_create: 2,
            record_proximity_threshold: 0.1,
            tick_interval_secs: 300,
            updated_at: chrono::Utc::now(),
        };
        let scores = score_forms(&survey, &config);
        for w in scores.windows(2) {
            assert!(
                w[0].total >= w[1].total,
                "Scores not sorted: {} ({}) >= {} ({})",
                w[0].form,
                w[0].total,
                w[1].form,
                w[1].total
            );
        }
    }
}
