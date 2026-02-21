//! Observability storage: system logs + time-series metrics.

use super::Database;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricSample {
    pub ts: DateTime<Utc>,
    pub scope: String,
    pub metric: String,
    pub value: f64,
    pub labels: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricPoint {
    pub ts: DateTime<Utc>,
    pub value: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricSeries {
    pub metric: String,
    pub scope: String,
    pub labels: Option<Value>,
    pub points: Vec<MetricPoint>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemLogEntry {
    pub ts: DateTime<Utc>,
    pub level: String,
    pub source: String,
    pub component: String,
    pub message: String,
    pub worker_id: Option<String>,
    pub search_job_id: Option<i64>,
    pub search_id: Option<String>,
    pub context: Option<Value>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SystemLogRow {
    pub id: i64,
    pub ts: DateTime<Utc>,
    pub level: String,
    pub source: String,
    pub component: String,
    pub message: String,
    pub worker_id: Option<String>,
    pub search_job_id: Option<i64>,
    pub search_id: Option<String>,
    pub context: Option<Value>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct WorkerRateRow {
    pub worker_id: String,
    pub hostname: String,
    pub search_type: String,
    pub rate: f64,
    pub tested: f64,
    pub found: f64,
}

fn rate_from_samples(latest: f64, previous: f64, dt: f64) -> f64 {
    if dt <= 0.0 {
        0.0
    } else {
        (latest - previous) / dt
    }
}

#[derive(Clone, Debug, sqlx::FromRow)]
struct WorkerRateRaw {
    worker_id: String,
    hostname: String,
    search_type: String,
    tested_latest: f64,
    tested_prev: f64,
    dt: f64,
    found: f64,
}

impl Database {
    pub async fn insert_metric_samples(&self, samples: &[MetricSample]) -> Result<()> {
        if samples.is_empty() {
            return Ok(());
        }
        let ts: Vec<DateTime<Utc>> = samples.iter().map(|s| s.ts).collect();
        let scopes: Vec<String> = samples.iter().map(|s| s.scope.clone()).collect();
        let metrics: Vec<String> = samples.iter().map(|s| s.metric.clone()).collect();
        let values: Vec<f64> = samples.iter().map(|s| s.value).collect();
        let labels: Vec<Value> = samples
            .iter()
            .map(|s| s.labels.clone().unwrap_or(Value::Null))
            .collect();

        sqlx::query(
            "INSERT INTO metric_samples (ts, scope, metric, value, labels)\n             SELECT * FROM UNNEST($1::timestamptz[], $2::text[], $3::text[], $4::double precision[], $5::jsonb[])",
        )
        .bind(&ts)
        .bind(&scopes)
        .bind(&metrics)
        .bind(&values)
        .bind(&labels)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_system_log(&self, log: &SystemLogEntry) -> Result<()> {
        sqlx::query(
            "INSERT INTO system_logs (ts, level, source, component, message, worker_id, search_job_id, search_id, context)\n             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(log.ts)
        .bind(&log.level)
        .bind(&log.source)
        .bind(&log.component)
        .bind(&log.message)
        .bind(&log.worker_id)
        .bind(&log.search_job_id)
        .bind(&log.search_id)
        .bind(&log.context)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_system_logs(&self, logs: &[SystemLogEntry]) -> Result<()> {
        if logs.is_empty() {
            return Ok(());
        }
        let ts: Vec<DateTime<Utc>> = logs.iter().map(|l| l.ts).collect();
        let levels: Vec<String> = logs.iter().map(|l| l.level.clone()).collect();
        let sources: Vec<String> = logs.iter().map(|l| l.source.clone()).collect();
        let components: Vec<String> = logs.iter().map(|l| l.component.clone()).collect();
        let messages: Vec<String> = logs.iter().map(|l| l.message.clone()).collect();
        let worker_ids: Vec<Option<String>> = logs.iter().map(|l| l.worker_id.clone()).collect();
        let search_job_ids: Vec<Option<i64>> = logs.iter().map(|l| l.search_job_id).collect();
        let search_ids: Vec<Option<String>> = logs.iter().map(|l| l.search_id.clone()).collect();
        let contexts: Vec<Value> = logs
            .iter()
            .map(|l| l.context.clone().unwrap_or(Value::Null))
            .collect();

        sqlx::query(
            "INSERT INTO system_logs (ts, level, source, component, message, worker_id, search_job_id, search_id, context)\n             SELECT * FROM UNNEST($1::timestamptz[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[], $7::bigint[], $8::text[], $9::jsonb[])",
        )
        .bind(&ts)
        .bind(&levels)
        .bind(&sources)
        .bind(&components)
        .bind(&messages)
        .bind(&worker_ids)
        .bind(&search_job_ids)
        .bind(&search_ids)
        .bind(&contexts)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Returns true if TimescaleDB continuous aggregates handle rollups.
    /// When true, manual rollup and prune methods become no-ops.
    fn timescaledb_enabled() -> bool {
        std::env::var("TIMESCALEDB").map(|v| v == "true" || v == "1").unwrap_or(false)
    }

    pub async fn rollup_metrics_hour(&self, hour_start: DateTime<Utc>) -> Result<()> {
        if Self::timescaledb_enabled() {
            return Ok(());
        }
        let hour_end = hour_start + chrono::Duration::hours(1);
        sqlx::query(
            "INSERT INTO metric_rollups_hourly (bucket_start, scope, metric, labels, count, sum, min, max)\n             SELECT date_trunc('hour', ts) AS bucket_start, scope, metric, labels,\n                    COUNT(*)::BIGINT, SUM(value)::DOUBLE PRECISION, MIN(value)::DOUBLE PRECISION, MAX(value)::DOUBLE PRECISION\n             FROM metric_samples\n             WHERE ts >= $1 AND ts < $2\n             GROUP BY bucket_start, scope, metric, labels\n             ON CONFLICT (bucket_start, scope, metric, labels)\n             DO UPDATE SET count = EXCLUDED.count, sum = EXCLUDED.sum, min = EXCLUDED.min, max = EXCLUDED.max",
        )
        .bind(hour_start)
        .bind(hour_end)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn rollup_metrics_day(&self, day_start: DateTime<Utc>) -> Result<()> {
        if Self::timescaledb_enabled() {
            return Ok(());
        }
        let day_end = day_start + chrono::Duration::days(1);
        sqlx::query(
            "INSERT INTO metric_rollups_daily (bucket_start, scope, metric, labels, count, sum, min, max)\n             SELECT date_trunc('day', ts) AS bucket_start, scope, metric, labels,\n                    COUNT(*)::BIGINT, SUM(value)::DOUBLE PRECISION, MIN(value)::DOUBLE PRECISION, MAX(value)::DOUBLE PRECISION\n             FROM metric_samples\n             WHERE ts >= $1 AND ts < $2\n             GROUP BY bucket_start, scope, metric, labels\n             ON CONFLICT (bucket_start, scope, metric, labels)\n             DO UPDATE SET count = EXCLUDED.count, sum = EXCLUDED.sum, min = EXCLUDED.min, max = EXCLUDED.max",
        )
        .bind(day_start)
        .bind(day_end)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn prune_metric_samples(&self, days: i64) -> Result<u64> {
        if Self::timescaledb_enabled() {
            return Ok(0);
        }
        let result =
            sqlx::query("DELETE FROM metric_samples WHERE ts < NOW() - ($1 || ' days')::interval")
                .bind(days.to_string())
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }

    pub async fn prune_metric_rollups(&self, days: i64) -> Result<u64> {
        if Self::timescaledb_enabled() {
            return Ok(0);
        }
        let result = sqlx::query(
            "DELETE FROM metric_rollups_hourly WHERE bucket_start < NOW() - ($1 || ' days')::interval",
        )
        .bind(days.to_string())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn prune_metric_rollups_daily(&self, days: i64) -> Result<u64> {
        if Self::timescaledb_enabled() {
            return Ok(0);
        }
        let result = sqlx::query(
            "DELETE FROM metric_rollups_daily WHERE bucket_start < NOW() - ($1 || ' days')::interval",
        )
        .bind(days.to_string())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn prune_system_logs(&self, days: i64) -> Result<u64> {
        if Self::timescaledb_enabled() {
            return Ok(0);
        }
        let result =
            sqlx::query("DELETE FROM system_logs WHERE ts < NOW() - ($1 || ' days')::interval")
                .bind(days.to_string())
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }

    pub async fn get_system_logs(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        level: Option<&str>,
        source: Option<&str>,
        component: Option<&str>,
        worker_id: Option<&str>,
        limit: i64,
    ) -> Result<Vec<SystemLogRow>> {
        let rows = sqlx::query_as::<_, SystemLogRow>(
            "SELECT id, ts, level, source, component, message, worker_id, search_job_id, search_id, context\n             FROM system_logs\n             WHERE ts BETWEEN $1 AND $2\n               AND ($3::text IS NULL OR level = $3)\n               AND ($4::text IS NULL OR source = $4)\n               AND ($5::text IS NULL OR component = $5)\n               AND ($6::text IS NULL OR worker_id = $6)\n             ORDER BY ts DESC\n             LIMIT $7",
        )
        .bind(from)
        .bind(to)
        .bind(level)
        .bind(source)
        .bind(component)
        .bind(worker_id)
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_metric_points(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        metric: &str,
        scope: Option<&str>,
        worker_id: Option<&str>,
        label_key: Option<&str>,
        label_value: Option<&str>,
        rollup: &str,
    ) -> Result<Vec<MetricPoint>> {
        if rollup == "hour" {
            let rows = sqlx::query_as::<_, (DateTime<Utc>, f64)>(
                "SELECT bucket_start AS ts, (sum / NULLIF(count, 0)) AS value\n                 FROM metric_rollups_hourly\n                 WHERE bucket_start BETWEEN $1 AND $2\n                   AND metric = $3\n                   AND ($4::text IS NULL OR scope = $4)\n                   AND ($5::text IS NULL OR labels->>'worker_id' = $5)\n                   AND ($6::text IS NULL OR labels->>$6 = $7)\n                 ORDER BY bucket_start",
            )
            .bind(from)
            .bind(to)
            .bind(metric)
            .bind(scope)
            .bind(worker_id)
            .bind(label_key)
            .bind(label_value)
            .fetch_all(&self.read_pool)
            .await?;
            return Ok(rows
                .into_iter()
                .map(|(ts, value)| MetricPoint { ts, value })
                .collect());
        }
        if rollup == "day" {
            let rows = sqlx::query_as::<_, (DateTime<Utc>, f64)>(
                "SELECT bucket_start AS ts, (sum / NULLIF(count, 0)) AS value\n                 FROM metric_rollups_daily\n                 WHERE bucket_start BETWEEN $1 AND $2\n                   AND metric = $3\n                   AND ($4::text IS NULL OR scope = $4)\n                   AND ($5::text IS NULL OR labels->>'worker_id' = $5)\n                   AND ($6::text IS NULL OR labels->>$6 = $7)\n                 ORDER BY bucket_start",
            )
            .bind(from)
            .bind(to)
            .bind(metric)
            .bind(scope)
            .bind(worker_id)
            .bind(label_key)
            .bind(label_value)
            .fetch_all(&self.read_pool)
            .await?;
            return Ok(rows
                .into_iter()
                .map(|(ts, value)| MetricPoint { ts, value })
                .collect());
        }
        let rows = sqlx::query_as::<_, (DateTime<Utc>, f64)>(
            "SELECT ts, value\n                 FROM metric_samples\n                 WHERE ts BETWEEN $1 AND $2\n                   AND metric = $3\n                   AND ($4::text IS NULL OR scope = $4)\n                   AND ($5::text IS NULL OR labels->>'worker_id' = $5)\n                   AND ($6::text IS NULL OR labels->>$6 = $7)\n                 ORDER BY ts",
        )
        .bind(from)
        .bind(to)
        .bind(metric)
        .bind(scope)
        .bind(worker_id)
        .bind(label_key)
        .bind(label_value)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(ts, value)| MetricPoint { ts, value })
            .collect())
    }

    pub async fn max_metric_in_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        metric: &str,
        scope: Option<&str>,
    ) -> Result<Option<f64>> {
        let value = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT MAX(value) FROM metric_samples WHERE ts BETWEEN $1 AND $2 AND metric = $3 AND ($4::text IS NULL OR scope = $4)",
        )
        .bind(from)
        .bind(to)
        .bind(metric)
        .bind(scope)
        .fetch_one(&self.read_pool)
        .await?;
        Ok(value)
    }

    pub async fn avg_metric_in_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        metric: &str,
        scope: Option<&str>,
    ) -> Result<Option<f64>> {
        let value = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT AVG(value) FROM metric_samples WHERE ts BETWEEN $1 AND $2 AND metric = $3 AND ($4::text IS NULL OR scope = $4)",
        )
        .bind(from)
        .bind(to)
        .bind(metric)
        .bind(scope)
        .fetch_one(&self.read_pool)
        .await?;
        Ok(value)
    }

    pub async fn delta_metric_in_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        metric: &str,
        scope: Option<&str>,
    ) -> Result<Option<f64>> {
        let first = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT value FROM metric_samples WHERE ts BETWEEN $1 AND $2 AND metric = $3 AND ($4::text IS NULL OR scope = $4) ORDER BY ts ASC LIMIT 1",
        )
        .bind(from)
        .bind(to)
        .bind(metric)
        .bind(scope)
        .fetch_one(&self.read_pool)
        .await?;
        let last = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT value FROM metric_samples WHERE ts BETWEEN $1 AND $2 AND metric = $3 AND ($4::text IS NULL OR scope = $4) ORDER BY ts DESC LIMIT 1",
        )
        .bind(from)
        .bind(to)
        .bind(metric)
        .bind(scope)
        .fetch_one(&self.read_pool)
        .await?;
        Ok(match (first, last) {
            (Some(a), Some(b)) => Some(b - a),
            _ => None,
        })
    }

    pub async fn count_system_logs_by_level(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<(String, i64)>> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT level, COUNT(*)::BIGINT FROM system_logs WHERE ts BETWEEN $1 AND $2 GROUP BY level",
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    /// Compute top workers by tested/sec over a recent window.
    pub async fn get_top_workers_by_rate(
        &self,
        window_minutes: i64,
        limit: i64,
    ) -> Result<Vec<WorkerRateRow>> {
        let rows = sqlx::query_as::<_, WorkerRateRaw>(
            "WITH recent AS (
                SELECT
                    labels->>'worker_id' AS worker_id,
                    labels->>'hostname' AS hostname,
                    labels->>'search_type' AS search_type,
                    ts,
                    value,
                    ROW_NUMBER() OVER (PARTITION BY labels->>'worker_id' ORDER BY ts DESC) AS rn
                FROM metric_samples
                WHERE metric = 'worker.tested'
                  AND ts > NOW() - ($1 || ' minutes')::interval
                  AND labels ? 'worker_id'
            ),
            diffs AS (
                SELECT
                    a.worker_id,
                    a.hostname,
                    a.search_type,
                    a.value AS tested_latest,
                    b.value AS tested_prev,
                    EXTRACT(EPOCH FROM (a.ts - b.ts))::float8 AS dt
                FROM recent a
                JOIN recent b
                  ON a.worker_id = b.worker_id
                 AND a.rn = 1
                 AND b.rn = 2
                WHERE a.ts > b.ts
            ),
            found_latest AS (
                SELECT
                    labels->>'worker_id' AS worker_id,
                    MAX(value) AS found_latest
                FROM metric_samples
                WHERE metric = 'worker.found'
                  AND ts > NOW() - ($1 || ' minutes')::interval
                  AND labels ? 'worker_id'
                GROUP BY labels->>'worker_id'
            )
            SELECT
                d.worker_id,
                COALESCE(d.hostname, d.worker_id) AS hostname,
                COALESCE(d.search_type, '') AS search_type,
                d.tested_latest,
                d.tested_prev,
                d.dt,
                COALESCE(f.found_latest, 0) AS found
            FROM diffs d
            LEFT JOIN found_latest f ON f.worker_id = d.worker_id
            ORDER BY d.tested_latest DESC
            LIMIT $2",
        )
        .bind(window_minutes.to_string())
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;
        let mut mapped: Vec<WorkerRateRow> = rows
            .into_iter()
            .map(|row| WorkerRateRow {
                worker_id: row.worker_id,
                hostname: row.hostname,
                search_type: row.search_type,
                rate: rate_from_samples(row.tested_latest, row.tested_prev, row.dt),
                tested: row.tested_latest,
                found: row.found,
            })
            .collect();
        mapped.sort_by(|a, b| {
            b.rate
                .partial_cmp(&a.rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if mapped.len() > limit as usize {
            mapped.truncate(limit as usize);
        }
        Ok(mapped)
    }

    /// Refresh all materialized views used by dashboard RPCs.
    ///
    /// Uses `CONCURRENTLY` to avoid locking reads during refresh.
    /// Called from the hourly housekeeping loop in the dashboard.
    pub async fn refresh_materialized_views(&self) -> Result<()> {
        sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY mv_dashboard_stats")
            .execute(&self.pool)
            .await?;
        sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY mv_form_leaderboard")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::rate_from_samples;

    #[test]
    fn rate_from_samples_handles_zero_dt() {
        assert_eq!(rate_from_samples(100.0, 50.0, 0.0), 0.0);
    }

    #[test]
    fn rate_from_samples_computes_rate() {
        let rate = rate_from_samples(150.0, 50.0, 10.0);
        assert!((rate - 10.0).abs() < 1e-6);
    }
}
