//! Phase state machine, auto-strategy generation, and orchestration tick loop.
//!
//! The orchestration engine runs every 30 seconds from the dashboard and:
//! 1. Checks active phases for completion (all blocks done, first prime found, etc.)
//! 2. Activates next eligible phases (dependencies met, conditions satisfied)
//! 3. Aggregates progress and cost to the project level
//! 4. Marks projects completed when all phases are done
//! 5. Checks budget alerts

use anyhow::Result;

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
            eprintln!("Orchestration error for project '{}': {}", project.slug, e);
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
                    eprintln!(
                        "Project '{}': generated follow-up phase '{}'",
                        project.slug, followup.name
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
                eprintln!(
                    "Project '{}': phase '{}' eligible but fleet insufficient: {}",
                    project.slug, phase.name, reason
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
                    eprintln!(
                        "Project '{}': activating phase '{}' with {} workers (recommended: {})",
                        project.slug, phase.name, fleet.worker_count, recommended
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
        eprintln!(
            "Project '{}' {}: {} tested, {} found",
            project.slug, new_status, total_tested, total_found
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
            eprintln!(
                "Project '{}' paused: budget exceeded (${:.2} >= ${:.2})",
                project.slug, total_cost_usd, max_cost
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

    eprintln!(
        "Project '{}': activated phase '{}' → search job {}",
        project.slug, phase.name, job_id
    );

    Ok(())
}
