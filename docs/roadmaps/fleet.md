# Fleet & Distributed Compute Roadmap

The roadmap for transforming darkreach from a private fleet tool into a modern, open distributed prime-hunting platform — the cloud-native successor to PrimeGrid.

**Vision:** A multi-form, AI-powered prime discovery platform where anyone can contribute compute via a native client or Docker container. Modern infra (Rust, PostgreSQL, WebSocket, Docker) replaces 2005-era BOINC. AI agents autonomously research forms, design campaigns, and coordinate workers.

**Key files:** `src/{dashboard,db,fleet,worker_client,pg_worker,search_manager,agent,verify}.rs`

**Reference:** See `docs/architecture-comparison.md` for detailed comparison with GIMPS, PrimeGrid, and BOINC.

---

## Phase 0: Correctness Foundation (must-have before scaling)

Before adding workers, every result must be trustworthy. GIMPS learned this the hard way — 1-2% of LL results were silently corrupted before Gerbicz checking.

### 0.1 Per-Block Checkpointing
**Problem:** Worker crash mid-block loses all partial work. Block restarts from zero.
**Solution:** Workers commit progress within blocks (every 10% or 60 seconds). On reclaim, new worker resumes from last checkpoint.
- Add `block_checkpoint` column to `work_blocks` (JSONB: last_n_tested, tested_count, found_count)
- Workers update checkpoint on heartbeat
- `claim_work_block()` returns checkpoint data so new worker can resume
- **Gate:** Required before any fleet scaling

### 0.2 Gerbicz Error Checking for Long Tests
**Problem:** Hardware errors (bit flips, overheating) corrupt primality test results silently.
**Solution:** Implement Gerbicz-Li error checking for Proth and LLR tests.
- Every L² iterations (~4M), verify checksum: `d(t+1) == u(0) * d(t)^(2^L) mod N`
- On mismatch: rollback to last valid state, replay
- Overhead: ~0.1% with L=2000
- **Scope:** kbn, cullen_woodall, carol_kynea, twin, sophie_germain (all forms using Proth/LLR)
- **Reference:** `docs/architecture-comparison.md` § Gerbicz Error Checking

### 0.3 Worker Reliability Scoring
**Problem:** All workers treated equally. No way to detect unreliable hardware.
**Solution:** Track per-worker reliability over rolling 30-day window.
- `worker_reliability` table: worker_id, total_results, valid_results, error_results, reliability_score
- Score = valid / total (0.0 to 1.0)
- Workers below 0.80 get blocks re-checked by a second worker
- Workers above 0.95 get "trusted" status (skip double-check)
- **Gate:** Required before volunteer computing (untrusted workers)

### 0.4 Exponential Backoff on Failures
**Problem:** Workers hammer coordinator at 10s intervals during outages.
**Solution:** Heartbeat backoff: 10s → 20s → 40s → ... → 5min max. Reset on success.
- Apply to both HTTP and PG heartbeat paths
- Log warning at each escalation level
- **Quick win:** Small change, big reliability improvement

---

## Phase 1: Scaling Foundations (2-20 workers)

### 1.1 Batch Block Claiming
**Problem:** One DB roundtrip per block claim. Bottleneck at 50+ workers.
**Solution:** `claim_work_blocks(job_id, worker_id, count)` — claim N blocks atomically.
- Workers claim 5-10 blocks, process sequentially, claim more when running low
- Reduces DB queries by 5-10x
- New PG function: SELECT multiple rows with `FOR UPDATE SKIP LOCKED LIMIT N`

### 1.2 Dynamic Stale Timeouts
**Problem:** Hard-coded 120s timeout. Large blocks (kbn n>500K) take longer, get reclaimed mid-work.
**Solution:** Scale timeout with block size and form type.
- `stale_timeout = max(120, estimated_block_duration * 3)`
- Estimate from historical block completion times per form
- Store `estimated_duration_s` on `work_blocks` at creation time

### 1.3 Command Queue (Replace Single pending_command)
**Problem:** Only one pending command per worker. Second command overwrites first.
**Solution:** `worker_commands` table with queue semantics.
- `worker_commands(id, worker_id, command, created_at, acked_at)`
- Workers ACK commands explicitly
- Coordinator retries unacked commands after 30s
- Supports: stop, restart, update_config, reassign

