//! Phase state machine, auto-strategy generation, and orchestration tick loop.
//!
//! The orchestration engine runs every 30 seconds from the dashboard and:
//! 1. Checks active phases for completion (all blocks done, first prime found, etc.)
//! 2. Activates next eligible phases (dependencies met, conditions satisfied)
//! 3. Aggregates progress and cost to the project level
//! 4. Marks projects completed when all phases are done
//! 5. Checks budget alerts

use anyhow::Result;
use tracing::{info, warn};

use super::config::{Objective, PhaseConfig, ProjectConfig};
use super::cost::extract_range_from_params;
use super::types::{ProjectPhaseRow, ProjectRow};
use crate::db::Database;

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
            warn!(slug = %project.slug, error = %e, "orchestration error");
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
                info!(
                    slug = %project.slug,
                    phase = %phase.name,
                    tested = summary.total_tested,
                    found = summary.total_found,
                    "phase completed"
                );
            }
        }
    }

    // 1b. Generate follow-up phases for completed phases (adaptive strategy)
    let phases_snapshot = db.get_project_phases(project.id).await?;
    for phase in phases_snapshot.iter().filter(|p| p.status == "completed") {
        if let Some(followup) = generate_followup_phase(project, phase, &phases_snapshot) {
            let next_order = phases_snapshot
                .iter()
                .map(|p| p.phase_order)
                .max()
                .unwrap_or(0)
                + 1;
            match db.insert_phase(project.id, &followup, next_order).await {
                Ok(_) => {
                    db.insert_project_event(
                        project.id,
                        "phase_generated",
                        &format!(
                            "Auto-generated follow-up phase '{}' after '{}' ({})",
                            followup.name, phase.name, followup.description
                        ),
                        None,
                    )
                    .await?;
                    info!(
                        slug = %project.slug,
                        phase = %followup.name,
                        "generated follow-up phase"
                    );
                }
                Err(e) => {
                    // Duplicate name constraint — phase was already generated in a previous tick
                    if e.to_string().contains("duplicate") || e.to_string().contains("unique") {
                        // Expected: the phase was already generated
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    // Reload phases after potential status changes and adaptive generation
    let phases = db.get_project_phases(project.id).await?;

    // 2. Activate next eligible phases (check fleet requirements first)
    let fleet = db
        .get_fleet_summary()
        .await
        .unwrap_or_else(|_| crate::db::FleetSummary {
            worker_count: 0,
            total_cores: 0,
            max_ram_gb: 0,
            active_search_types: vec![],
        });

    for phase in phases.iter().filter(|p| p.status == "pending") {
        if should_activate(phase, &phases) {
            // Enforce infrastructure and worker requirements before activation
            if let Some(reason) = check_fleet_requirements(project, &fleet) {
                warn!(
                    slug = %project.slug,
                    phase = %phase.name,
                    reason,
                    "phase eligible but fleet insufficient"
                );
                db.insert_project_event(
                    project.id,
                    "fleet_insufficient",
                    &format!("Phase '{}' waiting: {}", phase.name, reason),
                    None,
                )
                .await
                .ok();
                continue;
            }

            // Warn if below recommended_workers (non-blocking)
            if let Some(recommended) = project
                .infrastructure
                .get("recommended_workers")
                .or_else(|| project.budget.get("recommended_workers"))
                .and_then(serde_json::Value::as_u64)
            {
                if (fleet.worker_count as u64) < recommended {
                    warn!(
                        slug = %project.slug,
                        phase = %phase.name,
                        workers = fleet.worker_count,
                        recommended,
                        "activating phase with fewer workers than recommended"
                    );
                }
            }

            activate_phase(db, project, phase).await?;
        }
    }

    // 3. Aggregate progress to project level
    let phases = db.get_project_phases(project.id).await?;
    let total_tested: i64 = phases.iter().map(|p| p.total_tested).sum();
    let total_found: i64 = phases.iter().map(|p| p.total_found).sum();
    db.update_project_progress(project.id, total_tested, total_found)
        .await?;

    // 3b. Compute actual cost from work block durations
    let cloud_rate = project
        .budget
        .get("cloud_rate_usd_per_core_hour")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.04);
    let mut total_core_hours = 0.0f64;
    for phase in phases.iter().filter(|p| p.search_job_id.is_some()) {
        let job_id = phase.search_job_id.unwrap();
        let hours = db.get_job_core_hours(job_id).await.unwrap_or(0.0);
        total_core_hours += hours;
    }
    let total_cost_usd = total_core_hours * cloud_rate;
    db.update_project_cost(project.id, total_core_hours, total_cost_usd)
        .await?;

    // 3c. Link best prime found for this form to the project
    if let Ok(Some(best)) = db.get_best_prime_for_form(&project.form).await {
        if best.digits > project.best_digits {
            db.update_project_best_prime(project.id, Some(best.id), best.digits)
                .await?;
        }
    }

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
        info!(
            slug = %project.slug,
            status = new_status,
            total_tested,
            total_found,
            "project completed"
        );
    }

    // 5. Check budget alerts (use freshly computed cost, not stale project row)
    if let Some(max_cost) = project
        .budget
        .get("max_cost_usd")
        .and_then(serde_json::Value::as_f64)
    {
        if total_cost_usd >= max_cost {
            db.update_project_status(project.id, "paused").await?;
            db.insert_project_event(
                project.id,
                "budget_exceeded",
                &format!(
                    "Budget exceeded: ${:.2} >= ${:.2} — project paused",
                    total_cost_usd, max_cost
                ),
                None,
            )
            .await?;
            warn!(
                slug = %project.slug,
                cost_usd = format_args!("{:.2}", total_cost_usd),
                max_cost_usd = format_args!("{:.2}", max_cost),
                "project paused: budget exceeded"
            );
        } else if let Some(alert) = project
            .budget
            .get("cost_alert_threshold_usd")
            .and_then(serde_json::Value::as_f64)
        {
            if total_cost_usd >= alert {
                db.insert_project_event(
                    project.id,
                    "budget_alert",
                    &format!(
                        "Cost alert: ${:.2} >= ${:.2} threshold",
                        total_cost_usd, alert
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
pub(crate) fn is_phase_complete(condition: &str, summary: &crate::db::JobBlockSummary) -> bool {
    match condition {
        "all_blocks_done" => summary.available == 0 && summary.claimed == 0,
        "first_prime_found" => summary.total_found > 0,
        _ => summary.available == 0 && summary.claimed == 0,
    }
}

/// Check if a pending phase should be activated (all dependencies met,
/// activation condition satisfied).
pub(crate) fn should_activate(phase: &ProjectPhaseRow, all_phases: &[ProjectPhaseRow]) -> bool {
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

/// Generate a follow-up phase when a completed phase indicates more work is needed.
///
/// Rules:
/// - If a sweep/survey found 0 primes → extend the range by the same span
/// - Only generates one follow-up per phase (checks for existing follow-up name)
/// - The follow-up depends on the completed phase
///
/// Returns `None` if no follow-up is warranted (primes were found, or a follow-up
/// already exists).
pub(crate) fn generate_followup_phase(
    project: &ProjectRow,
    completed_phase: &ProjectPhaseRow,
    all_phases: &[ProjectPhaseRow],
) -> Option<PhaseConfig> {
    // Only generate follow-ups when no primes were found
    if completed_phase.total_found > 0 {
        return None;
    }

    // Don't generate if this phase was already a follow-up (prevent infinite chains)
    let followup_name = format!("{}-extend", completed_phase.name);
    if all_phases.iter().any(|p| p.name == followup_name) {
        return None;
    }

    // Don't generate for phases that are themselves extensions (max 1 level deep)
    if completed_phase.name.ends_with("-extend") {
        return None;
    }

    // Extract the range from the completed phase's search params
    let (range_start, range_end) =
        super::cost::extract_range_from_params(&completed_phase.search_params);
    if range_end <= range_start {
        return None;
    }

    let span = range_end - range_start;
    let new_start = range_end + 1;
    let new_end = new_start + span;

    // Build new search params by cloning and updating the range
    let mut new_params = completed_phase.search_params.clone();
    if let Some(obj) = new_params.as_object_mut() {
        // Update whichever range keys exist
        if obj.contains_key("start") {
            obj.insert("start".to_string(), serde_json::json!(new_start));
            obj.insert("end".to_string(), serde_json::json!(new_end));
        } else if obj.contains_key("min_n") {
            obj.insert("min_n".to_string(), serde_json::json!(new_start));
            obj.insert("max_n".to_string(), serde_json::json!(new_end));
        } else if obj.contains_key("min_exp") {
            obj.insert("min_exp".to_string(), serde_json::json!(new_start));
            obj.insert("max_exp".to_string(), serde_json::json!(new_end));
        } else if obj.contains_key("min_digits") {
            obj.insert("min_digits".to_string(), serde_json::json!(new_start));
            obj.insert("max_digits".to_string(), serde_json::json!(new_end));
        } else if obj.contains_key("min_base") {
            obj.insert("min_base".to_string(), serde_json::json!(new_start));
            obj.insert("max_base".to_string(), serde_json::json!(new_end));
        }
    }

    Some(PhaseConfig {
        name: followup_name,
        description: format!(
            "Auto-extend {} search {}..{} (no primes found in {}..{})",
            project.form, new_start, new_end, range_start, range_end
        ),
        search_params: new_params,
        block_size: Some(completed_phase.block_size),
        depends_on: Some(vec![completed_phase.name.clone()]),
        activation_condition: Some("previous_phase_found_zero".to_string()),
        completion: completed_phase.completion_condition.clone(),
    })
}

/// Check if the fleet meets a project's infrastructure requirements.
///
/// Compares the project's `[infrastructure]` config (min_cores, min_ram_gb,
/// required_tools) and `[workers]` config (min_workers) against the current
/// fleet capabilities. Returns `None` if all requirements are met, or
/// `Some(reason)` describing the first unmet requirement.
pub(crate) fn check_fleet_requirements(
    project: &ProjectRow,
    fleet: &crate::db::FleetSummary,
) -> Option<String> {
    // Check min_workers from [workers] config
    if let Some(min_workers) = project
        .infrastructure
        .get("min_workers")
        .or_else(|| {
            // Also check top-level workers config stored alongside infrastructure
            // (the dashboard serializes WorkerConfig into the project row)
            None
        })
        .and_then(serde_json::Value::as_u64)
    {
        if (fleet.worker_count as u64) < min_workers {
            return Some(format!(
                "Need {} workers, only {} active",
                min_workers, fleet.worker_count
            ));
        }
    }

    // Parse infrastructure requirements from the JSON column
    if let Some(min_cores) = project
        .infrastructure
        .get("min_cores")
        .and_then(serde_json::Value::as_u64)
    {
        if (fleet.total_cores as u64) < min_cores {
            return Some(format!(
                "Need {} cores, fleet has {}",
                min_cores, fleet.total_cores
            ));
        }
    }

    if let Some(min_ram) = project
        .infrastructure
        .get("min_ram_gb")
        .and_then(serde_json::Value::as_u64)
    {
        if (fleet.max_ram_gb as u64) < min_ram {
            return Some(format!(
                "Need {} GB RAM, best worker has {} GB",
                min_ram, fleet.max_ram_gb
            ));
        }
    }

    if let Some(tools) = project
        .infrastructure
        .get("required_tools")
        .and_then(serde_json::Value::as_array)
    {
        for tool in tools {
            if let Some(tool_name) = tool.as_str() {
                if !tool_name.is_empty()
                    && !fleet
                        .active_search_types
                        .iter()
                        .any(|s| s.contains(tool_name))
                {
                    return Some(format!(
                        "Required tool '{}' not available in fleet",
                        tool_name
                    ));
                }
            }
        }
    }

    None
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

    info!(
        slug = %project.slug,
        phase = %phase.name,
        job_id,
        "activated phase"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::FleetSummary;
    use crate::project::types::ProjectPhaseRow;

    // ── Helper factories ────────────────────────────────────────

    fn make_phase(
        name: &str,
        status: &str,
        depends_on: Vec<String>,
        activation_condition: Option<String>,
        total_found: i64,
    ) -> ProjectPhaseRow {
        ProjectPhaseRow {
            id: 1,
            project_id: 1,
            name: name.into(),
            description: String::new(),
            phase_order: 0,
            status: status.into(),
            search_params: serde_json::json!({"search_type": "factorial", "start": 1000, "end": 2000}),
            block_size: 1000,
            depends_on,
            activation_condition,
            completion_condition: "all_blocks_done".into(),
            search_job_id: None,
            total_tested: 1000,
            total_found,
            started_at: None,
            completed_at: None,
        }
    }

    fn make_project_row(infra: serde_json::Value) -> crate::project::ProjectRow {
        crate::project::ProjectRow {
            id: 1,
            slug: "test".into(),
            name: "Test".into(),
            description: String::new(),
            objective: "survey".into(),
            form: "factorial".into(),
            status: "active".into(),
            toml_source: None,
            target: serde_json::json!({}),
            competitive: serde_json::json!(null),
            strategy: serde_json::json!({}),
            infrastructure: infra,
            budget: serde_json::json!({}),
            total_tested: 0,
            total_found: 0,
            best_prime_id: None,
            best_digits: 0,
            total_core_hours: 0.0,
            total_cost_usd: 0.0,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            updated_at: chrono::Utc::now(),
        }
    }

    fn make_fleet(workers: u32, cores: u32, ram_gb: u32, search_types: Vec<String>) -> FleetSummary {
        FleetSummary {
            worker_count: workers,
            total_cores: cores,
            max_ram_gb: ram_gb,
            active_search_types: search_types,
        }
    }

    // ── is_phase_complete ───────────────────────────────────────

    #[test]
    fn phase_complete_all_blocks_done_when_no_remaining() {
        let summary = crate::db::JobBlockSummary {
            available: 0,
            claimed: 0,
            completed: 50,
            failed: 0,
            total_tested: 50000,
            total_found: 3,
        };
        assert!(is_phase_complete("all_blocks_done", &summary));
    }

    #[test]
    fn phase_not_complete_when_blocks_still_claimed() {
        let summary = crate::db::JobBlockSummary {
            available: 0,
            claimed: 2,
            completed: 8,
            failed: 0,
            total_tested: 8000,
            total_found: 0,
        };
        assert!(!is_phase_complete("all_blocks_done", &summary));
    }

    #[test]
    fn phase_not_complete_when_blocks_still_available() {
        let summary = crate::db::JobBlockSummary {
            available: 5,
            claimed: 0,
            completed: 5,
            failed: 0,
            total_tested: 5000,
            total_found: 0,
        };
        assert!(!is_phase_complete("all_blocks_done", &summary));
    }

    #[test]
    fn phase_complete_first_prime_found() {
        let summary = crate::db::JobBlockSummary {
            available: 10,
            claimed: 2,
            completed: 3,
            failed: 0,
            total_tested: 3000,
            total_found: 1,
        };
        assert!(is_phase_complete("first_prime_found", &summary));
    }

    #[test]
    fn phase_not_complete_first_prime_not_found() {
        let summary = crate::db::JobBlockSummary {
            available: 5,
            claimed: 3,
            completed: 2,
            failed: 0,
            total_tested: 2000,
            total_found: 0,
        };
        assert!(!is_phase_complete("first_prime_found", &summary));
    }

    #[test]
    fn phase_complete_unknown_condition_treated_as_all_blocks_done() {
        let summary = crate::db::JobBlockSummary {
            available: 0,
            claimed: 0,
            completed: 10,
            failed: 0,
            total_tested: 10000,
            total_found: 0,
        };
        assert!(is_phase_complete("some_unknown_condition", &summary));
    }

    #[test]
    fn phase_not_complete_unknown_condition_with_remaining() {
        let summary = crate::db::JobBlockSummary {
            available: 1,
            claimed: 0,
            completed: 9,
            failed: 0,
            total_tested: 9000,
            total_found: 5,
        };
        assert!(!is_phase_complete("some_unknown_condition", &summary));
    }

    // ── should_activate ─────────────────────────────────────────

    #[test]
    fn activate_no_dependencies() {
        let phase = make_phase("sweep", "pending", vec![], None, 0);
        assert!(should_activate(&phase, &[phase.clone()]));
    }

    #[test]
    fn activate_met_dependency() {
        let sweep = make_phase("sweep", "completed", vec![], None, 5);
        let extend = make_phase("extend", "pending", vec!["sweep".into()], None, 0);
        assert!(should_activate(&extend, &[sweep, extend.clone()]));
    }

    #[test]
    fn activate_unmet_dependency_active() {
        let sweep = make_phase("sweep", "active", vec![], None, 0);
        let extend = make_phase("extend", "pending", vec!["sweep".into()], None, 0);
        assert!(!should_activate(&extend, &[sweep, extend.clone()]));
    }

    #[test]
    fn activate_unmet_dependency_pending() {
        let sweep = make_phase("sweep", "pending", vec![], None, 0);
        let extend = make_phase("extend", "pending", vec!["sweep".into()], None, 0);
        assert!(!should_activate(&extend, &[sweep, extend.clone()]));
    }

    #[test]
    fn activate_missing_dependency() {
        // Phase depends on "nonexistent" which is not in all_phases
        let phase = make_phase("extend", "pending", vec!["nonexistent".into()], None, 0);
        assert!(!should_activate(&phase, &[phase.clone()]));
    }

    #[test]
    fn activate_condition_previous_found_zero_satisfied() {
        let sweep = make_phase("sweep", "completed", vec![], None, 0); // found=0
        let extend = make_phase(
            "extend",
            "pending",
            vec!["sweep".into()],
            Some("previous_phase_found_zero".into()),
            0,
        );
        assert!(should_activate(&extend, &[sweep, extend.clone()]));
    }

    #[test]
    fn activate_condition_previous_found_zero_not_satisfied() {
        let sweep = make_phase("sweep", "completed", vec![], None, 5); // found=5
        let extend = make_phase(
            "extend",
            "pending",
            vec!["sweep".into()],
            Some("previous_phase_found_zero".into()),
            0,
        );
        assert!(!should_activate(&extend, &[sweep, extend.clone()]));
    }

    #[test]
    fn activate_condition_previous_found_prime_satisfied() {
        let sweep = make_phase("sweep", "completed", vec![], None, 3); // found=3
        let verify = make_phase(
            "verify",
            "pending",
            vec!["sweep".into()],
            Some("previous_phase_found_prime".into()),
            0,
        );
        assert!(should_activate(&verify, &[sweep, verify.clone()]));
    }

    #[test]
    fn activate_condition_previous_found_prime_not_satisfied() {
        let sweep = make_phase("sweep", "completed", vec![], None, 0); // found=0
        let verify = make_phase(
            "verify",
            "pending",
            vec!["sweep".into()],
            Some("previous_phase_found_prime".into()),
            0,
        );
        assert!(!should_activate(&verify, &[sweep, verify.clone()]));
    }

    #[test]
    fn activate_unknown_condition_passes() {
        let sweep = make_phase("sweep", "completed", vec![], None, 5);
        let next = make_phase(
            "next",
            "pending",
            vec!["sweep".into()],
            Some("unknown_condition_xyz".into()),
            0,
        );
        // Unknown conditions default to true (match _ => {})
        assert!(should_activate(&next, &[sweep, next.clone()]));
    }

    #[test]
    fn activate_multiple_dependencies_all_met() {
        let a = make_phase("a", "completed", vec![], None, 1);
        let b = make_phase("b", "completed", vec![], None, 2);
        let c = make_phase(
            "c",
            "pending",
            vec!["a".into(), "b".into()],
            None,
            0,
        );
        assert!(should_activate(&c, &[a, b, c.clone()]));
    }

    #[test]
    fn activate_multiple_dependencies_one_unmet() {
        let a = make_phase("a", "completed", vec![], None, 1);
        let b = make_phase("b", "active", vec![], None, 0);
        let c = make_phase(
            "c",
            "pending",
            vec!["a".into(), "b".into()],
            None,
            0,
        );
        assert!(!should_activate(&c, &[a, b, c.clone()]));
    }

    // ── generate_followup_phase ─────────────────────────────────

    #[test]
    fn followup_generated_with_start_end_keys() {
        let project = make_project_row(serde_json::json!(null));
        let phase = make_phase("sweep", "completed", vec![], None, 0);
        let result = generate_followup_phase(&project, &phase, &[phase.clone()]);
        assert!(result.is_some());
        let followup = result.unwrap();
        assert_eq!(followup.name, "sweep-extend");
        let (start, end) = extract_range_from_params(&followup.search_params);
        assert_eq!(start, 2001);
        assert_eq!(end, 3001);
    }

    #[test]
    fn followup_skipped_when_primes_found() {
        let project = make_project_row(serde_json::json!(null));
        let phase = make_phase("sweep", "completed", vec![], None, 5);
        assert!(generate_followup_phase(&project, &phase, &[phase.clone()]).is_none());
    }

    #[test]
    fn followup_prevented_for_extend_phase() {
        let project = make_project_row(serde_json::json!(null));
        let mut phase = make_phase("sweep-extend", "completed", vec![], None, 0);
        phase.search_params = serde_json::json!({"search_type": "factorial", "start": 2001, "end": 3001});
        assert!(generate_followup_phase(&project, &phase, &[phase.clone()]).is_none());
    }

    #[test]
    fn followup_prevented_when_already_exists() {
        let project = make_project_row(serde_json::json!(null));
        let sweep = make_phase("sweep", "completed", vec![], None, 0);
        let extend = make_phase("sweep-extend", "pending", vec!["sweep".into()], None, 0);
        assert!(generate_followup_phase(&project, &sweep, &[sweep.clone(), extend]).is_none());
    }

    #[test]
    fn followup_uses_min_n_max_n_keys() {
        let project = make_project_row(serde_json::json!(null));
        let mut phase = make_phase("sweep", "completed", vec![], None, 0);
        phase.search_params = serde_json::json!({
            "search_type": "kbn",
            "k": 3,
            "base": 2,
            "min_n": 100000,
            "max_n": 200000,
        });
        let followup = generate_followup_phase(&project, &phase, &[phase.clone()]).unwrap();
        let (start, end) = extract_range_from_params(&followup.search_params);
        assert_eq!(start, 200001);
        assert_eq!(end, 300001);
        // Verify extra params preserved
        assert_eq!(followup.search_params["k"], 3);
        assert_eq!(followup.search_params["base"], 2);
    }

    #[test]
    fn followup_uses_min_exp_max_exp_keys() {
        let project = make_project_row(serde_json::json!(null));
        let mut phase = make_phase("sweep", "completed", vec![], None, 0);
        phase.search_params = serde_json::json!({
            "search_type": "wagstaff",
            "min_exp": 13000000,
            "max_exp": 14000000,
        });
        let followup = generate_followup_phase(&project, &phase, &[phase.clone()]).unwrap();
        let (start, end) = extract_range_from_params(&followup.search_params);
        assert_eq!(start, 14000001);
        assert_eq!(end, 15000001);
    }

    #[test]
    fn followup_uses_min_digits_max_digits_keys() {
        let project = make_project_row(serde_json::json!(null));
        let mut phase = make_phase("sweep", "completed", vec![], None, 0);
        phase.search_params = serde_json::json!({
            "search_type": "palindromic",
            "min_digits": 1,
            "max_digits": 11,
        });
        let followup = generate_followup_phase(&project, &phase, &[phase.clone()]).unwrap();
        let (start, end) = extract_range_from_params(&followup.search_params);
        assert_eq!(start, 12);
        assert_eq!(end, 22);
    }

    #[test]
    fn followup_preserves_block_size_and_completion() {
        let project = make_project_row(serde_json::json!(null));
        let mut phase = make_phase("sweep", "completed", vec![], None, 0);
        phase.block_size = 5000;
        phase.completion_condition = "first_prime_found".into();
        let followup = generate_followup_phase(&project, &phase, &[phase.clone()]).unwrap();
        assert_eq!(followup.block_size, Some(5000));
        assert_eq!(followup.completion, "first_prime_found");
    }

    #[test]
    fn followup_depends_on_completed_phase() {
        let project = make_project_row(serde_json::json!(null));
        let phase = make_phase("sweep", "completed", vec![], None, 0);
        let followup = generate_followup_phase(&project, &phase, &[phase.clone()]).unwrap();
        assert_eq!(followup.depends_on, Some(vec!["sweep".to_string()]));
        assert_eq!(followup.activation_condition, Some("previous_phase_found_zero".to_string()));
    }

    #[test]
    fn followup_not_generated_for_zero_span() {
        let project = make_project_row(serde_json::json!(null));
        let mut phase = make_phase("sweep", "completed", vec![], None, 0);
        // start == end → span is 0
        phase.search_params = serde_json::json!({"search_type": "factorial", "start": 100, "end": 100});
        assert!(generate_followup_phase(&project, &phase, &[phase.clone()]).is_none());
    }

    // ── generate_auto_strategy ──────────────────────────────────

    fn make_config(form: &str, objective: Objective, range_start: Option<u64>, range_end: Option<u64>) -> ProjectConfig {
        ProjectConfig {
            project: super::super::config::ProjectMeta {
                name: "test".into(),
                description: String::new(),
                objective,
                form: form.into(),
                author: String::new(),
                tags: vec![],
            },
            target: super::super::config::TargetConfig {
                target_digits: None,
                range_start,
                range_end,
            },
            competitive: None,
            strategy: super::super::config::StrategyConfig::default(),
            infrastructure: None,
            budget: None,
            workers: None,
        }
    }

    #[test]
    fn auto_strategy_factorial_record_single_phase() {
        let config = make_config("factorial", Objective::Record, Some(500), Some(5500));
        let phases = generate_auto_strategy(&config);
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].name, "sweep");
        assert_eq!(phases[0].completion, "all_blocks_done");
        assert_eq!(phases[0].block_size, Some(100));
        // Range should match config
        let (s, e) = extract_range_from_params(&phases[0].search_params);
        assert_eq!(s, 500);
        assert_eq!(e, 5500);
    }

    #[test]
    fn auto_strategy_factorial_survey_single_phase() {
        let config = make_config("factorial", Objective::Survey, Some(1), Some(500));
        let phases = generate_auto_strategy(&config);
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].name, "survey");
        let (s, e) = extract_range_from_params(&phases[0].search_params);
        assert_eq!(s, 1);
        assert_eq!(e, 500);
    }

    #[test]
    fn auto_strategy_wagstaff_record_two_phases() {
        let config = make_config("wagstaff", Objective::Record, Some(14_000_000), Some(20_000_000));
        let phases = generate_auto_strategy(&config);
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].name, "sweep");
        assert_eq!(phases[1].name, "extend");
        assert_eq!(phases[1].depends_on, Some(vec!["sweep".to_string()]));
        assert_eq!(phases[1].activation_condition, Some("previous_phase_found_zero".to_string()));
        // Sweep covers first half, extend covers second half
        let (s1, e1) = extract_range_from_params(&phases[0].search_params);
        let (s2, e2) = extract_range_from_params(&phases[1].search_params);
        assert_eq!(s1, 14_000_000);
        assert!(e1 < e2); // sweep ends before extend
        assert_eq!(e2, 20_000_000);
    }

    #[test]
    fn auto_strategy_kbn_single_phase() {
        let config = make_config("kbn", Objective::Survey, Some(1), Some(500_000));
        let phases = generate_auto_strategy(&config);
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].name, "sweep");
        assert_eq!(phases[0].block_size, Some(100_000));
        // Should include k and base defaults
        assert!(phases[0].search_params.get("k").is_some());
        assert!(phases[0].search_params.get("base").is_some());
    }

    #[test]
    fn auto_strategy_twin_same_as_kbn() {
        let config = make_config("twin", Objective::Survey, Some(1), Some(100_000));
        let phases = generate_auto_strategy(&config);
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].name, "sweep");
    }

    #[test]
    fn auto_strategy_sophie_germain() {
        let config = make_config("sophie_germain", Objective::Record, Some(100), Some(50_000));
        let phases = generate_auto_strategy(&config);
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].name, "sweep");
    }

    #[test]
    fn auto_strategy_palindromic() {
        let config = make_config("palindromic", Objective::Survey, Some(1), Some(21));
        let phases = generate_auto_strategy(&config);
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].name, "sweep");
        assert_eq!(phases[0].block_size, Some(2));
        let (s, e) = extract_range_from_params(&phases[0].search_params);
        assert_eq!(s, 1);
        assert_eq!(e, 21);
    }

    #[test]
    fn auto_strategy_near_repdigit() {
        let config = make_config("near_repdigit", Objective::Survey, Some(3), Some(15));
        let phases = generate_auto_strategy(&config);
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].name, "sweep");
    }

    #[test]
    fn auto_strategy_generic_form() {
        let config = make_config("repunit", Objective::Custom, Some(100), Some(5000));
        let phases = generate_auto_strategy(&config);
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].name, "sweep");
        assert_eq!(phases[0].block_size, Some(1000));
    }

    #[test]
    fn auto_strategy_uses_defaults_when_no_range() {
        let config = make_config("factorial", Objective::Record, None, None);
        let phases = generate_auto_strategy(&config);
        assert_eq!(phases.len(), 1);
        // Should use default start=1000
        let (s, _e) = extract_range_from_params(&phases[0].search_params);
        assert_eq!(s, 1000);
    }

    // ── check_fleet_requirements ────────────────────────────────

    #[test]
    fn fleet_sufficient_no_requirements() {
        let project = make_project_row(serde_json::json!(null));
        let fleet = make_fleet(2, 16, 32, vec!["factorial".into()]);
        assert!(check_fleet_requirements(&project, &fleet).is_none());
    }

    #[test]
    fn fleet_sufficient_all_requirements_met() {
        let project = make_project_row(serde_json::json!({
            "min_workers": 2,
            "min_cores": 16,
            "min_ram_gb": 32,
            "required_tools": ["factorial"],
        }));
        let fleet = make_fleet(2, 16, 32, vec!["factorial".into()]);
        assert!(check_fleet_requirements(&project, &fleet).is_none());
    }

    #[test]
    fn fleet_insufficient_workers() {
        let project = make_project_row(serde_json::json!({"min_workers": 4}));
        let fleet = make_fleet(2, 16, 32, vec![]);
        let reason = check_fleet_requirements(&project, &fleet);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("workers"));
    }

    #[test]
    fn fleet_insufficient_cores() {
        let project = make_project_row(serde_json::json!({"min_cores": 64}));
        let fleet = make_fleet(4, 32, 64, vec![]);
        let reason = check_fleet_requirements(&project, &fleet);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("cores"));
    }

    #[test]
    fn fleet_insufficient_ram() {
        let project = make_project_row(serde_json::json!({"min_ram_gb": 128}));
        let fleet = make_fleet(4, 64, 64, vec![]);
        let reason = check_fleet_requirements(&project, &fleet);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("RAM"));
    }

    #[test]
    fn fleet_missing_required_tool() {
        let project = make_project_row(serde_json::json!({"required_tools": ["gwnum"]}));
        let fleet = make_fleet(4, 64, 64, vec!["factorial".into()]);
        let reason = check_fleet_requirements(&project, &fleet);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("gwnum"));
    }

    #[test]
    fn fleet_tool_matched_by_substring() {
        // required_tools checks if fleet active_search_types contain the tool name
        let project = make_project_row(serde_json::json!({"required_tools": ["factorial"]}));
        let fleet = make_fleet(1, 8, 16, vec!["factorial-search".into()]);
        // "factorial-search" contains "factorial", so this should pass
        assert!(check_fleet_requirements(&project, &fleet).is_none());
    }

    #[test]
    fn fleet_empty_fails_with_any_requirement() {
        let project = make_project_row(serde_json::json!({"min_workers": 1}));
        let fleet = make_fleet(0, 0, 0, vec![]);
        assert!(check_fleet_requirements(&project, &fleet).is_some());
    }

    #[test]
    fn fleet_empty_tool_string_ignored() {
        // Empty string in required_tools should be skipped
        let project = make_project_row(serde_json::json!({"required_tools": [""]}));
        let fleet = make_fleet(1, 8, 16, vec![]);
        assert!(check_fleet_requirements(&project, &fleet).is_none());
    }
}
