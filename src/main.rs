//! # Main — CLI Entry Point and Search Orchestration
//!
//! Routes CLI subcommands to engine search functions and infrastructure services.
//! Handles shared concerns: database connection, checkpoint loading, progress
//! reporting, worker coordination, and the Rayon thread pool configuration.
//!
//! ## Subcommands
//!
//! Each engine form has a corresponding subcommand (factorial, kbn, palindromic,
//! primorial, cullen_woodall, wagstaff, carol_kynea, twin, sophie_germain,
//! repunit, gen_fermat, near_repdigit). The `dashboard` subcommand starts the
//! web server. The `work` subcommand connects to a search job via PostgreSQL.
//!
//! ## Global Options
//!
//! - `--database-url` / `DATABASE_URL`: PostgreSQL connection for prime storage.
//! - `--checkpoint`: JSON file for resumable search state.
//! - `--mr-rounds`: Miller–Rabin iterations (default 15).
//! - `--sieve-limit`: Sieve depth (0 = auto-tune per GIMPS heuristic).
//! - `--qos`: macOS QoS P-core scheduling via `pthread_set_qos_class_self_np`.
//! - `--threads`: Rayon thread pool size (0 = all cores).

use anyhow::Result;
use clap::{Parser, Subcommand};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
use primehunt::{
    carol_kynea, cullen_woodall, db, events, factorial, gen_fermat, kbn, near_repdigit,
    palindromic, pg_worker, primorial, progress, project, repunit, sophie_germain, twin, verify,
    wagstaff, worker_client, CoordinationClient,
};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "primehunt", about = "Hunt for special-form prime numbers")]
struct Cli {
    /// PostgreSQL connection URL (or set DATABASE_URL env var)
    #[arg(long, env = "DATABASE_URL")]
    database_url: Option<String>,

    /// Path to checkpoint file for resuming searches
    #[arg(long, default_value = "primehunt.checkpoint")]
    checkpoint: PathBuf,

    /// Miller-Rabin rounds for primality testing (default: 15, higher = more certain but slower)
    #[arg(long, default_value_t = 15)]
    mr_rounds: u32,

    /// Sieve limit for generating small primes used in modular pre-filtering.
    /// Set to 0 for auto-tuning based on candidate size.
    #[arg(long, default_value_t = 0)]
    sieve_limit: u64,

    /// Minimum digit count to use PRST for primality testing (0 to disable)
    #[arg(long, default_value_t = 10_000)]
    prst_min_digits: u64,

    /// Path to PRST binary (auto-detected from PATH if not set)
    #[arg(long)]
    prst_path: Option<PathBuf>,

    /// Minimum digit count to use PFGW for primality testing (0 to disable)
    #[arg(long, default_value_t = 10_000)]
    pfgw_min_digits: u64,

    /// Path to PFGW binary (auto-detected from PATH if not set)
    #[arg(long)]
    pfgw_path: Option<PathBuf>,

    /// Coordinator dashboard URL (e.g. http://coordinator:8080). When set, worker registers
    /// itself, heartbeats progress, and reports found primes to the coordinator.
    #[arg(long)]
    coordinator: Option<String>,

    /// Worker ID for fleet identification (defaults to hostname)
    #[arg(long)]
    worker_id: Option<String>,

    /// Set macOS QoS class to user-initiated for rayon threads (P-core scheduling on Apple Silicon)
    #[arg(long)]
    qos: bool,

