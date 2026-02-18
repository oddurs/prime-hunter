-- Agent memory: persistent key/value store for cross-task knowledge.
-- Agents write learned patterns, conventions, and gotchas here so future
-- agents inherit accumulated project knowledge at spawn time.

CREATE TABLE agent_memory (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  key TEXT NOT NULL UNIQUE,
  value TEXT NOT NULL,
  category TEXT NOT NULL DEFAULT 'general'
    CHECK (category IN ('pattern','convention','gotcha','preference','architecture','general')),
  created_by_task BIGINT REFERENCES agent_tasks(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_agent_memory_category ON agent_memory (category);

-- Enable realtime for the frontend memory UI
ALTER PUBLICATION supabase_realtime ADD TABLE agent_memory;
