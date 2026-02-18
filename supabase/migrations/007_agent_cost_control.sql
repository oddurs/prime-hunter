-- Phase 4: Cost Control & Budget Enforcement

-- Per-task max cost limit (nullable = no limit)
ALTER TABLE agent_tasks ADD COLUMN max_cost_usd NUMERIC(10,2);

-- Fix event_type constraint to include 'claimed' and 'budget_exceeded'
ALTER TABLE agent_events DROP CONSTRAINT agent_events_event_type_check;
ALTER TABLE agent_events ADD CONSTRAINT agent_events_event_type_check
  CHECK (event_type IN ('created','started','completed','failed','cancelled','message','tool_call','error','claimed','budget_exceeded'));
