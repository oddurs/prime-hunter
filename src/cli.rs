//! # CLI Execution Functions
//!
//! Extracted from `main.rs` to keep the entry point slim. Contains the execution
//! logic for each subcommand: search dispatch, work-block claiming, verification,
//! project management, and rayon configuration.

use anyhow::Result;
use darkreach::{
    carol_kynea, cullen_woodall, db, events, factorial, gen_fermat, kbn, near_repdigit,
    palindromic, pg_worker, primorial, progress, project, repunit, sophie_germain, twin, verify,
    wagstaff, CoordinationClient,
};
use std::sync::Arc;
use tracing::{info, info_span, warn};

use super::{Cli, Commands, ProjectAction};

// ── Search Dispatch ─────────────────────────────────────────────

/// Run a search subcommand (all 12 forms). Handles DB connection, worker
/// coordination, search dispatch, progress reporting, and cleanup.
pub fn run_search(cli: &Cli) -> Result<()> {
    let database_url = cli.database_url.as_deref().ok_or_else(|| {
        anyhow::anyhow!("DATABASE_URL is required (set via --database-url or env)")
    })?;

    let num_cores = rayon::current_num_threads();
    info!(
        cores = num_cores,
        mr_rounds = cli.mr_rounds,
        "darkreach starting"
    );

    let rt = tokio::runtime::Runtime::new()?;
    let database = rt.block_on(db::Database::connect(database_url))?;
    let db = Arc::new(database);
    let rt_handle = rt.handle().clone();

    // Handle the `work` subcommand (block-claiming loop)
    if let Commands::Work { search_job_id } = &cli.command {
        return run_work_loop(cli, &db, &rt_handle, *search_job_id);
    }

    let progress = progress::Progress::new();
    let reporter_handle = progress.start_reporter();
    let event_bus = Arc::new(events::EventBus::new());

    let search_type = search_type_for(&cli.command);
    let search_params = search_params_for(&cli.command);

    // Set up PG coordination client for heartbeat and stop-check
    let worker_id = cli.worker_id.clone().unwrap_or_else(get_hostname);

    let pg_client = pg_worker::PgWorkerClient::new(
        db.pool().clone(),
        rt_handle.clone(),
        &worker_id,
        search_type,
        &search_params,
    );

    sync_progress_to_atomics(&progress, &pg_client.tested, &pg_client.found, &pg_client.current);

    let heartbeat_handle = Some(pg_client.start_heartbeat());

    let coord: Option<&dyn CoordinationClient> = Some(&pg_client);

    let mr = cli.mr_rounds;
    let sl = cli.sieve_limit;
    let eb = Some(event_bus.as_ref() as &events::EventBus);

    event_bus.emit(events::Event::SearchStarted {
        search_type: search_type.to_string(),
        params: search_params.clone(),
        timestamp: std::time::Instant::now(),
    });

    let search_start = std::time::Instant::now();
    let result = dispatch_search(
        &cli.command,
        &progress,
        &db,
        &rt_handle,
        &cli.checkpoint,
        &search_params,
        mr,
        sl,
        coord,
        eb,
    );

    event_bus.emit(events::Event::SearchCompleted {
        search_type: search_type.to_string(),
        tested: progress.tested.load(std::sync::atomic::Ordering::Relaxed),
        found: progress.found.load(std::sync::atomic::Ordering::Relaxed),
        elapsed_secs: search_start.elapsed().as_secs_f64(),
        timestamp: std::time::Instant::now(),
    });
    event_bus.flush();

    progress.stop();
    let _ = reporter_handle.join();
    progress.print_status();

    pg_client.deregister();
    if let Some(handle) = heartbeat_handle {
        let _ = handle.join();
    }

    info!("Search complete");
    result
}

/// Return the search type string for a given command variant.
fn search_type_for(cmd: &Commands) -> &'static str {
    match cmd {
        Commands::Factorial { .. } => "factorial",
        Commands::Palindromic { .. } => "palindromic",
        Commands::Kbn { .. } => "kbn",
        Commands::NearRepdigit { .. } => "near_repdigit",
        Commands::Primorial { .. } => "primorial",
        Commands::CullenWoodall { .. } => "cullen_woodall",
        Commands::Wagstaff { .. } => "wagstaff",
        Commands::CarolKynea { .. } => "carol_kynea",
        Commands::Twin { .. } => "twin",
        Commands::SophieGermain { .. } => "sophie_germain",
        Commands::Repunit { .. } => "repunit",
        Commands::GenFermat { .. } => "gen_fermat",
        Commands::Dashboard { .. }
        | Commands::Work { .. }
        | Commands::Verify { .. }
        | Commands::Project { .. }
        | Commands::Register { .. }
        | Commands::Run => {
            unreachable!()
        }
    }
}

