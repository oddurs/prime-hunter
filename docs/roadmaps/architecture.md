# Architecture Roadmap — Workers + Central Brain

> Master plan for migrating darkreach from a deployable coordinator model to dumb nodes + smart central brain + PostgreSQL as the coordination bus.
>
> **Supersedes:** `fleet.md`, `public-compute.md`, `cluster.md`
>
> **Snapshot date:** February 20, 2026

---

## Status

| Phase | Description | Status |
|-------|-------------|--------|
| Phase 0 | Naming & Concept Migration | In Progress |
| Phase 1 | Kill the Deployable Coordinator | Planned |
| Phase 2 | Role-Based Auth & Dashboard Split | Planned |
| Phase 3 | Operator Experience | Planned |
| Phase 4 | AI Orchestration Engine | Planned |
| Phase 5 | Network Scaling | Planned |

---

## Terminology

The old model used language that implied a single coordinator managing a fleet. The new model is decentralized: nodes talk to PostgreSQL, the dashboard observes, and an AI brain makes decisions.

| Old Term | New Term | Notes |
|----------|----------|-------|
| Volunteer | **Operator** | Person running nodes. "Volunteer" implies charity; operators are participants. |
| Worker | **Node** | Compute machine. A node is autonomous — it claims work from PG, runs it, reports results. |
| Fleet | **Network** | Collection of nodes. "Fleet" implies central command; a network is peer-like. |
| Coordinator | **Dashboard/API** | Web frontend + REST API. No orchestration logic — pure observation and control plane. |
| SearchManager | **Search Jobs (PG)** | Search lifecycle managed by database rows, not an in-memory process. |
| DeploymentManager | **(removed)** | SSH deployment is an ops concern, not a runtime concern. Gate behind `#[cfg(feature = "ops")]`. |
| Fleet heartbeat (HTTP) | **Node heartbeat (PG)** | Nodes heartbeat directly to PostgreSQL via `PgWorkerClient`. No HTTP coordinator needed. |

---

## Phase 0: Naming & Concept Migration

Pure refactoring — same functionality, new names. No behavioral changes. The goal is to align the codebase, database, API, CLI, and frontend with the new terminology before making architectural changes.

### 0.1 Database Migration

Rename tables and columns to use new terminology. Provide backward-compatible views so existing queries continue to work during the transition.

```sql
-- Migration 025: Terminology migration
ALTER TABLE volunteers RENAME TO operators;
ALTER TABLE operators RENAME COLUMN volunteer_id TO operator_id;

-- Backward-compat view
CREATE VIEW volunteers AS SELECT operator_id AS volunteer_id, * FROM operators;

ALTER TABLE workers RENAME TO nodes;
CREATE VIEW workers AS SELECT * FROM nodes;

-- Update foreign keys in work_blocks, search_jobs, etc.
```

**Key files:** `supabase/migrations/025_terminology_migration.sql`

### 0.2 API Route Aliases

Add new routes alongside legacy routes. Both return identical responses. Legacy routes emit a deprecation header.

```
/api/v1/operators/*     (new)    alongside  /api/v1/volunteers/*  (legacy)
/api/v1/nodes/*         (new)    alongside  /api/v1/workers/*     (legacy)
/api/v1/network/*       (new)    alongside  /api/v1/fleet/*       (legacy)
```

**Key files:** `src/dashboard/mod.rs`, `src/dashboard/routes_volunteer.rs` (rename to `routes_operator.rs`)

### 0.3 Rust Source Renames

Rename modules and types to use new terminology. Update all internal references.

| Old | New |
|-----|-----|
| `src/volunteer.rs` | `src/operator.rs` |
| `src/fleet.rs` | `src/network.rs` (then remove in Phase 1) |
| `src/db/volunteers.rs` | `src/db/operators.rs` |
| `src/worker_client.rs` | `src/node_client.rs` |
| `src/pg_worker.rs` | `src/pg_node.rs` |
| `VolunteerConfig` | `OperatorConfig` |
| `WorkerClient` | `NodeClient` |
| `PgWorkerClient` | `PgNodeClient` |

