# Fleet Architecture: Darkreach vs. World-Class Systems

A comparison of darkreach's distributed coordination against GIMPS, PrimeGrid, and BOINC — the systems that have found every record-setting prime in the last 30 years.

---

## Executive Summary

Darkreach has a **functional MVP** for fleet coordination: PostgreSQL-backed block claiming, heartbeat registration, and 3-tier verification. But compared to GIMPS and PrimeGrid, it's missing several critical patterns that these systems developed over decades of distributed prime hunting at scale.

**What darkreach does well:**
- Multi-form flexibility (12 forms vs GIMPS's 1)
- `FOR UPDATE SKIP LOCKED` atomic block claiming (elegant, no deadlocks)
- PgWorkerClient direct-to-DB path (eliminates coordinator as bottleneck)
- 3-tier verification pipeline (deterministic → GMP MR → PFGW cross-verify)
- Agent infrastructure for autonomous campaign management

**What's missing vs. world-class:**
- No per-block checkpointing (partial work lost on crash)
- No result double-checking or proof-based verification
- No worker reliability tracking or adaptive replication
- No tiered assignment deadlines (one-size-fits-all 120s stale timeout)
- No batch block claiming (sequential one-at-a-time claims)
- No Gerbicz error checking during long-running tests
- Single coordinator (no HA/failover)
- No exponential backoff on heartbeat failures

---

## Architecture Comparison

### Work Distribution

| | Darkreach | GIMPS | PrimeGrid/BOINC |
|---|---|---|---|
| **Model** | Pull (worker claims blocks) | Pull (client requests assignment) | Pull (scheduler sends work) |
| **Granularity** | Fixed block_size per job | One exponent per assignment | One candidate per work unit |
| **Claiming** | `FOR UPDATE SKIP LOCKED` | PrimeNet HTTP API | Shared-memory feeder → scheduler |
| **Batch claiming** | No (1 block per query) | 1 assignment per request | Configurable (1-N per request) |
| **Deadlines** | 120s hard timeout | 30-360 days, tiered by reliability | Configurable `delay_bound` per WU |
| **Work pipeline** | Single-pass (sieve → test) | Multi-stage (TF → P-1 → PRP → proof) | Two-phase (sieve → test) |

### Fault Tolerance

| | Darkreach | GIMPS | PrimeGrid/BOINC |
|---|---|---|---|
| **Checkpoint frequency** | 60s (per-search, not per-block) | 30 min (per-exponent) | App-specific |
| **Crash recovery** | Block reclaimed after 120s, restarted from zero | Resume from 30-min checkpoint | Result re-sent to new host |
| **Partial work preservation** | Lost | Preserved via checkpoint | Lost (new instance) |
| **Error detection** | GMP `is_probably_prime(25)` | Gerbicz (99.999+%) + Jacobi | Quorum comparison |
| **Stale detection** | Worker heartbeat timeout (60s) | 120-day reliability window | Transitioner daemon |
| **Worker reliability** | None tracked | Scored over 120 days, gates access | Consecutive valid results tracked |

### Verification

| | Darkreach | GIMPS | PrimeGrid/BOINC |
|---|---|---|---|
| **Method** | 3-tier (proof → GMP → PFGW) | Proof-based (Pietrzak VDF) | Quorum (min_quorum results must agree) |
| **Cost** | 100% (full re-test) | 1% (proof verification) | 200%+ (double/triple computation) |
| **Double-check** | None | Eliminated by proofs (2020) | Configurable (2-3x redundancy) |
| **Cross-software** | PFGW in tier 3 | 4 independent programs | LLR then PFGW |
| **Certificates** | Proth/LLR/Pocklington/Morrison/BLS | Pietrzak VDF + Gerbicz | Form-specific |

### Scale Numbers

| | Darkreach (today) | GIMPS | PrimeGrid |
|---|---|---|---|
| **Workers** | 4 (1 server) | ~500,000 hosts | ~16,000 hosts |
| **Compute** | 4 vCPU dedicated | ~4.71 PFLOPS | ~1,860 TFLOPS |
| **Forms** | 12 | 1 (Mersenne) | 10+ |
| **Coordinator** | 1 (no HA) | PrimeNet (load-balanced) | BOINC server (multi-daemon) |
| **Work units active** | 80 blocks | ~200,000 assignments | Thousands |
| **Checkpoint** | JSON file | Binary, CRC-protected | App-specific |

---

## GIMPS Architecture Deep Dive

### PrimeNet V5 Protocol (9 operations)

```
100  UPDATE_COMPUTER_INFO   — Register hardware (CPU, RAM, GPU)
101  PROGRAM_OPTIONS        — Set work preferences
102  GET_ASSIGNMENT         — Request work (pull model)
103  REGISTER_ASSIGNMENT    — Reserve specific exponent
104  ASSIGNMENT_PROGRESS    — Report interim progress (hourly)
105  ASSIGNMENT_RESULT      — Submit completed result
106  ASSIGNMENT_UNRESERVE   — Release assignment
107  BENCHMARK_DATA         — Upload performance data
108  PING_SERVER            — Health check
```

Key design: Communication is **batched hourly**, not real-time. This reduces server load by 360x compared to darkreach's 10-second heartbeat.

### Multi-Stage Elimination Pipeline

```
Trial Factoring (TF) → P-1 Factoring → ECM → PRP Test → Proof Certification
   (minutes)            (hours)         (hours)  (1 week GPU)  (minutes)
   Eliminates ~30%      Eliminates ~10%  Deep     Definitive    1/100th cost
```

Each stage is orders of magnitude cheaper than the next. Most candidates are eliminated before reaching the expensive PRP test.

### Tiered Assignment System

GIMPS assigns work based on **demonstrated reliability** over 120 days:

| Category | Deadline | Requirements |
|----------|----------|-------------|
| 0 (urgent) | 30 days | 3+ results, high reliability |
| 1 | 90 days | 5+ results, 0.90+ reliability |
| 2 | 180 days | Medium reliability |
| 3 | 270 days | Basic |
| 4 (bulk) | 360 days | Minimum 5,500 MHz |

### The Proof Revolution (2020)

Pietrzak VDF proofs eliminated double-checking entirely:
- Generated during PRP test with ~1-5% overhead
- Verification at 1/100th cost
- Combined with Gerbicz error checking: "complete confidence in correctness"
- 256MB proof file per exponent (configurable)

### Gerbicz Error Checking

```
Primary sequence: u(t) = a^(2^t) mod N
Checksum: d(t) = product of u(i*L) for i=0..t (mod N)

Every L^2 iterations (~4M):
  Verify: d(t+1) == u(0) * d(t)^(2^L) mod N
  If mismatch: rollback to last valid checkpoint, replay
  Overhead: ~0.1% with L=2000
```

Before Gerbicz, ~1-2% of LL results were corrupted by hardware errors. For multi-day tests, this made double-checking mandatory. After Gerbicz, error rate dropped to <0.001%.

---

## BOINC Architecture Deep Dive

### Six-Daemon Backend

```
Feeder → Shared Memory → Scheduler (CGI)
                              ↕
                         MySQL DB
                              ↕
Transitioner → Validator → Assimilator → File Deleter
```

- **Feeder**: Pre-loads work units from MySQL into shared memory (eliminates DB as bottleneck for scheduling)
- **Scheduler**: CGI handles client HTTP requests, serves work from shared memory
- **Transitioner**: State machine for work units (generates new instances when needed)
- **Validator**: Application-specific result comparison (byte-for-byte or tolerance-based)
- **Assimilator**: Project-specific result processing

### Adaptive Replication

BOINC tracks `CV(H, V)` — consecutive valid results from host H using app version V:
- If CV exceeds threshold → reduce replication (quorum of 1)
- On error → reset counter, resume full replication
- Goal: reduce redundancy from 100% to 5-10% for reliable hosts

### Work Unit Lifecycle

```
UNSENT → SENT → (deadline passes) → TIMED_OUT → resend
                → COMPLETED → validate → CANONICAL (or ERROR)
```

Key fields: `min_quorum`, `target_nresults`, `delay_bound`, `max_total_results`

---

## Darkreach Current Architecture

### Block-Based Coordination (PostgreSQL)

```sql
-- Atomic block claiming (no deadlocks, no duplicates)
UPDATE work_blocks
SET status = 'claimed', claimed_by = $worker_id, claimed_at = NOW()
WHERE id = (
    SELECT id FROM work_blocks
    WHERE search_job_id = $job_id AND status = 'available'
    ORDER BY block_start LIMIT 1
    FOR UPDATE SKIP LOCKED
)
RETURNING id, block_start, block_end;
```

### Dual Coordination Paths

1. **HTTP** (`WorkerClient`): Worker → HTTP → Coordinator → PostgreSQL
2. **Direct PG** (`PgWorkerClient`): Worker → PostgreSQL (no coordinator needed)

Both use identical shared-state pattern: `Arc<AtomicU64>` counters + `Arc<Mutex<String>>` for current candidate.

### Stale Block Reclamation

```
Every 30s: reclaim blocks where:
  - status = 'claimed'
  - claimed_at > 120 seconds ago
  - worker's last_heartbeat > 60 seconds ago (worker is dead)
```

### Known Limitations

1. **Sequential block claiming** — one block per DB roundtrip
2. **No per-block checkpointing** — partial work lost on crash
3. **Hard-coded timeouts** — 120s stale, 60s alive
4. **Dual-path overhead** — both PG and in-memory fleet updated on every heartbeat
5. **No command acknowledgment** — coordinator can't confirm worker received "stop"
6. **No worker reliability tracking** — all workers treated equally
7. **No exponential backoff** — workers hammer coordinator at 10s intervals during outages
8. **Single coordinator** — no failover, no HA
9. **No Gerbicz error checking** — silent hardware errors corrupt results

---

## Key Patterns to Adopt

### Tier 1: Critical (enable correct results at any scale)

1. **Per-block checkpointing** — Save progress within blocks so crashes don't lose work
2. **Gerbicz error checking for Proth/LLR** — Detect hardware errors during long tests
3. **Worker reliability scoring** — Track error rates, gate access to high-priority work
4. **Exponential backoff on heartbeat failures** — Don't hammer coordinator during outages

### Tier 2: Important (enable scaling to 10-50 workers)

5. **Batch block claiming** — Claim 5-10 blocks at once to reduce DB roundtrips
6. **Dynamic stale timeouts** — Scale with block size and expected throughput
7. **Command acknowledgment** — Workers ACK stop commands, coordinator retries
8. **Block-level progress reporting** — Commit tested/found counts within blocks

### Tier 3: Scalable (enable 50-1000+ workers)

9. **Proof-based verification** — Generate verifiable proofs during testing (1/100th verification cost)
10. **Adaptive replication** — Skip double-checking for proven-reliable workers
11. **Coordinator HA** — Multiple coordinators behind load balancer
12. **Work pipeline stages** — Separate sieve and test into independently claimable stages

### Tier 4: World-class (competitive with GIMPS/PrimeGrid)

13. **Tiered assignment system** — Priority access for fast/reliable workers
14. **GPU acceleration** — CUDA/OpenCL sieving and testing
15. **Pietrzak VDF proofs** — Cryptographic verification for kbn/Mersenne forms
16. **BOINC integration** — Tap into volunteer computing networks
