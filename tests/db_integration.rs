//! Database integration tests for the darkreach `db` module.
//!
//! These tests exercise every major database operation in the application:
//! prime CRUD, worker coordination, search job lifecycle, agent task management,
//! template expansion with dependency resolution, agent memory/roles/observability,
//! operator (volunteer) trust progression, credit accounting, and project management.
//!
//! # Prerequisites
//!
//! - A running PostgreSQL instance with the `TEST_DATABASE_URL` environment variable set.
//! - Example: `TEST_DATABASE_URL=postgres://user:pass@localhost:5432/darkreach_test`
//!
//! # How to run
//!
//! ```bash
//! # Run all database integration tests (single-threaded to avoid table conflicts):
//! TEST_DATABASE_URL=postgres://... cargo test --test db_integration -- --test-threads=1
//!
//! # Run a specific test:
//! TEST_DATABASE_URL=postgres://... cargo test --test db_integration operator_trust_progression
//! ```
//!
//! # Testing strategy
//!
//! Each test calls `setup()` which connects to the test database and truncates all
//! tables via `common::setup_test_db()`. This guarantees full isolation: every test
//! starts with an empty database plus re-seeded reference data (agent roles, templates,
//! budgets). Tests are organized into sections by domain (Prime CRUD, Worker Coordination,
//! Search Jobs, Agent Management, etc.) and should be run single-threaded (`--test-threads=1`)
//! since they share the same database instance.
//!
//! The `require_db!()` macro at the top of each test skips gracefully when
//! `TEST_DATABASE_URL` is not set, allowing `cargo test` to pass in environments
//! without a test database.

mod common;

use darkreach::db::{Database, PrimeFilter};

/// Skip the test if TEST_DATABASE_URL is not set.
///
/// This macro provides a clean skip mechanism for CI environments or local dev
/// setups where a PostgreSQL test database is not available. It prints a diagnostic
/// message to stderr and returns early, so the test is marked as "passed" (not failed).
macro_rules! require_db {
    () => {
        if !common::has_test_db() {
            eprintln!("Skipping: TEST_DATABASE_URL not set");
            return;
        }
    };
}

/// Connects to the test database and truncates all tables.
///
/// Wrapper around `common::setup_test_db()` for brevity. Every test that
/// touches the database should call this first to ensure a clean slate.
async fn setup() -> Database {
    common::setup_test_db().await
}

// == Prime CRUD ================================================================
// Tests for inserting, retrieving, filtering, sorting, and paginating prime
// records in the `primes` table. This is the core data model -- every search
// module writes discovered primes here.
// ==============================================================================

/// Verifies that the test database connection succeeds.
///
/// Exercises: `Database::connect`, `TEST_DATABASE_URL` resolution, migration setup.
///
/// This is the most basic smoke test. If this fails, all other database tests
/// will also fail, so it serves as a quick diagnostic for environment issues.
#[tokio::test]
async fn connect_to_test_db() {
    require_db!();
    let _db = setup().await;
    // If we get here without panic, connection succeeded
}

/// Tests the full prime insert-and-retrieve cycle.
///
/// Exercises: `primes` table INSERT via `db.insert_prime()`, SELECT via
/// `db.get_primes_filtered()` with default filters.
///
/// Inserts a factorial prime `5! + 1 = 121` (3 digits) with deterministic proof,
/// then retrieves it and verifies all fields match: form, expression, digits,
/// and proof_method.
#[tokio::test]
async fn insert_prime_and_retrieve() {
    require_db!();
    let db = setup().await;

    db.insert_prime(
        "factorial",
        "5! + 1",
        3,
        r#"{"form":"factorial"}"#,
        "deterministic",
        None,
    )
    .await
    .unwrap();

    let primes = db
        .get_primes_filtered(10, 0, &PrimeFilter::default())
        .await
        .unwrap();
    assert_eq!(primes.len(), 1);
    assert_eq!(primes[0].form, "factorial");
    assert_eq!(primes[0].expression, "5! + 1");
    assert_eq!(primes[0].digits, 3);
    assert_eq!(primes[0].proof_method, "deterministic");
}

/// Tests that duplicate prime expressions are silently ignored.
///
/// Exercises: `primes` table UNIQUE constraint on `expression`, `db.insert_prime_ignore()`
/// (INSERT ... ON CONFLICT DO NOTHING).
///
/// Inserts a prime with "deterministic" proof, then attempts to re-insert the same
/// expression with "probabilistic" proof. The second insert should be silently
/// ignored, and the original proof_method should be preserved. This prevents
/// search workers from overwriting a deterministic proof with a weaker one.
#[tokio::test]
async fn insert_duplicate_expression_ignored() {
    require_db!();
    let db = setup().await;

    db.insert_prime("factorial", "5! + 1", 3, "{}", "deterministic", None)
        .await
        .unwrap();

    // insert_prime_ignore should not error on duplicate
    db.insert_prime_ignore("factorial", "5! + 1", 3, "{}", "probabilistic")
        .await
        .unwrap();

    let primes = db
        .get_primes_filtered(10, 0, &PrimeFilter::default())
        .await
        .unwrap();
    assert_eq!(primes.len(), 1);
    // Original proof_method should be preserved
    assert_eq!(primes[0].proof_method, "deterministic");
}

