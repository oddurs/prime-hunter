//! Shared test helpers for integration tests.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Once;

/// Returns the test database URL from the `TEST_DATABASE_URL` environment variable.
/// Panics if the variable is not set.
pub fn test_db_url() -> String {
    std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must be set for integration tests")
}

/// Returns true if the test database URL is configured.
pub fn has_test_db() -> bool {
    std::env::var("TEST_DATABASE_URL").is_ok()
}

/// One-time schema initialization.
static SCHEMA_INIT: Once = Once::new();

/// Ensure the test database schema is set up (runs migrations once per test suite).
pub fn ensure_schema() {
    SCHEMA_INIT.call_once(|| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = sqlx::PgPool::connect(&test_db_url()).await.unwrap();
            run_migrations(&pool).await;
        });
    });
}

/// Connect to the test database (also ensures schema is set up).
pub async fn setup_test_db() -> primehunt::db::Database {
    ensure_schema();
    let db = primehunt::db::Database::connect(&test_db_url())
        .await
        .expect("Failed to connect to test database");
    truncate_all_tables(db.pool()).await;
    db
}

/// Build an Axum test app router connected to the test database.
pub async fn build_test_app() -> axum::Router {
    let db = setup_test_db().await;
    let state = primehunt::dashboard::AppState::with_db(
        db,
        &test_db_url(),
        PathBuf::from("/tmp/primehunt-test-checkpoint"),
        0,
    );
    primehunt::dashboard::build_router(state, None)
}

/// Truncate all tables to ensure test isolation.
pub async fn truncate_all_tables(pool: &sqlx::PgPool) {
    sqlx::raw_sql(
        "TRUNCATE TABLE agent_role_templates, agent_task_deps, agent_memory,
                       agent_events, agent_tasks, agent_budgets, agent_templates,
                       agent_roles, work_blocks, search_jobs, workers, primes
         CASCADE",
    )
    .execute(pool)
    .await
    .unwrap();

    // Re-seed roles
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

    // Re-seed default templates
    sqlx::raw_sql(
        "INSERT INTO agent_templates (name, description, steps) VALUES
          ('fix-bug', 'Bug fix workflow: investigate, fix, verify',
           '[{\"title\":\"Investigate\",\"description\":\"Find root cause\",\"permission_level\":0},{\"title\":\"Fix\",\"description\":\"Implement fix\",\"permission_level\":1,\"depends_on_step\":0},{\"title\":\"Verify\",\"description\":\"Run tests\",\"permission_level\":1,\"depends_on_step\":1}]'::jsonb)
         ON CONFLICT (name) DO NOTHING",
    )
    .execute(pool)
    .await
    .unwrap();

    // Re-seed role-template associations
    sqlx::raw_sql(
        "INSERT INTO agent_role_templates (role_name, template_name) VALUES
          ('engine', 'fix-bug'),
          ('frontend', 'fix-bug')
         ON CONFLICT DO NOTHING",
    )
    .execute(pool)
    .await
    .unwrap();

    // Re-seed default budgets
    sqlx::raw_sql(
        "INSERT INTO agent_budgets (period, budget_usd) VALUES
          ('daily', 10.00), ('weekly', 50.00), ('monthly', 150.00)",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// Run all migrations against the test database, skipping Supabase-specific commands.
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
        "supabase/migrations/013_agent_roles.sql",
    ];

    for file in &migration_files {
        let path = std::path::Path::new(file);
        if !path.exists() {
            panic!("Migration file not found: {}", file);
        }
        let sql = std::fs::read_to_string(path).unwrap();
        let cleaned = clean_migration_sql(&sql);
        if !cleaned.trim().is_empty() {
            sqlx::raw_sql(&cleaned).execute(pool).await.unwrap_or_else(|e| {
                panic!("Migration {} failed: {}", file, e);
            });
        }
    }
}

/// Remove Supabase-specific SQL (ALTER PUBLICATION, RLS, policies).
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