/// Serialize command parameters to a JSON search_params string.
fn search_params_for(cmd: &Commands) -> String {
    match cmd {
        Commands::Factorial { start, end } => {
            serde_json::json!({"form": "factorial", "start": start, "end": end}).to_string()
        }
        Commands::Palindromic { base, min_digits, max_digits } => serde_json::json!({
            "form": "palindromic", "base": base, "min_digits": min_digits, "max_digits": max_digits
        }).to_string(),
        Commands::Kbn { k, base, min_n, max_n } => serde_json::json!({
            "form": "kbn", "k": k, "base": base, "min_n": min_n, "max_n": max_n
        }).to_string(),
        Commands::NearRepdigit { min_digits, max_digits } => serde_json::json!({
            "form": "near_repdigit", "min_digits": min_digits, "max_digits": max_digits
        }).to_string(),
        Commands::Primorial { start, end } => {
            serde_json::json!({"form": "primorial", "start": start, "end": end}).to_string()
        }
        Commands::CullenWoodall { min_n, max_n } => {
            serde_json::json!({"form": "cullen_woodall", "min_n": min_n, "max_n": max_n}).to_string()
        }
        Commands::Wagstaff { min_exp, max_exp } => {
            serde_json::json!({"form": "wagstaff", "min_exp": min_exp, "max_exp": max_exp}).to_string()
        }
        Commands::CarolKynea { min_n, max_n } => {
            serde_json::json!({"form": "carol_kynea", "min_n": min_n, "max_n": max_n}).to_string()
        }
        Commands::Twin { k, base, min_n, max_n } => serde_json::json!({
            "form": "twin", "k": k, "base": base, "min_n": min_n, "max_n": max_n
        }).to_string(),
        Commands::SophieGermain { k, base, min_n, max_n } => serde_json::json!({
            "form": "sophie_germain", "k": k, "base": base, "min_n": min_n, "max_n": max_n
        }).to_string(),
        Commands::Repunit { base, min_n, max_n } => serde_json::json!({
            "form": "repunit", "base": base, "min_n": min_n, "max_n": max_n
        }).to_string(),
        Commands::GenFermat { fermat_exp, min_base, max_base } => serde_json::json!({
            "form": "gen_fermat", "fermat_exp": fermat_exp, "min_base": min_base, "max_base": max_base
        }).to_string(),
        Commands::Dashboard { .. }
        | Commands::Work { .. }
        | Commands::Verify { .. }
        | Commands::Project { .. }
        | Commands::Register { .. }
        | Commands::Run => {
            unreachable!()
        }
    }
}