Preserve `pub type WorkerClient = NodeClient;` aliases in `src/lib.rs` for one release cycle.

### 0.4 CLI Renames

| Old Command | New Command | Notes |
|-------------|-------------|-------|
| `darkreach join` | `darkreach register` | Register this machine as a node |
| `darkreach volunteer` | `darkreach run` | Start the node work loop |
| `darkreach work` | `darkreach run --job <ID>` | Run a specific search job |

Old commands remain as hidden aliases (clap `hide = true`) for backward compatibility.

**Key files:** `src/cli.rs`, `src/main.rs`

### 0.5 Frontend Renames

| Old | New |
|-----|-----|
| `frontend/src/app/fleet/` | `frontend/src/app/network/` |
| "Fleet" in nav | "Network" |
| `useFleet()` hook | `useNetwork()` |
| `frontend/src/hooks/use-fleet.ts` | `frontend/src/hooks/use-network.ts` |
| `FleetStatus` component | `NetworkStatus` component |

**Key files:** `frontend/src/components/app-header.tsx`, `frontend/src/app/network/page.tsx`

### 0.6 Documentation Updates

Update all CLAUDE.md files, roadmaps, and references to use new terminology. Update `CLAUDE.md` domain map table.

### Verification

- `cargo test` passes (all 449 tests)
- Old CLI commands still work via hidden aliases
- Old API endpoints return same responses (with deprecation header)
- Frontend builds and shows "Network" instead of "Fleet"
- No functionality changes — pure rename

---

## Phase 1: Kill the Deployable Coordinator

The current architecture has a coordinator process (`darkreach dashboard`) that maintains in-memory state: `SearchManager` (running searches), `Fleet` (worker registry), `DeploymentManager` (SSH deploys). This is fragile — coordinator restart loses all state.

**Target architecture:** PostgreSQL is the sole coordination bus. The dashboard process is a stateless web server that reads from and writes to PG. Nodes talk directly to PG. No in-memory coordination state.

### 1.1 Remove SearchManager

**Current:** `SearchManager` spawns child processes, tracks running searches in memory. Dashboard restart kills all searches.
**Target:** Dashboard creates `search_jobs` + `work_blocks` rows in PG. Nodes claim blocks from PG. No child process spawning.

- Remove `src/search_manager.rs`
- Dashboard "Start Search" button creates `search_jobs` row with status `active`
- Dashboard "Generate Blocks" creates `work_blocks` rows for the job
- Job completion: when all blocks are `completed`, mark job as `completed`
- Background task checks for completable jobs every 30s

**Key files:** `src/search_manager.rs` (remove), `src/dashboard/routes_search.rs` (simplify)

### 1.2 Remove DeploymentManager

**Current:** `DeploymentManager` maintains SSH connections, tracks deployment state in memory.
**Target:** Gate behind `#[cfg(feature = "ops")]` feature flag. Not needed for the core platform.

- Move `src/deploy.rs` behind `#[cfg(feature = "ops")]`
- Remove deployment WebSocket messages from default build
- Ops tooling becomes a separate concern (scripts, Ansible, Terraform)

**Key files:** `src/deploy.rs`, `src/dashboard/routes_deploy.rs`

### 1.3 Remove In-Memory Fleet

**Current:** `Fleet` struct holds a `HashMap<WorkerId, WorkerInfo>` behind a `Mutex`. Updated on every heartbeat. Duplicates PG state.
**Target:** Remove `Fleet` entirely. All node queries go through PG.

- Remove `src/fleet.rs` (or `src/network.rs` after Phase 0)
- Remove `fleet` field from `AppState`
- All dashboard endpoints that read fleet data query `nodes` table directly
- WebSocket updates query PG on a 5s interval (or use Supabase Realtime)

**Key files:** `src/fleet.rs` (remove), `src/dashboard/mod.rs` (remove from AppState)

### 1.4 Deprecate HTTP Worker Heartbeat

**Current:** Nodes can heartbeat via HTTP (`POST /api/worker/heartbeat`) or PG (`worker_heartbeat()` RPC). Two paths, same data.
**Target:** PG heartbeat only. Remove the `--coordinator` CLI flag.

