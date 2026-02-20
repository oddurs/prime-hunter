-- Agent Observability: extend agent_events with queryable columns, add agent_logs table,
-- and expand event types for tool_result and diagnosis events.

-- 1. Extend agent_events with indexed columns for analytics
--    (data already exists in detail JSONB but is not efficiently queryable)
ALTER TABLE agent_events ADD COLUMN IF NOT EXISTS tool_name TEXT;
ALTER TABLE agent_events ADD COLUMN IF NOT EXISTS input_tokens BIGINT;
ALTER TABLE agent_events ADD COLUMN IF NOT EXISTS output_tokens BIGINT;
ALTER TABLE agent_events ADD COLUMN IF NOT EXISTS duration_ms BIGINT;

CREATE INDEX IF NOT EXISTS idx_agent_events_tool ON agent_events (tool_name) WHERE tool_name IS NOT NULL;

-- 2. Expand event_type constraint to include tool_result and diagnosis
ALTER TABLE agent_events DROP CONSTRAINT agent_events_event_type_check;
ALTER TABLE agent_events ADD CONSTRAINT agent_events_event_type_check
  CHECK (event_type IN (
    'created','started','completed','failed','cancelled','message',
    'tool_call','tool_result','error','claimed','budget_exceeded',
    'parent_completed','parent_failed','diagnosis'
  ));

-- 3. Create agent_logs table for full transcript replay
CREATE TABLE IF NOT EXISTS agent_logs (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  task_id BIGINT NOT NULL REFERENCES agent_tasks(id) ON DELETE CASCADE,
  stream TEXT NOT NULL CHECK (stream IN ('stdout', 'stderr')),
  line_num INTEGER NOT NULL,
  msg_type TEXT,
  content TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_agent_logs_task ON agent_logs (task_id, line_num);
