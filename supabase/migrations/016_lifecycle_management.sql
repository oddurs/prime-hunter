-- Lifecycle management: prevent workers from claiming blocks on paused/cancelled jobs,
-- and add 'paused' status to project phases.

-- Replace claim_work_block to JOIN on search_jobs and only claim from running jobs.
-- This is the key fix: workers cannot claim blocks from paused/cancelled/completed jobs.
CREATE OR REPLACE FUNCTION claim_work_block(p_job_id BIGINT, p_worker_id TEXT)
RETURNS TABLE(block_id BIGINT, block_start BIGINT, block_end BIGINT) LANGUAGE sql AS $$
    UPDATE work_blocks
    SET status = 'claimed', claimed_by = p_worker_id, claimed_at = NOW()
    WHERE id = (
        SELECT wb.id FROM work_blocks wb
        JOIN search_jobs sj ON sj.id = wb.search_job_id
        WHERE wb.search_job_id = p_job_id
          AND wb.status = 'available'
          AND sj.status = 'running'
        ORDER BY wb.block_start LIMIT 1
        FOR UPDATE OF wb SKIP LOCKED
    )
    RETURNING id AS block_id, work_blocks.block_start, work_blocks.block_end;
$$;

-- Add 'paused' to project_phases status CHECK constraint.
-- Drop and recreate since ALTER CHECK is not supported.
ALTER TABLE project_phases DROP CONSTRAINT IF EXISTS project_phases_status_check;
ALTER TABLE project_phases ADD CONSTRAINT project_phases_status_check
    CHECK (status IN ('pending', 'active', 'paused', 'completed', 'skipped', 'failed'));
