//! Search job and work block operations.
//!
//! Search jobs represent a configured search (e.g., "kbn k=3 b=2 n=1..10000")
//! that is divided into work blocks for distributed execution. Workers claim
//! blocks using `FOR UPDATE SKIP LOCKED` for safe concurrent access.
//!
//! ## Lifecycle
//!
//! 1. `create_search_job` — inserts job + generates work_blocks in a transaction
//! 2. Workers call `claim_work_block` to atomically grab available blocks
//! 3. On completion, `complete_work_block_with_cores` records duration and stats
//! 4. `reclaim_stale_blocks` recovers blocks from crashed workers (runs every 30s)
//! 5. `get_job_block_summary` aggregates block status for progress reporting

use super::{Database, JobBlockSummary, SearchJobRow, WorkBlock, WorkBlockDetails};
use anyhow::Result;
use serde_json::Value;

impl Database {
    /// Create a new search job and generate its work blocks in a single transaction.
    ///
    /// The range [range_start, range_end) is divided into blocks of `block_size`,
    /// each inserted as a row in `work_blocks` with status 'available'.
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

        // Estimate block duration from the cost model for dynamic stale timeout
        let estimated_duration_s: Option<i32> = {
            use crate::project::{estimate_digits_for_form, secs_per_candidate};
            let mid = ((range_start + range_end) / 2) as u64;
            let avg_digits = estimate_digits_for_form(search_type, mid);
            if avg_digits > 0 {
                let spc = secs_per_candidate(search_type, avg_digits, false);
                let candidates_per_block = block_size as f64;
                let est = (spc * candidates_per_block).ceil() as i32;
                if est > 0 { Some(est) } else { None }
            } else {
                None
            }
        };

        let mut start = range_start;
        while start < range_end {
            let end = (start + block_size).min(range_end);
            sqlx::query(
                "INSERT INTO work_blocks (search_job_id, block_start, block_end, estimated_duration_s)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(job_id)
            .bind(start)
            .bind(end)
            .bind(estimated_duration_s)
            .execute(&mut *tx)
            .await?;
            start = end;
        }
        tx.commit().await?;
        Ok(job_id)
    }

