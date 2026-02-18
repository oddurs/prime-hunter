# Agents Roadmap

AI agent architecture: autonomous task execution, tool integration, orchestration, safety, and cost control.

**Key files:** `src/{dashboard,db}.rs`, `supabase/migrations/006_agents.sql`, `frontend/src/app/agents/page.tsx`, `frontend/src/hooks/use-agents.ts`

---

## Current State

Foundation layer is in place:
- **Schema:** `agent_tasks`, `agent_events`, `agent_budgets` tables in Supabase (migration 006)
- **Backend:** REST API for CRUD on tasks/events/budgets, WebSocket push of active tasks and budget summaries
- **Frontend:** Agents page with task list (filterable), activity feed (real-time), budget cards (editable), new task dialog
- **Data flow:** Supabase Realtime for live updates, WebSocket for coordination summary

What's missing: no agent actually runs. Tasks sit in `pending` and nothing picks them up. The entire execution layer — spawning agents, running tools, reporting progress, enforcing budgets — needs to be built.

---

## Phase 1: Agent Execution Engine

The core runtime that picks up tasks and executes them.

### Task Runner Daemon

**Current:** Tasks are created via the UI but never executed.

**Target:** Background service in the Rust dashboard process that polls for `pending` tasks, spawns agent processes, and manages their lifecycle.

```rust
// Pseudocode for the task runner loop
loop {
    if active_agents < max_concurrency && budget_remaining() > 0 {
        if let Some(task) = claim_next_task().await {
            spawn_agent(task).await;
        }
    }
    reap_completed_agents().await;
    sleep(5s).await;
}
```

**Key decisions:**
- **Subprocess model:** Each agent task spawns a Claude Code CLI process (`claude --task "..." --model <model>`). This isolates agents from each other and from the coordinator.
- **Concurrency limit:** Configurable `max_agents` (default: 2). Prevents runaway spending and resource exhaustion.
- **Task claiming:** Use `FOR UPDATE SKIP LOCKED` pattern (same as work blocks) so multiple coordinators don't double-claim.

### Agent Process Management

**Target:** Wrap each agent in a managed subprocess with:
- **Stdin/stdout streaming:** Capture agent output in real-time, feed it into `agent_events`
- **Timeout enforcement:** Kill agents that exceed a configurable wall-clock limit (default: 30 minutes)
- **Graceful shutdown:** Send SIGTERM, wait 10s, then SIGKILL
- **Exit code handling:** Map exit codes to task status (`0` = completed, non-zero = failed)

```
agent_tasks.status lifecycle:
  pending → in_progress → completed
                        → failed (non-zero exit, timeout, budget exceeded)
                        → cancelled (user action or budget kill)
```

### Event Streaming

**Current:** Events are only written on task creation and cancellation.

**Target:** Stream agent activity into `agent_events` in real-time:
- Parse structured output from the agent process (JSON lines or delimited format)
- Emit events for: `started`, `tool_call` (with tool name and args), `message` (agent thinking/output), `error`, `completed`/`failed`
- Rate-limit event inserts (batch every 2s) to avoid overwhelming the database

### Schema Changes

```sql
-- Add to agent_tasks
ALTER TABLE agent_tasks ADD COLUMN timeout_secs INTEGER NOT NULL DEFAULT 1800;
ALTER TABLE agent_tasks ADD COLUMN max_cost_usd NUMERIC(10,2);
ALTER TABLE agent_tasks ADD COLUMN working_directory TEXT;
ALTER TABLE agent_tasks ADD COLUMN pid INTEGER;

-- Add to agent_events
ALTER TABLE agent_events ADD COLUMN tool_name TEXT;
ALTER TABLE agent_events ADD COLUMN duration_ms INTEGER;
ALTER TABLE agent_events ADD COLUMN tokens INTEGER;
```

---

## Phase 2: Tool & Permission Framework

Agents need tools to interact with the codebase and infrastructure, with guardrails.

### Tool Registry