/// Tests filtering primes by their mathematical form.
///
/// Exercises: `PrimeFilter.form` field, SQL WHERE clause on `primes.form`.
///
/// Inserts three primes of different forms (factorial, kbn, palindromic),
/// then filters by `form = "factorial"` and verifies only the matching
/// prime is returned. This powers the dashboard's form-specific views.
#[tokio::test]
async fn filter_primes_by_form() {
    require_db!();
    let db = setup().await;

    db.insert_prime("factorial", "5! + 1", 3, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("kbn", "3*2^5+1", 2, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("palindromic", "10301", 5, "{}", "det", None)
        .await
        .unwrap();

    let filter = PrimeFilter {
        form: Some("factorial".into()),
        ..Default::default()
    };
    let primes = db.get_primes_filtered(10, 0, &filter).await.unwrap();
    assert_eq!(primes.len(), 1);
    assert_eq!(primes[0].form, "factorial");
}

/// Tests filtering primes by digit count range.
///
/// Exercises: `PrimeFilter.min_digits` and `PrimeFilter.max_digits`, SQL
/// WHERE clause `digits BETWEEN min AND max`.
///
/// Inserts primes with 3, 31, and 5 digits, then filters for 4-10 digits.
/// Only the 5-digit palindromic prime should match.
#[tokio::test]
async fn filter_primes_by_digit_range() {
    require_db!();
    let db = setup().await;

    db.insert_prime("factorial", "5! + 1", 3, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("kbn", "3*2^100+1", 31, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("palindromic", "10301", 5, "{}", "det", None)
        .await
        .unwrap();

    let filter = PrimeFilter {
        min_digits: Some(4),
        max_digits: Some(10),
        ..Default::default()
    };
    let primes = db.get_primes_filtered(10, 0, &filter).await.unwrap();
    assert_eq!(primes.len(), 1);
    assert_eq!(primes[0].expression, "10301");
}

/// Tests free-text search across prime expressions.
///
/// Exercises: `PrimeFilter.search` field, SQL `ILIKE '%search%'` on `primes.expression`.
///
/// Inserts two factorial primes containing "73!" and one kbn prime, then
/// searches for "73!". Both factorial primes should match; the kbn prime should not.
#[tokio::test]
async fn filter_primes_search_text() {
    require_db!();
    let db = setup().await;

    db.insert_prime("factorial", "73! + 1", 106, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("factorial", "73! - 1", 106, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("kbn", "3*2^5+1", 2, "{}", "det", None)
        .await
        .unwrap();

    let filter = PrimeFilter {
        search: Some("73!".into()),
        ..Default::default()
    };
    let primes = db.get_primes_filtered(10, 0, &filter).await.unwrap();
    assert_eq!(primes.len(), 2);
}

/// Tests ascending and descending sort order on primes.
///
/// Exercises: `PrimeFilter.sort_by` and `PrimeFilter.sort_dir`, SQL `ORDER BY`.
///
/// Inserts three primes with digit counts 10, 20, and 5. Verifies ascending
/// sort returns [5, 10, 20] and descending (the default) returns [20, 10, 5].
/// Sort column validation prevents SQL injection (tested in security_tests.rs).
#[tokio::test]
async fn sort_primes_ascending_descending() {
    require_db!();
    let db = setup().await;

    db.insert_prime("factorial", "A", 10, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("factorial", "B", 20, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("factorial", "C", 5, "{}", "det", None)
        .await
        .unwrap();

    // Ascending by digits
    let filter = PrimeFilter {
        sort_by: Some("digits".into()),
        sort_dir: Some("asc".into()),
        ..Default::default()
    };
    let primes = db.get_primes_filtered(10, 0, &filter).await.unwrap();
    assert_eq!(primes.len(), 3);
    assert_eq!(primes[0].digits, 5);
    assert_eq!(primes[1].digits, 10);
    assert_eq!(primes[2].digits, 20);

    // Descending by digits (default)
    let filter = PrimeFilter {
        sort_by: Some("digits".into()),
        ..Default::default()
    };
    let primes = db.get_primes_filtered(10, 0, &filter).await.unwrap();
    assert_eq!(primes[0].digits, 20);
    assert_eq!(primes[2].digits, 5);
}

/// Tests LIMIT/OFFSET pagination of prime results.
///
/// Exercises: `get_primes_filtered(limit, offset, ...)`, SQL `LIMIT` and `OFFSET`.
///
/// Inserts 5 primes, then paginates with page size 2. Verifies:
/// - Page 1 (offset 0): 2 results
/// - Page 2 (offset 2): 2 results
/// - Page 3 (offset 4): 1 result (last page)
/// - No ID overlap between pages (pagination is stable)
#[tokio::test]
async fn paginate_primes() {
    require_db!();
    let db = setup().await;

    for i in 1..=5 {
        db.insert_prime("factorial", &format!("{}! + 1", i), i, "{}", "det", None)
            .await
            .unwrap();
    }

    // Page 1: limit 2, offset 0
    let page1 = db
        .get_primes_filtered(2, 0, &PrimeFilter::default())
        .await
        .unwrap();
    assert_eq!(page1.len(), 2);

    // Page 2: limit 2, offset 2
    let page2 = db
        .get_primes_filtered(2, 2, &PrimeFilter::default())
        .await
        .unwrap();
    assert_eq!(page2.len(), 2);

    // Page 3: limit 2, offset 4
    let page3 = db
        .get_primes_filtered(2, 4, &PrimeFilter::default())
        .await
        .unwrap();
    assert_eq!(page3.len(), 1);

    // No overlap between pages
    let ids1: Vec<i64> = page1.iter().map(|p| p.id).collect();
    let ids2: Vec<i64> = page2.iter().map(|p| p.id).collect();
    let ids3: Vec<i64> = page3.iter().map(|p| p.id).collect();
    for id in &ids1 {
        assert!(!ids2.contains(id));
        assert!(!ids3.contains(id));
    }
}

/// Tests the filtered count query for dashboard total display.
///
/// Exercises: `db.get_filtered_count()`, SQL `SELECT COUNT(*)` with optional WHERE.
///
/// Inserts 3 primes (1 factorial, 2 kbn), verifies total count is 3 and
/// kbn-filtered count is 2. Used by the frontend to display "showing X of Y"
/// pagination metadata.
#[tokio::test]
async fn get_filtered_count() {
    require_db!();
    let db = setup().await;

    db.insert_prime("factorial", "5! + 1", 3, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("kbn", "3*2^5+1", 2, "{}", "det", None)
        .await
        .unwrap();
    db.insert_prime("kbn", "3*2^7+1", 3, "{}", "det", None)
        .await
        .unwrap();

    let count = db
        .get_filtered_count(&PrimeFilter::default())
        .await
        .unwrap();
    assert_eq!(count, 3);

    let filter = PrimeFilter {
        form: Some("kbn".into()),
        ..Default::default()
    };
    let count = db.get_filtered_count(&filter).await.unwrap();
    assert_eq!(count, 2);
}

// == Worker Coordination =======================================================
// Tests for the `workers` table: registration via upsert, deletion, command
// dispatch (stop/reconfigure), heartbeat RPC, and stale worker pruning.
// These operations power the fleet management dashboard.
// ==============================================================================

/// Tests worker upsert (insert-or-update) and retrieval.
///
/// Exercises: `workers` table INSERT/UPDATE via `db.upsert_worker()`,
/// `db.get_all_workers()`.
///
/// First inserts a worker, verifies its fields, then upserts the same worker_id
/// with different values. The second upsert should update (not duplicate) the row,
/// leaving exactly 1 worker with the new hostname and core count.
#[tokio::test]
async fn worker_upsert_and_retrieve() {
    require_db!();
    let db = setup().await;

    db.upsert_worker(
        "test-worker-1",
        "host1.example.com",
        8,
        "factorial",
        r#"{"start":1}"#,
    )
    .await
    .unwrap();

    let workers = db.get_all_workers().await.unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].worker_id, "test-worker-1");
    assert_eq!(workers[0].hostname, "host1.example.com");
    assert_eq!(workers[0].cores, 8);
    assert_eq!(workers[0].search_type, "factorial");

    // Upsert same worker updates fields
    db.upsert_worker(
        "test-worker-1",
        "host1-updated.com",
        16,
        "kbn",
        r#"{"k":3}"#,
    )
    .await
    .unwrap();

    let workers = db.get_all_workers().await.unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].hostname, "host1-updated.com");
    assert_eq!(workers[0].cores, 16);
}

/// Tests deleting a specific worker by ID.
///
/// Exercises: `db.delete_worker()`, `workers` table DELETE.
///
/// Inserts two workers, deletes one, and verifies only the other remains.
#[tokio::test]
async fn worker_delete() {
    require_db!();
    let db = setup().await;

    db.upsert_worker("w1", "host1", 4, "factorial", "")
        .await
        .unwrap();
    db.upsert_worker("w2", "host2", 8, "kbn", "").await.unwrap();

    db.delete_worker("w1").await.unwrap();

    let workers = db.get_all_workers().await.unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].worker_id, "w2");
}

/// Tests the worker command dispatch and one-shot consumption pattern.
///
/// Exercises: `db.set_worker_command()`, `db.worker_heartbeat_rpc()`,
/// `workers` table `pending_command` column.
///
/// The coordinator can set a command (e.g., "stop") on a worker. The next
/// heartbeat RPC returns the command and atomically clears it. Subsequent
/// heartbeats return `None`. This one-shot pattern ensures commands are
/// delivered exactly once even if the coordinator retries.
#[tokio::test]
async fn worker_set_and_clear_command() {
    require_db!();
    let db = setup().await;

    db.upsert_worker("w1", "host1", 4, "factorial", "")
        .await
        .unwrap();

    // Set a stop command
    db.set_worker_command("w1", "stop").await.unwrap();

    // Heartbeat RPC should return and clear the command
    let cmd = db
        .worker_heartbeat_rpc(
            "w1",
            "host1",
            4,
            "factorial",
            "",
            100,
            5,
            "testing",
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(cmd, Some("stop".to_string()));

    // Second heartbeat should return None (command was cleared)
    let cmd = db
        .worker_heartbeat_rpc(
            "w1",
            "host1",
            4,
            "factorial",
            "",
            200,
            5,
            "testing",
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(cmd, None);
}

/// Tests automatic pruning of workers that have stopped heartbeating.
///
/// Exercises: `db.prune_stale_workers(timeout_secs)`, SQL `DELETE WHERE
/// last_heartbeat < NOW() - INTERVAL`.
///
/// Inserts two workers, artificially ages one worker's heartbeat to 300 seconds
/// ago, then prunes with a 120-second threshold. Only the stale worker should
/// be removed. This prevents the fleet dashboard from showing ghost workers.
#[tokio::test]
async fn worker_prune_stale() {
    require_db!();
    let db = setup().await;

    db.upsert_worker("fresh", "host1", 4, "factorial", "")
        .await
        .unwrap();
    db.upsert_worker("stale", "host2", 4, "factorial", "")
        .await
        .unwrap();

    // Make the 'stale' worker's heartbeat old
    sqlx::query(
        "UPDATE workers SET last_heartbeat = NOW() - INTERVAL '300 seconds' WHERE worker_id = 'stale'",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let pruned = db.prune_stale_workers(120).await.unwrap();
    assert_eq!(pruned, 1);

    let workers = db.get_all_workers().await.unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].worker_id, "fresh");
}

// == Search Jobs ===============================================================
// Tests for the distributed search job system: job creation with automatic
// block generation, status lifecycle (running -> cancelled/completed), and
// block claiming/completion tracking.
// ==============================================================================

/// Tests search job creation with automatic work block generation.
///
/// Exercises: `db.create_search_job()`, `search_jobs` table INSERT,
/// `work_blocks` table batch INSERT, `db.get_job_block_summary()`.
///
/// Creates a factorial search job spanning range [1, 1000] with block_size 100.
/// Verifies the job metadata (type, status, range) and that exactly 10 blocks
/// (1000/100) are generated, all in "available" state.
#[tokio::test]
async fn search_job_create_with_blocks() {
    require_db!();
    let db = setup().await;

    let params = serde_json::json!({"form": "factorial", "start": 1, "end": 1000});
    let job_id = db
        .create_search_job("factorial", &params, 1, 1000, 100)
        .await
        .unwrap();

    let job = db.get_search_job(job_id).await.unwrap().unwrap();
    assert_eq!(job.search_type, "factorial");
    assert_eq!(job.status, "running");
    assert_eq!(job.range_start, 1);
    assert_eq!(job.range_end, 1000);
    assert_eq!(job.block_size, 100);

    let summary = db.get_job_block_summary(job_id).await.unwrap();
    assert_eq!(summary.available, 10); // 1000/100 = 10 blocks
    assert_eq!(summary.claimed, 0);
    assert_eq!(summary.completed, 0);
}

/// Tests the search job status lifecycle: running -> cancelled.
///
/// Exercises: `db.update_search_job_status()`, `search_jobs` table UPDATE,
/// `stopped_at` timestamp auto-population.
///
/// Creates a job (initially "running"), cancels it, and verifies the status
/// transitions correctly and the `stopped_at` timestamp is set. Jobs can also
/// transition to "completed" when all blocks finish, but cancellation is the
/// manual override path used by operators.
#[tokio::test]
async fn search_job_status_lifecycle() {
    require_db!();
    let db = setup().await;

    let params = serde_json::json!({"form": "factorial"});
    let job_id = db
        .create_search_job("factorial", &params, 1, 100, 10)
        .await
        .unwrap();

    // Initially running
    let job = db.get_search_job(job_id).await.unwrap().unwrap();
    assert_eq!(job.status, "running");
    assert!(job.stopped_at.is_none());

    // Cancel it
    db.update_search_job_status(job_id, "cancelled", None)
        .await
        .unwrap();

    let job = db.get_search_job(job_id).await.unwrap().unwrap();
    assert_eq!(job.status, "cancelled");
    assert!(job.stopped_at.is_some());
}

/// Tests work block claiming and completion with progress tracking.
///
/// Exercises: `db.claim_work_block()` (SELECT ... FOR UPDATE SKIP LOCKED),
/// `db.complete_work_block()`, `db.get_job_block_summary()`.
///
/// Creates a job with 3 blocks, registers a worker (needed for the FK),
/// claims the first block, completes it (reporting 10 tested, 2 found),
/// and verifies the block summary reflects the progress: 2 available,
/// 1 completed, correct totals.
#[tokio::test]
async fn get_job_block_summary_after_completion() {
    require_db!();
    let db = setup().await;

    // Create a worker (needed for claim_work_block foreign key)
    db.upsert_worker("block-worker", "host", 4, "factorial", "")
        .await
        .unwrap();

    let params = serde_json::json!({"form": "factorial"});
    let job_id = db
        .create_search_job("factorial", &params, 1, 30, 10)
        .await
        .unwrap();

    // Claim and complete first block
    let block = db
        .claim_work_block(job_id, "block-worker")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(block.block_start, 1);
    assert_eq!(block.block_end, 11);

    db.complete_work_block(block.block_id, 10, 2).await.unwrap();

    let summary = db.get_job_block_summary(job_id).await.unwrap();
    assert_eq!(summary.available, 2); // 2 remaining
    assert_eq!(summary.completed, 1);
    assert_eq!(summary.total_tested, 10);
    assert_eq!(summary.total_found, 2);
}

// == Agent Task Management =====================================================
// Tests for the AI agent task system: CRUD operations, status transitions
// (pending -> in_progress -> completed/cancelled), and task listing by status.
// ==============================================================================

/// Tests the full agent task lifecycle: create -> start -> cancel.
///
/// Exercises: `agent_tasks` table INSERT/UPDATE, `db.create_agent_task()`,
/// `db.get_agent_task()`, `db.update_agent_task_status()`, `db.cancel_agent_task()`,
/// `db.get_agent_tasks()`.
///
/// Creates a task, verifies initial state (pending), transitions to in_progress
/// (which sets `started_at`), cancels it (which sets `completed_at`), and
/// confirms it appears in filtered task listings. This covers the happy path
/// and the abort path for agent tasks.
#[tokio::test]
async fn agent_task_lifecycle() {
    require_db!();
    let db = setup().await;

    // Create a task
    let task = db
        .create_agent_task(
            "Test task",
            "Do something",
            "normal",
            Some("opus"),
            "manual",
            None,
            1,
            None,
        )
        .await
        .unwrap();
    assert_eq!(task.title, "Test task");
    assert_eq!(task.status, "pending");
    assert_eq!(task.priority, "normal");

    // Retrieve by ID
    let fetched = db.get_agent_task(task.id).await.unwrap().unwrap();
    assert_eq!(fetched.title, "Test task");

    // Update status to in_progress
    db.update_agent_task_status(task.id, "in_progress")
        .await
        .unwrap();
    let fetched = db.get_agent_task(task.id).await.unwrap().unwrap();
    assert_eq!(fetched.status, "in_progress");
    assert!(fetched.started_at.is_some());

    // Cancel it
    db.cancel_agent_task(task.id).await.unwrap();
    let fetched = db.get_agent_task(task.id).await.unwrap().unwrap();
    assert_eq!(fetched.status, "cancelled");
    assert!(fetched.completed_at.is_some());

    // List tasks by status
    let tasks = db.get_agent_tasks(Some("cancelled"), 10).await.unwrap();
    assert!(tasks.iter().any(|t| t.id == task.id));
}

// == Agent Task Decomposition ==================================================
// Tests for the template expansion system that decomposes high-level tasks
// into ordered subtasks with dependency tracking. This powers the multi-step
// agent workflows (e.g., "fix-bug" = Investigate -> Fix -> Verify).
// ==============================================================================

/// Tests that template expansion creates a parent task and child subtasks.
///
/// Exercises: `db.expand_template()`, `agent_tasks` table (parent + children),
/// `agent_templates` table lookup, child task generation from template steps.
///
/// Expands the "fix-bug" template (3 steps: Investigate, Fix, Verify) with
/// a high-priority parent task. Verifies:
/// - Parent task has correct title, priority, template_name, and on_child_failure policy
/// - Exactly 3 children are created, each referencing the parent
/// - Children inherit the parent's priority
/// - Children have correct titles from the template steps
/// - Children are marked as "automated" source (vs. "manual" for user-created)
#[tokio::test]
async fn template_expand_creates_parent_and_children() {
    require_db!();
    let db = setup().await;

    let parent_id = db
        .expand_template(
            "fix-bug",
            "Fix login bug",
            "Users can't log in",
            "high",
            Some(5.0),
            1,
            None,
        )
        .await
        .unwrap();

    // Parent task exists
    let parent = db.get_agent_task(parent_id).await.unwrap().unwrap();
    assert_eq!(parent.title, "Fix login bug");
    assert_eq!(parent.priority, "high");
    assert_eq!(parent.template_name.as_deref(), Some("fix-bug"));
    assert_eq!(parent.on_child_failure, "fail");
    assert_eq!(parent.status, "pending");

    // Children exist
    let children = db.get_child_tasks(parent_id).await.unwrap();
    assert_eq!(children.len(), 3, "fix-bug template has 3 steps");
    for child in &children {
        assert_eq!(child.parent_task_id, Some(parent_id));
        assert_eq!(child.priority, "high");
        assert_eq!(child.source, "automated");
    }
    assert!(children[0].title.contains("Investigate"));
    assert!(children[1].title.contains("Fix"));
    assert!(children[2].title.contains("Verify"));
}

/// Tests that template expansion creates inter-step dependencies.
///
/// Exercises: `agent_task_deps` table INSERT via `db.expand_template()`,
/// `db.get_task_deps()`.
///
/// The "fix-bug" template defines a sequential pipeline:
/// - Step 0 (Investigate): no dependencies
/// - Step 1 (Fix): depends on step 0
/// - Step 2 (Verify): depends on step 1
///
/// This DAG is materialized as rows in `agent_task_deps` during expansion.
/// The dependency system prevents agents from starting a step before its
/// prerequisites are completed.
#[tokio::test]
async fn template_expand_creates_dependencies() {
    require_db!();
    let db = setup().await;

    let parent_id = db
        .expand_template("fix-bug", "Test deps", "test", "normal", None, 2, None)
        .await
        .unwrap();

    let children = db.get_child_tasks(parent_id).await.unwrap();
    assert_eq!(children.len(), 3);

    // Step 0 (Investigate) has no deps
    let deps0 = db.get_task_deps(children[0].id).await.unwrap();
    assert!(deps0.is_empty());

    // Step 1 (Fix) depends on step 0 (Investigate)
    let deps1 = db.get_task_deps(children[1].id).await.unwrap();
    assert_eq!(deps1, vec![children[0].id]);

    // Step 2 (Verify) depends on step 1 (Fix)
    let deps2 = db.get_task_deps(children[2].id).await.unwrap();
    assert_eq!(deps2, vec![children[1].id]);
}

/// Tests that task claiming respects dependency ordering.
///
/// Exercises: `db.claim_pending_agent_task()`, `agent_task_deps` join check.
///
/// After expanding a template, only step 0 (no deps) should be claimable.
/// Steps 1 and 2 have unmet dependencies (their predecessors are not yet
/// "completed"), so `claim_pending_agent_task` should skip them and return
/// `None` after step 0 is claimed. This enforces the correct execution order
/// in the agent pipeline.
#[tokio::test]
async fn claim_skips_tasks_with_unmet_deps() {
    require_db!();
    let db = setup().await;

    let parent_id = db
        .expand_template("fix-bug", "Dep test", "test", "normal", None, 1, None)
        .await
        .unwrap();

    let children = db.get_child_tasks(parent_id).await.unwrap();
    // Only step 0 should be claimable (no deps)
    let claimed = db.claim_pending_agent_task("test-agent").await.unwrap();
    assert!(claimed.is_some());
    let claimed = claimed.unwrap();
    assert_eq!(claimed.id, children[0].id, "Should claim step 0 (no deps)");

    // Step 1 depends on step 0 which is now in_progress, not completed
    // So nothing more should be claimable
    let next = db.claim_pending_agent_task("test-agent").await.unwrap();
    assert!(
        next.is_none(),
        "Step 1 should not be claimable while step 0 is in_progress"
    );
}

/// Tests that parent tasks (containers) are never directly claimed.
///
/// Exercises: `db.claim_pending_agent_task()` with parent task filtering.
///
/// Creates both a standalone task and a template-expanded task (which has
/// children). Even though the template parent has "urgent" priority (which
/// would normally sort first), `claim_pending_agent_task` should skip it
/// and claim either the standalone task or a child task instead. Parent tasks
/// are orchestration containers -- they complete automatically when all
/// children finish.
#[tokio::test]
async fn claim_skips_parent_tasks() {
    require_db!();
    let db = setup().await;

    // Create a standalone task and a template task
    db.create_agent_task(
        "Standalone",
        "solo task",
        "normal",
        None,
        "manual",
        None,
        1,
        None,
    )
    .await
    .unwrap();
    let _parent_id = db
        .expand_template("fix-bug", "Parent", "test", "urgent", None, 1, None)
        .await
        .unwrap();

    // The parent task has children, so it should be skipped.
    // With urgent priority the parent would be first, but it should be skipped.
    // The first claimable should be the standalone task or the first child.
    let claimed = db
        .claim_pending_agent_task("test-agent")
        .await
        .unwrap()
        .unwrap();
    // Parent should never be claimed (it has children)
    assert_ne!(
        claimed.title, "Parent",
        "Parent tasks should not be claimed directly"
    );
}

/// Tests automatic parent completion when all children finish successfully.
///
/// Exercises: `db.try_complete_parent()`, parent task auto-completion logic.
///
/// Expands a template, completes all 3 child tasks, then calls
/// `try_complete_parent`. The parent should automatically transition to
/// "completed" status. This is the happy-path rollup: when every subtask
/// succeeds, the parent succeeds too.
#[tokio::test]
async fn try_complete_parent_succeeds() {
    require_db!();
    let db = setup().await;

    let parent_id = db
        .expand_template("fix-bug", "Complete test", "test", "normal", None, 1, None)
        .await
        .unwrap();

    let children = db.get_child_tasks(parent_id).await.unwrap();

    // Complete all children
    for child in &children {
        db.complete_agent_task(child.id, "completed", None, 100, 0.01)
            .await
            .unwrap();
    }

    // Now try_complete_parent should succeed
    let result = db.try_complete_parent(parent_id).await.unwrap();
    assert!(result.is_some(), "Parent should be auto-completed");
    let parent = result.unwrap();
    assert_eq!(parent.status, "completed");
}

/// Tests parent failure propagation when a child task fails.
///
/// Exercises: `db.try_complete_parent()` with `on_child_failure = 'fail'` policy.
///
/// Expands a template (default policy: fail parent on child failure), completes
/// the first child, fails the second, and cancels the third. When
/// `try_complete_parent` is called, the parent should be marked as "failed"
/// because at least one child failed and the policy is "fail" (not "continue").
/// This ensures the orchestrator knows the workflow did not succeed.
#[tokio::test]
async fn try_complete_parent_fails_on_child_failure() {
    require_db!();
    let db = setup().await;

    let parent_id = db
        .expand_template("fix-bug", "Fail test", "test", "normal", None, 1, None)
        .await
        .unwrap();

    let children = db.get_child_tasks(parent_id).await.unwrap();

    // Complete first child, fail second, cancel third
    db.complete_agent_task(children[0].id, "completed", None, 100, 0.01)
        .await
        .unwrap();
    db.complete_agent_task(
        children[1].id,
        "failed",
        Some(&serde_json::json!({"error": "test"})),
        50,
        0.005,
    )
    .await
    .unwrap();
    db.cancel_agent_task(children[2].id).await.unwrap();

    // Parent should be marked as failed (on_child_failure = 'fail')
    let result = db.try_complete_parent(parent_id).await.unwrap();
    assert!(result.is_some());
    let parent = result.unwrap();
    assert_eq!(parent.status, "failed");
}

/// Tests permission level inheritance from parent to child tasks.
///
/// Exercises: permission_level capping during `db.expand_template()`.
///
/// The "fix-bug" template defines step permission levels: [0, 1, 1].
/// Child permission levels are capped at `min(step_level, parent_level)`.
///
/// With parent level 1: children get [min(0,1)=0, min(1,1)=1, min(1,1)=1].
/// With parent level 0: all children are capped at 0 regardless of template.
///
/// This implements the principle of least privilege: a constrained parent
/// cannot grant its children more permissions than it has itself.
#[tokio::test]
async fn permission_inheritance() {
    require_db!();
    let db = setup().await;

    // Parent has permission_level 1, template steps request 0 and 1
    let parent_id = db
        .expand_template("fix-bug", "Perm test", "test", "normal", None, 1, None)
        .await
        .unwrap();

    let children = db.get_child_tasks(parent_id).await.unwrap();
    // Step 0 requests level 0, parent level is 1 -> min(0, 1) = 0
    assert_eq!(children[0].permission_level, 0);
    // Step 1 requests level 1, parent level is 1 -> min(1, 1) = 1
    assert_eq!(children[1].permission_level, 1);
    // Step 2 requests level 1, parent level is 1 -> min(1, 1) = 1
    assert_eq!(children[2].permission_level, 1);

    // Now test with a more restrictive parent (level 0)
    let parent_id_0 = db
        .expand_template("fix-bug", "Perm test 0", "test", "normal", None, 0, None)
        .await
        .unwrap();

    let children_0 = db.get_child_tasks(parent_id_0).await.unwrap();
    // All children should be capped at 0
    for child in &children_0 {
        assert_eq!(
            child.permission_level, 0,
            "Child permission should be capped at parent level 0"
        );
    }
}

// == Agent Memory ==============================================================
// Tests for the agent key-value memory store: upsert, retrieve by category,
// update existing keys, and deletion. This persistent memory allows agents
// to learn and remember patterns across task executions.
// ==============================================================================

/// Tests agent memory upsert, retrieval, and category filtering.
///
/// Exercises: `agent_memory` table INSERT/SELECT, `db.upsert_agent_memory()`,
/// `db.get_all_agent_memory()`, `db.get_agent_memory_by_category()`.
///
/// Inserts a memory entry with key "test_key", value "test_value", category "pattern",
/// then verifies it appears in both the full listing and the category-filtered listing.
/// Also verifies that querying a non-existent category returns an empty result.
#[tokio::test]
async fn agent_memory_upsert_and_retrieve() {
    require_db!();
    let db = setup().await;

    let entry = db
        .upsert_agent_memory("test_key", "test_value", "pattern", None)
        .await
        .unwrap();
    assert_eq!(entry.key, "test_key");
    assert_eq!(entry.value, "test_value");
    assert_eq!(entry.category, "pattern");

    let all = db.get_all_agent_memory().await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].key, "test_key");

    let by_cat = db.get_agent_memory_by_category("pattern").await.unwrap();
    assert_eq!(by_cat.len(), 1);

    let empty = db.get_agent_memory_by_category("gotcha").await.unwrap();
    assert!(empty.is_empty());
}

/// Tests that upserting an existing key updates both value and category.
///
/// Exercises: `agent_memory` table UPSERT (INSERT ... ON CONFLICT UPDATE),
/// `db.upsert_agent_memory()` idempotency.
///
/// Inserts a key, then upserts the same key with new value and category.
/// Verifies the update succeeded and there is still only one entry (no duplicate).
/// This is how agents refine their learned patterns over time.
#[tokio::test]
async fn agent_memory_upsert_updates_existing() {
    require_db!();
    let db = setup().await;

    db.upsert_agent_memory("key1", "original", "general", None)
        .await
        .unwrap();

    let updated = db
        .upsert_agent_memory("key1", "updated_value", "gotcha", None)
        .await
        .unwrap();
    assert_eq!(updated.value, "updated_value");
    assert_eq!(updated.category, "gotcha");

    // Still only one entry
    let all = db.get_all_agent_memory().await.unwrap();
    assert_eq!(all.len(), 1);
}

/// Tests agent memory deletion.
///
/// Exercises: `db.delete_agent_memory()`, `agent_memory` table DELETE.
///
/// Inserts a key, deletes it (returns true), attempts to delete again
/// (returns false since it no longer exists), and confirms the table is empty.
#[tokio::test]
async fn agent_memory_delete() {
    require_db!();
    let db = setup().await;

    db.upsert_agent_memory("to_delete", "value", "general", None)
        .await
        .unwrap();

    let deleted = db.delete_agent_memory("to_delete").await.unwrap();
    assert!(deleted);

    let not_found = db.delete_agent_memory("to_delete").await.unwrap();
    assert!(!not_found);

    let all = db.get_all_agent_memory().await.unwrap();
    assert!(all.is_empty());
}

// == Agent Roles ===============================================================
// Tests for the role-based agent system: CRUD on roles, role-template
// associations, and role assignment to tasks. Roles define default permissions,
// models, and cost budgets for different agent specializations.
// ==============================================================================

/// Tests retrieval and validation of the 4 seeded agent roles.
///
/// Exercises: `agent_roles` table SELECT, `db.get_all_roles()`.
///
/// Verifies all 4 seeded roles exist (engine, frontend, ops, research)
/// with correct default permission levels, models, and cost caps. This
/// validates the test seed data in `truncate_all_tables()`.
#[tokio::test]
async fn role_crud_lifecycle() {
    require_db!();
    let db = setup().await;

    // get_all_roles returns the 4 seeded roles
    let roles = db.get_all_roles().await.unwrap();
    assert_eq!(roles.len(), 4, "Should have 4 seeded roles");

    let names: Vec<&str> = roles.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"engine"));
    assert!(names.contains(&"frontend"));
    assert!(names.contains(&"ops"));
    assert!(names.contains(&"research"));

    // Verify engine role fields
    let engine = roles.iter().find(|r| r.name == "engine").unwrap();
    assert_eq!(engine.default_permission_level, 2);
    assert_eq!(engine.default_model, "sonnet");
    assert!(engine.system_prompt.is_some());
    assert_eq!(engine.default_max_cost_usd, Some(5.0));

    // Verify research role defaults
    let research = roles.iter().find(|r| r.name == "research").unwrap();
    assert_eq!(research.default_permission_level, 0);
    assert_eq!(research.default_model, "haiku");
    assert_eq!(research.default_max_cost_usd, Some(1.0));
}