- Remove `POST /api/worker/heartbeat` endpoint
- Remove `--coordinator` flag from CLI
- All nodes use `PgNodeClient` (requires `DATABASE_URL`)
- Simplifies auth: no API tokens needed for heartbeat (PG auth handles it)

**Key files:** `src/dashboard/routes_worker.rs` (remove heartbeat), `src/cli.rs` (remove --coordinator)

### 1.5 Simplify WebSocket

**Current:** WebSocket sends deployment status, SearchManager state, fleet data.
**Target:** WebSocket sends only real-time events: prime discoveries, search status changes, node connect/disconnect.

- Remove deployment messages
- Remove SearchManager state messages
- Node status: emit events on connect/disconnect (from PG `LISTEN/NOTIFY` or polling)
- Prime discoveries: keep existing Supabase Realtime path
- Search progress: dashboard polls PG or uses Supabase Realtime on `work_blocks`

**Key files:** `src/dashboard/ws.rs`

### 1.6 Merge Worker Tables

**Current:** `workers` table (from cluster MVP) and `operator_nodes` table (from volunteer system) have overlapping concerns.
**Target:** Single `nodes` table with all fields.

```sql
-- Migration 026: Merge worker tables
CREATE TABLE nodes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    operator_id UUID REFERENCES operators(id),
    hostname TEXT NOT NULL,
    cores INTEGER,
    os TEXT,
    arch TEXT,
    ram_gb REAL,
    has_gpu BOOLEAN DEFAULT false,
    gpu_model TEXT,
    gpu_vram_gb REAL,
    version TEXT,
    status TEXT DEFAULT 'offline',  -- online, offline, working, stale
    current_job_id UUID REFERENCES search_jobs(id),
    current_block_id UUID REFERENCES work_blocks(id),
    last_heartbeat TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);
```

**Key files:** `supabase/migrations/026_merge_worker_tables.sql`, `src/db/nodes.rs`

### 1.7 Remove Internal HTTP Heartbeat Routes

Clean up routes that were only used for coordinator-to-worker HTTP communication.

- Remove `/api/worker/heartbeat`
- Remove `/api/worker/command`
- Remove `/api/worker/register`
- Keep `/api/v1/nodes/*` (public API for dashboard consumption)

**Key files:** `src/dashboard/routes_worker.rs`

### Verification

- `darkreach dashboard` starts without `SearchManager`, `Fleet`, or `DeploymentManager` in `AppState`
- Nodes connect via PG only (`DATABASE_URL` required, `--coordinator` removed)
- Creating a search from dashboard creates `search_jobs` + `work_blocks` in PG
- Dashboard shows node status by querying `nodes` table
- No in-memory coordination state — dashboard can restart without losing anything
- `cargo test` passes

---

## Phase 2: Role-Based Auth & Dashboard Split

The current dashboard is admin-only. To support operators (people running nodes), the dashboard needs role-based access control. Admins see everything; operators see their nodes, their contributions, and public data.

### 2.1 User Profiles Table

```sql
-- Migration 027: User profiles with roles
CREATE TABLE user_profiles (
    id UUID PRIMARY KEY REFERENCES auth.users(id),
    display_name TEXT,
    role TEXT NOT NULL DEFAULT 'operator',  -- 'admin' or 'operator'
    operator_id UUID REFERENCES operators(id),
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- RLS: users can read own profile, admins can read all
ALTER TABLE user_profiles ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Users can read own profile" ON user_profiles
    FOR SELECT USING (auth.uid() = id);
CREATE POLICY "Admins can read all profiles" ON user_profiles
    FOR SELECT USING (
        EXISTS (SELECT 1 FROM user_profiles WHERE id = auth.uid() AND role = 'admin')
    );
```

**Key files:** `supabase/migrations/027_user_profiles.sql`

### 2.2 Frontend Auth Context

Extend `useAuth()` hook to include role and operator association.

```typescript
interface AuthContext {
    user: User | null;
    role: 'admin' | 'operator' | null;
    operatorId: string | null;
    isAdmin: boolean;
    isOperator: boolean;
}
```

