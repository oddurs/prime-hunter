//! Trust, reliability, and verification operations for network scaling.
//!
//! Consolidates node reliability scoring (30-day rolling window), the
//! verification queue (independent re-check of operator blocks), batch
//! block claiming, and live block progress reporting.
//!
//! ## Verification Flow
//!
//! 1. Operator completes a work block.
//! 2. Coordinator checks trust level + required quorum.
//! 3. If quorum ≥ 2: block is queued for independent verification.
//! 4. A different node claims the verification block and re-runs the search.
//! 5. Results are compared: matched → mark verified; conflict → escalate.
//!
//! ## Reliability Scoring
//!
//! Each completed block is recorded in `node_block_results`. The 30-day
//! rolling reliability score (valid / total) feeds into the effective
//! trust level calculation.

use super::Database;
use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

/// Work block with optional checkpoint for resume-from-checkpoint support.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkBlockWithCheckpoint {
    pub block_id: i64,
    pub block_start: i64,
    pub block_end: i64,
    pub block_checkpoint: Option<Value>,
}

/// A verification block assigned to a verifier node.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct VerificationBlock {
    pub verification_id: i64,
    pub search_job_id: i64,
    pub block_start: i64,
    pub block_end: i64,
    pub search_type: String,
    pub params: Value,
}

/// Result of comparing original and verifier results.
#[derive(Debug, Clone)]
pub enum VerificationOutcome {
    /// Original and verifier agree on tested/found counts.
    Matched,
    /// Results disagree — needs investigation or third check.
    Conflict {
        original_found: i64,
        verifier_found: i64,
    },
}

/// Rolling 30-day reliability data for a node.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct NodeReliability {
    pub worker_id: String,
    pub total_blocks: i64,
    pub valid_blocks: i64,
    pub reliability_score: f64,
}

impl Database {
    // ── Node Reliability ──────────────────────────────────────────

