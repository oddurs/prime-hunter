-- Migration: get_form_leaderboard() RPC
--
-- Returns per-form aggregate statistics as a JSON array, ordered by
-- prime count descending. Each entry includes: form name, total count,
-- largest digit count + expression, most recent discovery timestamp,
-- verified count, and verified percentage.
--
-- Used by the dashboard Form Leaderboard component for at-a-glance
-- analytics across all prime forms.

CREATE OR REPLACE FUNCTION get_form_leaderboard()
RETURNS JSON LANGUAGE plpgsql STABLE AS $$
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
      FROM primes GROUP BY form
    ),
    form_largest AS (
      SELECT DISTINCT ON (form) form, expression as largest_expression
      FROM primes ORDER BY form, digits DESC
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
END; $$;
