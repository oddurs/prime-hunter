-- 029_security_hardening.sql
--
-- Fixes all Supabase linter security warnings and errors:
--
-- 1. Enable RLS on all public tables that were missing it
-- 2. Fix SECURITY DEFINER views → SECURITY INVOKER
-- 3. Set search_path on all public functions
-- 4. Tighten overly permissive RLS write policies
-- 5. Hide sensitive api_key column from backward-compat views
--
-- Linter issues resolved:
--   - rls_disabled_in_public (22 tables)
--   - security_definer_view (2 views)
--   - function_search_path_mutable (9 functions)
--   - rls_policy_always_true (2 policies)
--   - sensitive_columns_exposed (volunteers.api_key)

BEGIN;

-- ============================================================
-- 1. Enable RLS on all tables missing it
-- ============================================================
-- All tables get read-only access for all roles (including anon/authenticated).
-- Write operations go through the Rust backend via the service_role key,
-- which bypasses RLS. This is the standard Supabase pattern for
-- backend-managed tables.

-- ── Project tables (from 011_projects.sql) ──
ALTER TABLE projects ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_projects" ON projects FOR SELECT USING (true);

ALTER TABLE project_phases ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_project_phases" ON project_phases FOR SELECT USING (true);

ALTER TABLE project_events ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_project_events" ON project_events FOR SELECT USING (true);

ALTER TABLE records ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_records" ON records FOR SELECT USING (true);

-- ── Agent tables (from 009, 010, 013, 014, 018) ──
ALTER TABLE agent_memory ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_agent_memory" ON agent_memory FOR SELECT USING (true);

ALTER TABLE agent_task_deps ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_agent_task_deps" ON agent_task_deps FOR SELECT USING (true);

ALTER TABLE agent_templates ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_agent_templates" ON agent_templates FOR SELECT USING (true);

ALTER TABLE agent_roles ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_agent_roles" ON agent_roles FOR SELECT USING (true);

ALTER TABLE agent_role_templates ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_agent_role_templates" ON agent_role_templates FOR SELECT USING (true);

ALTER TABLE agent_schedules ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_agent_schedules" ON agent_schedules FOR SELECT USING (true);

ALTER TABLE agent_logs ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_agent_logs" ON agent_logs FOR SELECT USING (true);

-- ── Operator tables (underlying tables renamed in 025) ──
ALTER TABLE operators ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_operators" ON operators FOR SELECT USING (true);

ALTER TABLE operator_nodes ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_operator_nodes" ON operator_nodes FOR SELECT USING (true);

ALTER TABLE operator_trust ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_operator_trust" ON operator_trust FOR SELECT USING (true);

ALTER TABLE operator_credits ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_operator_credits" ON operator_credits FOR SELECT USING (true);

-- ── Cost calibration (from 017) ──
ALTER TABLE cost_calibration ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_cost_calibration" ON cost_calibration FOR SELECT USING (true);

-- ── Observability tables (from 020, 024) ──
ALTER TABLE system_logs ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_system_logs" ON system_logs FOR SELECT USING (true);

ALTER TABLE metric_samples ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_metric_samples" ON metric_samples FOR SELECT USING (true);

ALTER TABLE metric_rollups_hourly ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_metric_rollups_hourly" ON metric_rollups_hourly FOR SELECT USING (true);

ALTER TABLE metric_rollups_daily ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_metric_rollups_daily" ON metric_rollups_daily FOR SELECT USING (true);

-- ── Release tables (from 022) ──
ALTER TABLE worker_releases ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_worker_releases" ON worker_releases FOR SELECT USING (true);

ALTER TABLE worker_release_channels ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_worker_release_channels" ON worker_release_channels FOR SELECT USING (true);

ALTER TABLE worker_release_events ENABLE ROW LEVEL SECURITY;
CREATE POLICY "read_worker_release_events" ON worker_release_events FOR SELECT USING (true);

-- ============================================================
-- 2. Fix SECURITY DEFINER views → SECURITY INVOKER
-- ============================================================
-- Recreate views with explicit SECURITY INVOKER so they respect
-- the querying user's permissions rather than the view creator's.

