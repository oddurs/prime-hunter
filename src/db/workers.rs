//! Worker coordination — heartbeat, registration, pruning.
//!
//! Workers connect to the coordinator via HTTP heartbeats. Each heartbeat
//! upserts a row in the `workers` table with the worker's current state
//! (tested count, found count, checkpoint, system metrics). Stale workers
//! (no heartbeat for 60s+) are pruned by the background task.

use super::{Database, WorkerRow};
use anyhow::Result;
use redis::AsyncCommands;
use serde_json::Value;

impl Database {
    /// Upsert a worker registration. Creates the row on first heartbeat,
    /// updates search state and timestamp on subsequent heartbeats.
    pub async fn upsert_worker(
        &self,
        worker_id: &str,
        hostname: &str,
        cores: i32,
        search_type: &str,
        search_params: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO workers (worker_id, hostname, cores, search_type, search_params, last_heartbeat)
             VALUES ($1, $2, $3, $4, $5, NOW())
             ON CONFLICT (worker_id) DO UPDATE SET
               hostname = EXCLUDED.hostname, cores = EXCLUDED.cores,
               search_type = EXCLUDED.search_type, search_params = EXCLUDED.search_params,
               last_heartbeat = NOW()",
        )
        .bind(worker_id)
        .bind(hostname)
        .bind(cores)
        .bind(search_type)
        .bind(search_params)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Full heartbeat via PostgreSQL RPC function `worker_heartbeat()`.
    ///
    /// The function atomically upserts the worker row and returns any pending
    /// command (e.g., "stop", "restart") which the worker should execute.
    /// After returning, the command is cleared server-side.
    pub async fn worker_heartbeat_rpc(
        &self,
        worker_id: &str,
        hostname: &str,
        cores: i32,
        search_type: &str,
        search_params: &str,
        tested: i64,
        found: i64,
        current: &str,
        checkpoint: Option<&str>,
        metrics: Option<&Value>,
    ) -> Result<Option<String>> {
        let command: Option<String> =
            sqlx::query_scalar("SELECT worker_heartbeat($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
                .bind(worker_id)
                .bind(hostname)
                .bind(cores)
                .bind(search_type)
                .bind(search_params)
                .bind(tested)
                .bind(found)
                .bind(current)
                .bind(checkpoint)
                .bind(metrics)
                .fetch_one(&self.pool)
                .await?;
        Ok(command)
    }

    /// Remove a worker from the registry (explicit disconnect).
    pub async fn delete_worker(&self, worker_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM workers WHERE worker_id = $1")
            .bind(worker_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Set a pending command for a worker (e.g., "stop", "restart").
    /// The command is delivered on the worker's next heartbeat.
    pub async fn set_worker_command(&self, worker_id: &str, command: &str) -> Result<()> {
        sqlx::query("UPDATE workers SET pending_command = $1 WHERE worker_id = $2")
            .bind(command)
            .bind(worker_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get all registered workers, ordered by worker_id.
    pub async fn get_all_workers(&self) -> Result<Vec<WorkerRow>> {
        let rows = sqlx::query_as::<_, WorkerRow>(
            "SELECT worker_id, hostname, cores, search_type, search_params,
                    tested, found, current, checkpoint, metrics,
                    registered_at, last_heartbeat
             FROM workers ORDER BY worker_id",
        )
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    /// Compute aggregated fleet capabilities from active workers.
    ///
    /// Workers are considered active if they heartbeated within the last 60 seconds.
    /// RAM is extracted from the `metrics` JSONB column (`total_memory_gb` field).
    pub async fn get_fleet_summary(&self) -> Result<super::FleetSummary> {
        let workers = self.get_all_workers().await?;
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(60);
        let active: Vec<_> = workers
            .iter()
            .filter(|w| w.last_heartbeat > cutoff)
            .collect();

        let worker_count = active.len() as u32;
        let total_cores: u32 = active.iter().map(|w| w.cores.max(0) as u32).sum();
        let max_ram_gb = active
            .iter()
            .filter_map(|w| {
                w.metrics
                    .as_ref()
                    .and_then(|m| m.get("total_memory_gb"))
                    .and_then(|v| v.as_f64())
                    .map(|gb| gb as u32)
            })
            .max()
            .unwrap_or(0);

        let mut search_types: Vec<String> = active
            .iter()
            .map(|w| w.search_type.clone())
            .filter(|s| !s.is_empty())
            .collect();
        search_types.sort();
        search_types.dedup();

        Ok(super::FleetSummary {
            worker_count,
            total_cores,
            max_ram_gb,
            active_search_types: search_types,
        })
    }

    /// Delete workers whose last heartbeat is older than `timeout_secs`.
    /// Returns the number of pruned workers.
    ///
    /// When Redis is available, stale worker expiry is handled by Redis TTL
    /// and this method only prunes the PG shadow copies.
    pub async fn prune_stale_workers(&self, timeout_secs: i64) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM workers WHERE last_heartbeat < NOW() - ($1 || ' seconds')::interval",
        )
        .bind(timeout_secs.to_string())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    // ── Redis-backed worker operations ──────────────────────────────

    /// Write worker heartbeat to Redis (HSET + EXPIRE + SADD).
    ///
    /// Redis stores the real-time worker state with a 60-second TTL, providing
    /// automatic stale worker cleanup. The `workers:active` set tracks all
    /// live worker IDs for fast enumeration.
    pub async fn redis_heartbeat(
        &self,
        worker_id: &str,
        hostname: &str,
        cores: i32,
        search_type: &str,
        search_params: &str,
        tested: i64,
        found: i64,
        current: &str,
        checkpoint: Option<&str>,
        metrics: Option<&Value>,
    ) -> Result<()> {
        let mut conn = self
            .redis
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Redis not configured"))?
            .clone();
        let key = format!("worker:{}", worker_id);
        let now = chrono::Utc::now().to_rfc3339();
        let metrics_str = metrics
            .map(|m| serde_json::to_string(m).unwrap_or_default())
            .unwrap_or_default();
        let checkpoint_str = checkpoint.unwrap_or("");

        redis::pipe()
            .hset_multiple(
                &key,
                &[
                    ("worker_id", worker_id),
                    ("hostname", hostname),
                    ("search_type", search_type),
                    ("search_params", search_params),
                    ("current", current),
                    ("checkpoint", checkpoint_str),
                    ("last_heartbeat", &now),
                ],
            )
            .hset(&key, "cores", cores)
            .hset(&key, "tested", tested)
            .hset(&key, "found", found)
            .hset(&key, "metrics", &metrics_str)
            .expire(&key, 60)
            .sadd("workers:active", worker_id)
            .exec_async(&mut conn)
            .await?;

        Ok(())
    }

    /// Read all active workers from Redis.
    ///
    /// Enumerates the `workers:active` set, then HGETALL each worker hash.
    /// Workers whose keys have expired are automatically cleaned from the set.
    pub async fn redis_get_all_workers(&self) -> Result<Vec<WorkerRow>> {
        let mut conn = self
            .redis
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Redis not configured"))?
            .clone();

        let worker_ids: Vec<String> = conn.smembers("workers:active").await?;
        let mut workers = Vec::with_capacity(worker_ids.len());
        let mut expired = Vec::new();

        for wid in &worker_ids {
            let key = format!("worker:{}", wid);
            let exists: bool = conn.exists(&key).await?;
            if !exists {
                expired.push(wid.clone());
                continue;
            }
            let map: std::collections::HashMap<String, String> = conn.hgetall(&key).await?;
            if map.is_empty() {
                expired.push(wid.clone());
                continue;
            }

            let last_hb = map
                .get("last_heartbeat")
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(chrono::Utc::now);
            let metrics_val = map
                .get("metrics")
                .filter(|s| !s.is_empty())
                .and_then(|s| serde_json::from_str(s).ok());

            workers.push(WorkerRow {
                worker_id: map.get("worker_id").cloned().unwrap_or_default(),
                hostname: map.get("hostname").cloned().unwrap_or_default(),
                cores: map
                    .get("cores")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                search_type: map.get("search_type").cloned().unwrap_or_default(),
                search_params: map.get("search_params").cloned().unwrap_or_default(),
                tested: map
                    .get("tested")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                found: map
                    .get("found")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                current: map.get("current").cloned().unwrap_or_default(),
                checkpoint: map
                    .get("checkpoint")
                    .filter(|s| !s.is_empty())
                    .cloned(),
                metrics: metrics_val,
                registered_at: last_hb,
                last_heartbeat: last_hb,
            });
        }

        // Clean expired worker IDs from the active set
        if !expired.is_empty() {
            for wid in &expired {
                let _: Result<(), _> = conn.srem("workers:active", wid).await;
            }
        }

        Ok(workers)
    }

    /// Remove a worker from Redis (DEL hash + SREM from active set).
    pub async fn redis_delete_worker(&self, worker_id: &str) -> Result<()> {
        let mut conn = self
            .redis
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Redis not configured"))?
            .clone();
        let key = format!("worker:{}", worker_id);
        redis::pipe()
            .del(&key)
            .srem("workers:active", worker_id)
            .exec_async(&mut conn)
            .await?;
        Ok(())
    }
}