### 1.4 Block-Level Progress Reporting
**Problem:** Progress only visible at worker level, not block level.
**Solution:** Workers update `work_blocks.tested` and `work_blocks.found` on every heartbeat.
- Dashboard shows per-block progress bars
- Enables accurate ETA calculation per job
- Feeds into dynamic stale timeout estimation

### 1.5 Remove Dual-Path Redundancy
**Problem:** Both PG and in-memory fleet updated on every heartbeat (unnecessary overhead).
**Solution:** PG is source of truth. In-memory fleet becomes a read cache, rebuilt from PG on startup.
- Remove `lock_or_recover(&state.fleet).heartbeat()` from hot path
- Fleet struct becomes read-only cache, refreshed every 5s from PG
- Eliminates mutex contention on heartbeat endpoint

---

## Phase 2: Docker & Container Distribution

### 2.1 Worker Docker Image
**Current:** Dockerfile builds coordinator+dashboard. Workers deployed via SSH+systemd.
**Target:** Dedicated worker image that auto-discovers coordinator and claims work.
```
docker run -e DATABASE_URL=... -e COORDINATOR_URL=... darkreach/worker
```
- Multi-stage build: Rust binary + GMP only (minimal image)
- Auto-detect CPU cores, set `--threads` accordingly
- Health check endpoint for orchestration
- Graceful shutdown on SIGTERM (finish current candidate, checkpoint, exit)

### 2.2 Docker Compose Fleet Template
```yaml
services:
  coordinator:
    image: darkreach/coordinator
    ports: [7001:7001]
  worker:
    image: darkreach/worker
    deploy:
      replicas: 4
    environment:
      COORDINATOR_URL: http://coordinator:7001
```
- One-command fleet deployment: `docker compose up --scale worker=4`
- Volume mount for checkpoints (persist across restarts)

### 2.3 Container Registry CI/CD
- GitHub Actions: build + push to GHCR on every release tag
- Multi-arch images: amd64 + arm64
- Semantic versioning: `darkreach/worker:v0.2.0`, `darkreach/worker:latest`
- Security scanning (Trivy) in CI

### 2.4 Kubernetes Manifests (Optional)
- StatefulSet for coordinator (persistent volume for state)
- Deployment for workers (stateless, auto-scaling)
- ConfigMap for search parameters
- HorizontalPodAutoscaler based on CPU utilization
- **Defer until there's actual K8s demand**

---

## Phase 3: Native Volunteer Client

### 3.1 Auto-Updating Client Binary
**Goal:** Users download a single binary that keeps itself current.
- Self-update mechanism: check GHCR/GitHub releases on startup
- Platform support: Linux amd64, Linux arm64, macOS amd64, macOS arm64, Windows
- CLI: `darkreach join` — registers with coordinator, starts claiming work
- Background mode: `darkreach join --daemon` (systemd/launchd service installer)

### 3.2 Work Preferences
**Inspired by GIMPS's work type preferences:**
- Users choose: which forms to search, max CPU %, max RAM, GPU preference
- `darkreach join --forms kbn,factorial --max-cpu 80 --max-ram 4G`
- Preferences stored in coordinator, work matched accordingly
- Scheduler: match work to capable workers (GPU work to GPU hosts, high-RAM work for P-1)

### 3.3 Volunteer Registration & Identity
- UUID-based worker identity (generated on first run, persisted locally)
- Optional account linking (GitHub/email) for credit tracking
- No authentication required to contribute (lower barrier to entry)
- Leaderboard: top contributors by form, by total compute hours

### 3.4 Trust & Verification for Untrusted Workers
**Critical for volunteer computing — can't trust results from unknown hardware.**
- New results from unproven workers get **mandatory double-check** (quorum of 2)
- After 10 consecutive valid results: promote to "trusted" (quorum of 1)
- On any error: reset trust, require double-check
- High-value results (potential records) always get triple-check + cross-software verification
- **Inspired by:** BOINC adaptive replication + GIMPS reliability scoring

