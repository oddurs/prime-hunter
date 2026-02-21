-- Migration 030: Materialized views and additional indexes
--
-- Phase 1 of the database infrastructure roadmap: optimize Supabase.
--
-- 1. Additional indexes for common dashboard queries
-- 2. Materialized views for the 4 expensive RPC functions
-- 3. Updated RPCs to query materialized views instead of raw tables

-- ── Additional Indexes ──────────────────────────────────────────

-- Discovery timeline: used by get_discovery_timeline()
CREATE INDEX IF NOT EXISTS idx_primes_found_at_form
    ON primes (found_at, form);

-- Digit distribution: used by get_digit_distribution()
CREATE INDEX IF NOT EXISTS idx_primes_digits_form
    ON primes (digits, form);

-- Filtered browse queries: form-specific sorted by digits
CREATE INDEX IF NOT EXISTS idx_primes_form_digits
    ON primes (form, digits DESC);

-- ── Materialized Views ──────────────────────────────────────────

-- mv_dashboard_stats: replaces the full table scan in get_stats()
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_dashboard_stats AS
SELECT
    COUNT(*) AS total,
    COALESCE(
        (SELECT json_agg(row_to_json(t))
         FROM (
             SELECT form, COUNT(*) AS count
             FROM primes
             GROUP BY form
             ORDER BY count DESC
         ) t),
        '[]'::json
    ) AS by_form,
    COALESCE(
        (SELECT digits FROM primes ORDER BY digits DESC LIMIT 1),
        0
    ) AS largest_digits,
    (SELECT expression FROM primes ORDER BY digits DESC LIMIT 1) AS largest_expression;

CREATE UNIQUE INDEX IF NOT EXISTS idx_mv_dashboard_stats_unique
    ON mv_dashboard_stats (total);

-- mv_form_leaderboard: replaces the complex CTE in get_form_leaderboard()
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_form_leaderboard AS
WITH form_stats AS (
    SELECT form,
        COUNT(*) AS count,
        MAX(digits) AS largest_digits,
        MAX(found_at) AS latest_found_at,
        COUNT(*) FILTER (WHERE verified = true) AS verified_count
    FROM primes GROUP BY form
),
form_largest AS (
    SELECT DISTINCT ON (form) form, expression AS largest_expression
    FROM primes ORDER BY form, digits DESC
)
SELECT s.form, s.count, s.largest_digits,
    l.largest_expression, s.latest_found_at, s.verified_count,
    CASE WHEN s.count > 0
        THEN ROUND(100.0 * s.verified_count / s.count, 1)
        ELSE 0 END AS verified_pct
FROM form_stats s
JOIN form_largest l ON s.form = l.form
ORDER BY s.count DESC;

CREATE UNIQUE INDEX IF NOT EXISTS idx_mv_form_leaderboard_form
    ON mv_form_leaderboard (form);

-- ── Updated RPC Functions ───────────────────────────────────────

-- get_stats() now reads from mv_dashboard_stats (instant, no table scan)
CREATE OR REPLACE FUNCTION get_stats()
RETURNS JSON
LANGUAGE plpgsql
STABLE
AS $$
DECLARE
    result JSON;
BEGIN
    SELECT json_build_object(
        'total', total,
        'by_form', by_form,
        'largest_digits', largest_digits,
        'largest_expression', largest_expression
    ) INTO result
    FROM mv_dashboard_stats
    LIMIT 1;

    -- Fallback to live query if materialized view is empty
    IF result IS NULL THEN
        SELECT json_build_object(
            'total', (SELECT COUNT(*) FROM primes),
            'by_form', COALESCE(
                (SELECT json_agg(row_to_json(t))
                 FROM (
                     SELECT form, COUNT(*) AS count
                     FROM primes
                     GROUP BY form
                     ORDER BY count DESC
                 ) t),
                '[]'::json
            ),
            'largest_digits', COALESCE(
                (SELECT digits FROM primes ORDER BY digits DESC LIMIT 1),
                0
            ),
            'largest_expression', (
                SELECT expression FROM primes ORDER BY digits DESC LIMIT 1
            )
        ) INTO result;
    END IF;

    RETURN result;
END;
$$;

-- get_form_leaderboard() now reads from mv_form_leaderboard
CREATE OR REPLACE FUNCTION get_form_leaderboard()
RETURNS JSON
LANGUAGE plpgsql
STABLE
AS $$
DECLARE
    result JSON;
BEGIN
    SELECT COALESCE(json_agg(row_to_json(t)), '[]'::json)
    INTO result
    FROM (
        SELECT form, count, largest_digits, largest_expression,
               latest_found_at, verified_count, verified_pct
        FROM mv_form_leaderboard
        ORDER BY count DESC
    ) t;

    -- Fallback if materialized view is empty
    IF result = '[]'::json THEN
        SELECT COALESCE(json_agg(row_to_json(t)), '[]'::json)
        INTO result
        FROM (
            WITH form_stats AS (
                SELECT form,
                    COUNT(*) AS count,
                    MAX(digits) AS largest_digits,
                    MAX(found_at) AS latest_found_at,
                    COUNT(*) FILTER (WHERE verified = true) AS verified_count
                FROM primes GROUP BY form
            ),
            form_largest AS (
                SELECT DISTINCT ON (form) form, expression AS largest_expression
                FROM primes ORDER BY form, digits DESC
            )
            SELECT s.form, s.count, s.largest_digits,
                l.largest_expression, s.latest_found_at, s.verified_count,
                CASE WHEN s.count > 0
                    THEN ROUND(100.0 * s.verified_count / s.count, 1)
                    ELSE 0 END AS verified_pct
            FROM form_stats s
            JOIN form_largest l ON s.form = l.form
            ORDER BY s.count DESC
        ) t;
    END IF;

    RETURN result;
END;
$$;