**Key files:** `frontend/src/hooks/use-auth.ts`, `frontend/src/contexts/auth-context.tsx`

### 2.3 Navigation Split

| Page | Admin | Operator |
|------|-------|----------|
| Dashboard (overview) | Yes | Yes |
| Browse Primes | Yes | Yes |
| Leaderboard | Yes | Yes |
| Network (all nodes) | Yes | No |
| Searches (all jobs) | Yes | No |
| Projects | Yes | No |
| Releases | Yes | No |
| Agents | Yes | No |
| Deployments | Yes | No |
| My Nodes | No | Yes |
| My Stats | No | Yes |
| Account | Yes | Yes |
| Docs | Yes | Yes |

**Key files:** `frontend/src/components/app-header.tsx`, `frontend/src/components/nav-links.tsx`

### 2.4 Admin-Only Page Gating

`RoleGuard` component wraps admin-only pages. Redirects unauthorized users to dashboard.

```typescript
function RoleGuard({ requiredRole, children }: { requiredRole: 'admin'; children: ReactNode }) {
    const { role } = useAuth();
    if (role !== requiredRole) {
        redirect('/');
    }
    return <>{children}</>;
}
```

**Key files:** `frontend/src/components/role-guard.tsx`

### 2.5 Shared Pages (Role-Adapted)

Some pages show different content based on role:
- **Dashboard:** Admin sees network-wide stats. Operator sees personal stats + network summary.
- **Browse:** Same for both roles.
- **Leaderboard:** Same for both, but operator's own rank is highlighted.

### 2.6 API Auth Middleware

REST API endpoints check role from Supabase JWT.

```rust
async fn require_admin(
    Extension(user): Extension<AuthUser>,
) -> Result<(), StatusCode> {
    if user.role != "admin" {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}
```

**Key files:** `src/dashboard/auth.rs`, `src/dashboard/mod.rs`

### Verification

- Admin login shows full navigation (all pages accessible)
- Operator signup shows limited navigation (My Nodes, My Stats, Dashboard, Browse, Leaderboard, Docs)
- Unauthorized page access redirects to dashboard
- API endpoints return 403 for unauthorized role access
- Supabase RLS enforces row-level access

---

## Phase 3: Operator Experience

Make the operator journey seamless: sign up, get an API key, run a node, see results.

### 3.1 Onboarding Flow

New page: `frontend/src/app/setup/page.tsx`

Steps:
1. **Sign up** (Supabase Auth — email or GitHub OAuth)
2. **Generate API key** (displayed once, stored hashed in `operators` table)
3. **Install instructions** (platform-specific: Linux, macOS, Windows, Docker)
4. **Connection test** (verify node appears in dashboard)
5. **First work block** (celebrate when first result is submitted)

**Key files:** `frontend/src/app/setup/page.tsx`, `frontend/src/components/setup-wizard.tsx`

### 3.2 My Nodes Page

New page: `frontend/src/app/my-nodes/page.tsx`

Shows:
- List of operator's nodes (hostname, status, version, current work)
- Per-node stats (blocks completed, primes found, uptime)
- Node health indicators (heartbeat age, error rate)
- Actions: rename node, deactivate node, view logs

**Key files:** `frontend/src/app/my-nodes/page.tsx`, `frontend/src/hooks/use-my-nodes.ts`

### 3.3 Account Page

New page: `frontend/src/app/account/page.tsx`

Shows:
- Profile (display name, email)
- API key management (rotate, revoke)
- Notification preferences
- Contribution summary (total blocks, total primes, compute hours)

**Key files:** `frontend/src/app/account/page.tsx`

### 3.4 API Key Rotation Endpoint

```
POST /api/v1/operators/me/rotate-key
Authorization: Bearer <supabase-jwt>

Response: { "api_key": "dr_...", "expires_at": null }
```

The API key is used by the node binary to authenticate PG connections (via a connection proxy or direct PG role). Old key is invalidated immediately.

**Key files:** `src/dashboard/routes_operator.rs`

