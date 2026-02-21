//! Operator account management — registration, trust scoring, credit ledger.
//!
//! Handles CRUD operations for operator accounts and their worker machines,
//! trust level progression (adaptive replication), and credit tracking for
//! the public operator computing platform.

use super::Database;
use anyhow::Result;
use serde::Serialize;

/// Operator account row from the `operators` table.
#[derive(Serialize, sqlx::FromRow)]
pub struct OperatorRow {
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

/// Trust record for an operator (adaptive replication).
#[derive(Serialize, sqlx::FromRow)]
pub struct OperatorTrustRow {
    pub volunteer_id: uuid::Uuid,
    pub consecutive_valid: i32,
    pub total_valid: i32,
    pub total_invalid: i32,
    pub trust_level: i16,
}

/// Leaderboard entry from the `operator_leaderboard` view.
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

    /// Register a new operator account. Returns the generated API key and username.
    pub async fn register_operator(&self, username: &str, email: &str) -> Result<OperatorRow> {
        let row = sqlx::query_as::<_, OperatorRow>(
            "INSERT INTO operators (username, email)
             VALUES ($1, $2)
             RETURNING *",
        )
        .bind(username)
        .bind(email)
        .fetch_one(&self.pool)
        .await?;

        // Initialize trust record
        sqlx::query(
            "INSERT INTO operator_trust (volunteer_id) VALUES ($1)
             ON CONFLICT DO NOTHING",
        )
        .bind(row.id)
        .execute(&self.pool)
        .await?;

        Ok(row)
    }

