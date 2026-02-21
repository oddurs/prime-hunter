//! Strategy engine database operations.
//!
//! Manages the `strategy_decisions` audit log, `strategy_config` singleton,
//! and the `form_yield_rates` view. The strategy engine uses these to
//! autonomously score search forms and create projects/jobs.

use super::Database;
use anyhow::Result;
use serde::Serialize;

/// Row from the `strategy_decisions` audit log.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct StrategyDecisionRow {
    pub id: i64,
    pub decision_type: String,
    pub form: Option<String>,
    pub summary: String,
    pub reasoning: String,
    pub params: Option<serde_json::Value>,
    pub estimated_cost_usd: Option<f64>,
    pub action_taken: String,
    pub override_reason: Option<String>,
    pub project_id: Option<i64>,
    pub search_job_id: Option<i64>,
    pub scores: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Row from the `strategy_config` singleton table.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct StrategyConfigRow {
    pub id: i64,
    pub enabled: bool,
    pub max_concurrent_projects: i32,
    pub max_monthly_budget_usd: f64,
    pub max_per_project_budget_usd: f64,
    pub preferred_forms: Vec<String>,
    pub excluded_forms: Vec<String>,
    pub min_idle_workers_to_create: i32,
    pub record_proximity_threshold: f64,
    pub tick_interval_secs: i32,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Row from the `form_yield_rates` view.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct FormYieldRateRow {
    pub form: String,
    pub job_count: i64,
    pub total_tested: i64,
    pub total_found: i64,
    pub yield_rate: f64,
    pub max_range_searched: i64,
}

impl Database {
    /// Insert a strategy decision into the audit log.
    pub async fn insert_strategy_decision(
        &self,
        decision_type: &str,
        form: Option<&str>,
        summary: &str,
        reasoning: &str,
        params: Option<&serde_json::Value>,
        estimated_cost_usd: Option<f64>,
        action_taken: &str,
        project_id: Option<i64>,
        search_job_id: Option<i64>,
        scores: Option<&serde_json::Value>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO strategy_decisions
                (decision_type, form, summary, reasoning, params,
                 estimated_cost_usd, action_taken, project_id, search_job_id, scores)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING id",
        )
        .bind(decision_type)
        .bind(form)
        .bind(summary)
        .bind(reasoning)
        .bind(params)
        .bind(estimated_cost_usd)
        .bind(action_taken)
        .bind(project_id)
        .bind(search_job_id)
        .bind(scores)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    /// Fetch recent strategy decisions, newest first.
    pub async fn get_strategy_decisions(&self, limit: i64) -> Result<Vec<StrategyDecisionRow>> {
        let rows = sqlx::query_as::<_, StrategyDecisionRow>(
            "SELECT id, decision_type, form, summary, reasoning, params,
                    estimated_cost_usd, action_taken, override_reason,
                    project_id, search_job_id, scores, created_at
             FROM strategy_decisions
             ORDER BY created_at DESC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    /// Get the strategy engine configuration (singleton row).
    pub async fn get_strategy_config(&self) -> Result<StrategyConfigRow> {
        let row = sqlx::query_as::<_, StrategyConfigRow>(
            "SELECT id, enabled, max_concurrent_projects, max_monthly_budget_usd,
                    max_per_project_budget_usd, preferred_forms, excluded_forms,
                    min_idle_workers_to_create, record_proximity_threshold,
                    tick_interval_secs, updated_at
             FROM strategy_config
             LIMIT 1",
        )
        .fetch_one(&self.read_pool)
        .await?;
        Ok(row)
    }

    /// Update strategy engine configuration.
    pub async fn update_strategy_config(
        &self,
        enabled: Option<bool>,
        max_concurrent_projects: Option<i32>,
        max_monthly_budget_usd: Option<f64>,
        max_per_project_budget_usd: Option<f64>,
        preferred_forms: Option<&[String]>,
        excluded_forms: Option<&[String]>,
        min_idle_workers_to_create: Option<i32>,
        record_proximity_threshold: Option<f64>,
        tick_interval_secs: Option<i32>,
    ) -> Result<StrategyConfigRow> {
        let row = sqlx::query_as::<_, StrategyConfigRow>(
            "UPDATE strategy_config SET
                enabled = COALESCE($1, enabled),
                max_concurrent_projects = COALESCE($2, max_concurrent_projects),
                max_monthly_budget_usd = COALESCE($3, max_monthly_budget_usd),
                max_per_project_budget_usd = COALESCE($4, max_per_project_budget_usd),
                preferred_forms = COALESCE($5, preferred_forms),
                excluded_forms = COALESCE($6, excluded_forms),
                min_idle_workers_to_create = COALESCE($7, min_idle_workers_to_create),
                record_proximity_threshold = COALESCE($8, record_proximity_threshold),
                tick_interval_secs = COALESCE($9, tick_interval_secs),
                updated_at = NOW()
             WHERE id = (SELECT id FROM strategy_config LIMIT 1)
             RETURNING id, enabled, max_concurrent_projects, max_monthly_budget_usd,
                       max_per_project_budget_usd, preferred_forms, excluded_forms,
                       min_idle_workers_to_create, record_proximity_threshold,
                       tick_interval_secs, updated_at",
        )
        .bind(enabled)
        .bind(max_concurrent_projects)
        .bind(max_monthly_budget_usd)
        .bind(max_per_project_budget_usd)
        .bind(preferred_forms)
        .bind(excluded_forms)
        .bind(min_idle_workers_to_create)
        .bind(record_proximity_threshold)
        .bind(tick_interval_secs)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get per-form yield rate statistics from the materialized view.
    pub async fn get_form_yield_rates(&self) -> Result<Vec<FormYieldRateRow>> {
        let rows = sqlx::query_as::<_, FormYieldRateRow>(
            "SELECT form, job_count, total_tested, total_found, yield_rate, max_range_searched
             FROM form_yield_rates
             ORDER BY yield_rate DESC",
        )
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    /// Get the maximum searched range for a specific form.
    pub async fn get_max_searched_range(&self, form: &str) -> Result<i64> {
        let max_range: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(range_end) FROM search_jobs
             WHERE search_type = $1 AND status IN ('completed', 'running')",
        )
        .bind(form)
        .fetch_one(&self.read_pool)
        .await?;
        Ok(max_range.unwrap_or(0))
    }

    /// Admin override of a strategy decision.
    pub async fn override_strategy_decision(
        &self,
        id: i64,
        action_taken: &str,
        reason: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE strategy_decisions
             SET action_taken = $2, override_reason = $3
             WHERE id = $1",
        )
        .bind(id)
        .bind(action_taken)
        .bind(reason)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get total strategy engine spend this month.
    pub async fn get_monthly_strategy_spend(&self) -> Result<f64> {
        let spend: Option<f64> = sqlx::query_scalar(
            "SELECT COALESCE(SUM(p.total_cost_usd), 0.0)
             FROM strategy_decisions sd
             JOIN projects p ON p.id = sd.project_id
             WHERE sd.decision_type = 'create_project'
               AND sd.action_taken = 'executed'
               AND sd.created_at >= date_trunc('month', NOW())",
        )
        .fetch_one(&self.read_pool)
        .await?;
        Ok(spend.unwrap_or(0.0))
    }
}
