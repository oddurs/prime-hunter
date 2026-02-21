-- 025_operator_rename.sql
--
-- Rename volunteer tables to operator tables (Phase 0).
--
-- This migration renames the core volunteer infrastructure to use "operator"
-- terminology, which better reflects the role of users who run compute nodes
-- in the darkreach fleet. Backward-compatibility views are created so that
-- existing queries referencing the old names continue to work.
--
-- Renames:
--   volunteers        → operators          (+ backward-compat view)
--   volunteer_workers  → operator_nodes     (+ backward-compat view)
--   volunteer_trust    → operator_trust     (+ backward-compat view)
--   credit_log         → operator_credits   (+ backward-compat view)
--   volunteer_leaderboard (view) → operator_leaderboard (+ backward-compat alias)
--
-- FK column names (e.g. volunteer_id) are kept as-is for Phase 0 to avoid
-- cascading changes across the Rust codebase and frontend queries.

BEGIN;

-- ============================================================
-- 1. Rename tables
-- ============================================================

ALTER TABLE volunteers RENAME TO operators;
ALTER TABLE volunteer_workers RENAME TO operator_nodes;
ALTER TABLE volunteer_trust RENAME TO operator_trust;
ALTER TABLE credit_log RENAME TO operator_credits;

-- ============================================================
-- 2. Rename indexes
-- ============================================================

ALTER INDEX idx_volunteers_api_key RENAME TO idx_operators_api_key;
ALTER INDEX idx_volunteers_username RENAME TO idx_operators_username;
ALTER INDEX idx_volunteer_workers_volunteer RENAME TO idx_operator_nodes_volunteer;
ALTER INDEX idx_credit_log_volunteer RENAME TO idx_operator_credits_volunteer;

-- ============================================================
-- 3. Drop the old leaderboard view (references old table names)
-- ============================================================

DROP VIEW IF EXISTS volunteer_leaderboard;

-- ============================================================
-- 4. Create the new operator_leaderboard view
-- ============================================================

CREATE VIEW operator_leaderboard AS
SELECT
  o.id,
  o.username,
  o.team,
  o.credit,
  o.primes_found,
  o.joined_at,
  o.last_seen,
  COALESCE(ot.trust_level, 1) AS trust_level,
  COUNT(DISTINCT on_node.id) AS worker_count
FROM operators o
LEFT JOIN operator_trust ot ON ot.volunteer_id = o.id
LEFT JOIN operator_nodes on_node ON on_node.volunteer_id = o.id
GROUP BY o.id, o.username, o.team, o.credit, o.primes_found, o.joined_at, o.last_seen, ot.trust_level
ORDER BY o.credit DESC;

-- ============================================================
-- 5. Backward-compatibility views (old names → new tables)
-- ============================================================

CREATE VIEW volunteers AS SELECT * FROM operators;
CREATE VIEW volunteer_workers AS SELECT * FROM operator_nodes;
CREATE VIEW volunteer_trust AS SELECT * FROM operator_trust;
CREATE VIEW credit_log AS SELECT * FROM operator_credits;
CREATE VIEW volunteer_leaderboard AS SELECT * FROM operator_leaderboard;

COMMIT;