    /// Number of rayon worker threads (defaults to all logical cores)
    #[arg(long)]
    threads: Option<usize>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Search for factorial primes (n! +/- 1)
    Factorial {
        /// Start of search range (n)
        #[arg(long)]
        start: u64,
        /// End of search range (n)
        #[arg(long)]
        end: u64,
    },
    /// Search for palindromic primes in a given base
    Palindromic {
        /// Number base (default 10)
        #[arg(long, default_value_t = 10)]
        base: u32,
        /// Minimum digit count
        #[arg(long)]
        min_digits: u64,
        /// Maximum digit count
        #[arg(long)]
        max_digits: u64,
    },
    /// Search for primes of form k*b^n +/- 1
    Kbn {
        /// Multiplier k
        #[arg(long)]
        k: u64,
        /// Base b
        #[arg(long)]
        base: u32,
        /// Minimum exponent n
        #[arg(long)]
        min_n: u64,
        /// Maximum exponent n
        #[arg(long)]
        max_n: u64,
    },
    /// Search for near-repdigit palindromic primes (all 9s with symmetric modifications)
    NearRepdigit {
        /// Minimum digit count (odd values only)
        #[arg(long)]
        min_digits: u64,
        /// Maximum digit count (odd values only)
        #[arg(long)]
        max_digits: u64,
    },
    /// Search for primorial primes (p# +/- 1)
    Primorial {
        /// Start of search range (smallest prime p to test)
        #[arg(long)]
        start: u64,
        /// End of search range (largest prime p to test)
        #[arg(long)]
        end: u64,
    },
    /// Search for Cullen primes (n*2^n + 1) and Woodall primes (n*2^n - 1)
    CullenWoodall {
        /// Minimum n value
        #[arg(long)]
        min_n: u64,
        /// Maximum n value
        #[arg(long)]
        max_n: u64,
    },
    /// Search for Wagstaff primes ((2^p + 1) / 3 for prime p)
    Wagstaff {
        /// Minimum prime exponent
        #[arg(long)]
        min_exp: u64,
        /// Maximum prime exponent
        #[arg(long)]
        max_exp: u64,
    },
    /// Search for Carol primes ((2^n-1)^2-2) and Kynea primes ((2^n+1)^2-2)
    CarolKynea {
        /// Minimum n value
        #[arg(long)]
        min_n: u64,
        /// Maximum n value
        #[arg(long)]
        max_n: u64,
    },
    /// Search for twin primes of form k*b^n +/- 1 (both must be prime)
    Twin {
        /// Multiplier k
        #[arg(long)]
        k: u64,
        /// Base b
        #[arg(long)]
        base: u32,
        /// Minimum exponent n
        #[arg(long)]
        min_n: u64,
        /// Maximum exponent n
        #[arg(long)]
        max_n: u64,
    },
    /// Search for Sophie Germain primes: p=k*b^n-1 where both p and 2p+1 are prime
    SophieGermain {
        /// Multiplier k
        #[arg(long)]
        k: u64,
        /// Base b
        #[arg(long)]
        base: u32,
        /// Minimum exponent n
        #[arg(long)]
        min_n: u64,
        /// Maximum exponent n
        #[arg(long)]
        max_n: u64,
    },
    /// Search for repunit primes: R(b,n) = (b^n-1)/(b-1) for prime n
    Repunit {
        /// Number base (default 10)
        #[arg(long, default_value_t = 10)]
        base: u32,
        /// Minimum exponent n (must be prime)
        #[arg(long)]
        min_n: u64,
        /// Maximum exponent n
        #[arg(long)]
        max_n: u64,
    },
    /// Search for generalized Fermat primes: b^(2^n) + 1 for even b
    GenFermat {
        /// Fermat exponent n (candidate = b^(2^n) + 1)
        #[arg(long)]
        fermat_exp: u32,
        /// Minimum base b (must be even)
        #[arg(long)]
        min_base: u64,
        /// Maximum base b
        #[arg(long)]
        max_base: u64,
    },
    /// Launch web dashboard to browse results and monitor searches
    Dashboard {
        /// Port to listen on
        #[arg(long, default_value_t = 8080)]
        port: u16,
        /// Directory to serve static files from (e.g. Next.js export)
        #[arg(long)]
        static_dir: Option<PathBuf>,
    },
    /// Claim and execute work blocks from a search job
    Work {
        /// Search job ID to claim blocks from
        #[arg(long)]
        search_job_id: i64,
    },
    /// Verify discovered primes
    Verify {
        /// Verify a specific prime by ID
        #[arg(long)]
        id: Option<i64>,
        /// Verify all unverified primes
        #[arg(long)]
        all: bool,
        /// Filter by prime form
        #[arg(long)]
        form: Option<String>,
        /// Max primes per batch
        #[arg(long, default_value_t = 100)]
        batch_size: i64,
        /// Re-verify even if already verified
        #[arg(long)]
        force: bool,
        /// Verification tool to use: "default" (tier1+tier2), "pfgw" (PFGW cross-verification)
        #[arg(long, default_value = "default")]
        tool: String,
    },
    /// Manage prime-hunting projects (campaigns with phases, budgets, records)
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
}

