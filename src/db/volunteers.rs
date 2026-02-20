//! Volunteer account management — registration, trust scoring, credit ledger.
//!
//! Handles CRUD operations for volunteer accounts and their worker machines,
//! trust level progression (adaptive replication), and credit tracking for
//! the public volunteer computing platform.

use anyhow::Result;
use super::Database;
use serde::Serialize;

/// Volunteer account row from the `volunteers` table.
#[derive(Serialize, sqlx::FromRow)]
pub struct VolunteerRow {
    pub id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub api_key: String,
    pub team: Option<String>,
    pub credit: i64,
    pub primes_found: i32,
    pub joined_at: chrono::DateTime<chrono::Utc>,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
}

/// Trust record for a volunteer (adaptive replication).
#[derive(Serialize, sqlx::FromRow)]
pub struct VolunteerTrustRow {
    pub volunteer_id: uuid::Uuid,
    pub consecutive_valid: i32,
    pub total_valid: i32,
    pub total_invalid: i32,
    pub trust_level: i16,
}

/// Leaderboard entry from the `volunteer_leaderboard` view.
#[derive(Serialize, sqlx::FromRow)]
pub struct LeaderboardRow {
    pub id: uuid::Uuid,
    pub username: String,
    pub team: Option<String>,
    pub credit: i64,
    pub primes_found: i32,
    pub joined_at: chrono::DateTime<chrono::Utc>,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
    pub trust_level: Option<i16>,
    pub worker_count: Option<i64>,
}

impl Database {
    // ── Registration ──────────────────────────────────────────────

    /// Register a new volunteer account. Returns the generated API key and username.
    pub async fn register_volunteer(
        &self,
        username: &str,
        email: &str,
    ) -> Result<VolunteerRow> {
        let row = sqlx::query_as::<_, VolunteerRow>(
            "INSERT INTO volunteers (username, email)
             VALUES ($1, $2)
             RETURNING *",
        )
        .bind(username)
        .bind(email)
        .fetch_one(&self.pool)
        .await?;

        // Initialize trust record
        sqlx::query(
            "INSERT INTO volunteer_trust (volunteer_id) VALUES ($1)
             ON CONFLICT DO NOTHING",
        )
        .bind(row.id)
        .execute(&self.pool)
        .await?;

        Ok(row)
    }

