# Public Compute Roadmap

> **Deprecated (Feb 2026):** This roadmap has been superseded by [`architecture.md`](architecture.md) and [`network.md`](network.md). Retained for historical context. New work should reference those documents instead.

Build a volunteer-grade public compute offering for darkreach.

**Goal:** match the release, reliability, and operator quality of GIMPS/PrimeGrid while keeping darkreach's own architecture and search strategy.

**Snapshot date:** February 20, 2026.

**Key references:** `docs/roadmaps/competitive-analysis.md`, `docs/roadmaps/server.md`, `docs/roadmaps/ops.md`, `docs/references.md`

---

## Current Progress

### Implemented on February 20, 2026

- Added worker release manifest file: `deploy/releases/worker-manifest.json`
- Added release discovery endpoints:
  - `GET /api/volunteer/worker/latest?channel=stable`
  - `GET /api/v1/worker/latest?channel=stable`
- Added API docs entry in `website/src/app/docs/api/page.tsx`
- Added worker capability metadata persistence (`os`, `arch`, `ram_gb`, `has_gpu`, `gpu_model`, `gpu_vram_gb`)
- Added capability-aware assignment gating using job params:
  - `min_cores`
  - `min_ram_gb`
  - `requires_gpu`
  - `required_os`
  - `required_arch`
- Added startup update check in volunteer mode using:
  - `GET /api/v1/worker/latest?channel=...`
  - `DARKREACH_UPDATE_CHANNEL` (default `stable`)
- Added worker update execution path:
  - download + SHA-256 verify + unpack + stage binary
  - optional auto-apply with `DARKREACH_AUTO_UPDATE_APPLY=1` (Unix)
  - enable staging with `DARKREACH_AUTO_UPDATE=1`
- Added optional artifact signature verification in updater:
  - set `DARKREACH_VERIFY_WORKER_SIG=1`
  - provide `DARKREACH_WORKER_PUBKEY_PATH=/path/to/worker-signing.pub`
- Added release workflow artifact packaging + checksums + generated `worker-manifest.json` for tagged releases
- Added DB-backed release control plane:
  - `POST /api/releases/worker` (upsert version metadata)
  - `POST /api/releases/rollout` (channel -> version + rollout percent)
  - `POST /api/releases/rollback` (channel rollback to previous version)
  - `GET /api/releases/worker` (list versions + channel mappings)
  - `GET /api/releases/events` (rollout/rollback audit trail)
  - `GET /api/releases/health` (adoption by worker version + channel targets)
- Added frontend release management page:
  - `/releases` operator controls for rollout/rollback
  - release metadata upsert form (`version`, `artifacts`, `notes`, `published_at`)
  - channel targets, adoption view, and event timeline panels
- `GET /api/volunteer/worker/latest` now resolves channel from DB first, then manifest fallback
- Rollout percent now affects selection deterministically by `worker_id`:
  - `GET /api/volunteer/worker/latest?channel=stable&worker_id=<id>`
  - `rollout_percent=0` keeps all workers on previous version
  - `rollout_percent=100` sends all workers to channel target version

Manifest path can be overridden with `DARKREACH_WORKER_RELEASE_MANIFEST`.

---

## Problem Statement

darkreach already supports distributed private compute (workers, scheduling, dashboard, orchestration), but public compute needs additional guarantees:

- Frictionless installs and updates for non-technical users
- Strong trust model for untrusted hosts
- Safe staged rollouts (canary -> stable)
- Attribution and engagement loops to keep volunteers active
- Verifiable output quality at large scale

Competitors solve this with two major patterns:

- **GIMPS/PrimeNet model:** tightly controlled first-party clients with reliability scoring and assignment expiry.
- **PrimeGrid/BOINC model:** general volunteer client + project app versioning + validator/quorum pipeline.

darkreach should adopt the strengths of both without full BOINC migration in v1.

---

## Competitor Release Model (What Users Actually Get)

### GIMPS

- User deliverable: Prime95/mprime binaries, checksums, straightforward onboarding.
- Release behavior: upgrade cadence tied to algorithm improvements and reliability fixes.
- Control-plane behavior: assignment deadlines, recycling expired work, reliability-aware distribution.

### PrimeGrid

- User deliverable: BOINC client + PrimeGrid project attachment.
- Release behavior: server-managed app versions and platform-specific executables.
- Control-plane behavior: scheduler, validator, assimilator, and redundant result consensus.
- Engagement layer: recurring challenge calendar and team competition.

### Implication for darkreach

darkreach needs both:

- A direct, branded worker path (simple first run)
- A validator/reputation layer equivalent to BOINC's trust model

---

## Target Product Shape

### Volunteer-facing deliverables

- Signed worker binaries for Linux/macOS/Windows
- One-command/device onboarding (`register -> benchmark -> start`)
- Auto-update channels (`stable`, `beta`) with rollback
- Host profile report (CPU/GPU/RAM/OS capabilities)

