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

use super::{AgentScheduleRow, Database};
use anyhow::Result;

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
        sqlx::query("UPDATE agent_schedules SET last_checked_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Create a new agent schedule.
    pub async fn create_agent_schedule(
        &self,
        name: &str,
        description: &str,
        trigger_type: &str,
        cron_expr: Option<&str>,
        event_filter: Option<&str>,
        action_type: &str,
        template_name: Option<&str>,
        role_name: Option<&str>,
        task_title: &str,
        task_description: &str,
        priority: &str,
        max_cost_usd: Option<f64>,
        permission_level: i32,
    ) -> Result<AgentScheduleRow> {
        let row = sqlx::query_as::<_, AgentScheduleRow>(
            "INSERT INTO agent_schedules (name, description, trigger_type, cron_expr, event_filter,
                    action_type, template_name, role_name, task_title, task_description,
                    priority, max_cost_usd, permission_level)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
             RETURNING id, name, description, enabled, trigger_type, cron_expr, event_filter,
                    action_type, template_name, role_name, task_title, task_description,
                    priority, max_cost_usd::FLOAT8 AS max_cost_usd, permission_level,
                    fire_count, last_fired_at, last_checked_at, created_at, updated_at",
        )
        .bind(name)
        .bind(description)
        .bind(trigger_type)
        .bind(cron_expr)
        .bind(event_filter)
        .bind(action_type)
        .bind(template_name)
        .bind(role_name)
        .bind(task_title)
        .bind(task_description)
        .bind(priority)
        .bind(max_cost_usd)
        .bind(permission_level)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Update an existing schedule's fields.
    pub async fn update_agent_schedule(
        &self,
        id: i64,
        updates: &serde_json::Value,
    ) -> Result<Option<AgentScheduleRow>> {
        // Build SET clause from provided fields
        let mut sets = Vec::new();
        let mut idx = 2u32; // $1 is id
        if updates.get("name").is_some() { sets.push(format!("name = ${}", idx)); idx += 1; }
        if updates.get("description").is_some() { sets.push(format!("description = ${}", idx)); idx += 1; }
        if updates.get("enabled").is_some() { sets.push(format!("enabled = ${}", idx)); idx += 1; }
        if updates.get("trigger_type").is_some() { sets.push(format!("trigger_type = ${}", idx)); idx += 1; }
        if updates.get("cron_expr").is_some() { sets.push(format!("cron_expr = ${}", idx)); idx += 1; }
        if updates.get("event_filter").is_some() { sets.push(format!("event_filter = ${}", idx)); idx += 1; }
        if updates.get("task_title").is_some() { sets.push(format!("task_title = ${}", idx)); idx += 1; }
        if updates.get("task_description").is_some() { sets.push(format!("task_description = ${}", idx)); idx += 1; }
        if updates.get("priority").is_some() { sets.push(format!("priority = ${}", idx)); idx += 1; }
        if updates.get("max_cost_usd").is_some() { sets.push(format!("max_cost_usd = ${}", idx)); idx += 1; }
        if updates.get("permission_level").is_some() { sets.push(format!("permission_level = ${}", idx)); idx += 1; }
        let _ = idx;

        if sets.is_empty() {
            return self.get_agent_schedule_by_id(id).await;
        }
        sets.push("updated_at = NOW()".to_string());

        let sql = format!(
            "UPDATE agent_schedules SET {} WHERE id = $1
             RETURNING id, name, description, enabled, trigger_type, cron_expr, event_filter,
                    action_type, template_name, role_name, task_title, task_description,
                    priority, max_cost_usd::FLOAT8 AS max_cost_usd, permission_level,
                    fire_count, last_fired_at, last_checked_at, created_at, updated_at",
            sets.join(", ")
        );

        let mut query = sqlx::query_as::<_, AgentScheduleRow>(&sql).bind(id);
        if let Some(v) = updates.get("name").and_then(|v| v.as_str()) { query = query.bind(v); }
        if let Some(v) = updates.get("description").and_then(|v| v.as_str()) { query = query.bind(v); }
        if let Some(v) = updates.get("enabled").and_then(|v| v.as_bool()) { query = query.bind(v); }
        if let Some(v) = updates.get("trigger_type").and_then(|v| v.as_str()) { query = query.bind(v); }
        if let Some(v) = updates.get("cron_expr") { query = query.bind(v.as_str()); }
        if let Some(v) = updates.get("event_filter") { query = query.bind(v.as_str()); }
        if let Some(v) = updates.get("task_title").and_then(|v| v.as_str()) { query = query.bind(v); }
        if let Some(v) = updates.get("task_description").and_then(|v| v.as_str()) { query = query.bind(v); }
        if let Some(v) = updates.get("priority").and_then(|v| v.as_str()) { query = query.bind(v); }
        if let Some(v) = updates.get("max_cost_usd").and_then(|v| v.as_f64()) { query = query.bind(v); }
        if let Some(v) = updates.get("permission_level").and_then(|v| v.as_i64()) { query = query.bind(v as i32); }

        let row = query.fetch_optional(&self.pool).await?;
        Ok(row)
    }

    /// Delete a schedule by ID.
    pub async fn delete_agent_schedule(&self, id: i64) -> Result<bool> {
        let result = sqlx::query("DELETE FROM agent_schedules WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get a schedule by ID.
    pub async fn get_agent_schedule_by_id(&self, id: i64) -> Result<Option<AgentScheduleRow>> {
        let row = sqlx::query_as::<_, AgentScheduleRow>(
            "SELECT id, name, description, enabled, trigger_type, cron_expr, event_filter,
                    action_type, template_name, role_name, task_title, task_description,
                    priority, max_cost_usd::FLOAT8 AS max_cost_usd, permission_level,
                    fire_count, last_fired_at, last_checked_at, created_at, updated_at
             FROM agent_schedules WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;
        Ok(row)
    }
}
