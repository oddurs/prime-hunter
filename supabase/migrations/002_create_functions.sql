-- Aggregate stats function: total, by_form, largest prime
CREATE OR REPLACE FUNCTION get_stats()
RETURNS JSON
LANGUAGE plpgsql
STABLE
AS $$
DECLARE
    result JSON;
BEGIN
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
    RETURN result;
END;
$$;

-- Discovery timeline: bucket primes by time period and form
CREATE OR REPLACE FUNCTION get_discovery_timeline(bucket_type TEXT DEFAULT 'day')
RETURNS JSON
LANGUAGE plpgsql
STABLE
AS $$
DECLARE
    result JSON;
    trunc_unit TEXT;
BEGIN
    -- Map bucket_type to date_trunc interval
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
        FROM primes
        GROUP BY bucket, form
        ORDER BY bucket
    ) t;

    RETURN result;
END;
$$;

-- Digit distribution: bucket primes by digit count ranges
CREATE OR REPLACE FUNCTION get_digit_distribution(bucket_size_param BIGINT DEFAULT 10)
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
        SELECT
            (digits / bucket_size_param) * bucket_size_param AS bucket_start,
            form,
            COUNT(*) AS count
        FROM primes
        GROUP BY bucket_start, form
        ORDER BY bucket_start
    ) t;

    RETURN result;
END;
$$;
