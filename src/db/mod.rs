//! # Database — PostgreSQL Storage Layer
//!
//! Provides async database operations for prime records, search metadata, and
//! fleet coordination via `sqlx::PgPool` connecting to Supabase PostgreSQL.
//!
//! ## Schema
//!
//! - `primes`: expression, form, digits, found_at, proof_method, search_params
//! - `search_jobs`: job configuration, status, progress tracking
//! - `work_blocks`: distributable work units for cluster coordination
//! - `workers`: heartbeat-based fleet registry
//! - `agent_tasks`: AI agent task queue
//!
//! ## Module Structure
//!
//! Operations are split into submodules by domain:
//!
//! - [`primes`] — Prime record CRUD (insert, query, filter, verify)
//! - [`workers`] — Worker heartbeat, registration, pruning
//! - [`jobs`] — Search job lifecycle and work block coordination
//! - [`agents`] — Agent tasks, events, budgets, templates
//! - [`memory`] — Agent memory key-value store
//! - [`roles`] — Agent role configuration
//! - [`schedules`] — Agent schedule automation
//! - [`projects`] — Multi-phase project management
//! - [`calibrations`] — Cost model calibration coefficients
//! - [`records`] — World record tracking
//!
//! ## Sync Wrapper
//!
//! Engine modules run inside Rayon thread pools (no Tokio runtime). The
//! `insert_prime_sync` method bridges async sqlx operations into sync contexts
//! via `tokio::runtime::Handle::block_on`. This is safe because Rayon threads
//! are not Tokio tasks — they won't deadlock the executor.

mod agents;
mod calibrations;
mod jobs;
mod memory;
mod primes;
mod projects;
mod records;
mod roles;
mod schedules;
pub mod volunteers;
mod workers;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

// ── Prime types ─────────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct PrimeRecord {
    pub id: i64,
    pub form: String,
    pub expression: String,
    pub digits: i64,
    pub found_at: chrono::DateTime<chrono::Utc>,
    pub proof_method: String,
}

