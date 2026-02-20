//! CLI integration tests using assert_cmd.
//!
//! Tests without database: always run (help, arg validation).
//! Tests with database: gated on TEST_DATABASE_URL environment variable.

mod common;

use assert_cmd::Command;
use predicates::prelude::*;

#[allow(deprecated)]
fn darkreach() -> Command {
    Command::cargo_bin("darkreach").unwrap()
}

// --- Help and arg validation (no database needed) ---

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

#[test]
fn help_factorial_shows_args() {
    darkreach()
        .args(["factorial", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--start").and(predicate::str::contains("--end")));
}

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

#[test]
fn help_dashboard_shows_args() {
    darkreach()
        .args(["dashboard", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--port").and(predicate::str::contains("--static-dir")));
}

#[test]
fn unknown_subcommand_fails() {
    darkreach()
        .arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn factorial_missing_required_args_fails() {
    darkreach()
        .args(["--database-url", "postgres://fake", "factorial"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--start").or(predicate::str::contains("required")));
}

#[test]
fn kbn_missing_required_args_fails() {
    darkreach()
        .args(["--database-url", "postgres://fake", "kbn"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--k").or(predicate::str::contains("required")));
}

#[test]
fn palindromic_missing_required_args_fails() {
    darkreach()
        .args(["--database-url", "postgres://fake", "palindromic"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--min-digits").or(predicate::str::contains("required")));
}

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

// --- Search integration tests (require TEST_DATABASE_URL) ---

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
