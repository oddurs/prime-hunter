-- Track worker binary version/channel for release adoption and canary observability.

ALTER TABLE volunteer_workers
  ADD COLUMN IF NOT EXISTS worker_version TEXT,
  ADD COLUMN IF NOT EXISTS update_channel TEXT;

CREATE INDEX IF NOT EXISTS idx_volunteer_workers_worker_version
  ON volunteer_workers(worker_version);
