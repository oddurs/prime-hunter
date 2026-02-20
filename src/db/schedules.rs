//! Agent schedule operations.
//!
//! Schedules automate task creation based on cron expressions (time-based)
//! or event filters (e.g., PrimeFound, SearchCompleted). Each schedule can
//! create a single task or expand a template when triggered.
//!
//! ## Trigger types
//!
//! - `cron`: fires on a cron schedule (e.g., "0 2 * * *" for daily at 2am)
//! - `event`: fires when a matching event occurs (e.g., "PrimeFound")

use anyhow::Result;
use super::{Database, AgentScheduleRow};

impl Database {
    /// Get all schedules, ordered by name.
    pub async fn get_agent_schedules(&self) -> Result<Vec<AgentScheduleRow>> {
        let rows = sqlx::query_as::<_, AgentScheduleRow>(
            "SELECT id, name, description, enabled, trigger_type, cron_expr, event_filter,
                    action_type, template_name, role_name, task_title, task_description,
                    priority, max_cost_usd::FLOAT8 AS max_cost_usd, permission_level,
                    fire_count, last_fired_at, last_checked_at, created_at, updated_at
             FROM agent_schedules ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get only enabled schedules (for the scheduler tick loop).
    pub async fn get_enabled_schedules(&self) -> Result<Vec<AgentScheduleRow>> {
        let rows = sqlx::query_as::<_, AgentScheduleRow>(
            "SELECT id, name, description, enabled, trigger_type, cron_expr, event_filter,
                    action_type, template_name, role_name, task_title, task_description,
                    priority, max_cost_usd::FLOAT8 AS max_cost_usd, permission_level,
                    fire_count, last_fired_at, last_checked_at, created_at, updated_at
             FROM agent_schedules WHERE enabled = true ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Toggle a schedule's enabled state.
    pub async fn update_schedule_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        sqlx::query("UPDATE agent_schedules SET enabled = $1, updated_at = NOW() WHERE id = $2")
            .bind(enabled)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Record that a schedule has fired: increment fire_count and set last_fired_at.
    pub async fn fire_schedule(&self, id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE agent_schedules SET fire_count = fire_count + 1, last_fired_at = NOW(), updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update last_checked_at for a schedule (used by the cron evaluator).
    pub async fn mark_schedule_checked(&self, id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE agent_schedules SET last_checked_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
