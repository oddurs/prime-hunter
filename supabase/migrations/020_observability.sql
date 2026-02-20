-- Observability: system logs + time-series metrics (raw + hourly rollups)

CREATE TABLE IF NOT EXISTS system_logs (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    level TEXT NOT NULL CHECK (level IN ('debug','info','warn','error')),
    source TEXT NOT NULL,
    component TEXT NOT NULL,
    message TEXT NOT NULL,
    worker_id TEXT,
    search_job_id BIGINT,
    search_id TEXT,
    context JSONB
);

CREATE INDEX IF NOT EXISTS idx_system_logs_ts ON system_logs (ts DESC);
CREATE INDEX IF NOT EXISTS idx_system_logs_level ON system_logs (level);
CREATE INDEX IF NOT EXISTS idx_system_logs_component ON system_logs (component);
CREATE INDEX IF NOT EXISTS idx_system_logs_worker ON system_logs (worker_id) WHERE worker_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_system_logs_job ON system_logs (search_job_id) WHERE search_job_id IS NOT NULL;

-- Narrow time-series table for metrics
CREATE TABLE IF NOT EXISTS metric_samples (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    scope TEXT NOT NULL,
    metric TEXT NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    labels JSONB
);

CREATE INDEX IF NOT EXISTS idx_metric_samples_metric_ts ON metric_samples (metric, ts DESC);
CREATE INDEX IF NOT EXISTS idx_metric_samples_scope_ts ON metric_samples (scope, ts DESC);
CREATE INDEX IF NOT EXISTS idx_metric_samples_labels_worker ON metric_samples ((labels->>'worker_id'));

-- Hourly rollups for long-term retention
CREATE TABLE IF NOT EXISTS metric_rollups_hourly (
    bucket_start TIMESTAMPTZ NOT NULL,
    scope TEXT NOT NULL,
    metric TEXT NOT NULL,
    labels JSONB,
    count BIGINT NOT NULL,
    sum DOUBLE PRECISION NOT NULL,
    min DOUBLE PRECISION NOT NULL,
    max DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (bucket_start, scope, metric, labels)
);

CREATE INDEX IF NOT EXISTS idx_metric_rollups_metric_bucket ON metric_rollups_hourly (metric, bucket_start DESC);
CREATE INDEX IF NOT EXISTS idx_metric_rollups_scope_bucket ON metric_rollups_hourly (scope, bucket_start DESC);
