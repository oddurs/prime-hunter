-- Add duration and core tracking to work_blocks for cost computation.
-- duration_secs: wall-clock time the worker spent on this block.
-- cores_used: number of cores the worker used (from worker heartbeat).
ALTER TABLE work_blocks ADD COLUMN IF NOT EXISTS duration_secs DOUBLE PRECISION;
ALTER TABLE work_blocks ADD COLUMN IF NOT EXISTS cores_used INTEGER;

-- Update complete_work_block to compute duration from claimed_at → NOW().
CREATE OR REPLACE FUNCTION complete_work_block_with_duration(
    p_block_id BIGINT,
    p_tested BIGINT,
    p_found BIGINT,
    p_cores_used INTEGER DEFAULT 1
) RETURNS VOID LANGUAGE sql AS $$
    UPDATE work_blocks
    SET status = 'completed',
        completed_at = NOW(),
        tested = p_tested,
        found = p_found,
        cores_used = p_cores_used,
        duration_secs = EXTRACT(EPOCH FROM (NOW() - COALESCE(claimed_at, NOW())))
    WHERE id = p_block_id;
$$;

-- Query: total core-hours for all completed blocks of a search job.
-- core_hours = SUM(duration_secs * cores_used) / 3600
CREATE OR REPLACE FUNCTION get_job_core_hours(p_job_id BIGINT)
RETURNS DOUBLE PRECISION LANGUAGE sql STABLE AS $$
    SELECT COALESCE(
        SUM(duration_secs * COALESCE(cores_used, 1)) / 3600.0,
        0.0
    )
    FROM work_blocks
    WHERE search_job_id = p_job_id AND status = 'completed';
$$;