### Operator-facing deliverables

- Release channels and rollout controls (percent rollout + instant rollback)
- Assignment policy engine (deadlines, reclaim, reliability thresholds)
- Validation pipeline (quorum, consensus, conflict retry)
- Reputation/anti-abuse system
- Public status and transparency pages

---

## Phased Roadmap

### Phase 0: Foundations (1-2 weeks)

### Deliverables

- Versioned worker artifact naming and manifest format
- Build-sign-publish pipeline in CI
- API additions for update checks and host capability registration

### Exit criteria

- Fresh host can discover latest worker version from API
- Signed artifacts available for all target OSes

---

### Phase 1: Public Worker MVP (2-4 weeks)

### Deliverables

- New `darkreach volunteer` mode with:
  - account registration flow
  - machine fingerprint + capability probe
  - secure token issuance and rotation
  - assignment fetch + heartbeat + result submit
- Auto-update client:
  - poll manifest
  - download + checksum/signature validation
  - safe restart

### Reliability controls

- TTL-based assignment expiry
- orphaned-assignment reclaim
- local checkpoint + safe resume on update

### Exit criteria

- Time-to-first-assignment under 5 minutes on clean host
- Successful update from N to N+1 without manual intervention

---

### Phase 2: Trust & Validation (3-5 weeks)

### Deliverables

- Validator service with redundant execution policy
- Work unit schema fields:
  - `min_quorum`
  - `target_results`
  - `max_delay`
  - `consensus_rule`
- Result states:
  - `pending`
  - `validated`
  - `conflicted`
  - `retry_required`

### Reputation model

- Host reliability score (agreement ratio, timeout ratio, stale ratio)
- Dynamic assignment sizing based on score
- Quarantine policy for suspicious hosts

### Exit criteria

- Quorum-enabled jobs resolve automatically
- Validation disagreements are observable and retryable from dashboard

---

### Phase 3: Release Engineering Parity (2-4 weeks)

### Deliverables

- Channelized rollouts:
  - canary (1-5%)
  - ramp (25/50/100%)
  - rollback to previous good version
- Compatibility gates by capability class
- Release dashboard:
  - adoption by version
  - crash rate
  - bad-result correlation by version

### Exit criteria

- Broken release can be halted and rolled back in under 10 minutes
- Version health visible in one dashboard panel

---

### Phase 4: Throughput + Proof Quality (4-8 weeks)

### Deliverables

- Deeper staged pipeline:
  - sieve
  - prefilter (P-1/ECM where applicable)
  - PRP/test
  - proof/certificate capture when tooling supports it
- Capability-aware routing:
  - CPU tiers
  - optional GPU tiers
  - memory-fit policy
- Cost/performance auto-tuning using real host telemetry

### Exit criteria

- Measurable throughput increase per volunteer host
- Lower expensive-test waste rate on weak/unstable hosts

---

### Phase 5: Volunteer Retention Layer (2-3 weeks)

### Deliverables

- Credits and per-host contribution history
- Team support and team standings
- Time-boxed challenges/campaigns with badges
- Public project pages with reproducible discovery metadata

### Exit criteria

- Retention metrics available (7-day, 30-day)
- Challenge pipeline can be configured without code changes

---

## Architecture Additions Needed

### Backend

- New services:
  - update service
  - validator service
  - reputation scorer
- New tables:
  - `worker_versions`
  - `worker_release_channels`
  - `work_results`
  - `validation_sets`
  - `volunteer_credits`

### API

- `GET /api/volunteer/worker/latest?channel=stable`
- `POST /api/volunteer/register`
- `POST /api/volunteer/heartbeat`
- `POST /api/volunteer/result`
- `POST /api/volunteer/result/{id}/validate`
- `GET /api/releases/worker`
- `POST /api/releases/rollout`

### Frontend

- Release management page
- Validation conflicts view
- Volunteer profile, credits, and team pages

---

## Security & Integrity Baseline

- Artifact signing and verification mandatory
- API tokens scoped to host identity + expiry
- Replay protection on result submission
- Strict clock-skew handling for assignment expiry
- Signed audit log entries for key lifecycle events (assign, submit, validate, credit)

---

## Success Metrics

- Time-to-first-work: <= 5 minutes
- Assignment completion rate: >= 90% within policy deadline
- Validated-result ratio: >= 99.5% after warm-up period
- Release rollback latency: <= 10 minutes
- 30-day active volunteer retention: >= 35%

---

## Why This Sequence

1. Distribution and updates first, because no public compute system works without reliable client lifecycle management.
2. Validation second, because untrusted hosts require consensus/reputation before results are trusted.
3. Rollout safety third, because increasing contributor count amplifies release blast radius.
4. Throughput optimization fourth, once reliability and lifecycle controls are in place.
5. Engagement last, once the core compute loop is trustworthy and stable.
