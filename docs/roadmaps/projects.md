# Projects Roadmap

Campaign-style prime discovery management: orchestration, cost tracking, records, strategy.

**Key files:** `src/project.rs`, `src/db.rs` (project methods), `src/dashboard.rs` (project API), `frontend/src/app/projects/page.tsx`

---

## Current State

The Projects feature provides TOML-defined campaigns with multi-phase orchestration, auto-strategy generation for all 12 forms (including verification objectives), cost estimation, world record tracking via t5k.org scraping, and a full frontend UI with phase timeline, cost tracker with live graph, and record comparison cards. 37 unit tests cover parsing, cost estimation, phase activation, conditions, verification strategy, and template validation. 13 template TOML files cover all forms.

**What works today:**
- TOML project definition with import via CLI and API
- Project CRUD (create, list, show, activate, pause, cancel)
- Phase state machine with dependency checking and activation conditions
- Phase skip, force-complete, and retry operations (API + CLI)
- Auto-strategy generation per form and objective (including verification)
- Cost estimation with per-form power-law timing model + preview endpoint
- Actual cost tracking from work block durations (updated every orchestration tick)
- Per-phase cost breakdown in project detail API
- Best prime linking to project
- Phase dependency validation (Kahn's algorithm for cycle detection)
- Expanded completion conditions: `all_blocks_done`, `first_prime_found`, `n_primes_found:N`, `target_digits_reached:D`, `timeout_hours:H`, `budget_reached:USD`
- Expanded activation conditions: `previous_phase_found_zero`, `previous_phase_found_prime`, `time_since_start:H`
- t5k.org record scraping (8 forms) with 24-hour refresh, retries, exponential backoff
- Event pruning (500 per project) + paginated events API
- WebSocket push of project/record summaries
- Frontend: project list, detail view, phase timeline, cost tracker, live cost graph, record comparison

---

## Tier 1: Critical Fixes (high impact, low effort)

### 1.1 Persist cost and core-hour tracking

**Current:** `total_cost_usd` and `total_core_hours` on the projects table are written at creation time but never updated during orchestration. Budget alerts read stale values. The frontend shows $0.00 for running projects.

**Target:** Compute actual cost from work block execution times and update the project row every orchestration tick.

```rust
// In orchestrate_project():
// 1. Sum wall-clock time from completed work_blocks for all project phases
// 2. Convert to core-hours: sum(block_duration_secs * cores_used) / 3600
// 3. Apply cloud rate: core_hours * cloud_rate_usd_per_core_hour
// 4. Update project: total_core_hours, total_cost_usd
```

**Requires:**
- New DB method `update_project_cost(project_id, total_cost_usd, total_core_hours)`
- Track `completed_at - claimed_at` on work_blocks (column may need adding)
- Cloud rate from project's `budget.cloud_rate_usd_per_core_hour` (default $0.04)

**Rationale:** Budget alerts and cost-based pause are the primary safety mechanism for long-running campaigns. Without actual cost tracking, users can't trust the budget system.

### 1.2 Link best prime to project

**Current:** `best_prime_id` and `best_digits` on the projects table are never populated. The project detail page shows best_digits=0 even when primes have been found.

**Target:** After updating phase progress in `orchestrate_project()`, query for the largest prime found by any phase's search job and update the project.

```rust
// In orchestrate_project(), after progress aggregation:
if let Some(best) = db.get_best_prime_for_form(&project.form).await? {
    db.update_project_best_prime(project.id, Some(best.id), best.digits).await?;
}
```

**Requires:** New DB method `update_project_best_prime(project_id, best_prime_id, best_digits)`.

### 1.3 Validate phase dependencies at creation time

**Current:** Phase `depends_on` arrays accept arbitrary strings. A typo like `depends_on = ["sweeep"]` creates a deadlock where the phase can never activate.

**Target:** In `create_project()`, validate that every `depends_on` reference points to another phase in the same project. Also detect circular dependencies (A depends on B depends on A).

```rust
fn validate_phase_graph(phases: &[PhaseConfig]) -> Result<()> {
    let names: HashSet<_> = phases.iter().map(|p| &p.name).collect();
    for phase in phases {
        for dep in phase.depends_on.as_deref().unwrap_or_default() {
            anyhow::ensure!(names.contains(dep),
                "Phase '{}' depends on unknown phase '{}'", phase.name, dep);
        }
    }
    // Topological sort to detect cycles
    // ...
}
```

**Rationale:** Silent deadlocks waste compute and confuse users. Fail fast at import time.

### 1.4 Add work_blocks duration tracking

**Current:** `work_blocks` has `claimed_at` and `completed_at` timestamps but no derived duration or core-count column. Cost estimation requires joining and computing elapsed time.

**Target:** Add a `duration_secs` column (or compute from timestamps) and `cores_used` to work_blocks, populated when a block completes.

**Rationale:** Foundation for accurate cost tracking (Tier 1.1) and per-phase cost breakdown (Tier 2.2).

---

## Tier 2: Operational Improvements (high impact, medium effort)

### 2.1 Phase skip and force-complete endpoints

**Current:** A stuck phase (bad search params, missing tool) blocks the entire project. The only option is to cancel the project.

**Target:** Add endpoints and CLI commands:
- `POST /api/projects/{slug}/phases/{phase_name}/skip` — mark phase as "skipped", unblock dependents
- `POST /api/projects/{slug}/phases/{phase_name}/force-complete` — mark as "completed" regardless of block status
- `POST /api/projects/{slug}/phases/{phase_name}/retry` — reset a failed phase to pending

```
darkreach project skip-phase <slug> <phase-name>
darkreach project force-complete-phase <slug> <phase-name>
darkreach project retry-phase <slug> <phase-name>
```

**Rationale:** Operators need escape hatches for long-running campaigns. Without these, any phase failure is a project-level failure.

### 2.2 Per-phase cost breakdown

**Current:** Cost is project-level only. Users can't see which phase is expensive.

**Target:** Compute and display per-phase cost:
- Join `project_phases → search_jobs → work_blocks`
- Sum `duration_secs * cores_used` per phase
- Apply project's cloud rate
- Return in `/api/projects/{slug}` response and display in frontend

**Frontend:** Add a cost column to the phase timeline component.

### 2.3 Cost estimate preview in new project dialog

**Current:** Users create a project and only see the cost estimate after. No way to preview cost before committing.

**Target:** Add a "Preview Cost" button in `NewProjectDialog` that calls `/api/projects/estimate` (new endpoint) with the form data and displays the `CostEstimate` inline.

```
POST /api/projects/estimate
Body: { form, objective, target, strategy }
Response: { estimated_candidates, total_core_hours, total_cost_usd,
            estimated_duration_hours, workers_recommended }
```

### 2.4 Calibrate cost model against actual measurements

**Current:** `secs_per_candidate()` uses hardcoded power-law curves that have never been validated against real timing data.

**Target:** After accumulating real execution data, fit timing curves per form:
1. Collect `(form, digit_count, wall_clock_secs, has_pfgw, has_gwnum)` from completed work blocks
2. Fit `secs = a * (digits/1000)^b` per form via least-squares
3. Store calibrated coefficients in the database
4. Fall back to hardcoded values when no data exists

**Rationale:** Accurate cost estimates are the foundation for budget planning. A 10x error in timing means a 10x error in cost.

### 2.5 Robust record scraping with retries and fallback

**Current:** `fetch_t5k_record()` makes a single HTTP request with no timeout, retry, or fallback. If t5k.org layout changes, parsing silently returns `None`.

**Target:**
- Add 10-second timeout to reqwest client
- Retry up to 3 times with exponential backoff on network errors
- Log warnings when HTML structure doesn't match expected format
- Cache last-known-good record so a scraping failure doesn't erase existing data
- Consider adding OEIS as a fallback source for digit counts

### 2.6 Event pruning and archival

**Current:** `project_events` table grows unbounded. A multi-month campaign could accumulate thousands of events per project.

**Target:**
- Keep only the most recent 500 events per project
- Archive older events to a `project_events_archive` table (or delete)
- Run pruning in the 30-second orchestration tick
- Add `GET /api/projects/{slug}/events?offset=N&limit=N` pagination

---

## Tier 3: Strategy and Intelligence (medium impact, high effort)

### 3.1 Verification objective auto-strategy

**Current:** `generate_auto_strategy()` has no specific logic for the "verification" objective. It falls through to the generic single-phase sweep.

**Target:** For verification projects:
1. Phase 1: Independent re-test of claimed primes using different algorithm (e.g., PFGW if original used GMP)
2. Phase 2: Generate deterministic proof if applicable
3. Phase 3: Cross-verify with third tool if digits > threshold

**Applies to:** factorial, primorial, kbn forms (where deterministic proofs exist).

### 3.2 Expanded completion and activation conditions

**Current:** Two completion conditions (`all_blocks_done`, `first_prime_found`) and two activation conditions (`previous_phase_found_zero`, `previous_phase_found_prime`).

**Target:** Add:

| Condition Type | Name | Semantics |
|----------------|------|-----------|
| Completion | `target_digits_reached` | Stop when a prime >= target digits is found |
| Completion | `n_primes_found` | Stop after finding N primes |
| Completion | `timeout_hours` | Stop after N hours elapsed |
| Completion | `budget_reached` | Stop when phase cost exceeds threshold |
| Activation | `previous_phase_cost_under` | Only activate if prior phase cost < $X |
| Activation | `time_since_start` | Wait N hours after project activation |

**Rationale:** Real campaigns need flexible stopping criteria. A record hunt should stop as soon as the target is reached, not after all blocks are processed.

### 3.3 Adaptive phase generation

**Current:** Auto-strategy generates phases at project creation time. Phase parameters are static.

**Target:** Allow the orchestration engine to dynamically generate new phases based on results:
- If a survey phase finds primes near the target digits, auto-generate a focused phase around that range
- If a sweep phase finds no primes, extend the range with a new phase
- If cost per prime is much higher than estimated, suggest pausing and re-evaluating

```rust
// In orchestrate_project(), after phase completion:
if should_generate_followup_phase(&project, &completed_phase) {
    let new_phase = generate_followup_phase(&project, &completed_phase);
    db.add_phase_to_project(project.id, &new_phase).await?;
    db.insert_project_event(project.id, "phase_generated",
        &format!("Auto-generated follow-up phase '{}'", new_phase.name), None).await?;
}
```

### 3.4 Infrastructure requirement enforcement

**Current:** The `[infrastructure]` section in TOML (`min_ram_gb`, `min_cores`, `required_tools`) is stored but never checked.

**Target:** At phase activation time, verify that at least one worker in the fleet meets the infrastructure requirements:
- Check worker heartbeats for RAM and core count
- Check available tools (PFGW, GWNUM) against `required_tools`
- If no suitable worker exists, log event and skip activation until one appears
- Optionally auto-assign phases to workers that meet requirements

### 3.5 Worker count auto-scaling

**Current:** The `[workers]` section (`min_workers`, `max_workers`, `recommended_workers`) is stored but never read.

**Target:** Use worker config to influence orchestration:
- Don't activate a phase unless `min_workers` are available in the fleet
- Set `block_size` inversely proportional to `recommended_workers` for even distribution
- Emit warnings if fewer than `recommended_workers` are active

---

## Tier 4: Frontend Polish (medium impact, medium effort)

### 4.1 Phase dependency visualization

**Current:** The phase timeline shows a vertical list with status icons. Dependencies are not visually represented.

**Target:** Draw dependency arrows between phases. Show why a pending phase is blocked (which dependency is unmet, what activation condition is waiting).

### 4.2 Live cost graph

**Current:** Cost tracker shows a single number and progress bar.

**Target:** Time-series chart of cumulative cost over the project lifetime:
- X-axis: time since project activation
- Y-axis: cumulative USD spent
- Horizontal line at budget limit
- Projected completion line based on current burn rate

### 4.3 Record comparison deep-dive

**Current:** Record comparison cards show our-best vs world-record as a progress bar.

**Target:** Expand to show:
- Gap in digits (how far to go)
- Estimated core-years to close the gap (using cost model)
- Link to t5k.org Top 20 page
- Historical record progression (if data available)

### 4.4 Project diff view

**Current:** No way to compare TOML source with runtime state.

**Target:** Show side-by-side diff of original TOML vs current state. Highlight phases that have been auto-generated or modified at runtime. Allow exporting current state back to TOML for version control.

### 4.5 Bulk project operations

**Current:** Actions (activate, pause, cancel) are per-project only.

**Target:** Multi-select in project list with bulk pause/activate/cancel. Useful for fleet-wide operations (e.g., pause all projects before maintenance).

---

## Tier 5: Advanced Features (high effort, strategic value)

### 5.1 Multi-project resource scheduling

**Current:** Each project operates independently. If multiple active projects compete for workers, they all generate work blocks and workers claim arbitrarily.

**Target:** Priority-based scheduling:
- Project priority field (1-10)
- Higher-priority projects get work blocks claimed first
- Fair-share scheduling: each project gets proportional worker time
- Preemption: high-priority project can steal blocks from low-priority

### 5.2 Project templates and cloning

**Current:** Templates are static TOML files. No way to clone an existing project with modifications.

**Target:**
- `darkreach project clone <slug> --name <new-name> [--range-start N]` — clone with overrides
- `POST /api/projects/{slug}/clone` — API equivalent
- Frontend "Clone" button on project detail page
- Template library in the UI with one-click creation

### 5.3 Cross-project analytics

**Current:** Each project tracks its own metrics. No aggregate view.

**Target:** Dashboard widget showing:
- Total primes found across all projects
- Total cost across all projects
- Discovery rate trends (primes/hour, primes/dollar)
- Form-level analytics (which forms are most productive)
- Leaderboard: most productive projects

### 5.4 Project notifications and webhooks

**Current:** Project events are logged to the database. No external notifications.

**Target:**
- Email/Slack/Discord notifications on key events (prime found, phase completed, budget exceeded, project completed)
- Webhook URL in project config: `POST` event payload to external URL
- Notification preferences per project in TOML `[notifications]` section

### 5.5 TOML version control integration

**Current:** TOML source is stored in the database but not synced with the filesystem.

**Target:**
- Watch `projects/` directory for TOML changes
- Auto-import updated TOML files (diff against stored version)
- Export running project state back to TOML
- Git integration: auto-commit TOML changes when project state changes

---

## Implementation Priority

| Item | Effort | Impact | Priority | Status |
|------|--------|--------|----------|--------|
| 1.1 Persist cost tracking | Low | Critical | **P0** | Done |
| 1.2 Link best prime | Low | High | **P0** | Done |
| 1.3 Validate phase deps | Low | High | **P0** | Done |
| 1.4 Work block duration | Low | High | **P0** | Done |
| 2.1 Phase skip/force | Medium | High | **P1** | Done |
| 2.2 Per-phase cost | Medium | High | **P1** | Done |
| 2.3 Cost preview dialog | Low | Medium | **P1** | Done |
| 2.5 Robust scraping | Low | Medium | **P1** | Done |
| 2.6 Event pruning | Low | Low | **P2** | Done |
| 3.2 Expanded conditions | Medium | High | **P2** | Done |
| 3.1 Verification strategy | Medium | Medium | **P2** | Done |
| 4.2 Live cost graph | Medium | Medium | **P2** | Done |
| 2.4 Calibrate cost model | Medium | Medium | **P2** | Done |
| 4.1 Dependency viz | Medium | Medium | **P2** | Done |
| 3.3 Adaptive phases | High | High | **P3** | Done |
| 3.4 Infra enforcement | Medium | Medium | **P3** | Done |
| 3.5 Worker auto-scaling | Medium | Medium | **P3** | Done |
| 4.3 Record deep-dive | Medium | Low | **P3** | Done |
| 4.4 Project diff view | Medium | Low | **P3** | Done |
| 4.5 Bulk operations | Low | Low | **P3** | Done |
| 5.1 Multi-project scheduling | High | High | **P4** | |
| 5.2 Templates and cloning | Medium | Medium | **P4** | |
| 5.3 Cross-project analytics | High | Medium | **P4** | |
| 5.4 Notifications/webhooks | High | Medium | **P4** | |
| 5.5 TOML version control | High | Low | **P4** | |
