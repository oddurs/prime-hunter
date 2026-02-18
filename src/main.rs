use anyhow::Result;
use clap::{Parser, Subcommand};
use primehunt::{db, factorial, kbn, palindromic, progress};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(name = "primehunt", about = "Hunt for special-form prime numbers")]
struct Cli {
    /// Path to SQLite database for storing results
    #[arg(long, default_value = "primehunt.db")]
    db: PathBuf,

    /// Path to checkpoint file for resuming searches
    #[arg(long, default_value = "primehunt.checkpoint")]
    checkpoint: PathBuf,

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let num_cores = rayon::current_num_threads();
    eprintln!("primehunt starting with {} CPU cores", num_cores);

    let database = db::Database::open(&cli.db)?;
    let db = Arc::new(Mutex::new(database));

    let progress = progress::Progress::new();
    let reporter_handle = progress.start_reporter();

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
    };

    let result = match cli.command {
        Commands::Factorial { start, end } => {
            factorial::search(start, end, &progress, &db, &cli.checkpoint, &search_params)
        }
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
            &cli.checkpoint,
            &search_params,
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
            &cli.checkpoint,
            &search_params,
        ),
    };

    progress.stop();
    let _ = reporter_handle.join();
    progress.print_status();
    eprintln!("Search complete.");

    result
}
