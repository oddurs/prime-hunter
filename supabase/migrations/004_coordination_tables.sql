-- Workers table: replaces in-memory fleet::Fleet
CREATE TABLE workers (
    worker_id       TEXT PRIMARY KEY,
    hostname        TEXT NOT NULL,
    cores           INTEGER NOT NULL DEFAULT 1,
    search_type     TEXT NOT NULL,
    search_params   TEXT NOT NULL DEFAULT '',
    tested          BIGINT NOT NULL DEFAULT 0,
    found           BIGINT NOT NULL DEFAULT 0,
    current         TEXT NOT NULL DEFAULT '',
    checkpoint      TEXT,
    metrics         JSONB,
    registered_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_heartbeat  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    pending_command TEXT
);
CREATE INDEX idx_workers_heartbeat ON workers (last_heartbeat);

-- Search jobs: defines a search that workers can claim blocks from
CREATE TABLE search_jobs (
    id              BIGSERIAL PRIMARY KEY,
    search_type     TEXT NOT NULL,
    params          JSONB NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending','running','paused','completed','cancelled','failed')),
    error           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at      TIMESTAMPTZ,
    stopped_at      TIMESTAMPTZ,
    range_start     BIGINT NOT NULL,
    range_end       BIGINT NOT NULL,
    block_size      BIGINT NOT NULL DEFAULT 10000,
    total_tested    BIGINT NOT NULL DEFAULT 0,
    total_found     BIGINT NOT NULL DEFAULT 0
);

-- Work blocks: individual claimable units of work
CREATE TABLE work_blocks (
    id              BIGSERIAL PRIMARY KEY,
    search_job_id   BIGINT NOT NULL REFERENCES search_jobs(id) ON DELETE CASCADE,
    block_start     BIGINT NOT NULL,
    block_end       BIGINT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'available'
                    CHECK (status IN ('available','claimed','completed','failed')),
    claimed_by      TEXT REFERENCES workers(worker_id) ON DELETE SET NULL,
    claimed_at      TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    tested          BIGINT NOT NULL DEFAULT 0,
    found           BIGINT NOT NULL DEFAULT 0,
    UNIQUE (search_job_id, block_start)
);
CREATE INDEX idx_work_blocks_claimable ON work_blocks (search_job_id, status) WHERE status = 'available';
CREATE INDEX idx_work_blocks_stale ON work_blocks (claimed_at) WHERE status = 'claimed';

-- RPC: UPSERT worker + atomically read & clear pending command
CREATE OR REPLACE FUNCTION worker_heartbeat(
    p_worker_id TEXT, p_hostname TEXT, p_cores INTEGER,
    p_search_type TEXT, p_search_params TEXT,
    p_tested BIGINT, p_found BIGINT, p_current TEXT,
    p_checkpoint TEXT, p_metrics JSONB
) RETURNS TEXT LANGUAGE plpgsql AS $$
DECLARE v_command TEXT;
BEGIN
    SELECT pending_command INTO v_command FROM workers WHERE worker_id = p_worker_id FOR UPDATE;
    INSERT INTO workers (worker_id, hostname, cores, search_type, search_params,
                         tested, found, current, checkpoint, metrics, last_heartbeat, pending_command)
    VALUES (p_worker_id, p_hostname, p_cores, p_search_type, p_search_params,
            p_tested, p_found, p_current, p_checkpoint, p_metrics, NOW(), NULL)
    ON CONFLICT (worker_id) DO UPDATE SET
        tested = EXCLUDED.tested, found = EXCLUDED.found, current = EXCLUDED.current,
        checkpoint = EXCLUDED.checkpoint, metrics = EXCLUDED.metrics,
        last_heartbeat = NOW(), pending_command = NULL;
    RETURN v_command;
END; $$;

-- RPC: Atomic block claiming with FOR UPDATE SKIP LOCKED
CREATE OR REPLACE FUNCTION claim_work_block(p_job_id BIGINT, p_worker_id TEXT)
RETURNS TABLE(block_id BIGINT, block_start BIGINT, block_end BIGINT) LANGUAGE sql AS $$
    UPDATE work_blocks
    SET status = 'claimed', claimed_by = p_worker_id, claimed_at = NOW()
    WHERE id = (
        SELECT id FROM work_blocks
        WHERE search_job_id = p_job_id AND status = 'available'
        ORDER BY block_start LIMIT 1
        FOR UPDATE SKIP LOCKED
    )
    RETURNING id AS block_id, work_blocks.block_start, work_blocks.block_end;
$$;

-- RPC: Reclaim blocks from dead workers
CREATE OR REPLACE FUNCTION reclaim_stale_blocks(p_stale_seconds INTEGER DEFAULT 120)
RETURNS INTEGER LANGUAGE sql AS $$
    WITH reclaimed AS (
        UPDATE work_blocks SET status = 'available', claimed_by = NULL, claimed_at = NULL
        WHERE status = 'claimed'
          AND claimed_at < NOW() - (p_stale_seconds || ' seconds')::interval
          AND NOT EXISTS (
              SELECT 1 FROM workers
              WHERE workers.worker_id = work_blocks.claimed_by
                AND workers.last_heartbeat > NOW() - INTERVAL '60 seconds'
          )
        RETURNING id
    ) SELECT COUNT(*)::INTEGER FROM reclaimed;
$$;

-- RLS policies
ALTER TABLE workers ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read workers" ON workers FOR SELECT USING (true);

ALTER TABLE search_jobs ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read search_jobs" ON search_jobs FOR SELECT USING (true);

ALTER TABLE work_blocks ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read work_blocks" ON work_blocks FOR SELECT USING (true);
