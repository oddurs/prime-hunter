-- Worker release catalog and channel rollout control.
--
-- Enables DB-backed canary/ramp/rollback workflows for volunteer worker updates.

CREATE TABLE IF NOT EXISTS worker_releases (
  version       TEXT PRIMARY KEY,
  artifacts     JSONB NOT NULL,
  notes         TEXT,
  published_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS worker_release_channels (
  channel         TEXT PRIMARY KEY,
  version         TEXT NOT NULL REFERENCES worker_releases(version) ON DELETE RESTRICT,
  rollout_percent INTEGER NOT NULL DEFAULT 100 CHECK (rollout_percent >= 0 AND rollout_percent <= 100),
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS worker_release_events (
  id              BIGSERIAL PRIMARY KEY,
  channel         TEXT NOT NULL,
  from_version    TEXT,
  to_version      TEXT NOT NULL,
  rollout_percent INTEGER NOT NULL CHECK (rollout_percent >= 0 AND rollout_percent <= 100),
  changed_by      TEXT,
  changed_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_worker_release_events_channel_changed_at
  ON worker_release_events(channel, changed_at DESC);
