-- AI Engine: persistent state and decision audit trail
--
-- Supports the unified OODA decision loop that replaces independent
-- strategy_tick() and orchestrate_tick() background loops.

-- Singleton: persistent AI engine state (learned weights, cost model version)
CREATE TABLE IF NOT EXISTS ai_engine_state (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    scoring_weights JSONB NOT NULL DEFAULT '{
        "record_gap": 0.20,
        "yield_rate": 0.15,
        "cost_efficiency": 0.20,
        "opportunity_density": 0.15,
        "fleet_fit": 0.10,
        "momentum": 0.10,
        "competition": 0.10
    }'::jsonb,
    cost_model_version INTEGER NOT NULL DEFAULT 0,
    last_tick_at TIMESTAMPTZ,
    last_learn_at TIMESTAMPTZ,
    tick_count BIGINT NOT NULL DEFAULT 0,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed singleton row
INSERT INTO ai_engine_state (scoring_weights, cost_model_version)
SELECT '{
    "record_gap": 0.20,
    "yield_rate": 0.15,
    "cost_efficiency": 0.20,
    "opportunity_density": 0.15,
    "fleet_fit": 0.10,
    "momentum": 0.10,
    "competition": 0.10
}'::jsonb, 0
WHERE NOT EXISTS (SELECT 1 FROM ai_engine_state);

-- Decision audit trail (complements strategy_decisions for AI engine decisions)
CREATE TABLE IF NOT EXISTS ai_engine_decisions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tick_id BIGINT NOT NULL,
    decision_type TEXT NOT NULL,
    form TEXT,
    action TEXT NOT NULL,
    reasoning TEXT NOT NULL,
    confidence FLOAT8,
    snapshot_hash TEXT,
    params JSONB,
    outcome JSONB,
    outcome_measured_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for recent decision queries
CREATE INDEX IF NOT EXISTS idx_ai_engine_decisions_created
    ON ai_engine_decisions (created_at DESC);

-- Index for per-form decision analysis
CREATE INDEX IF NOT EXISTS idx_ai_engine_decisions_form
    ON ai_engine_decisions (form, created_at DESC)
    WHERE form IS NOT NULL;

-- Cost observations view for the LEARN phase OLS fitting.
-- Aggregates timing data from completed work blocks.
CREATE OR REPLACE VIEW cost_observations AS
SELECT
    sj.search_type AS form,
    -- Estimate digits from range midpoint using the search type
    CASE
        WHEN sj.search_type IN ('palindromic', 'near_repdigit') THEN
            ((wb.block_start + wb.block_end) / 2.0)
        WHEN sj.search_type IN ('kbn', 'twin', 'sophie_germain', 'cullen_woodall', 'wagstaff', 'carol_kynea', 'gen_fermat') THEN
            ((wb.block_start + wb.block_end) / 2.0) * 0.301
        WHEN sj.search_type IN ('factorial') THEN
            ((wb.block_start + wb.block_end) / 2.0) * LN((wb.block_start + wb.block_end) / 2.0 / EXP(1.0)) / LN(10.0)
        WHEN sj.search_type IN ('primorial') THEN
            ((wb.block_start + wb.block_end) / 2.0) / LN(10.0)
        WHEN sj.search_type = 'repunit' THEN
            ((wb.block_start + wb.block_end) / 2.0)
        ELSE
            ((wb.block_start + wb.block_end) / 2.0) * 0.301
    END AS digits,
    -- Seconds per candidate
    CASE
        WHEN wb.tested > 0 THEN
            EXTRACT(EPOCH FROM (wb.completed_at - wb.claimed_at))::float8 / wb.tested
        ELSE NULL
    END AS secs,
    wb.tested,
    wb.completed_at
FROM work_blocks wb
JOIN search_jobs sj ON sj.id = wb.search_job_id
WHERE wb.status = 'completed'
  AND wb.tested > 0
  AND wb.completed_at IS NOT NULL
  AND wb.claimed_at IS NOT NULL
  AND wb.completed_at > wb.claimed_at;