    /// List all search jobs, most recent first.
    pub async fn get_search_jobs(&self) -> Result<Vec<SearchJobRow>> {
        let rows = sqlx::query_as::<_, SearchJobRow>(
            "SELECT id, search_type, params, status, error,
                    created_at, started_at, stopped_at,
                    range_start, range_end, block_size,
                    total_tested, total_found
             FROM search_jobs ORDER BY id DESC",
        )
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    /// List recent or active search jobs, capped by limit.
    ///
    /// Includes any running/paused/pending jobs plus those stopped within
    /// the last `hours` hours.
    pub async fn get_recent_search_jobs(
        &self,
        hours: i64,
        limit: i64,
    ) -> Result<Vec<SearchJobRow>> {
        let rows = sqlx::query_as::<_, SearchJobRow>(
            "SELECT id, search_type, params, status, error,
                    created_at, started_at, stopped_at,
                    range_start, range_end, block_size,
                    total_tested, total_found
             FROM search_jobs
             WHERE status IN ('running','paused','pending')
                OR stopped_at > NOW() - ($1 || ' hours')::interval
             ORDER BY id DESC
             LIMIT $2",
        )
        .bind(hours.to_string())
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    /// Get a single search job by ID.
    pub async fn get_search_job(&self, job_id: i64) -> Result<Option<SearchJobRow>> {
        let row = sqlx::query_as::<_, SearchJobRow>(
            "SELECT id, search_type, params, status, error,
                    created_at, started_at, stopped_at,
                    range_start, range_end, block_size,
                    total_tested, total_found
             FROM search_jobs WHERE id = $1",
        )
        .bind(job_id)
        .fetch_optional(&self.read_pool)
        .await?;
        Ok(row)
    }

    /// Update a search job's status. Sets `stopped_at` for terminal states.
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

    /// Atomically claim an available work block using `FOR UPDATE SKIP LOCKED`.
    ///
    /// The PostgreSQL function `claim_work_block(job_id, worker_id)` finds the
    /// lowest-numbered available block, marks it as 'claimed', and returns it.
    /// Returns `None` if no blocks are available.
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

    /// Complete a work block with default 1 core.
    pub async fn complete_work_block(&self, block_id: i64, tested: i64, found: i64) -> Result<()> {
        self.complete_work_block_with_cores(block_id, tested, found, 1)
            .await
    }

    /// Complete a work block, recording duration (from claimed_at) and cores used.
    ///
    /// The PostgreSQL function `complete_work_block_with_duration` computes
    /// `duration_secs` from `claimed_at` and stores it alongside the core count
    /// for cost calibration.
    pub async fn complete_work_block_with_cores(
        &self,
        block_id: i64,
        tested: i64,
        found: i64,
        cores_used: i32,
    ) -> Result<()> {
        sqlx::query("SELECT complete_work_block_with_duration($1, $2, $3, $4)")
            .bind(block_id)
            .bind(tested)
            .bind(found)
            .bind(cores_used)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Mark a work block as failed (e.g., worker crashed during processing).
    pub async fn fail_work_block(&self, block_id: i64) -> Result<()> {
        sqlx::query("UPDATE work_blocks SET status = 'failed' WHERE id = $1")
            .bind(block_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Reclaim blocks that have been claimed for longer than `stale_seconds`.
    ///
    /// The PostgreSQL function `reclaim_stale_blocks` resets claimed blocks back
    /// to 'available' status, allowing other workers to pick them up. This handles
    /// the case where a worker crashes without completing its block.
    pub async fn reclaim_stale_blocks(&self, stale_seconds: i32) -> Result<i32> {
        let count: i32 = sqlx::query_scalar("SELECT reclaim_stale_blocks($1)")
            .bind(stale_seconds)
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// Get aggregated block status counts for a search job.
    ///
    /// Returns counts of available/claimed/completed/failed blocks plus
    /// totals for tested candidates and found primes across completed blocks.
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
        .fetch_one(&self.read_pool)
        .await?;
        Ok(row)
    }

    /// Aggregate block counts across all search jobs (for Prometheus metrics).
    ///
    /// Returns a `JobBlockSummary` with global available/claimed counts. Used by
    /// the dashboard background loop to update `darkreach_work_blocks_*` gauges.
    pub async fn get_all_block_summary(&self) -> Result<JobBlockSummary> {
        let row = sqlx::query_as::<_, JobBlockSummary>(
            "SELECT
                COUNT(*) FILTER (WHERE status = 'available') AS available,
                COUNT(*) FILTER (WHERE status = 'claimed') AS claimed,
                COUNT(*) FILTER (WHERE status = 'completed') AS completed,
                COUNT(*) FILTER (WHERE status = 'failed') AS failed,
                COALESCE(SUM(tested) FILTER (WHERE status = 'completed'), 0)::BIGINT AS total_tested,
                COALESCE(SUM(found) FILTER (WHERE status = 'completed'), 0)::BIGINT AS total_found
             FROM work_blocks",
        )
        .fetch_one(&self.read_pool)
        .await?;
        Ok(row)
    }

    /// Get total core-hours for all completed work blocks of a search job.
    ///
    /// Delegates to the PostgreSQL function `get_job_core_hours` which computes
    /// `SUM(duration_secs * cores_used) / 3600.0` across completed blocks.
    pub async fn get_job_core_hours(&self, job_id: i64) -> Result<f64> {
        let hours: f64 = sqlx::query_scalar("SELECT get_job_core_hours($1)")
            .bind(job_id)
            .fetch_one(&self.read_pool)
            .await?;
        Ok(hours)
    }

    /// Get details of a completed work block for verification queue.
    pub async fn get_work_block_details(
        &self,
        block_id: i64,
    ) -> Result<Option<WorkBlockDetails>> {
        let row = sqlx::query_as::<_, WorkBlockDetails>(
            "SELECT id AS block_id, block_start, block_end,
                    COALESCE(tested, 0) AS tested,
                    COALESCE(found, 0) AS found,
                    COALESCE(claimed_by, '') AS claimed_by
             FROM work_blocks WHERE id = $1",
        )
        .bind(block_id)
        .fetch_optional(&self.read_pool)
        .await?;
        Ok(row)
    }

    /// Link a search job to a project (set the FK on search_jobs).
    pub async fn link_search_job_to_project(&self, job_id: i64, project_id: i64) -> Result<()> {
        sqlx::query("UPDATE search_jobs SET project_id = $1 WHERE id = $2")
            .bind(project_id)
            .bind(job_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