### 3.5 CLI `darkreach run` Improvements

```bash
# First-time setup
darkreach register --api-key dr_abc123

# Start working
darkreach run

# With preferences
darkreach run --forms kbn,factorial --max-cpu 80 --max-ram 4G

# Specific job
darkreach run --job <job-id>
```

The `run` command:
1. Reads config from `~/.darkreach/config.toml`
2. Connects to PG using API key credentials
3. Registers/updates node metadata (hostname, cores, OS, RAM, GPU)
4. Enters work loop: claim block, execute, report, repeat
5. Checks for updates on startup (respects release channel)

**Key files:** `src/cli.rs`, `src/operator.rs`

### 3.6 Network Status Public Endpoint

```
GET /api/v1/network/status

Response: {
    "nodes_online": 12,
    "total_cores": 96,
    "active_searches": 3,
    "primes_found_today": 7,
    "total_primes": 1423
}
```

Public (no auth required). Consumed by the public website and operator onboarding flow.

**Key files:** `src/dashboard/routes_network.rs`

### Verification

End-to-end flow works:
1. Sign up on `app.darkreach.ai`
2. Generate API key in setup wizard
3. `darkreach register --api-key dr_...` on a machine
4. `darkreach run` starts claiming and completing work
5. Node appears in "My Nodes" page with live status
6. Results appear in Browse and Leaderboard

---

## Phase 4: AI Orchestration Engine

The "central brain" — an AI system that observes the network, researches prime forms, designs search campaigns, and generates work automatically. Operators contribute compute; the AI decides what to compute.

### 4.1 Strategy Engine

New module: `src/strategy.rs`

The strategy engine runs a continuous loop:

```
Survey → Strategize → Generate → Monitor → React → (repeat)
```

**Survey:** Gather current state.
- Network capacity (online nodes, total cores, available compute hours/day)
- Active searches (progress, estimated completion, primes found)
- World records (t5k.org data, OEIS sequences, competitive landscape)
- Historical performance (primes per core-hour by form, by digit range)

**Strategize:** Decide what to search.
- Rank forms by ROI: `(probability of discovery * impact) / compute cost`
- Consider: provability (t5k.org requires proofs), competition (avoid crowded forms), novelty
- Allocate compute budget across forms (e.g., 40% kbn, 30% factorial, 20% palindromic, 10% exploration)

**Generate:** Create search jobs.
- For each allocated form: create `search_jobs` rows with appropriate parameters
- Generate `work_blocks` for active jobs
- Set block sizes based on node capabilities and form characteristics

**Monitor:** Track progress.
- Watch for primes, dry spells, stalled jobs, node failures
- Compare actual throughput to predictions

**React:** Adjust strategy.
- Prime found: push harder in that range (cluster around discovery)
- Dry spell: widen range or pivot to different form
- Node failure: redistribute work
- New world record by competitor: update target

**Key files:** `src/strategy.rs`, `src/db/strategy.rs`

### 4.2 Project System Integration

The existing project system (`src/project/`) provides campaign management. The AI strategy engine creates projects automatically.

```
AI Strategy → Creates Project → Project generates Search Jobs → Jobs generate Work Blocks → Nodes claim blocks
```

- AI creates projects with budget, timeline, and success criteria
- Projects track cost (compute hours consumed) and results (primes found)
- AI can pause/cancel projects that underperform
- Admin can override AI decisions via dashboard

**Key files:** `src/project/mod.rs`, `src/strategy.rs`

### 4.3 Dashboard as Observation Plane

The dashboard shows AI decisions but does not make them. Admins can:

- View current strategy (allocation, reasoning)
- See AI decision history (why it started/stopped searches)
- Override: pause a search, force a form, adjust allocation
- Set constraints: max compute budget, excluded forms, priority targets

New dashboard pages:
- `/strategy` — Current AI strategy, allocation chart, decision log
- `/strategy/history` — Past decisions with outcomes

**Key files:** `frontend/src/app/strategy/page.tsx`, `frontend/src/hooks/use-strategy.ts`

### Verification