**Target:** Define which tools an agent can use based on task configuration:

| Tool | Description | Risk | Default |
|------|-------------|------|---------|
| `read_file` | Read any file in the project | Low | Allowed |
| `write_file` | Write/edit files | Medium | Allowed |
| `bash` | Execute shell commands | High | Restricted |
| `search` | Grep/glob the codebase | Low | Allowed |
| `git` | Git operations (commit, branch, push) | High | Restricted |
| `web_search` | Search the internet | Low | Allowed |
| `supabase` | Direct database queries | High | Restricted |
| `deploy` | Trigger deployments | Critical | Denied |

### Permission Levels

```
Level 0 (read-only):  read_file, search, web_search
Level 1 (standard):   Level 0 + write_file, bash (sandboxed)
Level 2 (trusted):    Level 1 + git (branch only, no push to main), bash (unrestricted)
Level 3 (admin):      Level 2 + deploy, supabase, git push
```

Agents default to Level 1. Task creator can escalate in the UI. Level 3 requires explicit confirmation.

### Sandbox for Bash Execution

**Target:** Agents running bash commands operate in a restricted environment:
- Working directory locked to the project root (no `cd /` escapes)
- No network access except to allowed hosts (localhost, supabase URL)
- Time limit per command (60s default)
- Output capture with size limits (1MB)
- Blocklist: `rm -rf /`, `sudo`, `curl | sh`, `docker run`, destructive git operations

### Git Safety

Agents that need git access:
- Always work on a branch (`agent/<task-id>-<slug>`)
- Never commit directly to `main` or `master`
- Never force-push
- Commits tagged with `Co-Authored-By: Agent <model>` and task ID in the message
- PR creation requires Level 2+

---

## Phase 3: Task Decomposition & Orchestration

Move from single-task execution to multi-step, multi-agent workflows.

### Subtask System

**Current:** `parent_task_id` column exists but is unused.

**Target:** Tasks can spawn child tasks, creating a tree:
```
Task #1: "Implement feature X"
├── Task #2: "Research existing patterns" (agent: haiku, read-only)
├── Task #3: "Write implementation" (agent: sonnet, standard)
│   ├── Task #5: "Write unit tests" (agent: haiku, standard)
│   └── Task #6: "Fix lint errors" (agent: haiku, standard)
└── Task #4: "Create PR" (agent: sonnet, trusted)
```

**Execution rules:**
- Child tasks inherit the parent's permission level (can be reduced, never escalated)
- Parent task waits for all children to complete before its own completion
- If any child fails, parent can retry, skip, or fail (configurable strategy)
- Cost rolls up: parent's `cost_usd` includes all children

### Task Templates

**Target:** Pre-defined task templates for common workflows:

| Template | Steps | Models |
|----------|-------|--------|
| `implement-feature` | Research → Plan → Implement → Test → PR | haiku → sonnet → sonnet → haiku → sonnet |
| `fix-bug` | Reproduce → Diagnose → Fix → Test | haiku → sonnet → sonnet → haiku |
| `code-review` | Read PR → Analyze → Comment | sonnet (single step) |
| `run-search` | Configure → Start search → Monitor → Report | haiku → haiku → haiku → haiku |
| `update-docs` | Read code changes → Update docs → PR | haiku → sonnet → sonnet |

Templates are stored as JSON in a `agent_templates` table and selectable in the New Task dialog.

### Dependency Graph

**Target:** Tasks can declare dependencies beyond parent-child:

```sql
CREATE TABLE agent_task_deps (
  task_id BIGINT REFERENCES agent_tasks(id) ON DELETE CASCADE,
  depends_on BIGINT REFERENCES agent_tasks(id) ON DELETE CASCADE,
  PRIMARY KEY (task_id, depends_on)
);
```

The scheduler respects the dependency graph: a task only becomes runnable when all `depends_on` tasks are completed.

---

## Phase 4: Cost Control & Budget Enforcement

Hard limits on agent spending to prevent runaway costs.

