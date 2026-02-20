-- Daily rollups for long-term observability retention

CREATE TABLE IF NOT EXISTS metric_rollups_daily (
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

CREATE INDEX IF NOT EXISTS idx_metric_rollups_daily_metric_bucket ON metric_rollups_daily (metric, bucket_start DESC);
CREATE INDEX IF NOT EXISTS idx_metric_rollups_daily_scope_bucket ON metric_rollups_daily (scope, bucket_start DESC);
