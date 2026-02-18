-- Agent management tables for AI coding agents

-- agent_tasks: Task queue for AI agents
CREATE TABLE agent_tasks (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','in_progress','completed','failed','cancelled')),
  priority TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('low','normal','high','urgent')),
  agent_model TEXT,
  assigned_agent TEXT,
  source TEXT NOT NULL DEFAULT 'manual' CHECK (source IN ('manual','automated','agent')),
  result JSONB,
  tokens_used BIGINT NOT NULL DEFAULT 0,
  cost_usd NUMERIC(10,4) NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  started_at TIMESTAMPTZ,
  completed_at TIMESTAMPTZ,
  parent_task_id BIGINT REFERENCES agent_tasks(id)
);

CREATE INDEX idx_agent_tasks_status ON agent_tasks (status);
CREATE INDEX idx_agent_tasks_created ON agent_tasks (created_at DESC);

-- agent_events: Activity feed
CREATE TABLE agent_events (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  task_id BIGINT REFERENCES agent_tasks(id) ON DELETE CASCADE,
  event_type TEXT NOT NULL CHECK (event_type IN ('created','started','completed','failed','cancelled','message','tool_call','error')),
  agent TEXT,
  summary TEXT NOT NULL DEFAULT '',
  detail JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_agent_events_task ON agent_events (task_id);
CREATE INDEX idx_agent_events_created ON agent_events (created_at DESC);

-- agent_budgets: Spending limits and tracking
CREATE TABLE agent_budgets (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  period TEXT NOT NULL CHECK (period IN ('daily','weekly','monthly')),
  budget_usd NUMERIC(10,2) NOT NULL DEFAULT 10.00,
  spent_usd NUMERIC(10,4) NOT NULL DEFAULT 0,
  tokens_used BIGINT NOT NULL DEFAULT 0,
  period_start TIMESTAMPTZ NOT NULL DEFAULT date_trunc('day', now()),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- RLS
ALTER TABLE agent_tasks ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_budgets ENABLE ROW LEVEL SECURITY;

CREATE POLICY "auth_read_agent_tasks" ON agent_tasks FOR SELECT TO authenticated USING (true);
CREATE POLICY "auth_read_agent_events" ON agent_events FOR SELECT TO authenticated USING (true);
CREATE POLICY "auth_read_agent_budgets" ON agent_budgets FOR SELECT TO authenticated USING (true);
CREATE POLICY "auth_write_agent_tasks" ON agent_tasks FOR ALL TO authenticated USING (true) WITH CHECK (true);
CREATE POLICY "auth_write_agent_budgets" ON agent_budgets FOR ALL TO authenticated USING (true) WITH CHECK (true);

-- Realtime
ALTER PUBLICATION supabase_realtime ADD TABLE agent_tasks;
ALTER PUBLICATION supabase_realtime ADD TABLE agent_events;

-- Seed default budgets
INSERT INTO agent_budgets (period, budget_usd) VALUES
  ('daily', 10.00),
  ('weekly', 50.00),
  ('monthly', 150.00);