/// Tests looking up a single role by name.
///
/// Exercises: `db.get_role_by_name()`, `agent_roles` table WHERE clause.
///
/// Looks up the "ops" role and verifies its permission level. Also verifies
/// that querying a non-existent role returns `None` (no panic or error).
#[tokio::test]
async fn role_get_by_name() {
    require_db!();
    let db = setup().await;

    let role = db.get_role_by_name("ops").await.unwrap();
    assert!(role.is_some());
    let role = role.unwrap();
    assert_eq!(role.name, "ops");
    assert_eq!(role.default_permission_level, 3);

    // Non-existent role
    let missing = db.get_role_by_name("nonexistent").await.unwrap();
    assert!(missing.is_none());
}

/// Tests the many-to-many association between roles and templates.
///
/// Exercises: `agent_role_templates` table JOIN, `db.get_role_templates()`.
///
/// Verifies that the "engine" and "frontend" roles both have the "fix-bug"
/// template associated (seeded in `truncate_all_tables`), while "research"
/// has no templates. This controls which workflow templates each agent
/// specialization can expand.
#[tokio::test]
async fn role_templates_association() {
    require_db!();
    let db = setup().await;

    // Engine role should have fix-bug template (seeded in test helper)
    let templates = db.get_role_templates("engine").await.unwrap();
    let names: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();
    assert!(
        names.contains(&"fix-bug"),
        "Engine role should have fix-bug template"
    );

    // Frontend role should also have fix-bug
    let frontend_templates = db.get_role_templates("frontend").await.unwrap();
    let frontend_names: Vec<&str> = frontend_templates.iter().map(|t| t.name.as_str()).collect();
    assert!(
        frontend_names.contains(&"fix-bug"),
        "Frontend role should have fix-bug template"
    );

    // Research role has no templates seeded in test helper
    let research_templates = db.get_role_templates("research").await.unwrap();
    assert!(
        research_templates.is_empty(),
        "Research role has no templates in test seed"
    );
}