### Real-Time Token Tracking

**Current:** `tokens_used` and `cost_usd` on tasks exist but are never updated during execution.

**Target:** Agent processes report token usage in real-time:
- Parse usage from Claude API response metadata (or from claude CLI `--usage` output)
- Update `agent_tasks.tokens_used` and `cost_usd` every event batch (2s)
- Roll up to `agent_budgets.spent_usd` and `tokens_used`

### Budget Enforcement

```
Before claiming a task:
  1. Check daily budget: spent_usd < budget_usd
  2. Check task max_cost: task.cost_usd < task.max_cost_usd
  3. Estimate remaining cost (tokens_used * rate * estimated_remaining_fraction)

During execution:
  4. After each event batch, re-check budgets
  5. If daily budget exceeded → pause all agents, notify user
  6. If task max_cost exceeded → kill that agent, mark task failed with "budget_exceeded"
```

### Budget Period Rotation

**Target:** Automatic budget period reset:
```sql
-- Run on schedule (cron or background task)
UPDATE agent_budgets
SET spent_usd = 0, tokens_used = 0,
    period_start = date_trunc(period, NOW()),
    updated_at = NOW()
WHERE period_start < date_trunc(period, NOW());
```

### Cost Estimation

**Target:** Before starting a task, estimate its cost based on:
- Task description length and complexity heuristic
- Historical data: average cost per template type
- Model pricing: opus (~$15/MTok input, $75/MTok output), sonnet (~$3/$15), haiku (~$0.25/$1.25)

Display estimated cost in the New Task dialog. Warn if it exceeds remaining daily budget.

### Pricing Configuration

```sql
CREATE TABLE agent_pricing (
  model TEXT PRIMARY KEY,
  input_cost_per_mtok NUMERIC(10,4) NOT NULL,
  output_cost_per_mtok NUMERIC(10,4) NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO agent_pricing VALUES
  ('opus',   15.00, 75.00, NOW()),
  ('sonnet',  3.00, 15.00, NOW()),
  ('haiku',   0.25,  1.25, NOW());
```

---

## Phase 5: Context & Memory

Give agents persistent knowledge across tasks.

### Project Context Injection

**Target:** Every agent task automatically receives:
- `CLAUDE.md` and domain-specific `CLAUDE.md` files
- Relevant roadmap sections based on task type
- Recent git log (last 20 commits)
- Summary of currently running searches and fleet status

This is assembled into a context document and passed to the agent process as a system prompt file.

### Agent Memory Store

**Target:** Agents can write and read persistent notes:

```sql
CREATE TABLE agent_memory (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  key TEXT NOT NULL UNIQUE,
  value TEXT NOT NULL,
  category TEXT NOT NULL DEFAULT 'general',
  created_by_task BIGINT REFERENCES agent_tasks(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_agent_memory_category ON agent_memory (category);
```

Categories: `pattern`, `convention`, `gotcha`, `preference`, `architecture`.

Agents can `READ_MEMORY(category)` and `WRITE_MEMORY(key, value, category)` as tool calls. Memory persists across tasks — patterns discovered by one agent are available to all future agents.

### Task History Context

**Target:** When an agent starts a task, it receives a summary of:
- The parent task's description and result (if subtask)
- Sibling task results (if part of a template workflow)
- Previous failed attempts at the same task (if retried)
- Related completed tasks (matched by title/description similarity)

---

## Phase 6: Specialized Agent Roles

Purpose-built agent configurations for different domains.

### Engine Agent

**Role:** Algorithm implementation and optimization.

**Context:** Loaded with `src/CLAUDE.md`, engine roadmap, all test files, and a cheatsheet of rug/GMP patterns.

**Tools:** Level 2 (read, write, bash, git branch). Access to `cargo test`, `cargo bench`.

**Templates:**
- `implement-prime-form`: Research form → Write module → Add tests → Wire into main.rs → PR
- `optimize-sieve`: Profile current sieve → Identify bottleneck → Implement improvement → Benchmark → PR

