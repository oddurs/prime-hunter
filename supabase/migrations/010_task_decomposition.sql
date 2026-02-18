-- Phase 3: Task Decomposition & Orchestration
-- Adds template-based task expansion, subtask dependencies, and parent auto-completion.

-- agent_templates: Predefined multi-step workflow templates.
-- Each template defines an ordered list of steps in JSONB. When expanded,
-- each step becomes a child agent_task linked to a parent via parent_task_id.
CREATE TABLE agent_templates (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  description TEXT NOT NULL DEFAULT '',
  steps JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- agent_task_deps: Directed dependency graph between sibling tasks.
-- A task cannot be claimed until all its dependencies are completed.
-- Used by claim_pending_agent_task to enforce execution ordering.
CREATE TABLE agent_task_deps (
  task_id BIGINT NOT NULL REFERENCES agent_tasks(id) ON DELETE CASCADE,
  depends_on BIGINT NOT NULL REFERENCES agent_tasks(id) ON DELETE CASCADE,
  PRIMARY KEY (task_id, depends_on),
  CHECK (task_id != depends_on)
);

-- New columns on agent_tasks for template workflows
ALTER TABLE agent_tasks ADD COLUMN template_name TEXT REFERENCES agent_templates(name);
ALTER TABLE agent_tasks ADD COLUMN on_child_failure TEXT NOT NULL DEFAULT 'fail'
  CHECK (on_child_failure IN ('fail', 'skip'));

-- Extend event_type to include parent lifecycle events
ALTER TABLE agent_events DROP CONSTRAINT agent_events_event_type_check;
ALTER TABLE agent_events ADD CONSTRAINT agent_events_event_type_check
  CHECK (event_type IN (
    'created','started','completed','failed','cancelled','message',
    'tool_call','error','claimed','budget_exceeded','parent_completed','parent_failed'
  ));

-- Seed 5 predefined workflow templates

INSERT INTO agent_templates (name, description, steps) VALUES
  ('implement-feature',
   'Full feature implementation: analyze, implement, test, review',
   '[
     {"title": "Analyze requirements", "description": "Read relevant code, understand the feature scope, identify files to change", "permission_level": 0},
     {"title": "Implement changes", "description": "Write the code changes following project conventions", "permission_level": 1, "depends_on_step": 0},
     {"title": "Run tests", "description": "Run cargo test and fix any failures", "permission_level": 1, "depends_on_step": 1},
     {"title": "Self-review", "description": "Review the changes for correctness, style, and documentation", "permission_level": 0, "depends_on_step": 2}
   ]'::jsonb),

  ('fix-bug',
   'Bug fix workflow: investigate root cause, fix, verify',
   '[
     {"title": "Investigate root cause", "description": "Read code and logs to identify the bug''s root cause", "permission_level": 0},
     {"title": "Implement fix", "description": "Write the minimal fix for the identified root cause", "permission_level": 1, "depends_on_step": 0},
     {"title": "Verify fix", "description": "Run tests and verify the bug is resolved without regressions", "permission_level": 1, "depends_on_step": 1}
   ]'::jsonb),

  ('code-review',
   'Code review workflow: read code, analyze quality, write review',
   '[
     {"title": "Read code", "description": "Read all relevant files and understand the changes", "permission_level": 0},
     {"title": "Analyze quality", "description": "Check for correctness, performance, security, and style issues", "permission_level": 0, "depends_on_step": 0},
     {"title": "Write review", "description": "Produce a structured code review with findings and suggestions", "permission_level": 0, "depends_on_step": 1}
   ]'::jsonb),

  ('run-search',
   'Prime search workflow: configure parameters, execute search, verify results',
   '[
     {"title": "Configure search", "description": "Determine optimal search parameters based on form and range", "permission_level": 0},
     {"title": "Execute search", "description": "Run the prime search with configured parameters", "permission_level": 1, "depends_on_step": 0},
     {"title": "Verify results", "description": "Re-verify any primes found using independent methods", "permission_level": 1, "depends_on_step": 1}
   ]'::jsonb),

  ('update-docs',
   'Documentation update: identify gaps, write docs, verify links',
   '[
     {"title": "Identify gaps", "description": "Scan existing documentation and identify outdated or missing sections", "permission_level": 0},
     {"title": "Write documentation", "description": "Update or create documentation for identified gaps", "permission_level": 1, "depends_on_step": 0},
     {"title": "Verify links", "description": "Check all links and references in the documentation are valid", "permission_level": 0, "depends_on_step": 1}
   ]'::jsonb);

-- Enable realtime for frontend subscriptions
ALTER PUBLICATION supabase_realtime ADD TABLE agent_templates;
ALTER PUBLICATION supabase_realtime ADD TABLE agent_task_deps;
