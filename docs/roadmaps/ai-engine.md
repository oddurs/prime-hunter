# AI Engine Roadmap

The autonomous intelligence layer that optimizes prime discovery end-to-end: form selection, sieve tuning, workforce allocation, research integration, and self-improvement.

**This is the pivotal differentiator.** GIMPS and PrimeGrid rely on human administrators to configure searches. darkreach's AI engine should autonomously decide what to search, how to search it, and how to allocate compute — producing better discoveries per dollar than any human could manually manage.

**Key files:** `src/strategy.rs`, `src/agent.rs`, `src/project/orchestration.rs`, `src/project/cost.rs`, `src/db/calibrations.rs`

**Related roadmaps:** [agents.md](agents.md) (execution infrastructure), [projects.md](projects.md) (campaign management), [engine.md](engine.md) (algorithms)

---

## Current State (Feb 2026)

### What exists

Three layers of AI autonomy are partially built:

| Layer | File | Status | What it does |
|-------|------|--------|-------------|
| **Agent Execution** | `src/agent.rs` | ~90% | Spawns Claude Code CLI subprocesses, context injection, domain detection, 4 permission levels, output streaming |
| **Strategy Engine** | `src/strategy.rs` | MVP | 5-component scoring model, survey→score→decide→execute pipeline, auto-project creation, stall detection, near-record verification |
| **Project Orchestration** | `src/project/orchestration.rs` | MVP | 30s tick loop, phase state machine, auto-strategy generation for all 12 forms, adaptive phases, cost tracking |

Supporting infrastructure:
- **Cost model** (`src/project/cost.rs`): Power-law timing curves per form, PFGW acceleration factors
- **Calibration** (`src/db/calibrations.rs`): Database-backed coefficient storage for cost model tuning
- **Strategy config** (`strategy_config` table): Enabled flag, budget limits, preferred/excluded forms, tick interval
- **Yield data** (`form_yield_rates` SQL view): Historical found/tested ratios per form
- **Record tracking** (`src/project/records.rs`): t5k.org scraping for world record comparison

### What's missing — the gap to full autonomy

| Gap | Impact | Current workaround |
|-----|--------|-------------------|
| **No task runner loop** | Agents never execute — tasks sit in `pending` forever | Manual intervention |
| **No daily budget enforcement** | Agents could run without spending limits | Not running agents |
| **Cost model uncalibrated** | Hardcoded power-law curves, never validated against real data | Estimates may be 10x off |
| **No second-order parameter tuning** | Strategy picks forms but doesn't optimize sieve depth, block size, or tool selection per form | Manual configuration |
| **No competitive awareness** | Strategy doesn't know what GIMPS/PrimeGrid are currently searching | Misses strategic niches |
| **No research integration** | Can't read papers, check OEIS, or discover new theoretical shortcuts | Human must feed insights |
| **No sieve optimization** | Fixed sieve depths, no adaptive tuning based on candidate survival rates | Suboptimal sieve/test ratio |
| **No auto-scaling** | Can't provision/deprovision nodes based on workload | Manual fleet management |
| **No discovery pipeline** | After finding a prime, no auto-verification, t5k submission, or publication | Manual post-discovery |
| **No learning loop** | Doesn't learn from past searches to improve future decisions | Static scoring weights |

---

## Phase 1: Task Runner & Budget Enforcement (Critical Path)

> **Priority: P0** — Nothing else works without this. Unblocks all subsequent phases.

### 1.1 Agent Task Runner Daemon

**Current:** Agent tasks created via UI sit in `pending` forever. Nothing picks them up.

**Target:** Background service in the dashboard process that polls for pending tasks, spawns Claude Code CLI subprocesses, streams output, and manages lifecycle.

```rust
// Background loop in dashboard startup
loop {
    if active_agents < MAX_AGENTS && daily_budget_remaining() > 0 {
        if let Some(task) = claim_next_task().await {  // FOR UPDATE SKIP LOCKED
            spawn_agent(task).await;
        }
    }
    reap_completed_agents().await;
    enforce_budgets().await;
    tokio::time::sleep(Duration::from_secs(5)).await;
}
```