/// Tests that tasks can be created with an associated role name.
///
/// Exercises: `agent_tasks.role_name` column, `db.create_agent_task()` with
/// `role_name` parameter.
///
/// Creates one task with role "engine" and one without a role. Verifies the
/// role_name is persisted and retrievable, and that `None` is valid for
/// role-agnostic tasks.
#[tokio::test]
async fn task_with_role_stores_role_name() {
    require_db!();
    let db = setup().await;

    // Create task with engine role
    let task = db
        .create_agent_task(
            "Optimize sieve",
            "Improve Montgomery multiplication performance",
            "normal",
            None,
            "manual",
            None,
            2,
            Some("engine"),
        )
        .await
        .unwrap();

    assert_eq!(task.role_name.as_deref(), Some("engine"));

    // Retrieve and verify
    let fetched = db.get_agent_task(task.id).await.unwrap().unwrap();
    assert_eq!(fetched.role_name.as_deref(), Some("engine"));

    // Create task without role
    let task2 = db
        .create_agent_task(
            "Generic task",
            "test",
            "normal",
            None,
            "manual",
            None,
            1,
            None,
        )
        .await
        .unwrap();
    assert!(task2.role_name.is_none());
}

/// Tests that template expansion propagates role_name to parent and children.
///
/// Exercises: `db.expand_template()` with `role_name` parameter, role
/// inheritance in child task generation.
///
/// Expands "fix-bug" with role "engine". Verifies both the parent task and
/// all 3 child tasks have `role_name = "engine"`. This ensures the entire
/// workflow runs under the same agent specialization's permissions and model.
#[tokio::test]
async fn expand_template_with_role() {
    require_db!();
    let db = setup().await;

    let parent_id = db
        .expand_template(
            "fix-bug",
            "Fix engine bug",
            "Sieve returns wrong results",
            "high",
            Some(5.0),
            2,
            Some("engine"),
        )
        .await
        .unwrap();

    // Parent should have role_name set
    let parent = db.get_agent_task(parent_id).await.unwrap().unwrap();
    assert_eq!(parent.role_name.as_deref(), Some("engine"));

    // Children should also have role_name set
    let children = db.get_child_tasks(parent_id).await.unwrap();
    assert_eq!(children.len(), 3);
    for child in &children {
        assert_eq!(
            child.role_name.as_deref(),
            Some("engine"),
            "Child task should inherit role_name"
        );
    }
}