- AI creates search jobs automatically when network has idle capacity
- Dashboard shows strategy decisions with reasoning
- Admin override takes effect within one strategy cycle (5 minutes)
- Primes are discovered without manual intervention
- Strategy adapts when network capacity changes (nodes join/leave)

---

## Phase 5: Network Scaling

Scale from tens to hundreds/thousands of nodes with untrusted operators.

### 5.1 Advanced Trust Model

Trust levels for nodes, earned through consistent correct results.

| Level | Name | Requirements | Privileges |
|-------|------|-------------|------------|
| 0 | New | Just registered | Double-checked results, small blocks only |
| 1 | Proven | 10 consecutive valid results | Single-check for routine work |
| 2 | Trusted | 100 valid results, >0.98 reliability | Large blocks, priority assignment |
| 3 | Verified | Hardware benchmarked, identity confirmed | High-value work (potential records) |
| 4 | Core | Long-term contributor, known hardware | Verification duties, trusted for triple-check |

**Reliability scoring:**
- `reliability = valid_results / total_results` (rolling 30-day window)
- Below 0.80: demote to Level 0, require double-check
- Hardware scoring: benchmark on registration, detect performance degradation

```sql
-- New columns on nodes table
ALTER TABLE nodes ADD COLUMN trust_level INTEGER DEFAULT 0;
ALTER TABLE nodes ADD COLUMN reliability_score REAL DEFAULT 1.0;
ALTER TABLE nodes ADD COLUMN valid_results INTEGER DEFAULT 0;
ALTER TABLE nodes ADD COLUMN total_results INTEGER DEFAULT 0;
ALTER TABLE nodes ADD COLUMN benchmark_score REAL;
```

**Key files:** `src/db/trust.rs`, `supabase/migrations/028_trust_model.sql`

### 5.2 Result Verification Pipeline

New module: `src/result_verifier.rs`

For untrusted nodes, results need independent verification before being accepted.

```
Node submits result
  → If node.trust_level >= 1 AND result is not high-value:
      Accept immediately
  → If node.trust_level == 0 OR result is high-value:
      Queue for verification (assign same block to a different node)
  → If verification matches: accept, increment node.valid_results
  → If verification conflicts: assign to a third node, investigate
```

High-value results (potential records, primes > 100K digits) always get triple-check + cross-software verification (PFGW + PRST).

**Key files:** `src/result_verifier.rs`, `src/db/verification.rs`

### 5.3 Node Specialization

Nodes declare capabilities and preferences. The work assignment system matches work to capable nodes.

```toml
# ~/.darkreach/config.toml
[preferences]
forms = ["kbn", "factorial", "palindromic"]
max_digits = 100000
max_cpu_percent = 80
max_ram_gb = 8

[hardware]
# Auto-detected on registration
cores = 16
ram_gb = 32
has_gpu = true
gpu_model = "RTX 4090"
```

Work assignment priority:
1. Match required capabilities (RAM, GPU, architecture)
2. Prefer nodes that have declared preference for the form
3. Prefer nodes with higher trust level
4. Prefer nodes with lower current load

**Key files:** `src/operator.rs`, `src/db/assignment.rs`

### 5.4 Batch Block Claiming

**Current:** One DB roundtrip per block claim. Bottleneck at 50+ nodes.
**Target:** Claim N blocks atomically.

```sql
-- Claim up to 5 blocks in one query
SELECT * FROM work_blocks
WHERE job_id = $1
  AND status = 'pending'
  AND (min_ram_gb IS NULL OR min_ram_gb <= $3)
ORDER BY block_number
LIMIT $2
FOR UPDATE SKIP LOCKED;
```

Nodes claim 5-10 blocks, process sequentially, claim more when running low. Reduces DB queries by 5-10x.

**Key files:** `src/pg_node.rs`, `src/db/blocks.rs`

### 5.5 Dynamic Block Sizing

Auto-tune block size based on node throughput and form characteristics.

- Fast forms (sieve-heavy): larger blocks (100K candidates)
- Slow forms (test-heavy): smaller blocks (1K candidates)
- Fast nodes: larger blocks (less overhead)
- Slow/unreliable nodes: smaller blocks (less wasted work on failure)

