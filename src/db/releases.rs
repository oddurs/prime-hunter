use super::Database;
use anyhow::{anyhow, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize, sqlx::FromRow)]
pub struct WorkerReleaseRow {
    pub version: String,
    pub artifacts: serde_json::Value,
    pub notes: Option<String>,
    pub published_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct WorkerReleaseChannelRow {
    pub channel: String,
    pub version: String,
    pub rollout_percent: i32,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct WorkerReleaseEventRow {
    pub id: i64,
    pub channel: String,
    pub from_version: Option<String>,
    pub to_version: String,
    pub rollout_percent: i32,
    pub changed_by: Option<String>,
    pub changed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct WorkerReleaseAdoptionRow {
    pub worker_version: Option<String>,
    pub workers: i64,
}

impl Database {
    pub async fn upsert_worker_release(
        &self,
        version: &str,
        artifacts: &serde_json::Value,
        notes: Option<&str>,
        published_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<WorkerReleaseRow> {
        let row = sqlx::query_as::<_, WorkerReleaseRow>(
            "INSERT INTO worker_releases (version, artifacts, notes, published_at)
             VALUES ($1, $2, $3, COALESCE($4, NOW()))
             ON CONFLICT (version) DO UPDATE SET
               artifacts = EXCLUDED.artifacts,
               notes = EXCLUDED.notes,
               published_at = EXCLUDED.published_at
             RETURNING version, artifacts, notes, published_at, created_at",
        )
        .bind(version)
        .bind(artifacts)
        .bind(notes)
        .bind(published_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_worker_releases(&self, limit: i64) -> Result<Vec<WorkerReleaseRow>> {
        let rows = sqlx::query_as::<_, WorkerReleaseRow>(
            "SELECT version, artifacts, notes, published_at, created_at
             FROM worker_releases
             ORDER BY published_at DESC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_worker_release_channels(&self) -> Result<Vec<WorkerReleaseChannelRow>> {
        let rows = sqlx::query_as::<_, WorkerReleaseChannelRow>(
            "SELECT channel, version, rollout_percent, updated_at
             FROM worker_release_channels
             ORDER BY channel",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_worker_release_for_channel(
        &self,
        channel: &str,
    ) -> Result<Option<WorkerReleaseRow>> {
        let row = sqlx::query_as::<_, WorkerReleaseRow>(
            "SELECT r.version, r.artifacts, r.notes, r.published_at, r.created_at
             FROM worker_release_channels c
             JOIN worker_releases r ON r.version = c.version
             WHERE c.channel = $1",
        )
        .bind(channel)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_worker_release_by_version(
        &self,
        version: &str,
    ) -> Result<Option<WorkerReleaseRow>> {
        let row = sqlx::query_as::<_, WorkerReleaseRow>(
            "SELECT version, artifacts, notes, published_at, created_at
             FROM worker_releases
             WHERE version = $1",
        )
        .bind(version)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn resolve_worker_release_for_channel(
        &self,
        channel: &str,
        worker_id: Option<&str>,
    ) -> Result<Option<WorkerReleaseRow>> {
        let Some(current) = self.get_worker_release_for_channel(channel).await? else {
            return Ok(None);
        };
        let channels = self.list_worker_release_channels().await?;
        let Some(ch) = channels.into_iter().find(|c| c.channel == channel) else {
            return Ok(Some(current));
        };

        if ch.rollout_percent >= 100 || worker_id.is_none() {
            return Ok(Some(current));
        }

        if ch.rollout_percent <= 0 {
            if let Some(prev) = self
                .previous_version_for_channel(channel, &current.version)
                .await?
            {
                return Ok(Some(prev));
            }
            return Ok(Some(current));
        }

        let wid = worker_id.unwrap_or_default();
        let bucket = rollout_bucket(wid);
        if bucket < ch.rollout_percent as u8 {
            return Ok(Some(current));
        }

        if let Some(prev) = self
            .previous_version_for_channel(channel, &current.version)
            .await?
        {
            return Ok(Some(prev));
        }

        Ok(Some(current))
    }

    async fn previous_version_for_channel(
        &self,
        channel: &str,
        current_version: &str,
    ) -> Result<Option<WorkerReleaseRow>> {
        let event = sqlx::query_as::<_, WorkerReleaseEventRow>(
            "SELECT id, channel, from_version, to_version, rollout_percent, changed_by, changed_at
             FROM worker_release_events
             WHERE channel = $1 AND to_version = $2
             ORDER BY id DESC
             LIMIT 1",
        )
        .bind(channel)
        .bind(current_version)
        .fetch_optional(&self.pool)
        .await?;

        let Some(event) = event else {
            return Ok(None);
        };
        let Some(prev) = event.from_version else {
            return Ok(None);
        };
        if prev == current_version {
            return Ok(None);
        }
        self.get_worker_release_by_version(&prev).await
    }

    pub async fn set_worker_release_channel(
        &self,
        channel: &str,
        version: &str,
        rollout_percent: i32,
        changed_by: Option<&str>,
    ) -> Result<WorkerReleaseChannelRow> {
        if !(0..=100).contains(&rollout_percent) {
            return Err(anyhow!("rollout_percent must be between 0 and 100"));
        }

        let mut tx = self.pool.begin().await?;

        let exists: Option<(String,)> =
            sqlx::query_as("SELECT version FROM worker_releases WHERE version = $1")
                .bind(version)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Err(anyhow!("unknown worker release version: {}", version));
        }

        let current: Option<(String,)> =
            sqlx::query_as("SELECT version FROM worker_release_channels WHERE channel = $1")
                .bind(channel)
                .fetch_optional(&mut *tx)
                .await?;
        let from_version = current.map(|v| v.0);

        let row = sqlx::query_as::<_, WorkerReleaseChannelRow>(
            "INSERT INTO worker_release_channels (channel, version, rollout_percent, updated_at)
             VALUES ($1, $2, $3, NOW())
             ON CONFLICT (channel) DO UPDATE SET
               version = EXCLUDED.version,
               rollout_percent = EXCLUDED.rollout_percent,
               updated_at = NOW()
             RETURNING channel, version, rollout_percent, updated_at",
        )
        .bind(channel)
        .bind(version)
        .bind(rollout_percent)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO worker_release_events
             (channel, from_version, to_version, rollout_percent, changed_by)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(channel)
        .bind(from_version)
        .bind(version)
        .bind(rollout_percent)
        .bind(changed_by)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(row)
    }

    pub async fn rollback_worker_release_channel(
        &self,
        channel: &str,
        changed_by: Option<&str>,
    ) -> Result<WorkerReleaseChannelRow> {
        let latest = sqlx::query_as::<_, WorkerReleaseEventRow>(
            "SELECT id, channel, from_version, to_version, rollout_percent, changed_by, changed_at
             FROM worker_release_events
             WHERE channel = $1
             ORDER BY id DESC
             LIMIT 1",
        )
        .bind(channel)
        .fetch_optional(&self.pool)
        .await?;

        let Some(event) = latest else {
            return Err(anyhow!("no rollout history for channel {}", channel));
        };

        let Some(prev) = event.from_version else {
            return Err(anyhow!(
                "channel {} has no previous version to roll back to",
                channel
            ));
        };

        self.set_worker_release_channel(channel, &prev, event.rollout_percent, changed_by)
            .await
    }

    pub async fn list_worker_release_events(
        &self,
        channel: Option<&str>,
        limit: i64,
    ) -> Result<Vec<WorkerReleaseEventRow>> {
        if let Some(channel) = channel {
            let rows = sqlx::query_as::<_, WorkerReleaseEventRow>(
                "SELECT id, channel, from_version, to_version, rollout_percent, changed_by, changed_at
                 FROM worker_release_events
                 WHERE channel = $1
                 ORDER BY id DESC
                 LIMIT $2",
            )
            .bind(channel)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
            return Ok(rows);
        }
        let rows = sqlx::query_as::<_, WorkerReleaseEventRow>(
            "SELECT id, channel, from_version, to_version, rollout_percent, changed_by, changed_at
             FROM worker_release_events
             ORDER BY id DESC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn worker_release_adoption(
        &self,
        active_within_hours: i64,
    ) -> Result<Vec<WorkerReleaseAdoptionRow>> {
        let rows = sqlx::query_as::<_, WorkerReleaseAdoptionRow>(
            "SELECT worker_version, COUNT(*)::bigint AS workers
             FROM volunteer_workers
             WHERE last_heartbeat IS NOT NULL
               AND last_heartbeat >= NOW() - ($1 || ' hours')::interval
             GROUP BY worker_version
             ORDER BY workers DESC, worker_version",
        )
        .bind(active_within_hours.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

fn rollout_bucket(worker_id: &str) -> u8 {
    let mut h = Sha256::new();
    h.update(worker_id.as_bytes());
    let digest = h.finalize();
    (digest[0] % 100) as u8
}

#[cfg(test)]
mod tests {
    use super::rollout_bucket;

    #[test]
    fn rollout_bucket_stable_and_bounded() {
        let a = rollout_bucket("worker-a");
        let b = rollout_bucket("worker-a");
        let c = rollout_bucket("worker-b");
        assert_eq!(a, b);
        assert!(a < 100);
        assert!(c < 100);
    }
}