// == Agent Observability =======================================================
// Tests for the agent logging, event tracking, cost analytics, and anomaly
// detection systems. These provide operational visibility into agent behavior
// and spending patterns.
// ==============================================================================

/// Tests batch log insertion and multi-dimensional querying.
///
/// Exercises: `agent_logs` table batch INSERT, `db.insert_agent_logs_batch()`,
/// `db.get_agent_logs()` with stream filtering and pagination, `db.get_agent_log_count()`.
///
/// Inserts 10 log lines (7 stdout, 3 stderr) for a task, then verifies:
/// - All 10 logs retrievable without filter
/// - Stream filter returns only matching logs (7 stdout, 3 stderr)
/// - Offset/limit pagination returns the correct slice
/// - Total count matches inserted rows
#[tokio::test]
async fn agent_log_insert_and_query() {
    require_db!();
    let db = setup().await;

    let task = db
        .create_agent_task(
            "Log test task",
            "Testing logs",
            "normal",
            None,
            "test",
            None,
            1,
            None,
        )
        .await
        .unwrap();

    // Insert 10 log lines: 7 stdout, 3 stderr
    let mut entries: Vec<(String, i32, Option<String>, String)> = Vec::new();
    for i in 1..=7 {
        entries.push((
            "stdout".into(),
            i,
            Some("assistant".into()),
            format!("stdout line {}", i),
        ));
    }
    for i in 1..=3 {
        entries.push(("stderr".into(), i, None, format!("stderr line {}", i)));
    }
    db.insert_agent_logs_batch(task.id, &entries).await.unwrap();

    // Query all logs
    let all = db.get_agent_logs(task.id, None, 0, 100).await.unwrap();
    assert_eq!(all.len(), 10);

    // Query stdout only
    let stdout_only = db
        .get_agent_logs(task.id, Some("stdout"), 0, 100)
        .await
        .unwrap();
    assert_eq!(stdout_only.len(), 7);
    assert!(stdout_only.iter().all(|l| l.stream == "stdout"));

    // Query stderr only
    let stderr_only = db
        .get_agent_logs(task.id, Some("stderr"), 0, 100)
        .await
        .unwrap();
    assert_eq!(stderr_only.len(), 3);

    // Test offset/limit pagination
    let page = db.get_agent_logs(task.id, None, 5, 3).await.unwrap();
    assert_eq!(page.len(), 3);

    // Test count
    let count = db.get_agent_log_count(task.id).await.unwrap();
    assert_eq!(count, 10);
}

/// Tests extended event fields for tool call tracking.
///
/// Exercises: `agent_events` table extended columns, `db.insert_agent_event_ex()`,
/// `db.get_agent_events()`, `db.get_agent_task_timeline()`.
///
/// Inserts an event with tool_name, input/output token counts, and duration.
/// Verifies all extended fields are persisted and appear in both the events
/// list and the task timeline view. These metrics power the agent cost
/// dashboard and performance monitoring.
#[tokio::test]
async fn agent_event_extended_fields() {
    require_db!();
    let db = setup().await;

    let task = db
        .create_agent_task(
            "Event ext test",
            "Testing extended events",
            "normal",
            None,
            "test",
            None,
            1,
            None,
        )
        .await
        .unwrap();

    db.insert_agent_event_ex(
        Some(task.id),
        "tool_call",
        Some("claude"),
        "Tool call: Read",
        None,
        Some("Read"),
        Some(1500),
        Some(300),
        Some(250),
    )
    .await
    .unwrap();

    let events = db.get_agent_events(Some(task.id), 10).await.unwrap();
    assert_eq!(events.len(), 1);
    let ev = &events[0];
    assert_eq!(ev.tool_name.as_deref(), Some("Read"));
    assert_eq!(ev.input_tokens, Some(1500));
    assert_eq!(ev.output_tokens, Some(300));
    assert_eq!(ev.duration_ms, Some(250));

    // Timeline also returns extended fields
    let timeline = db.get_agent_task_timeline(task.id).await.unwrap();
    assert_eq!(timeline.len(), 1);
    assert_eq!(timeline[0].tool_name.as_deref(), Some("Read"));
}

