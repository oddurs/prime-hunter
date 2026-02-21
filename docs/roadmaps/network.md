# Network & Distributed Compute Roadmap

> Renamed from fleet.md. See also: [architecture.md](architecture.md) for the master migration plan.

The roadmap for transforming darkreach from a private network tool into a modern, open distributed prime-hunting platform — the cloud-native successor to PrimeGrid.

**Vision:** A multi-form, AI-powered prime discovery platform where anyone can contribute compute via a native client or Docker container. Modern infra (Rust, PostgreSQL, WebSocket, Docker) replaces 2005-era BOINC. AI agents autonomously research forms, design campaigns, and coordinate nodes.

**Key files:** `src/{dashboard,db,network,operator_client,pg_operator,agent,verify}.rs`

**Reference:** See `docs/architecture-comparison.md` for detailed comparison with GIMPS, PrimeGrid, and BOINC.

---

## Phase 0: Correctness Foundation (must-have before scaling)

Before adding nodes, every result must be trustworthy. GIMPS learned this the hard way — 1-2% of LL results were silently corrupted before Gerbicz checking.

### 0.1 Per-Block Checkpointing
**Problem:** Node crash mid-block loses all partial work. Block restarts from zero.
**Solution:** Nodes commit progress within blocks (every 10% or 60 seconds). On reclaim, the new node resumes from last checkpoint.
- Add `block_checkpoint` column to `work_blocks` (JSONB: last_n_tested, tested_count, found_count)
- Nodes update checkpoint on heartbeat
- `claim_work_block()` returns checkpoint data so the new node can resume
- **Gate:** Required before any network scaling

### 0.2 Gerbicz Error Checking for Long Tests
**Problem:** Hardware errors (bit flips, overheating) corrupt primality test results silently.
**Solution:** Implement Gerbicz-Li error checking for Proth and LLR tests.
- Every L² iterations (~4M), verify checksum: `d(t+1) == u(0) * d(t)^(2^L) mod N`
- On mismatch: rollback to last valid state, replay
- Overhead: ~0.1% with L=2000
- **Scope:** kbn, cullen_woodall, carol_kynea, twin, sophie_germain (all forms using Proth/LLR)
- **Reference:** `docs/architecture-comparison.md` § Gerbicz Error Checking

### 0.3 Node Reliability Scoring
**Problem:** All nodes treated equally. No way to detect unreliable hardware.
**Solution:** Track per-node reliability over rolling 30-day window.
- `node_reliability` table: node_id, total_results, valid_results, error_results, reliability_score
- Score = valid / total (0.0 to 1.0)
- Nodes below 0.80 get blocks re-checked by a second node
- Nodes above 0.95 get "trusted" status (skip double-check)
- **Gate:** Required before operator computing (untrusted nodes)

### 0.4 Exponential Backoff on Failures
**Problem:** Nodes hammer the central service at 10s intervals during outages.
**Solution:** Heartbeat backoff: 10s → 20s → 40s → ... → 5min max. Reset on success.
- Apply to PG heartbeat path
- Log warning at each escalation level
- **Quick win:** Small change, big reliability improvement

---

## Phase 1: Scaling Foundations (2-20 nodes)

> **Architecture note (Feb 2026):** Phase 1 changes have been largely completed. SearchManager has been replaced by PG-backed search jobs. All coordination is now PG-only (no in-memory fleet, no HTTP coordinator heartbeat). The dual-path redundancy (1.5) has been removed.

### 1.1 Batch Block Claiming
**Problem:** One DB roundtrip per block claim. Bottleneck at 50+ nodes.
**Solution:** `claim_work_blocks(job_id, node_id, count)` — claim N blocks atomically.
- Nodes claim 5-10 blocks, process sequentially, claim more when running low
- Reduces DB queries by 5-10x
- New PG function: SELECT multiple rows with `FOR UPDATE SKIP LOCKED LIMIT N`

### 1.2 Dynamic Stale Timeouts
**Problem:** Hard-coded 120s timeout. Large blocks (kbn n>500K) take longer, get reclaimed mid-work.
**Solution:** Scale timeout with block size and form type.
- `stale_timeout = max(120, estimated_block_duration * 3)`
- Estimate from historical block completion times per form
- Store `estimated_duration_s` on `work_blocks` at creation time

