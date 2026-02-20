-- Migration 014: Agent Schedules
--
-- Adds scheduling and automation for the agent system. Schedules can be
-- triggered by cron expressions (e.g., "0 2 * * *" for daily at 2am) or
-- by events (e.g., "PrimeFound" fires when a prime is discovered).
--
-- Each schedule creates either a single task or expands a template when
-- its trigger condition is met. All seeded schedules start disabled.

CREATE TABLE agent_schedules (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  description TEXT NOT NULL DEFAULT '',
  enabled BOOLEAN NOT NULL DEFAULT true,

  -- Trigger: exactly one of cron_expr or event_filter must be set
  trigger_type TEXT NOT NULL CHECK (trigger_type IN ('cron', 'event')),
  cron_expr TEXT,                    -- e.g. "0 2 * * *" (daily 2am)
  event_filter TEXT,                 -- e.g. "PrimeFound", "SearchCompleted"

  -- What to do when triggered
  action_type TEXT NOT NULL CHECK (action_type IN ('task', 'template')),
  template_name TEXT REFERENCES agent_templates(name),
  role_name TEXT REFERENCES agent_roles(name),
  task_title TEXT NOT NULL,
  task_description TEXT NOT NULL DEFAULT '',
  priority TEXT NOT NULL DEFAULT 'normal',
  max_cost_usd NUMERIC(10,2),
  permission_level INTEGER NOT NULL DEFAULT 1 CHECK (permission_level BETWEEN 0 AND 3),

  -- Tracking
  fire_count INTEGER NOT NULL DEFAULT 0,
  last_fired_at TIMESTAMPTZ,
  last_checked_at TIMESTAMPTZ,

  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_agent_schedules_enabled ON agent_schedules (enabled) WHERE enabled = true;

-- Seed 7 example schedules (all disabled by default)
INSERT INTO agent_schedules (name, description, enabled, trigger_type, cron_expr, event_filter, action_type, template_name, role_name, task_title, task_description, priority, max_cost_usd, permission_level) VALUES
  ('nightly-analysis',
   'Run nightly analysis of search results and prime discoveries',
   false, 'cron', '0 2 * * *', NULL,
   'template', 'analyze-results', 'research',
   'Nightly Analysis', 'Analyze recent search results, prime discoveries, and throughput trends.', 'normal', 2.00, 0),

  ('weekly-sieve-tune',
   'Weekly sieve parameter optimization based on recent performance',
   false, 'cron', '0 4 * * 1', NULL,
   'template', 'optimize-sieve', 'engine',
   'Weekly Sieve Tune', 'Review sieve performance metrics and suggest parameter adjustments.', 'normal', 5.00, 1),

  ('daily-docs-update',
   'Daily documentation refresh from latest codebase changes',
   false, 'cron', '0 6 * * *', NULL,
   'template', 'update-docs', 'research',
   'Daily Docs Update', 'Update documentation to reflect recent code changes and discoveries.', 'low', 1.00, 1),

  ('monthly-fleet-review',
   'Monthly review of fleet utilization and scaling recommendations',
   false, 'cron', '0 3 1 * *', NULL,
   'template', 'scale-fleet', 'ops',
   'Monthly Fleet Review', 'Review fleet utilization, worker performance, and scaling needs.', 'normal', 5.00, 2),

  ('on-prime-verify',
   'Automatically re-verify newly discovered primes',
   false, 'event', NULL, 'PrimeFound',
   'task', NULL, 'engine',
   'Re-verify discovered prime', 'A new prime was discovered. Run independent re-verification to confirm the result.', 'high', 3.00, 1),

  ('on-search-complete-report',
   'Generate summary report when a search completes',
   false, 'event', NULL, 'SearchCompleted',
   'task', NULL, 'research',
   'Generate search summary', 'A search has completed. Generate a summary report of results, throughput, and notable findings.', 'normal', 1.00, 0),

  ('on-error-investigate',
   'Investigate errors automatically when they occur',
   false, 'event', NULL, 'Error',
   'task', NULL, 'ops',
   'Investigate error', 'An error was detected. Investigate the root cause and suggest remediation steps.', 'high', 3.00, 1);
