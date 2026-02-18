-- Phase 2: Tool & Permission Framework
-- Adds permission_level to agent_tasks to control which tools each agent can use.
-- Level 0 = Read-Only, 1 = Standard (default), 2 = Trusted, 3 = Admin.

ALTER TABLE agent_tasks
  ADD COLUMN permission_level INTEGER NOT NULL DEFAULT 1
  CHECK (permission_level BETWEEN 0 AND 3);