### 1.3 Command Queue (Replace Single pending_command)
**Problem:** Only one pending command per node. Second command overwrites first.
**Solution:** `node_commands` table with queue semantics.
- `node_commands(id, node_id, command, created_at, acked_at)`
- Nodes ACK commands explicitly
- Central service retries unacked commands after 30s
- Supports: stop, restart, update_config, reassign

### 1.4 Block-Level Progress Reporting
**Problem:** Progress only visible at node level, not block level.
**Solution:** Nodes update `work_blocks.tested` and `work_blocks.found` on every heartbeat.
- Dashboard shows per-block progress bars
- Enables accurate ETA calculation per job
- Feeds into dynamic stale timeout estimation

### 1.5 Remove Dual-Path Redundancy *(Completed)*
**Problem:** Both PG and in-memory network registry updated on every heartbeat (unnecessary overhead).
**Solution:** PG is source of truth. In-memory network registry removed.
- Removed in-memory fleet from hot path
- All state now lives in PG
- Eliminates mutex contention on heartbeat endpoint

---

## Phase 2: Docker & Container Distribution

### 2.1 Node Docker Image
**Current:** Dockerfile builds dashboard. Nodes deployed via SSH+systemd.
**Target:** Dedicated node image that auto-discovers the central service and claims work.
```
docker run -e DATABASE_URL=... darkreach/node
```
- Multi-stage build: Rust binary + GMP only (minimal image)
- Auto-detect CPU cores, set `--threads` accordingly
- Health check endpoint for orchestration
- Graceful shutdown on SIGTERM (finish current candidate, checkpoint, exit)

### 2.2 Docker Compose Network Template
```yaml
services:
  dashboard:
    image: darkreach/dashboard
    ports: [7001:7001]
  node:
    image: darkreach/node
    deploy:
      replicas: 4
    environment:
      DATABASE_URL: postgres://...
```
- One-command network deployment: `docker compose up --scale node=4`
- Volume mount for checkpoints (persist across restarts)

### 2.3 Container Registry CI/CD
- GitHub Actions: build + push to GHCR on every release tag
- Multi-arch images: amd64 + arm64
- Semantic versioning: `darkreach/node:v0.2.0`, `darkreach/node:latest`
- Security scanning (Trivy) in CI

### 2.4 Kubernetes Manifests (Optional)
- StatefulSet for dashboard/central service (persistent volume for state)
- Deployment for nodes (stateless, auto-scaling)
- ConfigMap for search parameters
- HorizontalPodAutoscaler based on CPU utilization
- **Defer until there's actual K8s demand**

---

## Phase 3: Native Operator Client

### 3.1 Auto-Updating Client Binary
**Goal:** Users download a single binary that keeps itself current.
- Self-update mechanism: check GHCR/GitHub releases on startup
- Platform support: Linux amd64, Linux arm64, macOS amd64, macOS arm64, Windows
- CLI: `darkreach register` — registers with the network, starts claiming work
- Background mode: `darkreach register --daemon` (systemd/launchd service installer)

### 3.2 Work Preferences
**Inspired by GIMPS's work type preferences:**
- Operators choose: which forms to search, max CPU %, max RAM, GPU preference
- `darkreach register --forms kbn,factorial --max-cpu 80 --max-ram 4G`
- Preferences stored in the central service, work matched accordingly
- Scheduler: match work to capable nodes (GPU work to GPU hosts, high-RAM work for P-1)

### 3.3 Operator Registration & Identity
- UUID-based node identity (generated on first run, persisted locally)
- Optional account linking (GitHub/email) for credit tracking
- No authentication required to contribute (lower barrier to entry)
- Leaderboard: top contributors by form, by total compute hours

