//! Shared test infrastructure for darkreach integration tests.
//!
//! This module provides database setup, schema migration, and table truncation
//! helpers used by all integration test files (`db_integration`, `api_integration`,
//! `security_tests`). It ensures each test starts with a clean, consistent database
//! state while avoiding redundant migration runs across the test suite.
//!
//! # Prerequisites
//!
//! - A running PostgreSQL instance (local or Docker).
//! - The `TEST_DATABASE_URL` environment variable pointing to the test database.
//!   Example: `postgres://user:pass@localhost:5432/darkreach_test`
//!
//! # Architecture
//!
//! ```text
//! ensure_schema() ──[Once]──> run_migrations()   (one-time DDL setup)
//!       |
//! setup_test_db() ──────────> truncate_all_tables()  (per-test DML reset)
//!       |                         |
//!       |                         +──> re-seed reference data
//!       v                              (roles, templates, budgets)
//!   Database instance
//! ```
//!
//! The `Once` guard in `ensure_schema` means migrations run exactly once per
//! `cargo test` invocation, regardless of how many tests call `setup_test_db`.
//! Table truncation runs before every individual test to guarantee isolation.
//!
//! # Usage
//!
//! ```rust,ignore
//! mod common;
//!
//! #[tokio::test]
//! async fn my_test() {
//!     if !common::has_test_db() { return; }
//!     let db = common::setup_test_db().await;
//!     // ... test with a clean database
//! }
//! ```
//!
//! For API-level tests, use `build_test_app()` which returns a fully wired
//! Axum router backed by the test database.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Once;

/// Returns the test database URL from the `TEST_DATABASE_URL` environment variable.
///
/// # Panics
///
/// Panics if `TEST_DATABASE_URL` is not set. This is intentional: any code path
/// that calls this function should already be guarded by `has_test_db()` or the
/// `require_db!()` macro, so a missing variable indicates a programming error
/// in the test harness rather than an expected skip condition.
pub fn test_db_url() -> String {
    std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL must be set for integration tests")
}

/// Returns `true` if the `TEST_DATABASE_URL` environment variable is set.
///
/// Used as a guard at the top of integration tests so they skip gracefully
/// in environments without a test database (e.g., CI lint jobs, local dev
/// without Docker). The `require_db!()` macro in each test file wraps this
/// check with an early return and a diagnostic `eprintln`.
pub fn has_test_db() -> bool {
    std::env::var("TEST_DATABASE_URL").is_ok()
}

/// One-time schema initialization guard.
///
/// `std::sync::Once` ensures `run_migrations` executes at most once per process,
/// even when multiple `#[tokio::test]` functions run concurrently or sequentially.
/// This avoids redundant DDL operations and "table already exists" errors.
static SCHEMA_INIT: Once = Once::new();

/// Runs all database migrations exactly once per test suite invocation.
///
/// Creates a temporary Tokio runtime to execute the async migration runner
/// synchronously inside the `Once::call_once` closure. Subsequent calls are
/// no-ops. This is safe because DDL (CREATE TABLE, etc.) is idempotent after
/// the first successful run within a test database lifecycle.
pub fn ensure_schema() {
    SCHEMA_INIT.call_once(|| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = sqlx::PgPool::connect(&test_db_url()).await.unwrap();
            run_migrations(&pool).await;
        });
    });
}

/// Creates a fresh, empty test database connection with schema guaranteed.
///
/// This is the primary entry point for database integration tests. It:
///
/// 1. Calls `ensure_schema()` to run migrations (idempotent after first call).
/// 2. Connects to the test database via `Database::connect`.
/// 3. Calls `truncate_all_tables()` to wipe all data and re-seed reference rows.
///
/// The resulting `Database` handle is ready for test operations with a clean
/// slate -- no leftover data from previous tests.
///
/// # Panics
///
/// Panics if the database connection fails. This is acceptable in a test context
/// since a missing database means the test environment is misconfigured.
pub async fn setup_test_db() -> darkreach::db::Database {
    ensure_schema();
    let db = darkreach::db::Database::connect(&test_db_url())
        .await
        .expect("Failed to connect to test database");
    truncate_all_tables(db.pool()).await;
    db
}