**Key design:**
- Subprocess isolation: each agent is a `claude` process, can't crash the coordinator
- Task claiming uses `FOR UPDATE SKIP LOCKED` (same pattern as work blocks)
- Stdout streaming into `agent_events` table for real-time dashboard display
- Timeout enforcement: SIGTERM after configurable limit (default 30 min), SIGKILL after 10s grace
- Exit code mapping: 0 = completed, non-zero = failed, budget kill = cancelled

**Schema changes:**
```sql
ALTER TABLE agent_tasks ADD COLUMN pid INTEGER;
ALTER TABLE agent_tasks ADD COLUMN timeout_secs INTEGER NOT NULL DEFAULT 1800;
ALTER TABLE agent_tasks ADD COLUMN max_cost_usd NUMERIC(10,2);
```

### 1.2 Daily Budget Enforcement

**Current:** `agent_budgets` table exists but `spent_usd` is never updated during execution.

**Target:** Hard budget enforcement at two levels:

1. **Per-task limit**: Each task has `max_cost_usd`. Agent killed immediately on breach.
2. **Daily budget**: Sum of all task costs checked before claiming. If daily limit reached, no new tasks start. Running tasks continue but no new ones spawn.

**Token tracking:**
- Parse usage from Claude CLI `--usage` output (JSON lines with input/output token counts)
- Update `agent_tasks.tokens_used` and `cost_usd` every 2s event batch
- Roll up to `agent_budgets.spent_usd` in real-time

**Budget period rotation:** Automatic daily reset via background task:
```sql
UPDATE agent_budgets
SET spent_usd = 0, tokens_used = 0,
    period_start = CURRENT_DATE,
    updated_at = NOW()
WHERE period_start < CURRENT_DATE;
```

### 1.3 Model Cost Optimization

**Target:** Automatic model selection based on task complexity:

| Task type | Default model | When to escalate |
|-----------|--------------|-----------------|
| Research, OEIS lookup, log analysis | Haiku ($0.25/$1.25 per MTok) | Never |
| Code implementation, test writing | Sonnet ($3/$15) | If first attempt fails |
| Architecture, complex algorithm design | Opus ($15/$75) | Only by explicit request |

The scheduler estimates task complexity from description length, domain keywords, and historical model success rates for similar tasks, then assigns the cheapest model likely to succeed.

---

## Phase 2: Cost Model Calibration & Feedback Loop

> **Priority: P1** — Accurate cost estimates are the foundation for intelligent resource allocation.

### 2.1 Calibrate Power-Law Curves Against Real Data

**Current:** `secs_per_candidate()` in `cost.rs` uses hardcoded `(base, exponent)` pairs that have never been validated. A 10x error in timing means a 10x error in budget allocation.

**Target:** After accumulating real execution data, fit timing curves per form:

```
Data collection:
  For each completed work_block:
    Record (form, digit_count, wall_clock_secs, has_pfgw, has_gwnum, cores_used)

Curve fitting (offline, triggered after N new data points):
  For each form:
    Fit secs = a * (digits/1000)^b via least-squares on log-log scale
    Store (a, b) in calibrations table
    Calculate R² goodness-of-fit
    Fall back to hardcoded values when R² < 0.8 or N < 20

Auto-update:
  Re-fit every 1000 completed blocks or weekly, whichever comes first
```

**Database:**
```sql
-- Already exists in calibrations table, needs population
INSERT INTO cost_calibrations (form, has_pfgw, base_coeff, exponent, r_squared, sample_count, calibrated_at)
VALUES ('factorial', true, 0.05, 2.3, 0.94, 1523, NOW());
```

### 2.2 PFGW/GWNUM Acceleration Factor Measurement

**Current:** PFGW acceleration factor is hardcoded at 50x. Real acceleration depends on form, digit count, and hardware.

**Target:** Measure actual acceleration by running paired tests:
1. For each form, periodically test a candidate with both GMP and PFGW
2. Record the ratio as the empirical acceleration factor
3. Use the measured factor in cost estimates

This feeds directly into the strategy engine's cost_efficiency scoring component.

### 2.3 Node Performance Profiling

**Current:** All nodes assumed equal. A Raspberry Pi and a 64-core Ryzen get the same work blocks.

**Target:** Per-node performance profile:
- Measure throughput (candidates/second) during first work block
- Store in `workers` table as `benchmark_score`
- Strategy engine uses this to assign appropriately-sized blocks
- Heavier forms (factorial, wagstaff) routed to faster nodes