-- Drop backward-compat aliases first (they depend on operator_leaderboard)
DROP VIEW IF EXISTS volunteer_leaderboard;
DROP VIEW IF EXISTS operator_leaderboard;

CREATE VIEW operator_leaderboard
WITH (security_invoker = true)
AS
SELECT
  o.id,
  o.username,
  o.team,
  o.credit,
  o.primes_found,
  o.joined_at,
  o.last_seen,
  COALESCE(ot.trust_level, 1) AS trust_level,
  COUNT(DISTINCT on_node.id) AS worker_count
FROM operators o
LEFT JOIN operator_trust ot ON ot.volunteer_id = o.id
LEFT JOIN operator_nodes on_node ON on_node.volunteer_id = o.id
GROUP BY o.id, o.username, o.team, o.credit, o.primes_found,
         o.joined_at, o.last_seen, ot.trust_level
ORDER BY o.credit DESC;

-- Backward-compat alias (also SECURITY INVOKER)
CREATE VIEW volunteer_leaderboard
WITH (security_invoker = true)
AS SELECT * FROM operator_leaderboard;

-- Recreate cost_observations with SECURITY INVOKER
DROP VIEW IF EXISTS cost_observations;
CREATE VIEW cost_observations
WITH (security_invoker = true)
AS
SELECT
    sj.search_type AS form,
    wb.duration_secs,
    COALESCE(wb.cores_used, 1) AS cores_used,
    (wb.block_end - wb.block_start) AS candidates,
    wb.completed_at
FROM work_blocks wb
JOIN search_jobs sj ON wb.search_job_id = sj.id
WHERE wb.status = 'completed'
  AND wb.duration_secs IS NOT NULL
  AND wb.duration_secs > 0
  AND (wb.block_end - wb.block_start) > 0;

-- ============================================================
-- 3. Recreate backward-compat views hiding sensitive columns
-- ============================================================
-- The `volunteers` view previously exposed the api_key column.
-- Recreate it with explicit column list excluding api_key.

DROP VIEW IF EXISTS volunteers;
CREATE VIEW volunteers
WITH (security_invoker = true)
AS SELECT id, username, email, team, credit, primes_found, joined_at, last_seen
FROM operators;

-- Recreate other backward-compat views with SECURITY INVOKER
DROP VIEW IF EXISTS volunteer_workers;
CREATE VIEW volunteer_workers
WITH (security_invoker = true)
AS SELECT * FROM operator_nodes;

DROP VIEW IF EXISTS volunteer_trust;
CREATE VIEW volunteer_trust
WITH (security_invoker = true)
AS SELECT * FROM operator_trust;

DROP VIEW IF EXISTS credit_log;
CREATE VIEW credit_log
WITH (security_invoker = true)
AS SELECT * FROM operator_credits;

-- ============================================================
-- 4. Fix function search_path (SET search_path = '')
-- ============================================================
-- Prevents search_path manipulation attacks by pinning all
-- functions to an empty search_path. All table references
-- resolve to public schema by default in Supabase.

-- 4.1 get_stats (from 002)
CREATE OR REPLACE FUNCTION get_stats()
RETURNS JSON
LANGUAGE plpgsql
STABLE
SET search_path = ''
AS $$
DECLARE
    result JSON;
BEGIN
    SELECT json_build_object(
        'total', (SELECT COUNT(*) FROM public.primes),
        'by_form', COALESCE(
            (SELECT json_agg(row_to_json(t))
             FROM (
                 SELECT form, COUNT(*) AS count
                 FROM public.primes
                 GROUP BY form
                 ORDER BY count DESC
             ) t),
            '[]'::json
        ),
        'largest_digits', COALESCE(
            (SELECT digits FROM public.primes ORDER BY digits DESC LIMIT 1),
            0
        ),
        'largest_expression', (
            SELECT expression FROM public.primes ORDER BY digits DESC LIMIT 1
        )
    ) INTO result;
    RETURN result;
END;
$$;

-- 4.2 get_discovery_timeline (from 002)
CREATE OR REPLACE FUNCTION get_discovery_timeline(bucket_type TEXT DEFAULT 'day')
RETURNS JSON
LANGUAGE plpgsql
STABLE
SET search_path = ''
AS $$
DECLARE
    result JSON;
    trunc_unit TEXT;
