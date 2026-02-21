//! AI engine database operations.
//!
//! Manages the `ai_engine_state` singleton and `ai_engine_decisions` audit trail.
//! Also provides the `cost_observations` query for the LEARN phase OLS fitting,
//! and helper queries for momentum scoring and agent result integration.

use super::Database;
use anyhow::Result;
use serde::Serialize;

/// Row from the `ai_engine_state` singleton table.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AiEngineStateRow {
    pub id: i64,
    pub scoring_weights: serde_json::Value,
    pub cost_model_version: i32,
    pub last_tick_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_learn_at: Option<chrono::DateTime<chrono::Utc>>,
    pub tick_count: i64,
    pub config: serde_json::Value,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Row from the `ai_engine_decisions` audit trail.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AiEngineDecisionRow {
    pub id: i64,
    pub tick_id: i64,
    pub decision_type: String,
    pub form: Option<String>,
    pub action: String,
    pub reasoning: String,
    pub confidence: Option<f64>,
    pub snapshot_hash: Option<String>,
    pub params: Option<serde_json::Value>,
    pub outcome: Option<serde_json::Value>,
    pub outcome_measured_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Database {
    /// Get the AI engine state singleton. Returns None if no row exists.
    pub async fn get_ai_engine_state(&self) -> Result<Option<AiEngineStateRow>> {
        let row = sqlx::query_as::<_, AiEngineStateRow>(
            "SELECT id, scoring_weights, cost_model_version, last_tick_at,
                    last_learn_at, tick_count, config, updated_at
             FROM ai_engine_state
             LIMIT 1",
        )
        .fetch_optional(&self.read_pool)
        .await?;
        Ok(row)
    }

    /// Upsert the AI engine state (scoring weights, tick count, cost model version).
    pub async fn upsert_ai_engine_state(
        &self,
        scoring_weights: &serde_json::Value,
        cost_model_version: i32,
        tick_count: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO ai_engine_state (id, scoring_weights, cost_model_version, tick_count, last_tick_at, updated_at)
             VALUES (1, $1, $2, $3, NOW(), NOW())
             ON CONFLICT (id) DO UPDATE SET
               scoring_weights = EXCLUDED.scoring_weights,
               cost_model_version = EXCLUDED.cost_model_version,
               tick_count = EXCLUDED.tick_count,
               last_tick_at = NOW(),
               updated_at = NOW()",
        )
        .bind(scoring_weights)
        .bind(cost_model_version)
        .bind(tick_count)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert a decision into the AI engine audit trail.
    pub async fn insert_ai_engine_decision(
        &self,
        tick_id: i64,
        decision_type: &str,
        form: Option<&str>,
        action: &str,
        reasoning: &str,
        confidence: f64,
        params: Option<&serde_json::Value>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO ai_engine_decisions
                (tick_id, decision_type, form, action, reasoning, confidence, params)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id",
        )
        .bind(tick_id)
        .bind(decision_type)
        .bind(form)
        .bind(action)
        .bind(reasoning)
        .bind(confidence)
        .bind(params)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    /// Get recent AI engine decisions, newest first.
    pub async fn get_ai_engine_decisions(&self, limit: i64) -> Result<Vec<AiEngineDecisionRow>> {
        let rows = sqlx::query_as::<_, AiEngineDecisionRow>(
            "SELECT id, tick_id, decision_type, form, action, reasoning,
                    confidence, snapshot_hash, params, outcome,
                    outcome_measured_at, created_at
             FROM ai_engine_decisions
             ORDER BY created_at DESC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    /// Get cost observations for OLS fitting in the LEARN phase.
    /// Returns (digits, secs_per_candidate) pairs from completed work blocks.
    pub async fn get_cost_observations(
        &self,
        form: &str,
        limit: i64,
    ) -> Result<Vec<crate::ai_engine::CostObservation>> {
        let rows = sqlx::query_as::<_, crate::ai_engine::CostObservation>(
            "SELECT digits::float8 as digits, secs::float8 as secs
             FROM cost_observations
             WHERE form = $1
               AND digits > 0
               AND secs > 0
               AND secs < 86400
             ORDER BY completed_at DESC
             LIMIT $2",
        )
        .bind(form)
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    /// Get recent prime discoveries for momentum scoring.
    /// Returns primes found in the last N days, grouped by form.
    pub async fn get_recent_primes_for_momentum(
        &self,
        days: i32,
    ) -> Result<Vec<crate::ai_engine::RecentDiscovery>> {
        let rows = sqlx::query_as::<_, RecentPrimeRow>(
            "SELECT form, digits, found_at
             FROM primes
             WHERE found_at > NOW() - ($1 || ' days')::interval
             ORDER BY found_at DESC
             LIMIT 100",
        )
        .bind(days)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| crate::ai_engine::RecentDiscovery {
                form: r.form,
                digits: r.digits,
                found_at: r.found_at,
            })
            .collect())
    }

    /// Get recent completed agent task results for feedback integration.
    pub async fn get_recent_agent_results(
        &self,
        limit: i64,
    ) -> Result<Vec<super::AgentTaskRow>> {
        let rows = sqlx::query_as::<_, super::AgentTaskRow>(
            "SELECT id, title, description, status, priority, agent_model,
                    assigned_agent, source, result, tokens_used, cost_usd,
                    created_at, started_at, completed_at, parent_task_id,
                    max_cost_usd, permission_level, template_name,
                    on_child_failure, role_name
             FROM agent_tasks
             WHERE status IN ('completed', 'failed')
               AND completed_at > NOW() - INTERVAL '24 hours'
             ORDER BY completed_at DESC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }
}

/// Helper row type for the recent primes query.
#[derive(sqlx::FromRow)]
struct RecentPrimeRow {
    form: String,
    digits: i64,
    found_at: chrono::DateTime<chrono::Utc>,
}
