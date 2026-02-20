//! Database row types for projects, phases, records, and events.
//!
//! These types map directly to PostgreSQL table rows via `sqlx::FromRow`.
//! They are used by both the `db::projects` module (for queries) and the
//! orchestration engine (for state management).

use serde::Serialize;

/// Database row for a project.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProjectRow {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub objective: String,
    pub form: String,
    pub status: String,
    pub toml_source: Option<String>,
    pub target: serde_json::Value,
    pub competitive: serde_json::Value,
    pub strategy: serde_json::Value,
    pub infrastructure: serde_json::Value,
    pub budget: serde_json::Value,
    pub total_tested: i64,
    pub total_found: i64,
    pub best_prime_id: Option<i64>,
    pub best_digits: i64,
    pub total_core_hours: f64,
    pub total_cost_usd: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Database row for a project phase.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProjectPhaseRow {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub description: String,
    pub phase_order: i32,
    pub status: String,
    pub search_params: serde_json::Value,
    pub block_size: i64,
    pub depends_on: Vec<String>,
    pub activation_condition: Option<String>,
    pub completion_condition: String,
    pub search_job_id: Option<i64>,
    pub total_tested: i64,
    pub total_found: i64,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Database row for a world record.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct RecordRow {
    pub id: i64,
    pub form: String,
    pub category: String,
    pub expression: String,
    pub digits: i64,
    pub holder: Option<String>,
    pub discovered_at: Option<chrono::NaiveDate>,
    pub source: Option<String>,
    pub source_url: Option<String>,
    pub our_best_id: Option<i64>,
    pub our_best_digits: i64,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Database row for a project event.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProjectEventRow {
    pub id: i64,
    pub project_id: i64,
    pub event_type: String,
    pub summary: String,
    pub detail: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