/// Tests daily cost aggregation across multiple agent tasks.
///
/// Exercises: `db.get_agent_daily_costs()`, cost_usd and tokens_used columns,
/// GROUP BY date aggregation.
///
/// Creates 3 tasks with different models and costs (sonnet: $0.05 + $0.03,
/// opus: $0.15), completes them, and verifies the daily cost summary totals
/// $0.23. This powers the agent spending dashboard and budget alerts.
#[tokio::test]
async fn agent_daily_cost_analytics() {
    require_db!();
    let db = setup().await;

    for (model, cost, tokens) in [
        ("sonnet", 0.05, 5000i64),
        ("sonnet", 0.03, 3000),
        ("opus", 0.15, 8000),
    ] {
        let task = db
            .create_agent_task(
                "Cost test",
                "",
                "normal",
                Some(model),
                "test",
                None,
                1,
                None,
            )
            .await
            .unwrap();
        let result = serde_json::json!({"text": "done"});
        db.complete_agent_task(task.id, "completed", Some(&result), tokens, cost)
            .await
            .unwrap();
    }

    let daily = db.get_agent_daily_costs(30).await.unwrap();
    assert!(!daily.is_empty());
    let total_cost: f64 = daily.iter().map(|r| r.total_cost).sum();
    assert!((total_cost - 0.23).abs() < 0.01);
}

/// Tests token usage anomaly detection using statistical thresholds.
///
/// Exercises: `db.get_agent_token_anomalies(z_score_threshold)`, statistical
/// analysis of `agent_tasks.tokens_used` grouped by template.
///
/// Creates 5 tasks under the "anomaly-test" template: 4 normal (1000 tokens)
/// and 1 outlier (10000 tokens). With a z-score threshold of 2.0, the outlier
/// should be flagged as anomalous. This helps detect runaway agents or
/// unexpectedly expensive workflows.
#[tokio::test]
async fn agent_token_anomaly_detection() {
    require_db!();
    let db = setup().await;

    sqlx::raw_sql(
        "INSERT INTO agent_templates (name, description, steps) VALUES
          ('anomaly-test', 'Test template', '[]'::jsonb)
         ON CONFLICT (name) DO NOTHING",
    )
    .execute(db.pool())
    .await
    .unwrap();

    // 4 normal tasks (1000 tokens), 1 outlier (10000 tokens)
    for tokens in [1000i64, 1000, 1000, 1000, 10000] {
        let task = db
            .create_agent_task(
                "Anomaly test",
                "",
                "normal",
                Some("sonnet"),
                "test",
                None,
                1,
                None,
            )
            .await
            .unwrap();
        sqlx::query("UPDATE agent_tasks SET template_name = 'anomaly-test' WHERE id = $1")
            .bind(task.id)
            .execute(db.pool())
            .await
            .unwrap();
        let result = serde_json::json!({"text": "done"});
        db.complete_agent_task(task.id, "completed", Some(&result), tokens, 0.01)
            .await
            .unwrap();
    }

    let anomalies = db.get_agent_token_anomalies(2.0).await.unwrap();
    assert!(!anomalies.is_empty());
    assert!(anomalies.iter().all(|t| t.tokens_used > 5000));
}

// == Operator (Volunteer) Management ===========================================
// Tests for the distributed compute operator system: registration, trust
// progression, credit accounting, node management, block claiming/submission,
// leaderboard ordering, and stale block reclamation. This powers the volunteer
// compute network where external operators contribute CPU cycles.
// ==============================================================================

/// Tests operator registration and API key lookup.
///
/// Exercises: `operators` table INSERT via `db.register_operator()`,
/// `db.get_operator_by_api_key()`.
///
/// Registers a new operator, verifies initial state (zero credit, zero primes),
/// confirms the generated API key is non-empty, then looks up the operator
/// by API key. Also verifies that a non-existent API key returns `None`.
#[tokio::test]
async fn operator_register_and_retrieve() {
    require_db!();
    let db = setup().await;

    // Register a new operator
    let op = db
        .register_operator("alice", "alice@example.com")
        .await
        .unwrap();
    assert_eq!(op.username, "alice");
    assert_eq!(op.email, "alice@example.com");
    assert!(!op.api_key.is_empty(), "API key should be generated");
    assert_eq!(op.credit, 0);
    assert_eq!(op.primes_found, 0);

    // Look up by API key
    let found = db
        .get_operator_by_api_key(&op.api_key)
        .await
        .unwrap()
        .expect("Operator should be found by API key");
    assert_eq!(found.id, op.id);
    assert_eq!(found.username, "alice");

    // Non-existent API key returns None
    let missing = db
        .get_operator_by_api_key("nonexistent-key-12345")
        .await
        .unwrap();
    assert!(missing.is_none());
}

/// Tests the operator trust progression pipeline across trust levels.
///
/// Exercises: `operator_trust` table INSERT/UPDATE, `db.get_operator_trust()`,
/// `db.record_valid_result()`, trust level advancement thresholds.
///
/// This validates the adaptive replication model:
/// - **Level 1** (new operator): all work is double-checked by a trusted worker.
///   Requires 10 consecutive valid results to advance.
/// - **Level 2** (reliable): work is spot-checked (random 10% verification).
///   Requires 100 total valid results to advance.
/// - **Level 3** (trusted): work is accepted without verification.
///
/// The test records 9 valid results (stays at level 1), then the 10th (advances
/// to level 2), then continues to 100 total valid results (advances to level 3).
#[tokio::test]
async fn operator_trust_progression() {
    require_db!();
    let db = setup().await;

    let op = db
        .register_operator("trusty", "trusty@example.com")
        .await
        .unwrap();

    // Initial trust should be level 1
    let trust = db
        .get_operator_trust(op.id)
        .await
        .unwrap()
        .expect("Trust record should exist after registration");
    assert_eq!(trust.trust_level, 1);
    assert_eq!(trust.consecutive_valid, 0);

    // Record 9 valid results -- should stay at level 1
    for _ in 0..9 {
        db.record_valid_result(op.id).await.unwrap();
    }
    let trust = db.get_operator_trust(op.id).await.unwrap().unwrap();
    assert_eq!(trust.trust_level, 1, "9 valid results: still level 1");
    assert_eq!(trust.consecutive_valid, 9);

    // 10th valid result: advance to level 2
    db.record_valid_result(op.id).await.unwrap();
    let trust = db.get_operator_trust(op.id).await.unwrap().unwrap();
    assert_eq!(trust.trust_level, 2, "10 valid results: level 2");
    assert_eq!(trust.consecutive_valid, 10);
    assert_eq!(trust.total_valid, 10);

    // Record more to reach 100 total valid -- advance to level 3
    for _ in 11..=100 {
        db.record_valid_result(op.id).await.unwrap();
    }
    let trust = db.get_operator_trust(op.id).await.unwrap().unwrap();
    assert_eq!(trust.trust_level, 3, "100 valid results: level 3");
    assert_eq!(trust.consecutive_valid, 100);
    assert_eq!(trust.total_valid, 100);
}

/// Tests that a single invalid result resets trust level and consecutive count.
///
/// Exercises: `db.record_invalid_result()`, trust demotion logic.
///
/// Builds an operator up to trust level 2 (15 valid results), then records
/// one invalid result. The operator should be demoted back to level 1 with
/// `consecutive_valid` reset to 0, while `total_valid` is preserved (not penalized
/// retroactively) and `total_invalid` incremented. This ensures bad actors
/// cannot maintain elevated trust.
#[tokio::test]
async fn operator_trust_reset_on_invalid() {
    require_db!();
    let db = setup().await;

    let op = db
        .register_operator("cheater", "cheater@example.com")
        .await
        .unwrap();

    // Build up to level 2
    for _ in 0..15 {
        db.record_valid_result(op.id).await.unwrap();
    }
    let trust = db.get_operator_trust(op.id).await.unwrap().unwrap();
    assert_eq!(trust.trust_level, 2);

    // One invalid result resets trust to level 1 and zeroes consecutive_valid
    db.record_invalid_result(op.id).await.unwrap();
    let trust = db.get_operator_trust(op.id).await.unwrap().unwrap();
    assert_eq!(trust.trust_level, 1, "Invalid result resets to level 1");
    assert_eq!(trust.consecutive_valid, 0);
    assert_eq!(trust.total_invalid, 1);
    assert_eq!(trust.total_valid, 15, "total_valid preserved");
}

