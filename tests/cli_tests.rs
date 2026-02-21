//! CLI integration tests for the `darkreach` binary.
//!
//! These tests exercise the command-line interface using `assert_cmd`, which
//! spawns the compiled binary as a subprocess and asserts on exit code, stdout,
//! and stderr. Tests are split into two tiers:
//!
//! - **No-database tests** (always run): help text, argument validation, error
//!   handling for missing/invalid arguments. These verify the `clap` CLI parser
//!   is correctly configured for all 12+ search subcommands.
//!
//! - **Database-dependent tests** (gated on `TEST_DATABASE_URL`): actual search
//!   execution against known prime sequences to verify end-to-end correctness.
//!   These run the search engine against small ranges and verify primes are found.
//!
//! # Prerequisites
//!
//! - The `darkreach` binary must be compiled (`cargo build`).
//! - For database tests: `TEST_DATABASE_URL` environment variable.
//!
//! # How to run
//!
//! ```bash
//! # Run all CLI tests (no database needed for help/validation tests):
//! cargo test --test cli_tests
//!
//! # Run only database-dependent search tests:
//! TEST_DATABASE_URL=postgres://... cargo test --test cli_tests -- factorial_finds kbn_finds
//! ```
//!
//! # Testing strategy
//!
//! Help and argument validation tests are pure CLI tests that do not touch
//! the network or database. They verify that `clap`'s derived parser correctly
//! exposes all subcommands and their required arguments.
//!
//! Search integration tests exercise the full pipeline: CLI parsing -> database
//! connection -> sieve -> primality test -> result logging. They use small
//! parameter ranges with known prime counts from OEIS to verify correctness.

mod common;

use assert_cmd::Command;
use predicates::prelude::*;

/// Constructs a `Command` targeting the compiled `darkreach` binary.
///
/// Uses `assert_cmd::Command::cargo_bin` which locates the binary in the
/// cargo target directory, ensuring we test the exact build artifact.
#[allow(deprecated)]
fn darkreach() -> Command {
    Command::cargo_bin("darkreach").unwrap()
}

// == Help and Argument Validation ==============================================
// These tests verify the CLI parser configuration without requiring a database.
// They ensure all 12 search subcommands plus utility commands are registered,
// each subcommand documents its required arguments, and invalid input is
// rejected with meaningful error messages.
// ==============================================================================

/// Verifies `--help` lists all search subcommands and utility commands.
///
/// Exercises: top-level `clap` parser, subcommand registration.
///
/// The darkreach binary supports 12 search forms (factorial, palindromic, kbn,
/// primorial, wagstaff, carol-kynea, twin, sophie-germain, repunit, gen-fermat,
/// cullen-woodall, near-repdigit) plus utility commands (dashboard, work, verify).
/// All must appear in the help text.
#[test]
fn help_shows_all_subcommands() {
    darkreach().arg("--help").assert().success().stdout(
        predicate::str::contains("factorial")
            .and(predicate::str::contains("palindromic"))
            .and(predicate::str::contains("kbn"))
            .and(predicate::str::contains("dashboard"))
            .and(predicate::str::contains("primorial"))
            .and(predicate::str::contains("wagstaff"))
            .and(predicate::str::contains("carol-kynea"))
            .and(predicate::str::contains("twin"))
            .and(predicate::str::contains("sophie-germain"))
            .and(predicate::str::contains("repunit"))
            .and(predicate::str::contains("gen-fermat"))
            .and(predicate::str::contains("cullen-woodall"))
            .and(predicate::str::contains("near-repdigit"))
            .and(predicate::str::contains("work"))
            .and(predicate::str::contains("verify")),
    );
}

