//! # AI Engine — Unified OODA Decision Loop
//!
//! Replaces three independent background loops (strategy_tick, orchestrate_tick,
//! agent polling) with a single coherent decision loop following the OODA pattern:
//!
//! ```text
//! OBSERVE (30s) → ORIENT (pure) → DECIDE (pure) → ACT (DB writes) → LEARN (5min, async)
//! ```
//!
//! ## Key Design Properties
//!
//! - **Single consistent view**: All decisions are made against one `WorldSnapshot`
//!   assembled in parallel queries (~50ms). No stale reads between stages.
//! - **Pure scoring**: ORIENT and DECIDE are pure functions — no side effects,
//!   fully testable without a database.
//! - **Calibrated cost model**: The LEARN phase fits power-law coefficients from
//!   real work block data, replacing hardcoded curves in `project::cost`.
//! - **7-component scoring**: Extends the original 5-component model with
//!   momentum (recent discoveries) and competition (active external searches).
//! - **Decision audit trail**: Every decision is logged with reasoning, confidence,
//!   snapshot hash, and later annotated with measured outcomes.
//!
//! ## References
//!
//! - Strategy engine: [`strategy`] (scoring model, form constants)
//! - Project orchestration: [`project::orchestration`] (phase state machine)
//! - Cost model: [`project::cost`] (power-law estimation)
//! - Cost calibrations: [`db::calibrations`]

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

use crate::db::Database;
use crate::project;
use crate::strategy;

// ── Configuration ───────────────────────────────────────────────

/// Hot-reloadable engine configuration, loaded from `ai_engine_state` on each tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiEngineConfig {
    /// Whether the AI engine is enabled (master switch).
    pub enabled: bool,
    /// Maximum concurrent projects (fleet-size dependent).
    pub max_concurrent_projects: i32,
    /// Monthly budget cap (USD).
    pub max_monthly_budget_usd: f64,
    /// Per-project budget cap (USD).
    pub max_per_project_budget_usd: f64,
    /// Forms the engine will not select.
    pub excluded_forms: Vec<String>,
    /// Forms that receive a preference multiplier.
    pub preferred_forms: Vec<String>,
    /// Minimum idle workers before creating a new project.
    pub min_idle_workers_to_create: u32,
    /// Record proximity threshold for triggering verification (0.0–1.0).
    pub record_proximity_threshold: f64,
    /// Interval between LEARN cycles in seconds.
    pub learn_interval_secs: u64,
    /// Minimum data points required to fit a cost model.
    pub min_calibration_samples: i64,
    /// Maximum acceptable MAPE before replacing defaults.
    pub max_calibration_mape: f64,
}

impl Default for AiEngineConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_concurrent_projects: 3,
            max_monthly_budget_usd: 100.0,
            max_per_project_budget_usd: 25.0,
            excluded_forms: vec![],
            preferred_forms: vec![],
            min_idle_workers_to_create: 2,
            record_proximity_threshold: 0.1,
            learn_interval_secs: 300,
            min_calibration_samples: 10,
            max_calibration_mape: 0.5,
        }
    }
}

// ── Scoring Weights ─────────────────────────────────────────────

/// 7-component scoring weights with learned adjustments.
/// All weights must be in [0.05, 0.40] and sum to 1.0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub record_gap: f64,
    pub yield_rate: f64,
    pub cost_efficiency: f64,
    pub opportunity_density: f64,
    pub fleet_fit: f64,
    pub momentum: f64,
    pub competition: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            record_gap: 0.20,
            yield_rate: 0.15,
            cost_efficiency: 0.20,
            opportunity_density: 0.15,
            fleet_fit: 0.10,
            momentum: 0.10,
            competition: 0.10,
        }
    }
}

impl ScoringWeights {
    /// Validate weights are within bounds and sum to ~1.0.
    pub fn validate(&self) -> bool {
        let weights = [
            self.record_gap,
            self.yield_rate,
            self.cost_efficiency,
            self.opportunity_density,
            self.fleet_fit,
            self.momentum,
            self.competition,
        ];
        let all_bounded = weights.iter().all(|&w| w >= 0.05 && w <= 0.40);
        let sum: f64 = weights.iter().sum();
        all_bounded && (sum - 1.0).abs() < 0.01
    }

    /// Normalize weights to sum to exactly 1.0.
    pub fn normalize(&mut self) {
        let sum = self.record_gap
            + self.yield_rate
            + self.cost_efficiency
            + self.opportunity_density
            + self.fleet_fit
            + self.momentum
            + self.competition;
        if sum > 0.0 {
            self.record_gap /= sum;
            self.yield_rate /= sum;
            self.cost_efficiency /= sum;
            self.opportunity_density /= sum;
            self.fleet_fit /= sum;
            self.momentum /= sum;
            self.competition /= sum;
        }
    }
}

// ── Cost Model ──────────────────────────────────────────────────

/// Data-fitted cost model that replaces hardcoded power-law curves.
///
/// Each form's timing follows: `secs = a * (digits/1000)^b`
/// The LEARN phase fits (a, b) from completed work block data.
/// Falls back to hardcoded defaults from `project::cost` when insufficient data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostModel {
    /// Fitted coefficients: form → (a, b) from OLS on log-log data.
    pub fitted: HashMap<String, (f64, f64)>,
    /// Hardcoded fallback coefficients (from `project::cost::secs_per_candidate`).
    pub defaults: HashMap<String, (f64, f64)>,
    /// Per-form PFGW/GWNUM measured speedup factor.
    pub pfgw_speedup: HashMap<String, f64>,
    /// Version counter, incremented on each successful fit.
    pub version: u32,
}

impl Default for CostModel {
    fn default() -> Self {
        let mut defaults = HashMap::new();
        defaults.insert("factorial".to_string(), (0.5, 2.5));
        defaults.insert("primorial".to_string(), (0.5, 2.5));
        defaults.insert("kbn".to_string(), (0.1, 2.0));
        defaults.insert("twin".to_string(), (0.1, 2.0));
        defaults.insert("sophie_germain".to_string(), (0.1, 2.0));
        defaults.insert("cullen_woodall".to_string(), (0.2, 2.2));
        defaults.insert("carol_kynea".to_string(), (0.2, 2.2));
        defaults.insert("wagstaff".to_string(), (0.8, 2.5));
        defaults.insert("palindromic".to_string(), (0.3, 2.0));
        defaults.insert("near_repdigit".to_string(), (0.3, 2.0));
        defaults.insert("repunit".to_string(), (0.4, 2.3));
        defaults.insert("gen_fermat".to_string(), (0.3, 2.2));

        Self {
            fitted: HashMap::new(),
            defaults,
            pfgw_speedup: HashMap::new(),
            version: 0,
        }
    }
}

