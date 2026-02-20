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

mod cli;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use darkreach::db;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "darkreach", about = "Hunt for special-form prime numbers")]
struct Cli {
    /// PostgreSQL connection URL (or set DATABASE_URL env var)
    #[arg(long, env = "DATABASE_URL")]
    database_url: Option<String>,

    /// Path to checkpoint file for resuming searches
    #[arg(long, default_value = "darkreach.checkpoint")]
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

    /// Coordinator dashboard URL (e.g. http://coordinator:7001). When set, worker registers
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
        #[arg(long, default_value_t = 7001)]
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
    /// Register as a volunteer and receive an API key
    Join {
        /// Your username
        #[arg(long)]
        username: String,
        /// Your email address
        #[arg(long)]
        email: String,
        /// Coordinator server URL
        #[arg(long)]
        server: String,
    },
    /// Run as a volunteer worker (claim work, compute, submit results)
    Volunteer,
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
    let _ = dotenvy::dotenv();

    // Initialize structured logging: LOG_FORMAT=json for K8s, human-readable otherwise
    let log_format = std::env::var("LOG_FORMAT").unwrap_or_default();
    if log_format == "json" {
        tracing_subscriber::fmt().json().with_target(false).init();
    } else {
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_target(false)
            .init();
    }

    let cli = Cli::parse();

    darkreach::prst::init(
        cli.prst_min_digits,
        cli.prst_path.clone(),
        std::time::Duration::from_secs(3600),
    );
    darkreach::pfgw::init(
        cli.pfgw_min_digits,
        cli.pfgw_path.clone(),
        std::time::Duration::from_secs(3600),
    );
    cli::configure_rayon(cli.threads, cli.qos);

    match &cli.command {
        Commands::Project { action } => cli::run_project(&cli, action),
        Commands::Dashboard { port, static_dir } => {
            let database_url = cli.database_url.as_deref().ok_or_else(|| {
                anyhow::anyhow!("DATABASE_URL is required (set via --database-url or env)")
            })?;
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(darkreach::dashboard::run(
                *port,
                database_url,
                &cli.checkpoint,
                static_dir.as_deref(),
            ))
        }
        Commands::Verify {
            id,
            all,
            form,
            batch_size,
            force,
            tool,
        } => {
            let database_url = cli.database_url.as_deref().ok_or_else(|| {
                anyhow::anyhow!("DATABASE_URL is required (set via --database-url or env)")
            })?;
            let rt = tokio::runtime::Runtime::new()?;
            let database = rt.block_on(db::Database::connect(database_url))?;
            cli::run_verify(&rt, &database, *id, *all, form.as_deref(), *batch_size, *force, tool)
        }
        Commands::Join {
            username,
            email,
            server,
        } => cli::run_join(server, username, email),
        Commands::Volunteer => cli::run_volunteer(&cli),
        _ => cli::run_search(&cli),
    }
}