```sql
ALTER TABLE workers ADD COLUMN benchmark_score FLOAT8;
ALTER TABLE workers ADD COLUMN preferred_forms TEXT[];  -- forms this node is fast at
```

---

## Phase 3: Intelligent Workforce Allocation

> **Priority: P1** — The core of what makes AI-managed search superior to manual configuration.

### 3.1 Dynamic Form-to-Node Assignment

**Current:** Strategy engine creates projects and nodes claim blocks randomly. A 4-core laptop might grab a Wagstaff block that needs 64 cores.

**Target:** Multi-factor assignment algorithm:

```
For each idle node:
  1. Score each active form for this node:
     - hardware_fit: node.cores / form.min_cores (capped at 1.0)
     - tool_fit: 1.0 if node has PFGW/GWNUM, 0.5 otherwise
     - memory_fit: node.ram_gb / form.min_ram_gb (capped at 1.0)
     - locality: 1.0 if node recently ran this form (warm cache), 0.8 otherwise
  2. Multiply by form's strategy score
  3. Assign to highest-scoring form
  4. Size block inversely to digit count (big blocks for small n, small blocks for large n)
```

**Implementation:** New `assign_work()` method in `pg_worker.rs` that replaces blind `claim_next_block()`:
- Queries node's hardware profile
- Queries active forms with unclaimed blocks
- Scores and assigns optimally
- Falls back to FIFO if scoring data insufficient

### 3.2 Scale-Aware Project Portfolio

**Current:** Strategy engine creates one project at a time for the highest-scoring form.

**Target:** Portfolio optimization based on fleet size:

| Fleet size | Strategy |
|-----------|----------|
| 1-4 nodes | **Focus**: Single highest-ROI form. All compute on one project. |
| 5-16 nodes | **Diversify**: 2-3 concurrent projects. Primary form gets 60% of nodes, secondary 25%, exploratory 15%. |
| 17-64 nodes | **Full portfolio**: 4-6 concurrent projects. Weighted allocation matching strategy scores. Add verification projects for near-record results. |
| 65+ nodes | **Frontier mode**: Reserve 20% for record attempts, 60% for high-yield surveys, 20% for exploratory/research. |

The strategy engine adjusts `max_concurrent_projects` and per-project node allocation based on `worker_count`.

### 3.3 Adaptive Block Sizing

**Current:** Block sizes are hardcoded per form in auto-strategy (e.g., factorial blocks of 100 n-values).

**Target:** Dynamic block sizing based on:
1. **Estimated time per block**: Target 5-15 minutes per block (short enough for responsive reallocation, long enough to amortize overhead)
2. **Node speed**: Faster nodes get larger blocks
3. **Digit count**: Larger digits = fewer candidates per block
4. **Sieve survival rate**: If sieve eliminates 99%, block can be much larger

```rust
fn optimal_block_size(form: &str, digit_estimate: u64, node_benchmark: f64) -> u64 {
    let spc = calibrated_secs_per_candidate(form, digit_estimate);
    let target_secs = 600.0; // 10 minute target
    let candidates_per_block = (target_secs / spc * node_benchmark).max(10.0);
    candidates_per_block as u64
}
```

---

## Phase 4: Sieve & Algorithm Optimization

> **Priority: P2** — The AI should optimize not just *what* to search but *how* to search it.

### 4.1 Adaptive Sieve Depth Tuning

**Current:** Sieve depth is fixed at `SIEVE_LIMIT = 10_000_000` for all forms and all digit ranges.

**Target:** Auto-tune sieve depth based on the crossover condition:

```
Continue sieving while: T_sieve(p) < T_test / p

Where:
  T_sieve(p) = time to sieve one prime against all candidates in block
  T_test = time for one primality test at current digit count
  p = current sieve prime
```

**Implementation:**
1. Measure `T_test` from recent completed blocks (calibration data)
2. Measure `T_sieve` from sieve phase timing
3. Compute optimal sieve depth: `p_max = T_test / T_sieve`
4. Store optimal depth per (form, digit_range) in calibrations table
5. Strategy engine injects `--sieve-limit` into search params

For large digits (>100K), deeper sieving saves orders of magnitude of test time. For small digits (<1K), shallow sieving is optimal.

**Expected gains:** 2-10x throughput improvement for high-digit searches.