/// Verifies `factorial --help` documents the --start and --end arguments.
///
/// Exercises: factorial subcommand `clap` parser.
///
/// The factorial search requires a range [start, end] specifying which n values
/// to test for n!+1 and n!-1 primality.
#[test]
fn help_factorial_shows_args() {
    darkreach()
        .args(["factorial", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--start").and(predicate::str::contains("--end")));
}

/// Verifies `kbn --help` documents all required arguments (k, base, min-n, max-n).
///
/// Exercises: kbn subcommand `clap` parser.
///
/// The k*b^n+/-1 search requires four parameters defining the search space:
/// the multiplier k, the base b, and the exponent range [min-n, max-n].
#[test]
fn help_kbn_shows_args() {
    darkreach()
        .args(["kbn", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--k")
                .and(predicate::str::contains("--base"))
                .and(predicate::str::contains("--min-n"))
                .and(predicate::str::contains("--max-n")),
        );
}

/// Verifies `palindromic --help` documents all required arguments.
///
/// Exercises: palindromic subcommand `clap` parser.
///
/// The palindromic search requires a base and digit range [min-digits, max-digits].
/// Even-digit palindromes are automatically skipped (always divisible by base+1).
#[test]
fn help_palindromic_shows_args() {
    darkreach()
        .args(["palindromic", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--base")
                .and(predicate::str::contains("--min-digits"))
                .and(predicate::str::contains("--max-digits")),
        );
}

/// Verifies `dashboard --help` documents the --port and --static-dir arguments.
///
/// Exercises: dashboard subcommand `clap` parser.
///
/// The dashboard serves the Next.js frontend and provides the REST API / WebSocket
/// endpoints. It requires a port and optional static file directory.
#[test]
fn help_dashboard_shows_args() {
    darkreach()
        .args(["dashboard", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--port").and(predicate::str::contains("--static-dir")));
}

/// Verifies that an unknown subcommand fails with a helpful error.
///
/// Exercises: `clap` error handling for unrecognized subcommands.
///
/// The error message should mention "unrecognized subcommand" so users know
/// what went wrong and can check `--help` for the valid command list.
#[test]
fn unknown_subcommand_fails() {
    darkreach()
        .arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

/// Verifies that `factorial` without required args fails with a clear error.
///
/// Exercises: `clap` required argument validation for the factorial subcommand.
///
/// Running `darkreach factorial` without `--start` and `--end` should fail
/// before attempting any database connection or computation. The error should
/// mention the missing argument name.
#[test]
fn factorial_missing_required_args_fails() {
    darkreach()
        .args(["--database-url", "postgres://fake", "factorial"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--start").or(predicate::str::contains("required")));
}

/// Verifies that `kbn` without required args fails with a clear error.
///
/// Exercises: `clap` required argument validation for the kbn subcommand.
///
/// Running `darkreach kbn` without `--k` should fail immediately.
#[test]
fn kbn_missing_required_args_fails() {
    darkreach()
        .args(["--database-url", "postgres://fake", "kbn"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--k").or(predicate::str::contains("required")));
}

/// Verifies that `palindromic` without required args fails with a clear error.
///
/// Exercises: `clap` required argument validation for the palindromic subcommand.
///
/// Running `darkreach palindromic` without `--min-digits` should fail immediately.
#[test]
fn palindromic_missing_required_args_fails() {
    darkreach()
        .args(["--database-url", "postgres://fake", "palindromic"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--min-digits").or(predicate::str::contains("required")));
}

/// Verifies that an unreachable database URL causes a connection failure.
///
/// Exercises: database connection error handling, non-zero exit code.
///
/// Provides a database URL pointing to a non-existent server (port 59999).
/// The binary should fail with a connection error rather than hanging or
/// panicking. A 10-second timeout prevents the test from blocking indefinitely
/// if the connection attempt does not fail fast enough.
#[test]
fn invalid_database_url_fails() {
    // An unreachable database URL should cause a connection error
    darkreach()
        .env(
            "DATABASE_URL",
            "postgres://invalid:invalid@127.0.0.1:59999/nonexistent",
        )
        .args([
            "--database-url",
            "postgres://invalid:invalid@127.0.0.1:59999/nonexistent",
            "factorial",
            "--start",
            "1",
            "--end",
            "10",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .failure();
}

// == Search Integration Tests ==================================================
// These tests run actual prime searches against small parameter ranges and
// verify that known primes are found. They require a test database for result
// logging and are skipped when TEST_DATABASE_URL is not set.
//
// Reference sequences from OEIS:
// - Factorial primes (n!+1): A002981 (1, 2, 3, 11, 27, 37, 41, 73, ...)
// - Factorial primes (n!-1): A002982 (3, 4, 6, 7, 12, 14, 30, 32, ...)
// - Mersenne primes (2^p-1): A000043 (2, 3, 5, 7, 13, 17, 19, 31, ...)
// - Palindromic primes (base 10): A002385 (2, 3, 5, 7, 11, 101, 131, ...)
// - Wagstaff primes ((2^p+1)/3): A000978 (3, 5, 7, 11, 13, 17, 19, ...)
// - Carol primes ((2^n-1)^2-2): A091515 (2, 3, 4, 6, 7, 10, 12, 15, ...)
// ==============================================================================

/// Skips the test and returns early if TEST_DATABASE_URL is not set.
///
/// Unlike `require_db!()` in the async tests, this macro works in synchronous
/// `#[test]` functions and returns the database URL string for use with
/// `--database-url`.
macro_rules! db_url_or_skip {
    () => {
        match std::env::var("TEST_DATABASE_URL") {
            Ok(url) => url,
            Err(_) => {
                eprintln!("Skipping: TEST_DATABASE_URL not set");
                return;
            }
        }
    };
}

/// Verifies the factorial search finds known n!+/-1 primes in the range [1, 50].
///
/// Exercises: full factorial search pipeline (CLI -> sieve -> GMP factorial ->
/// primality test -> database insert).
///
/// Known primes in this range (OEIS A002981, A002982):
/// - n!+1 primes: n = 1, 2, 3, 11, 27, 37, 41
/// - n!-1 primes: n = 3, 4, 6, 7, 12, 14, 30, 32
///
/// The test asserts that at least one "PRIME" message appears in stderr output.
/// Timeout: 60 seconds (factorial computation is fast for small n).
#[test]
fn factorial_finds_known_primes() {
    let db_url = db_url_or_skip!();
    // n!+1 primes: 1, 2, 3, 11, 27, 37, 41, 73, ...
    // n!-1 primes: 3, 4, 6, 7, 12, 14, 30, 32, ...
    // Range 1..50 should find several
    darkreach()
        .args([
            "--database-url",
            &db_url,
            "factorial",
            "--start",
            "1",
            "--end",
            "50",
        ])
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stderr(predicate::str::contains("PRIME"));
}

/// Verifies the kbn search finds known Mersenne primes 2^p - 1.
///
/// Exercises: full kbn search pipeline (CLI -> BSGS sieve -> Proth/LLR test ->
/// database insert).
///
/// With k=1, base=2, this searches for Mersenne primes (OEIS A000043).
/// Known primes in [2, 20]: p = 2, 3, 5, 7, 13, 17, 19 (seven Mersenne primes).
/// Timeout: 60 seconds.
#[test]
fn kbn_finds_mersenne_primes() {
    let db_url = db_url_or_skip!();
    // k=1, base=2: 2^n-1 primes for n=2,3,5,7,13,17,19
    darkreach()
        .args([
            "--database-url",
            &db_url,
            "kbn",
            "--k",
            "1",
            "--base",
            "2",
            "--min-n",
            "2",
            "--max-n",
            "20",
        ])
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stderr(predicate::str::contains("PRIME"));
}

/// Verifies the palindromic search finds known palindromic primes in base 10.
///
/// Exercises: full palindromic search pipeline (CLI -> palindrome generation ->
/// deep sieve -> primality test -> database insert).
///
/// Base 10 palindromic primes with 1-5 digits (OEIS A002385):
/// 1-digit: 2, 3, 5, 7
/// 3-digit: 101, 131, 151, 181, 191, 313, 353, 373, 383, 727, 757, 787, 797, 919, 929
/// 5-digit: 10301, 10501, 10601, ... (many)
///
/// Even-digit palindromes (2, 4 digits) are skipped automatically since they
/// are always divisible by 11 (base+1) in base 10.
/// Timeout: 60 seconds.
#[test]
fn palindromic_finds_known_primes() {
    let db_url = db_url_or_skip!();
    // Base 10, 1-5 digits: should find many palindromic primes (2,3,5,7,11,101,131,...)
    darkreach()
        .args([
            "--database-url",
            &db_url,
            "palindromic",
            "--base",
            "10",
            "--min-digits",
            "1",
            "--max-digits",
            "5",
        ])
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stderr(predicate::str::contains("PRIME"));
}

/// Verifies the Wagstaff search finds known Wagstaff primes (2^p+1)/3.
///
/// Exercises: full Wagstaff search pipeline (CLI -> multiplicative order sieve ->
/// PFGW/GWNUM test -> database insert).
///
/// Known Wagstaff prime exponents (OEIS A000978): 3, 5, 7, 11, 13, 17, 19, 23, 31, 43.
/// Range [3, 50] should find all 10. Note: Wagstaff primes have no known
/// deterministic primality test, so results are always "PRP" (probable prime).
/// Timeout: 60 seconds.
#[test]
fn wagstaff_finds_known_primes() {
    let db_url = db_url_or_skip!();
    // Known Wagstaff primes: exponents 3,5,7,11,13,17,19,23,31,43
    darkreach()
        .args([
            "--database-url",
            &db_url,
            "wagstaff",
            "--min-exp",
            "3",
            "--max-exp",
            "50",
        ])
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stderr(predicate::str::contains("PRIME").or(predicate::str::contains("PRP")));
}

/// Verifies the Carol/Kynea search finds known Carol and Kynea primes.
///
/// Exercises: full Carol/Kynea search pipeline (CLI -> candidate generation ->
/// LLR test -> PFGW acceleration -> database insert).
///
/// Carol primes are of the form (2^n - 1)^2 - 2. Kynea primes are (2^n + 1)^2 - 2.
/// Known Carol prime indices (OEIS A091515): 2, 3, 4, 6, 7, 10, 12, 15.
/// Range [2, 16] should find several. Gotcha: n=17 is NOT a Carol prime index
/// despite some older references listing it.
/// Timeout: 60 seconds.
#[test]
fn carol_kynea_finds_primes() {
    let db_url = db_url_or_skip!();
    // Carol primes at n=2,3,4,6,7,10,12,15
    darkreach()
        .args([
            "--database-url",
            &db_url,
            "carol-kynea",
            "--min-n",
            "2",
            "--max-n",
            "16",
        ])
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stderr(predicate::str::contains("PRIME"));
}
