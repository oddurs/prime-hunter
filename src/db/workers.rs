//! Worker coordination — heartbeat, registration, pruning.
//!
//! Workers connect to the coordinator via HTTP heartbeats. Each heartbeat
//! upserts a row in the `workers` table with the worker's current state
//! (tested count, found count, checkpoint, system metrics). Stale workers
//! (no heartbeat for 60s+) are pruned by the background task.

use anyhow::Result;
use serde_json::Value;
use super::{Database, WorkerRow};

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
        .fetch_all(&self.pool)
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
        let active: Vec<_> = workers.iter().filter(|w| w.last_heartbeat > cutoff).collect();

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
    pub async fn prune_stale_workers(&self, timeout_secs: i64) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM workers WHERE last_heartbeat < NOW() - ($1 || ' seconds')::interval",
        )
        .bind(timeout_secs.to_string())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
