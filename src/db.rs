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
//! ## Sync Wrapper
//!
//! Engine modules run inside Rayon thread pools (no Tokio runtime). The
//! `insert_prime_sync` method bridges async sqlx operations into sync contexts
//! via `tokio::runtime::Handle::block_on`. This is safe because Rayon threads
//! are not Tokio tasks — they won't deadlock the executor.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

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
    fn safe_sort_column(&self) -> &str {
        match self.sort_by.as_deref() {
            Some("digits") => "digits",
            Some("form") => "form",
            Some("expression") => "expression",
            Some("found_at") => "found_at",
            _ => "id",
        }
    }
    fn safe_sort_dir(&self) -> &str {
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

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        // Parse the URL manually to preserve the full username — sqlx's built-in
        // parser strips the ".project-ref" suffix that Supabase pooler requires.
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
            .username(&username);
        if let Some(ref pw) = password {
            opts = opts.password(pw);
        }
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;
        Ok(Database { pool })
    }

    pub async fn insert_prime(
        &self,
        form: &str,
        expression: &str,
        digits: u64,
        search_params: &str,
        proof_method: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO primes (form, expression, digits, found_at, search_params, proof_method)
             VALUES ($1, $2, $3, NOW(), $4, $5)",
        )
        .bind(form)
        .bind(expression)
        .bind(digits as i64)
        .bind(search_params)
        .bind(proof_method)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_prime_ignore(
        &self,
        form: &str,
        expression: &str,
        digits: u64,
        search_params: &str,
        proof_method: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO primes (form, expression, digits, found_at, search_params, proof_method)
             VALUES ($1, $2, $3, NOW(), $4, $5)
             ON CONFLICT (form, expression) DO NOTHING",
        )
        .bind(form)
        .bind(expression)
        .bind(digits as i64)
        .bind(search_params)
        .bind(proof_method)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Synchronous wrapper for engine modules running on rayon threads.
    /// Uses the provided tokio runtime handle to block on the async insert.
    pub fn insert_prime_sync(
        &self,
        rt: &tokio::runtime::Handle,
        form: &str,
        expression: &str,
        digits: u64,
        search_params: &str,
        proof_method: &str,
    ) -> Result<()> {
        rt.block_on(self.insert_prime(form, expression, digits, search_params, proof_method))
    }

    pub fn insert_prime_ignore_sync(
        &self,
        rt: &tokio::runtime::Handle,
        form: &str,
        expression: &str,
        digits: u64,
        search_params: &str,
        proof_method: &str,
    ) -> Result<()> {
        rt.block_on(self.insert_prime_ignore(form, expression, digits, search_params, proof_method))
    }

    pub async fn get_primes_filtered(
        &self,
        limit: i64,
        offset: i64,
        filter: &PrimeFilter,
    ) -> Result<Vec<PrimeRecord>> {
        let mut conditions = Vec::new();
        let mut param_idx = 1u32;

        if filter.form.is_some() {
            conditions.push(format!("form = ${}", param_idx));
            param_idx += 1;
        }
        if filter.search.is_some() {
            conditions.push(format!("expression LIKE ${}", param_idx));
            param_idx += 1;
        }
        if filter.min_digits.is_some() {
            conditions.push(format!("digits >= ${}", param_idx));
            param_idx += 1;
        }
        if filter.max_digits.is_some() {
            conditions.push(format!("digits <= ${}", param_idx));
            param_idx += 1;
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id, form, expression, digits, found_at, proof_method FROM primes{} ORDER BY {} {} LIMIT ${} OFFSET ${}",
            where_clause,
            filter.safe_sort_column(),
            filter.safe_sort_dir(),
            param_idx,
            param_idx + 1,
        );

        let mut query = sqlx::query_as::<_, PrimeRecord>(&sql);
        if let Some(ref form) = filter.form {
            query = query.bind(form);
        }
        if let Some(ref search) = filter.search {
            query = query.bind(format!("%{}%", search));
        }
        if let Some(min_d) = filter.min_digits {
            query = query.bind(min_d);
        }
        if let Some(max_d) = filter.max_digits {
            query = query.bind(max_d);
        }
        query = query.bind(limit);
        query = query.bind(offset);

        let records = query.fetch_all(&self.pool).await?;
        Ok(records)
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // --- Worker coordination ---

    pub async fn upsert_worker(
        &self,
        worker_id: &str,
        hostname: &str,
        cores: i32,
        search_type: &str,
        search_params: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO workers (worker_id, hostname, cores, search_type, search_params, last_heartbeat)
             VALUES ($1, $2, $3, $4, $5, NOW())
             ON CONFLICT (worker_id) DO UPDATE SET
               hostname = EXCLUDED.hostname, cores = EXCLUDED.cores,
               search_type = EXCLUDED.search_type, search_params = EXCLUDED.search_params,
               last_heartbeat = NOW()",
        )
        .bind(worker_id)
        .bind(hostname)
        .bind(cores)
        .bind(search_type)
        .bind(search_params)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn worker_heartbeat_rpc(
        &self,
        worker_id: &str,
        hostname: &str,
        cores: i32,
        search_type: &str,
        search_params: &str,
        tested: i64,
        found: i64,
        current: &str,
        checkpoint: Option<&str>,
        metrics: Option<&Value>,
    ) -> Result<Option<String>> {
        let command: Option<String> =
            sqlx::query_scalar("SELECT worker_heartbeat($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
                .bind(worker_id)
                .bind(hostname)
                .bind(cores)
                .bind(search_type)
                .bind(search_params)
                .bind(tested)
                .bind(found)
                .bind(current)
                .bind(checkpoint)
                .bind(metrics)
                .fetch_one(&self.pool)
                .await?;
        Ok(command)
    }

    pub async fn delete_worker(&self, worker_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM workers WHERE worker_id = $1")
            .bind(worker_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_worker_command(&self, worker_id: &str, command: &str) -> Result<()> {
        sqlx::query("UPDATE workers SET pending_command = $1 WHERE worker_id = $2")
            .bind(command)
            .bind(worker_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_all_workers(&self) -> Result<Vec<WorkerRow>> {
        let rows = sqlx::query_as::<_, WorkerRow>(
            "SELECT worker_id, hostname, cores, search_type, search_params,
                    tested, found, current, checkpoint, metrics,
                    registered_at, last_heartbeat
             FROM workers ORDER BY worker_id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn prune_stale_workers(&self, timeout_secs: i64) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM workers WHERE last_heartbeat < NOW() - ($1 || ' seconds')::interval",
        )
        .bind(timeout_secs.to_string())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    // --- Search jobs ---

    pub async fn create_search_job(
        &self,
        search_type: &str,
        params: &Value,
        range_start: i64,
        range_end: i64,
        block_size: i64,
    ) -> Result<i64> {
        let mut tx = self.pool.begin().await?;
        let job_id: i64 = sqlx::query_scalar(
            "INSERT INTO search_jobs (search_type, params, status, range_start, range_end, block_size, started_at)
             VALUES ($1, $2, 'running', $3, $4, $5, NOW())
             RETURNING id",
        )
        .bind(search_type)
        .bind(params)
        .bind(range_start)
        .bind(range_end)
        .bind(block_size)
        .fetch_one(&mut *tx)
        .await?;

        let mut start = range_start;
        while start < range_end {
            let end = (start + block_size).min(range_end);
            sqlx::query(
                "INSERT INTO work_blocks (search_job_id, block_start, block_end) VALUES ($1, $2, $3)",
            )
            .bind(job_id)
            .bind(start)
            .bind(end)
            .execute(&mut *tx)
            .await?;
            start = end;
        }
        tx.commit().await?;
        Ok(job_id)
    }

    pub async fn get_search_jobs(&self) -> Result<Vec<SearchJobRow>> {
        let rows = sqlx::query_as::<_, SearchJobRow>(
            "SELECT id, search_type, params, status, error,
                    created_at, started_at, stopped_at,
                    range_start, range_end, block_size,
                    total_tested, total_found
             FROM search_jobs ORDER BY id DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_search_job(&self, job_id: i64) -> Result<Option<SearchJobRow>> {
        let row = sqlx::query_as::<_, SearchJobRow>(
            "SELECT id, search_type, params, status, error,
                    created_at, started_at, stopped_at,
                    range_start, range_end, block_size,
                    total_tested, total_found
             FROM search_jobs WHERE id = $1",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn update_search_job_status(
        &self,
        job_id: i64,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let stopped = if matches!(status, "completed" | "cancelled" | "failed") {
            Some(chrono::Utc::now())
        } else {
            None
        };
        sqlx::query(
            "UPDATE search_jobs SET status = $1, error = $2, stopped_at = COALESCE($3, stopped_at) WHERE id = $4",
        )
        .bind(status)
        .bind(error)
        .bind(stopped)
        .bind(job_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn claim_work_block(
        &self,
        job_id: i64,
        worker_id: &str,
    ) -> Result<Option<WorkBlock>> {
        let row = sqlx::query_as::<_, WorkBlock>(
            "SELECT block_id, block_start, block_end FROM claim_work_block($1, $2)",
        )
        .bind(job_id)
        .bind(worker_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn complete_work_block(&self, block_id: i64, tested: i64, found: i64) -> Result<()> {
        sqlx::query(
            "UPDATE work_blocks SET status = 'completed', completed_at = NOW(), tested = $1, found = $2 WHERE id = $3",
        )
        .bind(tested)
        .bind(found)
        .bind(block_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn fail_work_block(&self, block_id: i64) -> Result<()> {
        sqlx::query("UPDATE work_blocks SET status = 'failed' WHERE id = $1")
            .bind(block_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn reclaim_stale_blocks(&self, stale_seconds: i32) -> Result<i32> {
        let count: i32 = sqlx::query_scalar("SELECT reclaim_stale_blocks($1)")
            .bind(stale_seconds)
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    pub async fn get_job_block_summary(&self, job_id: i64) -> Result<JobBlockSummary> {
        let row = sqlx::query_as::<_, JobBlockSummary>(
            "SELECT
                COUNT(*) FILTER (WHERE status = 'available') AS available,
                COUNT(*) FILTER (WHERE status = 'claimed') AS claimed,
                COUNT(*) FILTER (WHERE status = 'completed') AS completed,
                COUNT(*) FILTER (WHERE status = 'failed') AS failed,
                COALESCE(SUM(tested) FILTER (WHERE status = 'completed'), 0)::BIGINT AS total_tested,
                COALESCE(SUM(found) FILTER (WHERE status = 'completed'), 0)::BIGINT AS total_found
             FROM work_blocks WHERE search_job_id = $1",
        )
        .bind(job_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    // --- Agent management ---

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

    pub async fn get_agent_events(
        &self,
        task_id: Option<i64>,
        limit: i64,
    ) -> Result<Vec<AgentEventRow>> {
        let rows = if let Some(tid) = task_id {
            sqlx::query_as::<_, AgentEventRow>(
                "SELECT id, task_id, event_type, agent, summary, detail, created_at
                 FROM agent_events WHERE task_id = $1
                 ORDER BY created_at DESC LIMIT $2",
            )
            .bind(tid)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, AgentEventRow>(
                "SELECT id, task_id, event_type, agent, summary, detail, created_at
                 FROM agent_events
                 ORDER BY created_at DESC LIMIT $1",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    pub async fn insert_agent_event(
        &self,
        task_id: Option<i64>,
        event_type: &str,
        agent: Option<&str>,
        summary: &str,
        detail: Option<&Value>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO agent_events (task_id, event_type, agent, summary, detail)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(task_id)
        .bind(event_type)
        .bind(agent)
        .bind(summary)
        .bind(detail)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

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

    // --- Agent execution engine ---

    /// Claim the highest-priority pending agent task, atomically setting it to in_progress.
    /// Priority order: urgent=0 > high=1 > normal=2 > low=3, then FIFO by created_at.
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
    /// Returns true if spending is allowed (under budget on ALL periods).
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
    /// Resets spent_usd/tokens_used and advances period_start to the current window.
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

    // --- Agent templates & task decomposition ---

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
    /// If so, mark the parent as completed or failed based on `on_child_failure` policy.
    ///
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

    // --- Verification ---

    pub async fn get_unverified_primes(&self, limit: i64) -> Result<Vec<PrimeDetail>> {
        let rows = sqlx::query_as::<_, PrimeDetail>(
            "SELECT id, form, expression, digits, found_at, search_params, proof_method
             FROM primes WHERE NOT verified ORDER BY id LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_unverified_primes_filtered(
        &self,
        limit: i64,
        form: Option<&str>,
        force: bool,
    ) -> Result<Vec<PrimeDetail>> {
        let verified_clause = if force { "" } else { "NOT verified AND" };
        let (sql, has_form) = if form.is_some() {
            (
                format!(
                    "SELECT id, form, expression, digits, found_at, search_params, proof_method
                     FROM primes WHERE {} form = $1 ORDER BY id LIMIT $2",
                    verified_clause
                ),
                true,
            )
        } else {
            (
                format!(
                    "SELECT id, form, expression, digits, found_at, search_params, proof_method
                     FROM primes WHERE {} TRUE ORDER BY id LIMIT $1",
                    verified_clause
                ),
                false,
            )
        };
        let mut query = sqlx::query_as::<_, PrimeDetail>(&sql);
        if has_form {
            query = query.bind(form.unwrap());
            query = query.bind(limit);
        } else {
            query = query.bind(limit);
        }
        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_prime_by_id(&self, id: i64) -> Result<Option<PrimeDetail>> {
        let row = sqlx::query_as::<_, PrimeDetail>(
            "SELECT id, form, expression, digits, found_at, search_params, proof_method
             FROM primes WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn mark_verified(&self, id: i64, method: &str, tier: i16) -> Result<()> {
        sqlx::query(
            "UPDATE primes SET verified = true, verified_at = NOW(), verification_method = $1, verification_tier = $2 WHERE id = $3",
        )
        .bind(method)
        .bind(tier)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_verification_failed(&self, id: i64, reason: &str) -> Result<()> {
        sqlx::query(
            "UPDATE primes SET verification_method = $1, verification_tier = 0 WHERE id = $2",
        )
        .bind(reason)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_filtered_count(&self, filter: &PrimeFilter) -> Result<i64> {
        let mut conditions = Vec::new();
        let mut param_idx = 1u32;

        if filter.form.is_some() {
            conditions.push(format!("form = ${}", param_idx));
            param_idx += 1;
        }
        if filter.search.is_some() {
            conditions.push(format!("expression LIKE ${}", param_idx));
            param_idx += 1;
        }
        if filter.min_digits.is_some() {
            conditions.push(format!("digits >= ${}", param_idx));
            param_idx += 1;
        }
        if filter.max_digits.is_some() {
            conditions.push(format!("digits <= ${}", param_idx));
            param_idx += 1;
        }
        let _ = param_idx;

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let sql = format!("SELECT COUNT(*) as count FROM primes{}", where_clause);

        let mut query = sqlx::query_scalar::<_, i64>(&sql);
        if let Some(ref form) = filter.form {
            query = query.bind(form);
        }
        if let Some(ref search) = filter.search {
            query = query.bind(format!("%{}%", search));
        }
        if let Some(min_d) = filter.min_digits {
            query = query.bind(min_d);
        }
        if let Some(max_d) = filter.max_digits {
            query = query.bind(max_d);
        }

        let count = query.fetch_one(&self.pool).await?;
        Ok(count)
    }

    // --- Agent memory ---

    /// Retrieve all agent memory entries, ordered by category then key.
    pub async fn get_all_agent_memory(&self) -> Result<Vec<AgentMemoryRow>> {
        let rows = sqlx::query_as::<_, AgentMemoryRow>(
            "SELECT id, key, value, category, created_by_task, created_at, updated_at
             FROM agent_memory ORDER BY category, key",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Retrieve agent memory entries filtered by category.
    pub async fn get_agent_memory_by_category(&self, category: &str) -> Result<Vec<AgentMemoryRow>> {
        let rows = sqlx::query_as::<_, AgentMemoryRow>(
            "SELECT id, key, value, category, created_by_task, created_at, updated_at
             FROM agent_memory WHERE category = $1 ORDER BY key",
        )
        .bind(category)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Upsert an agent memory entry. If the key already exists, update value/category/task.
    pub async fn upsert_agent_memory(
        &self,
        key: &str,
        value: &str,
        category: &str,
        task_id: Option<i64>,
    ) -> Result<AgentMemoryRow> {
        let row = sqlx::query_as::<_, AgentMemoryRow>(
            "INSERT INTO agent_memory (key, value, category, created_by_task)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (key) DO UPDATE SET
               value = EXCLUDED.value,
               category = EXCLUDED.category,
               created_by_task = EXCLUDED.created_by_task,
               updated_at = now()
             RETURNING id, key, value, category, created_by_task, created_at, updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(category)
        .bind(task_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Delete an agent memory entry by key. Returns true if a row was deleted.
    pub async fn delete_agent_memory(&self, key: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM agent_memory WHERE key = $1")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
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

    /// Get previous attempts at tasks with the same title (failed/cancelled), excluding the given task.
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

    // --- Agent roles ---

    /// Retrieve all agent roles, ordered by name.
    pub async fn get_all_roles(&self) -> Result<Vec<AgentRoleRow>> {
        let rows = sqlx::query_as::<_, AgentRoleRow>(
            "SELECT id, name, description, domains, default_permission_level, default_model,
                    system_prompt, default_max_cost_usd::FLOAT8 AS default_max_cost_usd,
                    created_at, updated_at
             FROM agent_roles ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Retrieve a single role by name.
    pub async fn get_role_by_name(&self, name: &str) -> Result<Option<AgentRoleRow>> {
        let row = sqlx::query_as::<_, AgentRoleRow>(
            "SELECT id, name, description, domains, default_permission_level, default_model,
                    system_prompt, default_max_cost_usd::FLOAT8 AS default_max_cost_usd,
                    created_at, updated_at
             FROM agent_roles WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get templates associated with a role via the junction table.
    /// Returns templates linked through `agent_role_templates`.
    pub async fn get_role_templates(&self, role_name: &str) -> Result<Vec<AgentTemplateRow>> {
        let rows = sqlx::query_as::<_, AgentTemplateRow>(
            "SELECT t.id, t.name, t.description, t.steps, t.created_at, t.role_name
             FROM agent_templates t
             JOIN agent_role_templates rt ON rt.template_name = t.name
             WHERE rt.role_name = $1
             ORDER BY t.name",
        )
        .bind(role_name)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ── Project Management ──────────────────────────────────────

    /// Create a new project with phases from a parsed TOML configuration.
    pub async fn create_project(
        &self,
        config: &crate::project::ProjectConfig,
        toml_source: Option<&str>,
    ) -> Result<i64> {
        use crate::project;

        let slug = project::slugify(&config.project.name);
        let phases = if config.strategy.auto_strategy && config.strategy.phases.is_empty() {
            project::generate_auto_strategy(config)
        } else {
            config.strategy.phases.clone()
        };

        let mut tx = self.pool.begin().await?;

        let project_id: i64 = sqlx::query_scalar(
            "INSERT INTO projects (slug, name, description, objective, form, toml_source,
                                   target, competitive, strategy, infrastructure, budget)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             RETURNING id",
        )
        .bind(&slug)
        .bind(&config.project.name)
        .bind(&config.project.description)
        .bind(config.project.objective.to_string())
        .bind(&config.project.form)
        .bind(toml_source)
        .bind(serde_json::to_value(&config.target)?)
        .bind(serde_json::to_value(&config.competitive)?)
        .bind(serde_json::to_value(&config.strategy)?)
        .bind(serde_json::to_value(&config.infrastructure)?)
        .bind(serde_json::to_value(&config.budget)?)
        .fetch_one(&mut *tx)
        .await?;

        // Insert phases
        for (i, phase) in phases.iter().enumerate() {
            sqlx::query(
                "INSERT INTO project_phases
                    (project_id, name, description, phase_order, search_params,
                     block_size, depends_on, activation_condition, completion_condition)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(project_id)
            .bind(&phase.name)
            .bind(&phase.description)
            .bind(i as i32)
            .bind(&phase.search_params)
            .bind(phase.block_size.unwrap_or(1000))
            .bind(
                phase
                    .depends_on
                    .as_ref()
                    .cloned()
                    .unwrap_or_default()
                    .as_slice(),
            )
            .bind(phase.activation_condition.as_deref())
            .bind(&phase.completion)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(project_id)
    }

    /// List projects, optionally filtered by status.
    pub async fn get_projects(
        &self,
        status_filter: Option<&str>,
    ) -> Result<Vec<crate::project::ProjectRow>> {
        let rows = if let Some(status) = status_filter {
            sqlx::query_as::<_, crate::project::ProjectRow>(
                "SELECT id, slug, name, description, objective, form, status, toml_source,
                        target, competitive, strategy, infrastructure, budget,
                        total_tested, total_found, best_prime_id, best_digits,
                        total_core_hours::FLOAT8 AS total_core_hours,
                        total_cost_usd::FLOAT8 AS total_cost_usd,
                        created_at, started_at, completed_at, updated_at
                 FROM projects WHERE status = $1 ORDER BY created_at DESC",
            )
            .bind(status)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, crate::project::ProjectRow>(
                "SELECT id, slug, name, description, objective, form, status, toml_source,
                        target, competitive, strategy, infrastructure, budget,
                        total_tested, total_found, best_prime_id, best_digits,
                        total_core_hours::FLOAT8 AS total_core_hours,
                        total_cost_usd::FLOAT8 AS total_cost_usd,
                        created_at, started_at, completed_at, updated_at
                 FROM projects ORDER BY created_at DESC",
            )
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    /// Get a single project by slug.
    pub async fn get_project_by_slug(
        &self,
        slug: &str,
    ) -> Result<Option<crate::project::ProjectRow>> {
        let row = sqlx::query_as::<_, crate::project::ProjectRow>(
            "SELECT id, slug, name, description, objective, form, status, toml_source,
                    target, competitive, strategy, infrastructure, budget,
                    total_tested, total_found, best_prime_id, best_digits,
                    total_core_hours::FLOAT8 AS total_core_hours,
                    total_cost_usd::FLOAT8 AS total_cost_usd,
                    created_at, started_at, completed_at, updated_at
             FROM projects WHERE slug = $1",
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get all phases for a project, ordered by phase_order.
    pub async fn get_project_phases(
        &self,
        project_id: i64,
    ) -> Result<Vec<crate::project::ProjectPhaseRow>> {
        let rows = sqlx::query_as::<_, crate::project::ProjectPhaseRow>(
            "SELECT id, project_id, name, description, phase_order, status,
                    search_params, block_size, depends_on, activation_condition,
                    completion_condition, search_job_id,
                    total_tested, total_found, started_at, completed_at
             FROM project_phases WHERE project_id = $1 ORDER BY phase_order",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Update a project's status (draft → active, active → paused, etc.).
    pub async fn update_project_status(&self, project_id: i64, status: &str) -> Result<()> {
        let now = chrono::Utc::now();
        let started = if status == "active" {
            Some(now)
        } else {
            None
        };
        let completed = if matches!(status, "completed" | "cancelled" | "failed") {
            Some(now)
        } else {
            None
        };

        sqlx::query(
            "UPDATE projects SET status = $1, updated_at = NOW(),
                    started_at = COALESCE($2, started_at),
                    completed_at = COALESCE($3, completed_at)
             WHERE id = $4",
        )
        .bind(status)
        .bind(started)
        .bind(completed)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update aggregated progress counters on a project.
    pub async fn update_project_progress(
        &self,
        project_id: i64,
        total_tested: i64,
        total_found: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE projects SET total_tested = $1, total_found = $2, updated_at = NOW()
             WHERE id = $3",
        )
        .bind(total_tested)
        .bind(total_found)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update a phase's status.
    pub async fn update_phase_status(&self, phase_id: i64, status: &str) -> Result<()> {
        let completed = if matches!(status, "completed" | "skipped" | "failed") {
            Some(chrono::Utc::now())
        } else {
            None
        };

        sqlx::query(
            "UPDATE project_phases SET status = $1, completed_at = COALESCE($2, completed_at)
             WHERE id = $3",
        )
        .bind(status)
        .bind(completed)
        .bind(phase_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update phase progress counters.
    pub async fn update_phase_progress(
        &self,
        phase_id: i64,
        total_tested: i64,
        total_found: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE project_phases SET total_tested = $1, total_found = $2 WHERE id = $3",
        )
        .bind(total_tested)
        .bind(total_found)
        .bind(phase_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Activate a phase: set status to active, link the search job.
    pub async fn activate_phase(&self, phase_id: i64, search_job_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE project_phases SET status = 'active', search_job_id = $1, started_at = NOW()
             WHERE id = $2",
        )
        .bind(search_job_id)
        .bind(phase_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Link a search job to a project (set the FK on search_jobs).
    pub async fn link_search_job_to_project(
        &self,
        job_id: i64,
        project_id: i64,
    ) -> Result<()> {
        sqlx::query("UPDATE search_jobs SET project_id = $1 WHERE id = $2")
            .bind(project_id)
            .bind(job_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Insert a project event for the activity log.
    pub async fn insert_project_event(
        &self,
        project_id: i64,
        event_type: &str,
        summary: &str,
        detail: Option<&serde_json::Value>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO project_events (project_id, event_type, summary, detail)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(project_id)
        .bind(event_type)
        .bind(summary)
        .bind(detail)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get recent events for a project.
    pub async fn get_project_events(
        &self,
        project_id: i64,
        limit: i64,
    ) -> Result<Vec<crate::project::ProjectEventRow>> {
        let rows = sqlx::query_as::<_, crate::project::ProjectEventRow>(
            "SELECT id, project_id, event_type, summary, detail, created_at
             FROM project_events WHERE project_id = $1
             ORDER BY created_at DESC LIMIT $2",
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ── Records Tracking ────────────────────────────────────────

    /// Upsert a world record entry (insert or update on form+category).
    pub async fn upsert_record(
        &self,
        form: &str,
        category: &str,
        expression: &str,
        digits: i64,
        holder: Option<&str>,
        discovered_at: Option<&str>,
        source: Option<&str>,
        source_url: Option<&str>,
        our_best_id: Option<i64>,
        our_best_digits: i64,
    ) -> Result<()> {
        let disc_date = discovered_at.and_then(|d| {
            chrono::NaiveDate::parse_from_str(d, "%Y")
                .or_else(|_| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d"))
                .or_else(|_| chrono::NaiveDate::parse_from_str(d, "%b %Y"))
                .ok()
        });

        sqlx::query(
            "INSERT INTO records (form, category, expression, digits, holder, discovered_at,
                                  source, source_url, our_best_id, our_best_digits)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (form, category) DO UPDATE SET
               expression = EXCLUDED.expression,
               digits = EXCLUDED.digits,
               holder = EXCLUDED.holder,
               discovered_at = COALESCE(EXCLUDED.discovered_at, records.discovered_at),
               source = EXCLUDED.source,
               source_url = EXCLUDED.source_url,
               our_best_id = EXCLUDED.our_best_id,
               our_best_digits = EXCLUDED.our_best_digits,
               updated_at = NOW()",
        )
        .bind(form)
        .bind(category)
        .bind(expression)
        .bind(digits)
        .bind(holder)
        .bind(disc_date)
        .bind(source)
        .bind(source_url)
        .bind(our_best_id)
        .bind(our_best_digits)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all records with our-best comparison.
    pub async fn get_records(&self) -> Result<Vec<crate::project::RecordRow>> {
        let rows = sqlx::query_as::<_, crate::project::RecordRow>(
            "SELECT id, form, category, expression, digits, holder, discovered_at,
                    source, source_url, our_best_id, our_best_digits, fetched_at, updated_at
             FROM records ORDER BY form, category",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get our largest prime for a given form (used for records comparison).
    pub async fn get_best_prime_for_form(
        &self,
        form: &str,
    ) -> Result<Option<PrimeRecord>> {
        let row = sqlx::query_as::<_, PrimeRecord>(
            "SELECT id, form, expression, digits, found_at, proof_method
             FROM primes WHERE form = $1 ORDER BY digits DESC LIMIT 1",
        )
        .bind(form)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }
}

// --- Row types for coordination tables ---

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

// --- Row types for agent management ---

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