### 3.5 Credit & Contribution Tracking
- Track GHz-days (or core-hours) contributed per worker
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
Each stage is independently distributable. Workers can specialize:
- Fast-sieve workers: low RAM, many cores, process Stage 1-2
- Heavy-test workers: high RAM, fewer cores, process Stage 3-5
- GPU workers: Stage 1 sieving (massive parallelism)

### 5.2 Stage-Aware Block Claiming
- `work_blocks.stage` column (1-5)
- Workers request blocks for specific stages
- Coordinator ensures blocks flow through stages in order
- Stage 1 output feeds Stage 2 input (pipeline)

### 5.3 Candidate Queue Between Stages
- `candidate_queue` table: candidates that survived Stage N, awaiting Stage N+1
- Workers pull from queue instead of re-sieving
- Enables: "sieve once, test many times" (for double-checking)
- Feeds into: volunteer computing (sieve on server, distribute tests to volunteers)

---

## Phase 6: AI Agent Integration

### 6.1 Campaign Strategist Agent
- Analyzes world records (t5k.org scraping, already exists)
- Identifies forms where records are beatable with available compute
- Designs multi-phase campaigns with cost estimates
- Adjusts strategy based on results (found primes → push harder, dry spell → pivot)

### 6.2 Fleet Optimizer Agent
- Monitors worker throughput, identifies slow/failing workers
- Recommends block size adjustments per form
- Detects hardware degradation (declining throughput over time)
- Auto-scales Docker workers based on job queue depth

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

### 7.3 GPU Worker Type
- Separate worker image: `darkreach/worker-gpu`
- Auto-detect GPU (CUDA/OpenCL/ROCm)
- Work preferences: GPU-eligible forms routed to GPU workers
- Mixed workloads: sieve on GPU, test on CPU simultaneously

---

## Relationship to Infrastructure Roadmap

The infrastructure track (K8s, Docker CI/CD, Helm, Terraform, volunteer accounts, monitoring) is being developed concurrently. This fleet roadmap focuses on **distributed computing correctness and algorithms** — the two tracks are complementary:

| This Roadmap (Fleet) | Infra Track (Concurrent) |
|---|---|
| Per-block checkpointing | K8s-ready Rust code |
| Gerbicz error checking | Docker & CI/CD pipeline |
| Worker reliability scoring | Volunteer account system |
| Proof-based verification | Result verification & trust UI |
| Multi-stage work pipeline | Credit & leaderboard |
| AI agent integration | Helm charts & auto-scaling |
| GPU acceleration | Monitoring & observability |
| Batch block claiming | Terraform IaC |

**Coordination points:** Infra track provides the deployment substrate; this track provides the distributed computing logic that runs on it. Key interfaces: worker health endpoints (for K8s probes), metrics export (for monitoring), and the verification pipeline (for trust system).

---

## Priority Order

```
Phase 0: Correctness     ← Do now. Nothing else matters if results are wrong.
Phase 1: Scaling          ← Next. Enable 5-20 reliable workers.
Phase 2: Docker           ← Handled by infra track (concurrent).
Phase 3: Volunteer client ← After infra track delivers Docker + accounts.
Phase 4: Proofs           ← Parallel with Phase 3. Enables trustless verification.
Phase 5: Multi-stage      ← After proofs. Optimizes throughput per compute-hour.
Phase 6: AI agents        ← Ongoing. Unique differentiator, build incrementally.
Phase 7: GPU              ← When GPU-capable FFT (GWNUM) is integrated.
```

---

## Success Metrics

| Milestone | Target | Measure |
|-----------|--------|---------|
| **Reliable fleet** | 0 corrupted results | Gerbicz + double-check agreement rate |
| **10-worker fleet** | All workers productive | Block throughput, reclaim rate < 5% |
| **Docker deployment** | One-command fleet | `docker compose up --scale worker=N` works |
| **First volunteer** | External contributor | Someone outside the team runs a worker |
| **100 volunteers** | Sustainable community | Monthly active workers, retention rate |
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
| **Onboarding** | Install Prime95, configure | Install BOINC, attach | **`docker run` or download binary** |