/// Dispatch a CLI command variant to the corresponding engine search function.
fn dispatch_search(
    cmd: &Commands,
    progress: &Arc<progress::Progress>,
    db: &Arc<db::Database>,
    rt_handle: &tokio::runtime::Handle,
    checkpoint_path: &std::path::Path,
    search_params: &str,
    mr: u32,
    sl: u64,
    coord: Option<&dyn CoordinationClient>,
    eb: Option<&events::EventBus>,
) -> Result<()> {
    match cmd {
        Commands::Factorial { start, end } => factorial::search(
            *start,
            *end,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::Palindromic {
            base,
            min_digits,
            max_digits,
        } => palindromic::search(
            *base,
            *min_digits,
            *max_digits,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::Kbn {
            k,
            base,
            min_n,
            max_n,
        } => kbn::search(
            *k,
            *base,
            *min_n,
            *max_n,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::NearRepdigit {
            min_digits,
            max_digits,
        } => near_repdigit::search(
            *min_digits,
            *max_digits,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::Primorial { start, end } => primorial::search(
            *start,
            *end,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::CullenWoodall { min_n, max_n } => cullen_woodall::search(
            *min_n,
            *max_n,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::Wagstaff { min_exp, max_exp } => wagstaff::search(
            *min_exp,
            *max_exp,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::CarolKynea { min_n, max_n } => carol_kynea::search(
            *min_n,
            *max_n,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::Twin {
            k,
            base,
            min_n,
            max_n,
        } => twin::search(
            *k,
            *base,
            *min_n,
            *max_n,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::SophieGermain {
            k,
            base,
            min_n,
            max_n,
        } => sophie_germain::search(
            *k,
            *base,
            *min_n,
            *max_n,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::Repunit { base, min_n, max_n } => repunit::search(
            *base,
            *min_n,
            *max_n,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::GenFermat {
            fermat_exp,
            min_base,
            max_base,
        } => gen_fermat::search(
            *fermat_exp,
            *min_base,
            *max_base,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::Dashboard { .. }
        | Commands::Work { .. }
        | Commands::Verify { .. }
        | Commands::Project { .. }
        | Commands::Register { .. }
        | Commands::Run => {
            unreachable!()
        }
    }
}

/// Spawn a background thread that syncs Progress atomics into a worker client's atomics.
fn sync_progress_to_atomics(
    progress: &Arc<progress::Progress>,
    tested: &Arc<std::sync::atomic::AtomicU64>,
    found: &Arc<std::sync::atomic::AtomicU64>,
    current: &Arc<std::sync::Mutex<String>>,
) {
    let wc_tested = Arc::clone(tested);
    let wc_found = Arc::clone(found);
    let wc_current = Arc::clone(current);
    let progress_ref = Arc::clone(progress);
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        wc_tested.store(
            progress_ref
                .tested
                .load(std::sync::atomic::Ordering::Relaxed),
            std::sync::atomic::Ordering::Relaxed,
        );
        wc_found.store(
            progress_ref
                .found
                .load(std::sync::atomic::Ordering::Relaxed),
            std::sync::atomic::Ordering::Relaxed,
        );
        *wc_current.lock().unwrap() = progress_ref.current.lock().unwrap().clone();
    });
}

/// Get the system hostname, falling back to "worker".
fn get_hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "worker".to_string())
}

// ── Work Loop ───────────────────────────────────────────────────

/// Block-claiming work loop for the `work` subcommand.
pub fn run_work_loop(
    cli: &Cli,
    db: &Arc<db::Database>,
    rt_handle: &tokio::runtime::Handle,
    search_job_id: i64,
) -> Result<()> {
    let worker_id = cli.worker_id.clone().unwrap_or_else(get_hostname);

    let job = rt_handle
        .block_on(db.get_search_job(search_job_id))?
        .ok_or_else(|| anyhow::anyhow!("Search job {} not found", search_job_id))?;

    info!(
        search_job_id,
        search_type = %job.search_type,
        range_start = job.range_start,
        range_end = job.range_end,
        "Work mode started"
    );

    let search_params_str = serde_json::to_string(&job.params)?;
    let pg_client = pg_worker::PgWorkerClient::new(
        db.pool().clone(),
        rt_handle.clone(),
        &worker_id,
        &job.search_type,
        &search_params_str,
    );

    let progress = progress::Progress::new();
    let reporter_handle = progress.start_reporter();

    sync_progress_to_atomics(
        &progress,
        &pg_client.tested,
        &pg_client.found,
        &pg_client.current,
    );

    let heartbeat_handle = pg_client.start_heartbeat();
    let coord: Option<&dyn CoordinationClient> = Some(&pg_client);

    let mr = cli.mr_rounds;
    let sl = cli.sieve_limit;
    let mut blocks_completed = 0u64;
    let batch_size = 5;
    let mut pending_blocks: std::collections::VecDeque<db::WorkBlockWithCheckpoint> =
        std::collections::VecDeque::new();

    loop {
        if pg_client.is_stop_requested() {
            info!("Stop requested, exiting work loop");
            break;
        }

        // Batch claim blocks when the local queue is empty
        if pending_blocks.is_empty() {
            let blocks = rt_handle.block_on(
                db.claim_work_blocks(search_job_id, &worker_id, batch_size),
            )?;
            if blocks.is_empty() {
                info!("No more blocks available, work complete");
                break;
            }
            info!(count = blocks.len(), "Claimed blocks (batch)");
            pending_blocks.extend(blocks);
        }

        let block = pending_blocks.pop_front().unwrap();

        // Tell the heartbeat thread which block we're working on
        *pg_client.current_block_id.lock().unwrap() = Some(block.block_id);

        // Resume from checkpoint if the block was reclaimed mid-progress
        let effective_start = block
            .block_checkpoint
            .as_ref()
            .and_then(|cp| cp.get("last_tested").and_then(|v| v.as_i64()))
            .map(|last| last + 1)
            .unwrap_or(block.block_start);

        if effective_start > block.block_start {
            info!(
                block_id = block.block_id,
                effective_start,
                block_end = block.block_end,
                original_start = block.block_start,
                "Block resuming from checkpoint"
            );
        } else {
            info!(
                block_id = block.block_id,
                block_start = block.block_start,
                block_end = block.block_end,
                "Processing block"
            );
        }

        progress
            .tested
            .store(0, std::sync::atomic::Ordering::Relaxed);
        progress
            .found
            .store(0, std::sync::atomic::Ordering::Relaxed);

        let span = info_span!(
            "search_block",
            job_id = search_job_id,
            form = %job.search_type,
            block_id = block.block_id,
            range_start = effective_start,
            range_end = block.block_end,
        );
        let block_result = span.in_scope(|| {
            run_search_block(
                &job.search_type,
                &job.params,
                effective_start,
                block.block_end,
                &progress,
                db,
                rt_handle,
                &cli.checkpoint,
                mr,
                sl,
                coord,
            )
        });

        *pg_client.current_block_id.lock().unwrap() = None;

        let tested = progress.tested.load(std::sync::atomic::Ordering::Relaxed);
        let found = progress.found.load(std::sync::atomic::Ordering::Relaxed);

        match block_result {
            Ok(()) => {
                rt_handle.block_on(db.complete_work_block(
                    block.block_id,
                    tested as i64,
                    found as i64,
                ))?;
                blocks_completed += 1;
                info!(
                    block_id = block.block_id,
                    tested,
                    found,
                    "Block completed"
                );
            }
            Err(e) => {
                warn!(block_id = block.block_id, error = %e, "Block failed");
                rt_handle.block_on(db.fail_work_block(block.block_id))?;
            }
        }
    }

    progress.stop();
    let _ = reporter_handle.join();
    pg_client.deregister();
    let _ = heartbeat_handle.join();

    info!(blocks_completed, "Work loop finished");

    let summary = rt_handle.block_on(db.get_job_block_summary(search_job_id))?;
    if summary.available == 0 && summary.claimed == 0 {
        rt_handle.block_on(db.update_search_job_status(search_job_id, "completed", None))?;
        info!(search_job_id, "Search job marked completed");
    }

    Ok(())
}

/// Dispatch a single block to the appropriate search function.
fn run_search_block(
    search_type: &str,
    params: &serde_json::Value,
    block_start: i64,
    block_end: i64,
    progress: &Arc<progress::Progress>,
    db: &Arc<db::Database>,
    rt_handle: &tokio::runtime::Handle,
    checkpoint_path: &std::path::Path,
    mr: u32,
    sl: u64,
    coord: Option<&dyn CoordinationClient>,
) -> Result<()> {
    let sp = serde_json::to_string(params)?;
    let start = block_start as u64;
    let end = block_end as u64;
    let eb: Option<&events::EventBus> = None;

    match search_type {
        "factorial" => factorial::search(
            start,
            end,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            &sp,
            mr,
            sl,
            coord,
            eb,
        ),
        "primorial" => primorial::search(
            start,
            end,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            &sp,
            mr,
            sl,
            coord,
            eb,
        ),
        "palindromic" => {
            let base = params["base"].as_u64().unwrap_or(10) as u32;
            palindromic::search(
                base,
                start,
                end,
                progress,
                db,
                rt_handle,
                checkpoint_path,
                &sp,
                mr,
                sl,
                coord,
                eb,
            )
        }
        "near_repdigit" => near_repdigit::search(
            start,
            end,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            &sp,
            mr,
            sl,
            coord,
            eb,
        ),
        "kbn" => {
            let k = params["k"].as_u64().unwrap_or(1);
            let base = params["base"].as_u64().unwrap_or(2) as u32;
            kbn::search(
                k,
                base,
                start,
                end,
                progress,
                db,
                rt_handle,
                checkpoint_path,
                &sp,
                mr,
                sl,
                coord,
                eb,
            )
        }
        "cullen_woodall" => cullen_woodall::search(
            start,
            end,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            &sp,
            mr,
            sl,
            coord,
            eb,
        ),
        "wagstaff" => wagstaff::search(
            start,
            end,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            &sp,
            mr,
            sl,
            coord,
            eb,
        ),
        "carol_kynea" => carol_kynea::search(
            start,
            end,
            progress,
            db,
            rt_handle,
            checkpoint_path,
            &sp,
            mr,
            sl,
            coord,
            eb,
        ),
        "twin" => {
            let k = params["k"].as_u64().unwrap_or(1);
            let base = params["base"].as_u64().unwrap_or(2) as u32;
            twin::search(
                k,
                base,
                start,
                end,
                progress,
                db,
                rt_handle,
                checkpoint_path,
                &sp,
                mr,
                sl,
                coord,
                eb,
            )
        }
        "sophie_germain" => {
            let k = params["k"].as_u64().unwrap_or(1);
            let base = params["base"].as_u64().unwrap_or(2) as u32;
            sophie_germain::search(
                k,
                base,
                start,
                end,
                progress,
                db,
                rt_handle,
                checkpoint_path,
                &sp,
                mr,
                sl,
                coord,
                eb,
            )
        }
        "repunit" => {
            let base = params["base"].as_u64().unwrap_or(10) as u32;
            repunit::search(
                base,
                start,
                end,
                progress,
                db,
                rt_handle,
                checkpoint_path,
                &sp,
                mr,
                sl,
                coord,
                eb,
            )
        }
        "gen_fermat" => {
            let fermat_exp = params["fermat_exp"].as_u64().unwrap_or(1) as u32;
            gen_fermat::search(
                fermat_exp,
                start,
                end,
                progress,
                db,
                rt_handle,
                checkpoint_path,
                &sp,
                mr,
                sl,
                coord,
                eb,
            )
        }
        other => Err(anyhow::anyhow!("Unknown search type: {}", other)),
    }
}

// ── Verification ────────────────────────────────────────────────

/// Run the verify subcommand.
pub fn run_verify(
    rt: &tokio::runtime::Runtime,
    db: &db::Database,
    id: Option<i64>,
    all: bool,
    form: Option<&str>,
    batch_size: i64,
    force: bool,
    tool: &str,
) -> Result<()> {
    let primes = if let Some(id) = id {
        match rt.block_on(db.get_prime_by_id(id))? {
            Some(p) => vec![p],
            None => {
                eprintln!("Prime with id {} not found", id);
                return Ok(());
            }
        }
    } else if all || form.is_some() {
        rt.block_on(db.get_unverified_primes_filtered(batch_size, form, force))?
    } else {
        eprintln!("Specify --id <ID>, --all, or --form <FORM>");
        return Ok(());
    };

    if primes.is_empty() {
        eprintln!("No primes to verify");
        return Ok(());
    }

    eprintln!("Verifying {} primes...", primes.len());
    eprintln!(
        "{:<8} {:<40} {:<12} {:<12} Status",
        "ID", "Expression", "Tier 1", "Tier 2"
    );
    eprintln!("{}", "-".repeat(90));

    let mut verified = 0u64;
    let mut failed = 0u64;
    let mut skipped = 0u64;

    for prime in &primes {
        let result = if tool == "pfgw" {
            let candidate = verify::reconstruct_candidate(&prime.form, &prime.expression);
            match candidate {
                Ok(c) => verify::verify_pfgw(&prime.form, &prime.expression, &c),
                Err(e) => verify::VerifyResult::Failed {
                    reason: format!("Cannot reconstruct: {}", e),
                },
            }
        } else {
            verify::verify_prime(prime)
        };

        let expr_display = if prime.expression.len() > 38 {
            format!("{}...", &prime.expression[..35])
        } else {
            prime.expression.clone()
        };

        match &result {
            verify::VerifyResult::Verified { method, tier } => {
                let t1 = if *tier == 1 { "PASS" } else { "skip" };
                let t2 = if *tier == 2 { "PASS" } else { "-" };
                eprintln!(
                    "{:<8} {:<40} {:<12} {:<12} VERIFIED ({})",
                    prime.id, expr_display, t1, t2, method
                );
                rt.block_on(db.mark_verified(prime.id, method, *tier as i16))?;
                verified += 1;
            }
            verify::VerifyResult::Failed { reason } => {
                eprintln!(
                    "{:<8} {:<40} {:<12} {:<12} FAILED: {}",
                    prime.id, expr_display, "FAIL", "-", reason
                );
                rt.block_on(db.mark_verification_failed(prime.id, reason))?;
                failed += 1;
            }
            verify::VerifyResult::Skipped { reason } => {
                eprintln!(
                    "{:<8} {:<40} {:<12} {:<12} SKIPPED: {}",
                    prime.id, expr_display, "skip", "-", reason
                );
                skipped += 1;
            }
        }
    }

    eprintln!(
        "\nSummary: {} verified, {} failed, {} skipped",
        verified, failed, skipped
    );
    Ok(())
}

// ── Project Management ──────────────────────────────────────────

/// Handle the `project` subcommand and its actions.
pub fn run_project(cli: &Cli, action: &ProjectAction) -> Result<()> {
    // Estimate doesn't need a database connection
    if let ProjectAction::Estimate { file } = action {
        let config = project::parse_toml_file(file)?;
        let est = project::estimate_project_cost(&config);
        eprintln!("Cost estimate for '{}':", config.project.name);
        eprintln!("  Form:                {}", config.project.form);
        eprintln!("  Objective:           {}", config.project.objective);
        eprintln!("  Est. candidates:     {}", est.estimated_candidates);
        eprintln!("  Est. core-hours:     {:.1}", est.total_core_hours);
        eprintln!("  Est. cost (USD):     ${:.2}", est.total_cost_usd);
        eprintln!(
            "  Est. duration:       {:.1}h with {} workers",
            est.estimated_duration_hours, est.workers_recommended
        );
        return Ok(());
    }

    let database_url = cli.database_url.as_deref().ok_or_else(|| {
        anyhow::anyhow!("DATABASE_URL is required (set via --database-url or env)")
    })?;
    let rt = tokio::runtime::Runtime::new()?;
    let database = rt.block_on(db::Database::connect(database_url))?;

    match action {
        ProjectAction::Import { file } => {
            let toml_content = std::fs::read_to_string(file)?;
            let config = project::parse_toml(&toml_content)?;
            let id = rt.block_on(database.create_project(&config, Some(&toml_content)))?;
            let slug = project::slugify(&config.project.name);
            eprintln!(
                "Project '{}' created (id={}, slug={})",
                config.project.name, id, slug
            );
            eprintln!("  Objective: {}", config.project.objective);
            eprintln!("  Form:      {}", config.project.form);
            let phases = if config.strategy.auto_strategy && config.strategy.phases.is_empty() {
                project::generate_auto_strategy(&config)
            } else {
                config.strategy.phases.clone()
            };
            eprintln!("  Phases:    {}", phases.len());
            for phase in &phases {
                eprintln!("    - {}", phase.name);
            }
            eprintln!("\nActivate with: darkreach project activate {}", slug);
        }
        ProjectAction::List { status } => {
            let projects = rt.block_on(database.get_projects(status.as_deref()))?;
            if projects.is_empty() {
                eprintln!("No projects found");
                return Ok(());
            }
            eprintln!(
                "{:<30} {:<12} {:<12} {:<12} {:<10} {:<10}",
                "SLUG", "STATUS", "FORM", "OBJECTIVE", "TESTED", "FOUND"
            );
            eprintln!("{}", "-".repeat(86));
            for p in &projects {
                eprintln!(
                    "{:<30} {:<12} {:<12} {:<12} {:<10} {:<10}",
                    p.slug, p.status, p.form, p.objective, p.total_tested, p.total_found
                );
            }
        }
        ProjectAction::Show { slug } => {
            let proj = rt
                .block_on(database.get_project_by_slug(slug))?
                .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", slug))?;
            let phases = rt.block_on(database.get_project_phases(proj.id))?;
            let events = rt.block_on(database.get_project_events(proj.id, 10))?;

            eprintln!("Project: {} ({})", proj.name, proj.slug);
            eprintln!("  Status:      {}", proj.status);
            eprintln!("  Objective:   {}", proj.objective);
            eprintln!("  Form:        {}", proj.form);
            eprintln!("  Total tested: {}", proj.total_tested);
            eprintln!("  Total found:  {}", proj.total_found);
            eprintln!("  Best digits:  {}", proj.best_digits);
            eprintln!("  Core hours:   {:.1}", proj.total_core_hours);
            eprintln!("  Cost (USD):   ${:.2}", proj.total_cost_usd);
            eprintln!("  Created:     {}", proj.created_at);

            eprintln!("\nPhases ({}):", phases.len());
            for phase in &phases {
                let job_str = phase
                    .search_job_id
                    .map(|id| format!(" → job {}", id))
                    .unwrap_or_default();
                eprintln!(
                    "  [{:>9}] {}{} (tested={}, found={})",
                    phase.status, phase.name, job_str, phase.total_tested, phase.total_found
                );
            }

            if !events.is_empty() {
                eprintln!("\nRecent events:");
                for evt in &events {
                    eprintln!(
                        "  [{}] {}: {}",
                        evt.created_at.format("%Y-%m-%d %H:%M"),
                        evt.event_type,
                        evt.summary
                    );
                }
            }
        }
        ProjectAction::Activate { slug } => {
            let proj = rt
                .block_on(database.get_project_by_slug(slug))?
                .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", slug))?;
            if proj.status != "draft" && proj.status != "paused" {
                anyhow::bail!(
                    "Cannot activate project with status '{}' (must be 'draft' or 'paused')",
                    proj.status
                );
            }
            rt.block_on(database.update_project_status(proj.id, "active"))?;
            rt.block_on(database.insert_project_event(
                proj.id,
                "activated",
                &format!("Project '{}' activated", proj.name),
                None,
            ))?;
            eprintln!(
                "Project '{}' activated. Orchestration will start on next dashboard tick.",
                slug
            );
        }
        ProjectAction::Pause { slug } => {
            let proj = rt
                .block_on(database.get_project_by_slug(slug))?
                .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", slug))?;
            if proj.status != "active" {
                anyhow::bail!(
                    "Cannot pause project with status '{}' (must be 'active')",
                    proj.status
                );
            }
            rt.block_on(database.update_project_status(proj.id, "paused"))?;
            rt.block_on(database.insert_project_event(
                proj.id,
                "paused",
                &format!("Project '{}' paused", proj.name),
                None,
            ))?;
            eprintln!("Project '{}' paused", slug);
        }
        ProjectAction::Cancel { slug } => {
            let proj = rt
                .block_on(database.get_project_by_slug(slug))?
                .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", slug))?;
            rt.block_on(database.update_project_status(proj.id, "cancelled"))?;
            rt.block_on(database.insert_project_event(
                proj.id,
                "cancelled",
                &format!("Project '{}' cancelled", proj.name),
                None,
            ))?;
            eprintln!("Project '{}' cancelled", slug);
        }
        ProjectAction::RefreshRecords => {
            let updated = rt.block_on(project::refresh_all_records(&database))?;
            eprintln!("Refreshed {} records", updated);

            let records = rt.block_on(database.get_records())?;
            if !records.is_empty() {
                eprintln!(
                    "\n{:<18} {:<40} {:<12} {:<15} {:<12}",
                    "FORM", "RECORD", "DIGITS", "HOLDER", "OUR BEST"
                );
                eprintln!("{}", "-".repeat(97));
                for r in &records {
                    let expr = if r.expression.len() > 38 {
                        format!("{}...", &r.expression[..35])
                    } else {
                        r.expression.clone()
                    };
                    let our = if r.our_best_digits > 0 {
                        format!("{}", r.our_best_digits)
                    } else {
                        "-".to_string()
                    };
                    eprintln!(
                        "{:<18} {:<40} {:<12} {:<15} {:<12}",
                        r.form,
                        expr,
                        r.digits,
                        r.holder.as_deref().unwrap_or("-"),
                        our
                    );
                }
            }
        }
        ProjectAction::Estimate { .. } => unreachable!(),
    }

    Ok(())
}

// ── Operator Commands ───────────────────────────────────────────

/// Register as an operator and receive an API key.
pub fn run_register(server: &str, username: &str, email: &str) -> Result<()> {
    use darkreach::operator;

    info!(server, "Registering with coordinator");
    let config = operator::register(server, username, email)?;
    eprintln!("Registration successful!");
    eprintln!("  Username:  {}", config.username);
    eprintln!("  API Key:   {}", config.api_key);
    eprintln!("  Worker ID: {}", config.worker_id);
    eprintln!("\nConfig saved to ~/.darkreach/config.toml");
    eprintln!("Run `darkreach run` to start contributing.");
    Ok(())
}

/// Run the operator work loop (claim → compute → submit → repeat).
pub fn run_operator(cli: &Cli) -> Result<()> {
    use darkreach::{progress, operator};

    let config = operator::load_config()?;
    operator::register_worker(&config)?;
    let update_channel =
        std::env::var("DARKREACH_UPDATE_CHANNEL").unwrap_or_else(|_| "stable".to_string());
    let auto_update = std::env::var("DARKREACH_AUTO_UPDATE")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let apply_update = std::env::var("DARKREACH_AUTO_UPDATE_APPLY")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    match operator::check_for_update(&config, &update_channel) {
        Ok(Some(rel)) => {
            info!(
                current_version = env!("CARGO_PKG_VERSION"),
                latest_version = %rel.version,
                channel = %rel.channel,
                published_at = %rel.published_at,
                "Worker update available"
            );
            if auto_update {
                match operator::stage_or_apply_update(&rel, apply_update) {
                    Ok(res) => {
                        info!(
                            version = %res.version,
                            staged_binary = %res.staged_binary.display(),
                            applied = res.applied,
                            "Worker update staged"
                        );
                    }
                    Err(e) => {
                        warn!(error = %e, "Worker update staging failed");
                    }
                }
            }
        }
        Ok(None) => {
            info!(
                current_version = env!("CARGO_PKG_VERSION"),
                channel = %update_channel,
                "Worker is up to date"
            );
        }
        Err(e) => {
            warn!(error = %e, channel = %update_channel, "Worker update check failed");
        }
    }

    let database_url = cli.database_url.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "DATABASE_URL is required for operator mode (set via --database-url or env)"
        )
    })?;
    let rt = tokio::runtime::Runtime::new()?;
    let database = rt.block_on(db::Database::connect(database_url))?;
    let db = Arc::new(database);

    let cores = cli.threads.unwrap_or_else(rayon::current_num_threads);

    info!(
        server = %config.server,
        username = %config.username,
        cores = cores,
        "Operator mode starting"
    );

    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // SIGTERM/SIGINT handler
    {
        let stop_flag = Arc::clone(&stop);
        std::thread::spawn(move || {
            let sig_rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("signal handler runtime");
            sig_rt.block_on(async {
                let ctrl_c = tokio::signal::ctrl_c();
                #[cfg(unix)]
                {
                    let mut sigterm =
                        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                            .expect("SIGTERM handler");
                    tokio::select! {
                        _ = ctrl_c => {},
                        _ = sigterm.recv() => {},
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = ctrl_c.await;
                }
                stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
            });
        });
    }

    let mut blocks_completed = 0u64;

    loop {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            info!("Stop requested, exiting operator loop");
            break;
        }

        if let Err(e) = operator::heartbeat(&config) {
            warn!(error = %e, "Heartbeat failed");
        }

        let assignment = match operator::claim_work(&config, cores) {
            Ok(Some(a)) => a,
            Ok(None) => {
                info!("No work available, sleeping 30s...");
                std::thread::sleep(std::time::Duration::from_secs(30));
                continue;
            }
            Err(e) => {
                warn!(error = %e, "Failed to claim work, retrying in 30s");
                std::thread::sleep(std::time::Duration::from_secs(30));
                continue;
            }
        };

        info!(
            block_id = assignment.block_id,
            search_type = %assignment.search_type,
            "Claimed work block"
        );

        let prog = progress::Progress::new();
        let reporter = prog.start_reporter();
        let checkpoint = std::path::PathBuf::from(format!(
            "operator_block_{}.checkpoint",
            assignment.block_id
        ));

        let span = info_span!(
            "search_block",
            block_id = assignment.block_id,
            form = %assignment.search_type,
            range_start = assignment.block_start,
            range_end = assignment.block_end,
        );
        let result = span.in_scope(|| {
            run_search_block(
                &assignment.search_type,
                &assignment.params,
                assignment.block_start,
                assignment.block_end,
                &prog,
                &db,
                rt.handle(),
                &checkpoint,
                cli.mr_rounds,
                cli.sieve_limit,
                None,
            )
        });

        let tested = prog.tested.load(std::sync::atomic::Ordering::Relaxed);
        let found = prog.found.load(std::sync::atomic::Ordering::Relaxed);

        prog.stop();
        let _ = reporter.join();

        let submission = operator::ResultSubmission {
            block_id: assignment.block_id,
            tested: tested as i64,
            found: found as i64,
            primes: vec![],
        };

        match operator::submit_result(&config, &submission) {
            Ok(()) => {
                blocks_completed += 1;
                info!(
                    block_id = assignment.block_id,
                    tested, found, "Block result submitted"
                );
            }
            Err(e) => {
                warn!(block_id = assignment.block_id, error = %e, "Failed to submit result");
            }
        }

        let _ = std::fs::remove_file(&checkpoint);

        if let Err(e) = result {
            warn!(error = %e, "Block search failed");
        }
    }

    info!(blocks_completed, "Operator loop finished");
    Ok(())
}

// ── Rayon Configuration ─────────────────────────────────────────

/// Configure the rayon global thread pool with optional QoS and thread count.
pub fn configure_rayon(threads: Option<usize>, qos: bool) {
    let num_threads = threads.unwrap_or(0);

    #[cfg(target_os = "macos")]
    if qos {
        let result = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .spawn_handler(|thread| {
                std::thread::Builder::new().spawn(move || {
                    // SAFETY: pthread_set_qos_class_self_np is a well-defined macOS API
                    // that sets the QoS class for the current thread. No memory safety concerns.
                    unsafe {
                        libc::pthread_set_qos_class_self_np(
                            libc::qos_class_t::QOS_CLASS_USER_INITIATED,
                            0,
                        );
                    }
                    thread.run();
                })?;
                Ok(())
            })
            .build_global();

        match result {
            Ok(()) => {
                info!("Rayon threads configured with macOS QoS: user-initiated (P-core scheduling)");
            }
            Err(e) => {
                warn!(error = %e, "Could not configure rayon thread pool");
            }
        }
        return;
    }

    #[cfg(not(target_os = "macos"))]
    if qos {
        warn!("--qos flag is only effective on macOS, ignoring");
    }

    if num_threads > 0 {
        if let Err(e) = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build_global()
        {
            warn!(error = %e, "Could not configure rayon thread pool");
        }
    }
}