    /// Record a block completion result for reliability tracking.
    pub async fn record_block_result(
        &self,
        worker_id: &str,
        block_id: i64,
        valid: bool,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO node_block_results (worker_id, block_id, valid)
             VALUES ($1, $2, $3)",
        )
        .bind(worker_id)
        .bind(block_id)
        .bind(valid)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get the 30-day rolling reliability score for a node.
    /// Returns 1.0 for unknown nodes (benefit of the doubt).
    pub async fn get_node_reliability(&self, worker_id: &str) -> Result<f64> {
        let score: f64 = sqlx::query_scalar("SELECT node_reliability_30d($1)")
            .bind(worker_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(score)
    }

    /// Compute effective trust level combining operator trust + node reliability.
    ///
    /// If node reliability is below 0.90, cap the effective trust at level 2.
    /// If below 0.80, cap at level 1.
    pub async fn effective_trust_level(
        &self,
        volunteer_id: uuid::Uuid,
        worker_id: &str,
    ) -> Result<i16> {
        let trust = self.get_operator_trust(volunteer_id).await?;
        let base_level = trust.map(|t| t.trust_level).unwrap_or(1);

        let reliability = self.get_node_reliability(worker_id).await?;

        let effective = if reliability < 0.80 {
            base_level.min(1)
        } else if reliability < 0.90 {
            base_level.min(2)
        } else {
            base_level
        };

        Ok(effective)
    }

    // ── Verification Pipeline ─────────────────────────────────────

    /// Queue a completed block for independent verification.
    #[allow(clippy::too_many_arguments)]
    pub async fn queue_verification(
        &self,
        block_id: i64,
        job_id: i64,
        block_start: i64,
        block_end: i64,
        tested: i64,
        found: i64,
        worker_id: &str,
        volunteer_id: Option<uuid::Uuid>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar::<_, i64>(
            "INSERT INTO verification_queue
                (original_block_id, search_job_id, block_start, block_end,
                 original_tested, original_found, original_worker, original_volunteer_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id",
        )
        .bind(block_id)
        .bind(job_id)
        .bind(block_start)
        .bind(block_end)
        .bind(tested)
        .bind(found)
        .bind(worker_id)
        .bind(volunteer_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    /// Claim the next pending verification block, excluding same worker.
    ///
    /// Uses a two-step approach: CTE UPDATE to claim atomically, then JOIN
    /// to fetch search job details (RETURNING can't reference joined tables).
    pub async fn claim_verification_block(
        &self,
        verifier_worker_id: &str,
    ) -> Result<Option<VerificationBlock>> {
        let claimed_id: Option<i64> = sqlx::query_scalar(
            "WITH claimed AS (
                SELECT vq.id
                FROM verification_queue vq
                WHERE vq.status = 'pending'
                  AND vq.original_worker <> $1
                ORDER BY vq.id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
            )
            UPDATE verification_queue vq
            SET status = 'claimed',
                verification_worker = $1
            FROM claimed
            WHERE vq.id = claimed.id
            RETURNING vq.id",
        )
        .bind(verifier_worker_id)
        .fetch_optional(&self.pool)
        .await?;

        match claimed_id {
            Some(id) => {
                let block = sqlx::query_as::<_, VerificationBlock>(
                    "SELECT vq.id AS verification_id, vq.search_job_id,
                            vq.block_start, vq.block_end,
                            sj.search_type, sj.params
                     FROM verification_queue vq
                     JOIN search_jobs sj ON sj.id = vq.search_job_id
                     WHERE vq.id = $1",
                )
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
                Ok(block)
            }
            None => Ok(None),
        }
    }

    /// Submit verification results and compare with original.
    pub async fn submit_verification_result(
        &self,
        verification_id: i64,
        tested: i64,
        found: i64,
        verifier_worker_id: &str,
    ) -> Result<VerificationOutcome> {
        // Fetch original results
        let (_original_tested, original_found, original_worker, original_volunteer_id): (
            i64,
            i64,
            String,
            Option<uuid::Uuid>,
        ) = sqlx::query_as(
            "SELECT original_tested, original_found, original_worker, original_volunteer_id
             FROM verification_queue WHERE id = $1",
        )
        .bind(verification_id)
        .fetch_one(&self.pool)
        .await?;

        let matches = found == original_found;

        let status = if matches { "matched" } else { "conflict" };

        sqlx::query(
            "UPDATE verification_queue
             SET status = $2,
                 verification_worker = $3,
                 verification_tested = $4,
                 verification_found = $5,
                 completed_at = NOW()
             WHERE id = $1",
        )
        .bind(verification_id)
        .bind(status)
        .bind(verifier_worker_id)
        .bind(tested)
        .bind(found)
        .execute(&self.pool)
        .await?;

        if matches {
            // Record valid result for both workers
            self.record_block_result(&original_worker, verification_id, true)
                .await?;
            self.record_block_result(verifier_worker_id, verification_id, true)
                .await?;

            // Advance trust for original operator
            if let Some(vol_id) = original_volunteer_id {
                self.record_valid_result(vol_id).await?;
            }

            // Mark the original block as verified
            let block_id: i64 = sqlx::query_scalar(
                "SELECT original_block_id FROM verification_queue WHERE id = $1",
            )
            .bind(verification_id)
            .fetch_one(&self.pool)
            .await?;
            self.mark_block_verified(block_id as i32).await?;

            Ok(VerificationOutcome::Matched)
        } else {
            // Record invalid for original, valid for verifier (tentatively)
            self.record_block_result(&original_worker, verification_id, false)
                .await?;

            // Penalize trust for original operator
            if let Some(vol_id) = original_volunteer_id {
                self.record_invalid_result(vol_id).await?;
            }

            Ok(VerificationOutcome::Conflict {
                original_found,
                verifier_found: found,
            })
        }
    }

    // ── Block Progress ────────────────────────────────────────────

    /// Update live block progress (called from heartbeat).
    pub async fn update_block_progress(
        &self,
        block_id: i64,
        tested: i64,
        found: i64,
        checkpoint: Option<&Value>,
    ) -> Result<()> {
        sqlx::query("SELECT update_block_progress($1, $2, $3, $4)")
            .bind(block_id)
            .bind(tested)
            .bind(found)
            .bind(checkpoint)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Batch claim up to `count` work blocks atomically.
    pub async fn claim_work_blocks(
        &self,
        job_id: i64,
        worker_id: &str,
        count: i32,
    ) -> Result<Vec<WorkBlockWithCheckpoint>> {
        let rows = sqlx::query_as::<_, WorkBlockWithCheckpoint>(
            "SELECT block_id, block_start, block_end, block_checkpoint
             FROM claim_work_blocks($1, $2, $3)",
        )
        .bind(job_id)
        .bind(worker_id)
        .bind(count)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Check if a verification entry already exists for a block.
    pub async fn has_pending_verification(&self, block_id: i64) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM verification_queue
             WHERE original_block_id = $1
               AND status IN ('pending', 'claimed')",
        )
        .bind(block_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_outcome_debug() {
        let matched = VerificationOutcome::Matched;
        assert!(format!("{:?}", matched).contains("Matched"));

        let conflict = VerificationOutcome::Conflict {
            original_found: 3,
            verifier_found: 5,
        };
        let debug_str = format!("{:?}", conflict);
        assert!(debug_str.contains("Conflict"));
        assert!(debug_str.contains("3"));
        assert!(debug_str.contains("5"));
    }

    #[test]
    fn work_block_with_checkpoint_fields() {
        // Ensure the type derives Debug and Clone
        let block = WorkBlockWithCheckpoint {
            block_id: 1,
            block_start: 100,
            block_end: 200,
            block_checkpoint: Some(serde_json::json!({"last_tested": 150})),
        };
        let cloned = block.clone();
        assert_eq!(cloned.block_id, 1);
        assert_eq!(cloned.block_start, 100);
        assert!(cloned.block_checkpoint.is_some());
    }

    #[test]
    fn node_reliability_serialize() {
        let nr = NodeReliability {
            worker_id: "test-worker".to_string(),
            total_blocks: 100,
            valid_blocks: 95,
            reliability_score: 0.95,
        };
        let json = serde_json::to_value(&nr).unwrap();
        assert_eq!(json["reliability_score"], 0.95);
    }
}