/// Tests credit granting and accumulation on operator accounts.
///
/// Exercises: `operator_credits` table INSERT (credit log), `operators.credit`
/// column UPDATE, `db.grant_credit()`.
///
/// Registers an operator, creates a work block (needed for the credit_log FK),
/// grants 100 credit, verifies the balance, then grants 50 more and verifies
/// accumulation to 150. Credits incentivize operators to contribute compute
/// and are displayed on the leaderboard.
#[tokio::test]
async fn operator_credit_grant_and_accumulate() {
    require_db!();
    let db = setup().await;

    let op = db
        .register_operator("miner", "miner@example.com")
        .await
        .unwrap();

    // Create a search job and claim a block so we have a valid block_id for credit_log FK
    db.upsert_worker("credit-worker", "host", 4, "factorial", "")
        .await
        .unwrap();
    let params = serde_json::json!({"form": "factorial", "start": 1, "end": 100});
    let job_id = db
        .create_search_job("factorial", &params, 1, 100, 10)
        .await
        .unwrap();
    let block = db
        .claim_work_block(job_id, "credit-worker")
        .await
        .unwrap()
        .unwrap();

    // Grant 100 credit
    db.grant_credit(op.id, block.block_id as i32, 100, "block_completed")
        .await
        .unwrap();

    // Verify accumulation on operator record
    let found = db
        .get_operator_by_api_key(&op.api_key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.credit, 100);

    // Grant 50 more
    db.grant_credit(op.id, block.block_id as i32, 50, "bonus")
        .await
        .unwrap();
    let found = db
        .get_operator_by_api_key(&op.api_key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.credit, 150, "Credit should accumulate");
}

/// Tests operator node registration, heartbeat, and upsert behavior.
///
/// Exercises: `operator_nodes` table INSERT/UPDATE via `db.register_operator_node()`,
/// `db.operator_node_heartbeat()`, `db.get_operator_leaderboard()`.
///
/// Registers a worker node with full hardware capabilities (CPU, RAM, GPU, OS, arch),
/// sends a heartbeat, then re-registers the same worker_id with updated specs
/// (upsert behavior). Verifies the leaderboard shows exactly 1 worker for the
/// operator (the upsert did not create a duplicate).
#[tokio::test]
async fn operator_node_register_and_heartbeat() {
    require_db!();
    let db = setup().await;

    let op = db
        .register_operator("noderunner", "nodes@example.com")
        .await
        .unwrap();

    // Register a worker node
    db.register_operator_node(
        op.id,
        "node-alpha",
        "alpha.local",
        16,
        "Apple M1 Ultra",
        Some("darwin"),
        Some("aarch64"),
        Some(64),
        Some(true),
        Some("Apple M1 GPU"),
        Some(16),
        Some("0.5.0"),
        Some("stable"),
    )
    .await
    .unwrap();

    // Heartbeat updates timestamp (no error)
    db.operator_node_heartbeat("node-alpha").await.unwrap();

    // Register a second node for the same operator (upsert by worker_id)
    db.register_operator_node(
        op.id,
        "node-alpha",
        "alpha-updated.local",
        32,
        "Apple M2 Ultra",
        Some("darwin"),
        Some("aarch64"),
        Some(128),
        None,
        None,
        None,
        Some("0.6.0"),
        Some("beta"),
    )
    .await
    .unwrap();

    // Verify the leaderboard shows 1 worker for this operator
    let leaderboard = db.get_operator_leaderboard(10).await.unwrap();
    let entry = leaderboard.iter().find(|e| e.username == "noderunner");
    assert!(entry.is_some(), "Operator should appear on leaderboard");
    assert_eq!(
        entry.unwrap().worker_count,
        Some(1),
        "Should have 1 worker (upsert, not duplicate)"
    );
}

/// Tests the full operator work cycle: claim block -> submit result.
///
/// Exercises: `db.claim_operator_block()` (capability-aware block assignment),
/// `db.submit_operator_result()`, `db.get_job_block_summary()`.
///
/// Registers an operator, creates a search job with 5 blocks, claims one block
/// using hardware capabilities, submits results (100 tested, 3 found), and
/// verifies the job summary reflects the completed work. This is the core
/// volunteer compute loop.
#[tokio::test]
async fn operator_block_claim_and_submit() {
    require_db!();
    let db = setup().await;

    let op = db
        .register_operator("claimer", "claimer@example.com")
        .await
        .unwrap();

    // Create a search job with blocks
    let params = serde_json::json!({"form": "factorial", "start": 1, "end": 500});
    let job_id = db
        .create_search_job("factorial", &params, 1, 500, 100)
        .await
        .unwrap();

    // Claim a block
    let caps = darkreach::db::operators::WorkerCapabilities {
        cores: 8,
        ram_gb: 32,
        has_gpu: false,
        os: Some("linux".into()),
        arch: Some("x86_64".into()),
    };
    let block = db
        .claim_operator_block(op.id, &caps)
        .await
        .unwrap()
        .expect("Should claim a block");
    assert_eq!(block.block_start, 1);
    assert_eq!(block.block_end, 101);
    assert_eq!(block.search_job_id, job_id);
    assert_eq!(block.search_type.as_deref(), Some("factorial"));

    // Submit result
    db.submit_operator_result(block.block_id, 100, 3)
        .await
        .unwrap();

    // Verify block summary shows 1 completed
    let summary = db.get_job_block_summary(job_id).await.unwrap();
    assert_eq!(summary.completed, 1);
    assert_eq!(summary.total_tested, 100);
    assert_eq!(summary.total_found, 3);
}

/// Tests leaderboard ordering by credit (descending).
///
/// Exercises: `db.get_operator_leaderboard()`, `operators` table ORDER BY credit DESC.
///
/// Creates 3 operators with different credit levels (10, 500, 1000) and
/// verifies the leaderboard returns them in descending credit order:
/// top_rank (1000), mid_rank (500), low_rank (10). This is the public-facing
/// competitive ranking displayed on the dashboard.
#[tokio::test]
async fn operator_leaderboard_ordering() {
    require_db!();
    let db = setup().await;

    // Create 3 operators with different credit levels
    let op1 = db
        .register_operator("low_rank", "low@example.com")
        .await
        .unwrap();
    let op2 = db
        .register_operator("mid_rank", "mid@example.com")
        .await
        .unwrap();
    let op3 = db
        .register_operator("top_rank", "top@example.com")
        .await
        .unwrap();

    // Create a dummy search job and block for credit_log FK
    db.upsert_worker("lb-worker", "host", 4, "factorial", "")
        .await
        .unwrap();
    let params = serde_json::json!({"form": "factorial"});
    let job_id = db
        .create_search_job("factorial", &params, 1, 1000, 100)
        .await
        .unwrap();
    let block = db
        .claim_work_block(job_id, "lb-worker")
        .await
        .unwrap()
        .unwrap();

    // Grant different credits
    db.grant_credit(op1.id, block.block_id as i32, 10, "test")
        .await
        .unwrap();
    db.grant_credit(op2.id, block.block_id as i32, 500, "test")
        .await
        .unwrap();
    db.grant_credit(op3.id, block.block_id as i32, 1000, "test")
        .await
        .unwrap();

    let leaderboard = db.get_operator_leaderboard(10).await.unwrap();
    assert_eq!(leaderboard.len(), 3);

    // Leaderboard should be ordered by credit descending
    assert_eq!(leaderboard[0].username, "top_rank");
    assert_eq!(leaderboard[0].credit, 1000);
    assert_eq!(leaderboard[1].username, "mid_rank");
    assert_eq!(leaderboard[1].credit, 500);
    assert_eq!(leaderboard[2].username, "low_rank");
    assert_eq!(leaderboard[2].credit, 10);
}

/// Tests automatic reclamation of stale (timed-out) operator work blocks.
///
/// Exercises: `db.reclaim_stale_operator_blocks(timeout_secs)`, `work_blocks`
/// table UPDATE resetting status from "claimed" to "available".
///
/// Claims a block, artificially ages its `claimed_at` to 25 hours ago, then
/// runs reclamation with a 24-hour timeout. The block should be released back
/// to the available pool. This prevents work from being permanently locked by
/// operators who disconnect without submitting results.
#[tokio::test]
async fn operator_stale_block_reclaim() {
    require_db!();
    let db = setup().await;

    let op = db
        .register_operator("stale_op", "stale@example.com")
        .await
        .unwrap();

    // Create a search job with blocks
    let params = serde_json::json!({"form": "factorial", "start": 1, "end": 300});
    let job_id = db
        .create_search_job("factorial", &params, 1, 300, 100)
        .await
        .unwrap();

    // Claim a block
    let caps = darkreach::db::operators::WorkerCapabilities {
        cores: 4,
        ram_gb: 8,
        has_gpu: false,
        os: None,
        arch: None,
    };
    let block = db
        .claim_operator_block(op.id, &caps)
        .await
        .unwrap()
        .expect("Should claim a block");

    // Artificially age the claimed_at to simulate a stale block
    sqlx::query(
        "UPDATE work_blocks SET claimed_at = NOW() - INTERVAL '25 hours' WHERE id = $1",
    )
    .bind(block.block_id)
    .execute(db.pool())
    .await
    .unwrap();

    // Reclaim stale blocks with 24-hour timeout (86400 seconds)
    let reclaimed = db.reclaim_stale_operator_blocks(86400).await.unwrap();
    assert_eq!(reclaimed, 1, "Should reclaim 1 stale block");

    // The block should now be available again
    let summary = db.get_job_block_summary(job_id).await.unwrap();
    assert_eq!(summary.available, 3, "All 3 blocks should be available");
    assert_eq!(summary.claimed, 0, "No blocks should be claimed");
}

// == Project Management ========================================================
// Tests for the multi-phase project system: project creation with phase
// definitions, status transitions (draft -> active -> completed), phase
// activation with search job linking, and event logging.
// ==============================================================================

