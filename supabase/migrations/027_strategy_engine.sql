-- 027_strategy_engine.sql
--
-- AI Strategy Engine tables: autonomous decision-making for search form
-- selection, project creation, and fleet utilization. The engine analyzes
-- discovery data, scores search forms, and creates projects/jobs to keep
-- the node pool productive without manual intervention.

BEGIN;

-- ============================================================
-- 1. strategy_config — Singleton configuration row
-- ============================================================

CREATE TABLE strategy_config (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT false,
    max_concurrent_projects INTEGER NOT NULL DEFAULT 3,
    max_monthly_budget_usd DOUBLE PRECISION NOT NULL DEFAULT 100.0,
    max_per_project_budget_usd DOUBLE PRECISION NOT NULL DEFAULT 25.0,
    preferred_forms TEXT[] NOT NULL DEFAULT '{}',
    excluded_forms TEXT[] NOT NULL DEFAULT '{}',
    min_idle_workers_to_create INTEGER NOT NULL DEFAULT 2,
    record_proximity_threshold DOUBLE PRECISION NOT NULL DEFAULT 0.1,
    tick_interval_secs INTEGER NOT NULL DEFAULT 300,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Insert the singleton row
INSERT INTO strategy_config DEFAULT VALUES;

-- ============================================================
-- 2. strategy_decisions — Audit log of engine decisions
-- ============================================================

CREATE TABLE strategy_decisions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    decision_type TEXT NOT NULL CHECK (decision_type IN (
        'create_project', 'create_job', 'pause_job', 'verify_result', 'no_action'
    )),
    form TEXT,
    summary TEXT NOT NULL,
    reasoning TEXT NOT NULL,
    params JSONB,
    estimated_cost_usd DOUBLE PRECISION,
    action_taken TEXT NOT NULL DEFAULT 'executed' CHECK (action_taken IN (
        'executed', 'deferred', 'overridden'
    )),
    override_reason TEXT,
    project_id BIGINT REFERENCES projects(id),
    search_job_id BIGINT REFERENCES search_jobs(id),
    scores JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_strategy_decisions_created_at ON strategy_decisions(created_at DESC);
CREATE INDEX idx_strategy_decisions_form ON strategy_decisions(form);
CREATE INDEX idx_strategy_decisions_type ON strategy_decisions(decision_type);

-- ============================================================
-- 3. form_yield_rates — View for per-form yield statistics
-- ============================================================

CREATE OR REPLACE VIEW form_yield_rates AS
SELECT
    sj.search_type AS form,
    COUNT(DISTINCT sj.id) AS job_count,
    COALESCE(SUM(sj.total_tested), 0)::BIGINT AS total_tested,
    COALESCE(SUM(sj.total_found), 0)::BIGINT AS total_found,
    CASE
        WHEN COALESCE(SUM(sj.total_tested), 0) > 0
        THEN SUM(sj.total_found)::DOUBLE PRECISION / SUM(sj.total_tested)::DOUBLE PRECISION
        ELSE 0.0
    END AS yield_rate,
    COALESCE(MAX(sj.range_end), 0)::BIGINT AS max_range_searched
FROM search_jobs sj
WHERE sj.status IN ('completed', 'running')
GROUP BY sj.search_type;

-- ============================================================
-- 4. RLS policies
-- ============================================================

ALTER TABLE strategy_config ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_strategy_config" ON strategy_config FOR SELECT USING (true);

ALTER TABLE strategy_decisions ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_strategy_decisions" ON strategy_decisions FOR SELECT USING (true);

-- ============================================================
-- 5. Realtime
-- ============================================================

ALTER PUBLICATION supabase_realtime ADD TABLE strategy_decisions;

COMMIT;