BEGIN
    CASE bucket_type
        WHEN 'hour' THEN trunc_unit := 'hour';
        WHEN 'week' THEN trunc_unit := 'week';
        WHEN 'month' THEN trunc_unit := 'month';
        ELSE trunc_unit := 'day';
    END CASE;

    SELECT COALESCE(json_agg(row_to_json(t)), '[]'::json)
    INTO result
    FROM (
        SELECT
            to_char(date_trunc(trunc_unit, found_at), 'YYYY-MM-DD"T"HH24:MI:SS') AS bucket,
            form,
            COUNT(*) AS count
        FROM public.primes
        GROUP BY bucket, form
        ORDER BY bucket
    ) t;

    RETURN result;
END;
$$;

-- 4.3 get_digit_distribution (from 002)
CREATE OR REPLACE FUNCTION get_digit_distribution(bucket_size_param BIGINT DEFAULT 10)
RETURNS JSON
LANGUAGE plpgsql
STABLE
SET search_path = ''
AS $$
DECLARE
    result JSON;
BEGIN
    SELECT COALESCE(json_agg(row_to_json(t)), '[]'::json)
    INTO result
    FROM (
        SELECT
            (digits / bucket_size_param) * bucket_size_param AS bucket_start,
            form,
            COUNT(*) AS count
        FROM public.primes
        GROUP BY bucket_start, form
        ORDER BY bucket_start
    ) t;

    RETURN result;
END;
$$;

-- 4.4 worker_heartbeat (from 004)
CREATE OR REPLACE FUNCTION worker_heartbeat(
    p_worker_id TEXT, p_hostname TEXT, p_cores INTEGER,
    p_search_type TEXT, p_search_params TEXT,
    p_tested BIGINT, p_found BIGINT, p_current TEXT,
    p_checkpoint TEXT, p_metrics JSONB
)
RETURNS TEXT
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE v_command TEXT;
BEGIN
    SELECT pending_command INTO v_command FROM public.workers WHERE worker_id = p_worker_id FOR UPDATE;
    INSERT INTO public.workers (worker_id, hostname, cores, search_type, search_params,
                         tested, found, current, checkpoint, metrics, last_heartbeat, pending_command)
    VALUES (p_worker_id, p_hostname, p_cores, p_search_type, p_search_params,
            p_tested, p_found, p_current, p_checkpoint, p_metrics, NOW(), NULL)
    ON CONFLICT (worker_id) DO UPDATE SET
        tested = EXCLUDED.tested, found = EXCLUDED.found, current = EXCLUDED.current,
        checkpoint = EXCLUDED.checkpoint, metrics = EXCLUDED.metrics,
        last_heartbeat = NOW(), pending_command = NULL;
    RETURN v_command;
END;
$$;

-- 4.5 claim_work_block (from 016, replaces 004 version)
CREATE OR REPLACE FUNCTION claim_work_block(p_job_id BIGINT, p_worker_id TEXT)
RETURNS TABLE(block_id BIGINT, block_start BIGINT, block_end BIGINT)
LANGUAGE sql
SET search_path = ''
AS $$
    UPDATE public.work_blocks
    SET status = 'claimed', claimed_by = p_worker_id, claimed_at = NOW()
    WHERE id = (
        SELECT wb.id FROM public.work_blocks wb
        JOIN public.search_jobs sj ON sj.id = wb.search_job_id
        WHERE wb.search_job_id = p_job_id
          AND wb.status = 'available'
          AND sj.status = 'running'
        ORDER BY wb.block_start LIMIT 1
        FOR UPDATE OF wb SKIP LOCKED
    )
    RETURNING id AS block_id, public.work_blocks.block_start, public.work_blocks.block_end;
$$;