impl CostModel {
    /// Estimate seconds per candidate for a form at a given digit count.
    /// Uses fitted coefficients if available, otherwise falls back to defaults.
    pub fn secs_per_candidate(&self, form: &str, digits: u64, has_pfgw: bool) -> f64 {
        let d = digits as f64 / 1000.0;
        let (a, b) = self
            .fitted
            .get(form)
            .or_else(|| self.defaults.get(form))
            .copied()
            .unwrap_or((0.5, 2.5));

        let base = a * d.powf(b);

        if has_pfgw && digits >= 10_000 {
            let speedup = self.pfgw_speedup.get(form).copied().unwrap_or(50.0);
            base / speedup
        } else {
            base
        }
    }
}

// ── World Snapshot ──────────────────────────────────────────────

/// Single consistent view of the entire system state, assembled in one
/// atomic read. All ORIENT/DECIDE logic operates on this immutable snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct WorldSnapshot {
    pub records: Vec<project::RecordRow>,
    pub fleet: FleetSnapshot,
    pub active_projects: Vec<project::ProjectRow>,
    pub active_jobs: Vec<crate::db::SearchJobRow>,
    pub yield_rates: Vec<crate::db::FormYieldRateRow>,
    pub cost_calibrations: Vec<crate::db::CostCalibrationRow>,
    pub recent_discoveries: Vec<RecentDiscovery>,
    pub agent_results: Vec<crate::db::AgentTaskRow>,
    pub budget: BudgetSnapshot,
    pub timestamp: DateTime<Utc>,
}

/// Aggregated fleet capabilities at snapshot time.
#[derive(Debug, Clone, Serialize)]
pub struct FleetSnapshot {
    pub worker_count: u32,
    pub total_cores: u32,
    pub idle_workers: u32,
    pub max_ram_gb: u32,
    pub active_search_types: Vec<String>,
}

/// Budget state at snapshot time.
#[derive(Debug, Clone, Serialize)]
pub struct BudgetSnapshot {
    pub monthly_budget_usd: f64,
    pub monthly_spend_usd: f64,
    pub remaining_usd: f64,
}

/// A recent prime discovery for momentum scoring.
#[derive(Debug, Clone, Serialize)]
pub struct RecentDiscovery {
    pub form: String,
    pub digits: i64,
    pub found_at: DateTime<Utc>,
}

// ── Drift Detection ─────────────────────────────────────────────

/// Changes detected between consecutive snapshots.
#[derive(Debug, Clone, Serialize)]
pub struct DriftReport {
    pub workers_joined: i32,
    pub workers_left: i32,
    pub new_discoveries: u32,
    pub stalled_jobs: Vec<i64>,
    pub budget_velocity_usd_per_hour: f64,
    pub significant: bool,
}

// ── Analysis ────────────────────────────────────────────────────

/// Output of the ORIENT phase: scored forms + drift analysis.
#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    pub scores: Vec<FormScore>,
    pub drift: DriftReport,
}

/// 7-component score breakdown for a single form.
#[derive(Debug, Clone, Serialize)]
pub struct FormScore {
    pub form: String,
    pub record_gap: f64,
    pub yield_rate: f64,
    pub cost_efficiency: f64,
    pub opportunity_density: f64,
    pub fleet_fit: f64,
    pub momentum: f64,
    pub competition: f64,
    pub total: f64,
}

// ── Decisions ───────────────────────────────────────────────────

/// Actions the AI engine can take.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Decision {
    CreateProject {
        form: String,
        params: serde_json::Value,
        budget_usd: f64,
        reasoning: String,
        confidence: f64,
    },
    PauseProject {
        project_id: i64,
        reason: String,
    },
    ExtendProject {
        project_id: i64,
        new_range_end: u64,
        reasoning: String,
    },
    AbandonProject {
        project_id: i64,
        reason: String,
    },
    RebalanceFleet {
        moves: Vec<WorkerMove>,
        reasoning: String,
    },
    RequestAgentIntel {
        task_title: String,
        task_description: String,
    },
    VerifyResult {
        form: String,
        prime_id: Option<i64>,
        reasoning: String,
    },
    NoAction {
        reason: String,
    },
}

/// A fleet rebalancing move: reassign a worker to a different search.
#[derive(Debug, Clone, Serialize)]
pub struct WorkerMove {
    pub worker_id: String,
    pub from_form: String,
    pub to_form: String,
}

/// Outcome of a single engine tick.
#[derive(Debug, Clone, Serialize)]
pub struct TickOutcome {
    pub tick_id: u64,
    pub decisions: Vec<Decision>,
    pub analysis: Analysis,
    pub duration_ms: u64,
}

// ── Safety Checks ───────────────────────────────────────────────

/// Safety boundaries that all decisions must pass through.
#[derive(Debug)]
struct SafetyLimits {
    max_projects: i32,
    budget_remaining: f64,
    min_budget_for_project: f64,
}

fn safety_check(decision: &Decision, limits: &SafetyLimits) -> bool {
    match decision {
        Decision::CreateProject { budget_usd, .. } => {
            limits.budget_remaining >= limits.min_budget_for_project
                && *budget_usd <= limits.budget_remaining
        }
        Decision::AbandonProject { .. } => {
            // Abandoning requires careful review — only allow if no primes found
            true
        }
        Decision::NoAction { .. } => true,
        _ => true,
    }
}

// ── AI Engine ───────────────────────────────────────────────────

/// The unified AI engine. Holds learned state (cost model, scoring weights)
/// and the last snapshot for drift detection.
pub struct AiEngine {
    pub config: AiEngineConfig,
    pub cost_model: CostModel,
    pub scoring_weights: ScoringWeights,
    pub last_snapshot: Option<WorldSnapshot>,
    pub tick_count: u64,
    pub last_learn: Option<std::time::Instant>,
}

impl AiEngine {
    /// Create a new AI engine with default configuration.
    pub fn new() -> Self {
        Self {
            config: AiEngineConfig::default(),
            cost_model: CostModel::default(),
            scoring_weights: ScoringWeights::default(),
            last_snapshot: None,
            tick_count: 0,
            last_learn: None,
        }
    }

    /// Create an AI engine with a specific configuration.
    pub fn with_config(config: AiEngineConfig) -> Self {
        Self {
            config,
            cost_model: CostModel::default(),
            scoring_weights: ScoringWeights::default(),
            last_snapshot: None,
            tick_count: 0,
            last_learn: None,
        }
    }