### 4.2 Tool Selection Optimization

**Current:** PFGW is used when `--pfgw-path` is set and digits exceed `--pfgw-min-digits`. The threshold is hardcoded.

**Target:** AI determines optimal tool routing per candidate:

```
For each candidate:
  estimate_gmp_time = calibrated_secs_per_candidate(form, digits, tool="gmp")
  estimate_pfgw_time = calibrated_secs_per_candidate(form, digits, tool="pfgw")
  estimate_gwnum_time = calibrated_secs_per_candidate(form, digits, tool="gwnum")

  Use tool with lowest estimated time
```

The crossover point varies by form:
- **kbn**: GWNUM faster above ~5K digits, GMP faster below
- **factorial**: PFGW faster above ~2K digits
- **palindromic**: PFGW faster above ~3K digits

Store crossover points in calibrations table, auto-update as real data accumulates.

### 4.3 Multi-Stage Sieve Pipeline Control

**Current:** Single-stage sieve (trial division) for all forms.

**Target:** AI selects and configures the sieve pipeline per form:

```
Stage 1: Trial division         (p up to 10^6-10^9)     → eliminates ~90%
Stage 2: BSGS sieve             (p up to 10^9-10^15)    → eliminates ~50-80% of remaining
Stage 3: Pollard P-1            (factors up to ~2^80)    → eliminates ~1-3%
Stage 4: ECM (selective)        (factors up to ~50 digits) → eliminates ~1-5%
Stage 5: PRP/primality test     → applied to survivors only
```

The AI decides:
- How deep to sieve at each stage (based on calibrated crossover)
- Whether to apply Stage 3/4 (based on digit count — not worth it for <10K digits)
- How many ECM curves to run (based on expected factor size distribution)

### 4.4 Search Parameter Optimization

**Current:** Search parameters (k, base, min-n, max-n for kbn; start/end for factorial) are either user-specified or use defaults.

**Target:** AI optimizes parameters for maximum discovery probability:

For **kbn** searches:
- Analyze which (k, base) pairs have the highest yield rates from historical data
- Prioritize k-values with known algebraic advantages (Aurifeuillean factorizations)
- Avoid ranges already searched by PrimeGrid/GIMPS (competitive awareness)

For **factorial/primorial** searches:
- Focus on ranges just beyond known primes (n > current record ± 1000)
- Skip ranges already confirmed prime-free by other projects

For **palindromic** searches:
- Focus on near-repdigit parameterization (Propper-Batalov approach)
- Optimize digit count to target BLS-provable range

---

## Phase 5: Research & Competitive Intelligence

> **Priority: P2** — Knowing the landscape is essential for strategic allocation.

### 5.1 Competitive Landscape Monitoring

**Current:** t5k.org records scraped every 24 hours for 8 forms. No awareness of what competitors are actively searching.

**Target:** Automated competitive intelligence:

| Source | Data | Frequency |
|--------|------|-----------|
| t5k.org Top 5000 | Record primes per form, recent additions | Daily |
| PrimeGrid status pages | Active subprojects, completion percentages | Weekly |
| GIMPS status | Current wavefront, double-check progress | Weekly |
| mersenneforum.org | Active discussion on search ranges | Monthly (agent task) |
| OEIS sequences | New entries for our forms | Weekly |

**Implementation:** Scheduled agent tasks (Phase 7 of agents roadmap) that:
1. Scrape and parse status pages
2. Update a `competitive_intel` table
3. Feed findings into strategy scoring
4. Alert when a competitor finds a prime in a range we're searching

### 5.2 Theoretical Research Integration

**Target:** AI agents that read and apply mathematical research:

1. **OEIS cross-referencing**: When a new prime is found, check all related OEIS sequences. If our prime extends a known sequence, flag for publication.

