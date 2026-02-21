# deploy/ — Ops Domain

Deployment scripts, systemd units, Nginx config, Grafana dashboards, Helm charts, and Terraform infrastructure for the darkreach fleet.

## Directory Structure

```
deploy/
├── deploy.sh                      # Quick SSH deployment (install deps, build, deploy)
├── production-deploy.sh           # Full production setup (swap, UFW, kernel, nginx, systemd)
├── worker-deploy.sh               # Worker-specific deployment script
├── pgo-build.sh                   # Profile-Guided Optimization build
├── nginx-darkreach.conf           # Nginx reverse proxy (rate limit, WebSocket, TLS)
├── darkreach-coordinator.service  # Systemd coordinator unit (port 7001, security-hardened)
├── darkreach-worker@.service      # Systemd worker template (auto-restart, instance %i)
├── grafana/
│   └── darkreach.json             # Grafana dashboard definition
├── helm/
│   └── darkreach/
│       ├── Chart.yaml             # Helm chart metadata
│       ├── values.yaml            # Default values
│       ├── values-production.yaml # Production overrides
│       └── templates/
│           ├── coordinator-deployment.yaml
│           ├── coordinator-service.yaml
│           ├── coordinator-ingress.yaml
│           ├── worker-deployment.yaml
│           ├── configmap.yaml
│           ├── secrets.yaml
│           ├── servicemonitor.yaml # Prometheus ServiceMonitor
│           └── keda-scaledobject.yaml # KEDA autoscaling
├── terraform/
│   ├── environments/              # Per-environment tfvars
│   └── modules/                   # Terraform modules
└── releases/
    └── worker-manifest.json       # Worker binary release manifest
```

## Files

| File | Purpose |
|------|---------|
| `deploy.sh` | Quick SSH deploy: install Rust/GMP, clone/update, build release, copy to `/usr/local/bin`, install systemd units |
| `production-deploy.sh` | Full production: swap, UFW, kernel tuning, nginx, systemd, frontend deploy, search launch, verification |
| `worker-deploy.sh` | Node deployment with coordinator URL config (legacy; new nodes use `darkreach run`) |
| `pgo-build.sh` | PGO build: instrument → profile run → optimized build (5-15% speedup) |
| `nginx-darkreach.conf` | Nginx: rate limiting, WebSocket upgrade, static caching, security headers, gzip |
| `darkreach-coordinator.service` | Coordinator: port 7001, strict filesystem, no new privs, 512M memory limit, 65k fds |
| `darkreach-worker@.service` | Node template: `%i` instance, `--coordinator` flag, auto-restart 10s |

## Deployment Flow

```
deploy.sh → SSH to target → install deps (Rust, GMP)
  → clone/pull repo → cargo build --release
  → cp binary to /usr/local/bin/darkreach
  → systemctl enable/start coordinator + worker units
```

## Architecture Migration Note

> **Note:** The architecture is migrating from fleet/worker terminology to network/node terminology. Nodes now self-update via `darkreach run` instead of requiring SSH deployment. Old deployment scripts (`worker-deploy.sh`) remain for legacy setups but new nodes use the auto-update mechanism. See `docs/roadmaps/architecture.md`.

## Fleet Architecture

```
Coordinator (1 instance, CX22)
  ├── Serves dashboard on port 7001
  ├── Manages worker registry (in-memory, 60s stale timeout)
  ├── REST API + WebSocket for frontend
  └── PostgreSQL-based work distribution (Supabase)

Nodes (N instances, CCX23 4-node)
  ├── Register via HTTP heartbeat (10s interval)
  ├── Claim work blocks via FOR UPDATE SKIP LOCKED
  ├── Run search subcommands
  ├── Self-update via `darkreach run` auto-updater
  └── Report primes to coordinator + database
```

Multiple workers on one host: `systemctl start darkreach-worker@1 darkreach-worker@2`

## Build Optimization

Release profile in `Cargo.toml`:
```toml
[profile.release]
lto = "fat"           # Whole-program link-time optimization
codegen-units = 1     # Single codegen unit for maximum optimization
opt-level = 3         # Maximum optimization
```

**Apple Silicon:** Use `-Ctarget-cpu=apple-m1`, NOT `-Ctarget-cpu=native` (Rust bug #93889).

**PGO build:** `./deploy/pgo-build.sh` (instrument → profile → optimize).

**mimalloc:** Global allocator configured in `main.rs`.

## Kubernetes (Helm)

```bash
# Deploy to Kubernetes
helm install darkreach deploy/helm/darkreach/ -f deploy/helm/darkreach/values-production.yaml

# Worker autoscaling via KEDA (based on work_blocks queue depth)
# Prometheus metrics via ServiceMonitor
```

## Monitoring

- **Grafana dashboard**: `deploy/grafana/darkreach.json` (import to Grafana)
- **Prometheus metrics**: `darkreach_*` prefix, scraped via ServiceMonitor
- **Metric types**: candidates tested, primes found, tests/sec, sieve efficiency, worker health

## Agent Coding Guide

### Adding a new deployment target

1. Create deployment script in `deploy/<target>-deploy.sh`
2. Create systemd unit if long-running: `deploy/darkreach-<service>.service`
3. Update Helm chart if Kubernetes: add template in `deploy/helm/darkreach/templates/`
4. Update Terraform if cloud infra: add module in `deploy/terraform/modules/`

### Adding monitoring

1. Add Prometheus metric in `src/prom_metrics.rs` (prefix: `darkreach_`)
2. Add panel to Grafana dashboard in `deploy/grafana/darkreach.json`
3. Add alert rule if critical

### Node release management

- Release manifest: `deploy/releases/worker-manifest.json`
- Nodes auto-update via release channel (see `src/db/releases.rs`)
- Dashboard UI at `/releases` page

## Roadmap

See `docs/roadmaps/ops.md` for planned improvements.