    /// Look up an operator by API key (used for authentication).
    pub async fn get_operator_by_api_key(&self, api_key: &str) -> Result<Option<OperatorRow>> {
        let row = sqlx::query_as::<_, OperatorRow>("SELECT * FROM operators WHERE api_key = $1")
            .bind(api_key)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    /// Update operator last_seen timestamp.
    pub async fn touch_operator(&self, volunteer_id: uuid::Uuid) -> Result<()> {
        sqlx::query("UPDATE operators SET last_seen = NOW() WHERE id = $1")
            .bind(volunteer_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Worker Machines ───────────────────────────────────────────

    /// Register an operator's worker machine.
    pub async fn register_operator_node(
        &self,
        volunteer_id: uuid::Uuid,
        worker_id: &str,
        hostname: &str,
        cores: i32,
        cpu_model: &str,
        os: Option<&str>,
        arch: Option<&str>,
        ram_gb: Option<i32>,
        has_gpu: Option<bool>,
        gpu_model: Option<&str>,
        gpu_vram_gb: Option<i32>,
        worker_version: Option<&str>,
        update_channel: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO operator_nodes (
               volunteer_id, worker_id, hostname, cores, cpu_model,
               os, arch, ram_gb, has_gpu, gpu_model, gpu_vram_gb,
               worker_version, update_channel
             )
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
             ON CONFLICT (worker_id) DO UPDATE SET
               hostname = EXCLUDED.hostname,
               cores = EXCLUDED.cores,
               cpu_model = EXCLUDED.cpu_model,
               os = EXCLUDED.os,
               arch = EXCLUDED.arch,
               ram_gb = EXCLUDED.ram_gb,
               has_gpu = EXCLUDED.has_gpu,
               gpu_model = EXCLUDED.gpu_model,
               gpu_vram_gb = EXCLUDED.gpu_vram_gb,
               worker_version = EXCLUDED.worker_version,
               update_channel = EXCLUDED.update_channel,
               last_heartbeat = NOW()",
        )
        .bind(volunteer_id)
        .bind(worker_id)
        .bind(hostname)
        .bind(cores)
        .bind(cpu_model)
        .bind(os)
        .bind(arch)
        .bind(ram_gb)
        .bind(has_gpu)
        .bind(gpu_model)
        .bind(gpu_vram_gb)
        .bind(worker_version)
        .bind(update_channel)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update heartbeat timestamp for an operator node.
    pub async fn operator_node_heartbeat(&self, worker_id: &str) -> Result<()> {
        sqlx::query("UPDATE operator_nodes SET last_heartbeat = NOW() WHERE worker_id = $1")
            .bind(worker_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get all worker nodes for a specific operator.
    pub async fn get_operator_nodes(
        &self,
        volunteer_id: uuid::Uuid,
    ) -> Result<Vec<OperatorNodeRow>> {
        let rows = sqlx::query_as::<_, OperatorNodeRow>(
            "SELECT worker_id, hostname, cores, cpu_model, os, arch,
                    ram_gb, has_gpu, gpu_model, worker_version,
                    registered_at, last_heartbeat
             FROM operator_nodes
             WHERE volunteer_id = $1
             ORDER BY last_heartbeat DESC NULLS LAST",
        )
        .bind(volunteer_id)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    /// Get operator account by ID.
    pub async fn get_operator_by_id(&self, id: uuid::Uuid) -> Result<Option<OperatorRow>> {
        let row = sqlx::query_as::<_, OperatorRow>("SELECT * FROM operators WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    /// Rotate an operator's API key and return the new key.
    pub async fn rotate_operator_api_key(&self, volunteer_id: uuid::Uuid) -> Result<String> {
        let new_key: String = sqlx::query_scalar(
            "UPDATE operators SET api_key = encode(gen_random_bytes(32), 'hex')
             WHERE id = $1
             RETURNING api_key",
        )
        .bind(volunteer_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(new_key)
    }

    // ── Work Assignment ───────────────────────────────────────────

    /// Claim a work block for a volunteer. Picks the first available block
    /// that matches the volunteer's hardware capabilities and assigns it.
    /// Returns None if no blocks are available.
    pub async fn claim_operator_block(
        &self,
        volunteer_id: uuid::Uuid,
        caps: &WorkerCapabilities,
    ) -> Result<Option<OperatorWorkBlock>> {
        let row = sqlx::query_as::<_, OperatorWorkBlock>(
            "UPDATE work_blocks SET
               status = 'claimed',
               claimed_by = $1::text,
               volunteer_id = $2,
               claimed_at = NOW()
             WHERE id = (
               SELECT wb.id
               FROM work_blocks wb
               JOIN search_jobs sj ON sj.id = wb.search_job_id
               WHERE wb.status = 'available'
                 AND (
                   NOT (sj.params ? 'min_cores')
                   OR (
                     jsonb_typeof(sj.params->'min_cores') = 'number'
                     AND (sj.params->>'min_cores')::int <= $3
                   )
                 )
                 AND (
                   NOT (sj.params ? 'min_ram_gb')
                   OR (
                     jsonb_typeof(sj.params->'min_ram_gb') = 'number'
                     AND (sj.params->>'min_ram_gb')::int <= $4
                   )
                 )
                 AND (
                   NOT (sj.params ? 'requires_gpu')
                   OR lower(sj.params->>'requires_gpu') <> 'true'
                   OR $5 = TRUE
                 )
                 AND (
                   NOT (sj.params ? 'required_os')
                   OR ($6 IS NOT NULL AND lower(sj.params->>'required_os') = lower($6))
                 )
                 AND (
                   NOT (sj.params ? 'required_arch')
                   OR ($7 IS NOT NULL AND lower(sj.params->>'required_arch') = lower($7))
                 )
               ORDER BY wb.id
               FOR UPDATE SKIP LOCKED
               LIMIT 1
             )
             RETURNING id AS block_id, search_job_id, block_start, block_end",
        )
        .bind(volunteer_id.to_string())
        .bind(volunteer_id)
        .bind(caps.cores)
        .bind(caps.ram_gb)
        .bind(caps.has_gpu)
        .bind(caps.os.as_deref())
        .bind(caps.arch.as_deref())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(ref block) = row {
            // Look up the search job to get search_type and params
            let job = sqlx::query_as::<_, OperatorJobInfo>(
                "SELECT search_type, params FROM search_jobs WHERE id = $1",
            )
            .bind(block.search_job_id)
            .fetch_optional(&self.pool)
            .await?;

            if let Some(job) = job {
                return Ok(Some(OperatorWorkBlock {
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
    /// Complete a work block and return its processing duration and search type
    /// for histogram recording.
    pub async fn submit_operator_result(
        &self,
        block_id: i32,
        tested: i64,
        found: i64,
    ) -> Result<Option<(f64, String)>> {
        let row: Option<(f64, String)> = sqlx::query_as(
            "UPDATE work_blocks SET
               status = 'completed',
               tested = $2,
               found = $3,
               completed_at = NOW()
             WHERE id = $1
             RETURNING
               EXTRACT(EPOCH FROM (NOW() - COALESCE(claimed_at, created_at)))::float8,
               (SELECT search_type FROM search_jobs WHERE id = search_job_id)",
        )
        .bind(block_id)
        .bind(tested)
        .bind(found)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    // ── Trust & Credits ───────────────────────────────────────────

    /// Get the trust record for an operator.
    pub async fn get_operator_trust(
        &self,
        volunteer_id: uuid::Uuid,
    ) -> Result<Option<OperatorTrustRow>> {
        let row = sqlx::query_as::<_, OperatorTrustRow>(
            "SELECT * FROM operator_trust WHERE volunteer_id = $1",
        )
        .bind(volunteer_id)
        .fetch_optional(&self.read_pool)
        .await?;
        Ok(row)
    }

    /// Record a valid result and advance trust level if thresholds met.
    /// Trust levels: 1 (new) → 2 (proven, 10+ valid) → 3 (trusted, 100+ valid)
    ///                      → 4 (core, 500+ valid).
    pub async fn record_valid_result(&self, volunteer_id: uuid::Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE operator_trust SET
               consecutive_valid = consecutive_valid + 1,
               total_valid = total_valid + 1,
               trust_level = CASE
                 WHEN total_valid + 1 >= 500 THEN 4
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

    /// Record an invalid result: reset consecutive_valid, set trust to 0 (untrusted).
    pub async fn record_invalid_result(&self, volunteer_id: uuid::Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE operator_trust SET
               consecutive_valid = 0,
               total_invalid = total_invalid + 1,
               trust_level = 0
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
            "INSERT INTO operator_credits (volunteer_id, block_id, credit, reason)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(volunteer_id)
        .bind(block_id)
        .bind(credit)
        .bind(reason)
        .execute(&self.pool)
        .await?;

        sqlx::query("UPDATE operators SET credit = credit + $2 WHERE id = $1")
            .bind(volunteer_id)
            .bind(credit)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Increment primes_found for a volunteer.
    pub async fn increment_operator_primes(&self, volunteer_id: uuid::Uuid) -> Result<()> {
        sqlx::query("UPDATE operators SET primes_found = primes_found + 1 WHERE id = $1")
            .bind(volunteer_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Stats & Leaderboard ───────────────────────────────────────

    /// Get personal stats for a volunteer, including rank by credit.
    pub async fn get_operator_stats(
        &self,
        volunteer_id: uuid::Uuid,
    ) -> Result<Option<OperatorStatsRow>> {
        let row = sqlx::query_as::<_, OperatorStatsRow>(
            "SELECT
               v.username,
               v.credit,
               v.primes_found,
               COALESCE(vt.trust_level, 1) AS trust_level,
               (SELECT COUNT(*) + 1 FROM operators v2 WHERE v2.credit > v.credit) AS rank
             FROM operators v
             LEFT JOIN operator_trust vt ON vt.volunteer_id = v.id
             WHERE v.id = $1",
        )
        .bind(volunteer_id)
        .fetch_optional(&self.read_pool)
        .await?;
        Ok(row)
    }

    /// Get the volunteer leaderboard (top N by credit).
    pub async fn get_operator_leaderboard(&self, limit: i64) -> Result<Vec<LeaderboardRow>> {
        let rows =
            sqlx::query_as::<_, LeaderboardRow>("SELECT * FROM volunteer_leaderboard LIMIT $1")
                .bind(limit)
                .fetch_all(&self.read_pool)
                .await?;
        Ok(rows)
    }

    // ── Quorum & Verification ─────────────────────────────────────

    /// Set the minimum quorum for a work block based on volunteer trust.
    pub async fn set_block_quorum(&self, block_id: i32, min_quorum: i16) -> Result<()> {
        sqlx::query("UPDATE work_blocks SET min_quorum = $2 WHERE id = $1")
            .bind(block_id)
            .bind(min_quorum)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Mark a work block as verified after quorum is met.
    pub async fn mark_block_verified(&self, block_id: i32) -> Result<()> {
        sqlx::query("UPDATE work_blocks SET verified = TRUE WHERE id = $1")
            .bind(block_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get unverified volunteer blocks that need verification.
    pub async fn get_unverified_operator_blocks(
        &self,
        limit: i64,
    ) -> Result<Vec<UnverifiedBlock>> {
        let rows = sqlx::query_as::<_, UnverifiedBlock>(
            "SELECT wb.id AS block_id, wb.search_job_id, wb.volunteer_id,
                    wb.min_quorum, sj.search_type
             FROM work_blocks wb
             JOIN search_jobs sj ON sj.id = wb.search_job_id
             WHERE wb.status = 'completed'
               AND wb.volunteer_id IS NOT NULL
               AND wb.verified = FALSE
             ORDER BY wb.id
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    /// Reclaim stale volunteer blocks (24-hour timeout, vs 2-min for internal).
    pub async fn reclaim_stale_operator_blocks(&self, timeout_secs: i64) -> Result<i64> {
        let result = sqlx::query(
            "UPDATE work_blocks SET
               status = 'available',
               claimed_by = NULL,
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

#[derive(Debug, Clone)]
pub struct WorkerCapabilities {
    pub cores: i32,
    pub ram_gb: i32,
    pub has_gpu: bool,
    pub os: Option<String>,
    pub arch: Option<String>,
}

/// Work block assigned to a volunteer (subset of work_blocks columns).
#[derive(Serialize, sqlx::FromRow)]
pub struct OperatorWorkBlock {
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
struct OperatorJobInfo {
    search_type: String,
    params: serde_json::Value,
}

/// Personal stats row for a volunteer.
#[derive(Serialize, sqlx::FromRow)]
pub struct OperatorStatsRow {
    pub username: String,
    pub credit: i64,
    pub primes_found: i32,
    pub trust_level: Option<i16>,
    pub rank: Option<i64>,
}

/// Operator node row (subset of operator_nodes columns).
#[derive(Serialize, sqlx::FromRow)]
pub struct OperatorNodeRow {
    pub worker_id: String,
    pub hostname: Option<String>,
    pub cores: Option<i32>,
    pub cpu_model: Option<String>,
    pub os: Option<String>,
    pub arch: Option<String>,
    pub ram_gb: Option<i32>,
    pub has_gpu: Option<bool>,
    pub gpu_model: Option<String>,
    pub worker_version: Option<String>,
    pub registered_at: chrono::DateTime<chrono::Utc>,
    pub last_heartbeat: Option<chrono::DateTime<chrono::Utc>>,
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