    /// Look up a volunteer by API key (used for authentication).
    pub async fn get_volunteer_by_api_key(
        &self,
        api_key: &str,
    ) -> Result<Option<VolunteerRow>> {
        let row = sqlx::query_as::<_, VolunteerRow>(
            "SELECT * FROM volunteers WHERE api_key = $1",
        )
        .bind(api_key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Update volunteer last_seen timestamp.
    pub async fn touch_volunteer(&self, volunteer_id: uuid::Uuid) -> Result<()> {
        sqlx::query("UPDATE volunteers SET last_seen = NOW() WHERE id = $1")
            .bind(volunteer_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Worker Machines ───────────────────────────────────────────

    /// Register a volunteer's worker machine.
    pub async fn register_volunteer_worker(
        &self,
        volunteer_id: uuid::Uuid,
        worker_id: &str,
        hostname: &str,
        cores: i32,
        cpu_model: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO volunteer_workers (volunteer_id, worker_id, hostname, cores, cpu_model)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (worker_id) DO UPDATE SET
               hostname = EXCLUDED.hostname,
               cores = EXCLUDED.cores,
               cpu_model = EXCLUDED.cpu_model,
               last_heartbeat = NOW()",
        )
        .bind(volunteer_id)
        .bind(worker_id)
        .bind(hostname)
        .bind(cores)
        .bind(cpu_model)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update heartbeat timestamp for a volunteer worker.
    pub async fn volunteer_worker_heartbeat(
        &self,
        worker_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE volunteer_workers SET last_heartbeat = NOW() WHERE worker_id = $1",
        )
        .bind(worker_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Work Assignment ───────────────────────────────────────────

    /// Claim a work block for a volunteer. Picks the first available block
    /// that matches the volunteer's hardware capabilities and assigns it.
    /// Returns None if no blocks are available.
    pub async fn claim_volunteer_block(
        &self,
        volunteer_id: uuid::Uuid,
    ) -> Result<Option<VolunteerWorkBlock>> {
        let row = sqlx::query_as::<_, VolunteerWorkBlock>(
            "UPDATE work_blocks SET
               status = 'claimed',
               worker_id = $1::text,
               volunteer_id = $2,
               claimed_at = NOW()
             WHERE block_id = (
               SELECT block_id FROM work_blocks
               WHERE status = 'available'
               ORDER BY block_id
               FOR UPDATE SKIP LOCKED
               LIMIT 1
             )
             RETURNING block_id, search_job_id, block_start, block_end",
        )
        .bind(volunteer_id.to_string())
        .bind(volunteer_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(ref block) = row {
            // Look up the search job to get search_type and params
            let job = sqlx::query_as::<_, VolunteerJobInfo>(
                "SELECT search_type, params FROM search_jobs WHERE id = $1",
            )
            .bind(block.search_job_id)
            .fetch_optional(&self.pool)
            .await?;

            if let Some(job) = job {
                return Ok(Some(VolunteerWorkBlock {
                    block_id: block.block_id,
                    search_job_id: block.search_job_id,
                    block_start: block.block_start,
                    block_end: block.block_end,
                    search_type: Some(job.search_type),
                    params: Some(job.params),
                }));
            }
        }

        Ok(row)
    }

    // ── Result Submission ─────────────────────────────────────────

    /// Record a completed block result from a volunteer.
    pub async fn submit_volunteer_result(
        &self,
        block_id: i32,
        tested: i64,
        found: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE work_blocks SET
               status = 'completed',
               candidates_tested = $2,
               primes_found = $3,
               completed_at = NOW()
             WHERE block_id = $1",
        )
        .bind(block_id)
        .bind(tested)
        .bind(found)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Trust & Credits ───────────────────────────────────────────

    /// Get the trust record for a volunteer.
    pub async fn get_volunteer_trust(
        &self,
        volunteer_id: uuid::Uuid,
    ) -> Result<Option<VolunteerTrustRow>> {
        let row = sqlx::query_as::<_, VolunteerTrustRow>(
            "SELECT * FROM volunteer_trust WHERE volunteer_id = $1",
        )
        .bind(volunteer_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Record a valid result and advance trust level if thresholds met.
    /// Trust levels: 1 (new) → 2 (reliable, 10+ valid) → 3 (trusted, 100+ valid).
    pub async fn record_valid_result(
        &self,
        volunteer_id: uuid::Uuid,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE volunteer_trust SET
               consecutive_valid = consecutive_valid + 1,
               total_valid = total_valid + 1,
               trust_level = CASE
                 WHEN consecutive_valid + 1 >= 100 THEN 3
                 WHEN consecutive_valid + 1 >= 10 THEN 2
                 ELSE trust_level
               END
             WHERE volunteer_id = $1",
        )
        .bind(volunteer_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record an invalid result: reset consecutive_valid, bump trust down to 1.
    pub async fn record_invalid_result(
        &self,
        volunteer_id: uuid::Uuid,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE volunteer_trust SET
               consecutive_valid = 0,
               total_invalid = total_invalid + 1,
               trust_level = 1
             WHERE volunteer_id = $1",
        )
        .bind(volunteer_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Grant credit to a volunteer and log it.
    pub async fn grant_credit(
        &self,
        volunteer_id: uuid::Uuid,
        block_id: i32,
        credit: i64,
        reason: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO credit_log (volunteer_id, block_id, credit, reason)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(volunteer_id)
        .bind(block_id)
        .bind(credit)
        .bind(reason)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "UPDATE volunteers SET credit = credit + $2 WHERE id = $1",
        )
        .bind(volunteer_id)
        .bind(credit)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Increment primes_found for a volunteer.
    pub async fn increment_volunteer_primes(
        &self,
        volunteer_id: uuid::Uuid,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE volunteers SET primes_found = primes_found + 1 WHERE id = $1",
        )
        .bind(volunteer_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Stats & Leaderboard ───────────────────────────────────────

    /// Get personal stats for a volunteer, including rank by credit.
    pub async fn get_volunteer_stats(
        &self,
        volunteer_id: uuid::Uuid,
    ) -> Result<Option<VolunteerStatsRow>> {
        let row = sqlx::query_as::<_, VolunteerStatsRow>(
            "SELECT
               v.username,
               v.credit,
               v.primes_found,
               COALESCE(vt.trust_level, 1) AS trust_level,
               (SELECT COUNT(*) + 1 FROM volunteers v2 WHERE v2.credit > v.credit) AS rank
             FROM volunteers v
             LEFT JOIN volunteer_trust vt ON vt.volunteer_id = v.id
             WHERE v.id = $1",
        )
        .bind(volunteer_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get the volunteer leaderboard (top N by credit).
    pub async fn get_volunteer_leaderboard(
        &self,
        limit: i64,
    ) -> Result<Vec<LeaderboardRow>> {
        let rows = sqlx::query_as::<_, LeaderboardRow>(
            "SELECT * FROM volunteer_leaderboard LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ── Quorum & Verification ─────────────────────────────────────

    /// Set the minimum quorum for a work block based on volunteer trust.
    pub async fn set_block_quorum(
        &self,
        block_id: i32,
        min_quorum: i16,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE work_blocks SET min_quorum = $2 WHERE block_id = $1",
        )
        .bind(block_id)
        .bind(min_quorum)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Mark a work block as verified after quorum is met.
    pub async fn mark_block_verified(&self, block_id: i32) -> Result<()> {
        sqlx::query(
            "UPDATE work_blocks SET verified = TRUE WHERE block_id = $1",
        )
        .bind(block_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get unverified volunteer blocks that need verification.
    pub async fn get_unverified_volunteer_blocks(
        &self,
        limit: i64,
    ) -> Result<Vec<UnverifiedBlock>> {
        let rows = sqlx::query_as::<_, UnverifiedBlock>(
            "SELECT wb.block_id, wb.search_job_id, wb.volunteer_id,
                    wb.min_quorum, sj.search_type
             FROM work_blocks wb
             JOIN search_jobs sj ON sj.id = wb.search_job_id
             WHERE wb.status = 'completed'
               AND wb.volunteer_id IS NOT NULL
               AND wb.verified = FALSE
             ORDER BY wb.block_id
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Reclaim stale volunteer blocks (24-hour timeout, vs 2-min for internal).
    pub async fn reclaim_stale_volunteer_blocks(&self, timeout_secs: i64) -> Result<i64> {
        let result = sqlx::query(
            "UPDATE work_blocks SET
               status = 'available',
               worker_id = NULL,
               volunteer_id = NULL,
               claimed_at = NULL
             WHERE status = 'claimed'
               AND volunteer_id IS NOT NULL
               AND claimed_at < NOW() - ($1 || ' seconds')::interval",
        )
        .bind(timeout_secs.to_string())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() as i64)
    }
}

/// Work block assigned to a volunteer (subset of work_blocks columns).
#[derive(Serialize, sqlx::FromRow)]
pub struct VolunteerWorkBlock {
    pub block_id: i32,
    pub search_job_id: i64,
    pub block_start: i64,
    pub block_end: i64,
    #[sqlx(skip)]
    pub search_type: Option<String>,
    #[sqlx(skip)]
    pub params: Option<serde_json::Value>,
}

/// Search job info for populating work assignments.
#[derive(sqlx::FromRow)]
struct VolunteerJobInfo {
    search_type: String,
    params: serde_json::Value,
}

/// Personal stats row for a volunteer.
#[derive(Serialize, sqlx::FromRow)]
pub struct VolunteerStatsRow {
    pub username: String,
    pub credit: i64,
    pub primes_found: i32,
    pub trust_level: Option<i16>,
    pub rank: Option<i64>,
}

/// Unverified work block needing quorum check.
#[derive(sqlx::FromRow)]
pub struct UnverifiedBlock {
    pub block_id: i32,
    pub search_job_id: i64,
    pub volunteer_id: Option<uuid::Uuid>,
    pub min_quorum: Option<i16>,
    pub search_type: String,
}