```sql
ALTER TABLE work_blocks ADD COLUMN estimated_duration_s INTEGER;
```

Block size = `target_duration * node_throughput` where `target_duration` is 5-15 minutes (configurable per form).

**Key files:** `src/strategy.rs`, `src/db/blocks.rs`

### Verification

- Two nodes doing the same block produce matching results
- Trust levels progress correctly (new node starts at 0, earns levels)
- Node preferences are respected in work assignment
- Batch claiming reduces DB load (measurable via `pg_stat_statements`)
- Dynamic block sizing produces blocks that complete in target duration range

---

## Migration Path

The phases are designed to be implemented sequentially with clear boundaries.

```
Phase 0 (naming)  →  No behavioral change. Safe to deploy incrementally.
                      Can be done file-by-file over multiple PRs.

Phase 1 (kill coordinator)  →  Breaking change for nodes using --coordinator.
                                Requires coordinated release:
                                1. Release node binary with PG-only support
                                2. Update all nodes
                                3. Remove coordinator HTTP heartbeat
                                4. Deploy dashboard without SearchManager/Fleet

Phase 2 (auth)  →  Additive. No breaking changes. Deploy incrementally.

Phase 3 (operator UX)  →  Additive. New pages and endpoints. Deploy incrementally.

Phase 4 (AI)  →  Additive. Strategy engine runs alongside manual search creation.
                  Can be enabled/disabled via feature flag.

Phase 5 (scaling)  →  Additive. Trust model defaults to Level 1 for existing nodes.
                       Verification pipeline is opt-in per search job.
```

---

## Key Design Principles

1. **PostgreSQL is the coordination bus.** All state lives in PG. No in-memory coordination state. Dashboard and nodes are stateless processes that read/write PG.

2. **Nodes are dumb.** A node knows how to: connect to PG, claim a block, run a search, report results, heartbeat. It does not know about strategy, other nodes, or the overall search plan.

3. **The brain is centralized.** The AI strategy engine runs as a background task in the dashboard process (or as a separate service). It reads network state from PG, makes decisions, writes search jobs to PG.

4. **The dashboard is an observation plane.** It shows state, allows overrides, but does not orchestrate. If the dashboard goes down, nodes keep working (they talk to PG, not the dashboard).

5. **Trust is earned, not assumed.** New nodes start at trust level 0. Results are double-checked until the node proves reliable.

6. **Backward compatibility during migration.** Old CLI commands work via aliases. Old API endpoints return same data. Old table names accessible via views.

---

## Superseded Roadmaps

This roadmap supersedes:

- **`docs/roadmaps/fleet.md`** — Fleet coordination, distributed workers, Docker, volunteer client, verification, multi-stage pipeline, AI agents, GPU. All concepts are absorbed into this roadmap under the new terminology and architecture.
- **`docs/roadmaps/public-compute.md`** — Volunteer compute platform: release engineering, trust/validation, operator retention. Absorbed into Phase 3 (Operator Experience) and Phase 5 (Network Scaling).
- **`docs/roadmaps/cluster.md`** — Multi-node cluster management MVP and next steps. MVP is complete; remaining items absorbed into Phase 1 (Kill the Deployable Coordinator).

These files are preserved for historical reference but should not be used for planning. All new work should reference this roadmap.

---

## Success Metrics

| Milestone | Target | Measure |
|-----------|--------|---------|
| **Phase 0 complete** | Zero functionality regression | All 449 tests pass, old commands work |
| **Stateless dashboard** | Dashboard restartable without losing state | No `SearchManager`/`Fleet` in `AppState` |
| **First operator signup** | External person runs a node via dashboard | End-to-end onboarding flow works |
| **10-node network** | All nodes productive | Block throughput, reclaim rate < 5% |
| **AI-driven search** | Strategy engine creates productive searches | Primes found without manual intervention |
| **100-node network** | Trust model prevents bad results | 0 corrupted results accepted |
| **World record** | AI-designed campaign finds a record prime | Top5000 submission, fully autonomous |
