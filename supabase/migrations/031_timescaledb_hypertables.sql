-- Migration 031: TimescaleDB hypertables for time-series data
--
-- Phase 3 of the database infrastructure roadmap: self-hosted PostgreSQL.
--
-- Converts high-churn observability tables to TimescaleDB hypertables with:
-- - Automatic time-based partitioning (1-day chunks)
-- - Retention policies (auto-delete old data)
-- - Continuous aggregates (replace manual rollup code)
--
-- IMPORTANT: This migration requires TimescaleDB extension to be installed.
-- It is safe to skip if running on vanilla PostgreSQL (Supabase).
-- Set TIMESCALEDB=true in environment to enable TimescaleDB-specific behavior.

-- Only run if TimescaleDB extension is available
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_available_extensions WHERE name = 'timescaledb'
    ) THEN
        CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

        -- ── Convert metric_samples to hypertable ────────────────────
        -- 1-day chunks, 7-day retention (raw samples)
        IF NOT EXISTS (
            SELECT 1 FROM timescaledb_information.hypertables
            WHERE hypertable_name = 'metric_samples'
        ) THEN
            PERFORM create_hypertable('metric_samples', 'ts',
                chunk_time_interval => INTERVAL '1 day',
                migrate_data => true
            );
        END IF;

        -- ── Convert system_logs to hypertable ───────────────────────
        -- 1-day chunks, 30-day retention
        IF NOT EXISTS (
            SELECT 1 FROM timescaledb_information.hypertables
            WHERE hypertable_name = 'system_logs'
        ) THEN
            PERFORM create_hypertable('system_logs', 'ts',
                chunk_time_interval => INTERVAL '1 day',
                migrate_data => true
            );
        END IF;

        -- ── Retention policies ──────────────────────────────────────
        -- Auto-drop chunks older than retention period
        PERFORM add_retention_policy('metric_samples', INTERVAL '7 days',
            if_not_exists => true);
        PERFORM add_retention_policy('system_logs', INTERVAL '30 days',
            if_not_exists => true);

        -- ── Continuous aggregates ───────────────────────────────────
        -- Hourly rollups (replace manual rollup_metrics_hour)
        CREATE MATERIALIZED VIEW IF NOT EXISTS metric_hourly
        WITH (timescaledb.continuous) AS
        SELECT
            time_bucket('1 hour', ts) AS bucket_start,
            scope,
            metric,
            labels,
            COUNT(*)::BIGINT AS count,
            SUM(value)::DOUBLE PRECISION AS sum,
            MIN(value)::DOUBLE PRECISION AS min,
            MAX(value)::DOUBLE PRECISION AS max
        FROM metric_samples
        GROUP BY bucket_start, scope, metric, labels
        WITH NO DATA;

        -- Daily rollups (replace manual rollup_metrics_day)
        CREATE MATERIALIZED VIEW IF NOT EXISTS metric_daily
        WITH (timescaledb.continuous) AS
        SELECT
            time_bucket('1 day', ts) AS bucket_start,
            scope,
            metric,
            labels,
            COUNT(*)::BIGINT AS count,
            SUM(value)::DOUBLE PRECISION AS sum,
            MIN(value)::DOUBLE PRECISION AS min,
            MAX(value)::DOUBLE PRECISION AS max
        FROM metric_samples
        GROUP BY bucket_start, scope, metric, labels
        WITH NO DATA;

        -- Refresh policies for continuous aggregates
        PERFORM add_continuous_aggregate_policy('metric_hourly',
            start_offset => INTERVAL '3 hours',
            end_offset => INTERVAL '1 hour',
            schedule_interval => INTERVAL '1 hour',
            if_not_exists => true
        );

        PERFORM add_continuous_aggregate_policy('metric_daily',
            start_offset => INTERVAL '3 days',
            end_offset => INTERVAL '1 day',
            schedule_interval => INTERVAL '1 day',
            if_not_exists => true
        );

        -- Retention for continuous aggregate materialized data
        PERFORM add_retention_policy('metric_hourly', INTERVAL '365 days',
            if_not_exists => true);
        PERFORM add_retention_policy('metric_daily', INTERVAL '1825 days',
            if_not_exists => true);

        RAISE NOTICE 'TimescaleDB hypertables and continuous aggregates created';
    ELSE
        RAISE NOTICE 'TimescaleDB not available, skipping hypertable creation';
    END IF;
END $$;