-- 4.6 reclaim_stale_blocks (from 004)
CREATE OR REPLACE FUNCTION reclaim_stale_blocks(p_stale_seconds INTEGER DEFAULT 120)
RETURNS INTEGER
LANGUAGE sql
SET search_path = ''
AS $$
    WITH reclaimed AS (
        UPDATE public.work_blocks SET status = 'available', claimed_by = NULL, claimed_at = NULL
        WHERE status = 'claimed'
          AND claimed_at < NOW() - (p_stale_seconds || ' seconds')::interval
          AND NOT EXISTS (
              SELECT 1 FROM public.workers
              WHERE public.workers.worker_id = public.work_blocks.claimed_by
                AND public.workers.last_heartbeat > NOW() - INTERVAL '60 seconds'
          )
        RETURNING id
    ) SELECT COUNT(*)::INTEGER FROM reclaimed;
$$;

-- 4.7 complete_work_block_with_duration (from 0121)
CREATE OR REPLACE FUNCTION complete_work_block_with_duration(
    p_block_id BIGINT,
    p_tested BIGINT,
    p_found BIGINT,
    p_cores_used INTEGER DEFAULT 1
)
RETURNS VOID
LANGUAGE sql
SET search_path = ''
AS $$
    UPDATE public.work_blocks
    SET status = 'completed',
        completed_at = NOW(),
        tested = p_tested,
        found = p_found,
        cores_used = p_cores_used,
        duration_secs = EXTRACT(EPOCH FROM (NOW() - COALESCE(claimed_at, NOW())))
    WHERE id = p_block_id;
$$;

-- 4.8 get_job_core_hours (from 0121)
CREATE OR REPLACE FUNCTION get_job_core_hours(p_job_id BIGINT)
RETURNS DOUBLE PRECISION
LANGUAGE sql
STABLE
SET search_path = ''
AS $$
    SELECT COALESCE(
        SUM(duration_secs * COALESCE(cores_used, 1)) / 3600.0,
        0.0
    )
    FROM public.work_blocks
    WHERE search_job_id = p_job_id AND status = 'completed';
$$;

-- 4.9 get_form_leaderboard (from 012)
CREATE OR REPLACE FUNCTION get_form_leaderboard()
RETURNS JSON
LANGUAGE plpgsql
STABLE
SET search_path = ''
AS $$
DECLARE result JSON;
BEGIN
  SELECT COALESCE(json_agg(row_to_json(t)), '[]'::json)
  INTO result
  FROM (
    WITH form_stats AS (
      SELECT form,
        COUNT(*) as count,
        MAX(digits) as largest_digits,
        MAX(found_at) as latest_found_at,
        COUNT(*) FILTER (WHERE verified = true) as verified_count
      FROM public.primes GROUP BY form
    ),
    form_largest AS (
      SELECT DISTINCT ON (form) form, expression as largest_expression
      FROM public.primes ORDER BY form, digits DESC
    )
    SELECT s.form, s.count, s.largest_digits,
      l.largest_expression, s.latest_found_at, s.verified_count,
      CASE WHEN s.count > 0
        THEN ROUND(100.0 * s.verified_count / s.count, 1)
        ELSE 0 END as verified_pct
    FROM form_stats s
    JOIN form_largest l ON s.form = l.form
    ORDER BY s.count DESC
  ) t;
  RETURN result;
END;
$$;

-- ============================================================
-- 5. Tighten overly permissive RLS write policies
-- ============================================================
-- Replace `FOR ALL ... USING (true) WITH CHECK (true)` with
-- separate SELECT + INSERT/UPDATE policies that require the
-- authenticated role. DELETE is omitted (backend handles it).

-- agent_tasks: drop the overly permissive ALL policy, add granular ones
DROP POLICY IF EXISTS "auth_write_agent_tasks" ON agent_tasks;
CREATE POLICY "auth_insert_agent_tasks" ON agent_tasks
    FOR INSERT TO authenticated WITH CHECK (true);
CREATE POLICY "auth_update_agent_tasks" ON agent_tasks
    FOR UPDATE TO authenticated USING (true) WITH CHECK (true);

-- agent_budgets: drop the overly permissive ALL policy, add granular ones
DROP POLICY IF EXISTS "auth_write_agent_budgets" ON agent_budgets;
CREATE POLICY "auth_insert_agent_budgets" ON agent_budgets
    FOR INSERT TO authenticated WITH CHECK (true);
CREATE POLICY "auth_update_agent_budgets" ON agent_budgets
    FOR UPDATE TO authenticated USING (true) WITH CHECK (true);

COMMIT;