/// Builds a complete Axum test router wired to the test database.
///
/// This constructs the same application router used in production (`dashboard::build_router`)
/// but backed by the test database. It includes all API routes, middleware (CORS, body
/// limits), and application state. The checkpoint path is set to a temporary location
/// (`/tmp/darkreach-test-checkpoint`) since checkpoint persistence is not under test.
///
/// Used by `api_integration.rs` and `security_tests.rs` for HTTP-level testing
/// via `tower::ServiceExt::oneshot`.
pub async fn build_test_app() -> axum::Router {
    let db = setup_test_db().await;
    let state = darkreach::dashboard::AppState::with_db(
        db,
        &test_db_url(),
        PathBuf::from("/tmp/darkreach-test-checkpoint"),
    );
    darkreach::dashboard::build_router(state, None)
}

/// Truncates all application tables and re-seeds required reference data.
///
/// # Truncation strategy
///
/// Uses a single `TRUNCATE ... CASCADE` statement covering every application table
/// in dependency order. The `CASCADE` clause handles foreign key relationships
/// automatically, ensuring no orphaned references remain.
///
/// Tables truncated (grouped by domain):
/// - **Agent system**: `agent_logs`, `agent_schedules`, `agent_role_templates`,
///   `agent_task_deps`, `agent_memory`, `agent_events`, `agent_tasks`,
///   `agent_budgets`, `agent_templates`, `agent_roles`
/// - **Projects**: `project_events`, `project_phases`, `projects`
/// - **Operators**: `operator_credits`, `operator_trust`, `operator_nodes`, `operators`
/// - **Calibration**: `cost_calibration`
/// - **Observability**: `metric_rollups_daily`, `metric_rollups_hourly`,
///   `metric_samples`, `system_logs`
/// - **Coordination**: `work_blocks`, `search_jobs`, `workers`
/// - **Core**: `primes`
///
/// # Re-seeded reference data
///
/// After truncation, the following reference rows are inserted to satisfy
/// foreign key constraints and provide test fixtures:
///
/// 1. **Agent roles** (4 rows): `engine`, `frontend`, `ops`, `research` --
///    with domain-appropriate permission levels, models, and cost caps.
/// 2. **Agent templates** (1 row): `fix-bug` -- a 3-step workflow
///    (Investigate -> Fix -> Verify) used by template expansion tests.
/// 3. **Role-template associations** (2 rows): `engine` and `frontend` both
///    map to the `fix-bug` template.
/// 4. **Agent budgets** (3 rows): daily ($10), weekly ($50), monthly ($150)
///    budget periods for cost control tests.
///
/// # Why re-seed?
///
/// Many tests depend on reference data existing (e.g., `expand_template("fix-bug", ...)`
/// requires the template row to exist). Rather than having each test insert its
/// own fixtures, we centralize seeding here for consistency and brevity.
pub async fn truncate_all_tables(pool: &sqlx::PgPool) {
    sqlx::raw_sql(
        "TRUNCATE TABLE agent_logs, agent_schedules, agent_role_templates, agent_task_deps, agent_memory,
                       agent_events, agent_tasks, agent_budgets, agent_templates,
                       agent_roles, project_events, project_phases, projects,
                       operator_credits, operator_trust, operator_nodes, operators,
                       cost_calibration,
                       metric_rollups_daily, metric_rollups_hourly, metric_samples, system_logs,
                       work_blocks, search_jobs, workers, primes
         CASCADE",
    )
    .execute(pool)
    .await
    .unwrap();

    // Re-seed agent roles: four specialist roles with escalating permission levels.
    // These match the project's domain model where each AI agent role has a
    // default permission ceiling, preferred model, and cost budget.
    sqlx::raw_sql(
        "INSERT INTO agent_roles (name, description, domains, default_permission_level, default_model, system_prompt, default_max_cost_usd) VALUES
          ('engine', 'Engine specialist', '[\"engine\"]', 2, 'sonnet', 'Engine role prompt', 5.00),
          ('frontend', 'Frontend specialist', '[\"frontend\"]', 2, 'sonnet', 'Frontend role prompt', 3.00),
          ('ops', 'Ops specialist', '[\"deploy\",\"server\"]', 3, 'sonnet', 'Ops role prompt', 10.00),
          ('research', 'Research analyst', '[\"docs\"]', 0, 'haiku', 'Research role prompt', 1.00)",
    )
    .execute(pool)
    .await
    .unwrap();

    // Re-seed the "fix-bug" workflow template: a 3-step sequential pipeline.
    // Steps: Investigate (perm 0) -> Fix (perm 1, depends on step 0) -> Verify (perm 1, depends on step 1).
    // The `depends_on_step` field creates a DAG enforced by `agent_task_deps` at expansion time.
    sqlx::raw_sql(
        "INSERT INTO agent_templates (name, description, steps) VALUES
          ('fix-bug', 'Bug fix workflow: investigate, fix, verify',
           '[{\"title\":\"Investigate\",\"description\":\"Find root cause\",\"permission_level\":0},{\"title\":\"Fix\",\"description\":\"Implement fix\",\"permission_level\":1,\"depends_on_step\":0},{\"title\":\"Verify\",\"description\":\"Run tests\",\"permission_level\":1,\"depends_on_step\":1}]'::jsonb)
         ON CONFLICT (name) DO NOTHING",
    )
    .execute(pool)
    .await
    .unwrap();

    // Re-seed role-template associations so `engine` and `frontend` roles can
    // expand the `fix-bug` template. The `research` role intentionally has no
    // templates to test the empty-association case.
    sqlx::raw_sql(
        "INSERT INTO agent_role_templates (role_name, template_name) VALUES
          ('engine', 'fix-bug'),
          ('frontend', 'fix-bug')
         ON CONFLICT DO NOTHING",
    )
    .execute(pool)
    .await
    .unwrap();

    // Re-seed budget periods for agent cost control tests.
    // These represent the maximum spend allowed per time window.
    sqlx::raw_sql(
        "INSERT INTO agent_budgets (period, budget_usd) VALUES
          ('daily', 10.00), ('weekly', 50.00), ('monthly', 150.00)",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// Runs all database migrations against the test database in order.
///
/// # Migration execution order
///
/// Migrations are applied sequentially in the order listed below. This mirrors
/// the production migration order maintained in `supabase/migrations/`. Each
/// migration builds on the schema established by previous ones:
///
/// 1. `001_create_primes.sql` -- Core `primes` table (expression, form, digits, proof)
/// 2. `002_create_functions.sql` -- Database functions (statistics, aggregates)
/// 3. `004_coordination_tables.sql` -- `workers`, `search_jobs`, `work_blocks`
/// 4. `005_verification.sql` -- Verification pipeline tables
/// 5. `006_agents.sql` -- Agent task system (`agent_tasks`, `agent_events`)
/// 6. `007_agent_cost_control.sql` -- `agent_budgets`, cost tracking columns
/// 7. `008_agent_permissions.sql` -- Permission levels, role-based access
/// 8. `009_agent_memory.sql` -- `agent_memory` key-value store
/// 9. `010_task_decomposition.sql` -- `agent_templates`, `agent_task_deps`, child tasks
/// 10. `011_projects.sql` -- `projects`, `project_phases`, `project_events`
/// 11. `012_form_leaderboard.sql` -- Per-form leaderboard views
/// 12. `0121_project_cost_tracking.sql` -- Project-level cost aggregation
/// 13. `013_agent_roles.sql` -- `agent_roles`, `agent_role_templates`
/// 14. `014_agent_schedules.sql` -- `agent_schedules` for periodic tasks
/// 15. `015_add_certificate.sql` -- Primality certificate column on `primes`
/// 16. `016_lifecycle_management.sql` -- Task lifecycle (cancel, timeout)
/// 17. `017_cost_calibration.sql` -- `cost_calibration` for token cost estimates
/// 18. `018_agent_observability.sql` -- `agent_logs`, extended event fields
/// 19. `019_volunteers.sql` -- `operators`, `operator_trust`, `operator_nodes`, `operator_credits`
/// 20. `020_observability.sql` -- `metric_samples`, `metric_rollups_hourly`, `system_logs`
/// 21. `021_volunteer_worker_capabilities.sql` -- Worker hardware capability columns
/// 22. `022_worker_release_channels.sql` -- Release channel management tables
/// 23. `023_volunteer_worker_release_tracking.sql` -- Per-worker release version tracking
/// 24. `024_metric_rollups_daily.sql` -- Daily metric rollup materialization
/// 25. `025_operator_rename.sql` -- Rename volunteers -> operators (terminology change)
///
/// Note: Migration `003` is intentionally absent (superseded by later migrations).
///
/// # Supabase compatibility
///
/// Each migration SQL is passed through `clean_migration_sql()` to strip
/// Supabase-specific directives (RLS policies, ALTER PUBLICATION for Realtime)
/// that would fail on a plain PostgreSQL instance.
///
/// # Panics
///
/// Panics with a descriptive message if any migration file is missing or fails
/// to execute, since a broken schema makes all subsequent tests meaningless.
async fn run_migrations(pool: &sqlx::PgPool) {
    let migration_files = [
        "supabase/migrations/001_create_primes.sql",
        "supabase/migrations/002_create_functions.sql",
        "supabase/migrations/004_coordination_tables.sql",
        "supabase/migrations/005_verification.sql",
        "supabase/migrations/006_agents.sql",
        "supabase/migrations/007_agent_cost_control.sql",
        "supabase/migrations/008_agent_permissions.sql",
        "supabase/migrations/009_agent_memory.sql",
        "supabase/migrations/010_task_decomposition.sql",
        "supabase/migrations/011_projects.sql",
        "supabase/migrations/012_form_leaderboard.sql",
        "supabase/migrations/0121_project_cost_tracking.sql",
        "supabase/migrations/013_agent_roles.sql",
        "supabase/migrations/014_agent_schedules.sql",
        "supabase/migrations/015_add_certificate.sql",
        "supabase/migrations/016_lifecycle_management.sql",
        "supabase/migrations/017_cost_calibration.sql",
        "supabase/migrations/018_agent_observability.sql",
        "supabase/migrations/019_volunteers.sql",
        "supabase/migrations/020_observability.sql",
        "supabase/migrations/021_volunteer_worker_capabilities.sql",
        "supabase/migrations/022_worker_release_channels.sql",
        "supabase/migrations/023_volunteer_worker_release_tracking.sql",
        "supabase/migrations/024_metric_rollups_daily.sql",
        "supabase/migrations/025_operator_rename.sql",
    ];

    for file in &migration_files {
        let path = std::path::Path::new(file);
        if !path.exists() {
            panic!("Migration file not found: {}", file);
        }
        let sql = std::fs::read_to_string(path).unwrap();
        let cleaned = clean_migration_sql(&sql);
        if !cleaned.trim().is_empty() {
            sqlx::raw_sql(&cleaned)
                .execute(pool)
                .await
                .unwrap_or_else(|e| {
                    panic!("Migration {} failed: {}", file, e);
                });
        }
    }
}

/// Strips Supabase-specific SQL directives that fail on plain PostgreSQL.
///
/// Supabase migrations often include:
/// - `ALTER PUBLICATION supabase_realtime ...` -- Realtime replication setup
/// - `ENABLE ROW LEVEL SECURITY` -- RLS requires Supabase auth context
/// - `CREATE POLICY ...` -- RLS policies reference `auth.uid()` which does not exist
///
/// These are production-only concerns. The test database uses direct connections
/// without Supabase's auth layer, so these statements would cause errors.
/// Stripping them lets us reuse the same migration files for both environments.
fn clean_migration_sql(sql: &str) -> String {
    sql.lines()
        .filter(|line| {
            let t = line.trim();
            !t.starts_with("ALTER PUBLICATION")
                && !t.contains("ENABLE ROW LEVEL SECURITY")
                && !t.starts_with("CREATE POLICY")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