#[derive(Serialize)]
pub struct FormCount {
    pub form: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct Stats {
    pub total: i64,
    pub by_form: Vec<FormCount>,
    pub largest_digits: i64,
    pub largest_expression: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
pub struct PrimeFilter {
    pub form: Option<String>,
    pub search: Option<String>,
    pub min_digits: Option<i64>,
    pub max_digits: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_dir: Option<String>,
}

impl PrimeFilter {
    /// Whitelist sort column to prevent SQL injection.
    /// Unknown values default to "id".
    pub(crate) fn safe_sort_column(&self) -> &str {
        match self.sort_by.as_deref() {
            Some("digits") => "digits",
            Some("form") => "form",
            Some("expression") => "expression",
            Some("found_at") => "found_at",
            _ => "id",
        }
    }

    /// Whitelist sort direction to prevent SQL injection.
    /// Only "asc"/"ASC" are accepted; everything else defaults to "DESC".
    pub(crate) fn safe_sort_dir(&self) -> &str {
        match self.sort_dir.as_deref() {
            Some("asc") | Some("ASC") => "ASC",
            _ => "DESC",
        }
    }
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub struct PrimeDetail {
    pub id: i64,
    pub form: String,
    pub expression: String,
    pub digits: i64,
    pub found_at: chrono::DateTime<chrono::Utc>,
    pub search_params: String,
    pub proof_method: String,
}

#[derive(Serialize)]
pub struct TimelineBucket {
    pub bucket: String,
    pub form: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct DigitBucket {
    pub bucket_start: i64,
    pub form: String,
    pub count: i64,
}

// ── Worker types ────────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct WorkerRow {
    pub worker_id: String,
    pub hostname: String,
    pub cores: i32,
    pub search_type: String,
    pub search_params: String,
    pub tested: i64,
    pub found: i64,
    pub current: String,
    pub checkpoint: Option<String>,
    pub metrics: Option<Value>,
    pub registered_at: chrono::DateTime<chrono::Utc>,
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
}

// ── Search job types ────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct SearchJobRow {
    pub id: i64,
    pub search_type: String,
    pub params: Value,
    pub status: String,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub stopped_at: Option<chrono::DateTime<chrono::Utc>>,
    pub range_start: i64,
    pub range_end: i64,
    pub block_size: i64,
    pub total_tested: i64,
    pub total_found: i64,
}

#[derive(sqlx::FromRow)]
pub struct WorkBlock {
    pub block_id: i64,
    pub block_start: i64,
    pub block_end: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct JobBlockSummary {
    pub available: i64,
    pub claimed: i64,
    pub completed: i64,
    pub failed: i64,
    pub total_tested: i64,
    pub total_found: i64,
}

// ── Agent types ─────────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct AgentTaskRow {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub agent_model: Option<String>,
    pub assigned_agent: Option<String>,
    pub source: String,
    pub result: Option<Value>,
    pub tokens_used: i64,
    pub cost_usd: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub parent_task_id: Option<i64>,
    pub max_cost_usd: Option<f64>,
    pub permission_level: i32,
    pub template_name: Option<String>,
    pub on_child_failure: String,
    pub role_name: Option<String>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentTemplateRow {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub steps: Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub role_name: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AgentTaskDepRow {
    pub task_id: i64,
    pub depends_on: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AgentEventRow {
    pub id: i64,
    pub task_id: Option<i64>,
    pub event_type: String,
    pub agent: Option<String>,
    pub summary: String,
    pub detail: Option<Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub tool_name: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub duration_ms: Option<i64>,
}

/// A raw log line from an agent subprocess (stdout or stderr).
#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentLogRow {
    pub id: i64,
    pub task_id: i64,
    pub stream: String,
    pub line_num: i32,
    pub msg_type: Option<String>,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Daily cost aggregation grouped by model.
#[derive(Serialize, sqlx::FromRow)]
pub struct DailyCostRow {
    pub date: chrono::NaiveDate,
    pub model: String,
    pub total_cost: f64,
    pub total_tokens: i64,
    pub task_count: i64,
}

/// Template-level cost aggregation.
#[derive(Serialize, sqlx::FromRow)]
pub struct TemplateCostRow {
    pub template_name: String,
    pub task_count: i64,
    pub total_cost: f64,
    pub avg_cost: f64,
    pub total_tokens: i64,
    pub avg_tokens: f64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AgentBudgetRow {
    pub id: i64,
    pub period: String,
    pub budget_usd: f64,
    pub spent_usd: f64,
    pub tokens_used: i64,
    pub period_start: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AgentMemoryRow {
    pub id: i64,
    pub key: String,
    pub value: String,
    pub category: String,
    pub created_by_task: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// A named agent role that bundles domain context, permissions, default model,
/// and associated templates. Roles like "engine", "frontend", "ops", and "research"
/// provide domain-specific defaults when creating agent tasks.
#[derive(Serialize, sqlx::FromRow)]
pub struct AgentRoleRow {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub domains: Value,
    pub default_permission_level: i32,
    pub default_model: String,
    pub system_prompt: Option<String>,
    pub default_max_cost_usd: Option<f64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// ── Schedule types ──────────────────────────────────────────────

/// An agent schedule that triggers task creation on a cron expression or event.
#[derive(Serialize, sqlx::FromRow)]
pub struct AgentScheduleRow {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub trigger_type: String,
    pub cron_expr: Option<String>,
    pub event_filter: Option<String>,
    pub action_type: String,
    pub template_name: Option<String>,
    pub role_name: Option<String>,
    pub task_title: String,
    pub task_description: String,
    pub priority: String,
    pub max_cost_usd: Option<f64>,
    pub permission_level: i32,
    pub fire_count: i32,
    pub last_fired_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_checked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// ── Fleet summary ─────────────────────────────────────────────

/// Aggregated fleet capabilities, computed from active worker heartbeats.
/// Used by the orchestration engine to enforce infrastructure requirements
/// before activating project phases.
#[derive(Debug, Clone, Serialize)]
pub struct FleetSummary {
    /// Number of workers with recent heartbeats.
    pub worker_count: u32,
    /// Sum of `cores` across all active workers.
    pub total_cores: u32,
    /// Maximum RAM (GB) reported by any single worker via hardware metrics.
    pub max_ram_gb: u32,
    /// Search types currently running across the fleet.
    pub active_search_types: Vec<String>,
}

// ── Calibration types ───────────────────────────────────────────

/// Cost calibration coefficients for a prime form.
///
/// The power-law model: `secs_per_candidate = coeff_a * (digits / 1000) ^ coeff_b`
#[derive(Serialize, sqlx::FromRow)]
pub struct CostCalibrationRow {
    pub form: String,
    pub coeff_a: f64,
    pub coeff_b: f64,
    pub sample_count: i64,
    pub avg_error_pct: Option<f64>,
    pub fitted_at: chrono::DateTime<chrono::Utc>,
}

// ── Database struct and connection ──────────────────────────────

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Connect to PostgreSQL using the provided database URL.
    ///
    /// Manually parses the URL to preserve the full username — sqlx's built-in
    /// parser strips the ".project-ref" suffix that Supabase pooler requires.
    pub async fn connect(database_url: &str) -> Result<Self> {
        let url = url::Url::parse(database_url)?;
        let username = urlencoding::decode(url.username())?.into_owned();
        let password = url
            .password()
            .map(|p| urlencoding::decode(p).map(|s| s.into_owned()))
            .transpose()?;
        let mut opts = PgConnectOptions::new()
            .host(url.host_str().unwrap_or("localhost"))
            .port(url.port().unwrap_or(5432))
            .database(url.path().trim_start_matches('/'))
            .username(&username)
            .statement_cache_capacity(0);
        if let Some(ref pw) = password {
            opts = opts.password(pw);
        }
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(opts)
            .await?;
        Ok(Database { pool })
    }

    /// Get a reference to the underlying connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Health check: execute `SELECT 1` to verify database connectivity.
    ///
    /// Used by the `/readyz` Kubernetes readiness probe. Returns `Ok(())` if
    /// the database responds, or an error if the connection is broken.
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&self.pool)
            .await?;
        Ok(())
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_sort_column_whitelists_known_columns() {
        let cases = vec![
            ("digits", "digits"),
            ("form", "form"),
            ("expression", "expression"),
            ("found_at", "found_at"),
        ];
        for (input, expected) in cases {
            let filter = PrimeFilter {
                sort_by: Some(input.into()),
                ..Default::default()
            };
            assert_eq!(filter.safe_sort_column(), expected);
        }
    }

    #[test]
    fn safe_sort_column_defaults_to_id_for_unknown() {
        let unknown_inputs = vec![
            "id",
            "ID",
            "unknown",
            "'; DROP TABLE primes; --",
            "",
            "proof_method",
            "search_params",
        ];
        for input in unknown_inputs {
            let filter = PrimeFilter {
                sort_by: Some(input.into()),
                ..Default::default()
            };
            assert_eq!(
                filter.safe_sort_column(),
                "id",
                "Unknown sort_by '{}' should default to 'id'",
                input
            );
        }
    }

    #[test]
    fn safe_sort_column_defaults_to_id_when_none() {
        let filter = PrimeFilter::default();
        assert_eq!(filter.safe_sort_column(), "id");
    }

    #[test]
    fn safe_sort_dir_accepts_asc() {
        for input in ["asc", "ASC"] {
            let filter = PrimeFilter {
                sort_dir: Some(input.into()),
                ..Default::default()
            };
            assert_eq!(filter.safe_sort_dir(), "ASC");
        }
    }

    #[test]
    fn safe_sort_dir_defaults_to_desc() {
        let unknown_inputs = vec!["desc", "DESC", "Asc", "random", "'; DROP TABLE--", ""];
        for input in unknown_inputs {
            let filter = PrimeFilter {
                sort_dir: Some(input.into()),
                ..Default::default()
            };
            assert_eq!(
                filter.safe_sort_dir(),
                "DESC",
                "Unknown sort_dir '{}' should default to 'DESC'",
                input
            );
        }
    }

    #[test]
    fn safe_sort_dir_defaults_to_desc_when_none() {
        let filter = PrimeFilter::default();
        assert_eq!(filter.safe_sort_dir(), "DESC");
    }

    #[test]
    fn prime_filter_default_is_empty() {
        let filter = PrimeFilter::default();
        assert!(filter.form.is_none());
        assert!(filter.search.is_none());
        assert!(filter.min_digits.is_none());
        assert!(filter.max_digits.is_none());
        assert!(filter.sort_by.is_none());
        assert!(filter.sort_dir.is_none());
    }
}