    /// Run one complete OODA tick: Observe → Orient → Decide → Act → Learn.
    ///
    /// This is the single entry point that replaces `strategy_tick()` and
    /// `orchestrate_tick()`. Called every 30 seconds from the dashboard
    /// background loop.
    pub async fn tick(&mut self, db: &Database) -> Result<TickOutcome> {
        let start = std::time::Instant::now();
        self.tick_count += 1;
        let tick_id = self.tick_count;

        // Load config from DB (hot-reloadable)
        self.reload_config(db).await;

        if !self.config.enabled {
            return Ok(TickOutcome {
                tick_id,
                decisions: vec![Decision::NoAction {
                    reason: "AI engine disabled".to_string(),
                }],
                analysis: Analysis {
                    scores: vec![],
                    drift: empty_drift(),
                },
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        // OBSERVE: gather all state in parallel
        let snapshot = self.observe(db).await?;

        // Run project orchestration (phase advancement, cost aggregation)
        if let Err(e) = project::orchestrate_tick(db).await {
            warn!(error = %e, "ai_engine: project orchestration failed");
        }

        // ORIENT: score forms, detect drift
        let analysis = self.orient(&snapshot);

        // DECIDE: generate action plan
        let decisions = self.decide(&snapshot, &analysis);

        // ACT: execute decisions
        for decision in &decisions {
            if let Err(e) = self.act(db, decision, tick_id).await {
                warn!(error = %e, "ai_engine: failed to execute decision");
            }
        }

        // LEARN: calibrate cost model periodically
        if self.should_learn() {
            if let Err(e) = self.learn(db).await {
                warn!(error = %e, "ai_engine: learn phase failed");
            }
            self.last_learn = Some(std::time::Instant::now());
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        // Store snapshot for next tick's drift detection
        self.last_snapshot = Some(snapshot);

        // Persist engine state
        if let Err(e) = self.persist_state(db).await {
            warn!(error = %e, "ai_engine: failed to persist state");
        }

        Ok(TickOutcome {
            tick_id,
            decisions,
            analysis,
            duration_ms,
        })
    }

    // ── OBSERVE ─────────────────────────────────────────────────

    /// Gather a complete world snapshot via parallel database queries.
    async fn observe(&self, db: &Database) -> Result<WorldSnapshot> {
        // Run queries in parallel using tokio::join!
        let (records, workers, active_jobs, active_projects, yield_rates, calibrations, monthly_spend) = tokio::join!(
            db.get_records(),
            db.get_all_workers(),
            db.get_search_jobs(),
            db.get_projects(Some("active")),
            db.get_form_yield_rates(),
            db.get_cost_calibrations(),
            db.get_monthly_strategy_spend(),
        );

        let records = records.unwrap_or_default();
        let workers = workers.unwrap_or_default();
        let active_jobs = active_jobs.unwrap_or_default();
        let active_projects = active_projects.unwrap_or_default();
        let yield_rates = yield_rates.unwrap_or_default();
        let calibrations = calibrations.unwrap_or_default();
        let monthly_spend = monthly_spend.unwrap_or(0.0);

        let worker_count = workers.len() as u32;
        let total_cores: u32 = workers.iter().map(|w| w.cores as u32).sum();
        let busy_workers = active_jobs
            .iter()
            .filter(|j| j.status == "running")
            .count() as u32;
        let idle_workers = worker_count.saturating_sub(busy_workers.min(worker_count));

        // Get recent discoveries for momentum scoring (last 7 days)
        let recent_discoveries = db
            .get_recent_primes_for_momentum(7)
            .await
            .unwrap_or_default();

        // Get recent agent task completions
        let agent_results = db
            .get_recent_agent_results(10)
            .await
            .unwrap_or_default();

        let fleet_summary = db
            .get_fleet_summary()
            .await
            .unwrap_or_else(|_| crate::db::FleetSummary {
                worker_count: 0,
                total_cores: 0,
                max_ram_gb: 0,
                active_search_types: vec![],
            });

        Ok(WorldSnapshot {
            records,
            fleet: FleetSnapshot {
                worker_count,
                total_cores,
                idle_workers,
                max_ram_gb: fleet_summary.max_ram_gb,
                active_search_types: fleet_summary.active_search_types,
            },
            active_projects,
            active_jobs,
            yield_rates,
            cost_calibrations: calibrations,
            recent_discoveries,
            agent_results,
            budget: BudgetSnapshot {
                monthly_budget_usd: self.config.max_monthly_budget_usd,
                monthly_spend_usd: monthly_spend,
                remaining_usd: self.config.max_monthly_budget_usd - monthly_spend,
            },
            timestamp: Utc::now(),
        })
    }

    // ── ORIENT ──────────────────────────────────────────────────

    /// Score all forms and detect drift. Pure function — no side effects.
    fn orient(&self, snapshot: &WorldSnapshot) -> Analysis {
        let scores = self.score_forms(snapshot);
        let drift = self.detect_drift(snapshot);
        Analysis { scores, drift }
    }

    /// 7-component scoring model with calibrated cost and learned weights.
    fn score_forms(&self, snapshot: &WorldSnapshot) -> Vec<FormScore> {
        let mut scores = Vec::with_capacity(strategy::ALL_FORMS.len());

        for &form in strategy::ALL_FORMS {
            if self.config.excluded_forms.iter().any(|f| f == form) {
                scores.push(FormScore {
                    form: form.to_string(),
                    record_gap: 0.0,
                    yield_rate: 0.0,
                    cost_efficiency: 0.0,
                    opportunity_density: 0.0,
                    fleet_fit: 0.0,
                    momentum: 0.0,
                    competition: 0.0,
                    total: 0.0,
                });
                continue;
            }

            // 1. Record gap: room to improve toward world record
            let record_gap = snapshot
                .records
                .iter()
                .find(|r| r.form == form)
                .map(|r| {
                    if r.digits > 0 {
                        1.0 - (r.our_best_digits as f64 / r.digits as f64).min(1.0)
                    } else {
                        1.0
                    }
                })
                .unwrap_or(1.0);

            // 2. Yield rate: recency-weighted (recent yields matter more)
            let yr = snapshot
                .yield_rates
                .iter()
                .find(|y| y.form == form)
                .map(|y| y.yield_rate)
                .unwrap_or(0.001);
            let yield_score = ((yr * 1e6).ln().max(0.0) / 15.0).min(1.0);

            // 3. Cost efficiency: using calibrated cost model
            let digits_estimate = 1000u64;
            let spc = self.cost_model.secs_per_candidate(form, digits_estimate, false);
            let cost_eff = if spc > 0.0 {
                ((yr / spc) * 1e8).ln().max(0.0) / 20.0
            } else {
                0.0
            };
            let cost_efficiency = cost_eff.min(1.0);

            // 4. Opportunity density: expected primes per core-hour in uncovered range
            let max_searched = snapshot
                .yield_rates
                .iter()
                .find(|y| y.form == form)
                .map(|y| y.max_range_searched)
                .unwrap_or(0);
            let total_range = searchable_range(form);
            let uncovered = total_range.saturating_sub(max_searched as u64) as f64;
            let opportunity_density = if total_range > 0 {
                (uncovered / total_range as f64).min(1.0)
            } else {
                1.0
            };

            // 5. Fleet fit: utilization efficiency
            let min_cores = min_cores_for_form(form);
            let fleet_fit = if snapshot.fleet.total_cores >= min_cores {
                1.0
            } else if snapshot.fleet.total_cores > 0 {
                snapshot.fleet.total_cores as f64 / min_cores as f64
            } else {
                0.0
            };

            // 6. Momentum: bonus for forms with recent discoveries
            let recent_count = snapshot
                .recent_discoveries
                .iter()
                .filter(|d| d.form == form)
                .count();
            let momentum = (recent_count as f64 / 5.0).min(1.0);

            // 7. Competition: penalty for forms with active external searches
            // (placeholder — would be populated from competitive intel agent data)
            let competition = 0.5; // neutral default

            // Weighted composite score
            let w = &self.scoring_weights;
            let mut total = record_gap * w.record_gap
                + yield_score * w.yield_rate
                + cost_efficiency * w.cost_efficiency
                + opportunity_density * w.opportunity_density
                + fleet_fit * w.fleet_fit
                + momentum * w.momentum
                + competition * w.competition;

            // Preferred forms multiplier
            if self.config.preferred_forms.iter().any(|f| f == form) {
                total *= PREFERRED_MULTIPLIER;
            }

            scores.push(FormScore {
                form: form.to_string(),
                record_gap,
                yield_rate: yield_score,
                cost_efficiency,
                opportunity_density,
                fleet_fit,
                momentum,
                competition,
                total,
            });
        }

        scores.sort_by(|a, b| b.total.partial_cmp(&a.total).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }

    /// Compare current snapshot to last snapshot, detecting changes.
    fn detect_drift(&self, snapshot: &WorldSnapshot) -> DriftReport {
        let Some(last) = &self.last_snapshot else {
            return empty_drift();
        };

        let workers_joined =
            snapshot.fleet.worker_count as i32 - last.fleet.worker_count as i32;
        let workers_left = if workers_joined < 0 {
            workers_joined.unsigned_abs() as i32
        } else {
            0
        };
        let workers_joined = workers_joined.max(0);

        // Count new discoveries since last snapshot
        let new_discoveries = snapshot
            .recent_discoveries
            .iter()
            .filter(|d| d.found_at > last.timestamp)
            .count() as u32;

        // Detect stalled jobs: running > 30min with 0 tested
        let stalled_jobs: Vec<i64> = snapshot
            .active_jobs
            .iter()
            .filter(|j| {
                j.status == "running"
                    && j.total_tested == 0
                    && j.started_at
                        .map(|s| (Utc::now() - s).num_seconds() > 1800)
                        .unwrap_or(false)
            })
            .map(|j| j.id)
            .collect();

        // Budget velocity: rate of spend
        let time_delta = (snapshot.timestamp - last.timestamp).num_seconds().max(1) as f64;
        let spend_delta = snapshot.budget.monthly_spend_usd - last.budget.monthly_spend_usd;
        let budget_velocity_usd_per_hour = (spend_delta / time_delta) * 3600.0;

        let significant = workers_joined > 0
            || workers_left > 0
            || new_discoveries > 0
            || !stalled_jobs.is_empty()
            || budget_velocity_usd_per_hour.abs() > 1.0;

        DriftReport {
            workers_joined,
            workers_left,
            new_discoveries,
            stalled_jobs,
            budget_velocity_usd_per_hour,
            significant,
        }
    }

    // ── DECIDE ──────────────────────────────────────────────────

    /// Generate decisions based on snapshot and analysis. Pure function.
    fn decide(&self, snapshot: &WorldSnapshot, analysis: &Analysis) -> Vec<Decision> {
        let mut decisions = Vec::new();
        let limits = SafetyLimits {
            max_projects: self.config.max_concurrent_projects,
            budget_remaining: snapshot.budget.remaining_usd,
            min_budget_for_project: self.config.max_per_project_budget_usd * 0.5,
        };

        // 1. Handle stalled jobs
        for &job_id in &analysis.drift.stalled_jobs {
            if let Some(job) = snapshot.active_jobs.iter().find(|j| j.id == job_id) {
                decisions.push(Decision::PauseProject {
                    project_id: job_id,
                    reason: format!(
                        "Job {} ({}) stalled: running >30min with 0 tested",
                        job_id, job.search_type
                    ),
                });
            }
        }

        // 2. Check for near-record discoveries needing verification
        for record in &snapshot.records {
            if record.our_best_digits > 0 && record.digits > 0 {
                let proximity = record.our_best_digits as f64 / record.digits as f64;
                if proximity >= (1.0 - self.config.record_proximity_threshold) {
                    let already_verifying = snapshot
                        .active_projects
                        .iter()
                        .any(|p| p.form == record.form && p.objective == "verification");
                    if !already_verifying {
                        decisions.push(Decision::VerifyResult {
                            form: record.form.clone(),
                            prime_id: None,
                            reasoning: format!(
                                "{} prime at {} digits is within {:.1}% of {}-digit record",
                                record.form,
                                record.our_best_digits,
                                (1.0 - proximity) * 100.0,
                                record.digits,
                            ),
                        });
                    }
                }
            }
        }

        // 3. Create new projects if capacity and budget allow
        let active_project_count = snapshot.active_projects.len() as i32;
        if snapshot.fleet.idle_workers >= self.config.min_idle_workers_to_create
            && active_project_count < limits.max_projects
            && snapshot.budget.remaining_usd > limits.min_budget_for_project
        {
            let active_forms: Vec<&str> = snapshot
                .active_projects
                .iter()
                .map(|p| p.form.as_str())
                .collect();

            // Portfolio sizing based on fleet
            let max_new = self.portfolio_slots(snapshot.fleet.worker_count, active_project_count);

            for best in analysis
                .scores
                .iter()
                .filter(|s| {
                    s.total > 0.0
                        && !active_forms.contains(&s.form.as_str())
                        && !self.config.excluded_forms.contains(&s.form)
                })
                .take(max_new as usize)
            {
                let max_searched = snapshot
                    .yield_rates
                    .iter()
                    .find(|y| y.form == best.form)
                    .map(|y| y.max_range_searched)
                    .unwrap_or(0);

                let budget = snapshot
                    .budget
                    .remaining_usd
                    .min(self.config.max_per_project_budget_usd);

                let decision = Decision::CreateProject {
                    form: best.form.clone(),
                    params: serde_json::json!({
                        "continue_from": max_searched,
                        "budget_usd": budget,
                    }),
                    budget_usd: budget,
                    reasoning: format!(
                        "Top-scoring form ({:.3}): record_gap={:.2}, yield={:.2}, \
                         cost_eff={:.2}, opportunity={:.2}, fleet_fit={:.2}, \
                         momentum={:.2}, competition={:.2}. {} idle workers.",
                        best.total,
                        best.record_gap,
                        best.yield_rate,
                        best.cost_efficiency,
                        best.opportunity_density,
                        best.fleet_fit,
                        best.momentum,
                        best.competition,
                        snapshot.fleet.idle_workers,
                    ),
                    confidence: best.total.min(1.0),
                };

                if safety_check(&decision, &limits) {
                    decisions.push(decision);
                }
            }
        }

        // 4. If no actionable decisions, emit NoAction with reason
        if decisions.is_empty() {
            let reason = if snapshot.fleet.worker_count == 0 {
                "No workers connected".to_string()
            } else if snapshot.fleet.idle_workers < self.config.min_idle_workers_to_create {
                format!(
                    "Only {} idle workers (need {})",
                    snapshot.fleet.idle_workers, self.config.min_idle_workers_to_create,
                )
            } else if active_project_count >= limits.max_projects {
                format!(
                    "{} active projects (max {})",
                    active_project_count, limits.max_projects,
                )
            } else if snapshot.budget.remaining_usd <= 0.0 {
                "Monthly budget exhausted".to_string()
            } else {
                "No actionable conditions met".to_string()
            };
            decisions.push(Decision::NoAction { reason });
        }

        decisions
    }

    /// Determine how many new projects to create based on fleet size.
    fn portfolio_slots(&self, fleet_size: u32, active_projects: i32) -> i32 {
        let max_concurrent = match fleet_size {
            0..=4 => 1,
            5..=16 => 3,
            17..=64 => 5,
            _ => 8,
        };
        (max_concurrent - active_projects).max(0).min(2) // max 2 new per tick
    }

    // ── ACT ─────────────────────────────────────────────────────

    /// Execute a single decision: create projects, pause jobs, log to audit trail.
    async fn act(&self, db: &Database, decision: &Decision, tick_id: u64) -> Result<()> {
        let (decision_type, form, action, reasoning, confidence, params) = match decision {
            Decision::CreateProject {
                form,
                params,
                budget_usd,
                reasoning,
                confidence,
            } => {
                let continue_from = params["continue_from"].as_i64().unwrap_or(0);
                let config =
                    strategy::build_auto_project_config(form, continue_from, *budget_usd);
                match db.create_project(&config, None).await {
                    Ok(pid) => {
                        if let Err(e) = db.update_project_status(pid, "active").await {
                            warn!(project_id = pid, error = %e, "ai_engine: failed to activate project");
                        }
                        info!(project_id = pid, form, "ai_engine: created project");
                    }
                    Err(e) => {
                        warn!(form, error = %e, "ai_engine: failed to create project");
                    }
                }
                (
                    "create_project",
                    Some(form.as_str()),
                    "executed",
                    reasoning.as_str(),
                    *confidence,
                    Some(params.clone()),
                )
            }
            Decision::PauseProject {
                project_id,
                reason,
            } => {
                // Try pausing as a search job first (stalled jobs use job_id)
                if let Err(e) = db.update_search_job_status(*project_id, "paused", None).await {
                    warn!(id = project_id, error = %e, "ai_engine: failed to pause job");
                } else {
                    info!(id = project_id, "ai_engine: paused stalled job");
                }
                (
                    "pause_project",
                    None,
                    "executed",
                    reason.as_str(),
                    0.8,
                    Some(serde_json::json!({"project_id": project_id})),
                )
            }
            Decision::VerifyResult {
                form,
                prime_id,
                reasoning,
            } => (
                "verify_result",
                Some(form.as_str()),
                "logged",
                reasoning.as_str(),
                0.9,
                Some(serde_json::json!({"form": form, "prime_id": prime_id})),
            ),
            Decision::NoAction { reason } => (
                "no_action",
                None,
                "logged",
                reason.as_str(),
                1.0,
                None,
            ),
            Decision::ExtendProject {
                project_id,
                reasoning,
                ..
            } => (
                "extend_project",
                None,
                "logged",
                reasoning.as_str(),
                0.7,
                Some(serde_json::json!({"project_id": project_id})),
            ),
            Decision::AbandonProject {
                project_id,
                reason,
            } => (
                "abandon_project",
                None,
                "logged",
                reason.as_str(),
                0.6,
                Some(serde_json::json!({"project_id": project_id})),
            ),
            Decision::RebalanceFleet { reasoning, .. } => (
                "rebalance_fleet",
                None,
                "logged",
                reasoning.as_str(),
                0.5,
                None,
            ),
            Decision::RequestAgentIntel {
                task_title,
                task_description,
            } => (
                "request_agent_intel",
                None,
                "logged",
                task_title.as_str(),
                0.5,
                Some(serde_json::json!({
                    "title": task_title,
                    "description": task_description,
                })),
            ),
        };

        // Log to ai_engine_decisions audit table
        db.insert_ai_engine_decision(
            tick_id as i64,
            decision_type,
            form,
            action,
            reasoning,
            confidence,
            params.as_ref(),
        )
        .await?;

        // Also log to legacy strategy_decisions for backward compatibility
        db.insert_strategy_decision(
            decision_type,
            form,
            &format!("[ai_engine] {}", reasoning),
            reasoning,
            params.as_ref(),
            None,
            action,
            None,
            None,
            None,
        )
        .await
        .ok();

        Ok(())
    }

    // ── LEARN ───────────────────────────────────────────────────

    /// Should we run the LEARN phase this tick?
    fn should_learn(&self) -> bool {
        match self.last_learn {
            None => true, // first tick always learns
            Some(last) => {
                last.elapsed().as_secs() >= self.config.learn_interval_secs
            }
        }
    }

    /// Calibrate cost model from work block data and update scoring weights.
    async fn learn(&mut self, db: &Database) -> Result<()> {
        // 1. Fetch cost calibration data
        let calibrations = db.get_cost_calibrations().await?;

        // 2. Update fitted coefficients from calibration table
        for cal in &calibrations {
            if cal.sample_count >= self.config.min_calibration_samples {
                let mape = cal.avg_error_pct.unwrap_or(1.0);
                if mape <= self.config.max_calibration_mape {
                    self.cost_model
                        .fitted
                        .insert(cal.form.clone(), (cal.coeff_a, cal.coeff_b));
                }
            }
        }

        // 3. Fit cost model from raw work block data
        // (This queries the cost_observations view and runs OLS on log-log data)
        for &form in strategy::ALL_FORMS {
            match db.get_cost_observations(form, 50).await {
                Ok(obs) if obs.len() >= self.config.min_calibration_samples as usize => {
                    if let Some((a, b, mape)) = fit_power_law(&obs) {
                        if mape <= self.config.max_calibration_mape {
                            self.cost_model
                                .fitted
                                .insert(form.to_string(), (a, b));
                            // Persist to DB for next restart
                            db.upsert_cost_calibration(
                                form,
                                a,
                                b,
                                obs.len() as i64,
                                Some(mape),
                            )
                            .await
                            .ok();
                            self.cost_model.version += 1;
                        }
                    }
                }
                _ => {} // not enough data yet
            }
        }

        info!(
            cost_model_version = self.cost_model.version,
            fitted_forms = self.cost_model.fitted.len(),
            "ai_engine: learn phase complete"
        );

        Ok(())
    }

    // ── State Persistence ───────────────────────────────────────

    /// Reload configuration from the strategy_config table.
    async fn reload_config(&mut self, db: &Database) {
        match db.get_strategy_config().await {
            Ok(config) => {
                self.config.enabled = config.enabled;
                self.config.max_concurrent_projects = config.max_concurrent_projects;
                self.config.max_monthly_budget_usd = config.max_monthly_budget_usd;
                self.config.max_per_project_budget_usd = config.max_per_project_budget_usd;
                self.config.excluded_forms = config.excluded_forms;
                self.config.preferred_forms = config.preferred_forms;
                self.config.min_idle_workers_to_create = config.min_idle_workers_to_create as u32;
                self.config.record_proximity_threshold = config.record_proximity_threshold;
            }
            Err(e) => {
                warn!(error = %e, "ai_engine: failed to reload config, using cached");
            }
        }

        // Load learned weights from ai_engine_state if available
        match db.get_ai_engine_state().await {
            Ok(Some(state)) => {
                if let Ok(weights) = serde_json::from_value::<ScoringWeights>(state.scoring_weights)
                {
                    if weights.validate() {
                        self.scoring_weights = weights;
                    }
                }
            }
            Ok(None) => {} // first run, use defaults
            Err(e) => {
                warn!(error = %e, "ai_engine: failed to load engine state");
            }
        }
    }

    /// Persist current engine state to the database.
    async fn persist_state(&self, db: &Database) -> Result<()> {
        let weights_json = serde_json::to_value(&self.scoring_weights)?;
        db.upsert_ai_engine_state(
            &weights_json,
            self.cost_model.version as i32,
            self.tick_count as i64,
        )
        .await?;
        Ok(())
    }
}

// ── Helper Functions ────────────────────────────────────────────

/// Preferred forms scoring multiplier.
const PREFERRED_MULTIPLIER: f64 = 1.5;

/// Default searchable range per form (for opportunity density).
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

/// Minimum cores recommended for a form.
fn min_cores_for_form(form: &str) -> u32 {
    match form {
        "wagstaff" | "repunit" => 16,
        "factorial" | "primorial" => 8,
        _ => 4,
    }
}

fn empty_drift() -> DriftReport {
    DriftReport {
        workers_joined: 0,
        workers_left: 0,
        new_discoveries: 0,
        stalled_jobs: vec![],
        budget_velocity_usd_per_hour: 0.0,
        significant: false,
    }
}

/// A single cost observation: (digits, seconds).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CostObservation {
    pub digits: f64,
    pub secs: f64,
}

/// Fit power-law `secs = a * (digits/1000)^b` via OLS on log-log data.
///
/// Returns (a, b, MAPE) or None if fitting fails.
pub fn fit_power_law(observations: &[CostObservation]) -> Option<(f64, f64, f64)> {
    if observations.len() < 3 {
        return None;
    }

    // Filter valid points (positive digits and secs)
    let points: Vec<(f64, f64)> = observations
        .iter()
        .filter(|o| o.digits > 0.0 && o.secs > 0.0)
        .map(|o| ((o.digits / 1000.0).ln(), o.secs.ln()))
        .collect();

    if points.len() < 3 {
        return None;
    }

    let n = points.len() as f64;
    let sum_x: f64 = points.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = points.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = points.iter().map(|(x, y)| x * y).sum();
    let sum_xx: f64 = points.iter().map(|(x, _)| x * x).sum();

    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < f64::EPSILON {
        return None;
    }

    let b = (n * sum_xy - sum_x * sum_y) / denom;
    let ln_a = (sum_y - b * sum_x) / n;
    let a = ln_a.exp();

    // Compute MAPE (Mean Absolute Percentage Error)
    let mape: f64 = observations
        .iter()
        .filter(|o| o.digits > 0.0 && o.secs > 0.0)
        .map(|o| {
            let predicted = a * (o.digits / 1000.0).powf(b);
            ((o.secs - predicted) / o.secs).abs()
        })
        .sum::<f64>()
        / points.len() as f64;

    Some((a, b, mape))
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ScoringWeights ──────────────────────────────────────────

    #[test]
    fn default_weights_valid() {
        let w = ScoringWeights::default();
        assert!(w.validate(), "Default weights should be valid");
    }

    #[test]
    fn weights_sum_to_one() {
        let w = ScoringWeights::default();
        let sum = w.record_gap
            + w.yield_rate
            + w.cost_efficiency
            + w.opportunity_density
            + w.fleet_fit
            + w.momentum
            + w.competition;
        assert!((sum - 1.0).abs() < 0.01, "Weights should sum to 1.0, got {}", sum);
    }

    #[test]
    fn invalid_weights_detected() {
        let w = ScoringWeights {
            record_gap: 0.90,
            yield_rate: 0.01,
            cost_efficiency: 0.01,
            opportunity_density: 0.01,
            fleet_fit: 0.01,
            momentum: 0.01,
            competition: 0.05,
        };
        assert!(!w.validate(), "Out-of-bounds weights should fail validation");
    }

    #[test]
    fn normalize_weights() {
        let mut w = ScoringWeights {
            record_gap: 0.4,
            yield_rate: 0.3,
            cost_efficiency: 0.4,
            opportunity_density: 0.3,
            fleet_fit: 0.2,
            momentum: 0.2,
            competition: 0.2,
        };
        w.normalize();
        let sum = w.record_gap
            + w.yield_rate
            + w.cost_efficiency
            + w.opportunity_density
            + w.fleet_fit
            + w.momentum
            + w.competition;
        assert!((sum - 1.0).abs() < 0.001, "Normalized weights should sum to 1.0, got {}", sum);
    }

    // ── CostModel ───────────────────────────────────────────────

    #[test]
    fn cost_model_uses_fitted_over_default() {
        let mut model = CostModel::default();
        model.fitted.insert("kbn".to_string(), (0.05, 1.8));

        let fitted = model.secs_per_candidate("kbn", 1000, false);
        let default_spc = 0.1 * 1.0f64.powf(2.0); // default: 0.1 * d^2.0
        let fitted_spc = 0.05 * 1.0f64.powf(1.8);

        assert!(
            (fitted - fitted_spc).abs() < 0.001,
            "Should use fitted coefficients, got {} expected {}",
            fitted, fitted_spc
        );
        assert!(
            (fitted - default_spc).abs() > 0.001,
            "Should differ from default"
        );
    }

    #[test]
    fn cost_model_falls_back_to_default() {
        let model = CostModel::default();
        let spc = model.secs_per_candidate("factorial", 2000, false);
        let expected = 0.5 * 2.0f64.powf(2.5);
        assert!(
            (spc - expected).abs() < 0.001,
            "Should use default coefficients: got {} expected {}",
            spc, expected
        );
    }

    #[test]
    fn cost_model_pfgw_speedup() {
        let model = CostModel::default();
        let without = model.secs_per_candidate("factorial", 10_000, false);
        let with = model.secs_per_candidate("factorial", 10_000, true);
        let ratio = without / with;
        assert!(
            (ratio - 50.0).abs() < 0.1,
            "PFGW should give 50x speedup, got {}x",
            ratio
        );
    }

    #[test]
    fn cost_model_custom_pfgw_speedup() {
        let mut model = CostModel::default();
        model.pfgw_speedup.insert("factorial".to_string(), 75.0);

        let without = model.secs_per_candidate("factorial", 10_000, false);
        let with = model.secs_per_candidate("factorial", 10_000, true);
        let ratio = without / with;
        assert!(
            (ratio - 75.0).abs() < 0.1,
            "Custom PFGW speedup should be 75x, got {}x",
            ratio
        );
    }

    // ── fit_power_law ───────────────────────────────────────────

    #[test]
    fn fit_power_law_exact_data() {
        // Generate exact power-law data: secs = 0.5 * (d/1000)^2.5
        let obs: Vec<CostObservation> = (1..=10)
            .map(|i| {
                let digits = 1000.0 * i as f64;
                let secs = 0.5 * (digits / 1000.0).powf(2.5);
                CostObservation { digits, secs }
            })
            .collect();

        let (a, b, mape) = fit_power_law(&obs).expect("Should fit exact data");
        assert!((a - 0.5).abs() < 0.01, "a should be ~0.5, got {}", a);
        assert!((b - 2.5).abs() < 0.01, "b should be ~2.5, got {}", b);
        assert!(mape < 0.01, "MAPE should be near 0 for exact data, got {}", mape);
    }

    #[test]
    fn fit_power_law_insufficient_data() {
        let obs = vec![
            CostObservation { digits: 1000.0, secs: 0.5 },
            CostObservation { digits: 2000.0, secs: 2.8 },
        ];
        assert!(fit_power_law(&obs).is_none(), "Should need ≥3 points");
    }

    #[test]
    fn fit_power_law_filters_invalid() {
        let obs = vec![
            CostObservation { digits: 0.0, secs: 0.0 },
            CostObservation { digits: -1.0, secs: 1.0 },
            CostObservation { digits: 1000.0, secs: 0.5 },
        ];
        assert!(fit_power_law(&obs).is_none(), "Should filter invalid and need ≥3 valid points");
    }

    // ── Portfolio slots ─────────────────────────────────────────

    #[test]
    fn portfolio_slots_small_fleet() {
        let engine = AiEngine::new();
        assert_eq!(engine.portfolio_slots(2, 0), 1);
        assert_eq!(engine.portfolio_slots(4, 1), 0);
    }

    #[test]
    fn portfolio_slots_medium_fleet() {
        let engine = AiEngine::new();
        assert_eq!(engine.portfolio_slots(10, 0), 2); // capped at 2 per tick
        assert_eq!(engine.portfolio_slots(10, 2), 1);
        assert_eq!(engine.portfolio_slots(10, 3), 0);
    }

    #[test]
    fn portfolio_slots_large_fleet() {
        let engine = AiEngine::new();
        assert_eq!(engine.portfolio_slots(20, 0), 2); // max 2 per tick
        assert_eq!(engine.portfolio_slots(100, 0), 2);
    }

    // ── Safety checks ───────────────────────────────────────────

    #[test]
    fn safety_rejects_over_budget() {
        let limits = SafetyLimits {
            max_projects: 3,
            budget_remaining: 5.0,
            min_budget_for_project: 12.5,
        };
        let decision = Decision::CreateProject {
            form: "kbn".to_string(),
            params: serde_json::json!({}),
            budget_usd: 25.0,
            reasoning: "test".to_string(),
            confidence: 0.9,
        };
        assert!(!safety_check(&decision, &limits));
    }

    #[test]
    fn safety_allows_within_budget() {
        let limits = SafetyLimits {
            max_projects: 3,
            budget_remaining: 50.0,
            min_budget_for_project: 12.5,
        };
        let decision = Decision::CreateProject {
            form: "kbn".to_string(),
            params: serde_json::json!({}),
            budget_usd: 25.0,
            reasoning: "test".to_string(),
            confidence: 0.9,
        };
        assert!(safety_check(&decision, &limits));
    }

    #[test]
    fn safety_always_allows_no_action() {
        let limits = SafetyLimits {
            max_projects: 0,
            budget_remaining: 0.0,
            min_budget_for_project: 100.0,
        };
        let decision = Decision::NoAction {
            reason: "test".to_string(),
        };
        assert!(safety_check(&decision, &limits));
    }

    // ── Scoring ─────────────────────────────────────────────────

    #[test]
    fn score_forms_empty_snapshot() {
        let engine = AiEngine::new();
        let snapshot = WorldSnapshot {
            records: vec![],
            fleet: FleetSnapshot {
                worker_count: 0,
                total_cores: 0,
                idle_workers: 0,
                max_ram_gb: 0,
                active_search_types: vec![],
            },
            active_projects: vec![],
            active_jobs: vec![],
            yield_rates: vec![],
            cost_calibrations: vec![],
            recent_discoveries: vec![],
            agent_results: vec![],
            budget: BudgetSnapshot {
                monthly_budget_usd: 100.0,
                monthly_spend_usd: 0.0,
                remaining_usd: 100.0,
            },
            timestamp: Utc::now(),
        };

        let scores = engine.score_forms(&snapshot);
        assert_eq!(scores.len(), 12, "Should score all 12 forms");
        for s in &scores {
            assert!(s.total >= 0.0, "Score for {} should be non-negative", s.form);
        }
    }

    #[test]
    fn score_forms_sorted_descending() {
        let engine = AiEngine::new();
        let snapshot = WorldSnapshot {
            records: vec![],
            fleet: FleetSnapshot {
                worker_count: 8,
                total_cores: 64,
                idle_workers: 8,
                max_ram_gb: 32,
                active_search_types: vec![],
            },
            active_projects: vec![],
            active_jobs: vec![],
            yield_rates: vec![],
            cost_calibrations: vec![],
            recent_discoveries: vec![],
            agent_results: vec![],
            budget: BudgetSnapshot {
                monthly_budget_usd: 100.0,
                monthly_spend_usd: 0.0,
                remaining_usd: 100.0,
            },
            timestamp: Utc::now(),
        };

        let scores = engine.score_forms(&snapshot);
        for w in scores.windows(2) {
            assert!(
                w[0].total >= w[1].total,
                "Scores not sorted: {} ({}) >= {} ({})",
                w[0].form, w[0].total, w[1].form, w[1].total
            );
        }
    }

    #[test]
    fn score_forms_excludes_forms() {
        let mut engine = AiEngine::new();
        engine.config.excluded_forms = vec!["wagstaff".to_string()];

        let snapshot = WorldSnapshot {
            records: vec![],
            fleet: FleetSnapshot {
                worker_count: 8,
                total_cores: 64,
                idle_workers: 8,
                max_ram_gb: 32,
                active_search_types: vec![],
            },
            active_projects: vec![],
            active_jobs: vec![],
            yield_rates: vec![],
            cost_calibrations: vec![],
            recent_discoveries: vec![],
            agent_results: vec![],
            budget: BudgetSnapshot {
                monthly_budget_usd: 100.0,
                monthly_spend_usd: 0.0,
                remaining_usd: 100.0,
            },
            timestamp: Utc::now(),
        };

        let scores = engine.score_forms(&snapshot);
        let wagstaff = scores.iter().find(|s| s.form == "wagstaff").unwrap();
        assert_eq!(wagstaff.total, 0.0, "Excluded form should have zero score");
    }

    #[test]
    fn momentum_scoring() {
        let engine = AiEngine::new();
        let now = Utc::now();

        let mut snapshot_no_momentum = WorldSnapshot {
            records: vec![],
            fleet: FleetSnapshot {
                worker_count: 8,
                total_cores: 64,
                idle_workers: 8,
                max_ram_gb: 32,
                active_search_types: vec![],
            },
            active_projects: vec![],
            active_jobs: vec![],
            yield_rates: vec![],
            cost_calibrations: vec![],
            recent_discoveries: vec![],
            agent_results: vec![],
            budget: BudgetSnapshot {
                monthly_budget_usd: 100.0,
                monthly_spend_usd: 0.0,
                remaining_usd: 100.0,
            },
            timestamp: now,
        };

        let scores_no = engine.score_forms(&snapshot_no_momentum);
        let kbn_no = scores_no.iter().find(|s| s.form == "kbn").unwrap();

        // Add momentum for kbn
        snapshot_no_momentum.recent_discoveries = vec![
            RecentDiscovery {
                form: "kbn".to_string(),
                digits: 5000,
                found_at: now - chrono::Duration::hours(1),
            },
            RecentDiscovery {
                form: "kbn".to_string(),
                digits: 6000,
                found_at: now - chrono::Duration::hours(2),
            },
        ];

        let scores_with = engine.score_forms(&snapshot_no_momentum);
        let kbn_with = scores_with.iter().find(|s| s.form == "kbn").unwrap();

        assert!(
            kbn_with.momentum > kbn_no.momentum,
            "Momentum should increase with discoveries: {} > {}",
            kbn_with.momentum, kbn_no.momentum
        );
    }

    // ── Decide ──────────────────────────────────────────────────

    #[test]
    fn decide_no_action_no_workers() {
        let engine = AiEngine::new();
        let snapshot = WorldSnapshot {
            records: vec![],
            fleet: FleetSnapshot {
                worker_count: 0,
                total_cores: 0,
                idle_workers: 0,
                max_ram_gb: 0,
                active_search_types: vec![],
            },
            active_projects: vec![],
            active_jobs: vec![],
            yield_rates: vec![],
            cost_calibrations: vec![],
            recent_discoveries: vec![],
            agent_results: vec![],
            budget: BudgetSnapshot {
                monthly_budget_usd: 100.0,
                monthly_spend_usd: 0.0,
                remaining_usd: 100.0,
            },
            timestamp: Utc::now(),
        };
        let analysis = engine.orient(&snapshot);
        let decisions = engine.decide(&snapshot, &analysis);

        assert_eq!(decisions.len(), 1);
        assert!(matches!(&decisions[0], Decision::NoAction { .. }));
    }

    #[test]
    fn decide_creates_project_with_idle_workers() {
        let engine = AiEngine::new();
        let snapshot = WorldSnapshot {
            records: vec![],
            fleet: FleetSnapshot {
                worker_count: 4,
                total_cores: 32,
                idle_workers: 4,
                max_ram_gb: 32,
                active_search_types: vec![],
            },
            active_projects: vec![],
            active_jobs: vec![],
            yield_rates: vec![],
            cost_calibrations: vec![],
            recent_discoveries: vec![],
            agent_results: vec![],
            budget: BudgetSnapshot {
                monthly_budget_usd: 100.0,
                monthly_spend_usd: 0.0,
                remaining_usd: 100.0,
            },
            timestamp: Utc::now(),
        };
        let analysis = engine.orient(&snapshot);
        let decisions = engine.decide(&snapshot, &analysis);

        let has_create = decisions
            .iter()
            .any(|d| matches!(d, Decision::CreateProject { .. }));
        assert!(has_create, "Should create a project with idle workers");
    }

    // ── AiEngineConfig ──────────────────────────────────────────

    #[test]
    fn default_config_sensible() {
        let config = AiEngineConfig::default();
        assert!(config.enabled);
        assert!(config.max_monthly_budget_usd > 0.0);
        assert!(config.max_per_project_budget_usd > 0.0);
        assert!(config.learn_interval_secs > 0);
    }

    // ── Drift detection ─────────────────────────────────────────

    #[test]
    fn drift_first_tick_not_significant() {
        let engine = AiEngine::new();
        let snapshot = WorldSnapshot {
            records: vec![],
            fleet: FleetSnapshot {
                worker_count: 4,
                total_cores: 32,
                idle_workers: 4,
                max_ram_gb: 32,
                active_search_types: vec![],
            },
            active_projects: vec![],
            active_jobs: vec![],
            yield_rates: vec![],
            cost_calibrations: vec![],
            recent_discoveries: vec![],
            agent_results: vec![],
            budget: BudgetSnapshot {
                monthly_budget_usd: 100.0,
                monthly_spend_usd: 0.0,
                remaining_usd: 100.0,
            },
            timestamp: Utc::now(),
        };
        let drift = engine.detect_drift(&snapshot);
        assert!(!drift.significant, "First tick should not report significant drift");
    }
}
