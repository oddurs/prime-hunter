# Cluster Coordination Roadmap

> **Deprecated (Feb 2026):** This roadmap has been superseded by [`architecture.md`](architecture.md) and [`network.md`](network.md). Retained for historical context. New work should reference those documents instead.

Status: **MVP complete** — workers heartbeat to PostgreSQL, block-based work claiming works.

## Completed (MVP)

- [x] `workers` table replaces in-memory fleet (survives coordinator restart)
- [x] `search_jobs` + `work_blocks` tables for block-based work distribution
- [x] `worker_heartbeat()` RPC: UPSERT + atomic pending command read/clear
- [x] `claim_work_block()` RPC: `FOR UPDATE SKIP LOCKED` for safe concurrent claiming
- [x] `reclaim_stale_blocks()` RPC: reclaim blocks from dead workers
- [x] `PgWorkerClient`: heartbeats directly to PostgreSQL (no coordinator URL needed)
- [x] `CoordinationClient` trait: both HTTP and PG clients implement `is_stop_requested()`
- [x] `darkreach work --search-job-id <ID>`: block-claiming worker loop
- [x] Dashboard HTTP handlers write to PG (backward compat with HTTP-based workers)
- [x] WebSocket `build_update` reads fleet from PG
- [x] Background task reclaims stale blocks every 30s

## Next Steps

### Persist Deployments
Migrate `DeploymentManager` to a `deployments` table so SSH deploy state survives coordinator restarts. Currently all deployment tracking is in-memory.

### Persist SearchManager
Migrate local child process tracking to `search_jobs` table. The coordinator should be able to restart without losing track of which searches are running. Search processes already heartbeat independently.

### Frontend Block Visualization
Show per-block progress bars on the search job detail page. Display which worker owns which block, completion percentage, estimated time remaining.

### Frontend Direct Supabase Queries for Fleet
Read `workers` and `search_jobs` tables directly from Supabase in the frontend (no WebSocket needed for fleet data). WebSocket can be reserved for real-time events only.

### Leader Election
Use `pg_advisory_lock` so multiple dashboard instances can run with one acting as the "active" coordinator for local process management (spawning searches, SSH deploys).

### Binary Distribution
Auto-deploy the darkreach binary to remote hosts. Currently assumes the binary is pre-installed. Could use SCP + checksum verification.

### Checkpoint Sharing
Upload checkpoint state to Supabase so block handoffs preserve partial progress. When a block is reclaimed, the new worker can resume from the previous worker's checkpoint instead of restarting the block.

### Adaptive Block Sizing
Auto-tune `block_size` based on worker throughput. Smaller blocks = better load balancing but more overhead. Larger blocks = less overhead but risk of wasted work on failure.

### Job Priority and Scheduling
Priority queue for `search_jobs`. Worker affinity — let workers prefer certain search types based on hardware (e.g., high-memory servers for factorial, many-core for kbn).

### Supabase Realtime for Fleet
Subscribe to `workers` table changes via Supabase Realtime instead of polling. Reduces dashboard latency and database load.

### Auth for Workers
Workers authenticate with a service role key or API key instead of bypassing RLS. Add `INSERT`/`UPDATE` RLS policies gated on a worker role.
