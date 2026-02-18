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
                COALESCE(SUM(tested) FILTER (WHERE status = 'completed'), 0) AS total_tested,
                COALESCE(SUM(found) FILTER (WHERE status = 'completed'), 0) AS total_found
             FROM work_blocks WHERE search_job_id = $1",
        )
        .bind(job_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
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
        let (sql, has_form) = if let Some(_) = form {
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