### 3.4 Trust & Verification for Untrusted Nodes
**Critical for operator computing — can't trust results from unknown hardware.**
- New results from unproven nodes get **mandatory double-check** (quorum of 2)
- After 10 consecutive valid results: promote to "trusted" (quorum of 1)
- On any error: reset trust, require double-check
- High-value results (potential records) always get triple-check + cross-software verification
- **Inspired by:** BOINC adaptive replication + GIMPS reliability scoring

### 3.5 Credit & Contribution Tracking
- Track GHz-days (or core-hours) contributed per operator/node
- Credit system: weighted by work difficulty (10K-digit test > 1K-digit sieve)
- Public stats page: total compute, primes found, top contributors
- Badges: first prime found, 100 blocks completed, 1000 core-hours, etc.

---

## Phase 4: Proof-Based Verification

### 4.1 Proth/LLR Witness Certificates
**Current:** Proth and LLR tests return pass/fail but don't output the witness computation trace.
**Target:** Record intermediate values during Proth/LLR tests as verifiable certificates.
- Proth certificate: base `a`, intermediate residues at checkpoints
- LLR certificate: Lucas sequence values at checkpoints
- Verification: replay computation at checkpoints (10-20% of full test cost)
- Store certificates in `primes.certificate` (JSONB, already exists)

### 4.2 Pietrzak VDF Proofs (Long-Term)
**The gold standard — GIMPS uses this to verify PRP results at 1/100th cost.**
- Applicable to: kbn PRP tests, Wagstaff, any Fermat PRP
- Generates proof during test with ~1-5% overhead
- Verification: ~1% of original test time
- **Complex to implement** — requires deep FFT integration
- **Defer until:** Gerbicz + witness certificates are working

### 4.3 Cross-Software Verification Pipeline
**Current:** Tier 3 uses PFGW. Only one external tool.
**Target:** Multiple independent verification backends.
- PFGW (existing)
- PRST (existing for kbn)
- OpenPFGW (different codebase from PFGW)
- For any potential record: require 2+ independent confirmations
- Automated: verification daemon picks unverified primes, runs through pipeline

---

## Phase 5: Multi-Stage Work Pipeline

### 5.1 Sieve → Screen → Test → Prove
**Inspired by GIMPS's TF → P-1 → PRP → Proof pipeline.**
```
Stage 1: Deep Sieve       — eliminate ~95% of candidates (cheap)
Stage 2: P-1 Pre-filter   — eliminate ~5-10% more (moderate)
Stage 3: Quick PRP Screen — 2-round MR (fast, eliminates 99.9% of remaining)
Stage 4: Full Primality   — Proth/LLR/deterministic test (expensive)
Stage 5: Proof Generation — certificate for verified primes (moderate)
```
Each stage is independently distributable. Nodes can specialize:
- Fast-sieve nodes: low RAM, many cores, process Stage 1-2
- Heavy-test nodes: high RAM, fewer cores, process Stage 3-5
- GPU nodes: Stage 1 sieving (massive parallelism)

### 5.2 Stage-Aware Block Claiming
- `work_blocks.stage` column (1-5)
- Nodes request blocks for specific stages
- Coordinator ensures blocks flow through stages in order
- Stage 1 output feeds Stage 2 input (pipeline)

### 5.3 Candidate Queue Between Stages
- `candidate_queue` table: candidates that survived Stage N, awaiting Stage N+1
- Nodes pull from queue instead of re-sieving
- Enables: "sieve once, test many times" (for double-checking)
- Feeds into: operator computing (sieve on server, distribute tests to operators)

---

## Phase 6: AI Agent Integration

### 6.1 Campaign Strategist Agent
- Analyzes world records (t5k.org scraping, already exists)
- Identifies forms where records are beatable with available compute
- Designs multi-phase campaigns with cost estimates
- Adjusts strategy based on results (found primes → push harder, dry spell → pivot)

### 6.2 Network Optimizer Agent
- Monitors node throughput, identifies slow/failing nodes
- Recommends block size adjustments per form
- Detects hardware degradation (declining throughput over time)
- Auto-scales Docker nodes based on job queue depth

### 6.3 Discovery Announcer Agent
- When a significant prime is found: generates announcement, context, OEIS links
- Cross-references against known records
- Queues verification pipeline
- Drafts submission to Top5000 (t5k.org) if record-worthy

