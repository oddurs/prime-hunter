-- 028_network_scaling.sql
--
-- Network scaling & correctness: block progress tracking, verification queue,
-- node reliability scoring, batch claiming, and dynamic stale reclaim.
-- Prerequisite for untrusted operator scaling to 50+ nodes.

BEGIN;

-- ============================================================
-- 1. New columns on work_blocks — progress tracking & duration
-- ============================================================

ALTER TABLE work_blocks
    ADD COLUMN IF NOT EXISTS block_checkpoint JSONB,
    ADD COLUMN IF NOT EXISTS progress_tested BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS progress_found BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS estimated_duration_s INTEGER;

-- ============================================================
-- 2. node_block_results — per-block result history (30-day rolling)
-- ============================================================

CREATE TABLE node_block_results (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    worker_id TEXT NOT NULL,
    block_id BIGINT NOT NULL,
    valid BOOLEAN NOT NULL,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_node_block_results_worker_time
    ON node_block_results (worker_id, completed_at);

CREATE INDEX idx_node_block_results_recent
    ON node_block_results (completed_at DESC);

ALTER TABLE node_block_results ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_node_block_results" ON node_block_results
    FOR SELECT USING (true);

-- ============================================================
-- 3. verification_queue — blocks requiring independent re-check
-- ============================================================

CREATE TABLE verification_queue (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    original_block_id BIGINT NOT NULL REFERENCES work_blocks(id),
    search_job_id BIGINT NOT NULL REFERENCES search_jobs(id),
    block_start BIGINT NOT NULL,
    block_end BIGINT NOT NULL,
    original_tested BIGINT NOT NULL,
    original_found BIGINT NOT NULL,
    original_worker TEXT NOT NULL,
    original_volunteer_id UUID,
    verification_worker TEXT,
    verification_tested BIGINT,
    verification_found BIGINT,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'claimed', 'matched', 'conflict', 'resolved')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    resolution TEXT
);

CREATE INDEX idx_verification_queue_status
    ON verification_queue (status)
    WHERE status IN ('pending', 'claimed');

CREATE INDEX idx_verification_queue_original_block
    ON verification_queue (original_block_id);

ALTER TABLE verification_queue ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_verification_queue" ON verification_queue
    FOR SELECT USING (true);

-- ============================================================
-- 4. benchmark_score on operator_nodes (volunteer_workers)
-- ============================================================

ALTER TABLE operator_nodes
    ADD COLUMN IF NOT EXISTS benchmark_score REAL;

-- ============================================================
-- 5. Replace reclaim_stale_blocks — respect estimated_duration_s
-- ============================================================

DROP FUNCTION IF EXISTS reclaim_stale_blocks(INTEGER);
CREATE OR REPLACE FUNCTION reclaim_stale_blocks(p_stale_seconds INTEGER)
RETURNS INTEGER
LANGUAGE plpgsql AS $$
DECLARE
    reclaimed INTEGER;
BEGIN
    WITH stale AS (
        SELECT id
        FROM work_blocks
        WHERE status = 'claimed'
          AND volunteer_id IS NULL
          AND claimed_at < NOW() - (
              GREATEST(p_stale_seconds, COALESCE(estimated_duration_s * 3, p_stale_seconds))
              || ' seconds'
          )::interval
    )
    UPDATE work_blocks wb
    SET status = 'available',
        claimed_by = NULL,
        claimed_at = NULL
        -- block_checkpoint is preserved for resume
    FROM stale
    WHERE wb.id = stale.id;

    GET DIAGNOSTICS reclaimed = ROW_COUNT;
    RETURN reclaimed;
END;
$$;

-- ============================================================
-- 6. claim_work_blocks — batch claim up to N blocks atomically
-- ============================================================

CREATE OR REPLACE FUNCTION claim_work_blocks(
    p_job_id BIGINT,
    p_worker_id TEXT,
    p_count INTEGER
)
RETURNS TABLE(block_id BIGINT, block_start BIGINT, block_end BIGINT, block_checkpoint JSONB)
LANGUAGE plpgsql AS $$
BEGIN
    RETURN QUERY
    WITH claimed AS (
        SELECT wb.id
        FROM work_blocks wb
        WHERE wb.search_job_id = p_job_id
          AND wb.status = 'available'
        ORDER BY wb.id
        FOR UPDATE SKIP LOCKED
        LIMIT p_count
    )
    UPDATE work_blocks wb
    SET status = 'claimed',
        claimed_by = p_worker_id,
        claimed_at = NOW()
    FROM claimed
    WHERE wb.id = claimed.id
    RETURNING wb.id AS block_id, wb.block_start, wb.block_end, wb.block_checkpoint;
END;
$$;

-- ============================================================
-- 7. update_block_progress — live progress on heartbeat
-- ============================================================

CREATE OR REPLACE FUNCTION update_block_progress(
    p_block_id BIGINT,
    p_tested BIGINT,
    p_found BIGINT,
    p_checkpoint JSONB DEFAULT NULL
)
RETURNS VOID
LANGUAGE plpgsql AS $$
BEGIN
    UPDATE work_blocks
    SET progress_tested = p_tested,
        progress_found = p_found,
        block_checkpoint = COALESCE(p_checkpoint, block_checkpoint)
    WHERE id = p_block_id
      AND status = 'claimed';
END;
$$;

-- ============================================================
-- 8. node_reliability_30d — compute rolling reliability score
-- ============================================================

CREATE OR REPLACE FUNCTION node_reliability_30d(p_worker_id TEXT)
RETURNS DOUBLE PRECISION
LANGUAGE sql STABLE AS $$
    SELECT CASE
        WHEN COUNT(*) = 0 THEN 1.0
        ELSE COUNT(*) FILTER (WHERE valid)::DOUBLE PRECISION / COUNT(*)::DOUBLE PRECISION
    END
    FROM node_block_results
    WHERE worker_id = p_worker_id
      AND completed_at > NOW() - INTERVAL '30 days';
$$;

COMMIT;