2. **Paper monitoring**: Monthly agent task to search arxiv.org for:
   - New primality tests (e.g., Pell's Cubic Test, SuperBFPSW)
   - New sieving techniques
   - New factorization methods
   - Form-specific results (new factorial primes, palindromic records)

3. **Theorem application**: When a new theoretical result is found:
   - Research agent writes a summary with implementation implications
   - Creates an engine agent task to prototype the improvement
   - Estimates speedup based on published benchmarks
   - Prioritizes implementation by expected ROI

### 5.3 Discovery-to-Publication Pipeline

**Current:** Finding a prime requires manual verification, t5k.org submission, and OEIS updates.

**Target:** Fully automated post-discovery pipeline:

```
Prime found
  → Auto-verification (3-tier: deterministic → BPSW+MR → PFGW cross-check)
  → If verified, record in database as "verified"
  → Check if it's a record (compare against competitive_intel)
  → If record or significant:
      → Generate t5k.org submission draft
      → Generate OEIS update if extends known sequence
      → Alert operator for review and submission
      → Prepare arxiv preprint template (for major discoveries)
```

---

## Phase 6: Self-Improving Loop

> **Priority: P3** — The AI learns from its own performance to get better over time.

### 6.1 Strategy Weight Learning

**Current:** Scoring weights are hardcoded: record_gap=0.25, yield=0.20, cost_efficiency=0.20, coverage=0.20, fleet_fit=0.15.

**Target:** Learn optimal weights from historical outcomes:

```
For each completed project:
  outcome_score = f(primes_found, digits_achieved, cost_efficiency, time_to_discovery)

  // Gradient-free optimization: perturb weights, observe outcomes
  // Use Thompson sampling or Bayesian optimization
  if outcome_score > running_average:
    nudge weights toward this configuration
  else:
    nudge weights away
```

Store weight history in `strategy_weight_history` table. Allow manual override.

Constraints:
- All weights must be positive and sum to 1.0
- No single weight can exceed 0.40 (prevents degenerate strategies)
- Minimum 50 completed projects before auto-tuning activates
- Human can lock weights via config

### 6.2 Search Performance Analytics

**Target:** After each project completes, generate a performance report:

- **Yield analysis**: primes found vs predicted, by range segment
- **Cost analysis**: actual cost vs estimated, breakdown by sieve/test/proof
- **Timing analysis**: actual secs_per_candidate vs calibrated model
- **Comparative**: this project vs similar historical projects
- **Recommendations**: what to search next based on what we learned

Store reports in `project_reports` table. Display on dashboard project detail page.

### 6.3 Sieve Effectiveness Tracking

**Target:** Track sieve performance metrics per form:

```sql
CREATE TABLE sieve_metrics (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  form TEXT NOT NULL,
  sieve_depth BIGINT NOT NULL,
  digit_range_low INT NOT NULL,
  digit_range_high INT NOT NULL,
  candidates_before_sieve BIGINT NOT NULL,
  candidates_after_sieve BIGINT NOT NULL,
  sieve_time_secs FLOAT8 NOT NULL,
  test_time_saved_secs FLOAT8,  -- estimated
  recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

The AI uses this to:
- Verify that deeper sieving actually improves throughput (it should, but measure)
- Find the optimal sieve depth for each (form, digit_range) pair
- Detect when sieve parameters need retuning (e.g., after hardware changes)

### 6.4 Failure Pattern Analysis

**Target:** Detect and respond to recurring failure patterns:

| Pattern | Detection | Response |
|---------|-----------|----------|
| Node consistently fails on form X | >3 failures from same node on same form | Stop assigning form X to that node |
| Form X has 0 yield after N blocks | yield_rate < threshold for N > calibrated_expectation | Pause form, investigate parameters |
| PFGW crashes on specific input | Exit code 139 (SIGSEGV) pattern | Fall back to GMP for that candidate range |
| Network partition | N nodes go stale simultaneously | Pause dependent projects, alert operator |
| Cost overrun pattern | Actual > 2x estimated consistently | Recalibrate cost model immediately |

---

## Phase 7: Full Autonomous Mode

> **Priority: P3** — The end state: AI manages everything, human sets policy.

### 7.1 Policy-Based Control

**Target:** Human sets high-level policy, AI handles everything else:

```toml
# darkreach-policy.toml — the only file a human needs to touch

[budget]
monthly_limit_usd = 500.00
max_per_project_usd = 50.00
agent_daily_budget_usd = 10.00

[strategy]
mode = "balanced"  # "aggressive" | "balanced" | "conservative" | "record-hunt"
preferred_forms = ["kbn", "palindromic"]
excluded_forms = []
min_idle_before_new_project = 2

[discovery]
auto_verify = true
auto_submit_t5k = false  # requires human review
auto_update_oeis = false

[fleet]
auto_scale = true
max_nodes = 16
min_nodes = 2
deprovision_idle_hours = 24

[manual_overrides]
# Override any AI decision. Takes precedence over strategy.
# force_form = "wagstaff"
# force_range = { start = 13400000, end = 14000000 }
# pause_all = false
```

The AI reads this policy and makes all operational decisions within it. Manual overrides are always respected.

### 7.2 Autonomous Job Creation

**Current:** Strategy engine creates projects when idle workers exist and budget allows. Parameters are generic.

**Target:** Full autonomous lifecycle:

```
Every strategy tick (5 min):
  1. Survey: fleet status, active projects, yield data, competitive intel, cost calibration
  2. Score: rank all 12 forms with calibrated weights
  3. Allocate: determine optimal project portfolio for current fleet size
  4. Create: auto-generate projects with optimized parameters
  5. Assign: route work blocks to best-fit nodes
  6. Monitor: detect stalls, failures, near-records
  7. Adapt: pause underperformers, extend winners, rebalance portfolio
  8. Report: log all decisions with reasoning for audit trail
```

The human retains:
- Kill switch: `pause_all = true` in policy file
- Budget hard limits
- Approval for record submissions
- Approval for fleet scaling (if auto_scale disabled)

### 7.3 Fleet Auto-Scaling

**Target:** Automatically adjust fleet size based on workload:

```
Every 30 minutes:
  compute_demand = sum(project.recommended_nodes for active projects)
  current_supply = active_node_count

  if compute_demand > current_supply * 1.2 and budget_allows:
    provision_nodes(min(compute_demand - current_supply, max_scale_step))

  if current_supply > compute_demand * 1.5 for > deprovision_idle_hours:
    deprovision_nodes(current_supply - ceil(compute_demand * 1.1))
```

**Provisioning backends:**
- Hetzner API (current fleet)
- Cloud API adapters (future: AWS, GCP, Azure spot instances)
- Operator-contributed nodes (manual, not auto-scaled)

### 7.4 Cross-Form Insight Transfer

**Target:** When the AI discovers something useful in one form, apply it to others:

Examples:
- If a specific sieve depth works well for kbn, test it on twin/sophie_germain (which share kbn infrastructure)
- If PFGW acceleration is higher than expected for factorial, re-measure for primorial (similar structure)
- If a k-value shows anomalously high yield, investigate mathematically (may indicate unknown algebraic structure)

Store insights in `agent_memory` with category `cross_form_insight`.

---

## Phase 8: Advanced AI Capabilities

> **Priority: P4** — Cutting-edge features for when the fundamentals are solid.

### 8.1 Reinforcement Learning for Search Strategy

**Target:** Replace the weighted scoring model with a learned policy:

**State space:**
- Fleet size, core count, available tools per node
- Active project portfolio (forms, progress, yield rates)
- Budget remaining (daily, monthly)
- Competitive landscape (competitor activity per form)
- Historical yield curves per form

**Action space:**
- Create project (form, parameters, budget allocation)
- Pause/resume project
- Reallocate nodes between projects
- Adjust sieve parameters
- Request research investigation

**Reward signal:**
- +10 per prime found
- +100 per record prime
- +1000 per independently verified record
- -1 per dollar spent
- -10 per hour of idle compute
- -50 per failed project (no primes found)

**Approach:** Start with contextual bandits (form selection), graduate to full RL as data accumulates. Thompson sampling is practical with <100 data points; switch to neural policy after ~1000 completed projects.

### 8.2 LLM-Driven Algorithm Discovery

**Target:** Use Claude as a research assistant to propose and test new algorithms:

```
Monthly research cycle:
  1. Agent reads recent arxiv papers on primality testing
  2. Agent reads engine.md roadmap for planned improvements
  3. Agent proposes 2-3 algorithmic improvements with expected speedup
  4. For each proposal:
     a. Write a benchmark comparing old vs new approach
     b. Run benchmark on representative inputs
     c. If speedup > 10%, create implementation task
     d. If speedup < 10%, log reasoning and move on
```

Focus areas:
- New sieve techniques (batch GCD, algebraic factorizations)
- Novel primality tests (Pell's Cubic, circulant eigenvalue)
- Better starting points for Lucas sequence-based tests
- Form-specific optimizations (e.g., better Rödseth starting values for LLR)

### 8.3 Anomaly Detection in Prime Distribution

**Target:** Statistical analysis of discovered primes to find unexpected patterns:

- Deviation from expected prime density (Cramér's conjecture)
- Clustering in specific residue classes
- Unexpected gaps or concentrations
- Correlation between different forms (do factorial and primorial primes cluster?)

This is speculative but could lead to publishable mathematical results or inform search strategy.

---

## Implementation Priority

| Phase | Items | Effort | Impact | Dependencies |
|-------|-------|--------|--------|-------------|
| **1. Task Runner & Budget** | 1.1, 1.2, 1.3 | Large | **Critical** | Schema exists |
| **2. Cost Calibration** | 2.1, 2.2, 2.3 | Medium | High | Phase 1 + real search data |
| **3. Workforce Allocation** | 3.1, 3.2, 3.3 | Large | **Critical** | Phase 2 |
| **4. Sieve Optimization** | 4.1, 4.2, 4.3, 4.4 | Large | High | Phase 2 |
| **5. Research Intelligence** | 5.1, 5.2, 5.3 | Medium | High | Phase 1 |
| **6. Self-Improvement** | 6.1, 6.2, 6.3, 6.4 | Medium | Medium | Phases 2, 3 |
| **7. Full Autonomy** | 7.1, 7.2, 7.3, 7.4 | Large | **Strategic** | Phases 1-6 |
| **8. Advanced AI** | 8.1, 8.2, 8.3 | Very Large | Strategic | All prior phases |

**Recommended implementation order:**

```
Phase 1 (Task Runner)           ← START HERE, unblocks everything
  ↓
Phase 2 (Cost Calibration)      ← Needs real data from Phase 1
  ↓
Phase 3 + Phase 5 (parallel)    ← Workforce allocation + competitive intel
  ↓
Phase 4 (Sieve Optimization)    ← Most impactful engine gains
  ↓
Phase 6 (Self-Improvement)      ← Needs data from Phases 3-5
  ↓
Phase 7 (Full Autonomy)         ← Integration of all prior phases
  ↓
Phase 8 (Advanced AI)           ← Research frontier
```

---

## Design Principles

1. **AI suggests, human approves (initially).** Start with AI making recommendations that a human reviews. As trust builds and calibration improves, gradually increase autonomy. The `manual_overrides` section in policy always takes precedence.

2. **Measure everything.** Every decision the AI makes is logged with full reasoning, predicted outcome, and actual outcome. This data drives calibration and learning.

3. **Budget is a hard limit, not a guideline.** The AI must never exceed budget constraints. If uncertain about cost, estimate conservatively. Better to leave compute idle than to overspend.

4. **Cheaper models for cheaper tasks.** Use Haiku for research and monitoring, Sonnet for implementation, Opus only for complex architectural decisions. Agent cost should be <5% of compute cost.

5. **Calibrate before optimizing.** Don't tune sieve depths or tool selection until you have accurate timing data. Phase 2 (calibration) must complete before Phase 4 (optimization) begins.

6. **The AI should make darkreach better at finding primes, not just better at running searches.** The difference is research integration (Phase 5) and algorithm discovery (Phase 8). A truly intelligent engine doesn't just execute — it improves the execution strategy over time.

7. **Competitive awareness is a strategic advantage.** Knowing what GIMPS and PrimeGrid are searching lets darkreach focus on niches with higher ROI per core-hour. This is the single biggest strategic lever.

8. **Fail fast, learn fast.** Short project durations (days, not months) with rapid feedback. If a form isn't yielding results, pivot quickly. Don't stubbornly continue a search that calibration data says is unlikely to succeed.

---

## Success Metrics

| Metric | Current | Phase 3 Target | Phase 7 Target |
|--------|---------|---------------|---------------|
| Human intervention per discovery | ~100% | <50% | <5% |
| Cost estimate accuracy | Unknown (uncalibrated) | ±2x | ±20% |
| Node utilization | Manual | >80% | >95% |
| Time from idle node to productive work | Minutes (manual) | <60s (auto-assign) | <10s |
| Primes found per core-dollar | Baseline | 2x baseline | 5x baseline |
| Time from discovery to verification | Hours (manual) | <10 min (auto) | <2 min |
| Strategy decisions per human decision | 0 (all manual) | 5:1 | 50:1 |