---

## Phase 7: GPU Acceleration

### 7.1 GPU Sieving (CUDA/OpenCL)
- Sieve of Eratosthenes on GPU: 10-100x faster than CPU for large limits
- BSGS sieve parallelization on GPU
- **Huge impact:** Sieving is the most parallelizable phase

### 7.2 GPU Primality Testing
- GpuOwl/PRPLL for Mersenne-like forms
- GeneferOCL for Generalized Fermat numbers
- CUDA FFT for Proth/LLR tests
- **Gated on:** GWNUM/FFT integration

### 7.3 GPU Node Type
- Separate node image: `darkreach/node-gpu`
- Auto-detect GPU (CUDA/OpenCL/ROCm)
- Work preferences: GPU-eligible forms routed to GPU nodes
- Mixed workloads: sieve on GPU, test on CPU simultaneously

---

## Relationship to Infrastructure Roadmap

The infrastructure track (K8s, Docker CI/CD, Helm, Terraform, operator accounts, monitoring) is being developed concurrently. This network roadmap focuses on **distributed computing correctness and algorithms** — the two tracks are complementary:

| This Roadmap (Network) | Infra Track (Concurrent) |
|---|---|
| Per-block checkpointing | K8s-ready Rust code |
| Gerbicz error checking | Docker & CI/CD pipeline |
| Node reliability scoring | Operator account system |
| Proof-based verification | Result verification & trust UI |
| Multi-stage work pipeline | Credit & leaderboard |
| AI agent integration | Helm charts & auto-scaling |
| GPU acceleration | Monitoring & observability |
| Batch block claiming | Terraform IaC |

**Coordination points:** Infra track provides the deployment substrate; this track provides the distributed computing logic that runs on it. Key interfaces: node health endpoints (for K8s probes), metrics export (for monitoring), and the verification pipeline (for trust system).

---

## Priority Order

```
Phase 0: Correctness     ← Do now. Nothing else matters if results are wrong.
Phase 1: Scaling          ← Next. Enable 5-20 reliable nodes. (Partially complete — PG-only model.)
Phase 2: Docker           ← Handled by infra track (concurrent).
Phase 3: Operator client  ← After infra track delivers Docker + accounts.
Phase 4: Proofs           ← Parallel with Phase 3. Enables trustless verification.
Phase 5: Multi-stage      ← After proofs. Optimizes throughput per compute-hour.
Phase 6: AI agents        ← Ongoing. Unique differentiator, build incrementally.
Phase 7: GPU              ← When GPU-capable FFT (GWNUM) is integrated.
```

---

## Success Metrics

| Milestone | Target | Measure |
|-----------|--------|---------|
| **Reliable network** | 0 corrupted results | Gerbicz + double-check agreement rate |
| **10-node network** | All nodes productive | Block throughput, reclaim rate < 5% |
| **Docker deployment** | One-command network | `docker compose up --scale node=N` works |
| **First operator** | External contributor | Someone outside the team runs a node |
| **100 operators** | Sustainable community | Monthly active nodes, retention rate |
| **World record** | Top5000 submission | Verified prime in underserved form |
| **AI-found record** | Agent-designed campaign finds a record prime | Fully autonomous discovery |

---

## Competitive Positioning

| | GIMPS | PrimeGrid | **Darkreach** |
|---|---|---|---|
| **Forms** | 1 (Mersenne) | 10+ | 12+ |
| **Infra** | Custom (1997) | BOINC (2004) | **Rust + Docker + PG (2025)** |
| **AI** | None | None | **Agent-powered campaigns** |
| **Client** | Prime95/mprime | BOINC client | **Native Rust + Docker** |
| **Verification** | Pietrzak proofs | BOINC quorum | **3-tier + proofs (planned)** |
| **GPU** | GpuOwl/PRPLL | GeneferOCL | **Planned** |
| **DX** | 1990s web UI | 2010s BOINC UI | **Modern Next.js dashboard** |
| **Onboarding** | Install Prime95, configure | Install BOINC, attach | **`docker run` or download binary (operator self-service)** |
