//! Agent task, event, budget, and template operations.
//!
//! The agent system manages AI-driven autonomous tasks. Tasks follow a lifecycle:
//! pending → in_progress → completed/failed/cancelled. Parent tasks decompose
//! into child tasks via templates, with dependency tracking via `agent_task_deps`.
//!
//! Budget enforcement uses three time periods (daily/weekly/monthly) to cap
//! spending. The execution engine claims tasks atomically with `FOR UPDATE SKIP LOCKED`.

use anyhow::Result;
use serde_json::Value;
use super::{Database, AgentBudgetRow, AgentEventRow, AgentLogRow, AgentTaskRow, AgentTemplateRow, DailyCostRow, TemplateCostRow};

impl Database {
    /// Get agent tasks with optional status filter, most recent first.
    pub async fn get_agent_tasks(
        &self,
        status_filter: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AgentTaskRow>> {
        let rows = if let Some(status) = status_filter {
            sqlx::query_as::<_, AgentTaskRow>(
                "SELECT id, title, description, status, priority, agent_model, assigned_agent,
                        source, result, tokens_used, cost_usd::FLOAT8 AS cost_usd,
                        created_at, started_at, completed_at, parent_task_id,
                        max_cost_usd::FLOAT8 AS max_cost_usd, permission_level,
                        template_name, on_child_failure, role_name
                 FROM agent_tasks WHERE status = $1
                 ORDER BY created_at DESC LIMIT $2",
            )
            .bind(status)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, AgentTaskRow>(
                "SELECT id, title, description, status, priority, agent_model, assigned_agent,
                        source, result, tokens_used, cost_usd::FLOAT8 AS cost_usd,
                        created_at, started_at, completed_at, parent_task_id,
                        max_cost_usd::FLOAT8 AS max_cost_usd, permission_level,
                        template_name, on_child_failure, role_name
                 FROM agent_tasks
                 ORDER BY created_at DESC LIMIT $1",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    /// Get a single agent task by ID.
    pub async fn get_agent_task(&self, id: i64) -> Result<Option<AgentTaskRow>> {
        let row = sqlx::query_as::<_, AgentTaskRow>(
            "SELECT id, title, description, status, priority, agent_model, assigned_agent,
                    source, result, tokens_used, cost_usd::FLOAT8 AS cost_usd,
                    created_at, started_at, completed_at, parent_task_id,
                    max_cost_usd::FLOAT8 AS max_cost_usd, permission_level,
                    template_name, on_child_failure, role_name
             FROM agent_tasks WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Create a new agent task (manual or automated).
    pub async fn create_agent_task(
        &self,
        title: &str,
        description: &str,
        priority: &str,
        agent_model: Option<&str>,
        source: &str,
        max_cost_usd: Option<f64>,
        permission_level: i32,
        role_name: Option<&str>,
    ) -> Result<AgentTaskRow> {
        let row = sqlx::query_as::<_, AgentTaskRow>(
            "INSERT INTO agent_tasks (title, description, priority, agent_model, source, max_cost_usd, permission_level, role_name)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id, title, description, status, priority, agent_model, assigned_agent,
                       source, result, tokens_used, cost_usd::FLOAT8 AS cost_usd,
                       created_at, started_at, completed_at, parent_task_id,
                       max_cost_usd::FLOAT8 AS max_cost_usd, permission_level,
                       template_name, on_child_failure, role_name",
        )
        .bind(title)
        .bind(description)
        .bind(priority)
        .bind(agent_model)
        .bind(source)
        .bind(max_cost_usd)
        .bind(permission_level)
        .bind(role_name)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Update task status with appropriate timestamp handling.
    ///
    /// - `in_progress`: sets `started_at` if not already set
    /// - `completed`/`failed`: sets `completed_at`
    /// - Other statuses: only update status field
    pub async fn update_agent_task_status(&self, id: i64, status: &str) -> Result<()> {
        let now = chrono::Utc::now();
        match status {
            "in_progress" => {
                sqlx::query(
                    "UPDATE agent_tasks SET status = $1, started_at = COALESCE(started_at, $2) WHERE id = $3",
                )
                .bind(status)
                .bind(now)
                .bind(id)
                .execute(&self.pool)
                .await?;
            }
            "completed" | "failed" => {
                sqlx::query(
                    "UPDATE agent_tasks SET status = $1, completed_at = $2 WHERE id = $3",
                )
                .bind(status)
                .bind(now)
                .bind(id)
                .execute(&self.pool)
                .await?;
            }
            _ => {
                sqlx::query("UPDATE agent_tasks SET status = $1 WHERE id = $2")
                    .bind(status)
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }

    /// Cancel a pending or in-progress task.
    pub async fn cancel_agent_task(&self, id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE agent_tasks SET status = 'cancelled', completed_at = NOW()
             WHERE id = $1 AND status IN ('pending', 'in_progress')",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get agent events with optional task filter, most recent first.
    pub async fn get_agent_events(
        &self,
        task_id: Option<i64>,
        limit: i64,
    ) -> Result<Vec<AgentEventRow>> {
        let rows = if let Some(tid) = task_id {
            sqlx::query_as::<_, AgentEventRow>(
                "SELECT id, task_id, event_type, agent, summary, detail, created_at,
                        tool_name, input_tokens, output_tokens, duration_ms
                 FROM agent_events WHERE task_id = $1
                 ORDER BY created_at DESC LIMIT $2",
            )
            .bind(tid)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, AgentEventRow>(
                "SELECT id, task_id, event_type, agent, summary, detail, created_at,
                        tool_name, input_tokens, output_tokens, duration_ms
                 FROM agent_events
                 ORDER BY created_at DESC LIMIT $1",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    /// Insert an agent event (created, started, completed, failed, tool_call, etc.).
    pub async fn insert_agent_event(
        &self,
        task_id: Option<i64>,
        event_type: &str,
        agent: Option<&str>,
        summary: &str,
        detail: Option<&Value>,
    ) -> Result<()> {
        self.insert_agent_event_ex(task_id, event_type, agent, summary, detail, None, None, None, None).await
    }

    /// Extended event insert with tool_name, token counts, and duration.
    pub async fn insert_agent_event_ex(
        &self,
        task_id: Option<i64>,
        event_type: &str,
        agent: Option<&str>,
        summary: &str,
        detail: Option<&Value>,
        tool_name: Option<&str>,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
        duration_ms: Option<i64>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO agent_events (task_id, event_type, agent, summary, detail,
                                       tool_name, input_tokens, output_tokens, duration_ms)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(task_id)
        .bind(event_type)
        .bind(agent)
        .bind(summary)
        .bind(detail)
        .bind(tool_name)
        .bind(input_tokens)
        .bind(output_tokens)
        .bind(duration_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // --- Agent logs ---

    /// Batch insert log lines from an agent subprocess.
    pub async fn insert_agent_logs_batch(
        &self,
        task_id: i64,
        entries: &[(String, i32, Option<String>, String)],
    ) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let task_ids: Vec<i64> = entries.iter().map(|_| task_id).collect();
        let streams: Vec<&str> = entries.iter().map(|e| e.0.as_str()).collect();
        let line_nums: Vec<i32> = entries.iter().map(|e| e.1).collect();
        let msg_types: Vec<Option<&str>> = entries.iter().map(|e| e.2.as_deref()).collect();
        let contents: Vec<&str> = entries.iter().map(|e| e.3.as_str()).collect();

        sqlx::query(
            "INSERT INTO agent_logs (task_id, stream, line_num, msg_type, content)
             SELECT * FROM UNNEST($1::bigint[], $2::text[], $3::int[], $4::text[], $5::text[])",
        )
        .bind(&task_ids)
        .bind(&streams)
        .bind(&line_nums)
        .bind(&msg_types)
        .bind(&contents)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch log lines for a task, optionally filtered by stream.
    pub async fn get_agent_logs(
        &self,
        task_id: i64,
        stream: Option<&str>,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<AgentLogRow>> {
        let rows = if let Some(s) = stream {
            sqlx::query_as::<_, AgentLogRow>(
                "SELECT id, task_id, stream, line_num, msg_type, content, created_at
                 FROM agent_logs WHERE task_id = $1 AND stream = $2
                 ORDER BY line_num ASC LIMIT $3 OFFSET $4",
            )
            .bind(task_id)
            .bind(s)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, AgentLogRow>(
                "SELECT id, task_id, stream, line_num, msg_type, content, created_at
                 FROM agent_logs WHERE task_id = $1
                 ORDER BY line_num ASC LIMIT $2 OFFSET $3",
            )
            .bind(task_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    /// Count total log lines for a task.
    pub async fn get_agent_log_count(&self, task_id: i64) -> Result<i64> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM agent_logs WHERE task_id = $1",
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0)
    }

    // --- Agent analytics ---

    /// Get all events for a task in chronological order (timeline view).
    pub async fn get_agent_task_timeline(&self, task_id: i64) -> Result<Vec<AgentEventRow>> {
        let rows = sqlx::query_as::<_, AgentEventRow>(
            "SELECT id, task_id, event_type, agent, summary, detail, created_at,
                    tool_name, input_tokens, output_tokens, duration_ms
             FROM agent_events WHERE task_id = $1
             ORDER BY created_at ASC",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Daily cost breakdown grouped by model for the last N days.
    pub async fn get_agent_daily_costs(&self, days: i32) -> Result<Vec<DailyCostRow>> {
        let rows = sqlx::query_as::<_, DailyCostRow>(
            "SELECT DATE(completed_at) AS date,
                    COALESCE(agent_model, 'unknown') AS model,
                    COALESCE(SUM(cost_usd::FLOAT8), 0) AS total_cost,
                    COALESCE(SUM(tokens_used), 0) AS total_tokens,
                    COUNT(*) AS task_count
             FROM agent_tasks
             WHERE completed_at > NOW() - ($1 || ' days')::interval
             GROUP BY 1, 2
             ORDER BY 1",
        )
        .bind(days.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Cost aggregation by template.
    pub async fn get_agent_template_costs(&self) -> Result<Vec<TemplateCostRow>> {
        let rows = sqlx::query_as::<_, TemplateCostRow>(
            "SELECT template_name,
                    COUNT(*) AS task_count,
                    COALESCE(SUM(cost_usd::FLOAT8), 0) AS total_cost,
                    COALESCE(AVG(cost_usd::FLOAT8), 0) AS avg_cost,
                    COALESCE(SUM(tokens_used), 0) AS total_tokens,
                    COALESCE(AVG(tokens_used::FLOAT8), 0) AS avg_tokens
             FROM agent_tasks
             WHERE template_name IS NOT NULL
             GROUP BY template_name
             ORDER BY total_cost DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Find tasks with anomalously high token usage (> threshold * avg for their template).
    pub async fn get_agent_token_anomalies(&self, threshold: f64) -> Result<Vec<AgentTaskRow>> {
        let rows = sqlx::query_as::<_, AgentTaskRow>(
            "SELECT t.id, t.title, t.description, t.status, t.priority, t.agent_model,
                    t.assigned_agent, t.source, t.result, t.tokens_used,
                    t.cost_usd::FLOAT8 AS cost_usd, t.created_at, t.started_at,
                    t.completed_at, t.parent_task_id,
                    t.max_cost_usd::FLOAT8 AS max_cost_usd, t.permission_level,
                    t.template_name, t.on_child_failure, t.role_name
             FROM agent_tasks t
             WHERE t.template_name IS NOT NULL
               AND t.tokens_used > $1 * (
                 SELECT AVG(tokens_used) FROM agent_tasks
                 WHERE template_name = t.template_name
               )
             ORDER BY t.tokens_used DESC",
        )
        .bind(threshold)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get all budget period rows.
    pub async fn get_agent_budgets(&self) -> Result<Vec<AgentBudgetRow>> {
        let rows = sqlx::query_as::<_, AgentBudgetRow>(
            "SELECT id, period, budget_usd::FLOAT8 AS budget_usd, spent_usd::FLOAT8 AS spent_usd,
                    tokens_used, period_start, updated_at
             FROM agent_budgets ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Update a budget period's limit.
    pub async fn update_agent_budget(&self, id: i64, budget_usd: f64) -> Result<()> {
        sqlx::query(
            "UPDATE agent_budgets SET budget_usd = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(budget_usd)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Claim the highest-priority pending agent task, atomically setting it to in_progress.
    ///
    /// Priority order: urgent=0 > high=1 > normal=2 > low=3, then FIFO by created_at.
    /// Skips tasks with unsatisfied dependencies and parent tasks (which auto-complete
    /// when all children finish).
    pub async fn claim_pending_agent_task(
        &self,
        assigned_agent: &str,
    ) -> Result<Option<AgentTaskRow>> {
        let row = sqlx::query_as::<_, AgentTaskRow>(
            "WITH next_task AS (
                SELECT id FROM agent_tasks
                WHERE status = 'pending'
                -- Skip tasks that have unsatisfied dependencies
                AND NOT EXISTS (
                    SELECT 1 FROM agent_task_deps d
                    JOIN agent_tasks dep ON dep.id = d.depends_on
                    WHERE d.task_id = agent_tasks.id
                    AND dep.status NOT IN ('completed')
                )
                -- Skip parent tasks (they auto-complete when children finish)
                AND NOT EXISTS (
                    SELECT 1 FROM agent_tasks child
                    WHERE child.parent_task_id = agent_tasks.id
                )
                ORDER BY
                    CASE priority
                        WHEN 'urgent' THEN 0
                        WHEN 'high' THEN 1
                        WHEN 'normal' THEN 2
                        WHEN 'low' THEN 3
                        ELSE 4
                    END,
                    created_at ASC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            UPDATE agent_tasks SET
                status = 'in_progress',
                assigned_agent = $1,
                started_at = NOW()
            FROM next_task
            WHERE agent_tasks.id = next_task.id
            RETURNING agent_tasks.id, agent_tasks.title, agent_tasks.description,
                      agent_tasks.status, agent_tasks.priority, agent_tasks.agent_model,
                      agent_tasks.assigned_agent, agent_tasks.source, agent_tasks.result,
                      agent_tasks.tokens_used, agent_tasks.cost_usd::FLOAT8 AS cost_usd,
                      agent_tasks.created_at, agent_tasks.started_at, agent_tasks.completed_at,
                      agent_tasks.parent_task_id,
                      agent_tasks.max_cost_usd::FLOAT8 AS max_cost_usd,
                      agent_tasks.permission_level,
                      agent_tasks.template_name, agent_tasks.on_child_failure,
                      agent_tasks.role_name",
        )
        .bind(assigned_agent)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Mark an agent task as completed/failed with result data.
    ///
    /// Increments (not replaces) tokens_used and cost_usd to support
    /// multi-step execution where costs accumulate.
    pub async fn complete_agent_task(
        &self,
        task_id: i64,
        status: &str,
        result: Option<&serde_json::Value>,
        tokens_used: i64,
        cost_usd: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE agent_tasks SET
                status = $1,
                result = $2,
                tokens_used = tokens_used + $3,
                cost_usd = cost_usd + $4,
                completed_at = NOW()
             WHERE id = $5",
        )
        .bind(status)
        .bind(result)
        .bind(tokens_used)
        .bind(cost_usd)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Check if all budget periods allow more agent spending.
    ///
    /// Returns true if spending is allowed (under budget on ALL active periods).
    /// If any period is over budget, returns false.
    pub async fn check_agent_budget(&self) -> Result<bool> {
        let over_budget: Option<bool> = sqlx::query_scalar(
            "SELECT BOOL_OR(spent_usd >= budget_usd) FROM agent_budgets
             WHERE period_start <= NOW()
               AND period_start + CASE period
                   WHEN 'daily' THEN INTERVAL '1 day'
                   WHEN 'weekly' THEN INTERVAL '1 week'
                   WHEN 'monthly' THEN INTERVAL '1 month'
                   END > NOW()",
        )
        .fetch_optional(&self.pool)
        .await?;
        // If no rows match or no period is over budget, allow spending
        Ok(!over_budget.unwrap_or(false))
    }

    /// Increment spent_usd and tokens_used on ALL active budget period rows.
    pub async fn update_agent_budget_spending(
        &self,
        tokens: i64,
        cost: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE agent_budgets SET
                spent_usd = spent_usd + $1,
                tokens_used = tokens_used + $2,
                updated_at = NOW()
             WHERE period_start <= NOW()
               AND period_start + CASE period
                   WHEN 'daily' THEN INTERVAL '1 day'
                   WHEN 'weekly' THEN INTERVAL '1 week'
                   WHEN 'monthly' THEN INTERVAL '1 month'
                   END > NOW()",
        )
        .bind(cost)
        .bind(tokens)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Rotate budget periods whose window has elapsed.
    ///
    /// Resets spent_usd/tokens_used to zero and advances period_start to the
    /// current window boundary. Returns the number of periods rotated.
    pub async fn rotate_agent_budget_periods(&self) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE agent_budgets SET
                spent_usd = 0,
                tokens_used = 0,
                period_start = CASE period
                    WHEN 'daily' THEN date_trunc('day', NOW())
                    WHEN 'weekly' THEN date_trunc('week', NOW())
                    WHEN 'monthly' THEN date_trunc('month', NOW())
                    END,
                updated_at = NOW()
             WHERE period_start + CASE period
                 WHEN 'daily' THEN INTERVAL '1 day'
                 WHEN 'weekly' THEN INTERVAL '1 week'
                 WHEN 'monthly' THEN INTERVAL '1 month'
                 END <= NOW()",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Retrieve all agent workflow templates, ordered by name.
    pub async fn get_all_templates(&self) -> Result<Vec<AgentTemplateRow>> {
        let rows = sqlx::query_as::<_, AgentTemplateRow>(
            "SELECT id, name, description, steps, created_at, role_name
             FROM agent_templates ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Retrieve a single template by name.
    pub async fn get_template_by_name(&self, name: &str) -> Result<Option<AgentTemplateRow>> {
        let row = sqlx::query_as::<_, AgentTemplateRow>(
            "SELECT id, name, description, steps, created_at, role_name
             FROM agent_templates WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Expand a template into a parent task and child tasks within a single transaction.
    ///
    /// Creates one parent task (with `template_name` set) and one child task per step.
    /// Child permission levels are capped at the parent's level. Dependencies between
    /// steps (via `depends_on_step` indices) become rows in `agent_task_deps`.
    ///
    /// Returns the parent task ID on success.
    pub async fn expand_template(
        &self,
        template_name: &str,
        parent_title: &str,
        parent_desc: &str,
        priority: &str,
        max_cost_usd: Option<f64>,
        permission_level: i32,
        role_name: Option<&str>,
    ) -> Result<i64> {
        let template = self
            .get_template_by_name(template_name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Template '{}' not found", template_name))?;

        let steps = template
            .steps
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Template steps is not an array"))?;

        let mut tx = self.pool.begin().await?;

        // Insert parent task
        let parent_id: i64 = sqlx::query_scalar(
            "INSERT INTO agent_tasks (title, description, priority, source, max_cost_usd,
                                      permission_level, template_name, on_child_failure, role_name)
             VALUES ($1, $2, $3, 'manual', $4, $5, $6, 'fail', $7)
             RETURNING id",
        )
        .bind(parent_title)
        .bind(parent_desc)
        .bind(priority)
        .bind(max_cost_usd)
        .bind(permission_level)
        .bind(template_name)
        .bind(role_name)
        .fetch_one(&mut *tx)
        .await?;

        // Insert child tasks, collecting their IDs for dependency wiring
        let mut child_ids: Vec<i64> = Vec::with_capacity(steps.len());

        for step in steps {
            let step_title = step
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("Untitled step");
            let step_desc = step
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            let step_level = step
                .get("permission_level")
                .and_then(|l| l.as_i64())
                .unwrap_or(1) as i32;

            // Child permission = min(step requested, parent's level)
            let child_level = step_level.min(permission_level);

            let child_title = format!("{}: {}", parent_title, step_title);

            let child_id: i64 = sqlx::query_scalar(
                "INSERT INTO agent_tasks (title, description, priority, source, parent_task_id,
                                          permission_level, template_name, role_name)
                 VALUES ($1, $2, $3, 'automated', $4, $5, $6, $7)
                 RETURNING id",
            )
            .bind(&child_title)
            .bind(step_desc)
            .bind(priority)
            .bind(parent_id)
            .bind(child_level)
            .bind(template_name)
            .bind(role_name)
            .fetch_one(&mut *tx)
            .await?;

            child_ids.push(child_id);
        }

        // Wire up dependencies based on depends_on_step indices
        for (i, step) in steps.iter().enumerate() {
            if let Some(dep_idx) = step.get("depends_on_step").and_then(|d| d.as_u64()) {
                let dep_idx = dep_idx as usize;
                if dep_idx < child_ids.len() && dep_idx != i {
                    sqlx::query(
                        "INSERT INTO agent_task_deps (task_id, depends_on) VALUES ($1, $2)",
                    )
                    .bind(child_ids[i])
                    .bind(child_ids[dep_idx])
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }

        tx.commit().await?;
        Ok(parent_id)
    }

    /// Check if all children of a parent task are terminal (completed/cancelled/failed).
    ///
    /// If so, mark the parent as completed or failed based on `on_child_failure` policy.
    /// Returns `Some(parent_row)` if the parent was just completed, `None` if children
    /// are still pending/running.
    pub async fn try_complete_parent(&self, parent_id: i64) -> Result<Option<AgentTaskRow>> {
        // Count non-terminal children
        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM agent_tasks
             WHERE parent_task_id = $1
             AND status NOT IN ('completed', 'cancelled', 'failed')",
        )
        .bind(parent_id)
        .fetch_one(&self.pool)
        .await?;

        if pending > 0 {
            return Ok(None);
        }

        // All children are terminal — check for failures
        let parent = self
            .get_agent_task(parent_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Parent task {} not found", parent_id))?;

        // Already terminal
        if matches!(
            parent.status.as_str(),
            "completed" | "failed" | "cancelled"
        ) {
            return Ok(None);
        }

        let any_failed: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM agent_tasks
                WHERE parent_task_id = $1 AND status = 'failed'
            )",
        )
        .bind(parent_id)
        .fetch_one(&self.pool)
        .await?;

        let new_status = if any_failed && parent.on_child_failure == "fail" {
            "failed"
        } else {
            "completed"
        };

        sqlx::query(
            "UPDATE agent_tasks SET status = $1, completed_at = NOW() WHERE id = $2",
        )
        .bind(new_status)
        .bind(parent_id)
        .execute(&self.pool)
        .await?;

        self.get_agent_task(parent_id).await
    }

    /// Get all child tasks of a parent, ordered by ID (creation order).
    pub async fn get_child_tasks(&self, parent_id: i64) -> Result<Vec<AgentTaskRow>> {
        let rows = sqlx::query_as::<_, AgentTaskRow>(
            "SELECT id, title, description, status, priority, agent_model, assigned_agent,
                    source, result, tokens_used, cost_usd::FLOAT8 AS cost_usd,
                    created_at, started_at, completed_at, parent_task_id,
                    max_cost_usd::FLOAT8 AS max_cost_usd, permission_level,
                    template_name, on_child_failure, role_name
             FROM agent_tasks
             WHERE parent_task_id = $1
             ORDER BY id",
        )
        .bind(parent_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get dependency task IDs for a given task.
    pub async fn get_task_deps(&self, task_id: i64) -> Result<Vec<i64>> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            "SELECT depends_on FROM agent_task_deps WHERE task_id = $1",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Cancel all pending sibling tasks of a parent (used when on_child_failure = 'fail').
    pub async fn cancel_pending_siblings(&self, parent_id: i64) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE agent_tasks SET status = 'cancelled', completed_at = NOW()
             WHERE parent_task_id = $1 AND status = 'pending'",
        )
        .bind(parent_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Get sibling tasks that share the same parent, excluding the given task.
    pub async fn get_sibling_tasks(
        &self,
        parent_id: i64,
        exclude_id: i64,
    ) -> Result<Vec<AgentTaskRow>> {
        let rows = sqlx::query_as::<_, AgentTaskRow>(
            "SELECT id, title, description, status, priority, agent_model, assigned_agent,
                    source, result, tokens_used, cost_usd::FLOAT8 AS cost_usd,
                    created_at, started_at, completed_at, parent_task_id,
                    max_cost_usd::FLOAT8 AS max_cost_usd, permission_level,
                    template_name, on_child_failure, role_name
             FROM agent_tasks
             WHERE parent_task_id = $1 AND id != $2
             ORDER BY created_at DESC LIMIT 10",
        )
        .bind(parent_id)
        .bind(exclude_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get previous attempts at tasks with the same title (failed/cancelled).
    pub async fn get_previous_attempts(
        &self,
        title: &str,
        exclude_id: i64,
    ) -> Result<Vec<AgentTaskRow>> {
        let rows = sqlx::query_as::<_, AgentTaskRow>(
            "SELECT id, title, description, status, priority, agent_model, assigned_agent,
                    source, result, tokens_used, cost_usd::FLOAT8 AS cost_usd,
                    created_at, started_at, completed_at, parent_task_id,
                    max_cost_usd::FLOAT8 AS max_cost_usd, permission_level,
                    template_name, on_child_failure, role_name
             FROM agent_tasks
             WHERE title = $1 AND id != $2 AND status IN ('failed', 'cancelled')
             ORDER BY created_at DESC LIMIT 5",
        )
        .bind(title)
        .bind(exclude_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
