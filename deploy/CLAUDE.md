# deploy/ — Ops Domain

Deployment scripts and systemd units for fleet management.

## Files

| File | Purpose |
|------|---------|
| `deploy.sh` | SSH deployment script: installs Rust/GMP, clones/updates repo, builds release binary, copies to `/usr/local/bin`, installs systemd units |
| `production-deploy.sh` | Full production setup: swap, UFW, kernel tuning, nginx, systemd, frontend deploy, search launch, verification |
| `nginx-primehunt.conf` | Nginx reverse proxy: rate limiting, WebSocket, static caching, security headers, gzip |
| `primehunt-coordinator.service` | Systemd unit for dashboard: port 8080, static dir serving, security-hardened (strict filesystem, no new privs, 512M memory limit, 65k file descriptors) |
| `primehunt-worker@.service` | Template unit for workers: runs search with `--coordinator` flag, supports instance numbers (`%i`), auto-restarts every 10s |

## Deployment Flow

```
deploy.sh → SSH to target → install deps (Rust, GMP) → clone/pull repo
  → cargo build --release → cp binary to /usr/local/bin
  → systemctl enable/start coordinator + worker units
```

## Fleet Architecture

```
Coordinator (1 instance)
  ├── Serves web dashboard on port 8080
  ├── Manages worker registry (in-memory, 60s stale timeout)
  └── Aggregates results into shared SQLite DB

Workers (N instances via template unit)
  ├── Register with coordinator via HTTP heartbeat (10s interval)
  ├── Run search subcommands (factorial/palindromic/kbn)
  └── Report discovered primes to coordinator
```

Multiple worker instances on one host: `systemctl start primehunt-worker@1 primehunt-worker@2`

## Build Flags

Release profile in `Cargo.toml`:
```toml
[profile.release]
lto = "fat"           # Whole-program link-time optimization
codegen-units = 1     # Single codegen unit for maximum optimization
opt-level = 3         # Maximum optimization
```

**Apple Silicon note:** Do NOT use `-Ctarget-cpu=native` (Rust bug #93889 resolves to wrong chip). Use `-Ctarget-cpu=apple-m1` explicitly.

## Roadmap

See `docs/roadmaps/ops.md` for planned improvements: Apple Silicon optimization, GPU sieving, PGO builds, fleet automation.