/// Tests project creation with multi-phase configuration.
///
/// Exercises: `projects` table INSERT, `project_phases` table batch INSERT,
/// `db.create_project()`, `db.get_projects()`, `db.get_project_by_slug()`,
/// `db.get_project_phases()`.
///
/// Creates a factorial survey project with two phases ("sweep" and "extend"),
/// where "extend" depends on "sweep". Verifies:
/// - Project metadata (name, form, objective, status=draft)
/// - Slug generation from project name
/// - Phase ordering (sweep=0, extend=1)
/// - Phase dependency declarations
/// - Default status (pending for all phases)
#[tokio::test]
async fn project_create_and_phases() {
    require_db!();
    let db = setup().await;

    let config = darkreach::project::ProjectConfig {
        project: darkreach::project::ProjectMeta {
            name: "Test Factorial Survey".to_string(),
            description: "Integration test project".to_string(),
            objective: darkreach::project::Objective::Survey,
            form: "factorial".to_string(),
            author: "test".to_string(),
            tags: vec!["test".to_string()],
        },
        target: darkreach::project::TargetConfig {
            target_digits: None,
            range_start: Some(1),
            range_end: Some(1000),
        },
        competitive: None,
        strategy: darkreach::project::StrategyConfig {
            auto_strategy: false,
            phases: vec![
                darkreach::project::PhaseConfig {
                    name: "sweep".to_string(),
                    description: "First sweep".to_string(),
                    search_params: serde_json::json!({
                        "search_type": "factorial",
                        "start": 1,
                        "end": 500,
                    }),
                    block_size: Some(100),
                    depends_on: None,
                    activation_condition: None,
                    completion: "all_blocks_done".to_string(),
                },
                darkreach::project::PhaseConfig {
                    name: "extend".to_string(),
                    description: "Extend search".to_string(),
                    search_params: serde_json::json!({
                        "search_type": "factorial",
                        "start": 501,
                        "end": 1000,
                    }),
                    block_size: Some(100),
                    depends_on: Some(vec!["sweep".to_string()]),
                    activation_condition: None,
                    completion: "all_blocks_done".to_string(),
                },
            ],
        },
        infrastructure: None,
        budget: None,
        workers: None,
    };

    let project_id = db.create_project(&config, None).await.unwrap();
    assert!(project_id > 0);

    // Retrieve the project
    let projects = db.get_projects(None).await.unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "Test Factorial Survey");
    assert_eq!(projects[0].form, "factorial");
    assert_eq!(projects[0].objective, "survey");
    assert_eq!(projects[0].status, "draft");

    // Retrieve by slug
    let slug_project = db
        .get_project_by_slug("test-factorial-survey")
        .await
        .unwrap()
        .expect("Should find project by slug");
    assert_eq!(slug_project.id, project_id);

    // Retrieve phases
    let phases = db.get_project_phases(project_id).await.unwrap();
    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0].name, "sweep");
    assert_eq!(phases[0].phase_order, 0);
    assert_eq!(phases[0].status, "pending");
    assert_eq!(phases[0].block_size, 100);
    assert!(phases[0].depends_on.is_empty());
    assert_eq!(phases[1].name, "extend");
    assert_eq!(phases[1].phase_order, 1);
    assert_eq!(phases[1].depends_on, vec!["sweep".to_string()]);
}

/// Tests the full project and phase status transition lifecycle.
///
/// Exercises: `db.update_project_status()`, `db.activate_phase()`,
/// `db.update_phase_status()`, timestamp auto-population.
///
/// Walks through the complete lifecycle:
/// 1. Project starts as "draft" (no started_at)
/// 2. Project -> "active" (sets started_at)
/// 3. Phase: "pending" -> "active" via activate_phase (links search job, sets started_at)
/// 4. Phase: "active" -> "completed" (sets completed_at)
/// 5. Project -> "completed" (sets completed_at)
///
/// This mirrors how the project orchestrator drives multi-phase search campaigns.
#[tokio::test]
async fn project_phase_status_transitions() {
    require_db!();
    let db = setup().await;

    let config = darkreach::project::ProjectConfig {
        project: darkreach::project::ProjectMeta {
            name: "Phase Transition Test".to_string(),
            description: "".to_string(),
            objective: darkreach::project::Objective::Survey,
            form: "factorial".to_string(),
            author: "test".to_string(),
            tags: vec![],
        },
        target: darkreach::project::TargetConfig {
            target_digits: None,
            range_start: Some(1),
            range_end: Some(100),
        },
        competitive: None,
        strategy: darkreach::project::StrategyConfig {
            auto_strategy: false,
            phases: vec![darkreach::project::PhaseConfig {
                name: "sweep".to_string(),
                description: "Single phase".to_string(),
                search_params: serde_json::json!({
                    "search_type": "factorial",
                    "start": 1,
                    "end": 100,
                }),
                block_size: Some(50),
                depends_on: None,
                activation_condition: None,
                completion: "all_blocks_done".to_string(),
            }],
        },
        infrastructure: None,
        budget: None,
        workers: None,
    };

    let project_id = db.create_project(&config, None).await.unwrap();

    // Project starts as draft
    let project = db
        .get_project_by_slug("phase-transition-test")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(project.status, "draft");
    assert!(project.started_at.is_none());

    // Transition to active
    db.update_project_status(project_id, "active").await.unwrap();
    let project = db
        .get_project_by_slug("phase-transition-test")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(project.status, "active");
    assert!(project.started_at.is_some());

    // Phase: pending -> active (simulated via activate_phase with a dummy job)
    let phases = db.get_project_phases(project_id).await.unwrap();
    let phase = &phases[0];
    assert_eq!(phase.status, "pending");

    // Create a search job for the phase
    let job_id = db
        .create_search_job(
            "factorial",
            &serde_json::json!({"start": 1, "end": 100}),
            1,
            100,
            50,
        )
        .await
        .unwrap();
    db.activate_phase(phase.id, job_id).await.unwrap();

    let phases = db.get_project_phases(project_id).await.unwrap();
    assert_eq!(phases[0].status, "active");
    assert!(phases[0].started_at.is_some());
    assert_eq!(phases[0].search_job_id, Some(job_id));

    // Phase: active -> completed
    db.update_phase_status(phase.id, "completed").await.unwrap();
    let phases = db.get_project_phases(project_id).await.unwrap();
    assert_eq!(phases[0].status, "completed");
    assert!(phases[0].completed_at.is_some());

    // Project: active -> completed
    db.update_project_status(project_id, "completed")
        .await
        .unwrap();
    let project = db
        .get_project_by_slug("phase-transition-test")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(project.status, "completed");
    assert!(project.completed_at.is_some());
}

/// Tests the project event logging system.
///
/// Exercises: `project_events` table INSERT/SELECT, `db.insert_project_event()`,
/// `db.get_project_events()`.
///
/// Creates a project and inserts 3 events of different types (phase_activated,
/// phase_completed, budget_alert). Verifies:
/// - Events are returned in reverse chronological order (most recent first)
/// - Event types and messages are preserved
/// - JSON detail payloads are stored and retrievable
/// - All events reference the correct project_id
/// - The LIMIT parameter works correctly
///
/// This audit trail is displayed on the project detail page and used for
/// debugging campaign progress.
#[tokio::test]
async fn project_event_logging() {
    require_db!();
    let db = setup().await;

    let config = darkreach::project::ProjectConfig {
        project: darkreach::project::ProjectMeta {
            name: "Event Log Test".to_string(),
            description: "".to_string(),
            objective: darkreach::project::Objective::Custom,
            form: "factorial".to_string(),
            author: "test".to_string(),
            tags: vec![],
        },
        target: darkreach::project::TargetConfig::default(),
        competitive: None,
        strategy: darkreach::project::StrategyConfig {
            auto_strategy: true,
            phases: vec![],
        },
        infrastructure: None,
        budget: None,
        workers: None,
    };

    let project_id = db.create_project(&config, None).await.unwrap();

    // Insert several events
    db.insert_project_event(
        project_id,
        "phase_activated",
        "Phase 'sweep' activated: search job 1",
        Some(&serde_json::json!({"phase_id": 1, "search_job_id": 1})),
    )
    .await
    .unwrap();

    db.insert_project_event(
        project_id,
        "phase_completed",
        "Phase 'sweep' completed: 500 tested, 3 found",
        None,
    )
    .await
    .unwrap();

    db.insert_project_event(
        project_id,
        "budget_alert",
        "Cost alert: $4.50 >= $4.00 threshold",
        None,
    )
    .await
    .unwrap();

    // Retrieve events (ordered by created_at DESC)
    let events = db.get_project_events(project_id, 10).await.unwrap();
    assert_eq!(events.len(), 3);

    // Most recent event first
    assert_eq!(events[0].event_type, "budget_alert");
    assert_eq!(events[1].event_type, "phase_completed");
    assert_eq!(events[2].event_type, "phase_activated");

    // Verify detail JSON is preserved
    let activated = &events[2];
    assert!(activated.detail.is_some());
    let detail = activated.detail.as_ref().unwrap();
    assert_eq!(detail["phase_id"], 1);
    assert_eq!(detail["search_job_id"], 1);

    // Verify event belongs to the correct project
    for event in &events {
        assert_eq!(event.project_id, project_id);
    }

    // Limit works
    let limited = db.get_project_events(project_id, 1).await.unwrap();
    assert_eq!(limited.len(), 1);
}