### Frontend Agent

**Role:** UI features and components.

**Context:** `frontend/CLAUDE.md`, component inventory, existing page patterns.

**Tools:** Level 2. Access to `npm run build`, `npm run lint`.

**Templates:**
- `add-page`: Create hook → Create page → Add nav link → Build check → PR
- `add-component`: Create component → Add to page → Build check → PR

### Ops Agent

**Role:** Deployment and infrastructure.

**Context:** `deploy/CLAUDE.md`, ops roadmap, server inventory.

**Tools:** Level 3 (admin — can trigger deployments).

**Templates:**
- `deploy-update`: Build release → Deploy to server → Verify health → Report
- `scale-fleet`: Analyze current load → Recommend scaling → Deploy workers

### Research Agent

**Role:** Competitive analysis, discovery strategy, documentation.

**Context:** `docs/CLAUDE.md`, research roadmap, OEIS data, publication standards.

**Tools:** Level 0 (read-only + web search). Never writes code.

**Templates:**
- `research-form`: Survey literature → Check OEIS → Estimate ROI → Write summary
- `analyze-results`: Query Supabase → Compute statistics → Generate report

---

## Phase 7: Scheduling & Automation

Move from manual task creation to automated, event-driven agent work.

### Cron-Style Scheduling

**Target:** Recurring tasks that run on a schedule:

```sql
CREATE TABLE agent_schedules (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  name TEXT NOT NULL,
  template TEXT NOT NULL,
  cron_expr TEXT NOT NULL,       -- '0 9 * * *' = daily at 9AM
  params JSONB NOT NULL DEFAULT '{}',
  enabled BOOLEAN NOT NULL DEFAULT true,
  last_run_at TIMESTAMPTZ,
  next_run_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

**Example schedules:**
| Name | Cron | Template | Purpose |
|------|------|----------|---------|
| Daily results digest | `0 9 * * *` | `analyze-results` | Summarize yesterday's discoveries |
| Weekly code review | `0 10 * * 1` | `code-review` | Review uncommitted changes |
| Nightly test suite | `0 2 * * *` | `run-tests` | Full test suite with report |
| Monthly competitive scan | `0 12 1 * *` | `research-form` | Check PrimeGrid/GIMPS progress |

### Event-Driven Triggers

**Target:** Agents can be triggered by system events:

| Event | Trigger | Agent Action |
|-------|---------|-------------|
| Prime discovered | `INSERT` on `primes` table | Verify prime, update docs, post notification |
| Search job completed | `work_blocks` all done | Generate completion report, suggest next range |
| Worker offline > 10min | Stale heartbeat | Diagnose issue, attempt restart |
| Budget threshold (80%) | Budget check | Alert user, suggest cost optimizations |
| New commit pushed | Git webhook | Run tests, check for regressions |

### Self-Improving Loop

**Target:** Agents analyze their own performance and suggest improvements:
1. After each completed task, record: wall time, tokens used, cost, success/failure
2. Weekly analysis task identifies patterns: which templates are most efficient, which models are best for which tasks, common failure modes
3. Recommendations written to `agent_memory` for future agents to use

---

## Phase 8: Observability & Debugging

Deep visibility into what agents are doing and why.

### Agent Execution Timeline

**Frontend:** New visualization on the task detail view:
- Horizontal timeline showing each event (tool calls, messages, errors)
- Token usage sparkline overlaid on the timeline
- Expandable detail for each event (full tool call args, response, duration)

### Token Usage Analytics

**Frontend:** New section on the Budget tab:
- Cost per model over time (line chart)
- Cost per template type (bar chart)
- Token efficiency: output tokens per successful task completion
- Anomaly detection: flag tasks that used 3x+ the average tokens

### Agent Logs

**Target:** Full agent process output stored and queryable:

```sql
CREATE TABLE agent_logs (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  task_id BIGINT NOT NULL REFERENCES agent_tasks(id) ON DELETE CASCADE,
  stream TEXT NOT NULL CHECK (stream IN ('stdout', 'stderr')),
  content TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_agent_logs_task ON agent_logs (task_id, created_at);
```

**Frontend:** Log viewer on task detail page with:
- Stdout/stderr toggle
- Auto-scroll with pause
- Search within logs
- Download as text file

### Failure Analysis

**Target:** When a task fails:
1. Capture the last 50 events and full stderr
2. Auto-create a diagnosis event summarizing the failure
3. Suggest retry strategy based on failure type:
   - Timeout → Retry with longer timeout or split into subtasks
   - Budget exceeded → Retry with cheaper model
   - Tool error → Retry after fixing the underlying issue
   - Model error → Retry with different model

---

## Phase 9: Multi-Coordinator & Scaling

Scale the agent system beyond a single coordinator.

### Coordinator High Availability

**Target:** Multiple coordinator instances can run simultaneously:
- Task claiming already uses `FOR UPDATE SKIP LOCKED` — works with multiple coordinators
- Agent process spawned by the coordinator that claimed the task
- Heartbeat-based coordinator registry (similar to worker registry)
- If a coordinator dies, its in-progress tasks are reclaimed after timeout

### Agent Process Distribution

**Target:** Spawn agent processes on remote machines:
- Reuse the existing SSH deployment infrastructure (`deploy.rs`)
- Agent process runs on the remote machine, reports back to coordinator via PostgreSQL
- Useful for: running agents close to the code (local dev machine) while coordinator is in the cloud

### Rate Limiting & Fairness

**Target:** When multiple tasks compete for execution:
1. Priority queue: urgent > high > normal > low
2. Within same priority: FIFO by creation time
3. Model quotas: max N concurrent opus tasks (expensive), unlimited haiku
4. Starvation prevention: tasks waiting > 1 hour get priority boost

---

## Implementation Priority

| Phase | Effort | Impact | Dependencies |
|-------|--------|--------|-------------|
| 1. Execution engine | Large | **Critical** | None (schema exists) |
| 2. Tool & permissions | Medium | High | Phase 1 |
| 3. Task decomposition | Large | High | Phase 1 |
| 4. Cost control | Medium | **Critical** | Phase 1 |
| 5. Context & memory | Medium | High | Phase 1 |
| 6. Specialized roles | Medium | Medium | Phases 2, 3, 5 |
| 7. Scheduling & automation | Medium | High | Phases 1, 3 |
| 8. Observability | Medium | Medium | Phase 1 |
| 9. Multi-coordinator | Large | Low (until fleet grows) | All |

**Recommended order:** 1 → 4 → 2 → 5 → 3 → 8 → 7 → 6 → 9

Phase 1 and 4 are the critical path — nothing works without an execution engine, and nothing is safe without budget enforcement. Phase 2 (permissions) is essential before running agents on real code. Context injection (Phase 5) dramatically improves agent quality for minimal effort. Everything else builds on these foundations.

---

## Design Principles

1. **Agents are subprocesses, not threads.** Isolation prevents a rogue agent from crashing the coordinator or corrupting shared state.

2. **Budget is a hard limit, not a suggestion.** An agent that exceeds its budget is killed immediately. Operators must never be surprised by a bill.

3. **Least privilege by default.** Agents get read-only access unless explicitly granted more. Escalation requires human approval.

4. **Every action is auditable.** The `agent_events` table is the source of truth. If it's not in the event log, it didn't happen.

5. **Cheaper models first.** Use haiku for research/analysis, sonnet for implementation, opus only for complex architectural decisions. The scheduler should default to the cheapest model that can handle the task.

6. **Human in the loop for irreversible actions.** Push to remote, deploy to production, delete data — these always require human confirmation, regardless of agent permission level.

7. **Fail fast, retry smart.** Don't let agents spin for 30 minutes on something that's clearly broken. Detect failure patterns early and either retry with a different strategy or escalate to a human.