#[derive(Subcommand)]
enum ProjectAction {
    /// Import a project from a TOML file
    Import {
        /// Path to the TOML project file
        #[arg(long)]
        file: PathBuf,
    },
    /// List all projects
    List {
        /// Filter by status (draft, active, paused, completed, cancelled, failed)
        #[arg(long)]
        status: Option<String>,
    },
    /// Show project details
    Show {
        /// Project slug
        slug: String,
    },
    /// Activate a project (start orchestration)
    Activate {
        /// Project slug
        slug: String,
    },
    /// Pause a project
    Pause {
        /// Project slug
        slug: String,
    },
    /// Cancel a project
    Cancel {
        /// Project slug
        slug: String,
    },
    /// Estimate cost for a project TOML file
    Estimate {
        /// Path to the TOML project file
        #[arg(long)]
        file: PathBuf,
    },
    /// Refresh world records from t5k.org
    RefreshRecords,
}

fn main() -> Result<()> {
    // Load .env file if present (for DATABASE_URL etc.)
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();

    // Initialize PRST configuration (optional GWNUM-accelerated testing)
    primehunt::prst::init(
        cli.prst_min_digits,
        cli.prst_path.clone(),
        std::time::Duration::from_secs(3600),
    );

    // Initialize PFGW configuration (optional accelerated testing for non-kbn forms)
    primehunt::pfgw::init(
        cli.pfgw_min_digits,
        cli.pfgw_path.clone(),
        std::time::Duration::from_secs(3600),
    );

    // Configure rayon thread pool (optional QoS + thread count)
    configure_rayon(cli.threads, cli.qos);

    // Handle Project subcommand
    if let Commands::Project { action } = &cli.command {
        return run_project(&cli, action);
    }

    if let Commands::Dashboard { port, static_dir } = &cli.command {
        let database_url = cli.database_url.as_deref().ok_or_else(|| {
            anyhow::anyhow!("DATABASE_URL is required (set via --database-url or env)")
        })?;

        let rt = tokio::runtime::Runtime::new()?;
        return rt.block_on(primehunt::dashboard::run(
            *port,
            database_url,
            &cli.checkpoint,
            static_dir.as_deref(),
        ));
    }

    // Handle Verify subcommand
    if let Commands::Verify {
        id,
        all,
        form,
        batch_size,
        force,
        tool,
    } = &cli.command
    {
        let database_url = cli.database_url.as_deref().ok_or_else(|| {
            anyhow::anyhow!("DATABASE_URL is required (set via --database-url or env)")
        })?;
        let rt = tokio::runtime::Runtime::new()?;
        let database = rt.block_on(db::Database::connect(database_url))?;
        return run_verify(
            &rt,
            &database,
            *id,
            *all,
            form.as_deref(),
            *batch_size,
            *force,
            tool,
        );
    }

    let database_url = cli.database_url.as_deref().ok_or_else(|| {
        anyhow::anyhow!("DATABASE_URL is required (set via --database-url or env)")
    })?;

    let num_cores = rayon::current_num_threads();
    eprintln!(
        "primehunt starting with {} CPU cores, {} Miller-Rabin rounds",
        num_cores, cli.mr_rounds
    );

    // Create a tokio runtime for async DB operations from rayon threads
    let rt = tokio::runtime::Runtime::new()?;
    let database = rt.block_on(db::Database::connect(database_url))?;
    let db = Arc::new(database);
    let rt_handle = rt.handle().clone();

    // Handle the `work` subcommand (block-claiming loop)
    if let Commands::Work { search_job_id } = &cli.command {
        return run_work_loop(&cli, &db, &rt_handle, *search_job_id);
    }

    let progress = progress::Progress::new();
    let reporter_handle = progress.start_reporter();
    let event_bus = Arc::new(events::EventBus::new());

    let search_type = match &cli.command {
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
        | Commands::Project { .. } => {
            unreachable!()
        }
    };

    let search_params = match &cli.command {
        Commands::Factorial { start, end } => {
            serde_json::json!({"form": "factorial", "start": start, "end": end}).to_string()
        }
        Commands::Palindromic {
            base,
            min_digits,
            max_digits,
        } => serde_json::json!({
            "form": "palindromic",
            "base": base,
            "min_digits": min_digits,
            "max_digits": max_digits
        })
        .to_string(),
        Commands::Kbn {
            k,
            base,
            min_n,
            max_n,
        } => serde_json::json!({
            "form": "kbn",
            "k": k,
            "base": base,
            "min_n": min_n,
            "max_n": max_n
        })
        .to_string(),
        Commands::NearRepdigit {
            min_digits,
            max_digits,
        } => serde_json::json!({
            "form": "near_repdigit",
            "min_digits": min_digits,
            "max_digits": max_digits
        })
        .to_string(),
        Commands::Primorial { start, end } => {
            serde_json::json!({"form": "primorial", "start": start, "end": end}).to_string()
        }
        Commands::CullenWoodall { min_n, max_n } => {
            serde_json::json!({"form": "cullen_woodall", "min_n": min_n, "max_n": max_n})
                .to_string()
        }
        Commands::Wagstaff { min_exp, max_exp } => {
            serde_json::json!({"form": "wagstaff", "min_exp": min_exp, "max_exp": max_exp})
                .to_string()
        }
        Commands::CarolKynea { min_n, max_n } => {
            serde_json::json!({"form": "carol_kynea", "min_n": min_n, "max_n": max_n}).to_string()
        }
        Commands::Twin {
            k,
            base,
            min_n,
            max_n,
        } => serde_json::json!({
            "form": "twin",
            "k": k,
            "base": base,
            "min_n": min_n,
            "max_n": max_n
        })
        .to_string(),
        Commands::SophieGermain {
            k,
            base,
            min_n,
            max_n,
        } => serde_json::json!({
            "form": "sophie_germain",
            "k": k,
            "base": base,
            "min_n": min_n,
            "max_n": max_n
        })
        .to_string(),
        Commands::Repunit { base, min_n, max_n } => serde_json::json!({
            "form": "repunit",
            "base": base,
            "min_n": min_n,
            "max_n": max_n
        })
        .to_string(),
        Commands::GenFermat {
            fermat_exp,
            min_base,
            max_base,
        } => serde_json::json!({
            "form": "gen_fermat",
            "fermat_exp": fermat_exp,
            "min_base": min_base,
            "max_base": max_base
        })
        .to_string(),
        Commands::Dashboard { .. }
        | Commands::Work { .. }
        | Commands::Verify { .. }
        | Commands::Project { .. } => {
            unreachable!()
        }
    };

    // Set up coordinator connection: --coordinator (HTTP) or DATABASE_URL (PG)
    let worker_id = cli.worker_id.unwrap_or_else(|| {
        std::process::Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "worker".to_string())
    });

    // Build a coordination client: HTTP if --coordinator is set, else PG
    enum Client {
        Http(worker_client::WorkerClient),
        Pg(pg_worker::PgWorkerClient),
    }

    let client = if let Some(url) = &cli.coordinator {
        let c = worker_client::WorkerClient::new(url, &worker_id, search_type, &search_params);
        Some(Client::Http(c))
    } else {
        let pg = pg_worker::PgWorkerClient::new(
            db.pool().clone(),
            rt_handle.clone(),
            &worker_id,
            search_type,
            &search_params,
        );
        Some(Client::Pg(pg))
    };

    // Sync progress counters into the client's atomics (for both HTTP and PG)
    match &client {
        Some(Client::Http(c)) => {
            let wc_tested = Arc::clone(&c.tested);
            let wc_found = Arc::clone(&c.found);
            let wc_current = Arc::clone(&c.current);
            let progress_ref = Arc::clone(&progress);
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
        Some(Client::Pg(c)) => {
            let wc_tested = Arc::clone(&c.tested);
            let wc_found = Arc::clone(&c.found);
            let wc_current = Arc::clone(&c.current);
            let progress_ref = Arc::clone(&progress);
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
        None => {}
    }

    let heartbeat_handle = match &client {
        Some(Client::Http(c)) => Some(c.start_heartbeat()),
        Some(Client::Pg(c)) => Some(c.start_heartbeat()),
        None => None,
    };

    // Get a trait object reference for the search functions
    let coord: Option<&dyn CoordinationClient> = match &client {
        Some(Client::Http(c)) => Some(c),
        Some(Client::Pg(c)) => Some(c),
        None => None,
    };

    let mr = cli.mr_rounds;
    let sl = cli.sieve_limit;
    let eb = Some(event_bus.as_ref() as &events::EventBus);

    event_bus.emit(events::Event::SearchStarted {
        search_type: search_type.to_string(),
        params: search_params.clone(),
        timestamp: std::time::Instant::now(),
    });

    let search_start = std::time::Instant::now();
    let result = match cli.command {
        Commands::Factorial { start, end } => factorial::search(
            start,
            end,
            &progress,
            &db,
            &rt_handle,
            &cli.checkpoint,
            &search_params,
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
            base,
            min_digits,
            max_digits,
            &progress,
            &db,
            &rt_handle,
            &cli.checkpoint,
            &search_params,
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
            k,
            base,
            min_n,
            max_n,
            &progress,
            &db,
            &rt_handle,
            &cli.checkpoint,
            &search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::NearRepdigit {
            min_digits,
            max_digits,
        } => near_repdigit::search(
            min_digits,
            max_digits,
            &progress,
            &db,
            &rt_handle,
            &cli.checkpoint,
            &search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::Primorial { start, end } => primorial::search(
            start,
            end,
            &progress,
            &db,
            &rt_handle,
            &cli.checkpoint,
            &search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::CullenWoodall { min_n, max_n } => cullen_woodall::search(
            min_n,
            max_n,
            &progress,
            &db,
            &rt_handle,
            &cli.checkpoint,
            &search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::Wagstaff { min_exp, max_exp } => wagstaff::search(
            min_exp,
            max_exp,
            &progress,
            &db,
            &rt_handle,
            &cli.checkpoint,
            &search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::CarolKynea { min_n, max_n } => carol_kynea::search(
            min_n,
            max_n,
            &progress,
            &db,
            &rt_handle,
            &cli.checkpoint,
            &search_params,
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
            k,
            base,
            min_n,
            max_n,
            &progress,
            &db,
            &rt_handle,
            &cli.checkpoint,
            &search_params,
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
            k,
            base,
            min_n,
            max_n,
            &progress,
            &db,
            &rt_handle,
            &cli.checkpoint,
            &search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::Repunit { base, min_n, max_n } => repunit::search(
            base,
            min_n,
            max_n,
            &progress,
            &db,
            &rt_handle,
            &cli.checkpoint,
            &search_params,
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
            fermat_exp,
            min_base,
            max_base,
            &progress,
            &db,
            &rt_handle,
            &cli.checkpoint,
            &search_params,
            mr,
            sl,
            coord,
            eb,
        ),
        Commands::Dashboard { .. }
        | Commands::Work { .. }
        | Commands::Verify { .. }
        | Commands::Project { .. } => {
            unreachable!()
        }
    };

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

    match &client {
        Some(Client::Http(c)) => c.deregister(),
        Some(Client::Pg(c)) => c.deregister(),
        None => {}
    }
    if let Some(handle) = heartbeat_handle {
        let _ = handle.join();
    }

    eprintln!("Search complete.");
    result
}

/// Block-claiming work loop for the `work` subcommand.
fn run_work_loop(
    cli: &Cli,
    db: &Arc<db::Database>,
    rt_handle: &tokio::runtime::Handle,
    search_job_id: i64,
) -> Result<()> {
    let worker_id = cli.worker_id.clone().unwrap_or_else(|| {
        std::process::Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "worker".to_string())
    });

    // Fetch the search job to know what type of search to run
    let job = rt_handle
        .block_on(db.get_search_job(search_job_id))?
        .ok_or_else(|| anyhow::anyhow!("Search job {} not found", search_job_id))?;

    eprintln!(
        "Work mode: job {} ({}), claiming blocks from {}..{}",
        search_job_id, job.search_type, job.range_start, job.range_end
    );

    // Set up PG worker client for heartbeating
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

    // Sync progress into PG client
    let wc_tested = Arc::clone(&pg_client.tested);
    let wc_found = Arc::clone(&pg_client.found);
    let wc_current = Arc::clone(&pg_client.current);
    let progress_ref = Arc::clone(&progress);
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

    let heartbeat_handle = pg_client.start_heartbeat();
    let coord: Option<&dyn CoordinationClient> = Some(&pg_client);

    let mr = cli.mr_rounds;
    let sl = cli.sieve_limit;
    let mut blocks_completed = 0u64;

    loop {
        if pg_client.is_stop_requested() {
            eprintln!("Stop requested, exiting work loop");
            break;
        }

        // Claim next available block
        let block = rt_handle.block_on(db.claim_work_block(search_job_id, &worker_id))?;
        let block = match block {
            Some(b) => b,
            None => {
                eprintln!("No more blocks available, work complete");
                break;
            }
        };

        eprintln!(
            "Claimed block {} (range {}..{})",
            block.block_id, block.block_start, block.block_end
        );

        // Reset progress for this block
        progress
            .tested
            .store(0, std::sync::atomic::Ordering::Relaxed);
        progress
            .found
            .store(0, std::sync::atomic::Ordering::Relaxed);

        let block_result = run_search_block(
            &job.search_type,
            &job.params,
            block.block_start,
            block.block_end,
            &progress,
            db,
            rt_handle,
            &cli.checkpoint,
            mr,
            sl,
            coord,
        );

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
                eprintln!(
                    "Block {} completed (tested={}, found={})",
                    block.block_id, tested, found
                );
            }
            Err(e) => {
                eprintln!("Block {} failed: {}", block.block_id, e);
                rt_handle.block_on(db.fail_work_block(block.block_id))?;
            }
        }
    }

    progress.stop();
    let _ = reporter_handle.join();
    pg_client.deregister();
    let _ = heartbeat_handle.join();

    eprintln!("Work loop finished: {} blocks completed", blocks_completed);

    // Check if all blocks are done and update job status
    let summary = rt_handle.block_on(db.get_job_block_summary(search_job_id))?;
    if summary.available == 0 && summary.claimed == 0 {
        rt_handle.block_on(db.update_search_job_status(search_job_id, "completed", None))?;
        eprintln!("Search job {} marked completed", search_job_id);
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

/// Configure the rayon global thread pool with optional QoS and thread count.
fn configure_rayon(threads: Option<usize>, qos: bool) {
    let num_threads = threads.unwrap_or(0); // 0 = rayon default (all logical cores)

    #[cfg(target_os = "macos")]
    if qos {
        let result = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .spawn_handler(|thread| {
                std::thread::Builder::new().spawn(move || {
                    // Set QoS to user-initiated so macOS schedules on P-cores.
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
                eprintln!("Rayon threads configured with macOS QoS: user-initiated (P-core scheduling)");
            }
            Err(e) => {
                eprintln!("Warning: could not configure rayon thread pool: {}", e);
            }
        }
        return;
    }

    #[cfg(not(target_os = "macos"))]
    if qos {
        eprintln!("Warning: --qos flag is only effective on macOS, ignoring");
    }

    if num_threads > 0 {
        if let Err(e) = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build_global()
        {
            eprintln!("Warning: could not configure rayon thread pool: {}", e);
        }
    }
}

/// Run the verify subcommand.
fn run_verify(
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
            // Cross-verify using PFGW subprocess (independent tool)
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

/// Handle the `project` subcommand and its actions.
fn run_project(cli: &Cli, action: &ProjectAction) -> Result<()> {
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
        eprintln!("  Est. duration:       {:.1}h with {} workers", est.estimated_duration_hours, est.workers_recommended);
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
            eprintln!("Project '{}' created (id={}, slug={})", config.project.name, id, slug);
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
            eprintln!("\nActivate with: primehunt project activate {}", slug);
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
                    eprintln!("  [{}] {}: {}", evt.created_at.format("%Y-%m-%d %H:%M"), evt.event_type, evt.summary);
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
            eprintln!("Project '{}' activated. Orchestration will start on next dashboard tick.", slug);
        }
        ProjectAction::Pause { slug } => {
            let proj = rt
                .block_on(database.get_project_by_slug(slug))?
                .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", slug))?;
            if proj.status != "active" {
                anyhow::bail!("Cannot pause project with status '{}' (must be 'active')", proj.status);
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
