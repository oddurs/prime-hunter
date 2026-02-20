-- Volunteer Computing Platform
--
-- Adds volunteer accounts, worker machine tracking, trust scoring, and credit ledger.
-- Supports the public work API (Phase 3) and verification pipeline (Phase 4).

-- ── Volunteer accounts ──────────────────────────────────────────
CREATE TABLE IF NOT EXISTS volunteers (
  id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  username     TEXT UNIQUE NOT NULL,
  email        TEXT UNIQUE NOT NULL,
  api_key      TEXT UNIQUE NOT NULL DEFAULT encode(gen_random_bytes(32), 'hex'),
  team         TEXT,
  credit       BIGINT DEFAULT 0,
  primes_found INTEGER DEFAULT 0,
  joined_at    TIMESTAMPTZ DEFAULT NOW(),
  last_seen    TIMESTAMPTZ
);

CREATE INDEX idx_volunteers_api_key ON volunteers(api_key);
CREATE INDEX idx_volunteers_username ON volunteers(username);

-- ── Volunteer worker machines ───────────────────────────────────
CREATE TABLE IF NOT EXISTS volunteer_workers (
  id             SERIAL PRIMARY KEY,
  volunteer_id   UUID REFERENCES volunteers(id) ON DELETE CASCADE,
  worker_id      TEXT UNIQUE NOT NULL,
  hostname       TEXT,
  cores          INTEGER,
  cpu_model      TEXT,
  registered_at  TIMESTAMPTZ DEFAULT NOW(),
  last_heartbeat TIMESTAMPTZ
);

CREATE INDEX idx_volunteer_workers_volunteer ON volunteer_workers(volunteer_id);

-- ── Trust scoring (adaptive replication) ────────────────────────
CREATE TABLE IF NOT EXISTS volunteer_trust (
  volunteer_id      UUID PRIMARY KEY REFERENCES volunteers(id) ON DELETE CASCADE,
  consecutive_valid INTEGER DEFAULT 0,
  total_valid       INTEGER DEFAULT 0,
  total_invalid     INTEGER DEFAULT 0,
  trust_level       SMALLINT DEFAULT 1  -- 0=untrusted, 1=new, 2=reliable, 3=trusted
);

-- ── Credit ledger ───────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS credit_log (
  id            SERIAL PRIMARY KEY,
  volunteer_id  UUID REFERENCES volunteers(id) ON DELETE CASCADE,
  block_id      INTEGER,
  credit        BIGINT NOT NULL,
  reason        TEXT NOT NULL,  -- 'block_completed', 'prime_discovered', 'verification'
  granted_at    TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_credit_log_volunteer ON credit_log(volunteer_id);

-- ── Link work blocks and primes to volunteers ───────────────────
ALTER TABLE work_blocks ADD COLUMN IF NOT EXISTS volunteer_id UUID REFERENCES volunteers(id);
ALTER TABLE work_blocks ADD COLUMN IF NOT EXISTS verified BOOLEAN DEFAULT FALSE;
ALTER TABLE work_blocks ADD COLUMN IF NOT EXISTS min_quorum SMALLINT DEFAULT 1;

ALTER TABLE primes ADD COLUMN IF NOT EXISTS volunteer_id UUID REFERENCES volunteers(id);
ALTER TABLE primes ADD COLUMN IF NOT EXISTS discoverer TEXT;

-- ── Leaderboard view ────────────────────────────────────────────
CREATE OR REPLACE VIEW volunteer_leaderboard AS
SELECT
  v.id,
  v.username,
  v.team,
  v.credit,
  v.primes_found,
  v.joined_at,
  v.last_seen,
  COALESCE(vt.trust_level, 1) AS trust_level,
  COUNT(DISTINCT vw.id) AS worker_count
FROM volunteers v
LEFT JOIN volunteer_trust vt ON vt.volunteer_id = v.id
LEFT JOIN volunteer_workers vw ON vw.volunteer_id = v.id
GROUP BY v.id, v.username, v.team, v.credit, v.primes_found, v.joined_at, v.last_seen, vt.trust_level
ORDER BY v.credit DESC;
